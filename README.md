# fetch-mcp-rs

Advanced Rust MCP server for web content fetching with 13+ specialized tools. Convert HTML to Markdown, extract metadata, parse feeds, search Reddit/Wikipedia, and more.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-blue.svg)](https://www.rust-lang.org/)

## Features

### Core Capabilities

- **HTML to Markdown** - Readability algorithm + clean markdown conversion
- **Metadata Extraction** - Open Graph, Schema.org, Twitter Cards, HTML meta tags
- **Feed Parsing** - RSS 0.9/1.0/2.0, Atom, JSON Feed support
- **CSS Selectors** - Extract specific elements from HTML
- **Table Extraction** - Parse HTML tables to structured JSON
- **Sitemap Parsing** - Parse sitemap.xml and sitemap indexes
- **Link Extraction** - Extract all links with internal/external filtering
- **Batch Fetching** - Parallel URL fetching with rate limiting
- **Content Search** - Search within pages with context extraction
- **Reddit Integration** - Search posts, subreddits, with comment extraction
- **Wikipedia API** - Search, summaries, full articles, random articles
- **PDF Text Extraction** - Extract text and metadata from PDFs (optional)
- **Image Info** - Get image dimensions and format (optional)

### Advanced Features

- 🤖 **Robots.txt Validation** - Respect crawling rules (optional)
- 🔄 **In-Memory Caching** - 5-minute TTL for performance
- ⚡ **Rate Limiting** - Token bucket algorithm
- 🌐 **Proxy Support** - HTTP/HTTPS proxy configuration
- 📝 **Transport-Aware Logging** - No stderr pollution in stdio mode
- 🎯 **Dual User Agents** - Autonomous vs manual fetching modes

## Installation

### From Source

```bash
git clone https://github.com/ssoj13/fetch-mcp-rs
cd fetch-mcp-rs
cargo build --release
```

### Binary

The compiled binary will be in `target/release/fetch-mcp-rs` or `target/release/fetch-mcp-rs.exe` (Windows).

## Usage

### Command Line Options

```bash
fetch-mcp-rs [OPTIONS]

Options:
  --user-agent <USER_AGENT>  User agent string for HTTP requests
  --ignore-robots-txt        Ignore robots.txt restrictions (use with caution)
  --proxy-url <PROXY_URL>    HTTP proxy URL (e.g., http://proxy:8080)
  --log-file <LOG_FILE>      Log file path (optional, for debugging)
  --port <PORT>              Enable HTTP stream mode on specified port
  -h, --help                 Print help
```

### MCP Configuration

Add to your MCP settings:

```json
{
  "mcpServers": {
    "fetch": {
      "command": "/path/to/fetch-mcp-rs",
      "args": []
    }
  }
}
```

With custom options:

```json
{
  "mcpServers": {
    "fetch": {
      "command": "/path/to/fetch-mcp-rs",
      "args": [
        "--user-agent", "MyBot/1.0",
        "--proxy-url", "http://proxy:8080",
        "--log-file", "/tmp/fetch.log"
      ]
    }
  }
}
```

## Tools Reference

### 1. fetch

Fetch URL content and convert HTML to Markdown using Readability algorithm.

**Parameters:**
- `url` (string, required) - URL to fetch
- `raw` (boolean, optional) - Return raw HTML instead of Markdown

**Example:**
```json
{
  "url": "https://example.com/article",
  "raw": false
}
```

**Output:**
```json
{
  "content": "# Article Title\n\nContent here...",
  "url": "https://example.com/article"
}
```

---

### 2. fetch_metadata

Extract Open Graph, Schema.org, and HTML metadata from a URL.

**Parameters:**
- `url` (string, required) - URL to fetch metadata from

**Example:**
```json
{
  "url": "https://example.com"
}
```

**Output:**
```json
{
  "title": "Example Domain",
  "description": "Example description",
  "og_image": "https://example.com/image.jpg",
  "og_title": "Example Title",
  "author": "John Doe",
  "published_date": "2024-01-01",
  "language": "en",
  "keywords": ["example", "demo"],
  "twitter_card": "summary_large_image"
}
```

---

### 3. fetch_feed

Parse RSS/Atom feeds and extract entries.

**Parameters:**
- `url` (string, required) - Feed URL
- `max_entries` (number, optional) - Maximum entries to return (default: 10)

**Example:**
```json
{
  "url": "https://example.com/feed.xml",
  "max_entries": 5
}
```

**Output:**
```json
{
  "title": "Blog Feed",
  "description": "Latest posts",
  "link": "https://example.com",
  "entries": [
    {
      "title": "Post Title",
      "link": "https://example.com/post",
      "published": "2024-01-01T12:00:00Z",
      "summary": "Post summary...",
      "author": "Author Name"
    }
  ]
}
```

---

### 4. fetch_with_selector

Extract specific HTML elements using CSS selectors.

**Parameters:**
- `url` (string, required) - URL to fetch
- `selector` (string, required) - CSS selector (e.g., "div.content", "a[href]")
- `attribute` (string, optional) - Extract specific attribute instead of text

**Example:**
```json
{
  "url": "https://example.com",
  "selector": "a.link",
  "attribute": "href"
}
```

**Output:**
```json
[
  {
    "text": "Link text",
    "html": "<a class=\"link\" href=\"/page\">Link text</a>",
    "attributes": {
      "href": "/page",
      "class": "link"
    }
  }
]
```

---

### 5. extract_table

Extract HTML tables to structured JSON.

**Parameters:**
- `url` (string, required) - URL to fetch
- `table_index` (number, optional) - Extract specific table by index (0-based)

**Example:**
```json
{
  "url": "https://example.com/data",
  "table_index": 0
}
```

**Output:**
```json
[
  {
    "headers": ["Name", "Age", "City"],
    "rows": [
      ["John", "30", "NYC"],
      ["Jane", "25", "LA"]
    ]
  }
]
```

---

### 6. fetch_sitemap

Parse sitemap.xml and extract URLs.

**Parameters:**
- `url` (string, required) - Sitemap URL

**Example:**
```json
{
  "url": "https://example.com/sitemap.xml"
}
```

**Output:**
```json
{
  "urls": [
    {
      "loc": "https://example.com/page1",
      "lastmod": "2024-01-01",
      "changefreq": "weekly",
      "priority": 0.8
    }
  ],
  "sitemaps": [
    {
      "loc": "https://example.com/sitemap2.xml",
      "lastmod": "2024-01-01"
    }
  ]
}
```

---

### 7. fetch_links

Extract all links from a page with filtering options.

**Parameters:**
- `url` (string, required) - URL to fetch
- `internal_only` (boolean, optional) - Only internal links (same domain)
- `external_only` (boolean, optional) - Only external links (different domain)

**Example:**
```json
{
  "url": "https://example.com",
  "internal_only": true
}
```

**Output:**
```json
{
  "base_url": "https://example.com",
  "links": [
    {
      "href": "https://example.com/page",
      "text": "Page Title",
      "title": "Link title",
      "rel": "nofollow",
      "is_internal": true
    }
  ]
}
```

---

### 8. fetch_batch

Fetch multiple URLs in parallel with rate limiting.

**Parameters:**
- `urls` (array of strings, required) - URLs to fetch
- `max_concurrent` (number, optional) - Max concurrent requests (default: 5)
- `timeout` (number, optional) - Timeout per request in seconds (default: 30)

**Example:**
```json
{
  "urls": [
    "https://example.com/page1",
    "https://example.com/page2",
    "https://example.com/page3"
  ],
  "max_concurrent": 3,
  "timeout": 10
}
```

**Output:**
```json
[
  {
    "url": "https://example.com/page1",
    "status": 200,
    "success": true,
    "content_length": 1024,
    "error": null
  }
]
```

---

### 9. search_in_page

Search for text within a page with context extraction.

**Parameters:**
- `url` (string, required) - URL to search in
- `query` (string, required) - Search query
- `context_chars` (number, optional) - Characters of context around match (default: 100)
- `max_results` (number, optional) - Maximum results to return (default: 10)
- `case_sensitive` (boolean, optional) - Case-sensitive search (default: false)

**Example:**
```json
{
  "url": "https://example.com",
  "query": "search term",
  "context_chars": 50,
  "max_results": 5
}
```

**Output:**
```json
{
  "query": "search term",
  "total_matches": 3,
  "results": [
    {
      "match": "search term",
      "context": "...text before search term text after...",
      "position": 1234
    }
  ]
}
```

---

### 10. reddit

Search Reddit posts with advanced filtering.

**Parameters:**
- `query` (string, optional) - Search query (omit for subreddit browsing)
- `subreddit` (string, optional) - Specific subreddit (e.g., "rust")
- `sort` (string, optional) - Sort by: "hot", "new", "top", "rising" (default: "hot")
- `time` (string, optional) - Time filter for "top": "hour", "day", "week", "month", "year", "all"
- `limit` (number, optional) - Number of posts (default: 10, max: 100)
- `include_comments` (boolean, optional) - Fetch top comments (default: false)
- `comment_limit` (number, optional) - Max comments per post (default: 5)

**Example:**
```json
{
  "query": "rust programming",
  "subreddit": "rust",
  "sort": "top",
  "time": "week",
  "limit": 5,
  "include_comments": true
}
```

**Output:**
```json
[
  {
    "title": "Post Title",
    "author": "username",
    "subreddit": "rust",
    "score": 123,
    "url": "https://example.com",
    "permalink": "https://reddit.com/r/rust/comments/...",
    "selftext": "Post content...",
    "created_utc": 1234567890,
    "num_comments": 45,
    "comments": [
      {
        "author": "commenter",
        "body": "Comment text...",
        "score": 10
      }
    ]
  }
]
```

---

### 11. wiki

Search and fetch Wikipedia articles.

**Parameters:**
- `action` (string, required) - Action: "search", "summary", "full", "random"
- `query` (string, optional) - Search query (required for "search" and "summary")
- `limit` (number, optional) - Search results limit (default: 10)
- `language` (string, optional) - Wikipedia language code (default: "en")

**Examples:**

**Search:**
```json
{
  "action": "search",
  "query": "Rust programming",
  "limit": 5,
  "language": "en"
}
```

**Summary:**
```json
{
  "action": "summary",
  "query": "Rust (programming language)"
}
```

**Full Article:**
```json
{
  "action": "full",
  "query": "Rust (programming language)"
}
```

**Random Article:**
```json
{
  "action": "random",
  "language": "en"
}
```

**Output (summary/full):**
```json
{
  "title": "Rust (programming language)",
  "extract": "Rust is a multi-paradigm...",
  "url": "https://en.wikipedia.org/wiki/Rust_(programming_language)",
  "content": "Full article content..." // only in "full" action
}
```

---

### 12. fetch_pdf_text (Optional)

Extract text from PDF files.

**Parameters:**
- `url` (string, required) - PDF URL
- `max_pages` (number, optional) - Maximum pages to extract (default: all)

**Requires:** `pdf` feature enabled (default)

---

### 13. fetch_image_info (Optional)

Get image metadata without full download.

**Parameters:**
- `url` (string, required) - Image URL

**Requires:** `images` feature enabled (default)

---

## Features Configuration

### Default Features
```toml
default = ["pdf", "images"]
```

### Build Without Optional Features
```bash
# No PDF support
cargo build --no-default-features

# Only PDF, no images
cargo build --no-default-features --features pdf

# Full features
cargo build --features full
```

## Development

### Run Tests
```bash
cargo test
```

### Build Release
```bash
cargo build --release
```

### Enable Debug Logging
```bash
RUST_LOG=debug cargo run
```

## Architecture

### Modules

- **main.rs** - MCP server with 13 tool implementations
- **fetch.rs** - Core HTTP client with caching and rate limiting
- **html_convert.rs** - Readability + html2text conversion
- **metadata.rs** - HTML meta tag extraction (Open Graph, Schema.org)
- **feed.rs** - RSS/Atom/JSON feed parsing
- **selector.rs** - CSS selector extraction
- **sitemap.rs** - Sitemap XML parsing
- **links.rs** - Link extraction with filtering
- **batch.rs** - Parallel fetching with concurrency control
- **search.rs** - In-page text search with context
- **reddit.rs** - Reddit JSON API client
- **wiki.rs** - Wikipedia MediaWiki API client
- **robots.rs** - robots.txt validation
- **logging.rs** - Transport-aware logging

### Dependencies

Core:
- `rmcp 0.9.1` - MCP SDK with new API
- `reqwest 0.12` - HTTP client
- `tokio 1.48` - Async runtime

HTML/Content:
- `readability 0.3` - Content extraction
- `scraper 0.24` - HTML parsing
- `html2text 0.16` - HTML to text conversion

Feeds & Data:
- `feed-rs 2.3` - Feed parsing
- `webpage 2.0` - Metadata extraction
- `quick-xml 0.38` - XML parsing

Optional:
- `lopdf 0.38` - PDF text extraction
- `image 0.25` - Image processing

Performance:
- `cached 0.56` - In-memory caching
- `governor 0.10` - Rate limiting

## License

MIT License - see LICENSE file for details.

## Contributing

Contributions welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure `cargo test` passes
5. Submit a pull request

## Roadmap

- [ ] HTTP stream mode implementation
- [ ] Screenshot capture support
- [ ] JavaScript rendering (headless browser)
- [ ] Archive.org Wayback Machine integration
- [ ] Custom header support
- [ ] Cookie persistence
- [ ] Retry strategies
- [ ] Response streaming for large files

## Support

- Issues: https://github.com/ssoj13/fetch-mcp-rs/issues
- Discussions: https://github.com/ssoj13/fetch-mcp-rs/discussions

## Acknowledgments

- Built with [rmcp](https://github.com/modelcontextprotocol/rmcp) - Rust MCP SDK
- Inspired by the Python fetch-mcp-server
- Part of the [Model Context Protocol](https://modelcontextprotocol.io/) ecosystem
