use anyhow::{Context, Result};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use std::collections::HashSet;
use url::Url;

/// Extracted link information
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Hash)]
pub struct LinkInfo {
    /// Link URL (absolute)
    pub href: String,

    /// Link text content
    pub text: String,

    /// Link title attribute (if present)
    pub title: Option<String>,

    /// Link rel attribute (if present)
    pub rel: Option<String>,

    /// Whether link is internal (same domain) or external
    pub is_internal: bool,
}

/// Link extraction options
#[derive(Debug, Clone)]
pub struct LinkExtractionOptions {
    /// Include only internal links
    pub internal_only: bool,

    /// Include only external links
    pub external_only: bool,

    /// Deduplicate links
    pub deduplicate: bool,
}

impl Default for LinkExtractionOptions {
    fn default() -> Self {
        Self {
            internal_only: false,
            external_only: false,
            deduplicate: true,
        }
    }
}

/// Extract all links from HTML
pub fn extract_links(
    html: &str,
    base_url: &str,
    options: LinkExtractionOptions,
) -> Result<Vec<LinkInfo>> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("a[href]")
        .map_err(|e| anyhow::anyhow!("Failed to create link selector: {:?}", e))?;

    let base = Url::parse(base_url).context("Invalid base URL")?;
    let base_domain = base.host_str();

    let mut links = Vec::new();
    let mut seen = HashSet::new();

    for element in document.select(&selector) {
        let href = match element.value().attr("href") {
            Some(h) => h,
            None => continue,
        };

        // Skip empty hrefs, anchors, javascript:, mailto:, tel:
        if href.is_empty()
            || href.starts_with('#')
            || href.starts_with("javascript:")
            || href.starts_with("mailto:")
            || href.starts_with("tel:")
        {
            continue;
        }

        // Resolve relative URLs to absolute
        let absolute_url = match base.join(href) {
            Ok(url) => url.to_string(),
            Err(_) => {
                tracing::warn!("Failed to resolve URL: {}", href);
                continue;
            }
        };

        // Check if internal or external
        let link_url = Url::parse(&absolute_url).ok();
        let is_internal = link_url
            .as_ref()
            .and_then(|u| u.host_str())
            .map(|host| base_domain.map(|bd| host == bd).unwrap_or(false))
            .unwrap_or(false);

        // Apply filters
        if options.internal_only && !is_internal {
            continue;
        }
        if options.external_only && is_internal {
            continue;
        }

        // Deduplicate
        if options.deduplicate && !seen.insert(absolute_url.clone()) {
            continue;
        }

        let text = element.text().collect::<Vec<_>>().join(" ").trim().to_string();
        let title = element.value().attr("title").map(|s| s.to_string());
        let rel = element.value().attr("rel").map(|s| s.to_string());

        links.push(LinkInfo {
            href: absolute_url,
            text,
            title,
            rel,
            is_internal,
        });
    }

    Ok(links)
}

/// Extract only internal links (same domain) - convenience wrapper
pub fn extract_internal_links(html: &str, base_url: &str) -> Result<Vec<LinkInfo>> {
    extract_links(
        html,
        base_url,
        LinkExtractionOptions {
            internal_only: true,
            external_only: false,
            deduplicate: true,
        },
    )
}

/// Extract only external links (different domain) - convenience wrapper
pub fn extract_external_links(html: &str, base_url: &str) -> Result<Vec<LinkInfo>> {
    extract_links(
        html,
        base_url,
        LinkExtractionOptions {
            internal_only: false,
            external_only: true,
            deduplicate: true,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_links() {
        let html = r##"
            <!DOCTYPE html>
            <html>
            <body>
                <a href="https://example.com/page1">Page 1</a>
                <a href="/page2">Page 2</a>
                <a href="https://external.com">External</a>
                <a href="#anchor">Anchor</a>
                <a href="mailto:test@example.com">Email</a>
            </body>
            </html>
        "##;

        let result = extract_links(html, "https://example.com", LinkExtractionOptions::default());
        assert!(result.is_ok());

        let links = result.unwrap();
        // Should extract 3 links (page1, page2, external) and skip anchor and mailto
        assert_eq!(links.len(), 3);

        // Check that relative URL was resolved
        let page2 = links.iter().find(|l| l.href.contains("page2"));
        assert!(page2.is_some());
        assert_eq!(page2.unwrap().href, "https://example.com/page2");
    }

    #[test]
    fn test_extract_internal_links() {
        let html = r#"
            <a href="https://example.com/page1">Internal 1</a>
            <a href="/page2">Internal 2</a>
            <a href="https://external.com">External</a>
        "#;

        let result = extract_internal_links(html, "https://example.com");
        assert!(result.is_ok());

        let links = result.unwrap();
        assert_eq!(links.len(), 2);
        assert!(links.iter().all(|l| l.is_internal));
    }

    #[test]
    fn test_extract_external_links() {
        let html = r#"
            <a href="https://example.com/page1">Internal</a>
            <a href="https://external.com">External 1</a>
            <a href="https://another.com">External 2</a>
        "#;

        let result = extract_external_links(html, "https://example.com");
        assert!(result.is_ok());

        let links = result.unwrap();
        assert_eq!(links.len(), 2);
        assert!(links.iter().all(|l| !l.is_internal));
    }

    #[test]
    fn test_deduplicate_links() {
        let html = r#"
            <a href="/page1">Link 1</a>
            <a href="/page1">Link 1 again</a>
            <a href="/page2">Link 2</a>
        "#;

        let result = extract_links(html, "https://example.com", LinkExtractionOptions::default());
        assert!(result.is_ok());

        let links = result.unwrap();
        assert_eq!(links.len(), 2); // Deduplicated
    }

    #[test]
    fn test_link_attributes() {
        let html = r#"<a href="/page" title="Page Title" rel="nofollow">Link</a>"#;

        let result = extract_links(html, "https://example.com", LinkExtractionOptions::default());
        assert!(result.is_ok());

        let links = result.unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].text, "Link");
        assert_eq!(links[0].title, Some("Page Title".to_string()));
        assert_eq!(links[0].rel, Some("nofollow".to_string()));
    }
}
