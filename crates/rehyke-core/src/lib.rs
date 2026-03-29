pub mod anti_detect;
pub mod browser_fingerprint;
pub mod config;
pub mod converter;
pub mod error;
pub mod extractor;
pub mod fetcher;
pub mod output;
pub mod parser;
pub mod proxy;
pub mod renderer;
pub mod robots;
pub mod scheduler;
pub mod sitemap;
pub mod utils;

// Re-export key types for convenience.
pub use config::*;
pub use error::*;
pub use output::CrawlResult;

use chrono::Utc;
use regex::Regex;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::time::sleep;
use tracing::{debug, info, warn};
use url::Url;

use crate::anti_detect::AntiDetect;
use crate::browser_fingerprint::{BrowserFingerprint, FingerprintProfile};
use crate::converter::ConverterConfig;
use crate::extractor::ExtractedLinks;
use crate::fetcher::{ContentType, Fetcher};
use crate::output::{OutputHandler, RenderMethod};
use crate::parser::ParseConfig;
use crate::renderer::{Renderer, RendererConfig};
use crate::robots::RobotsTxt;
use crate::scheduler::{Scheduler, TaskSource};

/// Main orchestrator for the Rehyke web crawler.
///
/// Provides both a simple one-shot API ([`Rehyke::crawl`]) and a configurable
/// API ([`Rehyke::run`]) for performing web crawls.
///
/// # Examples
///
/// ## One-shot crawl (simplest)
///
/// ```rust,no_run
/// use rehyke_core::{Rehyke, ScanMode};
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let results = Rehyke::crawl("https://example.com", ScanMode::Lite).await?;
///     for page in &results {
///         println!("{}: {} words", page.title, page.markdown.split_whitespace().count());
///     }
///     Ok(())
/// }
/// ```
///
/// ## Configured crawl
///
/// ```rust,no_run
/// use rehyke_core::{CrawlConfigBuilder, Rehyke, ScanMode};
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let config = CrawlConfigBuilder::new()
///         .mode(ScanMode::Full)
///         .max_pages(500)
///         .clean_ads(true)
///         .clean_navigation(true)
///         .exclude_patterns(vec![r"\.pdf$".into(), r"/login".into()])
///         .build();
///
///     let results = Rehyke::new(config).run("https://example.com").await?;
///     println!("Crawled {} pages", results.len());
///     Ok(())
/// }
/// ```
///
/// ## JavaScript-rendered SPA (v0.2.0)
///
/// ```rust,no_run
/// use rehyke_core::{CrawlConfigBuilder, Rehyke, ScanMode, Viewport, WaitStrategy};
/// use std::time::Duration;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let config = CrawlConfigBuilder::new()
///         .mode(ScanMode::Full)
///         .enable_js(true)
///         .js_wait_strategy(WaitStrategy::NetworkIdle)
///         .js_wait_timeout(Duration::from_secs(12))
///         .js_scroll_count(8)
///         .dismiss_popups(true)
///         .detect_spa(true)
///         .viewport(Viewport::Desktop)
///         .randomize_fingerprint(true)
///         .build();
///
///     let results = Rehyke::new(config).run("https://my-react-app.com").await?;
///     for page in &results {
///         println!("[{:?}] {}", page.render_method, page.title);
///     }
///     Ok(())
/// }
/// ```
pub struct Rehyke {
    config: CrawlConfig,
}

impl Rehyke {
    /// Create a new crawler with the given configuration.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use rehyke_core::{CrawlConfigBuilder, Rehyke, ScanMode};
    ///
    /// let config = CrawlConfigBuilder::new().mode(ScanMode::Lite).build();
    /// let crawler = Rehyke::new(config);
    /// ```
    pub fn new(config: CrawlConfig) -> Self {
        Self { config }
    }

    /// Simple one-shot crawl API.
    ///
    /// Creates a default [`CrawlConfig`] with the given [`ScanMode`], fetches
    /// the URL, parses the content, converts it to markdown, and returns the
    /// results.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use rehyke_core::{Rehyke, ScanMode};
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     // Lite = single page, no link following
    ///     let pages = Rehyke::crawl("https://example.com", ScanMode::Lite).await?;
    ///     println!("{}", pages[0].markdown);
    ///     Ok(())
    /// }
    /// ```
    pub async fn crawl(url: &str, mode: ScanMode) -> Result<Vec<CrawlResult>> {
        let config = CrawlConfigBuilder::new().mode(mode).build();
        let crawler = Self::new(config);
        crawler.run(url).await
    }

    /// Full crawl API using the configured options.
    ///
    /// Implements the complete multi-page crawl pipeline:
    /// 1. Validate the seed URL
    /// 2. Optionally fetch and parse robots.txt
    /// 3. Add the seed to the scheduler
    /// 4. BFS crawl loop: pop URL → check robots → delay → fetch/render →
    ///    parse → extract links → feed links back to scheduler → repeat
    /// 5. Collect and return all results
    pub async fn run(&self, url: &str) -> Result<Vec<CrawlResult>> {
        let parsed_url = Url::parse(url).map_err(|e| RehykeError::ConfigError {
            message: format!("invalid seed URL '{}': {}", url, e),
        })?;

        info!(url = %parsed_url, mode = ?self.config.mode, "starting crawl");

        // Build components.
        let fetcher = Arc::new(Fetcher::new(&self.config)?);
        let scheduler = Arc::new(Scheduler::new(&self.config, self.config.mode));
        let anti_detect = AntiDetect::new(self.config.delay_strategy.clone());
        let mut output_handler = OutputHandler::new(self.config.output.clone());

        // Compile include/exclude regex patterns.
        let include_patterns = compile_patterns(&self.config.include_patterns)?;
        let exclude_patterns = compile_patterns(&self.config.exclude_patterns)?;

        // --- v0.2.0: Initialize headless browser renderer when JS is enabled ---
        let mut renderer: Option<Renderer> = if self.config.enable_js {
            let renderer_config = RendererConfig {
                render_timeout: self.config.js_wait_timeout,
                wait_strategy: self.config.js_wait_strategy.clone(),
                block_resources: true,
                tab_pool_size: (self.config.concurrency / 4).max(2),
                headless: true,
                max_scrolls: self.config.js_scroll_count,
                viewport: self.config.viewport,
                dismiss_popups: self.config.dismiss_popups,
                screenshot: self.config.screenshot,
                screenshot_format: self.config.screenshot_format,
                screenshot_output_dir: self.config.screenshot_output_dir.clone(),
                randomize_fingerprint: self.config.randomize_fingerprint,
                detect_spa: self.config.detect_spa,
            };
            let mut r = Renderer::new(renderer_config);

            if Renderer::is_available() {
                match r.initialize().await {
                    Ok(()) => {
                        info!("JS renderer ready");
                        Some(r)
                    }
                    Err(e) => {
                        warn!(error = %e, "JS renderer unavailable — falling back to static fetch");
                        None
                    }
                }
            } else {
                // Try to initialise anyway (the js feature stub always succeeds).
                let _ = r.initialize().await;
                if r.is_initialized() {
                    Some(r)
                } else {
                    warn!("Chrome not found — JS rendering disabled (falling back to static fetch)");
                    None
                }
            }
        } else {
            None
        };

        // Log browser fingerprint info when randomisation is requested.
        if self.config.randomize_fingerprint && self.config.enable_js {
            let fp = BrowserFingerprint::randomize(FingerprintProfile::from(self.config.viewport));
            debug!(
                ua = %fp.user_agent,
                viewport = format!("{}x{}", fp.viewport_width, fp.viewport_height),
                timezone = %fp.timezone,
                "Browser fingerprint randomised"
            );
        }

        // Fetch robots.txt if configured.
        let robots = if self.config.respect_robots_txt {
            fetch_robots_txt(&fetcher, &parsed_url).await
        } else {
            None
        };

        // Add seed URL to the scheduler.
        scheduler.add_seed(parsed_url.clone());

        // Concurrency limiter.
        let semaphore = Arc::new(Semaphore::new(self.config.concurrency));

        // Parse config shared across iterations.
        let parse_config = ParseConfig {
            clean_navigation: self.config.clean_navigation,
            clean_footers: self.config.clean_footers,
            clean_ads: self.config.clean_ads,
            clean_comments: true,
            extract_metadata: self.config.extract_metadata,
        };

        let converter_config = ConverterConfig {
            include_frontmatter: true,
            include_footer: false,
            max_blank_lines: 2,
        };

        // Main crawl loop.
        loop {
            if scheduler.is_done() {
                break;
            }

            // Try to get the next task.
            let task = match scheduler.next_task() {
                Some(t) => t,
                None => {
                    if !scheduler.is_done() {
                        sleep(std::time::Duration::from_millis(50)).await;
                        continue;
                    }
                    break;
                }
            };

            // Check robots.txt before fetching.
            if let Some(ref robots_txt) = robots {
                let path = task.url.path();
                if !robots_txt.is_allowed(path) {
                    debug!(url = %task.url, path = %path, "blocked by robots.txt");
                    scheduler.mark_completed(&task.url);
                    continue;
                }
            }

            // Check include/exclude patterns.
            let url_str = task.url.as_str();
            if !include_patterns.is_empty()
                && !include_patterns.iter().any(|p| p.is_match(url_str))
            {
                debug!(url = %task.url, "skipped: does not match include patterns");
                scheduler.mark_completed(&task.url);
                continue;
            }
            if exclude_patterns.iter().any(|p| p.is_match(url_str)) {
                debug!(url = %task.url, "skipped: matches exclude pattern");
                scheduler.mark_completed(&task.url);
                continue;
            }

            // Apply delay before fetching.
            let delay = anti_detect.get_delay();
            if !delay.is_zero() {
                sleep(delay).await;
            }

            // Acquire concurrency permit.
            let _permit = semaphore.acquire().await.map_err(|_| RehykeError::ConfigError {
                message: "concurrency semaphore closed unexpectedly".to_string(),
            })?;

            // ---------------------------------------------------------------
            // v0.2.0: JS rendering path
            // ---------------------------------------------------------------
            let (html_body, final_url, status_code, content_type_str, render_method) =
                if let Some(ref renderer) = renderer {
                    debug!(url = %task.url, depth = task.depth, "rendering page with JS");

                    match renderer.render(&task.url).await {
                        Ok(render_result) => {
                            anti_detect.record_response(200, render_result.elapsed);

                            if let Some(framework) = &render_result.detected_framework {
                                debug!(url = %task.url, framework = %framework, "SPA detected");
                            }
                            if render_result.popup_dismissed {
                                debug!(url = %task.url, "popup dismissed");
                            }
                            if render_result.pages_scrolled > 0 {
                                debug!(
                                    url = %task.url,
                                    scrolled = render_result.pages_scrolled,
                                    "scrolled for infinite-scroll content"
                                );
                            }

                            let rendered_url = Url::parse(&render_result.final_url)
                                .unwrap_or_else(|_| task.url.clone());
                            (
                                render_result.html,
                                rendered_url,
                                200u16,
                                "text/html".to_string(),
                                RenderMethod::JavaScript,
                            )
                        }
                        Err(e) => {
                            // Fall back to static fetch on render failure.
                            warn!(url = %task.url, error = %e, "JS render failed, falling back to static");
                            match fetcher.fetch_with_retry(&task.url).await {
                                Ok(r) => {
                                    anti_detect.record_response(r.status, r.elapsed);
                                    let ct = r
                                        .headers
                                        .get(reqwest::header::CONTENT_TYPE)
                                        .and_then(|v| v.to_str().ok())
                                        .unwrap_or("text/html")
                                        .to_string();
                                    (r.body, r.final_url, r.status, ct, RenderMethod::Static)
                                }
                                Err(fe) => {
                                    warn!(url = %task.url, error = %fe, "fetch failed");
                                    scheduler.mark_failed(&task.url);
                                    continue;
                                }
                            }
                        }
                    }
                } else {
                    // -------------------------------------------------------
                    // Static fetch path (no renderer or enable_js=false)
                    // -------------------------------------------------------
                    debug!(url = %task.url, depth = task.depth, "fetching page");
                    match fetcher.fetch_with_retry(&task.url).await {
                        Ok(r) => {
                            anti_detect.record_response(r.status, r.elapsed);
                            let ct = r
                                .headers
                                .get(reqwest::header::CONTENT_TYPE)
                                .and_then(|v| v.to_str().ok())
                                .unwrap_or("text/html")
                                .to_string();
                            (r.body, r.final_url, r.status, ct, RenderMethod::Static)
                        }
                        Err(e) => {
                            warn!(url = %task.url, error = %e, "fetch failed");
                            scheduler.mark_failed(&task.url);
                            continue;
                        }
                    }
                };

            // Determine content type for parsing.
            let content_type = fetcher::detect_content_type_from_str(&content_type_str, &final_url);

            // Parse the content.
            let parsed = match parser::parse(&html_body, &content_type, &parse_config) {
                Ok(p) => p,
                Err(e) => {
                    warn!(url = %task.url, error = %e, "parse failed");
                    scheduler.mark_failed(&task.url);
                    continue;
                }
            };

            // Convert to markdown.
            let markdown = converter::to_markdown_with_url(
                &parsed,
                final_url.as_str(),
                &converter_config,
            );

            let title = parsed
                .metadata
                .title
                .clone()
                .unwrap_or_else(|| "Untitled".to_string());

            // Extract links and feed them back to the scheduler.
            let links = if matches!(content_type, ContentType::Html | ContentType::Xhtml) {
                let html = scraper::Html::parse_document(&html_body);
                let extracted = extractor::extract_links(&html, &final_url);

                let internal_urls: Vec<Url> = extracted
                    .internal
                    .iter()
                    .filter_map(|u| Url::parse(u).ok())
                    .collect();
                if !internal_urls.is_empty() {
                    scheduler.add_urls(
                        internal_urls,
                        task.depth + 1,
                        TaskSource::InternalLink,
                    );
                }

                let external_urls: Vec<Url> = extracted
                    .external
                    .iter()
                    .filter_map(|u| Url::parse(u).ok())
                    .collect();
                if !external_urls.is_empty() {
                    scheduler.add_urls(
                        external_urls,
                        task.depth + 1,
                        TaskSource::ExternalLink,
                    );
                }

                let subdomain_urls: Vec<Url> = extracted
                    .subdomains
                    .iter()
                    .filter_map(|u| Url::parse(u).ok())
                    .collect();
                if !subdomain_urls.is_empty() {
                    scheduler.add_urls(
                        subdomain_urls,
                        task.depth + 1,
                        TaskSource::InternalLink,
                    );
                }

                extracted
            } else {
                ExtractedLinks::default()
            };

            // Build result.
            let result = CrawlResult {
                url: final_url.to_string(),
                title,
                markdown,
                metadata: parsed.metadata,
                links,
                crawled_at: Utc::now(),
                status_code,
                content_type: content_type_str.to_string(),
                depth: task.depth,
                render_method,
            };

            info!(
                url = %result.url,
                title = %result.title,
                status = result.status_code,
                depth = result.depth,
                markdown_len = result.markdown.len(),
                "page crawled"
            );

            output_handler.handle_result(result)?;
            scheduler.mark_completed(&task.url);
        }

        // Shut down the renderer cleanly.
        if let Some(ref mut r) = renderer {
            let _ = r.shutdown().await;
        }

        let stats = scheduler.stats.snapshot();
        info!(
            discovered = stats.total_discovered,
            crawled = stats.total_crawled,
            errors = stats.total_errors,
            skipped = stats.total_skipped,
            "crawl finished"
        );

        output_handler.finalize()
    }
}

/// Attempt to fetch and parse robots.txt for the seed URL's domain.
async fn fetch_robots_txt(fetcher: &Fetcher, seed_url: &Url) -> Option<RobotsTxt> {
    let robots_url_str = format!(
        "{}://{}/robots.txt",
        seed_url.scheme(),
        seed_url.host_str().unwrap_or(""),
    );
    let robots_url = match Url::parse(&robots_url_str) {
        Ok(u) => u,
        Err(_) => return None,
    };

    debug!(url = %robots_url, "fetching robots.txt");

    match fetcher.fetch_with_retry(&robots_url).await {
        Ok(result) if result.status == 200 => {
            let parsed = RobotsTxt::parse(&result.body);
            info!(
                url = %robots_url,
                sitemaps = parsed.sitemaps().len(),
                crawl_delay = ?parsed.crawl_delay(),
                "parsed robots.txt"
            );
            Some(parsed)
        }
        Ok(result) => {
            debug!(
                url = %robots_url,
                status = result.status,
                "robots.txt not found or not accessible, proceeding without"
            );
            None
        }
        Err(e) => {
            debug!(
                url = %robots_url,
                error = %e,
                "failed to fetch robots.txt, proceeding without"
            );
            None
        }
    }
}

/// Compile a list of regex pattern strings into compiled [`Regex`] objects.
fn compile_patterns(patterns: &[String]) -> Result<Vec<Regex>> {
    patterns
        .iter()
        .map(|p| {
            Regex::new(p).map_err(|e| RehykeError::ConfigError {
                message: format!("invalid regex pattern '{}': {}", p, e),
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // -----------------------------------------------------------------------
    // Rehyke::new — stores config correctly
    // -----------------------------------------------------------------------

    #[test]
    fn new_stores_config_mode() {
        let cfg = CrawlConfigBuilder::new().mode(ScanMode::Lite).build();
        let crawler = Rehyke::new(cfg);
        assert_eq!(crawler.config.mode, ScanMode::Lite);
    }

    #[test]
    fn new_stores_v020_js_fields() {
        let cfg = CrawlConfigBuilder::new()
            .enable_js(true)
            .js_scroll_count(7)
            .dismiss_popups(true)
            .viewport(Viewport::Mobile)
            .detect_spa(true)
            .randomize_fingerprint(true)
            .screenshot(true)
            .screenshot_format(ScreenshotFormat::Jpeg)
            .screenshot_output_dir(PathBuf::from("/tmp/x"))
            .build();

        let crawler = Rehyke::new(cfg);
        assert!(crawler.config.enable_js);
        assert_eq!(crawler.config.js_scroll_count, 7);
        assert!(crawler.config.dismiss_popups);
        assert_eq!(crawler.config.viewport, Viewport::Mobile);
        assert!(crawler.config.detect_spa);
        assert!(crawler.config.randomize_fingerprint);
        assert!(crawler.config.screenshot);
        assert_eq!(crawler.config.screenshot_format, ScreenshotFormat::Jpeg);
        assert_eq!(crawler.config.screenshot_output_dir, Some(PathBuf::from("/tmp/x")));
    }

    // -----------------------------------------------------------------------
    // compile_patterns helper
    // -----------------------------------------------------------------------

    #[test]
    fn compile_patterns_empty_list() {
        let result = compile_patterns(&[]);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn compile_patterns_valid_regex() {
        let patterns = vec![
            r"https://example\.com/.*".to_string(),
            r"/admin(/.*)?".to_string(),
            r"\.(pdf|zip)$".to_string(),
        ];
        let result = compile_patterns(&patterns);
        assert!(result.is_ok());
        let compiled = result.unwrap();
        assert_eq!(compiled.len(), 3);
        // Spot-check: the PDF pattern matches.
        assert!(compiled[2].is_match("report.pdf"));
        assert!(!compiled[2].is_match("report.html"));
    }

    #[test]
    fn compile_patterns_invalid_regex_returns_error() {
        let patterns = vec!["[unclosed bracket".to_string()];
        let result = compile_patterns(&patterns);
        assert!(result.is_err());
        match result.unwrap_err() {
            RehykeError::ConfigError { message } => {
                assert!(message.contains("[unclosed bracket"));
            }
            other => panic!("expected ConfigError, got {:?}", other),
        }
    }

    #[test]
    fn compile_patterns_first_invalid_short_circuits() {
        let patterns = vec![
            r"valid.*pattern".to_string(),
            "[bad".to_string(),
            r"another.*valid".to_string(),
        ];
        let result = compile_patterns(&patterns);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Rehyke::run — invalid URL returns error without network access
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn run_with_invalid_url_returns_config_error() {
        let crawler = Rehyke::new(CrawlConfig::default());
        let result = crawler.run("not a url at all").await;
        assert!(result.is_err());
        match result.unwrap_err() {
            RehykeError::ConfigError { message } => {
                assert!(
                    message.contains("invalid seed URL"),
                    "unexpected message: {}",
                    message
                );
            }
            other => panic!("expected ConfigError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn run_with_empty_url_returns_config_error() {
        let crawler = Rehyke::new(CrawlConfig::default());
        let result = crawler.run("").await;
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Config propagation to RendererConfig (structural test)
    // -----------------------------------------------------------------------

    #[test]
    fn renderer_config_fields_match_crawl_config() {
        // Verify that field names/types align between CrawlConfig and RendererConfig.
        // This is a compile-time/structural check — if it builds, the fields match.
        let crawl_cfg = CrawlConfigBuilder::new()
            .enable_js(true)
            .viewport(Viewport::Tablet)
            .js_scroll_count(4)
            .dismiss_popups(true)
            .screenshot(true)
            .screenshot_format(ScreenshotFormat::Jpeg)
            .screenshot_output_dir(PathBuf::from("/tmp"))
            .randomize_fingerprint(true)
            .detect_spa(true)
            .js_wait_timeout(std::time::Duration::from_secs(12))
            .js_wait_strategy(WaitStrategy::NetworkIdle)
            .build();

        // Construct RendererConfig the same way lib.rs does.
        let renderer_cfg = crate::renderer::RendererConfig {
            render_timeout: crawl_cfg.js_wait_timeout,
            wait_strategy: crawl_cfg.js_wait_strategy.clone(),
            block_resources: true,
            tab_pool_size: (crawl_cfg.concurrency / 4).max(2),
            headless: true,
            max_scrolls: crawl_cfg.js_scroll_count,
            viewport: crawl_cfg.viewport,
            dismiss_popups: crawl_cfg.dismiss_popups,
            screenshot: crawl_cfg.screenshot,
            screenshot_format: crawl_cfg.screenshot_format,
            screenshot_output_dir: crawl_cfg.screenshot_output_dir.clone(),
            randomize_fingerprint: crawl_cfg.randomize_fingerprint,
            detect_spa: crawl_cfg.detect_spa,
        };

        assert_eq!(renderer_cfg.viewport, Viewport::Tablet);
        assert_eq!(renderer_cfg.max_scrolls, 4);
        assert!(renderer_cfg.dismiss_popups);
        assert!(renderer_cfg.screenshot);
        assert_eq!(renderer_cfg.screenshot_format, ScreenshotFormat::Jpeg);
        assert!(renderer_cfg.randomize_fingerprint);
        assert!(renderer_cfg.detect_spa);
        assert!(renderer_cfg.tab_pool_size >= 2);
    }

    // -----------------------------------------------------------------------
    // BrowserFingerprint integration in lib context
    // -----------------------------------------------------------------------

    #[test]
    fn browser_fingerprint_from_viewport_profile() {
        use crate::browser_fingerprint::{BrowserFingerprint, FingerprintProfile};

        let profile = FingerprintProfile::from(Viewport::Mobile);
        let fp = BrowserFingerprint::randomize(profile);
        let script = fp.to_injection_script();

        // The script should contain key overrides.
        assert!(!script.is_empty());
        assert!(script.contains("navigator"));
    }
}
