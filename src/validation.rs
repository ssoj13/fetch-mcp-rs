use anyhow::{bail, Context, Result};
use regex::Regex;
use url::Url;

/// Validate and normalize URL
pub fn validate_url(url_str: &str) -> Result<String> {
    // Parse URL to validate format
    let url = Url::parse(url_str).context("Invalid URL format")?;

    // Only allow http and https schemes
    let scheme = url.scheme();
    if scheme != "http" && scheme != "https" {
        bail!("Only HTTP and HTTPS URLs are allowed, got: {}", scheme);
    }

    // Ensure URL has a host
    if url.host_str().is_none() {
        bail!("URL must have a host");
    }

    // Return normalized URL (Url::parse already normalizes)
    Ok(url.to_string())
}

/// Validate CSS selector syntax
pub fn validate_selector(selector: &str) -> Result<String> {
    use scraper::Selector;

    // Limit selector length to prevent DoS
    if selector.len() > 1000 {
        bail!("CSS selector too long (max 1000 characters)");
    }

    // Check for empty selector
    if selector.trim().is_empty() {
        bail!("CSS selector cannot be empty");
    }

    // Validate selector syntax by trying to parse it
    Selector::parse(selector)
        .map_err(|e| anyhow::anyhow!("Invalid CSS selector: {:?}", e))?;

    Ok(selector.trim().to_string())
}

/// Validate regex pattern
pub fn validate_regex(pattern: &str) -> Result<String> {
    // Limit pattern length to prevent ReDoS
    if pattern.len() > 500 {
        bail!("Regex pattern too long (max 500 characters)");
    }

    // Check for empty pattern
    if pattern.trim().is_empty() {
        bail!("Regex pattern cannot be empty");
    }

    // Validate regex syntax
    Regex::new(pattern)
        .context("Invalid regex pattern")?;

    Ok(pattern.to_string())
}

/// Validate and normalize limit parameter
pub fn validate_limit(limit: usize, max_limit: usize) -> Result<usize> {
    if limit == 0 {
        bail!("Limit must be greater than 0");
    }

    if limit > max_limit {
        bail!("Limit exceeds maximum allowed ({})", max_limit);
    }

    Ok(limit)
}

/// Validate array size
pub fn validate_array_size<T>(array: &[T], max_size: usize, name: &str) -> Result<()> {
    if array.is_empty() {
        bail!("{} array cannot be empty", name);
    }

    if array.len() > max_size {
        bail!("{} array too large (max {} items)", name, max_size);
    }

    Ok(())
}

/// Sanitize string input (remove control characters, normalize whitespace)
pub fn sanitize_string(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() || c.is_whitespace())
        .collect::<String>()
        .trim()
        .to_string()
}

/// Validate subreddit name
pub fn validate_subreddit(subreddit: &str) -> Result<String> {
    let sanitized = sanitize_string(subreddit);

    if sanitized.is_empty() {
        return Ok("all".to_string());
    }

    // Subreddit names: alphanumeric, underscores, hyphens, 3-21 chars
    if sanitized.len() < 3 || sanitized.len() > 21 {
        bail!("Subreddit name must be 3-21 characters");
    }

    // Validate characters (no special chars except _ and -)
    if !sanitized.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        bail!("Subreddit name contains invalid characters");
    }

    Ok(sanitized.to_lowercase())
}

/// Validate language code (ISO 639-1)
pub fn validate_language_code(lang: &str) -> Result<String> {
    let sanitized = sanitize_string(lang);

    // Must be 2 or 3 lowercase letters
    if sanitized.len() < 2 || sanitized.len() > 3 {
        bail!("Language code must be 2-3 characters");
    }

    if !sanitized.chars().all(|c| c.is_ascii_lowercase()) {
        bail!("Language code must be lowercase letters only");
    }

    Ok(sanitized)
}

/// Validate sort parameter for Reddit
pub fn validate_reddit_sort(sort: &str) -> Result<String> {
    let normalized = sanitize_string(sort).to_lowercase();

    match normalized.as_str() {
        "hot" | "new" | "top" | "rising" | "controversial" => Ok(normalized),
        _ => bail!("Invalid sort value. Must be: hot, new, top, rising, controversial"),
    }
}

/// Validate time filter for Reddit
pub fn validate_reddit_time(time: Option<&str>) -> Result<Option<String>> {
    match time {
        None => Ok(None),
        Some(t) => {
            let normalized = sanitize_string(t).to_lowercase();
            match normalized.as_str() {
                "hour" | "day" | "week" | "month" | "year" | "all" => Ok(Some(normalized)),
                _ => bail!("Invalid time filter. Must be: hour, day, week, month, year, all"),
            }
        }
    }
}

/// Validate Wikipedia action
pub fn validate_wiki_action(action: &str) -> Result<String> {
    let normalized = sanitize_string(action).to_lowercase();

    match normalized.as_str() {
        "search" | "summary" | "full" | "random" => Ok(normalized),
        _ => bail!("Invalid action. Must be: search, summary, full, random"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_url() {
        // Valid URLs
        assert!(validate_url("https://example.com").is_ok());
        assert!(validate_url("http://example.com/path?q=test").is_ok());

        // Invalid URLs
        assert!(validate_url("ftp://example.com").is_err());
        assert!(validate_url("javascript:alert(1)").is_err());
        assert!(validate_url("not a url").is_err());
        assert!(validate_url("file:///etc/passwd").is_err());
    }

    #[test]
    fn test_validate_selector() {
        assert!(validate_selector("div.class").is_ok());
        assert!(validate_selector("a[href]").is_ok());
        assert!(validate_selector("").is_err());
        assert!(validate_selector("   ").is_err());
        assert!(validate_selector("invalid[[[").is_err());
    }

    #[test]
    fn test_validate_regex() {
        assert!(validate_regex(r"\d+").is_ok());
        assert!(validate_regex("test.*").is_ok());
        assert!(validate_regex("").is_err());
        assert!(validate_regex("[").is_err()); // Invalid regex
    }

    #[test]
    fn test_validate_limit() {
        assert_eq!(validate_limit(10, 100).unwrap(), 10);
        assert!(validate_limit(0, 100).is_err());
        assert!(validate_limit(200, 100).is_err());
    }

    #[test]
    fn test_sanitize_string() {
        assert_eq!(sanitize_string("  hello  "), "hello");
        assert_eq!(sanitize_string("hello\nworld"), "hello\nworld");
        assert_eq!(sanitize_string("test\x00bad"), "testbad");
    }

    #[test]
    fn test_validate_subreddit() {
        assert_eq!(validate_subreddit("rust").unwrap(), "rust");
        assert_eq!(validate_subreddit("rust_lang").unwrap(), "rust_lang");
        assert!(validate_subreddit("ab").is_err()); // Too short
        assert!(validate_subreddit("a".repeat(25).as_str()).is_err()); // Too long
    }
}
