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
use tracing::{debug, info};
use url::Url;

use crate::converter::ConverterConfig;
use crate::extractor::ExtractedLinks;
use crate::fetcher::{ContentType, Fetcher};
use crate::output::RenderMethod;
use crate::parser::ParseConfig;

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
    /// Currently implements a single-page fetch as a foundation. The full
    /// multi-page crawl orchestrator (breadth-first with scheduler, dedup,
    /// robots checking, etc.) will be layered on top in a future phase.
    ///
    /// Steps:
    /// 1. Create a [`Fetcher`] from the config
    /// 2. Fetch the URL
    /// 3. Parse the HTML content
    /// 4. Convert to markdown
    /// 5. Return a [`CrawlResult`]
    pub async fn run(&self, url: &str) -> Result<Vec<CrawlResult>> {
        let parsed_url = Url::parse(url).map_err(|e| RehykeError::ConfigError {
            message: format!("invalid seed URL '{}': {}", url, e),
        })?;

        info!(url = %parsed_url, mode = ?self.config.mode, "starting crawl");

        // Step 1: Create fetcher from config.
        let fetcher = Fetcher::new(&self.config)?;

        // Step 2: Fetch the URL.
        debug!(url = %parsed_url, "fetching page");
        let fetch_result = fetcher.fetch_with_retry(&parsed_url).await?;

        let status_code = fetch_result.status;
        let content_type_str = fetch_result
            .headers
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("text/html")
            .to_string();

        // Step 3: Parse the content.
        let parse_config = ParseConfig {
            clean_navigation: self.config.clean_navigation,
            clean_footers: self.config.clean_footers,
            clean_ads: self.config.clean_ads,
            clean_comments: true,
            extract_metadata: self.config.extract_metadata,
        };

        let parsed = parser::parse(&fetch_result.body, &fetch_result.content_type, &parse_config)?;

        // Step 4: Convert to markdown.
        let converter_config = ConverterConfig {
            include_frontmatter: true,
            include_footer: false,
            max_blank_lines: 2,
        };
        let markdown =
            converter::to_markdown_with_url(&parsed, fetch_result.final_url.as_str(), &converter_config);

        // Extract the title from parsed metadata.
        let title = parsed
            .metadata
            .title
            .clone()
            .unwrap_or_else(|| "Untitled".to_string());

        // Step 5: Extract links for the result.
        let links = if matches!(fetch_result.content_type, ContentType::Html | ContentType::Xhtml)
        {
            let html = scraper::Html::parse_document(&fetch_result.body);
            extractor::extract_links(&html, &fetch_result.final_url)
        } else {
            ExtractedLinks::default()
        };

        let result = CrawlResult {
            url: fetch_result.final_url.to_string(),
            title,
            markdown,
            metadata: parsed.metadata,
            links,
            crawled_at: Utc::now(),
            status_code,
            content_type: content_type_str,
            depth: 0,
            render_method: if self.config.enable_js {
                RenderMethod::JavaScript
            } else {
                RenderMethod::Static
            },
        };

        info!(
            url = %result.url,
            title = %result.title,
            status = result.status_code,
            markdown_len = result.markdown.len(),
            "crawl complete"
        );

        Ok(vec![result])
    }
}
