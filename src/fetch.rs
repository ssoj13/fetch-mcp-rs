use anyhow::{Context, Result};
use bytes::Bytes;
use cached::proc_macro::cached;
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// User agent for autonomous fetching (via tool)
pub const DEFAULT_USER_AGENT_AUTONOMOUS: &str =
    "ModelContextProtocol/1.0 (Autonomous; +https://github.com/modelcontextprotocol/servers)";

/// User agent for manual fetching (via prompt)
pub const DEFAULT_USER_AGENT_MANUAL: &str =
    "ModelContextProtocol/1.0 (User-Specified; +https://github.com/modelcontextprotocol/servers)";

/// Create HTTP client with common settings
pub fn create_client(proxy_url: Option<&str>, user_agent: &str) -> Result<Client> {
    let mut builder = Client::builder()
        .user_agent(user_agent)
        .timeout(Duration::from_secs(30))
        .gzip(true)
        .brotli(true)
        .cookie_store(true)
        .redirect(reqwest::redirect::Policy::limited(10));

    if let Some(proxy) = proxy_url {
        builder = builder.proxy(reqwest::Proxy::all(proxy).context("Invalid proxy URL")?);
    }

    builder.build().context("Failed to create HTTP client")
}

/// Fetch URL and return raw response
pub async fn fetch_url_raw(client: &Client, url: &str) -> Result<Response> {
    tracing::debug!("Fetching URL: {}", url);

    let response = client
        .get(url)
        .send()
        .await
        .context(format!("Failed to fetch {}", url))?;

    if !response.status().is_success() {
        anyhow::bail!("HTTP {} for {}", response.status(), url);
    }

    Ok(response)
}

/// Fetch URL and return text content
pub async fn fetch_url_text(client: &Client, url: &str) -> Result<String> {
    let response = fetch_url_raw(client, url).await?;
    let text = response.text().await.context("Failed to read response text")?;
    Ok(text)
}

/// Fetch URL and return bytes
pub async fn fetch_url_bytes(client: &Client, url: &str) -> Result<Bytes> {
    let response = fetch_url_raw(client, url).await?;
    let bytes = response.bytes().await.context("Failed to read response bytes")?;
    Ok(bytes)
}

/// Cached fetch with 5-minute TTL (300 seconds)
/// Cache key = URL, cache stores text content
#[cached(
    time = 300,
    size = 100,
    key = "String",
    convert = r#"{ url.to_string() }"#,
    result = true
)]
pub async fn fetch_url_cached(client: &Client, url: &str) -> Result<String> {
    tracing::debug!("Cache miss for {}, fetching...", url);
    fetch_url_text(client, url).await
}

/// Content type detection result
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum ContentType {
    Html,
    Json,
    Xml,
    Feed,
    Pdf,
    Image,
    Text,
}

/// Detect content type from response headers and body
#[allow(dead_code)]
pub fn detect_content_type(response: &reqwest::Response, body: &str) -> ContentType {
    let content_type_header = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    // Check HTML
    if content_type_header.contains("text/html") || body.trim_start().starts_with("<!DOCTYPE") || body.trim_start().starts_with("<html") {
        return ContentType::Html;
    }

    // Check JSON
    if content_type_header.contains("application/json") || body.trim_start().starts_with('{') || body.trim_start().starts_with('[') {
        return ContentType::Json;
    }

    // Check XML
    if content_type_header.contains("application/xml")
        || content_type_header.contains("text/xml")
        || body.trim_start().starts_with("<?xml")
    {
        return ContentType::Xml;
    }

    // Check RSS/Atom
    if body.contains("<rss") || body.contains("<feed") {
        return ContentType::Feed;
    }

    // Check PDF
    if content_type_header.contains("application/pdf") || body.starts_with("%PDF") {
        return ContentType::Pdf;
    }

    // Check images
    if content_type_header.starts_with("image/") {
        return ContentType::Image;
    }

    ContentType::Text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_url() {
        let client = create_client(None, DEFAULT_USER_AGENT_AUTONOMOUS).unwrap();
        let result = fetch_url_text(&client, "https://httpbin.org/html").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_content_type_detection() {
        let html = "<!DOCTYPE html><html><body>Test</body></html>";
        let client = create_client(None, DEFAULT_USER_AGENT_AUTONOMOUS).unwrap();
        let response = client.get("https://httpbin.org/html").send().await.unwrap();
        let ct = detect_content_type(&response, html);
        assert_eq!(ct, ContentType::Html);
    }
}
