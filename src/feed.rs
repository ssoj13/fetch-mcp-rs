use anyhow::{Context, Result};
use feed_rs::parser;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// Feed information (RSS/Atom/JSON Feed)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FeedInfo {
    /// Feed title
    pub title: String,

    /// Feed description
    pub description: Option<String>,

    /// Feed link
    pub link: Option<String>,

    /// Feed type (RSS 2.0, Atom, JSON Feed, etc.)
    pub feed_type: String,

    /// Feed items/entries
    pub items: Vec<FeedItem>,
}

/// Single feed item
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FeedItem {
    /// Item title
    pub title: Option<String>,

    /// Item link/URL
    pub link: Option<String>,

    /// Publication date
    pub published: Option<String>,

    /// Item content/summary
    pub content: Option<String>,

    /// Item author
    pub author: Option<String>,
}

/// Parse feed from XML/JSON content
pub fn parse_feed(content: &str, max_items: usize) -> Result<FeedInfo> {
    let feed = parser::parse(content.as_bytes())
        .context("Failed to parse feed")?;

    let feed_type = match feed.feed_type {
        feed_rs::model::FeedType::Atom => "Atom",
        feed_rs::model::FeedType::JSON => "JSON Feed",
        feed_rs::model::FeedType::RSS0 => "RSS 0.9",
        feed_rs::model::FeedType::RSS1 => "RSS 1.0",
        feed_rs::model::FeedType::RSS2 => "RSS 2.0",
    };

    let items: Vec<FeedItem> = feed
        .entries
        .into_iter()
        .take(max_items)
        .map(|entry| {
            let link = entry
                .links
                .first()
                .map(|l| l.href.clone());

            let content = entry
                .content
                .and_then(|c| c.body)
                .or_else(|| entry.summary.map(|s| s.content));

            let author = entry
                .authors
                .first()
                .map(|a| a.name.clone());

            let published = entry
                .published
                .or(entry.updated)
                .map(|dt| dt.to_rfc3339());

            FeedItem {
                title: entry.title.map(|t| t.content),
                link,
                published,
                content,
                author,
            }
        })
        .collect();

    Ok(FeedInfo {
        title: feed.title.map(|t| t.content).unwrap_or_else(|| "Untitled Feed".to_string()),
        description: feed.description.map(|d| d.content),
        link: feed.links.first().map(|l| l.href.clone()),
        feed_type: feed_type.to_string(),
        items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rss() {
        let rss = r#"<?xml version="1.0" encoding="UTF-8"?>
        <rss version="2.0">
            <channel>
                <title>Test Feed</title>
                <description>A test RSS feed</description>
                <link>https://example.com</link>
                <item>
                    <title>First Post</title>
                    <link>https://example.com/post1</link>
                    <description>This is the first post</description>
                    <pubDate>Mon, 01 Jan 2024 00:00:00 GMT</pubDate>
                </item>
            </channel>
        </rss>"#;

        let result = parse_feed(rss, 10);
        assert!(result.is_ok());

        let feed = result.unwrap();
        assert_eq!(feed.title, "Test Feed");
        assert_eq!(feed.feed_type, "RSS 2.0");
        assert_eq!(feed.items.len(), 1);
        assert_eq!(feed.items[0].title, Some("First Post".to_string()));
    }
}
