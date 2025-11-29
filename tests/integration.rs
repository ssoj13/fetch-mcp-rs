//! Integration tests for stdio MCP transport mode

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot, Mutex};
use uuid::Uuid;

/// Get the path to the compiled binary
fn assert_cmd() -> String {
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // remove test executable name
    path.pop(); // remove "deps"
    path.push("fetch-mcp-rs");
    #[cfg(windows)]
    path.set_extension("exe");

    // If not found in debug, try release
    if !path.exists() {
        path.pop();
        path.pop(); // remove "debug"
        path.push("release");
        path.push("fetch-mcp-rs");
        #[cfg(windows)]
        path.set_extension("exe");
    }

    path.to_string_lossy().to_string()
}

/// Spawn the fetch MCP server binary with given args
async fn spawn_server(args: &[&str]) -> Result<ServerHandle> {
    let mut cmd = Command::new(assert_cmd());
    cmd.args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit());

    let mut child = cmd.spawn()?;
    let stdout = child.stdout.take().unwrap();
    let mut stdin = child.stdin.take().unwrap();

    let (tx_out, mut rx_out) = mpsc::channel::<serde_json::Value>(32);
    let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));

    // Writer task
    tokio::spawn(async move {
        while let Some(msg) = rx_out.recv().await {
            if let Ok(line) = serde_json::to_string(&msg) {
                let _ = stdin.write_all(line.as_bytes()).await;
                let _ = stdin.write_all(b"\n").await;
                let _ = stdin.flush().await;
            }
        }
    });

    // Reader task
    {
        let pending = pending.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                    if let Some(id) = v.get("id").and_then(|x| x.as_str()) {
                        if let Some(waiter) = pending.lock().await.remove(id) {
                            let _ = waiter.send(v);
                            continue;
                        }
                    }
                    // Notifications are ignored for now
                }
            }
        });
    }

    Ok(ServerHandle {
        child,
        tx_out,
        pending,
    })
}

type PendingMap = Arc<Mutex<HashMap<String, oneshot::Sender<serde_json::Value>>>>;

struct ServerHandle {
    child: Child,
    tx_out: mpsc::Sender<serde_json::Value>,
    pending: PendingMap,
}

impl ServerHandle {
    async fn request(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        let id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id.clone(), tx);
        self.tx_out
            .send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .await?;
        let resp = rx.await?;
        Ok(resp)
    }

    async fn call_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.request(
            "tools/call",
            json!({
                "name": name,
                "arguments": arguments
            }),
        )
        .await
    }

    async fn notify(&self, method: &str, params: serde_json::Value) -> Result<()> {
        self.tx_out
            .send(json!({"jsonrpc":"2.0","method":method,"params":params}))
            .await?;
        Ok(())
    }

    async fn initialize(&self) -> Result<serde_json::Value> {
        let resp = self.request(
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "test-client",
                    "version": "1.0.0"
                }
            }),
        )
        .await?;

        // Send initialized notification
        self.notify("initialized", json!({})).await?;

        Ok(resp)
    }

    async fn shutdown(&mut self) -> Result<()> {
        let _ = self.request("shutdown", json!({})).await;
        self.child.kill().await?;
        Ok(())
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

/// Test MCP handshake (initialize)
#[tokio::test]
async fn test_mcp_handshake() -> Result<()> {
    let server = spawn_server(&[]).await?;

    let init_resp = server.initialize().await?;

    assert!(init_resp.get("result").is_some());
    let result = &init_resp["result"];
    assert_eq!(result["protocolVersion"], "2024-11-05");
    assert!(result["serverInfo"]["name"].as_str().unwrap().contains("fetch-mcp-rs"));
    assert!(result["capabilities"]["tools"].is_object());

    Ok(())
}

/// Test listing available tools
#[tokio::test]
async fn test_list_tools() -> Result<()> {
    let server = spawn_server(&[]).await?;
    server.initialize().await?;

    let resp = server.request("tools/list", json!({})).await?;

    assert!(resp.get("result").is_some());
    let tools = resp["result"]["tools"].as_array().unwrap();

    // Should have 13+ tools
    assert!(tools.len() >= 13);

    // Check for key tools
    let tool_names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();

    assert!(tool_names.contains(&"fetch"));
    assert!(tool_names.contains(&"batch_fetch"));
    assert!(tool_names.contains(&"search_in_page"));
    assert!(tool_names.contains(&"extract_links"));

    Ok(())
}

/// Test fetch tool with real URL
#[tokio::test]
async fn test_fetch_tool() -> Result<()> {
    let server = spawn_server(&[]).await?;
    server.initialize().await?;

    let resp = server.call_tool(
        "fetch",
        json!({
            "url": "https://httpbin.org/html"
        }),
    ).await?;

    assert!(resp.get("result").is_some());
    let content = &resp["result"]["content"];
    assert!(content.is_array());

    let text = content[0]["text"].as_str().unwrap();
    assert!(text.contains("<html") || text.contains("<!DOCTYPE"));

    Ok(())
}

/// Test fetch with invalid URL
#[tokio::test]
async fn test_fetch_invalid_url() -> Result<()> {
    let server = spawn_server(&[]).await?;
    server.initialize().await?;

    let resp = server.call_tool(
        "fetch",
        json!({
            "url": "not-a-valid-url"
        }),
    ).await?;

    // Should return error
    assert!(resp.get("result").is_some());
    let result = &resp["result"];
    assert_eq!(result["isError"], true);

    Ok(())
}

/// Test batch_fetch with multiple URLs
#[tokio::test]
async fn test_batch_fetch() -> Result<()> {
    let server = spawn_server(&[]).await?;
    server.initialize().await?;

    let resp = server.call_tool(
        "batch_fetch",
        json!({
            "urls": [
                "https://httpbin.org/status/200",
                "https://httpbin.org/status/404"
            ],
            "max_concurrent": 2
        }),
    ).await?;

    assert!(resp.get("result").is_some());
    let content = &resp["result"]["content"];
    assert!(content.is_array());

    let text = content[0]["text"].as_str().unwrap();
    let data: serde_json::Value = serde_json::from_str(text)?;

    assert!(data["results"].is_array());
    assert_eq!(data["results"].as_array().unwrap().len(), 2);
    assert!(data["stats"].is_object());
    assert_eq!(data["stats"]["total"], 2);

    Ok(())
}

/// Test search_in_page
#[tokio::test]
async fn test_search_in_page() -> Result<()> {
    let server = spawn_server(&[]).await?;
    server.initialize().await?;

    // First fetch a page
    let fetch_resp = server.call_tool(
        "fetch",
        json!({
            "url": "https://httpbin.org/html"
        }),
    ).await?;

    let html = fetch_resp["result"]["content"][0]["text"].as_str().unwrap();

    // Now search in it
    let search_resp = server.call_tool(
        "search_in_page",
        json!({
            "content": html,
            "query": "html"
        }),
    ).await?;

    assert!(search_resp.get("result").is_some());
    let content = &search_resp["result"]["content"];
    let text = content[0]["text"].as_str().unwrap();
    let data: serde_json::Value = serde_json::from_str(text)?;

    assert!(data["total_matches"].as_u64().unwrap() > 0);

    Ok(())
}

/// Test extract_links
#[tokio::test]
async fn test_extract_links() -> Result<()> {
    let server = spawn_server(&[]).await?;
    server.initialize().await?;

    let html = r#"
        <html>
            <body>
                <a href="/relative">Relative</a>
                <a href="https://example.com/external">External</a>
            </body>
        </html>
    "#;

    let resp = server.call_tool(
        "extract_links",
        json!({
            "html": html,
            "base_url": "https://test.com"
        }),
    ).await?;

    assert!(resp.get("result").is_some());
    let content = &resp["result"]["content"];
    let text = content[0]["text"].as_str().unwrap();
    let links: Vec<serde_json::Value> = serde_json::from_str(text)?;

    assert_eq!(links.len(), 2);

    Ok(())
}

/// Test convert_html
#[tokio::test]
async fn test_convert_html() -> Result<()> {
    let server = spawn_server(&[]).await?;
    server.initialize().await?;

    let html = r#"
        <html>
            <body>
                <h1>Title</h1>
                <p>Paragraph</p>
            </body>
        </html>
    "#;

    let resp = server.call_tool(
        "convert_html",
        json!({
            "html": html,
            "format": "text"
        }),
    ).await?;

    assert!(resp.get("result").is_some());
    let content = &resp["result"]["content"];
    let text = content[0]["text"].as_str().unwrap();

    assert!(text.contains("Title") || text.contains("Paragraph"));

    Ok(())
}

/// Test extract_metadata
#[tokio::test]
async fn test_extract_metadata() -> Result<()> {
    let server = spawn_server(&[]).await?;
    server.initialize().await?;

    let html = r#"
        <html>
            <head>
                <title>Test Page</title>
                <meta name="description" content="Test description">
            </head>
            <body></body>
        </html>
    "#;

    let resp = server.call_tool(
        "extract_metadata",
        json!({
            "html": html,
            "url": "https://example.com"
        }),
    ).await?;

    assert!(resp.get("result").is_some());
    let content = &resp["result"]["content"];
    let text = content[0]["text"].as_str().unwrap();
    let data: serde_json::Value = serde_json::from_str(text)?;

    assert_eq!(data["title"], "Test Page");
    assert_eq!(data["description"], "Test description");

    Ok(())
}

/// Test extract_with_selector
#[tokio::test]
async fn test_extract_with_selector() -> Result<()> {
    let server = spawn_server(&[]).await?;
    server.initialize().await?;

    let html = r#"
        <html>
            <body>
                <div class="content">Hello World</div>
            </body>
        </html>
    "#;

    let resp = server.call_tool(
        "extract_with_selector",
        json!({
            "html": html,
            "selector": "div.content"
        }),
    ).await?;

    assert!(resp.get("result").is_some());
    let content = &resp["result"]["content"];
    let text = content[0]["text"].as_str().unwrap();
    let matches: Vec<String> = serde_json::from_str(text)?;

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0], "Hello World");

    Ok(())
}

/// Test server shutdown
#[tokio::test]
async fn test_shutdown() -> Result<()> {
    let mut server = spawn_server(&[]).await?;
    server.initialize().await?;

    server.shutdown().await?;

    Ok(())
}
