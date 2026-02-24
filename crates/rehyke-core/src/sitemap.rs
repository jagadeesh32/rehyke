use crate::error::{RehykeError, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use tracing::debug;
use url::Url;

/// A single entry extracted from a `<url>` element in a sitemap.
#[derive(Debug, Clone)]
pub struct SitemapEntry {
    /// The URL location (`<loc>`).
    pub loc: String,
    /// The last modification date (`<lastmod>`), if present.
    pub lastmod: Option<String>,
    /// The change frequency (`<changefreq>`), if present.
    pub changefreq: Option<String>,
    /// The priority (`<priority>`), if present.
    pub priority: Option<f64>,
}

/// A parsed sitemap document (either a URL set or a sitemap index).
#[derive(Debug, Clone)]
pub struct Sitemap {
    /// URL entries from `<urlset>` documents.
    pub entries: Vec<SitemapEntry>,
    /// Sub-sitemap URLs from `<sitemapindex>` documents.
    pub sub_sitemaps: Vec<String>,
}

/// Tags that we track while iterating through elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CurrentTag {
    Loc,
    Lastmod,
    Changefreq,
    Priority,
    Other,
}

impl Sitemap {
    /// Parse a sitemap XML string.
    ///
    /// Handles both `<urlset>` sitemaps (containing `<url>` elements) and
    /// `<sitemapindex>` sitemaps (containing `<sitemap>` elements with
    /// `<loc>` sub-elements).
    pub fn parse(xml: &str) -> Result<Self> {
        let is_index = Self::is_sitemap_index(xml);
        let mut entries: Vec<SitemapEntry> = Vec::new();
        let mut sub_sitemaps: Vec<String> = Vec::new();

        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text_start = true;
        reader.config_mut().trim_text_end = true;

        let mut buf = Vec::new();

        // State for building up the current entry.
        let mut in_url = false;
        let mut in_sitemap = false;
        let mut current_tag = CurrentTag::Other;

        let mut loc = String::new();
        let mut lastmod: Option<String> = None;
        let mut changefreq: Option<String> = None;
        let mut priority: Option<f64> = None;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                    let local = local_name_str(e.name().as_ref());
                    match local.as_str() {
                        "url" => {
                            in_url = true;
                            loc.clear();
                            lastmod = None;
                            changefreq = None;
                            priority = None;
                        }
                        "sitemap" if is_index => {
                            in_sitemap = true;
                            loc.clear();
                        }
                        "loc" => current_tag = CurrentTag::Loc,
                        "lastmod" => current_tag = CurrentTag::Lastmod,
                        "changefreq" => current_tag = CurrentTag::Changefreq,
                        "priority" => current_tag = CurrentTag::Priority,
                        _ => current_tag = CurrentTag::Other,
                    }
                }
                Ok(Event::Text(e)) => {
                    let text = e.unescape().unwrap_or_default().trim().to_string();
                    if text.is_empty() {
                        continue;
                    }
                    match current_tag {
                        CurrentTag::Loc => loc = text,
                        CurrentTag::Lastmod => lastmod = Some(text),
                        CurrentTag::Changefreq => changefreq = Some(text),
                        CurrentTag::Priority => priority = text.parse::<f64>().ok(),
                        CurrentTag::Other => {}
                    }
                }
                Ok(Event::End(e)) => {
                    let local = local_name_str(e.name().as_ref());
                    match local.as_str() {
                        "url" if in_url => {
                            if !loc.is_empty() {
                                entries.push(SitemapEntry {
                                    loc: loc.clone(),
                                    lastmod: lastmod.take(),
                                    changefreq: changefreq.take(),
                                    priority: priority.take(),
                                });
                            }
                            in_url = false;
                            current_tag = CurrentTag::Other;
                        }
                        "sitemap" if in_sitemap => {
                            if !loc.is_empty() {
                                sub_sitemaps.push(loc.clone());
                            }
                            in_sitemap = false;
                            current_tag = CurrentTag::Other;
                        }
                        "loc" | "lastmod" | "changefreq" | "priority" => {
                            current_tag = CurrentTag::Other;
                        }
                        _ => {}
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(RehykeError::ParseError {
                        url: String::new(),
                        message: format!("XML parse error in sitemap: {}", e),
                    });
                }
                _ => {}
            }
            buf.clear();
        }

        debug!(
            entries = entries.len(),
            sub_sitemaps = sub_sitemaps.len(),
            "parsed sitemap"
        );

        Ok(Self {
            entries,
            sub_sitemaps,
        })
    }

    /// Check whether the given XML content looks like a sitemap index
    /// (contains `<sitemapindex`).
    pub fn is_sitemap_index(xml: &str) -> bool {
        xml.contains("<sitemapindex")
    }

    /// Return references to all URL strings contained in this sitemap's
    /// entries.
    pub fn urls(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.loc.as_str()).collect()
    }

    /// Build a list of common sitemap URLs to probe for a given domain.
    pub fn common_sitemap_urls(base: &Url) -> Vec<String> {
        let origin = format!("{}://{}", base.scheme(), base.host_str().unwrap_or("localhost"));
        vec![
            format!("{}/sitemap.xml", origin),
            format!("{}/sitemap_index.xml", origin),
            format!("{}/sitemap/sitemap.xml", origin),
            format!("{}/sitemaps.xml", origin),
        ]
    }
}

/// Extract the local name from a potentially namespace-prefixed tag name
/// byte slice.  For example, `b"ns:url"` becomes `"url"`.
fn local_name_str(bytes: &[u8]) -> String {
    let full = std::str::from_utf8(bytes).unwrap_or("");
    match full.rsplit_once(':') {
        Some((_, local)) => local.to_string(),
        None => full.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Standard sitemap parsing
    // -----------------------------------------------------------------------

    #[test]
    fn test_standard_sitemap() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url>
    <loc>https://example.com/</loc>
    <lastmod>2024-01-15</lastmod>
    <changefreq>daily</changefreq>
    <priority>1.0</priority>
  </url>
  <url>
    <loc>https://example.com/about</loc>
    <lastmod>2024-01-10</lastmod>
    <changefreq>monthly</changefreq>
    <priority>0.8</priority>
  </url>
  <url>
    <loc>https://example.com/blog</loc>
  </url>
</urlset>"#;

        let sitemap = Sitemap::parse(xml).unwrap();
        assert_eq!(sitemap.entries.len(), 3);
        assert!(sitemap.sub_sitemaps.is_empty());

        assert_eq!(sitemap.entries[0].loc, "https://example.com/");
        assert_eq!(
            sitemap.entries[0].lastmod.as_deref(),
            Some("2024-01-15")
        );
        assert_eq!(
            sitemap.entries[0].changefreq.as_deref(),
            Some("daily")
        );
        assert_eq!(sitemap.entries[0].priority, Some(1.0));

        assert_eq!(sitemap.entries[2].loc, "https://example.com/blog");
        assert!(sitemap.entries[2].lastmod.is_none());
        assert!(sitemap.entries[2].changefreq.is_none());
        assert!(sitemap.entries[2].priority.is_none());
    }

    #[test]
    fn test_urls_method() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>https://example.com/a</loc></url>
  <url><loc>https://example.com/b</loc></url>
</urlset>"#;

        let sitemap = Sitemap::parse(xml).unwrap();
        let urls = sitemap.urls();
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0], "https://example.com/a");
        assert_eq!(urls[1], "https://example.com/b");
    }

    // -----------------------------------------------------------------------
    // Sitemap index parsing
    // -----------------------------------------------------------------------

    #[test]
    fn test_sitemap_index() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <sitemap>
    <loc>https://example.com/sitemap1.xml</loc>
    <lastmod>2024-01-15</lastmod>
  </sitemap>
  <sitemap>
    <loc>https://example.com/sitemap2.xml</loc>
  </sitemap>
</sitemapindex>"#;

        let sitemap = Sitemap::parse(xml).unwrap();
        assert!(sitemap.entries.is_empty());
        assert_eq!(sitemap.sub_sitemaps.len(), 2);
        assert_eq!(
            sitemap.sub_sitemaps[0],
            "https://example.com/sitemap1.xml"
        );
        assert_eq!(
            sitemap.sub_sitemaps[1],
            "https://example.com/sitemap2.xml"
        );
    }

    #[test]
    fn test_is_sitemap_index_detection() {
        let index_xml = r#"<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"></sitemapindex>"#;
        let urlset_xml = r#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"></urlset>"#;

        assert!(Sitemap::is_sitemap_index(index_xml));
        assert!(!Sitemap::is_sitemap_index(urlset_xml));
    }

    // -----------------------------------------------------------------------
    // Common URL generation
    // -----------------------------------------------------------------------

    #[test]
    fn test_common_sitemap_urls() {
        let base = Url::parse("https://example.com/some/page").unwrap();
        let urls = Sitemap::common_sitemap_urls(&base);
        assert_eq!(urls.len(), 4);
        assert_eq!(urls[0], "https://example.com/sitemap.xml");
        assert_eq!(urls[1], "https://example.com/sitemap_index.xml");
        assert_eq!(urls[2], "https://example.com/sitemap/sitemap.xml");
        assert_eq!(urls[3], "https://example.com/sitemaps.xml");
    }

    #[test]
    fn test_common_sitemap_urls_http() {
        let base = Url::parse("http://example.com/").unwrap();
        let urls = Sitemap::common_sitemap_urls(&base);
        assert!(urls[0].starts_with("http://"));
    }

    // -----------------------------------------------------------------------
    // Empty sitemap
    // -----------------------------------------------------------------------

    #[test]
    fn test_empty_urlset() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
</urlset>"#;

        let sitemap = Sitemap::parse(xml).unwrap();
        assert!(sitemap.entries.is_empty());
        assert!(sitemap.sub_sitemaps.is_empty());
        assert!(sitemap.urls().is_empty());
    }

    #[test]
    fn test_empty_sitemapindex() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
</sitemapindex>"#;

        let sitemap = Sitemap::parse(xml).unwrap();
        assert!(sitemap.entries.is_empty());
        assert!(sitemap.sub_sitemaps.is_empty());
    }

    // -----------------------------------------------------------------------
    // Entries with all optional fields
    // -----------------------------------------------------------------------

    #[test]
    fn test_entry_with_all_fields() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url>
    <loc>https://example.com/full</loc>
    <lastmod>2024-06-01T12:00:00+00:00</lastmod>
    <changefreq>weekly</changefreq>
    <priority>0.7</priority>
  </url>
</urlset>"#;

        let sitemap = Sitemap::parse(xml).unwrap();
        assert_eq!(sitemap.entries.len(), 1);
        let entry = &sitemap.entries[0];
        assert_eq!(entry.loc, "https://example.com/full");
        assert_eq!(
            entry.lastmod.as_deref(),
            Some("2024-06-01T12:00:00+00:00")
        );
        assert_eq!(entry.changefreq.as_deref(), Some("weekly"));
        assert_eq!(entry.priority, Some(0.7));
    }

    #[test]
    fn test_entry_loc_only() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url>
    <loc>https://example.com/minimal</loc>
  </url>
</urlset>"#;

        let sitemap = Sitemap::parse(xml).unwrap();
        assert_eq!(sitemap.entries.len(), 1);
        let entry = &sitemap.entries[0];
        assert_eq!(entry.loc, "https://example.com/minimal");
        assert!(entry.lastmod.is_none());
        assert!(entry.changefreq.is_none());
        assert!(entry.priority.is_none());
    }

    // -----------------------------------------------------------------------
    // Malformed / edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_url_without_loc_skipped() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url>
    <lastmod>2024-01-01</lastmod>
  </url>
  <url>
    <loc>https://example.com/valid</loc>
  </url>
</urlset>"#;

        let sitemap = Sitemap::parse(xml).unwrap();
        assert_eq!(sitemap.entries.len(), 1);
        assert_eq!(sitemap.entries[0].loc, "https://example.com/valid");
    }

    #[test]
    fn test_namespace_prefixed_tags() {
        // Some sitemaps use namespace prefixes like <ns:url>.
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<ns:urlset xmlns:ns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <ns:url>
    <ns:loc>https://example.com/ns</ns:loc>
  </ns:url>
</ns:urlset>"#;

        let sitemap = Sitemap::parse(xml).unwrap();
        assert_eq!(sitemap.entries.len(), 1);
        assert_eq!(sitemap.entries[0].loc, "https://example.com/ns");
    }

    #[test]
    fn test_invalid_priority_ignored() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url>
    <loc>https://example.com/bad-priority</loc>
    <priority>not-a-number</priority>
  </url>
</urlset>"#;

        let sitemap = Sitemap::parse(xml).unwrap();
        assert_eq!(sitemap.entries.len(), 1);
        assert!(sitemap.entries[0].priority.is_none());
    }

    #[test]
    fn test_multiple_entries() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>https://example.com/1</loc><priority>0.9</priority></url>
  <url><loc>https://example.com/2</loc><priority>0.5</priority></url>
  <url><loc>https://example.com/3</loc><priority>0.3</priority></url>
  <url><loc>https://example.com/4</loc><priority>0.1</priority></url>
</urlset>"#;

        let sitemap = Sitemap::parse(xml).unwrap();
        assert_eq!(sitemap.entries.len(), 4);
        assert_eq!(sitemap.entries[0].priority, Some(0.9));
        assert_eq!(sitemap.entries[3].priority, Some(0.1));
    }
}
