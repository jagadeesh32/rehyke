use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// ScanMode
// ---------------------------------------------------------------------------

/// High-level crawl presets that set sensible defaults for depth, page count,
/// and concurrency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScanMode {
    /// Quick surface-level crawl. Low depth, small page limit.
    Lite,
    /// Balanced crawl for most use-cases (default).
    Full,
    /// Exhaustive deep crawl.
    Deep,
}

impl ScanMode {
    /// Default maximum depth for this mode.
    pub fn default_max_depth(self) -> usize {
        match self {
            ScanMode::Lite => 2,
            ScanMode::Full => 5,
            ScanMode::Deep => 50,
        }
    }

    /// Default maximum number of pages for this mode.
    pub fn default_max_pages(self) -> usize {
        match self {
            ScanMode::Lite => 100,
            ScanMode::Full => 1_000,
            ScanMode::Deep => 50_000,
        }
    }

    /// Default concurrency (number of simultaneous requests) for this mode.
    pub fn default_concurrency(self) -> usize {
        match self {
            ScanMode::Lite => 5,
            ScanMode::Full => 10,
            ScanMode::Deep => 25,
        }
    }
}

impl Default for ScanMode {
    fn default() -> Self {
        ScanMode::Full
    }
}

// ---------------------------------------------------------------------------
// OutputMode
// ---------------------------------------------------------------------------

/// Where and how crawl results are stored.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputMode {
    /// Keep everything in memory and return it at the end.
    Memory,
    /// Write individual files to a directory.
    Files {
        output_dir: PathBuf,
        structure: FileStructure,
    },
    /// Write all output into a single file.
    SingleFile { output_path: PathBuf },
}

impl Default for OutputMode {
    fn default() -> Self {
        OutputMode::Memory
    }
}

// ---------------------------------------------------------------------------
// FileStructure
// ---------------------------------------------------------------------------

/// How files are laid out on disk when using `OutputMode::Files`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileStructure {
    /// All files in a single directory with sanitized names.
    Flat,
    /// Mirror the site's URL path hierarchy.
    Mirror,
}

impl Default for FileStructure {
    fn default() -> Self {
        FileStructure::Flat
    }
}

// ---------------------------------------------------------------------------
// WaitStrategy (for JavaScript-rendered pages)
// ---------------------------------------------------------------------------

/// Strategy used to decide when a JS-rendered page is "ready".
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WaitStrategy {
    /// Wait until there are no pending network requests.
    NetworkIdle,
    /// Wait until a specific CSS selector is present in the DOM.
    Selector { selector: String },
    /// Wait a fixed duration after page load.
    Duration {
        #[serde(with = "duration_serde")]
        duration: std::time::Duration,
    },
    /// Automatically determine the best strategy.
    Auto,
}

impl Default for WaitStrategy {
    fn default() -> Self {
        WaitStrategy::Auto
    }
}

// ---------------------------------------------------------------------------
// DelayStrategy (inter-request delays / politeness)
// ---------------------------------------------------------------------------

/// How to introduce delays between requests to the same domain.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DelayStrategy {
    /// Constant delay between requests.
    Fixed {
        #[serde(with = "duration_serde")]
        delay: Duration,
    },
    /// Random delay within [min, max].
    Random {
        #[serde(with = "duration_serde")]
        min: Duration,
        #[serde(with = "duration_serde")]
        max: Duration,
    },
    /// Adaptive delay that backs off when rate-limited.
    Adaptive {
        #[serde(with = "duration_serde")]
        initial: Duration,
    },
    /// No delay.
    None,
}

impl Default for DelayStrategy {
    fn default() -> Self {
        DelayStrategy::None
    }
}

// ---------------------------------------------------------------------------
// Proxy types
// ---------------------------------------------------------------------------

/// Protocol used by a proxy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProxyType {
    Http,
    Https,
    Socks5,
}

impl Default for ProxyType {
    fn default() -> Self {
        ProxyType::Http
    }
}

/// Authentication credentials for a proxy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyAuth {
    pub username: String,
    pub password: String,
}

/// Configuration for a single proxy endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    /// The proxy URL (e.g. `http://proxy.example.com:8080`).
    pub url: String,
    /// Protocol type of the proxy.
    #[serde(default)]
    pub proxy_type: ProxyType,
    /// Optional authentication.
    pub auth: Option<ProxyAuth>,
    /// Optional region label (for geo-routing proxies).
    pub region: Option<String>,
}

/// Strategy for choosing among multiple proxies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyStrategy {
    /// Cycle through proxies in order.
    RoundRobin,
    /// Pick a random proxy for each request.
    Random,
    /// Use the proxy with the fewest in-flight requests.
    LeastUsed,
    /// Only switch to another proxy when the current one fails.
    FailoverOnly,
}

impl Default for ProxyStrategy {
    fn default() -> Self {
        ProxyStrategy::RoundRobin
    }
}

// ---------------------------------------------------------------------------
// RetryConfig
// ---------------------------------------------------------------------------

/// Controls automatic retry behaviour for failed requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts.
    pub max_retries: u32,
    /// Delay before the first retry (doubles on each subsequent retry).
    #[serde(with = "duration_serde")]
    pub initial_delay: Duration,
    /// Upper bound on retry delay.
    #[serde(with = "duration_serde")]
    pub max_delay: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
        }
    }
}

// ---------------------------------------------------------------------------
// CrawlConfig
// ---------------------------------------------------------------------------

/// Complete configuration for a crawl job.
///
/// Use [`CrawlConfig::default()`] for sensible defaults or
/// [`CrawlConfigBuilder`] for a fluent builder API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlConfig {
    // -- scope --
    /// High-level scan preset.
    #[serde(default)]
    pub mode: ScanMode,
    /// Maximum link-follow depth from the seed URL.
    pub max_depth: usize,
    /// Maximum number of pages to crawl.
    pub max_pages: usize,

    // -- performance --
    /// Number of concurrent requests.
    pub concurrency: usize,

    // -- JavaScript rendering --
    /// Whether to run pages through a headless browser.
    #[serde(default)]
    pub enable_js: bool,
    /// How to wait for JS-rendered content to settle.
    #[serde(default)]
    pub js_wait_strategy: WaitStrategy,

    // -- output --
    /// Where to store crawl results.
    #[serde(default)]
    pub output: OutputMode,

    // -- network identity --
    /// User-Agent header sent with every request.
    pub user_agent: String,

    // -- proxy --
    /// List of proxy endpoints.
    #[serde(default)]
    pub proxies: Vec<ProxyConfig>,
    /// Strategy for selecting among proxies.
    #[serde(default)]
    pub proxy_strategy: ProxyStrategy,

    // -- politeness --
    /// Inter-request delay strategy.
    #[serde(default)]
    pub delay_strategy: DelayStrategy,

    // -- URL filtering --
    /// Regex patterns; URLs matching any pattern are skipped.
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
    /// Regex patterns; only URLs matching at least one pattern are crawled.
    #[serde(default)]
    pub include_patterns: Vec<String>,

    // -- robots / legality --
    /// Whether to honour robots.txt directives.
    #[serde(default = "default_true")]
    pub respect_robots_txt: bool,

    // -- content extraction --
    /// Extract structured metadata (title, description, etc.).
    #[serde(default = "default_true")]
    pub extract_metadata: bool,
    /// Remove navigation elements from extracted content.
    #[serde(default = "default_true")]
    pub clean_navigation: bool,
    /// Remove footer elements from extracted content.
    #[serde(default = "default_true")]
    pub clean_footers: bool,
    /// Remove advertisement elements from extracted content.
    #[serde(default = "default_true")]
    pub clean_ads: bool,

    // -- timeouts / retry --
    /// Per-request timeout.
    #[serde(with = "duration_serde")]
    pub timeout: Duration,
    /// Retry configuration.
    #[serde(default)]
    pub retry_config: RetryConfig,

    // -- custom request tweaks --
    /// Extra HTTP headers sent with every request.
    #[serde(default)]
    pub custom_headers: HashMap<String, String>,
    /// Cookies injected into every request.
    #[serde(default)]
    pub cookies: HashMap<String, String>,

    // -- URL normalisation --
    /// Strip the `www.` prefix when comparing / deduplicating URLs.
    #[serde(default = "default_true")]
    pub remove_www: bool,
}

/// Helper for serde defaults that should be `true`.
fn default_true() -> bool {
    true
}

impl Default for CrawlConfig {
    fn default() -> Self {
        let mode = ScanMode::default(); // Full
        Self {
            mode,
            max_depth: mode.default_max_depth(),
            max_pages: mode.default_max_pages(),
            concurrency: mode.default_concurrency(),
            enable_js: false,
            js_wait_strategy: WaitStrategy::default(),
            output: OutputMode::default(),
            user_agent: format!("rehyke/{}", env!("CARGO_PKG_VERSION")),
            proxies: Vec::new(),
            proxy_strategy: ProxyStrategy::default(),
            delay_strategy: DelayStrategy::default(),
            exclude_patterns: Vec::new(),
            include_patterns: Vec::new(),
            respect_robots_txt: true,
            extract_metadata: true,
            clean_navigation: true,
            clean_footers: true,
            clean_ads: true,
            timeout: Duration::from_secs(30),
            retry_config: RetryConfig::default(),
            custom_headers: HashMap::new(),
            cookies: HashMap::new(),
            remove_www: true,
        }
    }
}

// ---------------------------------------------------------------------------
// CrawlConfigBuilder
// ---------------------------------------------------------------------------

/// Fluent builder for [`CrawlConfig`].
///
/// Start from defaults and override only what you need:
///
/// ```rust,no_run
/// use rehyke_core::config::CrawlConfigBuilder;
///
/// let config = CrawlConfigBuilder::new()
///     .max_depth(3)
///     .concurrency(20)
///     .enable_js(true)
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct CrawlConfigBuilder {
    inner: CrawlConfig,
}

impl CrawlConfigBuilder {
    /// Create a builder pre-filled with [`CrawlConfig::default()`].
    pub fn new() -> Self {
        Self {
            inner: CrawlConfig::default(),
        }
    }

    /// Set the scan mode **and** update depth/pages/concurrency to the mode's
    /// defaults.  Any subsequent calls to [`max_depth`](Self::max_depth),
    /// [`max_pages`](Self::max_pages), or [`concurrency`](Self::concurrency)
    /// will override those values.
    pub fn mode(mut self, mode: ScanMode) -> Self {
        self.inner.mode = mode;
        self.inner.max_depth = mode.default_max_depth();
        self.inner.max_pages = mode.default_max_pages();
        self.inner.concurrency = mode.default_concurrency();
        self
    }

    pub fn max_depth(mut self, max_depth: usize) -> Self {
        self.inner.max_depth = max_depth;
        self
    }

    pub fn max_pages(mut self, max_pages: usize) -> Self {
        self.inner.max_pages = max_pages;
        self
    }

    pub fn concurrency(mut self, concurrency: usize) -> Self {
        self.inner.concurrency = concurrency;
        self
    }

    pub fn enable_js(mut self, enable: bool) -> Self {
        self.inner.enable_js = enable;
        self
    }

    pub fn js_wait_strategy(mut self, strategy: WaitStrategy) -> Self {
        self.inner.js_wait_strategy = strategy;
        self
    }

    pub fn output(mut self, output: OutputMode) -> Self {
        self.inner.output = output;
        self
    }

    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.inner.user_agent = user_agent.into();
        self
    }

    pub fn proxies(mut self, proxies: Vec<ProxyConfig>) -> Self {
        self.inner.proxies = proxies;
        self
    }

    pub fn proxy_strategy(mut self, strategy: ProxyStrategy) -> Self {
        self.inner.proxy_strategy = strategy;
        self
    }

    pub fn delay_strategy(mut self, strategy: DelayStrategy) -> Self {
        self.inner.delay_strategy = strategy;
        self
    }

    pub fn exclude_patterns(mut self, patterns: Vec<String>) -> Self {
        self.inner.exclude_patterns = patterns;
        self
    }

    pub fn include_patterns(mut self, patterns: Vec<String>) -> Self {
        self.inner.include_patterns = patterns;
        self
    }

    pub fn respect_robots_txt(mut self, respect: bool) -> Self {
        self.inner.respect_robots_txt = respect;
        self
    }

    pub fn extract_metadata(mut self, extract: bool) -> Self {
        self.inner.extract_metadata = extract;
        self
    }

    pub fn clean_navigation(mut self, clean: bool) -> Self {
        self.inner.clean_navigation = clean;
        self
    }

    pub fn clean_footers(mut self, clean: bool) -> Self {
        self.inner.clean_footers = clean;
        self
    }

    pub fn clean_ads(mut self, clean: bool) -> Self {
        self.inner.clean_ads = clean;
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.inner.timeout = timeout;
        self
    }

    pub fn retry_config(mut self, retry_config: RetryConfig) -> Self {
        self.inner.retry_config = retry_config;
        self
    }

    pub fn custom_headers(mut self, headers: HashMap<String, String>) -> Self {
        self.inner.custom_headers = headers;
        self
    }

    /// Insert a single custom header.
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.inner.custom_headers.insert(key.into(), value.into());
        self
    }

    pub fn cookies(mut self, cookies: HashMap<String, String>) -> Self {
        self.inner.cookies = cookies;
        self
    }

    /// Insert a single cookie.
    pub fn cookie(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.inner.cookies.insert(name.into(), value.into());
        self
    }

    pub fn remove_www(mut self, remove: bool) -> Self {
        self.inner.remove_www = remove;
        self
    }

    /// Consume the builder and return the final [`CrawlConfig`].
    pub fn build(self) -> CrawlConfig {
        self.inner
    }
}

impl Default for CrawlConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Serde helper for Duration fields
// ---------------------------------------------------------------------------

/// Serde adapter that serialises `std::time::Duration` as an integer number of
/// milliseconds and deserialises from the same representation, or from a
/// human-readable string such as `"30s"`, `"500ms"`, `"2m"`.
mod duration_serde {
    use serde::{self, Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_millis() as u64)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::Number(n) => {
                let ms = n
                    .as_u64()
                    .ok_or_else(|| serde::de::Error::custom("expected unsigned integer for ms"))?;
                Ok(Duration::from_millis(ms))
            }
            serde_json::Value::String(s) => parse_duration(&s).map_err(serde::de::Error::custom),
            _ => Err(serde::de::Error::custom(
                "expected a duration in milliseconds (integer) or a string like \"30s\"",
            )),
        }
    }

    /// Parse a human-readable duration string such as `"30s"`, `"500ms"`,
    /// `"1m30s"`, `"2h"`, or a bare number (interpreted as milliseconds).
    fn parse_duration(s: &str) -> Result<Duration, String> {
        let s = s.trim();
        if s.is_empty() {
            return Err("empty duration string".to_string());
        }

        // Bare number -> milliseconds.
        if let Ok(ms) = s.parse::<u64>() {
            return Ok(Duration::from_millis(ms));
        }

        let mut total = Duration::ZERO;
        let mut num_buf = String::new();
        let mut unit_buf = String::new();
        let mut reading_unit = false;

        for ch in s.chars() {
            if ch.is_ascii_digit() {
                if reading_unit {
                    // Finished reading a unit -- apply the last number+unit pair.
                    let num = num_buf
                        .parse::<u64>()
                        .map_err(|_| format!("invalid number: {}", num_buf))?;
                    total += apply_unit(num, &unit_buf)?;
                    num_buf.clear();
                    unit_buf.clear();
                    reading_unit = false;
                }
                num_buf.push(ch);
            } else if ch.is_ascii_alphabetic() {
                reading_unit = true;
                unit_buf.push(ch);
            }
            // Whitespace and other characters are ignored.
        }

        // Handle trailing number+unit or bare trailing number.
        if !num_buf.is_empty() && !unit_buf.is_empty() {
            let num = num_buf
                .parse::<u64>()
                .map_err(|_| format!("invalid number: {}", num_buf))?;
            total += apply_unit(num, &unit_buf)?;
        } else if !num_buf.is_empty() {
            // Trailing bare number with no unit -- treat as ms.
            let num = num_buf
                .parse::<u64>()
                .map_err(|_| format!("invalid number: {}", num_buf))?;
            total += Duration::from_millis(num);
        } else if !unit_buf.is_empty() {
            return Err(format!("trailing unit without a number: {}", unit_buf));
        }

        if total == Duration::ZERO {
            return Err(format!("could not parse duration: {}", s));
        }

        Ok(total)
    }

    fn apply_unit(num: u64, unit: &str) -> Result<Duration, String> {
        match unit {
            "ms" => Ok(Duration::from_millis(num)),
            "s" | "sec" | "secs" => Ok(Duration::from_secs(num)),
            "m" | "min" | "mins" => Ok(Duration::from_secs(num * 60)),
            "h" | "hr" | "hrs" => Ok(Duration::from_secs(num * 3600)),
            _ => Err(format!("unknown duration unit: {}", unit)),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn parse_bare_number() {
            assert_eq!(parse_duration("500").unwrap(), Duration::from_millis(500));
        }

        #[test]
        fn parse_ms() {
            assert_eq!(
                parse_duration("250ms").unwrap(),
                Duration::from_millis(250)
            );
        }

        #[test]
        fn parse_seconds() {
            assert_eq!(parse_duration("30s").unwrap(), Duration::from_secs(30));
        }

        #[test]
        fn parse_compound() {
            assert_eq!(parse_duration("1m30s").unwrap(), Duration::from_secs(90));
        }

        #[test]
        fn parse_hours() {
            assert_eq!(parse_duration("2h").unwrap(), Duration::from_secs(7200));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_uses_full_mode() {
        let cfg = CrawlConfig::default();
        assert_eq!(cfg.mode, ScanMode::Full);
        assert_eq!(cfg.max_depth, 5);
        assert_eq!(cfg.max_pages, 1_000);
        assert_eq!(cfg.concurrency, 10);
    }

    #[test]
    fn builder_overrides() {
        let cfg = CrawlConfigBuilder::new()
            .mode(ScanMode::Lite)
            .max_depth(1)
            .concurrency(2)
            .enable_js(true)
            .header("X-Custom", "value")
            .cookie("session", "abc123")
            .remove_www(false)
            .build();

        assert_eq!(cfg.mode, ScanMode::Lite);
        assert_eq!(cfg.max_depth, 1);
        assert_eq!(cfg.concurrency, 2);
        assert!(cfg.enable_js);
        assert_eq!(cfg.custom_headers.get("X-Custom").unwrap(), "value");
        assert_eq!(cfg.cookies.get("session").unwrap(), "abc123");
        assert!(!cfg.remove_www);
    }

    #[test]
    fn builder_mode_sets_defaults() {
        let cfg = CrawlConfigBuilder::new().mode(ScanMode::Deep).build();
        assert_eq!(cfg.max_depth, 50);
        assert_eq!(cfg.max_pages, 50_000);
        assert_eq!(cfg.concurrency, 25);
    }

    #[test]
    fn config_roundtrip_json() {
        let cfg = CrawlConfig::default();
        let json = serde_json::to_string_pretty(&cfg).expect("serialize");
        let cfg2: CrawlConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(cfg2.mode, cfg.mode);
        assert_eq!(cfg2.max_depth, cfg.max_depth);
        assert_eq!(cfg2.timeout, cfg.timeout);
    }

    #[test]
    fn scan_mode_defaults() {
        assert_eq!(ScanMode::Lite.default_max_depth(), 2);
        assert_eq!(ScanMode::Lite.default_max_pages(), 100);
        assert_eq!(ScanMode::Lite.default_concurrency(), 5);

        assert_eq!(ScanMode::Full.default_max_depth(), 5);
        assert_eq!(ScanMode::Full.default_max_pages(), 1_000);
        assert_eq!(ScanMode::Full.default_concurrency(), 10);

        assert_eq!(ScanMode::Deep.default_max_depth(), 50);
        assert_eq!(ScanMode::Deep.default_max_pages(), 50_000);
        assert_eq!(ScanMode::Deep.default_concurrency(), 25);
    }

    #[test]
    fn default_boolean_flags() {
        let cfg = CrawlConfig::default();
        assert!(cfg.respect_robots_txt);
        assert!(cfg.extract_metadata);
        assert!(cfg.clean_navigation);
        assert!(cfg.clean_footers);
        assert!(cfg.clean_ads);
        assert!(cfg.remove_www);
        assert!(!cfg.enable_js);
    }

    #[test]
    fn output_mode_files_serialization() {
        let output = OutputMode::Files {
            output_dir: PathBuf::from("/tmp/crawl"),
            structure: FileStructure::Mirror,
        };
        let json = serde_json::to_string(&output).expect("serialize");
        assert!(json.contains("\"type\":\"files\""));
        assert!(json.contains("\"mirror\""));
    }
}
