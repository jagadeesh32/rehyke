use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Viewport (v0.2.0 — headless browser viewport profiles)
// ---------------------------------------------------------------------------

/// Viewport size profile used when rendering pages with a headless browser.
///
/// Pass to [`CrawlConfigBuilder::viewport`] to control which browser size
/// Chrome emulates when JS rendering is enabled.
///
/// # Examples
///
/// ```rust
/// use rehyke_core::{CrawlConfigBuilder, Viewport};
///
/// // Desktop (1920×1080, no touch)
/// let cfg = CrawlConfigBuilder::new().viewport(Viewport::Desktop).build();
/// assert_eq!(cfg.viewport.dimensions(), (1920, 1080));
/// assert!(!cfg.viewport.is_mobile());
///
/// // Mobile (390×844, 3× DPR, touch)
/// let cfg = CrawlConfigBuilder::new().viewport(Viewport::Mobile).build();
/// let (w, h) = cfg.viewport.dimensions();
/// let dpr = cfg.viewport.device_scale_factor();
/// println!("{}×{} CSS px → {}×{} physical px", w, h,
///          (w as f64 * dpr) as u32, (h as f64 * dpr) as u32);
/// assert!(cfg.viewport.has_touch());
///
/// // Tablet (768×1024, 2× DPR, touch)
/// let cfg = CrawlConfigBuilder::new().viewport(Viewport::Tablet).build();
/// assert!(cfg.viewport.is_mobile());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Viewport {
    /// 1920×1080 desktop — no touch, no mobile emulation.
    Desktop,
    /// 768×1024 tablet — touch enabled, 2× device pixel ratio.
    Tablet,
    /// 390×844 mobile — touch enabled, 3× device pixel ratio.
    Mobile,
}

impl Viewport {
    /// Physical pixel dimensions `(width, height)` for this profile.
    pub fn dimensions(self) -> (u32, u32) {
        match self {
            Viewport::Desktop => (1920, 1080),
            Viewport::Tablet => (768, 1024),
            Viewport::Mobile => (390, 844),
        }
    }

    /// Device scale factor (CSS pixel ratio) for this profile.
    pub fn device_scale_factor(self) -> f64 {
        match self {
            Viewport::Desktop => 1.0,
            Viewport::Tablet => 2.0,
            Viewport::Mobile => 3.0,
        }
    }

    /// Whether the profile emulates a mobile device.
    pub fn is_mobile(self) -> bool {
        matches!(self, Viewport::Tablet | Viewport::Mobile)
    }

    /// Whether the profile has touch support.
    pub fn has_touch(self) -> bool {
        matches!(self, Viewport::Tablet | Viewport::Mobile)
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Viewport::Desktop
    }
}

// ---------------------------------------------------------------------------
// ScreenshotFormat (v0.2.0)
// ---------------------------------------------------------------------------

/// Image format for browser screenshots.
///
/// Pass to [`CrawlConfigBuilder::screenshot_format`].
/// Defaults to `Png` when [`CrawlConfigBuilder::screenshot`] is enabled.
///
/// # Examples
///
/// ```rust
/// use rehyke_core::{CrawlConfigBuilder, ScreenshotFormat};
/// use std::path::PathBuf;
///
/// // PNG — lossless, larger files, best for visual diffing
/// let cfg = CrawlConfigBuilder::new()
///     .screenshot(true)
///     .screenshot_format(ScreenshotFormat::Png)
///     .screenshot_output_dir(PathBuf::from("/tmp/shots"))
///     .build();
/// assert_eq!(cfg.screenshot_format, ScreenshotFormat::Png);
///
/// // JPEG — lossy but much smaller, good for archiving many pages
/// let cfg = CrawlConfigBuilder::new()
///     .screenshot(true)
///     .screenshot_format(ScreenshotFormat::Jpeg)
///     .build();
/// assert_eq!(cfg.screenshot_format, ScreenshotFormat::Jpeg);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScreenshotFormat {
    /// Lossless PNG (larger files, perfect quality).
    Png,
    /// Lossy JPEG (smaller files, configurable quality).
    Jpeg,
}

impl Default for ScreenshotFormat {
    fn default() -> Self {
        ScreenshotFormat::Png
    }
}

// ---------------------------------------------------------------------------
// ScanMode
// ---------------------------------------------------------------------------

/// High-level crawl presets that set sensible defaults for depth, page count,
/// and concurrency.
///
/// | Mode | Max depth | Max pages | Concurrency | Scope |
/// |------|-----------|-----------|-------------|-------|
/// | `Lite` | 2 | 100 | 5 | Single page + immediate links |
/// | `Full` | 5 | 1 000 | 10 | Entire domain (default) |
/// | `Deep` | 50 | 50 000 | 25 | Cross-domain exhaustive |
///
/// # Examples
///
/// ```rust
/// use rehyke_core::{CrawlConfigBuilder, ScanMode};
///
/// // Lite: single-page extraction, no link following
/// let cfg = CrawlConfigBuilder::new().mode(ScanMode::Lite).build();
/// assert_eq!(cfg.max_depth, 2);
/// assert_eq!(cfg.max_pages, 100);
///
/// // Full: balanced domain-wide crawl (default)
/// let cfg = CrawlConfigBuilder::new().mode(ScanMode::Full).build();
/// assert_eq!(cfg.max_pages, 1_000);
///
/// // Deep: cross-domain exhaustive crawl
/// let cfg = CrawlConfigBuilder::new().mode(ScanMode::Deep).build();
/// assert_eq!(cfg.max_depth, 50);
/// assert_eq!(cfg.concurrency, 25);
/// ```
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
///
/// Passed to [`CrawlConfigBuilder::js_wait_strategy`].
///
/// | Variant | Best for |
/// |---------|----------|
/// | `NetworkIdle` | React, Vue, Angular SPAs that make XHR/fetch calls on load |
/// | `Selector` | Pages with a known "ready" element (e.g. `#app`, `.content-loaded`) |
/// | `Duration` | Angular apps that need a fixed settle period after network idle |
/// | `Auto` | Unknown pages — Rehyke picks a sensible heuristic |
///
/// # Examples
///
/// ```rust
/// use rehyke_core::{CrawlConfigBuilder, WaitStrategy};
/// use std::time::Duration;
///
/// // Wait for network to go idle (best for most SPAs)
/// let cfg = CrawlConfigBuilder::new()
///     .enable_js(true)
///     .js_wait_strategy(WaitStrategy::NetworkIdle)
///     .build();
///
/// // Wait for a specific element — useful for Vue/Nuxt apps
/// let cfg = CrawlConfigBuilder::new()
///     .enable_js(true)
///     .js_wait_strategy(WaitStrategy::Selector {
///         selector: "#app, [data-v-app]".into(),
///     })
///     .build();
///
/// // Fixed 1.5-second settle time — useful for Angular change-detection
/// let cfg = CrawlConfigBuilder::new()
///     .enable_js(true)
///     .js_wait_strategy(WaitStrategy::Duration {
///         duration: Duration::from_millis(1500),
///     })
///     .build();
///
/// // Auto — let Rehyke decide
/// let cfg = CrawlConfigBuilder::new()
///     .enable_js(true)
///     .js_wait_strategy(WaitStrategy::Auto)
///     .build();
/// ```
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

    // -----------------------------------------------------------------------
    // v0.2.0 — Headless browser / JS rendering settings
    // -----------------------------------------------------------------------

    /// Viewport size profile sent to the headless browser.
    #[serde(default)]
    pub viewport: Viewport,

    /// How many scroll operations to perform when detecting infinite-scroll
    /// pages.  `0` disables auto-scrolling.
    #[serde(default)]
    pub js_scroll_count: usize,

    /// Automatically detect and dismiss common popups (cookie consent banners,
    /// GDPR modals, newsletter overlays) before extracting content.
    #[serde(default)]
    pub dismiss_popups: bool,

    /// Capture a full-page screenshot after JavaScript has settled.
    /// Screenshots are only taken when `enable_js` is also `true`.
    #[serde(default)]
    pub screenshot: bool,

    /// Image format for captured screenshots.
    #[serde(default)]
    pub screenshot_format: ScreenshotFormat,

    /// Directory where screenshot files are written.
    /// Defaults to the current working directory when `None`.
    pub screenshot_output_dir: Option<PathBuf>,

    /// Maximum time to wait for the JS wait-strategy to be satisfied.
    #[serde(with = "duration_serde")]
    pub js_wait_timeout: Duration,

    /// Automatically detect the SPA framework (React, Vue, Angular, …) and
    /// apply framework-specific wait logic before extracting content.
    #[serde(default)]
    pub detect_spa: bool,

    /// Randomize browser fingerprint details (viewport noise, WebGL vendor
    /// strings, navigator.languages) to reduce bot-detection signals.
    #[serde(default)]
    pub randomize_fingerprint: bool,
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
            // v0.2.0 fields
            viewport: Viewport::default(),
            js_scroll_count: 0,
            dismiss_popups: false,
            screenshot: false,
            screenshot_format: ScreenshotFormat::default(),
            screenshot_output_dir: None,
            js_wait_timeout: Duration::from_secs(10),
            detect_spa: false,
            randomize_fingerprint: false,
        }
    }
}

// ---------------------------------------------------------------------------
// CrawlConfigBuilder
// ---------------------------------------------------------------------------

/// Fluent builder for [`CrawlConfig`].
///
/// Start from defaults and override only what you need.
/// Call [`.build()`](Self::build) at the end to obtain the final [`CrawlConfig`].
///
/// # Examples
///
/// ## Static crawl — no JavaScript
///
/// ```rust
/// use rehyke_core::{CrawlConfigBuilder, ScanMode};
///
/// let config = CrawlConfigBuilder::new()
///     .mode(ScanMode::Full)
///     .max_pages(500)
///     .concurrency(20)
///     .clean_navigation(true)
///     .clean_ads(true)
///     .exclude_patterns(vec![r"\.pdf$".into(), r"/login".into()])
///     .build();
///
/// assert_eq!(config.max_pages, 500);
/// assert!(config.clean_ads);
/// ```
///
/// ## JavaScript SPA crawl (v0.2.0)
///
/// ```rust
/// use rehyke_core::{CrawlConfigBuilder, ScanMode, Viewport, WaitStrategy, ScreenshotFormat};
/// use std::time::Duration;
/// use std::path::PathBuf;
///
/// let config = CrawlConfigBuilder::new()
///     .mode(ScanMode::Full)
///     .enable_js(true)
///     // Wait strategies: NetworkIdle | Selector { selector } | Duration { duration } | Auto
///     .js_wait_strategy(WaitStrategy::NetworkIdle)
///     .js_wait_timeout(Duration::from_secs(12))
///     .js_scroll_count(8)          // scroll 8 viewports for infinite scroll
///     .dismiss_popups(true)        // dismiss cookie/GDPR banners automatically
///     .detect_spa(true)            // identify React/Vue/Angular in results
///     .viewport(Viewport::Desktop) // 1920×1080 | Tablet 768×1024 | Mobile 390×844
///     .randomize_fingerprint(true) // randomise UA, WebGL, languages, timezone
///     // Screenshot every crawled page as PNG
///     .screenshot(true)
///     .screenshot_format(ScreenshotFormat::Png)
///     .screenshot_output_dir(PathBuf::from("/tmp/shots"))
///     .max_pages(200)
///     .build();
///
/// assert!(config.enable_js);
/// assert_eq!(config.viewport, Viewport::Desktop);
/// assert!(config.screenshot);
/// ```
///
/// ## Delay + proxy configuration
///
/// ```rust
/// use rehyke_core::{CrawlConfigBuilder, DelayStrategy, ScanMode};
/// use std::time::Duration;
///
/// let config = CrawlConfigBuilder::new()
///     .mode(ScanMode::Deep)
///     .delay_strategy(DelayStrategy::Random {
///         min: Duration::from_millis(300),
///         max: Duration::from_millis(1500),
///     })
///     .respect_robots_txt(true)
///     .build();
///
/// assert!(config.respect_robots_txt);
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

    // -----------------------------------------------------------------------
    // v0.2.0 builder methods
    // -----------------------------------------------------------------------

    /// Set the browser viewport profile (desktop / tablet / mobile).
    pub fn viewport(mut self, viewport: Viewport) -> Self {
        self.inner.viewport = viewport;
        self
    }

    /// Number of scroll operations to perform for infinite-scroll detection.
    /// Set to `0` (default) to disable auto-scrolling.
    pub fn js_scroll_count(mut self, count: usize) -> Self {
        self.inner.js_scroll_count = count;
        self
    }

    /// Enable automatic popup dismissal (cookie consent, GDPR modals, etc.).
    pub fn dismiss_popups(mut self, dismiss: bool) -> Self {
        self.inner.dismiss_popups = dismiss;
        self
    }

    /// Capture a full-page screenshot after JS rendering.
    pub fn screenshot(mut self, screenshot: bool) -> Self {
        self.inner.screenshot = screenshot;
        self
    }

    /// Set the screenshot image format (PNG or JPEG).
    pub fn screenshot_format(mut self, format: ScreenshotFormat) -> Self {
        self.inner.screenshot_format = format;
        self
    }

    /// Directory where screenshots are saved.
    pub fn screenshot_output_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.inner.screenshot_output_dir = Some(dir.into());
        self
    }

    /// Maximum time to wait for the JS wait-strategy to succeed.
    pub fn js_wait_timeout(mut self, timeout: Duration) -> Self {
        self.inner.js_wait_timeout = timeout;
        self
    }

    /// Enable SPA framework auto-detection and framework-specific wait logic.
    pub fn detect_spa(mut self, detect: bool) -> Self {
        self.inner.detect_spa = detect;
        self
    }

    /// Randomize browser fingerprint details to reduce bot-detection signals.
    pub fn randomize_fingerprint(mut self, randomize: bool) -> Self {
        self.inner.randomize_fingerprint = randomize;
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

    // -----------------------------------------------------------------------
    // Viewport (v0.2.0)
    // -----------------------------------------------------------------------

    #[test]
    fn viewport_default_is_desktop() {
        assert_eq!(Viewport::default(), Viewport::Desktop);
    }

    #[test]
    fn viewport_dimensions_match_spec() {
        assert_eq!(Viewport::Desktop.dimensions(), (1920, 1080));
        assert_eq!(Viewport::Tablet.dimensions(), (768, 1024));
        assert_eq!(Viewport::Mobile.dimensions(), (390, 844));
    }

    #[test]
    fn viewport_device_scale_factor() {
        // Each profile has the correct CSS pixel ratio.
        assert!((Viewport::Desktop.device_scale_factor() - 1.0).abs() < f64::EPSILON);
        assert!((Viewport::Tablet.device_scale_factor() - 2.0).abs() < f64::EPSILON);
        assert!((Viewport::Mobile.device_scale_factor() - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn viewport_is_mobile_and_has_touch() {
        assert!(!Viewport::Desktop.is_mobile());
        assert!(!Viewport::Desktop.has_touch());

        assert!(Viewport::Tablet.is_mobile());
        assert!(Viewport::Tablet.has_touch());

        assert!(Viewport::Mobile.is_mobile());
        assert!(Viewport::Mobile.has_touch());
    }

    #[test]
    fn viewport_physical_pixel_dimensions() {
        // Physical pixels = CSS pixels × device scale factor.
        for vp in [Viewport::Desktop, Viewport::Tablet, Viewport::Mobile] {
            let (w, h) = vp.dimensions();
            let dpr = vp.device_scale_factor();
            let phys_w = (w as f64 * dpr).round() as u64;
            let phys_h = (h as f64 * dpr).round() as u64;
            assert!(phys_w > 0, "physical width must be positive for {:?}", vp);
            assert!(phys_h > 0, "physical height must be positive for {:?}", vp);
        }
    }

    #[test]
    fn viewport_serde_roundtrip() {
        for vp in [Viewport::Desktop, Viewport::Tablet, Viewport::Mobile] {
            let json = serde_json::to_string(&vp).expect("serialize viewport");
            let back: Viewport = serde_json::from_str(&json).expect("deserialize viewport");
            assert_eq!(vp, back);
        }
    }

    #[test]
    fn viewport_serde_values() {
        // Serialises to lowercase strings.
        assert_eq!(serde_json::to_string(&Viewport::Desktop).unwrap(), "\"desktop\"");
        assert_eq!(serde_json::to_string(&Viewport::Tablet).unwrap(), "\"tablet\"");
        assert_eq!(serde_json::to_string(&Viewport::Mobile).unwrap(), "\"mobile\"");
    }

    // -----------------------------------------------------------------------
    // ScreenshotFormat (v0.2.0)
    // -----------------------------------------------------------------------

    #[test]
    fn screenshot_format_default_is_png() {
        assert_eq!(ScreenshotFormat::default(), ScreenshotFormat::Png);
    }

    #[test]
    fn screenshot_format_serde_roundtrip() {
        for fmt in [ScreenshotFormat::Png, ScreenshotFormat::Jpeg] {
            let json = serde_json::to_string(&fmt).expect("serialize");
            let back: ScreenshotFormat = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(fmt, back);
        }
    }

    #[test]
    fn screenshot_format_serde_values() {
        assert_eq!(serde_json::to_string(&ScreenshotFormat::Png).unwrap(), "\"png\"");
        assert_eq!(serde_json::to_string(&ScreenshotFormat::Jpeg).unwrap(), "\"jpeg\"");
    }

    // -----------------------------------------------------------------------
    // WaitStrategy (v0.2.0)
    // -----------------------------------------------------------------------

    #[test]
    fn wait_strategy_default_is_auto() {
        assert!(matches!(WaitStrategy::default(), WaitStrategy::Auto));
    }

    #[test]
    fn wait_strategy_selector_stores_css() {
        let ws = WaitStrategy::Selector {
            selector: "#root".to_string(),
        };
        match ws {
            WaitStrategy::Selector { selector } => assert_eq!(selector, "#root"),
            other => panic!("expected Selector, got {:?}", other),
        }
    }

    #[test]
    fn wait_strategy_duration_stores_value() {
        let ws = WaitStrategy::Duration {
            duration: std::time::Duration::from_secs(5),
        };
        match ws {
            WaitStrategy::Duration { duration } => {
                assert_eq!(duration, std::time::Duration::from_secs(5))
            }
            other => panic!("expected Duration, got {:?}", other),
        }
    }

    #[test]
    fn wait_strategy_serde_roundtrip() {
        let strategies = vec![
            WaitStrategy::Auto,
            WaitStrategy::NetworkIdle,
            WaitStrategy::Selector {
                selector: "div.ready".to_string(),
            },
            WaitStrategy::Duration {
                duration: std::time::Duration::from_millis(1500),
            },
        ];
        for ws in strategies {
            let json = serde_json::to_string(&ws).expect("serialize WaitStrategy");
            let back: WaitStrategy = serde_json::from_str(&json).expect("deserialize WaitStrategy");
            // Compare variant by re-serialising (WaitStrategy isn't PartialEq, but json tags match).
            let json2 = serde_json::to_string(&back).expect("re-serialize");
            assert_eq!(json, json2);
        }
    }

    // -----------------------------------------------------------------------
    // CrawlConfigBuilder — v0.2.0 methods
    // -----------------------------------------------------------------------

    #[test]
    fn builder_v020_viewport() {
        let cfg = CrawlConfigBuilder::new().viewport(Viewport::Mobile).build();
        assert_eq!(cfg.viewport, Viewport::Mobile);

        let cfg2 = CrawlConfigBuilder::new().viewport(Viewport::Tablet).build();
        assert_eq!(cfg2.viewport, Viewport::Tablet);
    }

    #[test]
    fn builder_v020_screenshot_fields() {
        let dir = PathBuf::from("/tmp/shots");
        let cfg = CrawlConfigBuilder::new()
            .screenshot(true)
            .screenshot_format(ScreenshotFormat::Jpeg)
            .screenshot_output_dir(dir.clone())
            .build();

        assert!(cfg.screenshot);
        assert_eq!(cfg.screenshot_format, ScreenshotFormat::Jpeg);
        assert_eq!(cfg.screenshot_output_dir, Some(dir));
    }

    #[test]
    fn builder_v020_js_fields() {
        let timeout = Duration::from_secs(15);
        let cfg = CrawlConfigBuilder::new()
            .enable_js(true)
            .js_wait_strategy(WaitStrategy::NetworkIdle)
            .js_wait_timeout(timeout)
            .js_scroll_count(8)
            .dismiss_popups(true)
            .build();

        assert!(cfg.enable_js);
        assert!(matches!(cfg.js_wait_strategy, WaitStrategy::NetworkIdle));
        assert_eq!(cfg.js_wait_timeout, timeout);
        assert_eq!(cfg.js_scroll_count, 8);
        assert!(cfg.dismiss_popups);
    }

    #[test]
    fn builder_v020_detect_spa_and_fingerprint() {
        let cfg = CrawlConfigBuilder::new()
            .detect_spa(true)
            .randomize_fingerprint(true)
            .build();

        assert!(cfg.detect_spa);
        assert!(cfg.randomize_fingerprint);
    }

    #[test]
    fn builder_v020_selector_wait_strategy() {
        let cfg = CrawlConfigBuilder::new()
            .js_wait_strategy(WaitStrategy::Selector {
                selector: "#app".to_string(),
            })
            .build();

        assert!(matches!(
            cfg.js_wait_strategy,
            WaitStrategy::Selector { ref selector } if selector == "#app"
        ));
    }

    // -----------------------------------------------------------------------
    // CrawlConfig default values — v0.2.0 fields
    // -----------------------------------------------------------------------

    #[test]
    fn default_config_v020_defaults() {
        let cfg = CrawlConfig::default();

        // JS rendering off by default.
        assert!(!cfg.enable_js);
        assert!(!cfg.dismiss_popups);
        assert!(!cfg.screenshot);
        assert!(!cfg.detect_spa);
        assert!(!cfg.randomize_fingerprint);

        // Scroll disabled by default.
        assert_eq!(cfg.js_scroll_count, 0);

        // Sensible default wait timeout.
        assert!(cfg.js_wait_timeout >= Duration::from_secs(5),
            "js_wait_timeout should be at least 5 s by default");

        // Screenshot format defaults to PNG.
        assert_eq!(cfg.screenshot_format, ScreenshotFormat::Png);

        // No screenshot output dir by default.
        assert!(cfg.screenshot_output_dir.is_none());

        // Desktop viewport by default.
        assert_eq!(cfg.viewport, Viewport::Desktop);
    }

    #[test]
    fn default_config_v020_wait_strategy_is_auto() {
        let cfg = CrawlConfig::default();
        assert!(matches!(cfg.js_wait_strategy, WaitStrategy::Auto));
    }
}
