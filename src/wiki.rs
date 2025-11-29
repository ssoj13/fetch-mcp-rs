use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// Wikipedia search result
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WikiSearchResult {
    /// Article title
    pub title: String,

    /// Page ID
    pub page_id: i64,

    /// Short snippet
    pub snippet: String,
}

/// Wikipedia article content
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WikiArticle {
    /// Article title
    pub title: String,

    /// Page ID
    pub page_id: i64,

    /// Article URL
    pub url: String,

    /// Short summary (first paragraph)
    pub summary: Option<String>,

    /// Full article content in Markdown
    pub content: Option<String>,

    /// Article images (URLs)
    pub images: Vec<String>,

    /// Article categories
    pub categories: Vec<String>,

    /// Last modified timestamp
    pub last_modified: Option<String>,

    /// Article language
    pub language: String,
}

/// Wikipedia action type
#[derive(Debug, Clone, PartialEq)]
pub enum WikiAction {
    /// Search for articles
    Search,
    /// Get article summary
    Summary,
    /// Get full article content
    Full,
    /// Get random article
    Random,
}

impl WikiAction {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "search" => WikiAction::Search,
            "summary" => WikiAction::Summary,
            "full" => WikiAction::Full,
            "random" => WikiAction::Random,
            _ => WikiAction::Summary,
        }
    }
}

/// Wikipedia query options
#[derive(Debug, Clone)]
pub struct WikiOptions {
    /// Language code (en, ru, de, etc.)
    pub language: String,

    /// Action type
    pub action: WikiAction,

    /// Search limit (for search action)
    pub limit: usize,

    /// Extract images
    pub extract_images: bool,
}

impl Default for WikiOptions {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            action: WikiAction::Summary,
            limit: 10,
            extract_images: true,
        }
    }
}

/// Search Wikipedia articles
pub async fn wiki_search(
    client: &reqwest::Client,
    query: &str,
    options: &WikiOptions,
) -> Result<Vec<WikiSearchResult>> {
    let url = format!(
        "https://{}.wikipedia.org/w/api.php?action=query&list=search&srsearch={}&format=json&srlimit={}",
        options.language,
        urlencoding::encode(query),
        options.limit
    );

    tracing::debug!("Wikipedia search: {}", url);

    let response = client
        .get(&url)
        .header("User-Agent", "fetch-mcp-rs/0.1.0")
        .send()
        .await
        .context("Failed to search Wikipedia")?;

    let json: serde_json::Value = response
        .json()
        .await
        .context("Failed to parse Wikipedia JSON")?;

    let search_results = json["query"]["search"]
        .as_array()
        .context("Invalid Wikipedia search response")?;

    let results: Vec<WikiSearchResult> = search_results
        .iter()
        .filter_map(|item| {
            Some(WikiSearchResult {
                title: item["title"].as_str()?.to_string(),
                page_id: item["pageid"].as_i64()?,
                snippet: strip_html_tags(item["snippet"].as_str()?),
            })
        })
        .collect();

    Ok(results)
}

/// Get Wikipedia article content
pub async fn wiki_get_article(
    client: &reqwest::Client,
    title: &str,
    options: &WikiOptions,
) -> Result<WikiArticle> {
    // Get article extract and basic info
    let url = format!(
        "https://{}.wikipedia.org/w/api.php?action=query&prop=extracts|info|categories|images&titles={}&format=json&explaintext=1&exsectionformat=wiki&inprop=url&cllimit=50&imlimit=50",
        options.language,
        urlencoding::encode(title)
    );

    tracing::debug!("Fetching Wikipedia article: {}", url);

    let response = client
        .get(&url)
        .header("User-Agent", "fetch-mcp-rs/0.1.0")
        .send()
        .await
        .context("Failed to fetch Wikipedia article")?;

    let json: serde_json::Value = response
        .json()
        .await
        .context("Failed to parse Wikipedia JSON")?;

    let pages = json["query"]["pages"]
        .as_object()
        .context("Invalid Wikipedia response")?;

    let page = pages
        .values()
        .next()
        .context("No article found")?;

    // Check if page exists
    if page.get("missing").is_some() {
        anyhow::bail!("Article '{}' not found", title);
    }

    let title = page["title"].as_str().unwrap_or(title).to_string();
    let page_id = page["pageid"].as_i64().unwrap_or(0);
    let url = page["fullurl"].as_str().unwrap_or("").to_string();
    let extract = page["extract"].as_str().map(|s| s.to_string());

    // Split extract into summary (first paragraph) and full content
    let (summary, content) = if let Some(ref text) = extract {
        let paragraphs: Vec<&str> = text.split("\n\n").collect();
        let summary = paragraphs.first().map(|s| s.to_string());
        let content = if options.action == WikiAction::Full {
            Some(text.clone())
        } else {
            None
        };
        (summary, content)
    } else {
        (None, None)
    };

    // Extract categories
    let categories: Vec<String> = page["categories"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|cat| cat["title"].as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    // Extract images if requested
    let images = if options.extract_images {
        extract_wikipedia_images(client, &options.language, &title).await?
    } else {
        Vec::new()
    };

    let last_modified = page["touched"].as_str().map(|s| s.to_string());

    Ok(WikiArticle {
        title,
        page_id,
        url,
        summary,
        content,
        images,
        categories,
        last_modified,
        language: options.language.clone(),
    })
}

/// Get random Wikipedia article
pub async fn wiki_random(
    client: &reqwest::Client,
    options: &WikiOptions,
) -> Result<WikiArticle> {
    let url = format!(
        "https://{}.wikipedia.org/w/api.php?action=query&list=random&rnnamespace=0&rnlimit=1&format=json",
        options.language
    );

    let response = client
        .get(&url)
        .header("User-Agent", "fetch-mcp-rs/0.1.0")
        .send()
        .await
        .context("Failed to get random article")?;

    let json: serde_json::Value = response
        .json()
        .await
        .context("Failed to parse Wikipedia JSON")?;

    let random_page = json["query"]["random"]
        .as_array()
        .and_then(|arr| arr.first())
        .context("No random article found")?;

    let title = random_page["title"]
        .as_str()
        .context("No title in random article")?;

    wiki_get_article(client, title, options).await
}

/// Extract image URLs from Wikipedia article
async fn extract_wikipedia_images(
    client: &reqwest::Client,
    language: &str,
    title: &str,
) -> Result<Vec<String>> {
    let url = format!(
        "https://{}.wikipedia.org/w/api.php?action=query&titles={}&prop=images&format=json&imlimit=10",
        language,
        urlencoding::encode(title)
    );

    let response = client
        .get(&url)
        .header("User-Agent", "fetch-mcp-rs/0.1.0")
        .send()
        .await?;

    let json: serde_json::Value = response.json().await?;

    let pages = json["query"]["pages"]
        .as_object()
        .context("Invalid response")?;

    let page = pages.values().next().context("No page found")?;

    let image_titles: Vec<String> = page["images"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|img| {
                    let title = img["title"].as_str()?;
                    // Filter out non-image files (icons, etc.)
                    if title.ends_with(".svg") || title.contains("Icon") {
                        None
                    } else {
                        Some(title.to_string())
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    // Get actual image URLs
    let mut image_urls = Vec::new();
    for img_title in image_titles.iter().take(5) {
        if let Ok(img_url) = get_image_url(client, language, img_title).await {
            image_urls.push(img_url);
        }
    }

    Ok(image_urls)
}

/// Get actual image URL from image title
async fn get_image_url(client: &reqwest::Client, language: &str, image_title: &str) -> Result<String> {
    let url = format!(
        "https://{}.wikipedia.org/w/api.php?action=query&titles={}&prop=imageinfo&iiprop=url&format=json",
        language,
        urlencoding::encode(image_title)
    );

    let response = client.get(&url).send().await?;
    let json: serde_json::Value = response.json().await?;

    let pages = json["query"]["pages"].as_object().context("Invalid response")?;
    let page = pages.values().next().context("No page")?;
    let img_url = page["imageinfo"][0]["url"]
        .as_str()
        .context("No image URL")?
        .to_string();

    Ok(img_url)
}

/// Strip HTML tags from text
fn strip_html_tags(html: &str) -> String {
    let re = regex::Regex::new(r"<[^>]*>").unwrap();
    re.replace_all(html, "").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_wiki_search() {
        let client = reqwest::Client::new();
        let options = WikiOptions {
            language: "en".to_string(),
            limit: 5,
            ..Default::default()
        };

        let result = wiki_search(&client, "Rust programming language", &options).await;
        assert!(result.is_ok());

        let results = result.unwrap();
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_wiki_get_article() {
        let client = reqwest::Client::new();
        let options = WikiOptions {
            language: "en".to_string(),
            action: WikiAction::Summary,
            extract_images: false,
            ..Default::default()
        };

        let result = wiki_get_article(&client, "Rust (programming language)", &options).await;
        assert!(result.is_ok());

        let article = result.unwrap();
        assert_eq!(article.title, "Rust (programming language)");
        assert!(article.summary.is_some());
    }

    #[tokio::test]
    async fn test_wiki_random() {
        let client = reqwest::Client::new();
        let options = WikiOptions::default();

        let result = wiki_random(&client, &options).await;
        assert!(result.is_ok());

        let article = result.unwrap();
        assert!(!article.title.is_empty());
    }
}
