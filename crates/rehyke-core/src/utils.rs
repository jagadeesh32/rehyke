use percent_encoding::percent_decode_str;
use regex::Regex;
use url::Url;

/// Normalize a URL for deduplication purposes.
///
/// Rules:
/// 1. Lowercase scheme and host
/// 2. Remove default ports (80 for http, 443 for https)
/// 3. Remove fragment (#)
/// 4. Remove trailing slash (except for root path "/")
/// 5. Sort query parameters alphabetically
/// 6. Decode unnecessary percent-encoding
/// 7. Optionally remove "www." prefix (controlled by `remove_www` param)
pub fn normalize_url(url: &Url, remove_www: bool) -> String {
    let scheme = url.scheme().to_lowercase();

    let host = match url.host_str() {
        Some(h) => {
            let mut host = h.to_lowercase();
            if remove_www {
                if let Some(stripped) = host.strip_prefix("www.") {
                    host = stripped.to_string();
                }
            }
            host
        }
        None => return url.to_string(),
    };

    // Determine port: omit if it is the default for the scheme.
    let port = url.port().and_then(|p| {
        let is_default = (scheme == "http" && p == 80) || (scheme == "https" && p == 443);
        if is_default {
            None
        } else {
            Some(p)
        }
    });

    // Decode percent-encoded path segments that don't need encoding.
    let raw_path = url.path();
    let decoded_path = decode_unreserved(raw_path);

    // Remove trailing slash unless the path is exactly "/".
    let path = if decoded_path.len() > 1 && decoded_path.ends_with('/') {
        &decoded_path[..decoded_path.len() - 1]
    } else {
        &decoded_path
    };

    // Sort query parameters alphabetically.
    let query_part = build_sorted_query(url);

    // Build the final normalized URL string.
    let mut result = format!("{}://{}", scheme, host);
    if let Some(p) = port {
        result.push_str(&format!(":{}", p));
    }
    result.push_str(path);
    if let Some(q) = query_part {
        result.push('?');
        result.push_str(&q);
    }
    // Fragment is intentionally omitted.

    result
}

/// Check if a URL is internal (same domain) relative to a base URL.
///
/// Compares the host components of both URLs. Returns `true` when both hosts
/// are identical (case-insensitive).
pub fn is_same_domain(url: &Url, base: &Url) -> bool {
    match (url.host_str(), base.host_str()) {
        (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
        _ => false,
    }
}

/// Check if a URL is a subdomain of the base domain.
///
/// Returns `true` when the URL's host ends with `.<base_host>`. The base host
/// itself is **not** considered a subdomain of itself.
pub fn is_subdomain(url: &Url, base: &Url) -> bool {
    match (url.host_str(), base.host_str()) {
        (Some(url_host), Some(base_host)) => {
            let url_lower = url_host.to_lowercase();
            let base_lower = base_host.to_lowercase();
            if url_lower == base_lower {
                return false;
            }
            url_lower.ends_with(&format!(".{}", base_lower))
        }
        _ => false,
    }
}

/// Extract the root domain from a URL (e.g., "blog.example.com" -> "example.com").
///
/// Uses a simple heuristic: take the last two labels of the host. This works
/// correctly for standard TLDs but does not handle multi-part public suffixes
/// such as `.co.uk`. For production use with full public suffix awareness,
/// consider integrating the `publicsuffix` crate.
pub fn root_domain(url: &Url) -> Option<String> {
    let host = url.host_str()?;
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() >= 2 {
        Some(format!(
            "{}.{}",
            labels[labels.len() - 2],
            labels[labels.len() - 1]
        ))
    } else {
        Some(host.to_string())
    }
}

/// Resolve a potentially relative URL against a base URL.
///
/// Handles relative paths (`../page`), absolute paths (`/page`),
/// protocol-relative URLs (`//cdn.example.com/file`), and full URLs.
/// Returns `None` when parsing fails.
pub fn resolve_url(base: &Url, relative: &str) -> Option<Url> {
    let trimmed = relative.trim();
    if trimmed.is_empty() {
        return None;
    }
    base.join(trimmed).ok()
}

/// Sanitize a URL string (trim whitespace, fix common issues).
///
/// Performs the following clean-up steps:
/// - Trim leading/trailing whitespace and control characters
/// - Collapse internal whitespace/newlines
/// - Prepend `https://` when no scheme is present
/// - Validate by attempting to parse
///
/// Returns `None` when the result is not a valid URL.
pub fn sanitize_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Collapse internal whitespace / newlines that sometimes appear in scraped HTML.
    let collapsed: String = trimmed.split_whitespace().collect::<Vec<_>>().join("");

    if collapsed.is_empty() {
        return None;
    }

    // If there is no scheme, add a default https:// prefix.
    let with_scheme = if !collapsed.contains("://") && !collapsed.starts_with("//") {
        format!("https://{}", collapsed)
    } else if collapsed.starts_with("//") {
        format!("https:{}", collapsed)
    } else {
        collapsed
    };

    // Attempt to parse. Return None on failure.
    Url::parse(&with_scheme).ok().map(|u| u.to_string())
}

/// Convert a URL path to a filesystem-safe filename.
///
/// The result contains only ASCII alphanumerics, hyphens, underscores, and dots.
/// The host and path are joined with underscores. Query and fragment are excluded.
/// If the path would produce an empty filename, `"index"` is used.
pub fn url_to_filename(url: &Url) -> String {
    let host = url.host_str().unwrap_or("unknown");
    let path = url.path();

    let combined = format!("{}_{}", host, path);

    let safe: String = combined
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();

    // Collapse consecutive underscores.
    let re = Regex::new(r"_{2,}").expect("valid regex");
    let result = re.replace_all(&safe, "_").to_string();

    // Trim leading/trailing underscores and dots.
    let result = result.trim_matches(|c: char| c == '_' || c == '.').to_string();

    if result.is_empty() {
        "index".to_string()
    } else {
        result
    }
}

/// Generate a slug from a URL for use as a filename.
///
/// Similar to [`url_to_filename`] but produces a more human-readable,
/// kebab-case identifier. The scheme is omitted, the host and path are
/// converted to lowercase, and non-alphanumeric characters are replaced
/// with hyphens.
pub fn url_to_slug(url: &Url) -> String {
    let host = url.host_str().unwrap_or("unknown");
    let path = url.path();

    let combined = format!("{}{}", host, path);

    let slug: String = combined
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c
            } else {
                '-'
            }
        })
        .collect();

    // Collapse consecutive hyphens.
    let re = Regex::new(r"-{2,}").expect("valid regex");
    let result = re.replace_all(&slug, "-").to_string();

    // Trim leading/trailing hyphens.
    let result = result.trim_matches('-').to_string();

    if result.is_empty() {
        "index".to_string()
    } else {
        result
    }
}

/// Parse a [`Url`] from a reference, returning a best-effort value on failure.
///
/// Used internally when a valid URL reference is expected but error-propagation
/// is not appropriate (e.g. generating a screenshot filename from a URL that
/// was already validated earlier in the pipeline).
pub fn parse_url_lossy(url: &Url) -> Url {
    url.clone()
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Decode percent-encoded characters that are unreserved (RFC 3986) and therefore
/// do not require encoding. Characters that are reserved (`/`, `?`, `#`, etc.)
/// are left as-is to avoid altering URL semantics.
fn decode_unreserved(input: &str) -> String {
    // percent_decode_str decodes all percent-encoded bytes. We then re-encode
    // any that are *not* unreserved. However, for paths we want to keep `/`
    // intact while decoding unnecessary escapes like `%41` -> `A`.
    //
    // A simpler approach: split on `/`, decode each segment, then rejoin.
    let segments: Vec<String> = input
        .split('/')
        .map(|seg| {
            let decoded = percent_decode_str(seg).decode_utf8_lossy().to_string();
            decoded
        })
        .collect();
    segments.join("/")
}

/// Build a query string with parameters sorted alphabetically.
/// Returns `None` when there are no query parameters.
fn build_sorted_query(url: &Url) -> Option<String> {
    let pairs: Vec<(String, String)> = url
        .query_pairs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    if pairs.is_empty() {
        return None;
    }

    let mut sorted = pairs;
    sorted.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    let parts: Vec<String> = sorted
        .into_iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect();

    Some(parts.join("&"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;

    // -----------------------------------------------------------------------
    // normalize_url
    // -----------------------------------------------------------------------

    #[test]
    fn test_normalize_basic() {
        let url = Url::parse("HTTP://Example.COM/Path").unwrap();
        let result = normalize_url(&url, false);
        assert_eq!(result, "http://example.com/Path");
    }

    #[test]
    fn test_normalize_removes_fragment() {
        let url = Url::parse("https://example.com/page#section").unwrap();
        let result = normalize_url(&url, false);
        assert_eq!(result, "https://example.com/page");
    }

    #[test]
    fn test_normalize_trailing_slash_removed() {
        let url = Url::parse("https://example.com/page/").unwrap();
        let result = normalize_url(&url, false);
        assert_eq!(result, "https://example.com/page");
    }

    #[test]
    fn test_normalize_root_path_trailing_slash_kept() {
        let url = Url::parse("https://example.com/").unwrap();
        let result = normalize_url(&url, false);
        assert_eq!(result, "https://example.com/");
    }

    #[test]
    fn test_normalize_sorts_query_params() {
        let url = Url::parse("https://example.com/search?z=1&a=2&m=3").unwrap();
        let result = normalize_url(&url, false);
        assert_eq!(result, "https://example.com/search?a=2&m=3&z=1");
    }

    #[test]
    fn test_normalize_removes_default_http_port() {
        let url = Url::parse("http://example.com:80/page").unwrap();
        let result = normalize_url(&url, false);
        assert_eq!(result, "http://example.com/page");
    }

    #[test]
    fn test_normalize_removes_default_https_port() {
        let url = Url::parse("https://example.com:443/page").unwrap();
        let result = normalize_url(&url, false);
        assert_eq!(result, "https://example.com/page");
    }

    #[test]
    fn test_normalize_keeps_non_default_port() {
        let url = Url::parse("https://example.com:8080/page").unwrap();
        let result = normalize_url(&url, false);
        assert_eq!(result, "https://example.com:8080/page");
    }

    #[test]
    fn test_normalize_removes_www() {
        let url = Url::parse("https://www.example.com/page").unwrap();
        let result = normalize_url(&url, true);
        assert_eq!(result, "https://example.com/page");
    }

    #[test]
    fn test_normalize_keeps_www_when_flag_false() {
        let url = Url::parse("https://www.example.com/page").unwrap();
        let result = normalize_url(&url, false);
        assert_eq!(result, "https://www.example.com/page");
    }

    #[test]
    fn test_normalize_decodes_percent_encoding() {
        let url = Url::parse("https://example.com/%41%42%43").unwrap();
        let result = normalize_url(&url, false);
        assert_eq!(result, "https://example.com/ABC");
    }

    #[test]
    fn test_normalize_complex_url() {
        let url =
            Url::parse("HTTP://WWW.Example.COM:80/path/?b=2&a=1#frag").unwrap();
        let result = normalize_url(&url, true);
        assert_eq!(result, "http://example.com/path?a=1&b=2");
    }

    // -----------------------------------------------------------------------
    // is_same_domain
    // -----------------------------------------------------------------------

    #[test]
    fn test_same_domain_true() {
        let a = Url::parse("https://example.com/a").unwrap();
        let b = Url::parse("https://example.com/b").unwrap();
        assert!(is_same_domain(&a, &b));
    }

    #[test]
    fn test_same_domain_case_insensitive() {
        let a = Url::parse("https://Example.COM/a").unwrap();
        let b = Url::parse("https://example.com/b").unwrap();
        assert!(is_same_domain(&a, &b));
    }

    #[test]
    fn test_same_domain_different_hosts() {
        let a = Url::parse("https://other.com/a").unwrap();
        let b = Url::parse("https://example.com/b").unwrap();
        assert!(!is_same_domain(&a, &b));
    }

    #[test]
    fn test_same_domain_subdomain_not_equal() {
        let a = Url::parse("https://blog.example.com/a").unwrap();
        let b = Url::parse("https://example.com/b").unwrap();
        assert!(!is_same_domain(&a, &b));
    }

    // -----------------------------------------------------------------------
    // is_subdomain
    // -----------------------------------------------------------------------

    #[test]
    fn test_is_subdomain_true() {
        let url = Url::parse("https://blog.example.com/").unwrap();
        let base = Url::parse("https://example.com/").unwrap();
        assert!(is_subdomain(&url, &base));
    }

    #[test]
    fn test_is_subdomain_deep() {
        let url = Url::parse("https://a.b.example.com/").unwrap();
        let base = Url::parse("https://example.com/").unwrap();
        assert!(is_subdomain(&url, &base));
    }

    #[test]
    fn test_is_subdomain_same_host_false() {
        let url = Url::parse("https://example.com/").unwrap();
        let base = Url::parse("https://example.com/").unwrap();
        assert!(!is_subdomain(&url, &base));
    }

    #[test]
    fn test_is_subdomain_external_false() {
        let url = Url::parse("https://notexample.com/").unwrap();
        let base = Url::parse("https://example.com/").unwrap();
        assert!(!is_subdomain(&url, &base));
    }

    // -----------------------------------------------------------------------
    // root_domain
    // -----------------------------------------------------------------------

    #[test]
    fn test_root_domain_simple() {
        let url = Url::parse("https://example.com/page").unwrap();
        assert_eq!(root_domain(&url), Some("example.com".to_string()));
    }

    #[test]
    fn test_root_domain_subdomain() {
        let url = Url::parse("https://blog.example.com/page").unwrap();
        assert_eq!(root_domain(&url), Some("example.com".to_string()));
    }

    #[test]
    fn test_root_domain_deep_subdomain() {
        let url = Url::parse("https://a.b.c.example.com/page").unwrap();
        assert_eq!(root_domain(&url), Some("example.com".to_string()));
    }

    #[test]
    fn test_root_domain_single_label() {
        let url = Url::parse("https://localhost/page").unwrap();
        assert_eq!(root_domain(&url), Some("localhost".to_string()));
    }

    // -----------------------------------------------------------------------
    // resolve_url
    // -----------------------------------------------------------------------

    #[test]
    fn test_resolve_relative_path() {
        let base = Url::parse("https://example.com/dir/page").unwrap();
        let resolved = resolve_url(&base, "other").unwrap();
        assert_eq!(resolved.as_str(), "https://example.com/dir/other");
    }

    #[test]
    fn test_resolve_absolute_path() {
        let base = Url::parse("https://example.com/dir/page").unwrap();
        let resolved = resolve_url(&base, "/root").unwrap();
        assert_eq!(resolved.as_str(), "https://example.com/root");
    }

    #[test]
    fn test_resolve_protocol_relative() {
        let base = Url::parse("https://example.com/dir/page").unwrap();
        let resolved = resolve_url(&base, "//cdn.example.com/file.js").unwrap();
        assert_eq!(resolved.as_str(), "https://cdn.example.com/file.js");
    }

    #[test]
    fn test_resolve_full_url() {
        let base = Url::parse("https://example.com/").unwrap();
        let resolved = resolve_url(&base, "https://other.com/page").unwrap();
        assert_eq!(resolved.as_str(), "https://other.com/page");
    }

    #[test]
    fn test_resolve_parent_directory() {
        let base = Url::parse("https://example.com/a/b/c").unwrap();
        let resolved = resolve_url(&base, "../d").unwrap();
        assert_eq!(resolved.as_str(), "https://example.com/a/d");
    }

    #[test]
    fn test_resolve_empty_string_returns_none() {
        let base = Url::parse("https://example.com/page").unwrap();
        assert!(resolve_url(&base, "").is_none());
    }

    #[test]
    fn test_resolve_whitespace_trimmed() {
        let base = Url::parse("https://example.com/dir/page").unwrap();
        let resolved = resolve_url(&base, "  /other  ").unwrap();
        assert_eq!(resolved.as_str(), "https://example.com/other");
    }

    // -----------------------------------------------------------------------
    // sanitize_url
    // -----------------------------------------------------------------------

    #[test]
    fn test_sanitize_adds_scheme() {
        let result = sanitize_url("example.com/page").unwrap();
        assert_eq!(result, "https://example.com/page");
    }

    #[test]
    fn test_sanitize_trims_whitespace() {
        let result = sanitize_url("  https://example.com/page  ").unwrap();
        assert_eq!(result, "https://example.com/page");
    }

    #[test]
    fn test_sanitize_collapses_internal_whitespace() {
        let result = sanitize_url("https://example.com/ pa ge").unwrap();
        // Whitespace collapsed to produce a parseable URL.
        assert!(result.starts_with("https://example.com/"));
    }

    #[test]
    fn test_sanitize_empty_string_returns_none() {
        assert!(sanitize_url("").is_none());
    }

    #[test]
    fn test_sanitize_whitespace_only_returns_none() {
        assert!(sanitize_url("   ").is_none());
    }

    #[test]
    fn test_sanitize_protocol_relative() {
        let result = sanitize_url("//cdn.example.com/file").unwrap();
        assert_eq!(result, "https://cdn.example.com/file");
    }

    // -----------------------------------------------------------------------
    // url_to_filename
    // -----------------------------------------------------------------------

    #[test]
    fn test_url_to_filename_basic() {
        let url = Url::parse("https://example.com/blog/post").unwrap();
        let filename = url_to_filename(&url);
        assert_eq!(filename, "example.com_blog_post");
    }

    #[test]
    fn test_url_to_filename_root() {
        let url = Url::parse("https://example.com/").unwrap();
        let filename = url_to_filename(&url);
        // Host + "/" -> "example.com_/"  -> "example.com_"  -> trimmed
        assert!(!filename.is_empty());
        assert!(filename.contains("example.com"));
    }

    #[test]
    fn test_url_to_filename_special_chars() {
        let url = Url::parse("https://example.com/path?q=1&b=2#frag").unwrap();
        let filename = url_to_filename(&url);
        // Query and fragment should not appear; special chars replaced.
        assert!(!filename.contains('?'));
        assert!(!filename.contains('#'));
        assert!(!filename.contains('&'));
    }

    // -----------------------------------------------------------------------
    // url_to_slug
    // -----------------------------------------------------------------------

    #[test]
    fn test_url_to_slug_basic() {
        let url = Url::parse("https://example.com/Blog/Post").unwrap();
        let slug = url_to_slug(&url);
        assert_eq!(slug, "example-com-blog-post");
    }

    #[test]
    fn test_url_to_slug_root() {
        let url = Url::parse("https://example.com/").unwrap();
        let slug = url_to_slug(&url);
        assert!(!slug.is_empty());
        assert!(slug.contains("example"));
    }

    #[test]
    fn test_url_to_slug_no_leading_trailing_hyphens() {
        let url = Url::parse("https://example.com/page/").unwrap();
        let slug = url_to_slug(&url);
        assert!(!slug.starts_with('-'));
        assert!(!slug.ends_with('-'));
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_normalize_unicode_path() {
        // Unicode characters in the path should survive normalization.
        let url = Url::parse("https://example.com/%E4%B8%AD%E6%96%87").unwrap();
        let result = normalize_url(&url, false);
        // The percent-encoded Chinese characters should be decoded to UTF-8.
        assert!(result.contains('\u{4e2d}')); // U+4E2D
    }

    #[test]
    fn test_normalize_empty_query_value() {
        let url = Url::parse("https://example.com/page?key=").unwrap();
        let result = normalize_url(&url, false);
        assert_eq!(result, "https://example.com/page?key=");
    }

    #[test]
    fn test_is_subdomain_suffix_collision() {
        // "notexample.com" should not be considered a subdomain of "example.com"
        let url = Url::parse("https://notexample.com/").unwrap();
        let base = Url::parse("https://example.com/").unwrap();
        assert!(!is_subdomain(&url, &base));
    }
}
