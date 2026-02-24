use tracing::debug;
use url::Url;

/// Parsed robots.txt rules for one or more user agents.
#[derive(Debug, Clone, Default)]
pub struct RobotsTxt {
    rules: Vec<RobotsRule>,
    sitemaps: Vec<String>,
    crawl_delay: Option<f64>,
}

/// A single user-agent section in a robots.txt file.
#[derive(Debug, Clone)]
struct RobotsRule {
    user_agent: String,
    allow: Vec<String>,
    disallow: Vec<String>,
}

impl RobotsTxt {
    /// Parse the text content of a robots.txt file into structured rules.
    ///
    /// Recognised directives (case-insensitive):
    /// - `User-agent`
    /// - `Allow`
    /// - `Disallow`
    /// - `Sitemap`
    /// - `Crawl-delay`
    pub fn parse(content: &str) -> Self {
        let mut rules: Vec<RobotsRule> = Vec::new();
        let mut sitemaps: Vec<String> = Vec::new();
        let mut crawl_delay: Option<f64> = None;

        // Accumulate user-agent names until we see a directive that is not
        // User-agent; at that point we create one RobotsRule per collected
        // user-agent name and start filling their allow/disallow lists.
        let mut current_agents: Vec<String> = Vec::new();
        let mut current_allow: Vec<String> = Vec::new();
        let mut current_disallow: Vec<String> = Vec::new();

        let flush = |agents: &mut Vec<String>,
                     allow: &mut Vec<String>,
                     disallow: &mut Vec<String>,
                     rules: &mut Vec<RobotsRule>| {
            if !agents.is_empty() {
                for agent in agents.drain(..) {
                    rules.push(RobotsRule {
                        user_agent: agent,
                        allow: allow.clone(),
                        disallow: disallow.clone(),
                    });
                }
            }
            allow.clear();
            disallow.clear();
        };

        for line in content.lines() {
            let line = line.trim();

            // Skip empty lines and comments.
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Split on the first ':'.
            let (directive, value) = match line.split_once(':') {
                Some((d, v)) => (d.trim().to_ascii_lowercase(), v.trim().to_string()),
                None => continue,
            };

            // Strip inline comments from the value.
            let value = match value.split_once('#') {
                Some((v, _)) => v.trim().to_string(),
                None => value,
            };

            match directive.as_str() {
                "user-agent" => {
                    // If we already collected non-agent directives, flush
                    // the current group first.
                    if !current_allow.is_empty() || !current_disallow.is_empty() {
                        flush(
                            &mut current_agents,
                            &mut current_allow,
                            &mut current_disallow,
                            &mut rules,
                        );
                    }
                    current_agents.push(value.to_lowercase());
                }
                "allow" => {
                    if !value.is_empty() {
                        current_allow.push(value);
                    }
                }
                "disallow" => {
                    if !value.is_empty() {
                        current_disallow.push(value);
                    }
                }
                "sitemap" => {
                    if !value.is_empty() {
                        sitemaps.push(value);
                    }
                }
                "crawl-delay" => {
                    if let Ok(delay) = value.parse::<f64>() {
                        crawl_delay = Some(delay);
                    }
                }
                _ => {
                    // Unknown directive -- skip.
                }
            }
        }

        // Flush any remaining group.
        flush(
            &mut current_agents,
            &mut current_allow,
            &mut current_disallow,
            &mut rules,
        );

        debug!(
            rule_count = rules.len(),
            sitemap_count = sitemaps.len(),
            crawl_delay = ?crawl_delay,
            "parsed robots.txt"
        );

        Self {
            rules,
            sitemaps,
            crawl_delay,
        }
    }

    /// Check whether the given URL path is allowed for our crawler.
    ///
    /// Matching logic:
    /// 1. First look for rules targeting "rehyke" (case-insensitive).
    /// 2. If none exist, fall back to the wildcard "*" rules.
    /// 3. If no matching rules exist at all, allow by default.
    /// 4. Among matched rules, the most specific (longest) pattern wins.
    ///    If both an allow and a disallow match with the same specificity,
    ///    the allow takes precedence.
    pub fn is_allowed(&self, path: &str) -> bool {
        // Gather applicable rules: prefer "rehyke", then "*".
        let rehyke_rules: Vec<&RobotsRule> = self
            .rules
            .iter()
            .filter(|r| r.user_agent == "rehyke")
            .collect();

        let applicable = if !rehyke_rules.is_empty() {
            rehyke_rules
        } else {
            self.rules
                .iter()
                .filter(|r| r.user_agent == "*")
                .collect()
        };

        if applicable.is_empty() {
            return true;
        }

        // Find the longest matching allow and disallow pattern.
        let mut best_allow: Option<usize> = None;
        let mut best_disallow: Option<usize> = None;

        for rule in &applicable {
            for pattern in &rule.allow {
                if path_matches(path, pattern) {
                    let len = pattern.len();
                    if best_allow.map_or(true, |prev| len > prev) {
                        best_allow = Some(len);
                    }
                }
            }
            for pattern in &rule.disallow {
                if path_matches(path, pattern) {
                    let len = pattern.len();
                    if best_disallow.map_or(true, |prev| len > prev) {
                        best_disallow = Some(len);
                    }
                }
            }
        }

        match (best_allow, best_disallow) {
            (Some(a), Some(d)) => a >= d,
            (Some(_), None) => true,
            (None, Some(_)) => false,
            (None, None) => true,
        }
    }

    /// Return the sitemap URLs listed in the robots.txt file.
    pub fn sitemaps(&self) -> &[String] {
        &self.sitemaps
    }

    /// Return the crawl delay (in seconds) if one was specified.
    pub fn crawl_delay(&self) -> Option<f64> {
        self.crawl_delay
    }

    /// Build the canonical robots.txt URL for the given base URL.
    pub fn robots_url(base: &Url) -> String {
        format!(
            "{}://{}/robots.txt",
            base.scheme(),
            base.host_str().unwrap_or("localhost")
        )
    }
}

/// Check whether `path` matches a robots.txt pattern.
///
/// Supported pattern syntax:
/// - `*` matches any sequence of characters (including empty).
/// - `$` at the end of the pattern anchors the match to the end of the path.
/// - A simple prefix match is used when no wildcards are present.
fn path_matches(path: &str, pattern: &str) -> bool {
    let anchored = pattern.ends_with('$');
    let pattern = if anchored {
        &pattern[..pattern.len() - 1]
    } else {
        pattern
    };

    if !pattern.contains('*') {
        // Simple prefix match.
        if anchored {
            path == pattern
        } else {
            path.starts_with(pattern)
        }
    } else {
        // Wildcard matching: split pattern on '*' and match segments in order.
        let segments: Vec<&str> = pattern.split('*').collect();
        let mut pos = 0usize;

        // The first segment must match at the start.
        if !segments.is_empty() {
            let first = segments[0];
            if !path[pos..].starts_with(first) {
                return false;
            }
            pos += first.len();
        }

        // Middle segments can match anywhere after the current position.
        for seg in &segments[1..segments.len().saturating_sub(1).max(1)] {
            if seg.is_empty() {
                continue;
            }
            match path[pos..].find(seg) {
                Some(idx) => pos = pos + idx + seg.len(),
                None => return false,
            }
        }

        // The last segment (if different from the first) must be found after pos.
        if segments.len() > 1 {
            let last = segments[segments.len() - 1];
            if !last.is_empty() {
                if anchored {
                    if !path.ends_with(last) {
                        return false;
                    }
                    let last_start = path.len() - last.len();
                    if last_start < pos {
                        return false;
                    }
                    pos = path.len();
                } else {
                    match path[pos..].find(last) {
                        Some(idx) => pos = pos + idx + last.len(),
                        None => return false,
                    }
                }
            }
        }

        // If anchored, we must have consumed the whole path.
        if anchored && !pattern.ends_with('*') && pos != path.len() {
            return false;
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Basic allow / disallow parsing
    // -----------------------------------------------------------------------

    #[test]
    fn test_basic_disallow() {
        let content = "\
User-agent: *
Disallow: /admin
Disallow: /private/
";
        let robots = RobotsTxt::parse(content);
        assert!(!robots.is_allowed("/admin"));
        assert!(!robots.is_allowed("/admin/settings"));
        assert!(!robots.is_allowed("/private/"));
        assert!(!robots.is_allowed("/private/data"));
        assert!(robots.is_allowed("/public"));
        assert!(robots.is_allowed("/"));
    }

    #[test]
    fn test_basic_allow_overrides_disallow() {
        let content = "\
User-agent: *
Disallow: /dir/
Allow: /dir/page
";
        let robots = RobotsTxt::parse(content);
        assert!(robots.is_allowed("/dir/page"));
        assert!(robots.is_allowed("/dir/page/sub"));
        assert!(!robots.is_allowed("/dir/other"));
    }

    #[test]
    fn test_allow_same_length_takes_precedence() {
        let content = "\
User-agent: *
Disallow: /x
Allow: /x
";
        let robots = RobotsTxt::parse(content);
        assert!(robots.is_allowed("/x"));
    }

    // -----------------------------------------------------------------------
    // Wildcard user-agent matching
    // -----------------------------------------------------------------------

    #[test]
    fn test_wildcard_user_agent() {
        let content = "\
User-agent: *
Disallow: /secret
";
        let robots = RobotsTxt::parse(content);
        assert!(!robots.is_allowed("/secret"));
        assert!(robots.is_allowed("/open"));
    }

    #[test]
    fn test_specific_user_agent_rehyke() {
        let content = "\
User-agent: *
Disallow: /all-bots

User-agent: Rehyke
Disallow: /rehyke-only
Allow: /all-bots
";
        let robots = RobotsTxt::parse(content);
        // The Rehyke-specific rules should apply, not the wildcard.
        assert!(robots.is_allowed("/all-bots"));
        assert!(!robots.is_allowed("/rehyke-only"));
    }

    #[test]
    fn test_case_insensitive_user_agent() {
        let content = "\
User-agent: REHYKE
Disallow: /blocked
";
        let robots = RobotsTxt::parse(content);
        assert!(!robots.is_allowed("/blocked"));
    }

    // -----------------------------------------------------------------------
    // Sitemap extraction
    // -----------------------------------------------------------------------

    #[test]
    fn test_sitemap_extraction() {
        let content = "\
User-agent: *
Disallow: /tmp

Sitemap: https://example.com/sitemap.xml
Sitemap: https://example.com/sitemap2.xml
";
        let robots = RobotsTxt::parse(content);
        assert_eq!(robots.sitemaps().len(), 2);
        assert_eq!(robots.sitemaps()[0], "https://example.com/sitemap.xml");
        assert_eq!(robots.sitemaps()[1], "https://example.com/sitemap2.xml");
    }

    #[test]
    fn test_no_sitemaps() {
        let content = "\
User-agent: *
Disallow:
";
        let robots = RobotsTxt::parse(content);
        assert!(robots.sitemaps().is_empty());
    }

    // -----------------------------------------------------------------------
    // Path matching with wildcards
    // -----------------------------------------------------------------------

    #[test]
    fn test_wildcard_in_path() {
        let content = "\
User-agent: *
Disallow: /dir/*/private
";
        let robots = RobotsTxt::parse(content);
        assert!(!robots.is_allowed("/dir/abc/private"));
        assert!(!robots.is_allowed("/dir/xyz/private/data"));
        assert!(robots.is_allowed("/dir/abc/public"));
    }

    #[test]
    fn test_dollar_anchor() {
        let content = "\
User-agent: *
Disallow: /*.pdf$
";
        let robots = RobotsTxt::parse(content);
        assert!(!robots.is_allowed("/docs/report.pdf"));
        assert!(robots.is_allowed("/docs/report.pdf.bak"));
        assert!(robots.is_allowed("/docs/report.html"));
    }

    #[test]
    fn test_wildcard_star_only() {
        // A pattern of just "*" should match everything.
        assert!(path_matches("/anything", "*"));
        assert!(path_matches("/", "*"));
    }

    #[test]
    fn test_path_matches_prefix() {
        assert!(path_matches("/admin/page", "/admin"));
        assert!(!path_matches("/public", "/admin"));
    }

    #[test]
    fn test_path_matches_exact_with_anchor() {
        assert!(path_matches("/page.html", "/page.html$"));
        assert!(!path_matches("/page.htmlx", "/page.html$"));
    }

    // -----------------------------------------------------------------------
    // Crawl-delay parsing
    // -----------------------------------------------------------------------

    #[test]
    fn test_crawl_delay_integer() {
        let content = "\
User-agent: *
Crawl-delay: 10
Disallow:
";
        let robots = RobotsTxt::parse(content);
        assert_eq!(robots.crawl_delay(), Some(10.0));
    }

    #[test]
    fn test_crawl_delay_float() {
        let content = "\
User-agent: *
Crawl-delay: 0.5
Disallow:
";
        let robots = RobotsTxt::parse(content);
        assert_eq!(robots.crawl_delay(), Some(0.5));
    }

    #[test]
    fn test_no_crawl_delay() {
        let content = "\
User-agent: *
Disallow: /x
";
        let robots = RobotsTxt::parse(content);
        assert_eq!(robots.crawl_delay(), None);
    }

    // -----------------------------------------------------------------------
    // Empty / malformed robots.txt
    // -----------------------------------------------------------------------

    #[test]
    fn test_empty_content() {
        let robots = RobotsTxt::parse("");
        assert!(robots.is_allowed("/anything"));
        assert!(robots.sitemaps().is_empty());
        assert_eq!(robots.crawl_delay(), None);
    }

    #[test]
    fn test_comments_only() {
        let content = "\
# This is a comment
# Another comment
";
        let robots = RobotsTxt::parse(content);
        assert!(robots.is_allowed("/anything"));
    }

    #[test]
    fn test_malformed_lines_ignored() {
        let content = "\
This is not a valid directive
User-agent: *
Disallow /missing-colon
Disallow: /valid
invalid line
";
        let robots = RobotsTxt::parse(content);
        assert!(!robots.is_allowed("/valid"));
        assert!(robots.is_allowed("/other"));
    }

    #[test]
    fn test_inline_comments_stripped() {
        let content = "\
User-agent: *
Disallow: /secret # this is secret
";
        let robots = RobotsTxt::parse(content);
        assert!(!robots.is_allowed("/secret"));
    }

    // -----------------------------------------------------------------------
    // robots_url
    // -----------------------------------------------------------------------

    #[test]
    fn test_robots_url_construction() {
        let base = Url::parse("https://example.com/some/page").unwrap();
        assert_eq!(
            RobotsTxt::robots_url(&base),
            "https://example.com/robots.txt"
        );
    }

    #[test]
    fn test_robots_url_http() {
        let base = Url::parse("http://example.com/").unwrap();
        assert_eq!(
            RobotsTxt::robots_url(&base),
            "http://example.com/robots.txt"
        );
    }

    // -----------------------------------------------------------------------
    // Multiple user-agent groups
    // -----------------------------------------------------------------------

    #[test]
    fn test_multiple_groups() {
        let content = "\
User-agent: Googlebot
Disallow: /google-only

User-agent: *
Disallow: /blocked
";
        let robots = RobotsTxt::parse(content);
        // We are not Googlebot so wildcard rules apply.
        assert!(!robots.is_allowed("/blocked"));
        assert!(robots.is_allowed("/google-only"));
    }

    #[test]
    fn test_disallow_everything() {
        let content = "\
User-agent: *
Disallow: /
";
        let robots = RobotsTxt::parse(content);
        assert!(!robots.is_allowed("/"));
        assert!(!robots.is_allowed("/anything"));
    }

    #[test]
    fn test_allow_everything() {
        let content = "\
User-agent: *
Disallow:
";
        let robots = RobotsTxt::parse(content);
        assert!(robots.is_allowed("/anything"));
        assert!(robots.is_allowed("/"));
    }
}
