use anyhow::Result;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// Page metadata extracted from HTML
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PageMetadata {
    /// Page title
    pub title: Option<String>,

    /// Meta description
    pub description: Option<String>,

    /// Open Graph image URL
    pub og_image: Option<String>,

    /// Open Graph title
    pub og_title: Option<String>,

    /// Open Graph description
    pub og_description: Option<String>,

    /// Page author
    pub author: Option<String>,

    /// Publication date
    pub published_date: Option<String>,

    /// Canonical URL
    pub canonical_url: Option<String>,

    /// Page language
    pub language: Option<String>,

    /// Keywords
    pub keywords: Option<Vec<String>>,

    /// Twitter card type
    pub twitter_card: Option<String>,
}

/// Extract metadata from HTML content using scraper
pub fn extract_metadata(html: &str, url: &str) -> Result<PageMetadata> {
    let document = Html::parse_document(html);

    // Helper to get meta tag content by name
    let get_meta_name = |name: &str| -> Option<String> {
        let selector = Selector::parse(&format!("meta[name='{}']", name)).ok()?;
        document
            .select(&selector)
            .next()
            .and_then(|el| el.value().attr("content"))
            .map(|s| s.to_string())
    };

    // Helper to get meta tag content by property (for Open Graph)
    let get_meta_property = |property: &str| -> Option<String> {
        let selector = Selector::parse(&format!("meta[property='{}']", property)).ok()?;
        document
            .select(&selector)
            .next()
            .and_then(|el| el.value().attr("content"))
            .map(|s| s.to_string())
    };

    // Extract title
    let title = Selector::parse("title").ok()
        .and_then(|sel| document.select(&sel).next())
        .map(|el| el.text().collect::<String>().trim().to_string());

    // Extract keywords
    let keywords = get_meta_name("keywords")
        .map(|kw| kw.split(',').map(|s| s.trim().to_string()).collect());

    // Extract language from html tag
    let language = Selector::parse("html").ok()
        .and_then(|sel| document.select(&sel).next())
        .and_then(|el| el.value().attr("lang"))
        .map(|s| s.to_string());

    Ok(PageMetadata {
        title,
        description: get_meta_name("description"),
        og_image: get_meta_property("og:image"),
        og_title: get_meta_property("og:title"),
        og_description: get_meta_property("og:description"),
        author: get_meta_name("author"),
        published_date: get_meta_name("article:published_time")
            .or_else(|| get_meta_property("article:published_time")),
        canonical_url: Some(url.to_string()),
        language,
        keywords,
        twitter_card: get_meta_name("twitter:card"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_metadata() {
        let html = r#"
            <!DOCTYPE html>
            <html lang="en">
            <head>
                <title>Test Page</title>
                <meta name="description" content="This is a test page">
                <meta name="author" content="John Doe">
                <meta property="og:title" content="OG Title">
                <meta property="og:description" content="OG Description">
                <meta property="og:image" content="https://example.com/image.jpg">
            </head>
            <body>
                <h1>Test</h1>
            </body>
            </html>
        "#;

        let result = extract_metadata(html, "https://example.com");
        assert!(result.is_ok());

        let metadata = result.unwrap();
        assert_eq!(metadata.title, Some("Test Page".to_string()));
        assert_eq!(metadata.description, Some("This is a test page".to_string()));
        assert_eq!(metadata.author, Some("John Doe".to_string()));
    }
}
