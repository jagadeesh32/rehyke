pub mod anti_detect;
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
use crate::converter::ConverterConfig;
use crate::extractor::ExtractedLinks;
use crate::fetcher::{ContentType, Fetcher};
use crate::output::{OutputHandler, RenderMethod};
use crate::parser::ParseConfig;
use crate::robots::RobotsTxt;
use crate::scheduler::{Scheduler, TaskSource};

/// Main orchestrator for the Rehyke web crawler.
///
/// Provides both a simple one-shot API ([`Rehyke::crawl`]) and a configurable
/// API ([`Rehyke::run`]) for performing web crawls.
pub struct Rehyke {
    config: CrawlConfig,
}

impl Rehyke {
    /// Create a new crawler with the given configuration.
    pub fn new(config: CrawlConfig) -> Self {
        Self { config }
    }

    /// Simple one-shot crawl API.
    ///
    /// Creates a default [`CrawlConfig`] with the given [`ScanMode`], fetches
    /// the URL, parses the content, converts it to markdown, and returns the
    /// results.
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
    /// 4. BFS crawl loop: pop URL → check robots → delay → fetch → parse →
    ///    extract links → feed links back to scheduler → repeat
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

        let enable_js = self.config.enable_js;

        // Main crawl loop.
        loop {
            if scheduler.is_done() {
                break;
            }

            // Try to get the next task.
            let task = match scheduler.next_task() {
                Some(t) => t,
                None => {
                    // Queue might be temporarily empty due to rate limiting
                    // while tasks are still in-progress. Wait briefly and retry.
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

            // Fetch the page.
            debug!(url = %task.url, depth = task.depth, "fetching page");
            let fetch_result = match fetcher.fetch_with_retry(&task.url).await {
                Ok(r) => {
                    // Feed response status back to the adaptive delay strategy.
                    anti_detect.record_response(r.status, r.elapsed);
                    r
                }
                Err(e) => {
                    warn!(url = %task.url, error = %e, "fetch failed");
                    scheduler.mark_failed(&task.url);
                    continue;
                }
            };

            let status_code = fetch_result.status;
            let content_type_str = fetch_result
                .headers
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("text/html")
                .to_string();

            // Parse the content.
            let parsed = match parser::parse(
                &fetch_result.body,
                &fetch_result.content_type,
                &parse_config,
            ) {
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
                fetch_result.final_url.as_str(),
                &converter_config,
            );

            let title = parsed
                .metadata
                .title
                .clone()
                .unwrap_or_else(|| "Untitled".to_string());

            // Extract links and feed them back to the scheduler.
            let links =
                if matches!(fetch_result.content_type, ContentType::Html | ContentType::Xhtml) {
                    let html = scraper::Html::parse_document(&fetch_result.body);
                    let extracted = extractor::extract_links(&html, &fetch_result.final_url);

                    // Feed internal links back to the scheduler.
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

                    // Feed external links in Deep mode.
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

                    // Feed subdomain links.
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
                url: fetch_result.final_url.to_string(),
                title,
                markdown,
                metadata: parsed.metadata,
                links,
                crawled_at: Utc::now(),
                status_code,
                content_type: content_type_str,
                depth: task.depth,
                render_method: if enable_js {
                    RenderMethod::JavaScript
                } else {
                    RenderMethod::Static
                },
            };

            info!(
                url = %result.url,
                title = %result.title,
                status = result.status_code,
                depth = result.depth,
                markdown_len = result.markdown.len(),
                "page crawled"
            );

            // Send result to output handler.
            output_handler.handle_result(result)?;

            // Mark this URL as completed in the scheduler.
            scheduler.mark_completed(&task.url);
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
