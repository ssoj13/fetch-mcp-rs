use anyhow::{Context, Result};
use futures::stream::{self, StreamExt};
use governor::{Quota, RateLimiter};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

/// Result of a single fetch operation
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FetchResult {
    /// Original URL
    pub url: String,

    /// HTTP status code
    pub status: u16,

    /// Success flag
    pub success: bool,

    /// Response content (if successful)
    pub content: Option<String>,

    /// Error message (if failed)
    pub error: Option<String>,

    /// Response time in milliseconds
    pub response_time_ms: u64,

    /// Content length in bytes
    pub content_length: Option<usize>,
}

/// Batch fetch options
#[derive(Debug, Clone)]
pub struct BatchOptions {
    /// Maximum concurrent requests
    pub max_concurrent: usize,

    /// Rate limit: requests per second
    pub rate_limit: Option<u32>,

    /// Timeout for each request in seconds
    pub timeout: Duration,

    /// Stop on first error
    pub fail_fast: bool,

    /// Follow redirects
    pub follow_redirects: bool,
}

impl Default for BatchOptions {
    fn default() -> Self {
        Self {
            max_concurrent: 5,
            rate_limit: Some(10), // 10 requests per second
            timeout: Duration::from_secs(30),
            fail_fast: false,
            follow_redirects: true,
        }
    }
}

/// Fetch multiple URLs in parallel with rate limiting
pub async fn fetch_batch(
    client: &reqwest::Client,
    urls: Vec<String>,
    options: BatchOptions,
) -> Result<BatchFetchResult> {
    if urls.is_empty() {
        return Ok(BatchFetchResult {
            results: Vec::new(),
            stats: BatchStats {
                total: 0,
                success: 0,
                failed: 0,
                avg_response_time_ms: 0,
                total_bytes: 0,
                total_time_ms: 0,
            },
        });
    }

    let start_time = std::time::Instant::now();

    tracing::info!(
        "Batch fetching {} URLs (concurrent: {}, rate_limit: {:?})",
        urls.len(),
        options.max_concurrent,
        options.rate_limit
    );

    // Create rate limiter if specified
    let rate_limiter = options.rate_limit.map(|rate| {
        let quota = Quota::per_second(NonZeroU32::new(rate).unwrap());
        Arc::new(RateLimiter::direct(quota))
    });

    // Create stream of fetch tasks
    let fetch_stream = stream::iter(urls.into_iter().enumerate().map(|(index, url)| {
        let client = client.clone();
        let rate_limiter = rate_limiter.clone();
        let timeout = options.timeout;
        let follow_redirects = options.follow_redirects;

        async move {
            // Rate limiting
            if let Some(ref limiter) = rate_limiter {
                limiter.until_ready().await;
            }

            tracing::debug!("[{}] Fetching: {}", index, url);

            let start = std::time::Instant::now();
            let result = fetch_single_url(&client, &url, timeout, follow_redirects).await;
            let elapsed = start.elapsed();

            match result {
                Ok((status, content, content_length)) => {
                    tracing::debug!("[{}] Success: {} ({}ms)", index, url, elapsed.as_millis());
                    FetchResult {
                        url,
                        status,
                        success: true,
                        content: Some(content),
                        error: None,
                        response_time_ms: elapsed.as_millis() as u64,
                        content_length,
                    }
                }
                Err(e) => {
                    tracing::warn!("[{}] Failed: {} - {}", index, url, e);
                    FetchResult {
                        url,
                        status: 0,
                        success: false,
                        content: None,
                        error: Some(e.to_string()),
                        response_time_ms: elapsed.as_millis() as u64,
                        content_length: None,
                    }
                }
            }
        }
    }));

    // Execute fetches with concurrency control
    let results: Vec<FetchResult> = fetch_stream
        .buffer_unordered(options.max_concurrent)
        .collect()
        .await;

    // Check fail_fast option
    if options.fail_fast {
        let failed = results.iter().find(|r| !r.success);
        if let Some(failed_result) = failed {
            anyhow::bail!("Batch fetch failed (fail_fast): {}", failed_result.url);
        }
    }

    let total_time = start_time.elapsed();
    let stats = calculate_batch_stats(&results, total_time);

    tracing::info!(
        "Batch fetch completed: {} success, {} failed in {}ms",
        stats.success,
        stats.failed,
        stats.total_time_ms
    );

    Ok(BatchFetchResult { results, stats })
}

/// Fetch a single URL with timeout
async fn fetch_single_url(
    client: &reqwest::Client,
    url: &str,
    timeout: Duration,
    _follow_redirects: bool,
) -> Result<(u16, String, Option<usize>)> {
    // Note: redirect policy is set globally on the client (limited to 10 redirects)
    // Cannot be overridden per-request in reqwest 0.12
    let response = client
        .get(url)
        .timeout(timeout)
        .send()
        .await
        .context(format!("Failed to fetch {}", url))?;

    let status = response.status().as_u16();
    let content_length = response.content_length().map(|len| len as usize);

    if !response.status().is_success() {
        anyhow::bail!("HTTP {} for {}", status, url);
    }

    let content = response
        .text()
        .await
        .context("Failed to read response body")?;

    Ok((status, content, content_length))
}

/// Batch fetch result with statistics
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BatchFetchResult {
    /// Individual fetch results
    pub results: Vec<FetchResult>,

    /// Batch statistics
    pub stats: BatchStats,
}

/// Batch processing statistics
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BatchStats {
    /// Total URLs processed
    pub total: usize,

    /// Successful fetches
    pub success: usize,

    /// Failed fetches
    pub failed: usize,

    /// Average response time in milliseconds
    pub avg_response_time_ms: u64,

    /// Total bytes downloaded
    pub total_bytes: usize,

    /// Total time elapsed in milliseconds
    pub total_time_ms: u64,
}

/// Calculate statistics from batch results
pub fn calculate_batch_stats(results: &[FetchResult], total_time: Duration) -> BatchStats {
    let total = results.len();
    let success = results.iter().filter(|r| r.success).count();
    let failed = total - success;

    let avg_response_time_ms = if !results.is_empty() {
        results.iter().map(|r| r.response_time_ms).sum::<u64>() / results.len() as u64
    } else {
        0
    };

    let total_bytes = results
        .iter()
        .filter_map(|r| r.content_length)
        .sum();

    BatchStats {
        total,
        success,
        failed,
        avg_response_time_ms,
        total_bytes,
        total_time_ms: total_time.as_millis() as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_batch() {
        let client = reqwest::Client::new();

        let urls = vec![
            "https://httpbin.org/delay/1".to_string(),
            "https://httpbin.org/status/200".to_string(),
            "https://httpbin.org/status/404".to_string(),
        ];

        let options = BatchOptions {
            max_concurrent: 2,
            rate_limit: Some(5),
            timeout: Duration::from_secs(10),
            fail_fast: false,
            follow_redirects: true,
        };

        let result = fetch_batch(&client, urls, options).await;
        assert!(result.is_ok());

        let results = result.unwrap();
        assert_eq!(results.len(), 3);

        // Check that we got some successes
        let success_count = results.iter().filter(|r| r.success).count();
        assert!(success_count >= 2);
    }

    #[tokio::test]
    async fn test_batch_stats() {
        let results = vec![
            FetchResult {
                url: "https://example.com".to_string(),
                status: 200,
                success: true,
                content: Some("test".to_string()),
                error: None,
                response_time_ms: 100,
                content_length: Some(4),
            },
            FetchResult {
                url: "https://example2.com".to_string(),
                status: 404,
                success: false,
                content: None,
                error: Some("Not found".to_string()),
                response_time_ms: 50,
                content_length: None,
            },
        ];

        let stats = calculate_batch_stats(&results, Duration::from_secs(1));
        assert_eq!(stats.total, 2);
        assert_eq!(stats.success, 1);
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.avg_response_time_ms, 75);
        assert_eq!(stats.total_bytes, 4);
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let client = reqwest::Client::new();

        let urls = vec![
            "https://httpbin.org/delay/0".to_string(),
            "https://httpbin.org/delay/0".to_string(),
            "https://httpbin.org/delay/0".to_string(),
        ];

        let options = BatchOptions {
            max_concurrent: 10,
            rate_limit: Some(2), // 2 requests per second
            timeout: Duration::from_secs(10),
            fail_fast: false,
            follow_redirects: true,
        };

        let start = std::time::Instant::now();
        let result = fetch_batch(&client, urls, options).await;
        let elapsed = start.elapsed();

        assert!(result.is_ok());
        // With rate limit of 2 req/sec, 3 requests should take at least 1 second
        assert!(elapsed.as_secs() >= 1);
    }
}
