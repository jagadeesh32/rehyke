use crate::config::WaitStrategy;
use crate::error::{RehykeError, Result};
use std::time::Duration;
use tracing::{debug, info, warn};
use url::Url;

/// Configuration for the renderer
#[derive(Debug, Clone)]
pub struct RendererConfig {
    /// Timeout for rendering a page
    pub render_timeout: Duration,
    /// Wait strategy for page load
    pub wait_strategy: WaitStrategy,
    /// Block unnecessary resources (images, fonts, media) for speed
    pub block_resources: bool,
    /// Number of browser tabs to pool
    pub tab_pool_size: usize,
    /// Whether to run in headless mode
    pub headless: bool,
    /// Maximum number of scroll operations for infinite scroll pages
    pub max_scrolls: usize,
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
        }
    }
}

/// Result of rendering a page with JavaScript
#[derive(Debug, Clone)]
pub struct RenderResult {
    /// The rendered HTML after JavaScript execution
    pub html: String,
    /// The final URL (after any JS-driven navigation)
    pub final_url: String,
    /// Time taken to render
    pub elapsed: Duration,
}

/// Headless browser renderer for JavaScript-heavy pages
///
/// This renderer uses a headless Chromium browser to execute JavaScript
/// and extract the final rendered DOM.
pub struct Renderer {
    config: RendererConfig,
    initialized: bool,
}

impl Renderer {
    /// Create a new renderer with the given configuration
    pub fn new(config: RendererConfig) -> Self {
        Self {
            config,
            initialized: false,
        }
    }

    /// Initialize the browser (launch Chromium process)
    ///
    /// This is separate from `new()` because it is an async operation that can fail.
    pub async fn initialize(&mut self) -> Result<()> {
        // TODO: Launch chromiumoxide browser
        // For now, mark as initialized.
        // In production, this would:
        // 1. Find Chrome/Chromium binary
        // 2. Launch with headless flags
        // 3. Create tab pool
        info!("Renderer initialized (stub - chromium integration pending)");
        self.initialized = true;
        Ok(())
    }

    /// Render a page by navigating to the URL and executing JavaScript
    pub async fn render(&self, url: &Url) -> Result<RenderResult> {
        if !self.initialized {
            return Err(RehykeError::BrowserError {
                message: "Renderer not initialized. Call initialize() first.".to_string(),
            });
        }

        let start = std::time::Instant::now();
        debug!(
            url = %url,
            timeout = ?self.config.render_timeout,
            "Rendering page with JavaScript"
        );

        // TODO: Real chromiumoxide implementation
        // For now, return an error indicating JS rendering is not yet available.
        // This allows the crawler to fall back to static fetching.
        warn!(
            url = %url,
            elapsed = ?start.elapsed(),
            "JavaScript rendering not yet implemented, falling back to static fetch"
        );
        Err(RehykeError::RenderError {
            url: url.to_string(),
            message: "JavaScript rendering not yet implemented. Use static fetching.".to_string(),
        })
    }

    /// Render a page from already-fetched HTML (re-render with JS)
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

        Err(RehykeError::RenderError {
            url: base_url.to_string(),
            message: "JavaScript rendering not yet implemented.".to_string(),
        })
    }

    /// Check if the renderer is available (Chromium is installed)
    pub fn is_available() -> bool {
        // Check if Chrome/Chromium binary exists in common paths
        let common_paths = [
            "/usr/bin/chromium",
            "/usr/bin/chromium-browser",
            "/usr/bin/google-chrome",
            "/usr/bin/google-chrome-stable",
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
        ];

        common_paths.iter().any(|p| std::path::Path::new(p).exists())
    }

    /// Shutdown the browser
    pub async fn shutdown(&mut self) -> Result<()> {
        self.initialized = false;
        info!("Renderer shut down");
        Ok(())
    }

    /// Whether the renderer has been initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}

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
    }

    #[test]
    fn test_renderer_creation() {
        let config = RendererConfig::default();
        let renderer = Renderer::new(config);

        assert!(!renderer.is_initialized());
    }

    #[test]
    fn test_is_initialized_before_init() {
        let renderer = Renderer::new(RendererConfig::default());
        assert!(!renderer.is_initialized());
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
    async fn test_initialize_and_shutdown() {
        let mut renderer = Renderer::new(RendererConfig::default());

        assert!(!renderer.is_initialized());

        renderer.initialize().await.unwrap();
        assert!(renderer.is_initialized());

        renderer.shutdown().await.unwrap();
        assert!(!renderer.is_initialized());
    }

    #[tokio::test]
    async fn test_render_after_initialization_returns_render_error() {
        let mut renderer = Renderer::new(RendererConfig::default());
        renderer.initialize().await.unwrap();

        let url = Url::parse("https://example.com").unwrap();
        let result = renderer.render(&url).await;
        assert!(result.is_err());

        match result.unwrap_err() {
            RehykeError::RenderError { url, message } => {
                assert_eq!(url, "https://example.com/");
                assert!(message.contains("not yet implemented"));
            }
            other => panic!("Expected RenderError, got: {:?}", other),
        }
    }

    #[test]
    fn test_renderer_config_custom_values() {
        let config = RendererConfig {
            render_timeout: Duration::from_secs(60),
            wait_strategy: WaitStrategy::Selector { selector: "div.content".to_string() },
            block_resources: false,
            tab_pool_size: 8,
            headless: false,
            max_scrolls: 10,
        };

        assert_eq!(config.render_timeout, Duration::from_secs(60));
        assert!(matches!(config.wait_strategy, WaitStrategy::Selector { ref selector } if selector == "div.content"));
        assert!(!config.block_resources);
        assert_eq!(config.tab_pool_size, 8);
        assert!(!config.headless);
        assert_eq!(config.max_scrolls, 10);
    }
}
