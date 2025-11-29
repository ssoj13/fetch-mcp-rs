mod batch;
mod feed;
mod fetch;
mod html_convert;
mod image;
mod links;
mod logging;
mod metadata;
mod pdf;
mod reddit;
mod robots;
mod search;
mod selector;
mod sitemap;
mod validation;
mod wiki;

use anyhow::Result;
use clap::Parser;
use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
    transport::stdio,
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

/// Fetch MCP Server - Advanced web content fetching with 13+ tools
#[derive(Parser, Debug)]
#[command(name = "fetch-mcp-rs")]
#[command(about = "MCP server for advanced web content fetching", long_about = None)]
struct Cli {
    /// User agent string for HTTP requests
    #[arg(long)]
    user_agent: Option<String>,

    /// Ignore robots.txt restrictions (use with caution)
    #[arg(long)]
    ignore_robots_txt: bool,

    /// HTTP proxy URL (e.g., http://proxy:8080)
    #[arg(long)]
    proxy_url: Option<String>,

    /// Log file path (optional, for debugging)
    #[arg(long)]
    log_file: Option<PathBuf>,

    /// Enable HTTP stream mode on specified port (default: stdio mode)
    #[arg(long)]
    port: Option<u16>,
}

/// Global server state
struct ServerState {
    client: reqwest::Client,
    user_agent: String,
    ignore_robots: bool,
}

impl ServerState {
    fn new(user_agent: String, ignore_robots: bool, proxy_url: Option<&str>) -> Result<Self> {
        let client = fetch::create_client(proxy_url, &user_agent)?;
        Ok(Self {
            client,
            user_agent,
            ignore_robots,
        })
    }
}

#[derive(Clone)]
struct FetchServer {
    state: Arc<ServerState>,
    tool_router: ToolRouter<Self>,
}

impl FetchServer {
    fn new(state: Arc<ServerState>) -> Self {
        Self {
            state,
            tool_router: Self::tool_router(),
        }
    }

    fn server_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: Default::default(),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation {
                name: "fetch-mcp-rs".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                title: Some("Advanced Web Content Fetching Server".to_string()),
                website_url: Some("https://github.com/ssoj13/fetch-mcp-rs".to_string()),
                icons: None,
            },
            instructions: None,
        }
    }
}

// Helper for internal errors
fn internal_err<T: ToString>(msg: &'static str) -> impl FnOnce(T) -> McpError + Clone {
    move |err| McpError::internal_error(msg, Some(serde_json::json!({ "error": err.to_string() })))
}

// ============================================================================
// Tool Input Schemas
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
struct FetchArgs {
    url: String,
    #[serde(default)]
    max_length: Option<usize>,
    #[serde(default)]
    start_index: Option<usize>,
    #[serde(default)]
    raw: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct FetchMetadataArgs {
    url: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct FetchFeedArgs {
    url: String,
    #[serde(default = "default_max_items")]
    max_items: usize,
}

fn default_max_items() -> usize {
    20
}

#[derive(Debug, Deserialize, JsonSchema)]
struct FetchWithSelectorArgs {
    url: String,
    selector: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ExtractTableArgs {
    url: String,
    table_selector: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct FetchSitemapArgs {
    url: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct FetchLinksArgs {
    url: String,
    #[serde(default)]
    internal_only: bool,
    #[serde(default)]
    external_only: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct FetchBatchArgs {
    urls: Vec<String>,
    #[serde(default = "default_max_concurrent")]
    max_concurrent: usize,
    #[serde(default = "default_rate_limit")]
    rate_limit: Option<u32>,
}

fn default_max_concurrent() -> usize {
    5
}

fn default_rate_limit() -> Option<u32> {
    Some(10)
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SearchInPageArgs {
    url: String,
    query: String,
    #[serde(default)]
    case_sensitive: bool,
    #[serde(default)]
    use_regex: bool,
    #[serde(default = "default_max_matches")]
    max_matches: usize,
    #[serde(default)]
    extract_words: bool,
}

fn default_max_matches() -> usize {
    100
}

#[derive(Debug, Deserialize, JsonSchema)]
struct RedditArgs {
    query: Option<String>,
    #[serde(default = "default_subreddit")]
    subreddit: String,
    #[serde(default = "default_sort")]
    sort: String,
    time_filter: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    include_comments: bool,
}

fn default_subreddit() -> String {
    "all".to_string()
}

fn default_sort() -> String {
    "hot".to_string()
}

fn default_limit() -> usize {
    25
}

#[derive(Debug, Deserialize, JsonSchema)]
struct WikiArgs {
    query: String,
    #[serde(default = "default_language")]
    language: String,
    #[serde(default = "default_action")]
    action: String,
    #[serde(default = "default_wiki_limit")]
    limit: usize,
    #[serde(default = "default_extract_images")]
    extract_images: bool,
}

fn default_language() -> String {
    "en".to_string()
}

fn default_action() -> String {
    "summary".to_string()
}

fn default_wiki_limit() -> usize {
    10
}

fn default_extract_images() -> bool {
    true
}

#[cfg(feature = "pdf")]
#[derive(Debug, Deserialize, JsonSchema)]
struct FetchPdfArgs {
    url: String,
    #[serde(default)]
    max_pages: Option<usize>,
}

#[cfg(feature = "images")]
#[derive(Debug, Deserialize, JsonSchema)]
struct FetchImageArgs {
    url: String,
}

// ============================================================================
// Tool Implementations
// ============================================================================

#[tool_router]
impl FetchServer {
    /// Fetch URL content and convert HTML to Markdown
    #[tool(name = "fetch", description = "Fetch URL content and convert HTML to Markdown using Readability algorithm")]
    async fn fetch(&self, Parameters(args): Parameters<FetchArgs>) -> Result<CallToolResult, McpError> {
        // Validate URL format
        let url = validation::validate_url(&args.url)
            .map_err(internal_err("URL validation failed"))?;

        // Validate robots.txt if not ignored
        if !self.state.ignore_robots {
            robots::check_robots_txt_allowed(&self.state.client, &url, &self.state.user_agent)
                .await
                .map_err(internal_err("robots.txt check failed"))?;
        }

        let html = fetch::fetch_url_text(&self.state.client, &url)
            .await
            .map_err(internal_err("Failed to fetch URL"))?;

        let content = if args.raw {
            let text = if let Some(max_len) = args.max_length {
                let start = args.start_index.unwrap_or(0);
                let end = (start + max_len).min(html.len());
                html[start..end].to_string()
            } else {
                html
            };
            json!({"content": text, "raw": true})
        } else {
            let markdown = html_convert::html_to_markdown(&html, &args.url)
                .map_err(internal_err("Failed to convert HTML"))?;
            let text = if let Some(max_len) = args.max_length {
                let start = args.start_index.unwrap_or(0);
                let end = (start + max_len).min(markdown.len());
                markdown[start..end].to_string()
            } else {
                markdown
            };
            json!({"content": text, "url": args.url})
        };

        Ok(CallToolResult {
            content: vec![Content::text(content.to_string())],
            structured_content: None,
            is_error: None,
            meta: None,
        })
    }

    /// Extract Open Graph metadata
    #[tool(name = "fetch_metadata", description = "Extract Open Graph, Schema.org, and HTML metadata from a URL")]
    async fn fetch_metadata(&self, Parameters(args): Parameters<FetchMetadataArgs>) -> Result<CallToolResult, McpError> {
        // Validate URL
        let url = validation::validate_url(&args.url)
            .map_err(internal_err("URL validation failed"))?;

        let html = fetch::fetch_url_text(&self.state.client, &url)
            .await
            .map_err(internal_err("Failed to fetch URL"))?;

        let metadata = metadata::extract_metadata(&html, &url)
            .map_err(internal_err("Failed to extract metadata"))?;

        let result = serde_json::to_string_pretty(&metadata)
            .map_err(internal_err("Failed to serialize metadata"))?;

        Ok(CallToolResult {
            content: vec![Content::text(result)],
            structured_content: None,
            is_error: None,
            meta: None,
        })
    }

    /// Parse RSS/Atom feed
    #[tool(name = "fetch_feed", description = "Parse RSS, Atom, or JSON Feed from a URL")]
    async fn fetch_feed(&self, Parameters(args): Parameters<FetchFeedArgs>) -> Result<CallToolResult, McpError> {
        // Validate URL
        let url = validation::validate_url(&args.url)
            .map_err(internal_err("URL validation failed"))?;

        let content = fetch::fetch_url_text(&self.state.client, &url)
            .await
            .map_err(internal_err("Failed to fetch URL"))?;

        let feed_info = feed::parse_feed(&content, args.max_items)
            .map_err(internal_err("Failed to parse feed"))?;

        let result = serde_json::to_string_pretty(&feed_info)
            .map_err(internal_err("Failed to serialize feed"))?;

        Ok(CallToolResult {
            content: vec![Content::text(result)],
            structured_content: None,
            is_error: None,
            meta: None,
        })
    }

    /// Extract elements using CSS selector
    #[tool(name = "fetch_with_selector", description = "Fetch URL and extract elements using CSS selector")]
    async fn fetch_with_selector(&self, Parameters(args): Parameters<FetchWithSelectorArgs>) -> Result<CallToolResult, McpError> {
        // Validate URL and selector
        let url = validation::validate_url(&args.url)
            .map_err(internal_err("URL validation failed"))?;
        let selector = validation::validate_selector(&args.selector)
            .map_err(internal_err("Selector validation failed"))?;

        let html = fetch::fetch_url_text(&self.state.client, &url)
            .await
            .map_err(internal_err("Failed to fetch URL"))?;

        let elements = selector::select_elements(&html, &selector)
            .map_err(internal_err("Failed to select elements"))?;

        let result = serde_json::to_string_pretty(&elements)
            .map_err(internal_err("Failed to serialize elements"))?;

        Ok(CallToolResult {
            content: vec![Content::text(result)],
            structured_content: None,
            is_error: None,
            meta: None,
        })
    }

    /// Extract tables from HTML
    #[tool(name = "extract_table", description = "Extract tables from HTML page")]
    async fn extract_table(&self, Parameters(args): Parameters<ExtractTableArgs>) -> Result<CallToolResult, McpError> {
        // Validate URL
        let url = validation::validate_url(&args.url)
            .map_err(internal_err("URL validation failed"))?;

        let html = fetch::fetch_url_text(&self.state.client, &url)
            .await
            .map_err(internal_err("Failed to fetch URL"))?;

        let tables = selector::extract_table(&html, args.table_selector.as_deref())
            .map_err(internal_err("Failed to extract tables"))?;

        let result = serde_json::to_string_pretty(&tables)
            .map_err(internal_err("Failed to serialize tables"))?;

        Ok(CallToolResult {
            content: vec![Content::text(result)],
            structured_content: None,
            is_error: None,
            meta: None,
        })
    }

    /// Parse sitemap.xml
    #[tool(name = "fetch_sitemap", description = "Parse sitemap.xml or sitemap index from a URL")]
    async fn fetch_sitemap(&self, Parameters(args): Parameters<FetchSitemapArgs>) -> Result<CallToolResult, McpError> {
        // Validate URL
        let url = validation::validate_url(&args.url)
            .map_err(internal_err("URL validation failed"))?;

        let xml = fetch::fetch_url_text(&self.state.client, &url)
            .await
            .map_err(internal_err("Failed to fetch URL"))?;

        let sitemap_data = sitemap::parse_sitemap(&xml)
            .map_err(internal_err("Failed to parse sitemap"))?;

        let result = serde_json::to_string_pretty(&sitemap_data)
            .map_err(internal_err("Failed to serialize sitemap"))?;

        Ok(CallToolResult {
            content: vec![Content::text(result)],
            structured_content: None,
            is_error: None,
            meta: None,
        })
    }

    /// Extract links from webpage
    #[tool(name = "fetch_links", description = "Extract all links from a webpage with optional filtering (internal/external)")]
    async fn fetch_links(&self, Parameters(args): Parameters<FetchLinksArgs>) -> Result<CallToolResult, McpError> {
        // Validate URL
        let url = validation::validate_url(&args.url)
            .map_err(internal_err("URL validation failed"))?;

        let html = fetch::fetch_url_text(&self.state.client, &url)
            .await
            .map_err(internal_err("Failed to fetch URL"))?;

        // Use convenience wrappers if specific filtering requested
        let links_data = if args.internal_only && !args.external_only {
            links::extract_internal_links(&html, &url)
                .map_err(internal_err("Failed to extract internal links"))?
        } else if args.external_only && !args.internal_only {
            links::extract_external_links(&html, &url)
                .map_err(internal_err("Failed to extract external links"))?
        } else {
            let options = links::LinkExtractionOptions {
                internal_only: args.internal_only,
                external_only: args.external_only,
                deduplicate: true,
            };
            links::extract_links(&html, &url, options)
                .map_err(internal_err("Failed to extract links"))?
        };

        let result = serde_json::to_string_pretty(&links_data)
            .map_err(internal_err("Failed to serialize links"))?;

        Ok(CallToolResult {
            content: vec![Content::text(result)],
            structured_content: None,
            is_error: None,
            meta: None,
        })
    }

    /// Batch fetch multiple URLs
    #[tool(name = "fetch_batch", description = "Fetch multiple URLs in parallel with rate limiting and concurrency control")]
    async fn fetch_batch(&self, Parameters(args): Parameters<FetchBatchArgs>) -> Result<CallToolResult, McpError> {
        // Validate URLs array size
        validation::validate_array_size(&args.urls, 100, "URLs")
            .map_err(internal_err("Array validation failed"))?;

        // Validate each URL
        let validated_urls: Result<Vec<_>, _> = args.urls
            .iter()
            .map(|url| validation::validate_url(url))
            .collect();
        let urls = validated_urls.map_err(internal_err("URL validation failed"))?;

        let options = batch::BatchOptions {
            max_concurrent: args.max_concurrent,
            rate_limit: args.rate_limit,
            timeout: std::time::Duration::from_secs(30),
            fail_fast: false,
            follow_redirects: true,
        };

        let batch_result = batch::fetch_batch(&self.state.client, urls, options)
            .await
            .map_err(internal_err("Failed to batch fetch"))?;

        let result = serde_json::to_string_pretty(&batch_result)
            .map_err(internal_err("Failed to serialize batch results"))?;

        Ok(CallToolResult {
            content: vec![Content::text(result)],
            structured_content: None,
            is_error: None,
            meta: None,
        })
    }

    /// Search in page content
    #[tool(name = "search_in_page", description = "Search for text or regex pattern in page content with context")]
    async fn search_in_page(&self, Parameters(args): Parameters<SearchInPageArgs>) -> Result<CallToolResult, McpError> {
        // Validate URL
        let url = validation::validate_url(&args.url)
            .map_err(internal_err("URL validation failed"))?;

        // Validate regex if used
        let query = if args.use_regex {
            validation::validate_regex(&args.query)
                .map_err(internal_err("Regex validation failed"))?
        } else {
            args.query.clone()
        };

        let html = fetch::fetch_url_text(&self.state.client, &url)
            .await
            .map_err(internal_err("Failed to fetch URL"))?;

        let text = html_convert::html_to_text(&html);

        let options = search::SearchOptions {
            case_sensitive: args.case_sensitive,
            use_regex: args.use_regex,
            max_matches: args.max_matches,
            context_chars: 50,
            line_filter: None,
            extract_words: args.extract_words,
        };

        let search_result = search::search_in_text(&text, &query, options)
            .map_err(internal_err("Failed to search"))?;

        let result = serde_json::to_string_pretty(&search_result)
            .map_err(internal_err("Failed to serialize search results"))?;

        Ok(CallToolResult {
            content: vec![Content::text(result)],
            structured_content: None,
            is_error: None,
            meta: None,
        })
    }

    /// Fetch Reddit posts
    #[tool(name = "reddit", description = "Search or fetch posts from Reddit with optional comments")]
    async fn reddit(&self, Parameters(args): Parameters<RedditArgs>) -> Result<CallToolResult, McpError> {
        // Validate subreddit
        let subreddit = validation::validate_subreddit(&args.subreddit)
            .map_err(internal_err("Subreddit validation failed"))?;

        // Validate sort
        let sort = validation::validate_reddit_sort(&args.sort)
            .map_err(internal_err("Sort validation failed"))?;

        // Validate time filter
        let time_filter = validation::validate_reddit_time(args.time_filter.as_deref())
            .map_err(internal_err("Time filter validation failed"))?;

        // Validate limit
        let limit = validation::validate_limit(args.limit, 100)
            .map_err(internal_err("Limit validation failed"))?;

        let options = reddit::RedditOptions {
            subreddit,
            sort,
            time_filter,
            limit,
            include_comments: args.include_comments,
            max_comments: 10,
        };

        let posts = reddit::fetch_reddit_posts(&self.state.client, args.query.as_deref(), options)
            .await
            .map_err(internal_err("Failed to fetch Reddit posts"))?;

        let result = serde_json::to_string_pretty(&posts)
            .map_err(internal_err("Failed to serialize posts"))?;

        Ok(CallToolResult {
            content: vec![Content::text(result)],
            structured_content: None,
            is_error: None,
            meta: None,
        })
    }

    /// Wikipedia search/article
    #[tool(name = "wiki", description = "Search Wikipedia, get article summary/content, or get random article")]
    async fn wiki(&self, Parameters(args): Parameters<WikiArgs>) -> Result<CallToolResult, McpError> {
        // Validate language code
        let language = validation::validate_language_code(&args.language)
            .map_err(internal_err("Language code validation failed"))?;

        // Validate action
        let action_str = validation::validate_wiki_action(&args.action)
            .map_err(internal_err("Action validation failed"))?;

        // Validate limit
        let limit = validation::validate_limit(args.limit, 100)
            .map_err(internal_err("Limit validation failed"))?;

        let options = wiki::WikiOptions {
            language,
            action: wiki::WikiAction::from_str(&action_str),
            limit,
            extract_images: args.extract_images,
        };

        let result = match options.action {
            wiki::WikiAction::Search => {
                let results = wiki::wiki_search(&self.state.client, &args.query, &options)
                    .await
                    .map_err(internal_err("Failed to search Wikipedia"))?;
                serde_json::to_string_pretty(&results)
                    .map_err(internal_err("Failed to serialize results"))?
            }
            wiki::WikiAction::Random => {
                let article = wiki::wiki_random(&self.state.client, &options)
                    .await
                    .map_err(internal_err("Failed to get random article"))?;
                serde_json::to_string_pretty(&article)
                    .map_err(internal_err("Failed to serialize article"))?
            }
            _ => {
                let article = wiki::wiki_get_article(&self.state.client, &args.query, &options)
                    .await
                    .map_err(internal_err("Failed to get article"))?;
                serde_json::to_string_pretty(&article)
                    .map_err(internal_err("Failed to serialize article"))?
            }
        };

        Ok(CallToolResult {
            content: vec![Content::text(result)],
            structured_content: None,
            is_error: None,
            meta: None,
        })
    }

    /// Extract text from PDF
    #[cfg(feature = "pdf")]
    #[tool(name = "fetch_pdf_text", description = "Extract text and metadata from PDF files")]
    async fn fetch_pdf_text(&self, Parameters(args): Parameters<FetchPdfArgs>) -> Result<CallToolResult, McpError> {
        // Validate URL
        let url = validation::validate_url(&args.url)
            .map_err(internal_err("URL validation failed"))?;

        // Fetch PDF bytes
        let pdf_bytes = fetch::fetch_url_bytes(&self.state.client, &url)
            .await
            .map_err(internal_err("Failed to fetch PDF"))?;

        // Extract text
        let pdf_info = pdf::extract_pdf_text(&pdf_bytes, args.max_pages)
            .map_err(internal_err("Failed to extract PDF text"))?;

        let result = serde_json::to_string_pretty(&pdf_info)
            .map_err(internal_err("Failed to serialize PDF info"))?;

        Ok(CallToolResult {
            content: vec![Content::text(result)],
            structured_content: None,
            is_error: None,
            meta: None,
        })
    }

    /// Get image information
    #[cfg(feature = "images")]
    #[tool(name = "fetch_image_info", description = "Get image format, dimensions, size, and metadata")]
    async fn fetch_image_info(&self, Parameters(args): Parameters<FetchImageArgs>) -> Result<CallToolResult, McpError> {
        // Validate URL
        let url = validation::validate_url(&args.url)
            .map_err(internal_err("URL validation failed"))?;

        // Fetch image bytes
        let image_bytes = fetch::fetch_url_bytes(&self.state.client, &url)
            .await
            .map_err(internal_err("Failed to fetch image"))?;

        // Extract image info
        let image_info = image::extract_image_info(&image_bytes)
            .map_err(internal_err("Failed to extract image info"))?;

        let result = serde_json::to_string_pretty(&image_info)
            .map_err(internal_err("Failed to serialize image info"))?;

        Ok(CallToolResult {
            content: vec![Content::text(result)],
            structured_content: None,
            is_error: None,
            meta: None,
        })
    }
}

// Implement ServerHandler trait
#[tool_handler]
impl ServerHandler for FetchServer {
    fn get_info(&self) -> ServerInfo {
        self.server_info()
    }
}

// ============================================================================
// Main Entry Point
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Determine if running in stream mode
    let is_stream_mode = cli.port.is_some();

    // Initialize logging (no stderr in stdio mode)
    logging::init_logging(cli.log_file.clone(), is_stream_mode);

    tracing::info!(
        "fetch-mcp-rs starting (mode: {}, robots.txt: {})",
        if is_stream_mode { "HTTP stream" } else { "stdio" },
        if cli.ignore_robots_txt { "ignored" } else { "enabled" }
    );

    // Determine user agent
    let user_agent = cli.user_agent.unwrap_or_else(|| {
        if is_stream_mode {
            fetch::DEFAULT_USER_AGENT_MANUAL.to_string()
        } else {
            fetch::DEFAULT_USER_AGENT_AUTONOMOUS.to_string()
        }
    });

    tracing::debug!("User-Agent: {}", user_agent);

    // Create server state
    let state = Arc::new(ServerState::new(
        user_agent,
        cli.ignore_robots_txt,
        cli.proxy_url.as_deref(),
    )?);

    let server = FetchServer::new(state);

    if let Some(_port) = cli.port {
        // HTTP Stream mode - TODO: implement when needed
        tracing::error!("HTTP stream mode not yet implemented");
        anyhow::bail!("HTTP stream mode not yet implemented");
    } else {
        // Stdio mode
        tracing::debug!("Starting stdio server");
        let transport = stdio();
        let svc = server.serve(transport).await?;
        svc.waiting().await?;
    }

    Ok(())
}
