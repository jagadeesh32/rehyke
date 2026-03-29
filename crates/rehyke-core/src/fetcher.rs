use crate::config::{CrawlConfig, RetryConfig};
use crate::error::{RehykeError, Result};

use reqwest::{
    header::{self, HeaderMap, HeaderValue},
    Client,
};
use std::time::{Duration, Instant};
use tracing::{debug, warn};
use url::Url;

/// Default user agent string used when none is configured.
const DEFAULT_USER_AGENT: &str = "Rehyke/0.1.0";

// ---------------------------------------------------------------------------
// ContentType
// ---------------------------------------------------------------------------

/// Detected content type of the response.
#[derive(Debug, Clone, PartialEq)]
pub enum ContentType {
    Html,
    Xhtml,
    Xml,
    Rss,
    Atom,
    Json,
    JsonLd,
    Svg,
    PlainText,
    Sitemap,
    Other(String),
}

// ---------------------------------------------------------------------------
// FetchResult
// ---------------------------------------------------------------------------

/// Result of fetching a single URL.
#[derive(Debug, Clone)]
pub struct FetchResult {
    /// The originally requested URL.
    pub url: Url,
    /// HTTP status code returned by the server.
    pub status: u16,
    /// Response headers.
    pub headers: HeaderMap,
    /// Response body decoded to a string.
    pub body: String,
    /// Detected content type of the response.
    pub content_type: ContentType,
    /// Wall-clock time the request took.
    pub elapsed: Duration,
    /// The URL after following all redirects.
    pub final_url: Url,
}

// ---------------------------------------------------------------------------
// Fetcher
// ---------------------------------------------------------------------------

/// HTTP fetcher with retry, rate limiting, and user agent rotation.
pub struct Fetcher {
    client: Client,
    retry_config: RetryConfig,
    user_agent: Option<String>,
    #[allow(dead_code)]
    custom_headers: HeaderMap,
    #[allow(dead_code)]
    timeout: Duration,
}

impl Fetcher {
    // ------------------------------------------------------------------
    // Construction
    // ------------------------------------------------------------------

    /// Build a new [`Fetcher`] from the given [`CrawlConfig`].
    ///
    /// The underlying `reqwest::Client` is configured with:
    /// - HTTP/1.1 and HTTP/2 support (via ALPN negotiation, the default)
    /// - Gzip, Brotli, Deflate, and Zstd decompression
    /// - A maximum of 10 redirect hops
    /// - Connection / read / total timeouts drawn from the config
    /// - A persistent cookie store
    /// - Custom headers from the config
    /// - Optional proxy (uses the first entry in `config.proxies`)
    /// - TLS via rustls (set as the default via reqwest feature flags)
    pub fn new(config: &CrawlConfig) -> Result<Self> {
        let mut default_headers = HeaderMap::new();

        // Merge custom headers from the config.
        for (name, value) in &config.custom_headers {
            let header_name = header::HeaderName::from_bytes(name.as_bytes()).map_err(|e| {
                RehykeError::ConfigError {
                    message: format!("invalid header name '{}': {}", name, e),
                }
            })?;
            let header_value =
                HeaderValue::from_str(value).map_err(|e| RehykeError::ConfigError {
                    message: format!("invalid header value for '{}': {}", name, e),
                })?;
            default_headers.insert(header_name, header_value);
        }

        // Always accept common web content types.
        default_headers
            .entry(header::ACCEPT)
            .or_insert_with(|| {
                HeaderValue::from_static(
                    "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
                )
            });

        default_headers
            .entry(header::ACCEPT_LANGUAGE)
            .or_insert_with(|| HeaderValue::from_static("en-US,en;q=0.9"));

        let mut builder = Client::builder()
            .default_headers(default_headers.clone())
            .timeout(config.timeout)
            .redirect(reqwest::redirect::Policy::limited(10))
            .cookie_store(true)
            .gzip(true)
            .brotli(true)
            .deflate(true)
            .zstd(true)
            .https_only(false);

        // Proxy support -- use the first configured proxy.
        if let Some(proxy_config) = config.proxies.first() {
            let proxy =
                proxy_config.to_reqwest_proxy().map_err(|e| RehykeError::ProxyError {
                    message: format!("invalid proxy URL '{}': {}", proxy_config.url, e),
                })?;
            builder = builder.proxy(proxy);
            debug!(proxy = %proxy_config.url, "configured proxy");
        }

        let client = builder.build().map_err(|e| RehykeError::ConfigError {
            message: format!("failed to build HTTP client: {}", e),
        })?;

        Ok(Self {
            client,
            retry_config: config.retry_config.clone(),
            user_agent: Some(config.user_agent.clone()),
            custom_headers: default_headers,
            timeout: config.timeout,
        })
    }

    // ------------------------------------------------------------------
    // Single fetch (no retry)
    // ------------------------------------------------------------------

    /// Fetch a single URL without any retry logic.
    ///
    /// The caller is responsible for retries; see [`Fetcher::fetch_with_retry`]
    /// for the retry-aware wrapper.
    pub async fn fetch(&self, url: &Url) -> Result<FetchResult> {
        let ua = self.user_agent.as_deref().unwrap_or(DEFAULT_USER_AGENT);

        debug!(url = %url, user_agent = ua, "fetching");

        let start = Instant::now();

        let response = self
            .client
            .get(url.as_str())
            .header(header::USER_AGENT, ua)
            .send()
            .await
            .map_err(|e| classify_reqwest_error(&e, url))?;

        let status = response.status().as_u16();
        let final_url = response.url().clone();
        let headers = response.headers().clone();

        let body = response
            .text()
            .await
            .map_err(|e| classify_reqwest_error(&e, url))?;

        let elapsed = start.elapsed();
        let content_type = detect_content_type(&headers, url, &body);

        debug!(
            url = %url,
            status,
            content_type = ?content_type,
            body_len = body.len(),
            elapsed_ms = elapsed.as_millis(),
            "fetch complete"
        );

        Ok(FetchResult {
            url: url.clone(),
            status,
            headers,
            body,
            content_type,
            elapsed,
            final_url,
        })
    }

    // ------------------------------------------------------------------
    // Fetch with retry
    // ------------------------------------------------------------------

    /// Fetch a URL with automatic retries on transient failures.
    ///
    /// Retries occur on:
    /// - Network / IO errors (if `retry_config.retry_on_network_error`)
    /// - HTTP 429, 500, 502, 503, 504
    ///
    /// Backoff is exponential: `initial_delay * 2^attempt`, capped at
    /// `retry_config.max_delay`.  A `Retry-After` header on 429 responses
    /// is respected when present.
    ///
    /// HTTP 403 is **not** retried -- the anti-detect module is expected to
    /// handle user-agent / proxy rotation separately.
    pub async fn fetch_with_retry(&self, url: &Url) -> Result<FetchResult> {
        let max_retries = self.retry_config.max_retries;
        let mut attempt: u32 = 0;

        loop {
            match self.fetch(url).await {
                Ok(result) => {
                    // Decide whether the status code is retryable.
                    let retryable_statuses = [429, 500, 502, 503, 504];
                    if retryable_statuses.contains(&result.status)
                        && attempt < max_retries
                    {
                        let delay = self.compute_backoff(attempt, Some(&result.headers));
                        warn!(
                            url = %url,
                            status = result.status,
                            attempt = attempt + 1,
                            max_retries,
                            delay_ms = delay.as_millis(),
                            "retryable HTTP status, backing off"
                        );
                        tokio::time::sleep(delay).await;
                        attempt += 1;
                        continue;
                    }

                    // Non-retryable status -- 403 becomes a hard error.
                    if result.status == 403 {
                        return Err(RehykeError::HttpError {
                            url: url.to_string(),
                            status: 403,
                        });
                    }

                    // Any other non-2xx that is not in the retry list is a
                    // hard error as well.
                    if result.status >= 400 {
                        return Err(RehykeError::HttpError {
                            url: url.to_string(),
                            status: result.status,
                        });
                    }

                    return Ok(result);
                }

                Err(e) => {
                    // Network-level errors may be retryable.
                    let retryable = is_network_error(&e)
                        && attempt < max_retries;

                    if retryable {
                        let delay = self.compute_backoff(attempt, None);
                        warn!(
                            url = %url,
                            error = %e,
                            attempt = attempt + 1,
                            max_retries,
                            delay_ms = delay.as_millis(),
                            "network error, retrying"
                        );
                        tokio::time::sleep(delay).await;
                        attempt += 1;
                        continue;
                    }

                    return Err(e);
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    /// Compute exponential backoff delay for the given attempt.
    ///
    /// If headers contain a valid `Retry-After` value (seconds), that value
    /// is used instead of the computed backoff.
    fn compute_backoff(&self, attempt: u32, headers: Option<&HeaderMap>) -> Duration {
        // Check for Retry-After header.
        if let Some(hdrs) = headers {
            if let Some(ra) = hdrs.get(header::RETRY_AFTER) {
                if let Ok(s) = ra.to_str() {
                    if let Ok(secs) = s.trim().parse::<u64>() {
                        let capped = Duration::from_secs(secs).min(self.retry_config.max_delay);
                        return capped;
                    }
                }
            }
        }

        let delay = self
            .retry_config
            .initial_delay
            .saturating_mul(2u32.saturating_pow(attempt));
        delay.min(self.retry_config.max_delay)
    }
}

// ---------------------------------------------------------------------------
// Free functions
// ---------------------------------------------------------------------------

/// Map a `reqwest::Error` to the most specific `RehykeError` variant.
fn classify_reqwest_error(err: &reqwest::Error, url: &Url) -> RehykeError {
    if err.is_timeout() {
        return RehykeError::Timeout {
            url: url.to_string(),
        };
    }

    if err.is_connect() {
        // Attempt to distinguish DNS failures from general connection errors.
        let msg = err.to_string().to_lowercase();
        if msg.contains("dns") || msg.contains("resolve") || msg.contains("name resolution") {
            return RehykeError::DnsError {
                domain: url.host_str().unwrap_or("<unknown>").to_string(),
            };
        }
    }

    // reqwest wraps rustls / native-tls errors; sniff the Display output.
    let msg = err.to_string().to_lowercase();
    if msg.contains("tls") || msg.contains("ssl") || msg.contains("certificate") {
        return RehykeError::TlsError {
            url: url.to_string(),
            message: err.to_string(),
        };
    }

    if let Some(status) = err.status() {
        return RehykeError::HttpError {
            url: url.to_string(),
            status: status.as_u16(),
        };
    }

    // Fall back to a descriptive ConfigError since we only have a reference
    // to the reqwest::Error (which is not Clone) and cannot construct a
    // RequestError variant without ownership.
    RehykeError::ConfigError {
        message: format!("request failed for {}: {}", url, err),
    }
}

/// Return `true` if the error is a transient network-level failure that is
/// reasonable to retry.
fn is_network_error(err: &RehykeError) -> bool {
    matches!(
        err,
        RehykeError::Timeout { .. }
            | RehykeError::DnsError { .. }
            | RehykeError::RequestError(_)
            | RehykeError::ConfigError { .. }
    )
}

/// Detect the [`ContentType`] of a response from (in order of priority):
///
/// 1. The `Content-Type` header
/// 2. The URL file extension
/// 3. A best-effort sniff of the response body
///
/// Falls back to [`ContentType::Html`] when no signal is available.
pub fn detect_content_type(headers: &HeaderMap, url: &Url, body: &str) -> ContentType {
    // ----- 1. Content-Type header -----
    if let Some(ct) = headers.get(header::CONTENT_TYPE) {
        if let Ok(ct_str) = ct.to_str() {
            let ct_lower = ct_str.to_lowercase();

            if ct_lower.contains("text/html") {
                return ContentType::Html;
            }
            if ct_lower.contains("application/xhtml+xml") {
                return ContentType::Xhtml;
            }
            if ct_lower.contains("application/rss+xml") {
                return ContentType::Rss;
            }
            if ct_lower.contains("application/atom+xml") {
                return ContentType::Atom;
            }
            if ct_lower.contains("application/ld+json") {
                return ContentType::JsonLd;
            }
            if ct_lower.contains("application/json") || ct_lower.contains("text/json") {
                return ContentType::Json;
            }
            if ct_lower.contains("image/svg+xml") {
                return ContentType::Svg;
            }
            if ct_lower.contains("text/xml") || ct_lower.contains("application/xml") {
                // Could be sitemap, RSS, Atom, or generic XML -- try body sniff.
                return detect_xml_subtype(body);
            }
            if ct_lower.contains("text/plain") {
                return ContentType::PlainText;
            }

            // Unknown MIME -- record the essence (without parameters).
            let mime_essence = ct_lower
                .split(';')
                .next()
                .unwrap_or(&ct_lower)
                .trim()
                .to_string();
            return ContentType::Other(mime_essence);
        }
    }

    // ----- 2. URL extension -----
    if let Some(ext) = url_extension(url) {
        match ext.as_str() {
            "html" | "htm" => return ContentType::Html,
            "xhtml" | "xht" => return ContentType::Xhtml,
            "xml" => return detect_xml_subtype(body),
            "rss" => return ContentType::Rss,
            "atom" => return ContentType::Atom,
            "json" => return ContentType::Json,
            "jsonld" => return ContentType::JsonLd,
            "svg" => return ContentType::Svg,
            "txt" | "text" => return ContentType::PlainText,
            _ => {}
        }
    }

    // ----- 3. Body sniffing -----
    detect_from_body(body)
}

/// Determine the specific XML sub-type by inspecting the document body.
fn detect_xml_subtype(body: &str) -> ContentType {
    let trimmed = body.trim_start();

    // Skip the XML declaration if present.
    let inspectable = if trimmed.starts_with("<?xml") {
        // Jump past the declaration to the first real element.
        trimmed
            .find("?>")
            .map(|i| trimmed[i + 2..].trim_start())
            .unwrap_or(trimmed)
    } else {
        trimmed
    };

    let lower = inspectable.to_lowercase();

    if lower.starts_with("<rss") {
        return ContentType::Rss;
    }
    if lower.starts_with("<feed") {
        return ContentType::Atom;
    }
    if lower.starts_with("<urlset") || lower.starts_with("<sitemapindex") {
        return ContentType::Sitemap;
    }
    if lower.starts_with("<svg") {
        return ContentType::Svg;
    }

    ContentType::Xml
}

/// Attempt to infer content type purely from the body text.
fn detect_from_body(body: &str) -> ContentType {
    let trimmed = body.trim_start();
    let lower = trimmed.to_lowercase();

    if lower.starts_with("<?xml") {
        return detect_xml_subtype(body);
    }
    if lower.starts_with("<rss") {
        return ContentType::Rss;
    }
    if lower.starts_with("<feed") {
        return ContentType::Atom;
    }
    if lower.starts_with("<urlset") || lower.starts_with("<sitemapindex") {
        return ContentType::Sitemap;
    }
    if lower.starts_with("<!doctype html") || lower.starts_with("<html") {
        return ContentType::Html;
    }
    if lower.starts_with("<svg") {
        return ContentType::Svg;
    }
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return ContentType::Json;
    }

    // Default to Html for anything that looks remotely like a web page.
    ContentType::Html
}

/// Detect a [`ContentType`] from a plain Content-Type header string and a URL.
///
/// Used by the JS rendering path in `lib.rs` where the raw header string is
/// available but not a full [`HeaderMap`].
pub fn detect_content_type_from_str(content_type: &str, url: &Url) -> ContentType {
    let ct_lower = content_type.to_lowercase();
    if ct_lower.contains("text/html") {
        return ContentType::Html;
    }
    if ct_lower.contains("application/xhtml+xml") {
        return ContentType::Xhtml;
    }
    if ct_lower.contains("application/rss+xml") {
        return ContentType::Rss;
    }
    if ct_lower.contains("application/atom+xml") {
        return ContentType::Atom;
    }
    if ct_lower.contains("application/ld+json") {
        return ContentType::JsonLd;
    }
    if ct_lower.contains("application/json") || ct_lower.contains("text/json") {
        return ContentType::Json;
    }
    if ct_lower.contains("image/svg+xml") {
        return ContentType::Svg;
    }
    if ct_lower.contains("text/xml") || ct_lower.contains("application/xml") {
        return ContentType::Xml;
    }
    if ct_lower.contains("text/plain") {
        return ContentType::PlainText;
    }
    // Fall back to extension-based detection.
    if let Some(ext) = url_extension(url) {
        match ext.as_str() {
            "html" | "htm" => return ContentType::Html,
            "json" => return ContentType::Json,
            "xml" => return ContentType::Xml,
            "txt" => return ContentType::PlainText,
            "svg" => return ContentType::Svg,
            _ => {}
        }
    }
    ContentType::Html
}

/// Extract the lowercase file extension from the URL path, if any.
fn url_extension(url: &Url) -> Option<String> {
    let path = url.path();
    let last_segment = path.rsplit('/').next()?;
    let dot_pos = last_segment.rfind('.')?;
    let ext = &last_segment[dot_pos + 1..];
    if ext.is_empty() {
        None
    } else {
        Some(ext.to_lowercase())
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};

    // ----- helpers -----

    fn make_url(s: &str) -> Url {
        Url::parse(s).expect("test URL should be valid")
    }

    fn headers_with_ct(ct: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(CONTENT_TYPE, HeaderValue::from_str(ct).unwrap());
        h
    }

    // ---------------------------------------------------------------
    // Content type detection: from headers
    // ---------------------------------------------------------------

    #[test]
    fn detect_html_from_header() {
        let h = headers_with_ct("text/html; charset=utf-8");
        let url = make_url("https://example.com/page");
        assert_eq!(detect_content_type(&h, &url, ""), ContentType::Html);
    }

    #[test]
    fn detect_xhtml_from_header() {
        let h = headers_with_ct("application/xhtml+xml");
        let url = make_url("https://example.com/page");
        assert_eq!(detect_content_type(&h, &url, ""), ContentType::Xhtml);
    }

    #[test]
    fn detect_rss_from_header() {
        let h = headers_with_ct("application/rss+xml");
        let url = make_url("https://example.com/feed");
        assert_eq!(detect_content_type(&h, &url, ""), ContentType::Rss);
    }

    #[test]
    fn detect_atom_from_header() {
        let h = headers_with_ct("application/atom+xml");
        let url = make_url("https://example.com/feed");
        assert_eq!(detect_content_type(&h, &url, ""), ContentType::Atom);
    }

    #[test]
    fn detect_json_from_header() {
        let h = headers_with_ct("application/json");
        let url = make_url("https://example.com/api");
        assert_eq!(detect_content_type(&h, &url, ""), ContentType::Json);
    }

    #[test]
    fn detect_json_ld_from_header() {
        let h = headers_with_ct("application/ld+json");
        let url = make_url("https://example.com/data");
        assert_eq!(detect_content_type(&h, &url, ""), ContentType::JsonLd);
    }

    #[test]
    fn detect_svg_from_header() {
        let h = headers_with_ct("image/svg+xml");
        let url = make_url("https://example.com/image");
        assert_eq!(detect_content_type(&h, &url, ""), ContentType::Svg);
    }

    #[test]
    fn detect_plain_text_from_header() {
        let h = headers_with_ct("text/plain");
        let url = make_url("https://example.com/robots.txt");
        assert_eq!(detect_content_type(&h, &url, ""), ContentType::PlainText);
    }

    #[test]
    fn detect_xml_sitemap_from_header_and_body() {
        let h = headers_with_ct("application/xml");
        let url = make_url("https://example.com/sitemap.xml");
        let body = r#"<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"></urlset>"#;
        assert_eq!(detect_content_type(&h, &url, body), ContentType::Sitemap);
    }

    #[test]
    fn detect_xml_rss_from_header_and_body() {
        let h = headers_with_ct("text/xml; charset=utf-8");
        let url = make_url("https://example.com/feed.xml");
        let body = r#"<?xml version="1.0"?><rss version="2.0"><channel></channel></rss>"#;
        assert_eq!(detect_content_type(&h, &url, body), ContentType::Rss);
    }

    #[test]
    fn detect_other_from_header() {
        let h = headers_with_ct("application/pdf");
        let url = make_url("https://example.com/doc.pdf");
        assert_eq!(
            detect_content_type(&h, &url, ""),
            ContentType::Other("application/pdf".to_string())
        );
    }

    // ---------------------------------------------------------------
    // Content type detection: from URL extension
    // ---------------------------------------------------------------

    #[test]
    fn detect_html_from_url_extension() {
        let h = HeaderMap::new();
        let url = make_url("https://example.com/page.html");
        assert_eq!(detect_content_type(&h, &url, ""), ContentType::Html);
    }

    #[test]
    fn detect_htm_from_url_extension() {
        let h = HeaderMap::new();
        let url = make_url("https://example.com/page.htm");
        assert_eq!(detect_content_type(&h, &url, ""), ContentType::Html);
    }

    #[test]
    fn detect_rss_from_url_extension() {
        let h = HeaderMap::new();
        let url = make_url("https://example.com/feed.rss");
        assert_eq!(detect_content_type(&h, &url, ""), ContentType::Rss);
    }

    #[test]
    fn detect_atom_from_url_extension() {
        let h = HeaderMap::new();
        let url = make_url("https://example.com/feed.atom");
        assert_eq!(detect_content_type(&h, &url, ""), ContentType::Atom);
    }

    #[test]
    fn detect_json_from_url_extension() {
        let h = HeaderMap::new();
        let url = make_url("https://example.com/data.json");
        assert_eq!(detect_content_type(&h, &url, ""), ContentType::Json);
    }

    #[test]
    fn detect_svg_from_url_extension() {
        let h = HeaderMap::new();
        let url = make_url("https://example.com/logo.svg");
        assert_eq!(detect_content_type(&h, &url, ""), ContentType::Svg);
    }

    #[test]
    fn detect_txt_from_url_extension() {
        let h = HeaderMap::new();
        let url = make_url("https://example.com/robots.txt");
        assert_eq!(detect_content_type(&h, &url, ""), ContentType::PlainText);
    }

    #[test]
    fn detect_xml_from_url_extension_with_sitemap_body() {
        let h = HeaderMap::new();
        let url = make_url("https://example.com/sitemap.xml");
        let body = r#"<?xml version="1.0"?><urlset></urlset>"#;
        assert_eq!(detect_content_type(&h, &url, body), ContentType::Sitemap);
    }

    #[test]
    fn detect_xhtml_from_url_extension() {
        let h = HeaderMap::new();
        let url = make_url("https://example.com/page.xhtml");
        assert_eq!(detect_content_type(&h, &url, ""), ContentType::Xhtml);
    }

    // ---------------------------------------------------------------
    // Content type detection: from body sniffing
    // ---------------------------------------------------------------

    #[test]
    fn detect_html_from_body_doctype() {
        let h = HeaderMap::new();
        let url = make_url("https://example.com/page");
        let body = "<!DOCTYPE html><html><head></head><body></body></html>";
        assert_eq!(detect_content_type(&h, &url, body), ContentType::Html);
    }

    #[test]
    fn detect_html_from_body_html_tag() {
        let h = HeaderMap::new();
        let url = make_url("https://example.com/page");
        let body = "<html><head></head><body>Hello</body></html>";
        assert_eq!(detect_content_type(&h, &url, body), ContentType::Html);
    }

    #[test]
    fn detect_rss_from_body() {
        let h = HeaderMap::new();
        let url = make_url("https://example.com/feed");
        let body = r#"<rss version="2.0"><channel><title>Blog</title></channel></rss>"#;
        assert_eq!(detect_content_type(&h, &url, body), ContentType::Rss);
    }

    #[test]
    fn detect_atom_from_body() {
        let h = HeaderMap::new();
        let url = make_url("https://example.com/feed");
        let body = r#"<feed xmlns="http://www.w3.org/2005/Atom"><title>Blog</title></feed>"#;
        assert_eq!(detect_content_type(&h, &url, body), ContentType::Atom);
    }

    #[test]
    fn detect_sitemap_from_body() {
        let h = HeaderMap::new();
        let url = make_url("https://example.com/sitemap");
        let body = r#"<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><url><loc>https://example.com/</loc></url></urlset>"#;
        assert_eq!(detect_content_type(&h, &url, body), ContentType::Sitemap);
    }

    #[test]
    fn detect_sitemapindex_from_body() {
        let h = HeaderMap::new();
        let url = make_url("https://example.com/sitemap");
        let body = r#"<?xml version="1.0"?><sitemapindex><sitemap><loc>https://example.com/sitemap1.xml</loc></sitemap></sitemapindex>"#;
        assert_eq!(detect_content_type(&h, &url, body), ContentType::Sitemap);
    }

    #[test]
    fn detect_json_from_body() {
        let h = HeaderMap::new();
        let url = make_url("https://example.com/data");
        let body = r#"{"key": "value"}"#;
        assert_eq!(detect_content_type(&h, &url, body), ContentType::Json);
    }

    #[test]
    fn detect_json_array_from_body() {
        let h = HeaderMap::new();
        let url = make_url("https://example.com/data");
        let body = r#"[1, 2, 3]"#;
        assert_eq!(detect_content_type(&h, &url, body), ContentType::Json);
    }

    #[test]
    fn detect_svg_from_body() {
        let h = HeaderMap::new();
        let url = make_url("https://example.com/image");
        let body =
            r#"<svg xmlns="http://www.w3.org/2000/svg"><circle cx="50" cy="50" r="40"/></svg>"#;
        assert_eq!(detect_content_type(&h, &url, body), ContentType::Svg);
    }

    #[test]
    fn detect_xml_from_body() {
        let h = HeaderMap::new();
        let url = make_url("https://example.com/data");
        let body = r#"<?xml version="1.0"?><root><item>hello</item></root>"#;
        assert_eq!(detect_content_type(&h, &url, body), ContentType::Xml);
    }

    #[test]
    fn detect_default_html_from_empty_body() {
        let h = HeaderMap::new();
        let url = make_url("https://example.com/page");
        assert_eq!(detect_content_type(&h, &url, ""), ContentType::Html);
    }

    // ---------------------------------------------------------------
    // Fetcher construction
    // ---------------------------------------------------------------

    #[test]
    fn fetcher_new_with_default_config() {
        let config = CrawlConfig::default();
        let fetcher = Fetcher::new(&config);
        assert!(fetcher.is_ok(), "Fetcher::new should succeed with defaults");

        let f = fetcher.unwrap();
        assert!(f.user_agent.is_some());
        assert_eq!(f.timeout, config.timeout);
        assert_eq!(f.retry_config.max_retries, 3);
    }

    #[test]
    fn fetcher_new_with_custom_user_agent() {
        let mut config = CrawlConfig::default();
        config.user_agent = "CustomBot/1.0".to_string();
        let fetcher = Fetcher::new(&config).expect("should build");
        assert_eq!(fetcher.user_agent.as_deref(), Some("CustomBot/1.0"));
    }

    #[test]
    fn fetcher_new_with_custom_headers() {
        let mut config = CrawlConfig::default();
        config
            .custom_headers
            .insert("X-Custom".to_string(), "test-value".to_string());
        let fetcher = Fetcher::new(&config).expect("should build");
        assert!(fetcher.custom_headers.contains_key("x-custom"));
    }

    #[test]
    fn fetcher_new_with_custom_timeouts() {
        let mut config = CrawlConfig::default();
        config.timeout = Duration::from_secs(45);
        let fetcher = Fetcher::new(&config).expect("should build");
        assert_eq!(fetcher.timeout, Duration::from_secs(45));
    }

    #[test]
    fn fetcher_new_invalid_header_name() {
        let mut config = CrawlConfig::default();
        config
            .custom_headers
            .insert("invalid header\x00".to_string(), "value".to_string());
        let result = Fetcher::new(&config);
        assert!(result.is_err());
    }

    // ---------------------------------------------------------------
    // url_extension helper
    // ---------------------------------------------------------------

    #[test]
    fn url_extension_html() {
        let url = make_url("https://example.com/page.html");
        assert_eq!(url_extension(&url), Some("html".to_string()));
    }

    #[test]
    fn url_extension_none() {
        let url = make_url("https://example.com/page");
        assert_eq!(url_extension(&url), None);
    }

    #[test]
    fn url_extension_uppercase() {
        let url = make_url("https://example.com/page.HTML");
        assert_eq!(url_extension(&url), Some("html".to_string()));
    }

    #[test]
    fn url_extension_with_query() {
        let url = make_url("https://example.com/page.xml?foo=bar");
        assert_eq!(url_extension(&url), Some("xml".to_string()));
    }
}
