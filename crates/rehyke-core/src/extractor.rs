use crate::utils;
use url::Url;
use scraper::{Html, Selector};
use serde::{Serialize, Deserialize};
use tracing::debug;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// All links extracted from a document, classified by type.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtractedLinks {
    /// Links pointing to pages on the same domain.
    pub internal: Vec<String>,
    /// Links pointing to pages on a different domain.
    pub external: Vec<String>,
    /// Links pointing to a different subdomain of the same root domain.
    pub subdomains: Vec<String>,
    /// Resource URLs: CSS, JS, images, fonts, media.
    pub resources: Vec<String>,
    /// RSS / Atom feed URLs.
    pub feeds: Vec<String>,
    /// Sitemap XML references.
    pub sitemaps: Vec<String>,
}

// ---------------------------------------------------------------------------
// Internal link-type enum
// ---------------------------------------------------------------------------

/// Classification used during extraction before final placement.
#[derive(Debug, Clone, PartialEq)]
enum LinkType {
    /// Navigable page link (anchors, forms, canonical, etc.).
    Page,
    /// CSS, JS, image, font, media resource.
    Resource,
    /// RSS or Atom feed.
    Feed,
    /// Sitemap reference.
    Sitemap,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Extract all links from an HTML document and classify them.
///
/// The following sources are inspected:
///
/// 1. `<a href>` tags
/// 2. `<link href>` tags (stylesheets, canonical, alternate, feeds)
/// 3. `<script src>` tags
/// 4. `<iframe src>` tags
/// 5. `<form action>` tags
/// 6. `<area href>` tags (image maps)
/// 7. `<img src>` and `<img srcset>` tags
/// 8. `<video src>` and `<video poster>` tags
/// 9. `<audio src>` tags
/// 10. `<source src>` tags
/// 11. `<meta http-equiv="refresh" content="...;url=...">` tags
/// 12. `<meta property="og:url|og:image|twitter:image" content="...">` tags
///
/// Relative URLs are resolved against `base_url`. URLs with schemes such as
/// `javascript:`, `mailto:`, `tel:`, and `data:` are skipped.
pub fn extract_links(html: &Html, base_url: &Url) -> ExtractedLinks {
    let mut links = ExtractedLinks::default();

    let collected: Vec<(String, LinkType)> = [
        extract_anchor_links(html, base_url),
        extract_link_tags(html, base_url),
        extract_resource_links(html, base_url),
        extract_meta_links(html, base_url),
    ]
    .into_iter()
    .flatten()
    .collect();

    for (url_str, link_type) in collected {
        if let Ok(url) = Url::parse(&url_str) {
            classify_link(&url, base_url, link_type, &mut links);
        }
    }

    debug!(
        internal = links.internal.len(),
        external = links.external.len(),
        subdomains = links.subdomains.len(),
        resources = links.resources.len(),
        feeds = links.feeds.len(),
        sitemaps = links.sitemaps.len(),
        "extracted links from {}",
        base_url
    );

    links
}

/// Remove duplicate URLs from every category in `links`.
///
/// Preserves the first occurrence of each URL within each list.
pub fn dedup_links(links: &mut ExtractedLinks) {
    dedup_vec(&mut links.internal);
    dedup_vec(&mut links.external);
    dedup_vec(&mut links.subdomains);
    dedup_vec(&mut links.resources);
    dedup_vec(&mut links.feeds);
    dedup_vec(&mut links.sitemaps);
}

/// Extract links from XML content (RSS, Atom, or Sitemap).
///
/// Uses `quick-xml` to parse the document and collects URLs from:
/// - `<link>` elements (text content and `href` attributes)
/// - `<loc>` elements (sitemap `<url><loc>`)
/// - `<url>` wrapper elements in sitemaps (the `<loc>` child is used)
///
/// All URLs are resolved against `base_url` and classified.
pub fn extract_links_from_xml(xml: &str, base_url: &Url) -> ExtractedLinks {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut links = ExtractedLinks::default();
    let mut reader = Reader::from_str(xml);

    // Track what element we are currently inside.
    let mut current_tag: Option<String> = None;
    // Track whether we are inside a sitemap or feed context to classify links.
    let mut in_sitemap_context = false;
    let mut in_feed_context = false;

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let local_name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                let tag_lower = local_name.to_lowercase();

                // Detect document context from root-level elements.
                match tag_lower.as_str() {
                    "urlset" | "sitemapindex" => in_sitemap_context = true,
                    "feed" | "channel" | "rss" => in_feed_context = true,
                    _ => {}
                }

                // For <link> elements with an href attribute (Atom style).
                if tag_lower == "link" {
                    for attr in e.attributes().flatten() {
                        let key =
                            String::from_utf8_lossy(attr.key.local_name().as_ref()).to_lowercase();
                        if key == "href" {
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            if let Some(resolved) = resolve_and_filter(&val, base_url) {
                                let link_type = if in_sitemap_context {
                                    LinkType::Sitemap
                                } else if in_feed_context {
                                    LinkType::Feed
                                } else {
                                    LinkType::Page
                                };
                                if let Ok(url) = Url::parse(&resolved) {
                                    classify_link(&url, base_url, link_type, &mut links);
                                }
                            }
                        }
                    }
                }

                current_tag = Some(tag_lower);
            }
            Ok(Event::Text(ref e)) => {
                if let Some(ref tag) = current_tag {
                    let text = e.unescape().unwrap_or_default().trim().to_string();
                    if text.is_empty() {
                        buf.clear();
                        continue;
                    }

                    match tag.as_str() {
                        "link" => {
                            // RSS-style <link>URL</link>
                            if let Some(resolved) = resolve_and_filter(&text, base_url) {
                                let link_type = if in_sitemap_context {
                                    LinkType::Sitemap
                                } else if in_feed_context {
                                    LinkType::Feed
                                } else {
                                    LinkType::Page
                                };
                                if let Ok(url) = Url::parse(&resolved) {
                                    classify_link(&url, base_url, link_type, &mut links);
                                }
                            }
                        }
                        "loc" => {
                            // Sitemap <loc>URL</loc>
                            if let Some(resolved) = resolve_and_filter(&text, base_url) {
                                let link_type = if in_sitemap_context {
                                    LinkType::Sitemap
                                } else {
                                    LinkType::Page
                                };
                                if let Ok(url) = Url::parse(&resolved) {
                                    classify_link(&url, base_url, link_type, &mut links);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let local_name = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                let tag_lower = local_name.to_lowercase();
                if current_tag.as_deref() == Some(&tag_lower) {
                    current_tag = None;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                debug!("XML parse error: {}", e);
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    links
}

// ---------------------------------------------------------------------------
// Private extraction helpers
// ---------------------------------------------------------------------------

/// Extract links from `<a href="...">`, `<area href="...">`, and
/// `<form action="...">` tags.
fn extract_anchor_links(html: &Html, base_url: &Url) -> Vec<(String, LinkType)> {
    let mut results = Vec::new();

    // <a href>
    if let Ok(sel) = Selector::parse("a[href]") {
        for el in html.select(&sel) {
            if let Some(href) = el.value().attr("href") {
                if let Some(resolved) = resolve_and_filter(href, base_url) {
                    results.push((resolved, LinkType::Page));
                }
            }
        }
    }

    // <area href> (image maps)
    if let Ok(sel) = Selector::parse("area[href]") {
        for el in html.select(&sel) {
            if let Some(href) = el.value().attr("href") {
                if let Some(resolved) = resolve_and_filter(href, base_url) {
                    results.push((resolved, LinkType::Page));
                }
            }
        }
    }

    // <form action>
    if let Ok(sel) = Selector::parse("form[action]") {
        for el in html.select(&sel) {
            if let Some(action) = el.value().attr("action") {
                if let Some(resolved) = resolve_and_filter(action, base_url) {
                    results.push((resolved, LinkType::Page));
                }
            }
        }
    }

    results
}

/// Extract links from `<link>` tags.
///
/// Classifies based on `rel` attribute:
/// - `stylesheet` -> Resource
/// - `icon` / `apple-touch-icon` -> Resource
/// - `alternate` with RSS/Atom type -> Feed
/// - `sitemap` -> Sitemap
/// - `preload` / `prefetch` / `preconnect` -> Resource
/// - Everything else (canonical, prev, next, etc.) -> Page
fn extract_link_tags(html: &Html, base_url: &Url) -> Vec<(String, LinkType)> {
    let mut results = Vec::new();

    let sel = match Selector::parse("link[href]") {
        Ok(s) => s,
        Err(_) => return results,
    };

    for el in html.select(&sel) {
        let href = match el.value().attr("href") {
            Some(h) => h,
            None => continue,
        };

        let resolved = match resolve_and_filter(href, base_url) {
            Some(r) => r,
            None => continue,
        };

        let rel = el
            .value()
            .attr("rel")
            .unwrap_or("")
            .to_lowercase();
        let link_type_attr = el
            .value()
            .attr("type")
            .unwrap_or("")
            .to_lowercase();

        let link_type = if rel.contains("stylesheet") {
            LinkType::Resource
        } else if rel.contains("icon") || rel.contains("apple-touch-icon") {
            LinkType::Resource
        } else if rel.contains("sitemap") {
            LinkType::Sitemap
        } else if rel.contains("alternate") {
            // Check type for feed detection.
            if link_type_attr.contains("rss")
                || link_type_attr.contains("atom")
                || link_type_attr.contains("xml")
            {
                LinkType::Feed
            } else {
                LinkType::Page
            }
        } else if rel.contains("preload")
            || rel.contains("prefetch")
            || rel.contains("preconnect")
        {
            LinkType::Resource
        } else {
            LinkType::Page
        };

        results.push((resolved, link_type));
    }

    results
}

/// Extract links from resource elements: `<script src>`, `<img src>`,
/// `<img srcset>`, `<iframe src>`, `<video src>`, `<video poster>`,
/// `<audio src>`, and `<source src>`.
fn extract_resource_links(html: &Html, base_url: &Url) -> Vec<(String, LinkType)> {
    let mut results = Vec::new();

    // <script src>
    if let Ok(sel) = Selector::parse("script[src]") {
        for el in html.select(&sel) {
            if let Some(src) = el.value().attr("src") {
                if let Some(resolved) = resolve_and_filter(src, base_url) {
                    results.push((resolved, LinkType::Resource));
                }
            }
        }
    }

    // <img src>
    if let Ok(sel) = Selector::parse("img[src]") {
        for el in html.select(&sel) {
            if let Some(src) = el.value().attr("src") {
                if let Some(resolved) = resolve_and_filter(src, base_url) {
                    results.push((resolved, LinkType::Resource));
                }
            }
        }
    }

    // <img srcset>
    if let Ok(sel) = Selector::parse("img[srcset]") {
        for el in html.select(&sel) {
            if let Some(srcset) = el.value().attr("srcset") {
                for url_str in parse_srcset(srcset, base_url) {
                    results.push((url_str, LinkType::Resource));
                }
            }
        }
    }

    // <iframe src> -- classified as Page (navigable content)
    if let Ok(sel) = Selector::parse("iframe[src]") {
        for el in html.select(&sel) {
            if let Some(src) = el.value().attr("src") {
                if let Some(resolved) = resolve_and_filter(src, base_url) {
                    results.push((resolved, LinkType::Page));
                }
            }
        }
    }

    // <video src>
    if let Ok(sel) = Selector::parse("video[src]") {
        for el in html.select(&sel) {
            if let Some(src) = el.value().attr("src") {
                if let Some(resolved) = resolve_and_filter(src, base_url) {
                    results.push((resolved, LinkType::Resource));
                }
            }
        }
    }

    // <video poster>
    if let Ok(sel) = Selector::parse("video[poster]") {
        for el in html.select(&sel) {
            if let Some(poster) = el.value().attr("poster") {
                if let Some(resolved) = resolve_and_filter(poster, base_url) {
                    results.push((resolved, LinkType::Resource));
                }
            }
        }
    }

    // <audio src>
    if let Ok(sel) = Selector::parse("audio[src]") {
        for el in html.select(&sel) {
            if let Some(src) = el.value().attr("src") {
                if let Some(resolved) = resolve_and_filter(src, base_url) {
                    results.push((resolved, LinkType::Resource));
                }
            }
        }
    }

    // <source src>
    if let Ok(sel) = Selector::parse("source[src]") {
        for el in html.select(&sel) {
            if let Some(src) = el.value().attr("src") {
                if let Some(resolved) = resolve_and_filter(src, base_url) {
                    results.push((resolved, LinkType::Resource));
                }
            }
        }
    }

    results
}

/// Extract links from `<meta>` tags.
///
/// Handles:
/// - `<meta http-equiv="refresh" content="...;url=...">` redirect tags
/// - `<meta property="og:url" content="...">` Open Graph URLs
/// - `<meta property="og:image" content="...">` Open Graph images
/// - `<meta name="twitter:image" content="...">` Twitter card images
/// - `<meta property="twitter:image" content="...">` Twitter card images (alt)
fn extract_meta_links(html: &Html, base_url: &Url) -> Vec<(String, LinkType)> {
    let mut results = Vec::new();

    let sel = match Selector::parse("meta") {
        Ok(s) => s,
        Err(_) => return results,
    };

    for el in html.select(&sel) {
        // Meta refresh: <meta http-equiv="refresh" content="5;url=...">
        if let Some(http_equiv) = el.value().attr("http-equiv") {
            if http_equiv.eq_ignore_ascii_case("refresh") {
                if let Some(content) = el.value().attr("content") {
                    if let Some(url_str) = parse_meta_refresh_url(content) {
                        if let Some(resolved) = resolve_and_filter(&url_str, base_url) {
                            results.push((resolved, LinkType::Page));
                        }
                    }
                }
            }
        }

        // Open Graph and Twitter card meta tags.
        let property = el
            .value()
            .attr("property")
            .or_else(|| el.value().attr("name"))
            .unwrap_or("");
        let property_lower = property.to_lowercase();

        let content = match el.value().attr("content") {
            Some(c) => c,
            None => continue,
        };

        match property_lower.as_str() {
            "og:url" => {
                if let Some(resolved) = resolve_and_filter(content, base_url) {
                    results.push((resolved, LinkType::Page));
                }
            }
            "og:image" | "og:image:url" | "og:image:secure_url" => {
                if let Some(resolved) = resolve_and_filter(content, base_url) {
                    results.push((resolved, LinkType::Resource));
                }
            }
            "twitter:image" | "twitter:image:src" => {
                if let Some(resolved) = resolve_and_filter(content, base_url) {
                    results.push((resolved, LinkType::Resource));
                }
            }
            _ => {}
        }
    }

    results
}

/// Parse URLs from an `srcset` attribute value.
///
/// The `srcset` format is a comma-separated list of entries, where each entry
/// consists of a URL optionally followed by a width descriptor (`300w`) or
/// pixel density descriptor (`2x`).
///
/// Example: `"image-small.jpg 300w, image-large.jpg 1024w"`
fn parse_srcset(srcset: &str, base_url: &Url) -> Vec<String> {
    let mut urls = Vec::new();

    for entry in srcset.split(',') {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }

        // The URL is the first whitespace-delimited token.
        let url_part = match trimmed.split_whitespace().next() {
            Some(u) => u,
            None => continue,
        };

        if let Some(resolved) = resolve_and_filter(url_part, base_url) {
            urls.push(resolved);
        }
    }

    urls
}

// ---------------------------------------------------------------------------
// Classification
// ---------------------------------------------------------------------------

/// Classify a resolved URL and push it into the appropriate list in `links`.
fn classify_link(url: &Url, base_url: &Url, link_type: LinkType, links: &mut ExtractedLinks) {
    let url_str = url.to_string();
    match link_type {
        LinkType::Resource => links.resources.push(url_str),
        LinkType::Feed => links.feeds.push(url_str),
        LinkType::Sitemap => links.sitemaps.push(url_str),
        LinkType::Page => {
            if utils::is_same_domain(url, base_url) {
                links.internal.push(url_str);
            } else if utils::is_subdomain(url, base_url) {
                links.subdomains.push(url_str);
            } else {
                links.external.push(url_str);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// URL filtering helpers
// ---------------------------------------------------------------------------

/// Check whether a raw URL string should be skipped entirely.
///
/// Returns `true` for `javascript:`, `mailto:`, `tel:`, `data:` schemes,
/// as well as empty strings and bare fragment identifiers (`#`).
fn should_skip_url(url_str: &str) -> bool {
    let trimmed = url_str.trim();
    if trimmed.is_empty() || trimmed == "#" {
        return true;
    }
    let lower = trimmed.to_lowercase();
    lower.starts_with("javascript:")
        || lower.starts_with("mailto:")
        || lower.starts_with("tel:")
        || lower.starts_with("data:")
}

/// Resolve a potentially relative URL against `base_url` and return the
/// absolute URL string, or `None` if the URL should be skipped.
fn resolve_and_filter(raw: &str, base_url: &Url) -> Option<String> {
    let trimmed = raw.trim();
    if should_skip_url(trimmed) {
        return None;
    }
    let resolved = utils::resolve_url(base_url, trimmed)?;
    Some(resolved.to_string())
}

/// Parse the URL from a `<meta http-equiv="refresh">` content attribute.
///
/// Handles formats like:
/// - `"5;url=https://example.com/page"`
/// - `"0; URL=https://example.com/page"`
/// - `"5;URL='https://example.com/page'"`
fn parse_meta_refresh_url(content: &str) -> Option<String> {
    // Find the "url=" portion (case-insensitive).
    let lower = content.to_lowercase();
    let url_pos = lower.find("url=")?;
    let after_url = &content[url_pos + 4..];
    let trimmed = after_url.trim();

    // Strip optional quotes (single or double).
    let url_str = if (trimmed.starts_with('\'') && trimmed.ends_with('\''))
        || (trimmed.starts_with('"') && trimmed.ends_with('"'))
    {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    };

    let url_str = url_str.trim();
    if url_str.is_empty() {
        None
    } else {
        Some(url_str.to_string())
    }
}

// ---------------------------------------------------------------------------
// Deduplication helper
// ---------------------------------------------------------------------------

/// Deduplicate a `Vec<String>` while preserving order (first occurrence wins).
fn dedup_vec(vec: &mut Vec<String>) {
    let mut seen = std::collections::HashSet::new();
    vec.retain(|item| seen.insert(item.clone()));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use scraper::Html;
    use url::Url;

    fn base() -> Url {
        Url::parse("https://example.com/page").unwrap()
    }

    fn parse_html(html_str: &str) -> Html {
        Html::parse_document(html_str)
    }

    // -----------------------------------------------------------------------
    // Anchor link extraction (absolute and relative)
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_anchor_absolute() {
        let html = parse_html(
            r#"<html><body><a href="https://example.com/about">About</a></body></html>"#,
        );
        let links = extract_links(&html, &base());
        assert!(links.internal.contains(&"https://example.com/about".to_string()));
    }

    #[test]
    fn test_extract_anchor_relative() {
        let html = parse_html(
            r#"<html><body><a href="/contact">Contact</a></body></html>"#,
        );
        let links = extract_links(&html, &base());
        assert!(links.internal.contains(&"https://example.com/contact".to_string()));
    }

    #[test]
    fn test_extract_anchor_relative_path() {
        let html = parse_html(
            r#"<html><body><a href="other">Other</a></body></html>"#,
        );
        let links = extract_links(&html, &base());
        assert!(links.internal.contains(&"https://example.com/other".to_string()));
    }

    #[test]
    fn test_extract_anchor_external() {
        let html = parse_html(
            r#"<html><body><a href="https://other.com/page">Other</a></body></html>"#,
        );
        let links = extract_links(&html, &base());
        assert!(links.external.contains(&"https://other.com/page".to_string()));
    }

    #[test]
    fn test_extract_anchor_subdomain() {
        let html = parse_html(
            r#"<html><body><a href="https://blog.example.com/post">Blog</a></body></html>"#,
        );
        let links = extract_links(&html, &base());
        assert!(links.subdomains.contains(&"https://blog.example.com/post".to_string()));
    }

    // -----------------------------------------------------------------------
    // <link> tag extraction
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_link_stylesheet() {
        let html = parse_html(
            r#"<html><head><link rel="stylesheet" href="/style.css"></head><body></body></html>"#,
        );
        let links = extract_links(&html, &base());
        assert!(links.resources.contains(&"https://example.com/style.css".to_string()));
    }

    #[test]
    fn test_extract_link_canonical() {
        let html = parse_html(
            r#"<html><head><link rel="canonical" href="https://example.com/canonical"></head><body></body></html>"#,
        );
        let links = extract_links(&html, &base());
        assert!(links.internal.contains(&"https://example.com/canonical".to_string()));
    }

    #[test]
    fn test_extract_link_icon() {
        let html = parse_html(
            r#"<html><head><link rel="icon" href="/favicon.ico"></head><body></body></html>"#,
        );
        let links = extract_links(&html, &base());
        assert!(links.resources.contains(&"https://example.com/favicon.ico".to_string()));
    }

    // -----------------------------------------------------------------------
    // Resource extraction (<img>, <script>, <iframe>)
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_img_src() {
        let html = parse_html(
            r#"<html><body><img src="/images/photo.jpg"></body></html>"#,
        );
        let links = extract_links(&html, &base());
        assert!(links
            .resources
            .contains(&"https://example.com/images/photo.jpg".to_string()));
    }

    #[test]
    fn test_extract_script_src() {
        let html = parse_html(
            r#"<html><head><script src="/js/app.js"></script></head><body></body></html>"#,
        );
        let links = extract_links(&html, &base());
        assert!(links.resources.contains(&"https://example.com/js/app.js".to_string()));
    }

    #[test]
    fn test_extract_iframe_src() {
        let html = parse_html(
            r#"<html><body><iframe src="https://example.com/embed"></iframe></body></html>"#,
        );
        let links = extract_links(&html, &base());
        // Iframes are classified as pages (navigable content).
        assert!(links.internal.contains(&"https://example.com/embed".to_string()));
    }

    #[test]
    fn test_extract_video_src() {
        let html = parse_html(
            r#"<html><body><video src="/media/video.mp4"></video></body></html>"#,
        );
        let links = extract_links(&html, &base());
        assert!(links
            .resources
            .contains(&"https://example.com/media/video.mp4".to_string()));
    }

    #[test]
    fn test_extract_video_poster() {
        let html = parse_html(
            r#"<html><body><video poster="/media/poster.jpg" src="/media/video.mp4"></video></body></html>"#,
        );
        let links = extract_links(&html, &base());
        assert!(links
            .resources
            .contains(&"https://example.com/media/poster.jpg".to_string()));
    }

    #[test]
    fn test_extract_audio_src() {
        let html = parse_html(
            r#"<html><body><audio src="/media/song.mp3"></audio></body></html>"#,
        );
        let links = extract_links(&html, &base());
        assert!(links
            .resources
            .contains(&"https://example.com/media/song.mp3".to_string()));
    }

    #[test]
    fn test_extract_source_src() {
        let html = parse_html(
            r#"<html><body><video><source src="/media/video.webm"></video></body></html>"#,
        );
        let links = extract_links(&html, &base());
        assert!(links
            .resources
            .contains(&"https://example.com/media/video.webm".to_string()));
    }

    // -----------------------------------------------------------------------
    // srcset parsing
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_srcset_single() {
        let base = Url::parse("https://example.com/").unwrap();
        let result = parse_srcset("image.jpg 1x", &base);
        assert_eq!(result, vec!["https://example.com/image.jpg"]);
    }

    #[test]
    fn test_parse_srcset_multiple() {
        let base = Url::parse("https://example.com/").unwrap();
        let result = parse_srcset("small.jpg 300w, large.jpg 1024w", &base);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&"https://example.com/small.jpg".to_string()));
        assert!(result.contains(&"https://example.com/large.jpg".to_string()));
    }

    #[test]
    fn test_parse_srcset_absolute_urls() {
        let base = Url::parse("https://example.com/").unwrap();
        let result = parse_srcset("https://cdn.example.com/img.jpg 2x", &base);
        assert_eq!(result, vec!["https://cdn.example.com/img.jpg"]);
    }

    #[test]
    fn test_extract_img_srcset() {
        let html = parse_html(
            r#"<html><body><img srcset="small.jpg 300w, large.jpg 1024w" src="small.jpg"></body></html>"#,
        );
        let base = Url::parse("https://example.com/").unwrap();
        let links = extract_links(&html, &base);
        assert!(links.resources.contains(&"https://example.com/small.jpg".to_string()));
        assert!(links.resources.contains(&"https://example.com/large.jpg".to_string()));
    }

    // -----------------------------------------------------------------------
    // URL classification (internal vs external vs subdomain)
    // -----------------------------------------------------------------------

    #[test]
    fn test_classify_internal() {
        let mut links = ExtractedLinks::default();
        let url = Url::parse("https://example.com/page2").unwrap();
        classify_link(&url, &base(), LinkType::Page, &mut links);
        assert_eq!(links.internal, vec!["https://example.com/page2"]);
        assert!(links.external.is_empty());
        assert!(links.subdomains.is_empty());
    }

    #[test]
    fn test_classify_external() {
        let mut links = ExtractedLinks::default();
        let url = Url::parse("https://other.com/page").unwrap();
        classify_link(&url, &base(), LinkType::Page, &mut links);
        assert!(links.internal.is_empty());
        assert_eq!(links.external, vec!["https://other.com/page"]);
    }

    #[test]
    fn test_classify_subdomain() {
        let mut links = ExtractedLinks::default();
        let url = Url::parse("https://blog.example.com/post").unwrap();
        classify_link(&url, &base(), LinkType::Page, &mut links);
        assert!(links.internal.is_empty());
        assert_eq!(links.subdomains, vec!["https://blog.example.com/post"]);
    }

    #[test]
    fn test_classify_resource() {
        let mut links = ExtractedLinks::default();
        let url = Url::parse("https://example.com/style.css").unwrap();
        classify_link(&url, &base(), LinkType::Resource, &mut links);
        assert_eq!(links.resources, vec!["https://example.com/style.css"]);
        assert!(links.internal.is_empty());
    }

    #[test]
    fn test_classify_feed() {
        let mut links = ExtractedLinks::default();
        let url = Url::parse("https://example.com/feed.xml").unwrap();
        classify_link(&url, &base(), LinkType::Feed, &mut links);
        assert_eq!(links.feeds, vec!["https://example.com/feed.xml"]);
    }

    #[test]
    fn test_classify_sitemap() {
        let mut links = ExtractedLinks::default();
        let url = Url::parse("https://example.com/sitemap.xml").unwrap();
        classify_link(&url, &base(), LinkType::Sitemap, &mut links);
        assert_eq!(links.sitemaps, vec!["https://example.com/sitemap.xml"]);
    }

    // -----------------------------------------------------------------------
    // Skipping javascript: / mailto: / data: / tel: URLs
    // -----------------------------------------------------------------------

    #[test]
    fn test_skip_javascript_url() {
        assert!(should_skip_url("javascript:void(0)"));
        assert!(should_skip_url("JAVASCRIPT:alert(1)"));
    }

    #[test]
    fn test_skip_mailto_url() {
        assert!(should_skip_url("mailto:user@example.com"));
    }

    #[test]
    fn test_skip_tel_url() {
        assert!(should_skip_url("tel:+1234567890"));
    }

    #[test]
    fn test_skip_data_url() {
        assert!(should_skip_url("data:image/png;base64,abc"));
    }

    #[test]
    fn test_skip_empty_and_hash() {
        assert!(should_skip_url(""));
        assert!(should_skip_url("#"));
        assert!(should_skip_url("  "));
    }

    #[test]
    fn test_no_skip_normal_urls() {
        assert!(!should_skip_url("https://example.com"));
        assert!(!should_skip_url("/relative/path"));
        assert!(!should_skip_url("page.html"));
    }

    #[test]
    fn test_skip_urls_in_html() {
        let html = parse_html(
            r##"<html><body>
            <a href="javascript:void(0)">JS</a>
            <a href="mailto:test@example.com">Mail</a>
            <a href="tel:+123">Call</a>
            <a href="#">Top</a>
            <a href="https://example.com/real">Real</a>
            </body></html>"##,
        );
        let links = extract_links(&html, &base());
        // Only the real link should be extracted.
        assert_eq!(links.internal.len(), 1);
        assert!(links.internal.contains(&"https://example.com/real".to_string()));
    }

    // -----------------------------------------------------------------------
    // Deduplication
    // -----------------------------------------------------------------------

    #[test]
    fn test_dedup_links() {
        let mut links = ExtractedLinks::default();
        links.internal.push("https://example.com/a".to_string());
        links.internal.push("https://example.com/a".to_string());
        links.internal.push("https://example.com/b".to_string());
        links.external.push("https://other.com/x".to_string());
        links.external.push("https://other.com/x".to_string());
        links
            .resources
            .push("https://example.com/style.css".to_string());
        links
            .resources
            .push("https://example.com/style.css".to_string());

        dedup_links(&mut links);

        assert_eq!(
            links.internal,
            vec!["https://example.com/a", "https://example.com/b"]
        );
        assert_eq!(links.external, vec!["https://other.com/x"]);
        assert_eq!(links.resources, vec!["https://example.com/style.css"]);
    }

    #[test]
    fn test_dedup_preserves_order() {
        let mut links = ExtractedLinks::default();
        links.internal.push("https://example.com/c".to_string());
        links.internal.push("https://example.com/a".to_string());
        links.internal.push("https://example.com/c".to_string());
        links.internal.push("https://example.com/b".to_string());
        links.internal.push("https://example.com/a".to_string());

        dedup_links(&mut links);

        assert_eq!(
            links.internal,
            vec![
                "https://example.com/c",
                "https://example.com/a",
                "https://example.com/b"
            ]
        );
    }

    // -----------------------------------------------------------------------
    // Meta refresh extraction
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_meta_refresh_url_simple() {
        let result = parse_meta_refresh_url("5;url=https://example.com/new");
        assert_eq!(result, Some("https://example.com/new".to_string()));
    }

    #[test]
    fn test_parse_meta_refresh_url_case_insensitive() {
        let result = parse_meta_refresh_url("0; URL=https://example.com/redirect");
        assert_eq!(result, Some("https://example.com/redirect".to_string()));
    }

    #[test]
    fn test_parse_meta_refresh_url_quoted() {
        let result = parse_meta_refresh_url("5;URL='https://example.com/quoted'");
        assert_eq!(result, Some("https://example.com/quoted".to_string()));
    }

    #[test]
    fn test_parse_meta_refresh_no_url() {
        let result = parse_meta_refresh_url("5");
        assert!(result.is_none());
    }

    #[test]
    fn test_meta_refresh_in_html() {
        let html = parse_html(
            r#"<html><head><meta http-equiv="refresh" content="5;url=https://example.com/new"></head><body></body></html>"#,
        );
        let links = extract_links(&html, &base());
        assert!(links.internal.contains(&"https://example.com/new".to_string()));
    }

    // -----------------------------------------------------------------------
    // Feed link detection
    // -----------------------------------------------------------------------

    #[test]
    fn test_feed_link_rss() {
        let html = parse_html(
            r#"<html><head><link rel="alternate" type="application/rss+xml" href="/feed.xml"></head><body></body></html>"#,
        );
        let links = extract_links(&html, &base());
        assert!(links.feeds.contains(&"https://example.com/feed.xml".to_string()));
    }

    #[test]
    fn test_feed_link_atom() {
        let html = parse_html(
            r#"<html><head><link rel="alternate" type="application/atom+xml" href="/atom.xml"></head><body></body></html>"#,
        );
        let links = extract_links(&html, &base());
        assert!(links.feeds.contains(&"https://example.com/atom.xml".to_string()));
    }

    #[test]
    fn test_sitemap_link() {
        let html = parse_html(
            r#"<html><head><link rel="sitemap" href="/sitemap.xml"></head><body></body></html>"#,
        );
        let links = extract_links(&html, &base());
        assert!(links.sitemaps.contains(&"https://example.com/sitemap.xml".to_string()));
    }

    // -----------------------------------------------------------------------
    // XML / Sitemap link extraction
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_links_from_sitemap_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url>
    <loc>https://example.com/page1</loc>
  </url>
  <url>
    <loc>https://example.com/page2</loc>
  </url>
</urlset>"#;
        let base = Url::parse("https://example.com/").unwrap();
        let links = extract_links_from_xml(xml, &base);
        assert_eq!(links.sitemaps.len(), 2);
        assert!(links.sitemaps.contains(&"https://example.com/page1".to_string()));
        assert!(links.sitemaps.contains(&"https://example.com/page2".to_string()));
    }

    #[test]
    fn test_extract_links_from_rss_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Example</title>
    <link>https://example.com</link>
    <item>
      <title>Post 1</title>
      <link>https://example.com/post1</link>
    </item>
    <item>
      <title>Post 2</title>
      <link>https://example.com/post2</link>
    </item>
  </channel>
</rss>"#;
        let base = Url::parse("https://example.com/").unwrap();
        let links = extract_links_from_xml(xml, &base);
        // RSS links should be classified as feeds since we are inside <channel>.
        assert!(!links.feeds.is_empty());
        let all_feeds: Vec<&String> = links.feeds.iter().collect();
        assert!(all_feeds.iter().any(|u| u.contains("example.com")));
    }

    #[test]
    fn test_extract_links_from_atom_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Example Feed</title>
  <link href="https://example.com/"/>
  <entry>
    <title>Entry 1</title>
    <link href="https://example.com/entry1"/>
  </entry>
</feed>"#;
        let base = Url::parse("https://example.com/").unwrap();
        let links = extract_links_from_xml(xml, &base);
        assert!(!links.feeds.is_empty());
        let all: Vec<String> = links
            .feeds
            .iter()
            .chain(links.internal.iter())
            .cloned()
            .collect();
        assert!(all.iter().any(|u| u.contains("example.com")));
    }

    #[test]
    fn test_extract_links_from_sitemap_index_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <sitemap>
    <loc>https://example.com/sitemap1.xml</loc>
  </sitemap>
  <sitemap>
    <loc>https://example.com/sitemap2.xml</loc>
  </sitemap>
</sitemapindex>"#;
        let base = Url::parse("https://example.com/").unwrap();
        let links = extract_links_from_xml(xml, &base);
        assert_eq!(links.sitemaps.len(), 2);
        assert!(links
            .sitemaps
            .contains(&"https://example.com/sitemap1.xml".to_string()));
        assert!(links
            .sitemaps
            .contains(&"https://example.com/sitemap2.xml".to_string()));
    }

    // -----------------------------------------------------------------------
    // <area> and <form> tags
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_area_href() {
        let html = parse_html(
            r#"<html><body><map><area href="/region1"><area href="/region2"></map></body></html>"#,
        );
        let links = extract_links(&html, &base());
        assert!(links.internal.contains(&"https://example.com/region1".to_string()));
        assert!(links.internal.contains(&"https://example.com/region2".to_string()));
    }

    #[test]
    fn test_extract_form_action() {
        let html = parse_html(
            r#"<html><body><form action="/search"></form></body></html>"#,
        );
        let links = extract_links(&html, &base());
        assert!(links.internal.contains(&"https://example.com/search".to_string()));
    }

    // -----------------------------------------------------------------------
    // OG / Twitter meta tags
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_og_url() {
        let html = parse_html(
            r#"<html><head><meta property="og:url" content="https://example.com/canonical-page"></head><body></body></html>"#,
        );
        let links = extract_links(&html, &base());
        assert!(links
            .internal
            .contains(&"https://example.com/canonical-page".to_string()));
    }

    #[test]
    fn test_extract_og_image() {
        let html = parse_html(
            r#"<html><head><meta property="og:image" content="https://example.com/og.jpg"></head><body></body></html>"#,
        );
        let links = extract_links(&html, &base());
        assert!(links.resources.contains(&"https://example.com/og.jpg".to_string()));
    }

    #[test]
    fn test_extract_twitter_image() {
        let html = parse_html(
            r#"<html><head><meta name="twitter:image" content="https://example.com/tw.jpg"></head><body></body></html>"#,
        );
        let links = extract_links(&html, &base());
        assert!(links.resources.contains(&"https://example.com/tw.jpg".to_string()));
    }

    // -----------------------------------------------------------------------
    // Combined / integration test
    // -----------------------------------------------------------------------

    #[test]
    fn test_full_html_document() {
        let html_str = r#"<!DOCTYPE html>
<html>
<head>
    <link rel="stylesheet" href="/css/main.css">
    <link rel="alternate" type="application/rss+xml" href="/feed.xml">
    <link rel="canonical" href="https://example.com/page">
    <link rel="sitemap" href="/sitemap.xml">
    <script src="/js/app.js"></script>
    <meta http-equiv="refresh" content="10;url=https://example.com/redirected">
    <meta property="og:image" content="https://example.com/og-image.png">
</head>
<body>
    <a href="/about">About</a>
    <a href="https://other.com">Other</a>
    <a href="https://blog.example.com/post">Blog Post</a>
    <a href="javascript:void(0)">No-op</a>
    <a href="mailto:test@example.com">Email</a>
    <img src="/images/hero.jpg" srcset="/images/hero-sm.jpg 480w, /images/hero-lg.jpg 1024w">
    <iframe src="https://example.com/widget"></iframe>
    <video src="/video/intro.mp4" poster="/video/poster.jpg"></video>
</body>
</html>"#;

        let html = parse_html(html_str);
        let base = Url::parse("https://example.com/page").unwrap();
        let mut links = extract_links(&html, &base);
        dedup_links(&mut links);

        // Internal pages
        assert!(links.internal.contains(&"https://example.com/about".to_string()));
        assert!(links
            .internal
            .contains(&"https://example.com/page".to_string())); // canonical
        assert!(links
            .internal
            .contains(&"https://example.com/redirected".to_string())); // meta refresh
        assert!(links
            .internal
            .contains(&"https://example.com/widget".to_string())); // iframe

        // External
        assert!(links.external.contains(&"https://other.com/".to_string()));

        // Subdomains
        assert!(links
            .subdomains
            .contains(&"https://blog.example.com/post".to_string()));

        // Resources
        assert!(links
            .resources
            .contains(&"https://example.com/css/main.css".to_string()));
        assert!(links
            .resources
            .contains(&"https://example.com/js/app.js".to_string()));
        assert!(links
            .resources
            .contains(&"https://example.com/images/hero.jpg".to_string()));
        assert!(links
            .resources
            .contains(&"https://example.com/images/hero-sm.jpg".to_string()));
        assert!(links
            .resources
            .contains(&"https://example.com/images/hero-lg.jpg".to_string()));
        assert!(links
            .resources
            .contains(&"https://example.com/og-image.png".to_string()));
        assert!(links
            .resources
            .contains(&"https://example.com/video/intro.mp4".to_string()));
        assert!(links
            .resources
            .contains(&"https://example.com/video/poster.jpg".to_string()));

        // Feeds
        assert!(links.feeds.contains(&"https://example.com/feed.xml".to_string()));

        // Sitemaps
        assert!(links
            .sitemaps
            .contains(&"https://example.com/sitemap.xml".to_string()));

        // javascript: and mailto: should NOT be anywhere
        let all_urls: Vec<&String> = links
            .internal
            .iter()
            .chain(&links.external)
            .chain(&links.subdomains)
            .chain(&links.resources)
            .chain(&links.feeds)
            .chain(&links.sitemaps)
            .collect();
        assert!(!all_urls.iter().any(|u| u.starts_with("javascript:")));
        assert!(!all_urls.iter().any(|u| u.starts_with("mailto:")));
    }
}
