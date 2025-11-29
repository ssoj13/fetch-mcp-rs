use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// Reddit comment
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RedditComment {
    /// Comment author
    pub author: String,

    /// Comment body text
    pub body: String,

    /// Comment score (upvotes - downvotes)
    pub score: i32,

    /// Creation time (UTC timestamp)
    pub created_utc: i64,

    /// Permalink to comment
    pub permalink: String,
}

/// Reddit post
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RedditPost {
    /// Post title
    pub title: String,

    /// Post author
    pub author: String,

    /// Subreddit name
    pub subreddit: String,

    /// Post score (upvotes - downvotes)
    pub score: i32,

    /// External URL (if link post)
    pub url: Option<String>,

    /// Reddit permalink
    pub permalink: String,

    /// Creation time (UTC timestamp)
    pub created_utc: i64,

    /// Number of comments
    pub num_comments: i32,

    /// Self-text content (if text post)
    pub selftext: Option<String>,

    /// Top comments (if requested)
    pub comments: Option<Vec<RedditComment>>,

    /// Post flair text
    pub flair: Option<String>,

    /// Is post marked NSFW
    pub is_nsfw: bool,
}

/// Reddit search/fetch options
#[derive(Debug, Clone)]
pub struct RedditOptions {
    /// Subreddit name (default: "all")
    pub subreddit: String,

    /// Sort method: hot, new, top, rising, controversial
    pub sort: String,

    /// Time filter for "top" and "controversial": hour, day, week, month, year, all
    pub time_filter: Option<String>,

    /// Number of posts to fetch (max 100)
    pub limit: usize,

    /// Include top comments
    pub include_comments: bool,

    /// Max comments per post
    pub max_comments: usize,
}

impl Default for RedditOptions {
    fn default() -> Self {
        Self {
            subreddit: "all".to_string(),
            sort: "hot".to_string(),
            time_filter: None,
            limit: 25,
            include_comments: false,
            max_comments: 10,
        }
    }
}

/// Fetch posts from Reddit
pub async fn fetch_reddit_posts(
    client: &reqwest::Client,
    query: Option<&str>,
    options: RedditOptions,
) -> Result<Vec<RedditPost>> {
    let limit = options.limit.min(100);

    let url = if let Some(q) = query {
        // Search mode
        format!(
            "https://www.reddit.com/r/{}/search.json?q={}&restrict_sr=1&sort={}&limit={}",
            options.subreddit,
            urlencoding::encode(q),
            options.sort,
            limit
        )
    } else {
        // Browse mode
        let mut url = format!(
            "https://www.reddit.com/r/{}/{}.json?limit={}",
            options.subreddit, options.sort, limit
        );

        if let Some(t) = options.time_filter {
            url.push_str(&format!("&t={}", t));
        }

        url
    };

    tracing::debug!("Fetching Reddit: {}", url);

    let response = client
        .get(&url)
        .header("User-Agent", "fetch-mcp-rs/0.1.0")
        .send()
        .await
        .context("Failed to fetch from Reddit")?;

    if !response.status().is_success() {
        anyhow::bail!("Reddit API returned status: {}", response.status());
    }

    let json: serde_json::Value = response
        .json()
        .await
        .context("Failed to parse Reddit JSON")?;

    let children = json["data"]["children"]
        .as_array()
        .context("Invalid Reddit response structure")?;

    let mut posts = Vec::new();

    for child in children {
        let data = &child["data"];

        let title = data["title"].as_str().unwrap_or("").to_string();
        let author = data["author"].as_str().unwrap_or("[deleted]").to_string();
        let subreddit = data["subreddit"].as_str().unwrap_or("").to_string();
        let score = data["score"].as_i64().unwrap_or(0) as i32;
        let url_str = data["url"].as_str().map(|s| s.to_string());
        let permalink = format!("https://www.reddit.com{}", data["permalink"].as_str().unwrap_or(""));
        let created_utc = data["created_utc"].as_f64().unwrap_or(0.0) as i64;
        let num_comments = data["num_comments"].as_i64().unwrap_or(0) as i32;
        let selftext = data["selftext"].as_str().filter(|s| !s.is_empty()).map(|s| s.to_string());
        let flair = data["link_flair_text"].as_str().map(|s| s.to_string());
        let is_nsfw = data["over_18"].as_bool().unwrap_or(false);

        let comments = if options.include_comments && num_comments > 0 {
            fetch_reddit_comments(client, &permalink, options.max_comments)
                .await
                .ok()
        } else {
            None
        };

        posts.push(RedditPost {
            title,
            author,
            subreddit,
            score,
            url: url_str,
            permalink,
            created_utc,
            num_comments,
            selftext,
            comments,
            flair,
            is_nsfw,
        });
    }

    Ok(posts)
}

/// Fetch comments for a specific post
async fn fetch_reddit_comments(
    client: &reqwest::Client,
    permalink: &str,
    max_comments: usize,
) -> Result<Vec<RedditComment>> {
    let url = format!("{}.json?limit={}", permalink, max_comments);

    let response = client
        .get(&url)
        .header("User-Agent", "fetch-mcp-rs/0.1.0")
        .send()
        .await
        .context("Failed to fetch comments")?;

    let json: serde_json::Value = response.json().await.context("Failed to parse comments JSON")?;

    let mut comments = Vec::new();

    if let Some(comments_listing) = json.get(1) {
        if let Some(children) = comments_listing["data"]["children"].as_array() {
            for child in children.iter().take(max_comments) {
                let data = &child["data"];

                if data["body"].is_null() {
                    continue;
                }

                let author = data["author"].as_str().unwrap_or("[deleted]").to_string();
                let body = data["body"].as_str().unwrap_or("").to_string();
                let score = data["score"].as_i64().unwrap_or(0) as i32;
                let created_utc = data["created_utc"].as_f64().unwrap_or(0.0) as i64;
                let comment_permalink = format!(
                    "https://www.reddit.com{}",
                    data["permalink"].as_str().unwrap_or("")
                );

                comments.push(RedditComment {
                    author,
                    body,
                    score,
                    created_utc,
                    permalink: comment_permalink,
                });
            }
        }
    }

    Ok(comments)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_reddit_posts() {
        let client = reqwest::Client::new();

        let options = RedditOptions {
            subreddit: "rust".to_string(),
            sort: "hot".to_string(),
            limit: 5,
            ..Default::default()
        };

        let result = fetch_reddit_posts(&client, None, options).await;
        assert!(result.is_ok());

        let posts = result.unwrap();
        assert!(!posts.is_empty());
        assert!(posts.iter().all(|p| p.subreddit == "rust"));
    }

    #[tokio::test]
    async fn test_reddit_search() {
        let client = reqwest::Client::new();

        let options = RedditOptions {
            subreddit: "programming".to_string(),
            limit: 3,
            ..Default::default()
        };

        let result = fetch_reddit_posts(&client, Some("rust"), options).await;
        assert!(result.is_ok());
    }
}
