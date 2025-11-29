use anyhow::{Context, Result};
use readability::extractor::extract;
use std::io::Cursor;

/// Convert HTML to simplified Markdown using Readability algorithm
pub fn html_to_markdown(html: &str, url: &str) -> Result<String> {
    // Use Readability to extract main content
    let mut cursor = Cursor::new(html.as_bytes());

    match extract(&mut cursor, &url::Url::parse(url).context("Invalid URL")?) {
        Ok(product) => {
            // Convert extracted HTML to markdown using html2text
            let markdown = html2text::from_read(product.content.as_bytes(), 80)
                .context("Failed to convert HTML to markdown")?;
            Ok(markdown)
        }
        Err(_) => {
            // Fallback: if readability fails, just convert raw HTML
            tracing::warn!("Readability extraction failed for {}, using raw HTML conversion", url);
            let markdown = html2text::from_read(html.as_bytes(), 80)
                .context("Failed to convert HTML to text")?;
            Ok(markdown)
        }
    }
}

/// Convert HTML to plain text without markdown formatting
pub fn html_to_text(html: &str) -> String {
    html2text::from_read(html.as_bytes(), 80)
        .unwrap_or_else(|_| String::from(html))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_to_markdown() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test Page</title></head>
            <body>
                <h1>Main Title</h1>
                <p>This is a paragraph with <strong>bold</strong> text.</p>
                <ul>
                    <li>Item 1</li>
                    <li>Item 2</li>
                </ul>
            </body>
            </html>
        "#;

        let result = html_to_markdown(html, "https://example.com");
        assert!(result.is_ok());
        let markdown = result.unwrap();
        assert!(markdown.contains("Main Title"));
        assert!(markdown.contains("paragraph"));
    }

    #[test]
    fn test_html_to_text() {
        let html = "<p>Hello <b>world</b>!</p>";
        let text = html_to_text(html);
        assert!(text.contains("Hello"));
        assert!(text.contains("world"));
    }
}
