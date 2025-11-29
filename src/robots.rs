use anyhow::{Context, Result};
use robotstxt::DefaultMatcher;
use url::Url;

/// Check if a URL can be fetched according to robots.txt
pub async fn check_robots_txt_allowed(
    client: &reqwest::Client,
    url: &str,
    user_agent: &str,
) -> Result<()> {
    let parsed_url = Url::parse(url).context("Invalid URL")?;

    // Construct robots.txt URL
    let robots_url = format!(
        "{}://{}/robots.txt",
        parsed_url.scheme(),
        parsed_url.host_str().context("No host in URL")?
    );

    tracing::debug!("Fetching robots.txt from: {}", robots_url);

    // Fetch robots.txt
    let response = client
        .get(&robots_url)
        .send()
        .await
        .context("Failed to fetch robots.txt")?;

    // Handle 4xx errors
    if response.status().is_client_error() {
        if response.status().as_u16() == 401 || response.status().as_u16() == 403 {
            anyhow::bail!(
                "robots.txt returned {}, assuming autonomous fetching not allowed",
                response.status()
            );
        }
        // 404 and other 4xx = no robots.txt = allowed
        tracing::debug!("robots.txt returned {}, assuming allowed", response.status());
        return Ok(());
    }

    if !response.status().is_success() {
        anyhow::bail!("Failed to fetch robots.txt: HTTP {}", response.status());
    }

    let robots_content = response.text().await.context("Failed to read robots.txt")?;

    // Parse robots.txt
    let mut matcher = DefaultMatcher::default();
    let allowed = matcher.one_agent_allowed_by_robots(
        &robots_content,
        user_agent,
        url,
    );

    if !allowed {
        anyhow::bail!(
            "robots.txt disallows fetching {} for user-agent '{}'",
            url,
            user_agent
        );
    }

    tracing::debug!("robots.txt allows fetching {}", url);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_robots_txt_parsing() {
        let client = reqwest::Client::new();

        // Google allows crawling of homepage
        let result = check_robots_txt_allowed(
            &client,
            "https://www.google.com/",
            "Mozilla/5.0"
        ).await;

        assert!(result.is_ok());
    }
}
