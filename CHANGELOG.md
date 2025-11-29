# Changelog

All notable changes to fetch-mcp-rs will be documented in this file.

## [Unreleased]

### Added
- HTTP stream transport mode based on filesystem-mcp-rs reference
  - CLI flags: `--stream`, `--port` (default 8000), `--bind` (default 127.0.0.1)
  - Endpoints: `POST /mcp` (MCP RPC), `GET /health` (health check)
  - Graceful shutdown on Ctrl+C
  - Uses rmcp StreamableHttpService with LocalSessionManager and axum Router
- Comprehensive input validation for all 13 tools
  - URL validation (HTTP/HTTPS only)
  - CSS selector validation (max 1000 chars)
  - Regex pattern validation (max 500 chars, prevents ReDoS)
  - Array size limits (max 100 items)
- Integration of all helper functions (zero unused code warnings)
  - BatchStats with performance metrics in batch fetch results
  - Image size categorization and orientation detection
  - PDF max_pages parameter
  - Search total_occurrences and unique_words extraction
  - Link extraction convenience wrappers
- Comprehensive test suite (60 tests total)
  - 45 unit tests covering all modules
  - 4 HTTP transport integration tests
  - 11 stdio MCP integration tests
  - All tests run via single `cargo test` command

### Fixed
- Removed all 23 unused code warnings by integrating helper functions
- Updated logging to support both stdio and stream modes correctly
- Fixed 3 failing unit tests (categorize_image_size, extract_table, html_to_markdown)
- Fixed extract_table selector bug (headers extraction)
- Fixed batch_fetch test after BatchFetchResult refactoring

## [0.1.0] - 2025-01-15

### Added
- Initial Rust port of Python MCP fetch server
- 13+ MCP tools for web content fetching
- Support for HTML, JSON, XML, PDF, and image extraction
- Batch URL fetching with rate limiting
- In-page text search with regex support
- Link extraction with internal/external filtering
- Robots.txt compliance checking
- Response caching (100 entries, 300s TTL)
- stdio transport mode for local MCP clients
