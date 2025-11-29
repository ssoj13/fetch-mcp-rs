use anyhow::Result;
use quick_xml::events::Event;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// Sitemap URL entry
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SitemapUrl {
    /// URL location
    pub loc: String,

    /// Last modification date
    pub lastmod: Option<String>,

    /// Change frequency (always, hourly, daily, weekly, monthly, yearly, never)
    pub changefreq: Option<String>,

    /// Priority (0.0 to 1.0)
    pub priority: Option<f32>,
}

/// Sitemap index entry (for sitemap of sitemaps)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SitemapIndexEntry {
    /// Sitemap URL
    pub loc: String,

    /// Last modification date
    pub lastmod: Option<String>,
}

/// Parsed sitemap data
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SitemapData {
    /// Type of sitemap (urlset or sitemapindex)
    pub sitemap_type: String,

    /// URLs (if urlset type)
    pub urls: Vec<SitemapUrl>,

    /// Sitemap indices (if sitemapindex type)
    pub sitemaps: Vec<SitemapIndexEntry>,
}

/// Parse sitemap XML content
pub fn parse_sitemap(xml_content: &str) -> Result<SitemapData> {
    let mut reader = Reader::from_str(xml_content);
    reader.config_mut().trim_text(true);

    let mut is_urlset = false;
    let mut is_sitemapindex = false;

    let mut urls = Vec::new();
    let mut sitemaps = Vec::new();

    let mut current_url: Option<SitemapUrl> = None;
    let mut current_sitemap: Option<SitemapIndexEntry> = None;
    let mut current_tag = String::new();

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                match tag_name.as_str() {
                    "urlset" => is_urlset = true,
                    "sitemapindex" => is_sitemapindex = true,
                    "url" => {
                        current_url = Some(SitemapUrl {
                            loc: String::new(),
                            lastmod: None,
                            changefreq: None,
                            priority: None,
                        });
                    }
                    "sitemap" => {
                        current_sitemap = Some(SitemapIndexEntry {
                            loc: String::new(),
                            lastmod: None,
                        });
                    }
                    _ => {
                        current_tag = tag_name;
                    }
                }
            }

            Ok(Event::Text(e)) => {
                // In quick-xml 0.38+, BytesText doesn't have unescape() method
                // Text is already decoded by the parser, just convert to string
                let text = String::from_utf8_lossy(e.as_ref()).to_string();

                if let Some(ref mut url) = current_url {
                    match current_tag.as_str() {
                        "loc" => url.loc = text,
                        "lastmod" => url.lastmod = Some(text),
                        "changefreq" => url.changefreq = Some(text),
                        "priority" => {
                            url.priority = text.parse::<f32>().ok();
                        }
                        _ => {}
                    }
                } else if let Some(ref mut sm) = current_sitemap {
                    match current_tag.as_str() {
                        "loc" => sm.loc = text,
                        "lastmod" => sm.lastmod = Some(text),
                        _ => {}
                    }
                }
            }

            Ok(Event::End(e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                match tag_name.as_str() {
                    "url" => {
                        if let Some(url) = current_url.take() {
                            if !url.loc.is_empty() {
                                urls.push(url);
                            }
                        }
                    }
                    "sitemap" => {
                        if let Some(sm) = current_sitemap.take() {
                            if !sm.loc.is_empty() {
                                sitemaps.push(sm);
                            }
                        }
                    }
                    _ => {
                        current_tag.clear();
                    }
                }
            }

            Ok(Event::Eof) => break,

            Err(e) => {
                anyhow::bail!("Error parsing sitemap XML at position {}: {:?}", reader.buffer_position(), e);
            }

            _ => {}
        }

        buf.clear();
    }

    let sitemap_type = if is_sitemapindex {
        "sitemapindex"
    } else if is_urlset {
        "urlset"
    } else {
        "unknown"
    };

    Ok(SitemapData {
        sitemap_type: sitemap_type.to_string(),
        urls,
        sitemaps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_urlset_sitemap() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
            <url>
                <loc>https://example.com/page1</loc>
                <lastmod>2024-01-01</lastmod>
                <changefreq>daily</changefreq>
                <priority>0.8</priority>
            </url>
            <url>
                <loc>https://example.com/page2</loc>
                <lastmod>2024-01-02</lastmod>
            </url>
        </urlset>"#;

        let result = parse_sitemap(xml);
        assert!(result.is_ok());

        let sitemap = result.unwrap();
        assert_eq!(sitemap.sitemap_type, "urlset");
        assert_eq!(sitemap.urls.len(), 2);
        assert_eq!(sitemap.urls[0].loc, "https://example.com/page1");
        assert_eq!(sitemap.urls[0].lastmod, Some("2024-01-01".to_string()));
        assert_eq!(sitemap.urls[0].changefreq, Some("daily".to_string()));
        assert_eq!(sitemap.urls[0].priority, Some(0.8));
    }

    #[test]
    fn test_parse_sitemapindex() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
            <sitemap>
                <loc>https://example.com/sitemap1.xml</loc>
                <lastmod>2024-01-01</lastmod>
            </sitemap>
            <sitemap>
                <loc>https://example.com/sitemap2.xml</loc>
            </sitemap>
        </sitemapindex>"#;

        let result = parse_sitemap(xml);
        assert!(result.is_ok());

        let sitemap = result.unwrap();
        assert_eq!(sitemap.sitemap_type, "sitemapindex");
        assert_eq!(sitemap.sitemaps.len(), 2);
        assert_eq!(sitemap.sitemaps[0].loc, "https://example.com/sitemap1.xml");
        assert_eq!(sitemap.sitemaps[0].lastmod, Some("2024-01-01".to_string()));
    }

    #[test]
    fn test_parse_minimal_sitemap() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
            <url>
                <loc>https://example.com/</loc>
            </url>
        </urlset>"#;

        let result = parse_sitemap(xml);
        assert!(result.is_ok());

        let sitemap = result.unwrap();
        assert_eq!(sitemap.urls.len(), 1);
        assert_eq!(sitemap.urls[0].loc, "https://example.com/");
        assert_eq!(sitemap.urls[0].lastmod, None);
    }
}
