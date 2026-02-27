use pyo3::exceptions::{PyIOError, PyRuntimeError, PyTimeoutError, PyValueError};
use pyo3::prelude::*;
use std::path::PathBuf;
use std::time::Duration;

use rehyke_core::config::{
    CrawlConfig as CoreCrawlConfig, CrawlConfigBuilder, DelayStrategy,
    ScanMode as CoreScanMode, ScreenshotFormat as CoreScreenshotFormat,
    Viewport as CoreViewport, WaitStrategy,
};
use rehyke_core::error::RehykeError;
use rehyke_core::output::CrawlResult as CoreCrawlResult;
use rehyke_core::Rehyke as CoreRehyke;

// ---------------------------------------------------------------------------
// Error conversion
// ---------------------------------------------------------------------------

fn to_py_err(err: RehykeError) -> PyErr {
    match err {
        RehykeError::Timeout { .. } => PyTimeoutError::new_err(err.to_string()),
        RehykeError::IoError(_) => PyIOError::new_err(err.to_string()),
        RehykeError::ConfigError { .. } | RehykeError::UrlParseError(_) => {
            PyValueError::new_err(err.to_string())
        }
        _ => PyRuntimeError::new_err(err.to_string()),
    }
}

// ---------------------------------------------------------------------------
// ScanMode enum
// ---------------------------------------------------------------------------

/// Scan mode preset controlling crawl depth, page limits, and concurrency.
#[pyclass(eq, eq_int)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScanMode {
    /// Quick surface-level crawl.
    LITE = 0,
    /// Balanced crawl for most use-cases (default).
    FULL = 1,
    /// Exhaustive deep crawl.
    DEEP = 2,
}

impl From<ScanMode> for CoreScanMode {
    fn from(mode: ScanMode) -> Self {
        match mode {
            ScanMode::LITE => CoreScanMode::Lite,
            ScanMode::FULL => CoreScanMode::Full,
            ScanMode::DEEP => CoreScanMode::Deep,
        }
    }
}

impl From<CoreScanMode> for ScanMode {
    fn from(mode: CoreScanMode) -> Self {
        match mode {
            CoreScanMode::Lite => ScanMode::LITE,
            CoreScanMode::Full => ScanMode::FULL,
            CoreScanMode::Deep => ScanMode::DEEP,
        }
    }
}

// ---------------------------------------------------------------------------
// CrawlResult
// ---------------------------------------------------------------------------

/// Result of crawling a single page.
#[pyclass]
#[derive(Debug, Clone)]
struct CrawlResult {
    /// The URL that was crawled.
    #[pyo3(get)]
    url: String,
    /// Page title extracted from the document.
    #[pyo3(get)]
    title: String,
    /// Markdown representation of the page content.
    #[pyo3(get)]
    markdown: String,
    /// HTTP status code of the response.
    #[pyo3(get)]
    status_code: u16,
    /// Content-Type header value from the response.
    #[pyo3(get)]
    content_type: String,
    /// How the page was rendered: "static" or "javascript".
    #[pyo3(get)]
    render_method: String,
    /// Crawl depth at which this page was discovered.
    #[pyo3(get)]
    depth: u32,
}

impl From<CoreCrawlResult> for CrawlResult {
    fn from(r: CoreCrawlResult) -> Self {
        let render_method = match r.render_method {
            rehyke_core::output::RenderMethod::Static => "static".to_string(),
            rehyke_core::output::RenderMethod::JavaScript => "javascript".to_string(),
        };
        Self {
            url: r.url,
            title: r.title,
            markdown: r.markdown,
            status_code: r.status_code,
            content_type: r.content_type,
            render_method,
            depth: r.depth,
        }
    }
}

#[pymethods]
impl CrawlResult {
    fn __repr__(&self) -> String {
        format!(
            "CrawlResult(url='{}', title='{}', status_code={}, render='{}')",
            self.url, self.title, self.status_code, self.render_method
        )
    }

    fn __str__(&self) -> String {
        self.markdown.clone()
    }
}

// ---------------------------------------------------------------------------
// CrawlConfig
// ---------------------------------------------------------------------------

/// Configuration for a crawl job.
///
/// All parameters are optional and have sensible defaults.
///
/// v0.2.0 adds JavaScript rendering parameters:
///
/// ```python
/// config = CrawlConfig(
///     enable_js=True,
///     js_wait_strategy="network_idle",
///     js_wait_timeout=10.0,
///     scroll_count=5,
///     dismiss_popups=True,
///     viewport="desktop",
///     screenshot=True,
///     screenshot_format="png",
///     detect_spa=True,
///     randomize_fingerprint=True,
/// )
/// ```
#[pyclass]
#[derive(Debug, Clone)]
struct CrawlConfig {
    inner: CoreCrawlConfig,
}

#[pymethods]
impl CrawlConfig {
    #[new]
    #[pyo3(signature = (
        mode = None,
        max_depth = None,
        max_pages = None,
        concurrency = None,
        enable_js = false,
        user_agent = None,
        timeout_secs = None,
        max_retries = None,
        respect_robots_txt = true,
        clean_navigation = true,
        clean_footers = true,
        clean_ads = true,
        exclude_patterns = None,
        include_patterns = None,
        delay_min_ms = None,
        delay_max_ms = None,
        js_wait_strategy = "auto",
        js_wait_timeout = 10.0,
        scroll_count = 0,
        dismiss_popups = false,
        viewport = "desktop",
        screenshot = false,
        screenshot_format = "png",
        screenshot_dir = None,
        detect_spa = false,
        randomize_fingerprint = false,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        mode: Option<ScanMode>,
        max_depth: Option<usize>,
        max_pages: Option<usize>,
        concurrency: Option<usize>,
        enable_js: bool,
        user_agent: Option<String>,
        timeout_secs: Option<u64>,
        max_retries: Option<u32>,
        respect_robots_txt: bool,
        clean_navigation: bool,
        clean_footers: bool,
        clean_ads: bool,
        exclude_patterns: Option<Vec<String>>,
        include_patterns: Option<Vec<String>>,
        delay_min_ms: Option<u64>,
        delay_max_ms: Option<u64>,
        // v0.2.0 parameters
        js_wait_strategy: &str,
        js_wait_timeout: f64,
        scroll_count: usize,
        dismiss_popups: bool,
        viewport: &str,
        screenshot: bool,
        screenshot_format: &str,
        screenshot_dir: Option<String>,
        detect_spa: bool,
        randomize_fingerprint: bool,
    ) -> PyResult<Self> {
        let core_mode: CoreScanMode = mode.unwrap_or(ScanMode::FULL).into();

        let wait_strategy = parse_wait_strategy(js_wait_strategy).map_err(PyValueError::new_err)?;
        let core_viewport = parse_viewport(viewport).map_err(PyValueError::new_err)?;
        let core_screenshot_format =
            parse_screenshot_format(screenshot_format).map_err(PyValueError::new_err)?;

        let mut builder = CrawlConfigBuilder::new()
            .mode(core_mode)
            .enable_js(enable_js)
            .respect_robots_txt(respect_robots_txt)
            .clean_navigation(clean_navigation)
            .clean_footers(clean_footers)
            .clean_ads(clean_ads)
            // v0.2.0
            .js_wait_strategy(wait_strategy)
            .js_wait_timeout(Duration::from_secs_f64(js_wait_timeout))
            .js_scroll_count(scroll_count)
            .dismiss_popups(dismiss_popups)
            .viewport(core_viewport)
            .screenshot(screenshot)
            .screenshot_format(core_screenshot_format)
            .detect_spa(detect_spa)
            .randomize_fingerprint(randomize_fingerprint);

        if let Some(dir) = screenshot_dir {
            builder = builder.screenshot_output_dir(PathBuf::from(dir));
        }

        if let Some(depth) = max_depth {
            builder = builder.max_depth(depth);
        }
        if let Some(pages) = max_pages {
            builder = builder.max_pages(pages);
        }
        if let Some(conc) = concurrency {
            builder = builder.concurrency(conc);
        }
        if let Some(ua) = user_agent {
            builder = builder.user_agent(ua);
        }
        if let Some(secs) = timeout_secs {
            builder = builder.timeout(Duration::from_secs(secs));
        }
        if let Some(retries) = max_retries {
            let mut retry_config = rehyke_core::RetryConfig::default();
            retry_config.max_retries = retries;
            builder = builder.retry_config(retry_config);
        }
        if let Some(patterns) = exclude_patterns {
            builder = builder.exclude_patterns(patterns);
        }
        if let Some(patterns) = include_patterns {
            builder = builder.include_patterns(patterns);
        }

        // Delay strategy.
        match (delay_min_ms, delay_max_ms) {
            (Some(min), Some(max)) => {
                builder = builder.delay_strategy(DelayStrategy::Random {
                    min: Duration::from_millis(min),
                    max: Duration::from_millis(max),
                });
            }
            (Some(fixed), None) | (None, Some(fixed)) => {
                builder = builder.delay_strategy(DelayStrategy::Fixed {
                    delay: Duration::from_millis(fixed),
                });
            }
            (None, None) => {}
        }

        Ok(Self {
            inner: builder.build(),
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "CrawlConfig(mode={:?}, max_depth={}, max_pages={}, concurrency={}, enable_js={}, viewport={:?})",
            self.inner.mode,
            self.inner.max_depth,
            self.inner.max_pages,
            self.inner.concurrency,
            self.inner.enable_js,
            self.inner.viewport,
        )
    }
}

// ---------------------------------------------------------------------------
// Parse helpers for string-based Python parameters
// ---------------------------------------------------------------------------

fn parse_wait_strategy(s: &str) -> Result<WaitStrategy, String> {
    match s.to_lowercase().as_str() {
        "auto" => Ok(WaitStrategy::Auto),
        "network_idle" | "networkidle" | "idle" => Ok(WaitStrategy::NetworkIdle),
        s if s.starts_with("selector:") => {
            let selector = s.strip_prefix("selector:").unwrap_or("").trim().to_string();
            if selector.is_empty() {
                Err("selector wait strategy requires a non-empty CSS selector after 'selector:'"
                    .into())
            } else {
                Ok(WaitStrategy::Selector { selector })
            }
        }
        s => {
            // Try to parse as seconds for a Duration wait.
            if let Ok(secs) = s.parse::<f64>() {
                Ok(WaitStrategy::Duration {
                    duration: Duration::from_secs_f64(secs),
                })
            } else {
                Err(format!(
                    "unknown js_wait_strategy '{}': expected 'auto', 'network_idle', \
                     'selector:<CSS>', or a number of seconds",
                    s
                ))
            }
        }
    }
}

fn parse_viewport(s: &str) -> Result<CoreViewport, String> {
    match s.to_lowercase().as_str() {
        "desktop" => Ok(CoreViewport::Desktop),
        "tablet" => Ok(CoreViewport::Tablet),
        "mobile" => Ok(CoreViewport::Mobile),
        other => Err(format!(
            "unknown viewport '{}': expected 'desktop', 'tablet', or 'mobile'",
            other
        )),
    }
}

fn parse_screenshot_format(s: &str) -> Result<CoreScreenshotFormat, String> {
    match s.to_lowercase().as_str() {
        "png" => Ok(CoreScreenshotFormat::Png),
        "jpeg" | "jpg" => Ok(CoreScreenshotFormat::Jpeg),
        other => Err(format!(
            "unknown screenshot_format '{}': expected 'png' or 'jpeg'",
            other
        )),
    }
}

// ---------------------------------------------------------------------------
// Rehyke class
// ---------------------------------------------------------------------------

/// Main crawler class.
///
/// Create an instance with optional configuration, then call `crawl()` or
/// `crawl_to_file()` to perform crawls.
///
/// Example with JS rendering (v0.2.0+):
///
/// ```python
/// import rehyke
///
/// config = rehyke.CrawlConfig(
///     enable_js=True,
///     js_wait_strategy="network_idle",
///     js_wait_timeout=10.0,
///     scroll_count=5,
///     dismiss_popups=True,
///     viewport="desktop",
///     screenshot=True,
///     detect_spa=True,
/// )
///
/// crawler = rehyke.Rehyke(config)
/// results = crawler.crawl("https://react-app.example.com")
/// for page in results:
///     print(f"[{page.render_method}] {page.title}")
///     print(page.markdown[:500])
/// ```
#[pyclass]
#[derive(Debug)]
struct Rehyke {
    config: CoreCrawlConfig,
}

#[pymethods]
impl Rehyke {
    #[new]
    #[pyo3(signature = (config = None))]
    fn new(config: Option<CrawlConfig>) -> Self {
        Self {
            config: config.map(|c| c.inner).unwrap_or_default(),
        }
    }

    /// Crawl a URL and return a list of CrawlResult objects.
    fn crawl(&self, py: Python<'_>, url: &str) -> PyResult<Vec<CrawlResult>> {
        let config = self.config.clone();
        let url = url.to_string();

        py.allow_threads(|| {
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| PyRuntimeError::new_err(format!("failed to create runtime: {}", e)))?;

            rt.block_on(async {
                let crawler = CoreRehyke::new(config);
                let results = crawler.run(&url).await.map_err(to_py_err)?;
                Ok(results.into_iter().map(CrawlResult::from).collect())
            })
        })
    }

    /// Crawl a URL and write the markdown output to a file.
    fn crawl_to_file(&self, py: Python<'_>, url: &str, path: &str) -> PyResult<()> {
        let results = self.crawl(py, url)?;

        let mut content = String::new();
        for (i, result) in results.iter().enumerate() {
            if i > 0 {
                content.push_str("\n---\n\n");
            }
            content.push_str(&result.markdown);
        }

        std::fs::write(path, &content)
            .map_err(|e| PyIOError::new_err(format!("failed to write to '{}': {}", path, e)))?;

        Ok(())
    }

    fn __repr__(&self) -> String {
        format!(
            "Rehyke(mode={:?}, max_depth={}, concurrency={}, enable_js={})",
            self.config.mode,
            self.config.max_depth,
            self.config.concurrency,
            self.config.enable_js,
        )
    }
}

// ---------------------------------------------------------------------------
// Module-level convenience function
// ---------------------------------------------------------------------------

/// Simple one-shot crawl function.
///
/// ```python
/// import rehyke
/// results = rehyke.crawl("https://example.com", mode="full")
/// for page in results:
///     print(page.title)
///     print(page.markdown)
/// ```
#[pyfunction]
#[pyo3(signature = (url, mode = "full"))]
fn crawl(py: Python<'_>, url: &str, mode: &str) -> PyResult<Vec<CrawlResult>> {
    let core_mode = match mode.to_lowercase().as_str() {
        "lite" => CoreScanMode::Lite,
        "full" => CoreScanMode::Full,
        "deep" => CoreScanMode::Deep,
        other => {
            return Err(PyValueError::new_err(format!(
                "invalid mode '{}': expected 'lite', 'full', or 'deep'",
                other
            )));
        }
    };

    let url = url.to_string();

    py.allow_threads(|| {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| PyRuntimeError::new_err(format!("failed to create runtime: {}", e)))?;

        rt.block_on(async {
            let results = CoreRehyke::crawl(&url, core_mode).await.map_err(to_py_err)?;
            Ok(results.into_iter().map(CrawlResult::from).collect())
        })
    })
}

// ---------------------------------------------------------------------------
// Module registration
// ---------------------------------------------------------------------------

/// Python module for the Rehyke web crawler.
///
/// v0.2.0: JavaScript rendering, SPA support, infinite scroll, popup
/// dismissal, screenshots, and browser fingerprint diversity.
#[pymodule]
fn rehyke(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ScanMode>()?;
    m.add_class::<CrawlResult>()?;
    m.add_class::<CrawlConfig>()?;
    m.add_class::<Rehyke>()?;
    m.add_function(wrap_pyfunction!(crawl, m)?)?;
    Ok(())
}
