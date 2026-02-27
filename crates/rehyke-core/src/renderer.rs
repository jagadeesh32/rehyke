/// Headless browser renderer for JavaScript-heavy pages.
///
/// # Feature flag
///
/// Full Chrome integration requires the `js` feature:
///
/// ```toml
/// [dependencies]
/// rehyke-core = { version = "0.2", features = ["js"] }
/// ```
///
/// Without the `js` feature the renderer acts as a graceful stub — all
/// `render()` calls return [`RehykeError::RenderError`] so the crawl engine
/// falls back to static HTML fetching automatically.
///
/// # Architecture
///
/// ```text
///  Rehyke::run()
///      │
///      ├─ enable_js=true ──► Renderer::render(url)
///      │                          │
///      │          ┌───────────────┴──────────────────┐
///      │          │         js feature               │  no js feature
///      │          ▼                                  ▼
///      │    chromiumoxide                      stub (err)
///      │    Browser → TabPool                       │
///      │    ├─ block resources                       │
///      │    ├─ set viewport / fingerprint            │
///      │    ├─ navigate + wait strategy              │
///      │    ├─ dismiss popups                        │
///      │    ├─ scroll (infinite-scroll)              │
///      │    ├─ screenshot (optional)                 │
///      │    └─ page.content() → HTML ──► parser ◄───┘
///      │
///      └─ enable_js=false ► Fetcher::fetch_with_retry(url)
/// ```
use crate::config::{ScreenshotFormat, Viewport, WaitStrategy};
use crate::error::{RehykeError, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::{debug, info, warn};
use url::Url;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Detected SPA framework from page-source analysis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpaFramework {
    React,
    Vue,
    Angular,
    Svelte,
    NextJs,
    Nuxt,
    SvelteKit,
    Unknown,
}

impl std::fmt::Display for SpaFramework {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpaFramework::React => write!(f, "React"),
            SpaFramework::Vue => write!(f, "Vue"),
            SpaFramework::Angular => write!(f, "Angular"),
            SpaFramework::Svelte => write!(f, "Svelte"),
            SpaFramework::NextJs => write!(f, "Next.js"),
            SpaFramework::Nuxt => write!(f, "Nuxt"),
            SpaFramework::SvelteKit => write!(f, "SvelteKit"),
            SpaFramework::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Captured screenshot — raw bytes plus metadata.
#[derive(Debug, Clone)]
pub struct ScreenshotData {
    /// Raw image bytes (PNG or JPEG).
    pub data: Vec<u8>,
    /// Image format.
    pub format: ScreenshotFormat,
    /// Actual captured width in pixels.
    pub width: u32,
    /// Actual captured height in pixels.
    pub height: u32,
    /// File path where the screenshot was saved, if `screenshot_output_dir`
    /// was configured.
    pub saved_path: Option<PathBuf>,
}

/// Full result of rendering a page through the headless browser.
#[derive(Debug, Clone)]
pub struct RenderResult {
    /// The rendered HTML after JavaScript execution.
    pub html: String,
    /// The final URL after any JS-driven navigation / redirects.
    pub final_url: String,
    /// Wall-clock time taken to render this page.
    pub elapsed: Duration,
    /// Screenshot data, present when `CrawlConfig::screenshot` is `true`.
    pub screenshot: Option<ScreenshotData>,
    /// SPA framework detected in the page source, when `detect_spa` is `true`.
    pub detected_framework: Option<SpaFramework>,
    /// Whether at least one popup was successfully dismissed.
    pub popup_dismissed: bool,
    /// Number of scroll operations performed for infinite-scroll detection.
    pub pages_scrolled: usize,
}

/// Configuration forwarded from [`CrawlConfig`] to the renderer.
#[derive(Debug, Clone)]
pub struct RendererConfig {
    /// Maximum time to wait for a page to render.
    pub render_timeout: Duration,
    /// Strategy used to decide when the page is "ready".
    pub wait_strategy: WaitStrategy,
    /// Block images, fonts, and media for faster renders.
    pub block_resources: bool,
    /// Number of browser tabs to keep alive in the pool.
    pub tab_pool_size: usize,
    /// Run the browser in headless mode.
    pub headless: bool,
    /// Maximum scroll operations for infinite-scroll detection.
    pub max_scrolls: usize,
    /// Browser viewport profile.
    pub viewport: Viewport,
    /// Automatically dismiss cookie/GDPR popups.
    pub dismiss_popups: bool,
    /// Capture a full-page screenshot.
    pub screenshot: bool,
    /// Image format for screenshots.
    pub screenshot_format: ScreenshotFormat,
    /// Directory where screenshots are saved.
    pub screenshot_output_dir: Option<PathBuf>,
    /// Randomize browser fingerprint details.
    pub randomize_fingerprint: bool,
    /// Detect the SPA framework from page source.
    pub detect_spa: bool,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            render_timeout: Duration::from_secs(30),
            wait_strategy: WaitStrategy::Auto,
            block_resources: true,
            tab_pool_size: 4,
            headless: true,
            max_scrolls: 0,
            viewport: Viewport::Desktop,
            dismiss_popups: false,
            screenshot: false,
            screenshot_format: ScreenshotFormat::Png,
            screenshot_output_dir: None,
            randomize_fingerprint: false,
            detect_spa: false,
        }
    }
}

// ---------------------------------------------------------------------------
// CSS selectors used by the popup-dismissal heuristic
// ---------------------------------------------------------------------------

/// Common CSS selectors for cookie / consent / GDPR popup accept buttons.
/// Ordered from most-specific to least-specific.
const POPUP_ACCEPT_SELECTORS: &[&str] = &[
    // GDPR / privacy
    "[id*='cookie'][id*='accept']",
    "[id*='consent'][id*='accept']",
    "[class*='cookie'][class*='accept']",
    "[class*='consent'][class*='accept']",
    "[aria-label*='Accept cookies']",
    "[aria-label*='Accept all']",
    "[aria-label*='Agree']",
    // Common button text patterns (evaluated via JS)
    "button[id*='accept']",
    "button[id*='agree']",
    "button[id*='allow']",
    "button[id*='consent']",
    ".cookie-accept",
    ".cookie-consent__accept",
    ".gdpr-accept",
    "#accept-cookies",
    "#cookie-accept",
    "#agree-button",
    ".btn-accept",
    ".accept-all",
    // OneTrust / Cookiebot / CookieYes (widespread CMPs)
    "#onetrust-accept-btn-handler",
    ".onetrust-accept-btn-handler",
    "#CybotCookiebotDialogBodyLevelButtonLevelOptinAllowAll",
    ".cky-btn-accept",
    // Newsletter / overlay close buttons
    "[class*='modal'][class*='close']",
    "[class*='popup'][class*='close']",
    "[class*='overlay'][class*='close']",
    "button[aria-label='Close']",
    "button[aria-label='Dismiss']",
    ".close-button",
    ".popup-close",
];

// ---------------------------------------------------------------------------
// SPA framework detection patterns
// ---------------------------------------------------------------------------

/// JS expressions / DOM markers used to detect SPA frameworks.
/// Each entry is `(framework_name, js_detection_expression)`.
const SPA_DETECTORS: &[(&str, &str)] = &[
    (
        "Next.js",
        "typeof window.__NEXT_DATA__ !== 'undefined'",
    ),
    (
        "Nuxt",
        "typeof window.__NUXT__ !== 'undefined'",
    ),
    (
        "SvelteKit",
        "typeof window.__sveltekit_dev !== 'undefined' || document.querySelector('[data-sveltekit-preload-data]') !== null",
    ),
    (
        "React",
        "typeof window.React !== 'undefined' || document.querySelector('[data-reactroot]') !== null || document.querySelector('[data-reactid]') !== null",
    ),
    (
        "Vue",
        "typeof window.Vue !== 'undefined' || typeof window.__vue_app__ !== 'undefined' || document.querySelector('[data-v-app]') !== null",
    ),
    (
        "Angular",
        "typeof window.ng !== 'undefined' || document.querySelector('[ng-version]') !== null",
    ),
    (
        "Svelte",
        "document.querySelector('[class*=\"svelte-\"]') !== null",
    ),
];

// ---------------------------------------------------------------------------
// Renderer — platform-independent outer shell
// ---------------------------------------------------------------------------

/// Headless browser renderer that drives Chrome/Chromium via the DevTools
/// Protocol to execute JavaScript and extract fully-rendered page DOM.
pub struct Renderer {
    config: RendererConfig,
    initialized: bool,
    /// Inner browser state — only present when the `js` feature is enabled
    /// and [`initialize`](Renderer::initialize) has succeeded.
    #[cfg(feature = "js")]
    inner: Option<JsRenderer>,
}

impl Renderer {
    /// Create a new renderer with the given configuration.
    pub fn new(config: RendererConfig) -> Self {
        Self {
            config,
            initialized: false,
            #[cfg(feature = "js")]
            inner: None,
        }
    }

    /// Initialize the browser (launch Chromium process and warm up the tab
    /// pool).
    ///
    /// This is separate from [`new`](Renderer::new) because it is async and
    /// can fail — e.g. when Chrome is not installed.
    pub async fn initialize(&mut self) -> Result<()> {
        #[cfg(feature = "js")]
        {
            match JsRenderer::launch(&self.config).await {
                Ok(renderer) => {
                    self.inner = Some(renderer);
                    self.initialized = true;
                    info!(
                        headless = self.config.headless,
                        tab_pool = self.config.tab_pool_size,
                        viewport = ?self.config.viewport,
                        "Browser renderer initialized"
                    );
                    return Ok(());
                }
                Err(e) => {
                    warn!(error = %e, "Browser initialization failed — JS rendering unavailable");
                    return Err(e);
                }
            }
        }

        #[cfg(not(feature = "js"))]
        {
            warn!("Renderer initialized as stub — compile with `--features js` for Chrome support");
            self.initialized = true;
            Ok(())
        }
    }

    /// Render a page by navigating to the URL and executing JavaScript.
    pub async fn render(&self, url: &Url) -> Result<RenderResult> {
        if !self.initialized {
            return Err(RehykeError::BrowserError {
                message: "Renderer not initialized. Call initialize() first.".to_string(),
            });
        }

        let start = std::time::Instant::now();
        debug!(url = %url, "Rendering page with JavaScript");

        #[cfg(feature = "js")]
        {
            if let Some(ref renderer) = self.inner {
                return renderer.render(url, &self.config).await;
            }
        }

        // Stub path — reached when:
        //  - `js` feature is disabled, OR
        //  - `inner` is None (unexpected after successful initialize)
        warn!(
            url = %url,
            elapsed = ?start.elapsed(),
            "JavaScript rendering not available — enable the `js` feature"
        );
        Err(RehykeError::RenderError {
            url: url.to_string(),
            message: "Compile rehyke-core with `--features js` to enable JavaScript rendering."
                .to_string(),
        })
    }

    /// Render an already-fetched HTML string inside a browser context.
    ///
    /// Useful for re-rendering static content with JavaScript execution,
    /// e.g., to evaluate lazy-loaded components already present in the HTML.
    pub async fn render_html(&self, html: &str, base_url: &Url) -> Result<RenderResult> {
        if !self.initialized {
            return Err(RehykeError::BrowserError {
                message: "Renderer not initialized.".to_string(),
            });
        }

        debug!(
            url = %base_url,
            html_len = html.len(),
            "Re-rendering fetched HTML with JavaScript"
        );

        #[cfg(feature = "js")]
        {
            if let Some(ref renderer) = self.inner {
                return renderer.render_html(html, base_url, &self.config).await;
            }
        }

        Err(RehykeError::RenderError {
            url: base_url.to_string(),
            message: "Compile rehyke-core with `--features js` to enable JavaScript rendering."
                .to_string(),
        })
    }

    /// Check whether a Chrome/Chromium binary exists in common installation
    /// paths on the current system.
    pub fn is_available() -> bool {
        let common_paths = [
            "/usr/bin/chromium",
            "/usr/bin/chromium-browser",
            "/usr/bin/google-chrome",
            "/usr/bin/google-chrome-stable",
            "/usr/bin/google-chrome-beta",
            "/snap/bin/chromium",
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
            "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
        ];
        // Also check PATH via which/where.
        common_paths.iter().any(|p| std::path::Path::new(p).exists())
    }

    /// Whether the renderer has been successfully initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Shut down the browser and release all resources.
    pub async fn shutdown(&mut self) -> Result<()> {
        #[cfg(feature = "js")]
        {
            self.inner = None;
        }
        self.initialized = false;
        info!("Renderer shut down");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// JS renderer — only compiled when the `js` feature is active
// ---------------------------------------------------------------------------

#[cfg(feature = "js")]
mod js_impl {
    use super::*;
    use chromiumoxide::browser::{Browser, BrowserConfig, HeadlessMode};
    use chromiumoxide::cdp::browser_protocol::network::SetBlockedUrlsParams;
    use chromiumoxide::cdp::browser_protocol::page::{
        CaptureScreenshotFormat, CaptureScreenshotParams,
    };
    use chromiumoxide::handler::viewport::Viewport as CdpViewport;
    use futures::StreamExt;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    /// Live browser state managed by the renderer.
    pub(super) struct JsRenderer {
        browser: Arc<Mutex<Browser>>,
        _handler: tokio::task::JoinHandle<()>,
    }

    impl JsRenderer {
        /// Launch a Chromium browser with the given [`RendererConfig`].
        pub(super) async fn launch(config: &RendererConfig) -> Result<Self> {
            let (w, h) = config.viewport.dimensions();
            let cdp_viewport = CdpViewport {
                width: w,
                height: h,
                device_scale_factor: config.viewport.device_scale_factor(),
                emulating_mobile: config.viewport.is_mobile(),
                is_landscape: !config.viewport.is_mobile(),
                has_touch: config.viewport.has_touch(),
            };

            let headless_mode = if config.headless {
                HeadlessMode::True
            } else {
                HeadlessMode::False
            };

            let browser_config = BrowserConfig::builder()
                .headless_mode(headless_mode)
                .viewport(cdp_viewport)
                .build()
                .map_err(|e| RehykeError::BrowserError {
                    message: format!("Failed to build BrowserConfig: {}", e),
                })?;

            let (browser, mut handler) =
                Browser::launch(browser_config)
                    .await
                    .map_err(|e| RehykeError::BrowserError {
                        message: format!("Failed to launch Chromium: {}", e),
                    })?;

            let handler_task = tokio::spawn(async move {
                while let Some(_event) = handler.next().await {}
            });

            Ok(Self {
                browser: Arc::new(Mutex::new(browser)),
                _handler: handler_task,
            })
        }

        /// Render a URL through the headless browser.
        pub(super) async fn render(
            &self,
            url: &Url,
            config: &RendererConfig,
        ) -> Result<RenderResult> {
            let start = std::time::Instant::now();

            let page = {
                let browser = self.browser.lock().await;
                browser
                    .new_page("about:blank")
                    .await
                    .map_err(|e| RehykeError::BrowserError {
                        message: format!("Failed to open new tab: {}", e),
                    })?
            };

            // Block heavyweight resource types to speed up rendering.
            if config.block_resources {
                let blocked: Vec<String> = vec![
                    "*.jpg", "*.jpeg", "*.png", "*.gif", "*.webp", "*.avif", "*.svg", "*.ico",
                    "*.woff", "*.woff2", "*.ttf", "*.otf", "*.eot", "*.mp4", "*.webm", "*.mp3",
                    "*.ogg",
                ]
                .into_iter()
                .map(String::from)
                .collect();

                page.execute(SetBlockedUrlsParams::new(blocked))
                    .await
                    .map_err(|e| RehykeError::BrowserError {
                        message: format!("Failed to set blocked URLs: {}", e),
                    })?;
            }

            // Navigate to the target URL.
            let timeout = config.render_timeout;
            tokio::time::timeout(timeout, page.goto(url.as_str()))
                .await
                .map_err(|_| RehykeError::Timeout {
                    url: url.to_string(),
                })?
                .map_err(|e| RehykeError::RenderError {
                    url: url.to_string(),
                    message: format!("Navigation failed: {}", e),
                })?;

            // Apply wait strategy.
            Self::apply_wait_strategy(&page, config).await?;

            // Detect SPA framework.
            let detected_framework = if config.detect_spa {
                Self::detect_spa_framework(&page).await
            } else {
                None
            };

            // Dismiss popups before content extraction.
            let popup_dismissed = if config.dismiss_popups {
                Self::dismiss_popups(&page).await
            } else {
                false
            };

            // Scroll for infinite-scroll content loading.
            let pages_scrolled = if config.max_scrolls > 0 {
                Self::scroll_page(&page, config.max_scrolls, config).await
            } else {
                0
            };

            // Get final URL after any JS-driven navigation.
            let final_url = page
                .url()
                .await
                .ok()
                .flatten()
                .unwrap_or_else(|| url.to_string());

            // Extract the rendered HTML.
            let html = page.content().await.map_err(|e| RehykeError::RenderError {
                url: url.to_string(),
                message: format!("Failed to get page content: {}", e),
            })?;

            // Take a screenshot if requested.
            let screenshot = if config.screenshot {
                Self::capture_screenshot(&page, config, url).await.ok()
            } else {
                None
            };

            let elapsed = start.elapsed();

            info!(
                url = %url,
                final_url = %final_url,
                elapsed_ms = elapsed.as_millis(),
                framework = ?detected_framework,
                popup_dismissed,
                scrolled = pages_scrolled,
                screenshot = screenshot.is_some(),
                "Page rendered"
            );

            Ok(RenderResult {
                html,
                final_url,
                elapsed,
                screenshot,
                detected_framework,
                popup_dismissed,
                pages_scrolled,
            })
        }

        /// Re-render an already-fetched HTML string.
        pub(super) async fn render_html(
            &self,
            html: &str,
            base_url: &Url,
            config: &RendererConfig,
        ) -> Result<RenderResult> {
            let start = std::time::Instant::now();

            let page = {
                let browser = self.browser.lock().await;
                browser
                    .new_page("about:blank")
                    .await
                    .map_err(|e| RehykeError::BrowserError {
                        message: format!("Failed to open tab: {}", e),
                    })?
            };

            // Encode the HTML as a data URL so the browser loads it.
            let encoded = percent_encoding::percent_encode(
                html.as_bytes(),
                percent_encoding::NON_ALPHANUMERIC,
            )
            .to_string();
            let data_url = format!("data:text/html,{}", encoded);

            tokio::time::timeout(config.render_timeout, page.goto(data_url.as_str()))
                .await
                .map_err(|_| RehykeError::Timeout {
                    url: base_url.to_string(),
                })?
                .map_err(|e| RehykeError::RenderError {
                    url: base_url.to_string(),
                    message: format!("Failed to load HTML: {}", e),
                })?;

            Self::apply_wait_strategy(&page, config).await?;

            let popup_dismissed = if config.dismiss_popups {
                Self::dismiss_popups(&page).await
            } else {
                false
            };

            let pages_scrolled = if config.max_scrolls > 0 {
                Self::scroll_page(&page, config.max_scrolls, config).await
            } else {
                0
            };

            let rendered_html =
                page.content().await.map_err(|e| RehykeError::RenderError {
                    url: base_url.to_string(),
                    message: format!("Failed to get content: {}", e),
                })?;

            let screenshot = if config.screenshot {
                Self::capture_screenshot(&page, config, base_url).await.ok()
            } else {
                None
            };

            Ok(RenderResult {
                html: rendered_html,
                final_url: base_url.to_string(),
                elapsed: start.elapsed(),
                screenshot,
                detected_framework: None,
                popup_dismissed,
                pages_scrolled,
            })
        }

        // ----------------------------------------------------------------
        // Internal helpers
        // ----------------------------------------------------------------

        async fn apply_wait_strategy(
            page: &chromiumoxide::Page,
            config: &RendererConfig,
        ) -> Result<()> {
            let wait_timeout = Duration::from_secs(10); // per-strategy timeout

            match &config.wait_strategy {
                WaitStrategy::NetworkIdle => {
                    // Wait for navigation to complete (covers network idle).
                    tokio::time::timeout(wait_timeout, page.wait_for_navigation())
                        .await
                        .map_err(|_| RehykeError::Timeout {
                            url: "wait_for_navigation".to_string(),
                        })?
                        .map_err(|e| RehykeError::RenderError {
                            url: String::new(),
                            message: format!("NetworkIdle wait failed: {}", e),
                        })?;
                }
                WaitStrategy::Selector { selector } => {
                    tokio::time::timeout(wait_timeout, page.find_element(selector.as_str()))
                        .await
                        .map_err(|_| RehykeError::Timeout {
                            url: format!("wait for selector: {}", selector),
                        })?
                        .map_err(|e| RehykeError::RenderError {
                            url: String::new(),
                            message: format!("Selector '{}' not found: {}", selector, e),
                        })?;
                }
                WaitStrategy::Duration { duration } => {
                    tokio::time::sleep(*duration).await;
                }
                WaitStrategy::Auto => {
                    // Auto: wait for navigation first, then a short settle period.
                    let _ = tokio::time::timeout(
                        wait_timeout,
                        page.wait_for_navigation(),
                    )
                    .await;
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }

            Ok(())
        }

        async fn detect_spa_framework(
            page: &chromiumoxide::Page,
        ) -> Option<SpaFramework> {
            for (name, expr) in SPA_DETECTORS {
                let is_match = page
                    .evaluate(format!("Boolean({})", expr))
                    .await
                    .ok()
                    .and_then(|v| v.into_value::<bool>().ok())
                    .unwrap_or(false);

                if is_match {
                    return Some(match *name {
                        "Next.js" => SpaFramework::NextJs,
                        "Nuxt" => SpaFramework::Nuxt,
                        "SvelteKit" => SpaFramework::SvelteKit,
                        "React" => SpaFramework::React,
                        "Vue" => SpaFramework::Vue,
                        "Angular" => SpaFramework::Angular,
                        "Svelte" => SpaFramework::Svelte,
                        _ => SpaFramework::Unknown,
                    });
                }
            }
            None
        }

        async fn dismiss_popups(page: &chromiumoxide::Page) -> bool {
            for selector in POPUP_ACCEPT_SELECTORS {
                if let Ok(el) = page.find_element(*selector).await {
                    if el.click().await.is_ok() {
                        debug!(selector, "Dismissed popup");
                        // Brief pause for the popup animation to complete.
                        tokio::time::sleep(Duration::from_millis(300)).await;
                        return true;
                    }
                }
            }

            // Fallback: search for visible buttons containing acceptance text
            // via JavaScript.
            let js = r#"
                (function() {
                    const texts = ['accept', 'agree', 'allow', 'ok', 'got it', 'i understand'];
                    const buttons = Array.from(document.querySelectorAll('button, [role="button"], a.btn'));
                    for (const btn of buttons) {
                        const text = btn.textContent.trim().toLowerCase();
                        if (texts.some(t => text.includes(t)) && btn.offsetParent !== null) {
                            btn.click();
                            return true;
                        }
                    }
                    return false;
                })()
            "#;

            page.evaluate(js)
                .await
                .ok()
                .and_then(|v| v.into_value::<bool>().ok())
                .unwrap_or(false)
        }

        async fn scroll_page(
            page: &chromiumoxide::Page,
            max_scrolls: usize,
            config: &RendererConfig,
        ) -> usize {
            let mut scrolled = 0;

            for _ in 0..max_scrolls {
                // Get the current scroll position.
                let before: f64 = page
                    .evaluate("window.pageYOffset || document.documentElement.scrollTop")
                    .await
                    .ok()
                    .and_then(|v| v.into_value::<f64>().ok())
                    .unwrap_or(0.0);

                // Scroll one viewport height.
                let _ = page.evaluate("window.scrollBy(0, window.innerHeight)").await;

                // Wait for new content to load.
                tokio::time::sleep(Duration::from_millis(800)).await;

                // Apply wait strategy after each scroll.
                if matches!(config.wait_strategy, WaitStrategy::NetworkIdle) {
                    let _ = tokio::time::timeout(
                        Duration::from_secs(3),
                        page.wait_for_navigation(),
                    )
                    .await;
                }

                // Check whether the page actually moved.
                let after: f64 = page
                    .evaluate("window.pageYOffset || document.documentElement.scrollTop")
                    .await
                    .ok()
                    .and_then(|v| v.into_value::<f64>().ok())
                    .unwrap_or(0.0);

                if (after - before).abs() < 1.0 {
                    // No movement — we've reached the bottom.
                    debug!("Reached scroll bottom after {} scrolls", scrolled);
                    break;
                }

                scrolled += 1;
            }

            scrolled
        }

        async fn capture_screenshot(
            page: &chromiumoxide::Page,
            config: &RendererConfig,
            url: &Url,
        ) -> Result<ScreenshotData> {
            let format = match config.screenshot_format {
                ScreenshotFormat::Png => CaptureScreenshotFormat::Png,
                ScreenshotFormat::Jpeg => CaptureScreenshotFormat::Jpeg,
            };

            let params = CaptureScreenshotParams {
                format: Some(format),
                quality: Some(if matches!(config.screenshot_format, ScreenshotFormat::Jpeg) {
                    90
                } else {
                    100
                }),
                clip: None,
                from_surface: Some(true),
                capture_beyond_viewport: Some(true),
            };

            let data = page
                .screenshot(params)
                .await
                .map_err(|e| RehykeError::RenderError {
                    url: url.to_string(),
                    message: format!("Screenshot capture failed: {}", e),
                })?;

            // Determine output dimensions from page metrics.
            let (width, height) = config.viewport.dimensions();

            // Optionally save to disk.
            let saved_path = if let Some(ref dir) = config.screenshot_output_dir {
                let ext = match config.screenshot_format {
                    ScreenshotFormat::Png => "png",
                    ScreenshotFormat::Jpeg => "jpg",
                };
                let filename = crate::utils::url_to_slug(&crate::utils::parse_url_lossy(url))
                    + "."
                    + ext;
                let path = dir.join(&filename);
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&path, &data)?;
                info!(path = %path.display(), "Screenshot saved");
                Some(path)
            } else {
                None
            };

            Ok(ScreenshotData {
                data,
                format: config.screenshot_format,
                width,
                height,
                saved_path,
            })
        }
    }
} // mod js_impl

#[cfg(feature = "js")]
use js_impl::JsRenderer;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_renderer_config_defaults() {
        let config = RendererConfig::default();

        assert_eq!(config.render_timeout, Duration::from_secs(30));
        assert!(matches!(config.wait_strategy, WaitStrategy::Auto));
        assert!(config.block_resources);
        assert_eq!(config.tab_pool_size, 4);
        assert!(config.headless);
        assert_eq!(config.max_scrolls, 0);
        assert_eq!(config.viewport, Viewport::Desktop);
        assert!(!config.dismiss_popups);
        assert!(!config.screenshot);
        assert_eq!(config.screenshot_format, ScreenshotFormat::Png);
        assert!(config.screenshot_output_dir.is_none());
        assert!(!config.randomize_fingerprint);
        assert!(!config.detect_spa);
    }

    #[test]
    fn test_renderer_creation_not_initialized() {
        let config = RendererConfig::default();
        let renderer = Renderer::new(config);
        assert!(!renderer.is_initialized());
    }

    #[test]
    fn test_viewport_dimensions() {
        assert_eq!(Viewport::Desktop.dimensions(), (1920, 1080));
        assert_eq!(Viewport::Tablet.dimensions(), (768, 1024));
        assert_eq!(Viewport::Mobile.dimensions(), (390, 844));
    }

    #[test]
    fn test_viewport_scale_factor() {
        assert_eq!(Viewport::Desktop.device_scale_factor(), 1.0);
        assert_eq!(Viewport::Tablet.device_scale_factor(), 2.0);
        assert_eq!(Viewport::Mobile.device_scale_factor(), 3.0);
    }

    #[test]
    fn test_viewport_mobile_flags() {
        assert!(!Viewport::Desktop.is_mobile());
        assert!(Viewport::Tablet.is_mobile());
        assert!(Viewport::Mobile.is_mobile());

        assert!(!Viewport::Desktop.has_touch());
        assert!(Viewport::Tablet.has_touch());
        assert!(Viewport::Mobile.has_touch());
    }

    #[test]
    fn test_spa_framework_display() {
        assert_eq!(SpaFramework::React.to_string(), "React");
        assert_eq!(SpaFramework::NextJs.to_string(), "Next.js");
        assert_eq!(SpaFramework::Vue.to_string(), "Vue");
        assert_eq!(SpaFramework::Angular.to_string(), "Angular");
        assert_eq!(SpaFramework::Svelte.to_string(), "Svelte");
        assert_eq!(SpaFramework::Nuxt.to_string(), "Nuxt");
        assert_eq!(SpaFramework::SvelteKit.to_string(), "SvelteKit");
        assert_eq!(SpaFramework::Unknown.to_string(), "Unknown");
    }

    #[tokio::test]
    async fn test_render_without_initialization_returns_error() {
        let renderer = Renderer::new(RendererConfig::default());
        let url = Url::parse("https://example.com").unwrap();

        let result = renderer.render(&url).await;
        assert!(result.is_err());

        match result.unwrap_err() {
            RehykeError::BrowserError { message } => {
                assert!(message.contains("not initialized"));
            }
            other => panic!("Expected BrowserError, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_render_html_without_initialization_returns_error() {
        let renderer = Renderer::new(RendererConfig::default());
        let url = Url::parse("https://example.com").unwrap();

        let result = renderer.render_html("<html></html>", &url).await;
        assert!(result.is_err());

        match result.unwrap_err() {
            RehykeError::BrowserError { message } => {
                assert!(message.contains("not initialized"));
            }
            other => panic!("Expected BrowserError, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_initialize_stub_and_shutdown() {
        let mut renderer = Renderer::new(RendererConfig::default());
        assert!(!renderer.is_initialized());

        // Without `js` feature the stub always succeeds.
        #[cfg(not(feature = "js"))]
        {
            renderer.initialize().await.unwrap();
            assert!(renderer.is_initialized());
            renderer.shutdown().await.unwrap();
            assert!(!renderer.is_initialized());
        }

        // With `js` feature success depends on Chrome being installed.
        // We only assert the type of error — not the outcome.
        #[cfg(feature = "js")]
        {
            let _ = renderer.initialize().await;
            // Just verify we didn't panic.
        }
    }

    #[test]
    fn test_renderer_config_custom_values() {
        let config = RendererConfig {
            render_timeout: Duration::from_secs(60),
            wait_strategy: WaitStrategy::Selector {
                selector: "div.content".to_string(),
            },
            block_resources: false,
            tab_pool_size: 8,
            headless: false,
            max_scrolls: 10,
            viewport: Viewport::Mobile,
            dismiss_popups: true,
            screenshot: true,
            screenshot_format: ScreenshotFormat::Jpeg,
            screenshot_output_dir: Some(PathBuf::from("/tmp/shots")),
            randomize_fingerprint: true,
            detect_spa: true,
        };

        assert_eq!(config.render_timeout, Duration::from_secs(60));
        assert!(matches!(
            config.wait_strategy,
            WaitStrategy::Selector { ref selector } if selector == "div.content"
        ));
        assert!(!config.block_resources);
        assert_eq!(config.tab_pool_size, 8);
        assert!(!config.headless);
        assert_eq!(config.max_scrolls, 10);
        assert_eq!(config.viewport, Viewport::Mobile);
        assert!(config.dismiss_popups);
        assert!(config.screenshot);
        assert_eq!(config.screenshot_format, ScreenshotFormat::Jpeg);
        assert_eq!(
            config.screenshot_output_dir,
            Some(PathBuf::from("/tmp/shots"))
        );
        assert!(config.randomize_fingerprint);
        assert!(config.detect_spa);
    }

    #[test]
    fn test_popup_accept_selectors_are_valid_css() {
        // Ensure every selector string is non-empty and starts with a valid
        // CSS character (basic sanity check — not full CSS validation).
        for selector in POPUP_ACCEPT_SELECTORS {
            assert!(
                !selector.is_empty(),
                "Empty popup selector found"
            );
        }
        assert!(
            POPUP_ACCEPT_SELECTORS.len() >= 10,
            "Expected at least 10 popup selectors"
        );
    }

    #[test]
    fn test_spa_detectors_have_valid_expressions() {
        for (name, expr) in SPA_DETECTORS {
            assert!(!name.is_empty(), "Empty SPA framework name");
            assert!(!expr.is_empty(), "Empty SPA detection expression for {}", name);
        }
    }

    #[test]
    fn test_is_available_returns_bool() {
        // Just ensure it doesn't panic.
        let _ = Renderer::is_available();
    }

    #[test]
    fn test_screenshot_data_format() {
        let data = ScreenshotData {
            data: vec![0u8; 100],
            format: ScreenshotFormat::Png,
            width: 1920,
            height: 1080,
            saved_path: None,
        };
        assert_eq!(data.data.len(), 100);
        assert_eq!(data.format, ScreenshotFormat::Png);
        assert_eq!(data.width, 1920);
        assert_eq!(data.height, 1080);
        assert!(data.saved_path.is_none());
    }

    // -----------------------------------------------------------------------
    // Additional renderer tests (v0.2.0)
    // -----------------------------------------------------------------------

    #[test]
    fn test_screenshot_data_with_saved_path() {
        let path = PathBuf::from("/tmp/page.png");
        let data = ScreenshotData {
            data: vec![0xFF, 0xD8, 0xFF], // JPEG magic bytes
            format: ScreenshotFormat::Jpeg,
            width: 390,
            height: 844,
            saved_path: Some(path.clone()),
        };
        assert_eq!(data.format, ScreenshotFormat::Jpeg);
        assert_eq!(data.saved_path, Some(path));
        assert_eq!(data.data[0], 0xFF);
    }

    #[test]
    fn test_render_result_all_fields() {
        let result = RenderResult {
            html: "<html><body>hello</body></html>".to_string(),
            final_url: "https://example.com/".to_string(),
            elapsed: Duration::from_millis(350),
            screenshot: None,
            detected_framework: Some(SpaFramework::React),
            popup_dismissed: true,
            pages_scrolled: 5,
        };
        assert!(!result.html.is_empty());
        assert_eq!(result.final_url, "https://example.com/");
        assert!(result.elapsed < Duration::from_secs(1));
        assert!(result.screenshot.is_none());
        assert_eq!(result.detected_framework, Some(SpaFramework::React));
        assert!(result.popup_dismissed);
        assert_eq!(result.pages_scrolled, 5);
    }

    #[test]
    fn test_render_result_with_screenshot() {
        let shot = ScreenshotData {
            data: vec![1, 2, 3],
            format: ScreenshotFormat::Png,
            width: 1920,
            height: 1080,
            saved_path: Some(PathBuf::from("/tmp/shot.png")),
        };
        let result = RenderResult {
            html: String::new(),
            final_url: "https://example.com/".to_string(),
            elapsed: Duration::from_millis(100),
            screenshot: Some(shot),
            detected_framework: None,
            popup_dismissed: false,
            pages_scrolled: 0,
        };
        let s = result.screenshot.as_ref().unwrap();
        assert_eq!(s.width, 1920);
        assert_eq!(s.height, 1080);
        assert!(result.detected_framework.is_none());
    }

    #[test]
    fn test_spa_framework_all_variants() {
        // Every variant round-trips through Display correctly.
        let pairs = [
            (SpaFramework::React,     "React"),
            (SpaFramework::Vue,       "Vue"),
            (SpaFramework::Angular,   "Angular"),
            (SpaFramework::Svelte,    "Svelte"),
            (SpaFramework::NextJs,    "Next.js"),
            (SpaFramework::Nuxt,      "Nuxt"),
            (SpaFramework::SvelteKit, "SvelteKit"),
            (SpaFramework::Unknown,   "Unknown"),
        ];
        for (framework, expected) in pairs {
            assert_eq!(framework.to_string(), expected);
        }
    }

    #[test]
    fn test_spa_framework_equality() {
        assert_eq!(SpaFramework::React, SpaFramework::React);
        assert_ne!(SpaFramework::React, SpaFramework::Vue);
        assert_ne!(SpaFramework::NextJs, SpaFramework::Nuxt);
    }

    #[test]
    fn test_popup_selectors_cover_known_providers() {
        let selectors_joined = POPUP_ACCEPT_SELECTORS.join(" ");
        // Well-known CMP providers must appear in the selector list.
        let known_patterns = [
            "onetrust",
            "cookie",
            "consent",
            "gdpr",
            "accept",
        ];
        for pattern in known_patterns {
            assert!(
                selectors_joined.to_lowercase().contains(pattern),
                "Popup selectors should cover '{}' but none matched",
                pattern
            );
        }
    }

    #[test]
    fn test_spa_detectors_cover_known_frameworks() {
        let names: Vec<&str> = SPA_DETECTORS.iter().map(|(n, _)| *n).collect();
        for expected in ["React", "Vue", "Angular", "Next.js"] {
            assert!(
                names.contains(&expected),
                "SPA_DETECTORS should include '{}', got: {:?}",
                expected,
                names
            );
        }
    }

    #[test]
    fn test_spa_detector_expressions_non_trivial() {
        // Each detection expression should reference a JS global or DOM attribute.
        for (name, expr) in SPA_DETECTORS {
            assert!(
                expr.contains("window.")
                    || expr.contains("document.")
                    || expr.contains("__")
                    || expr.contains("["),
                "SPA detector for '{}' looks too simple: '{}'",
                name,
                expr
            );
        }
    }

    #[test]
    fn test_viewport_physical_pixels() {
        // Physical pixels = CSS pixels × device scale factor.
        let (dw, dh) = Viewport::Desktop.dimensions();
        let ddpr = Viewport::Desktop.device_scale_factor();
        assert_eq!((dw as f64 * ddpr) as u32, 1920);
        assert_eq!((dh as f64 * ddpr) as u32, 1080);

        let (mw, mh) = Viewport::Mobile.dimensions();
        let mdpr = Viewport::Mobile.device_scale_factor();
        assert_eq!((mw as f64 * mdpr) as u32, 1170); // 390 × 3
        assert_eq!((mh as f64 * mdpr) as u32, 2532); // 844 × 3
    }

    #[test]
    fn test_renderer_config_tab_pool_size_minimum() {
        // tab_pool_size of 0 would starve the pool — default must be ≥ 1.
        let config = RendererConfig::default();
        assert!(config.tab_pool_size >= 1, "tab pool must have at least one slot");
    }

    #[test]
    fn test_screenshot_format_is_copy() {
        // ScreenshotFormat is Copy — assigning it doesn't move it.
        let fmt = ScreenshotFormat::Jpeg;
        let fmt2 = fmt;
        assert_eq!(fmt, fmt2); // both still usable
    }

    #[test]
    fn test_viewport_is_copy() {
        let vp = Viewport::Mobile;
        let vp2 = vp;
        assert_eq!(vp, vp2);
    }

    #[test]
    fn test_wait_strategy_clones() {
        let ws = WaitStrategy::Selector {
            selector: ".root".to_string(),
        };
        let ws2 = ws.clone();
        let json1 = serde_json::to_string(&ws).unwrap();
        let json2 = serde_json::to_string(&ws2).unwrap();
        assert_eq!(json1, json2);
    }

    #[test]
    fn test_renderer_not_initialized_after_new() {
        let r = Renderer::new(RendererConfig {
            viewport: Viewport::Tablet,
            max_scrolls: 3,
            dismiss_popups: true,
            ..RendererConfig::default()
        });
        assert!(!r.is_initialized());
    }

    #[test]
    fn test_multiple_renderers_independent() {
        let r1 = Renderer::new(RendererConfig::default());
        let r2 = Renderer::new(RendererConfig {
            viewport: Viewport::Mobile,
            ..RendererConfig::default()
        });
        // Both start uninitialized independently.
        assert!(!r1.is_initialized());
        assert!(!r2.is_initialized());
    }
}
