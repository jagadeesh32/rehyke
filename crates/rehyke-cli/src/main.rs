use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use tracing::info;

use rehyke_core::{
    CrawlConfig, CrawlConfigBuilder, DelayStrategy, FileStructure, OutputMode, ProxyConfig,
    ProxyType, Rehyke, RetryConfig, ScanMode, ScreenshotFormat, Viewport, WaitStrategy,
};

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

/// Rehyke -- Crawl Everything. Miss Nothing.
///
/// Ultra-high-performance web crawler that converts web pages to clean Markdown.
/// With `--js` (v0.2.0+) it drives a headless Chrome browser for full
/// JavaScript rendering, SPA crawling, infinite-scroll detection, popup
/// dismissal, and screenshot capture.
#[derive(Debug, Parser)]
#[command(
    name = "rehyke",
    version,
    about = "Crawl Everything. Miss Nothing.",
    long_about = "Ultra-high-performance web crawler that converts web pages to clean Markdown.\n\nEnables JS rendering, SPA handling, infinite scroll, popup dismissal, and\nscreenshot capture with --js (requires Chrome/Chromium and the `js` feature)."
)]
struct Cli {
    /// URL to crawl
    url: String,

    /// Scan mode preset
    #[arg(long, value_enum, default_value_t = CliScanMode::Full)]
    mode: CliScanMode,

    /// Maximum crawl depth from the seed URL
    #[arg(long)]
    max_depth: Option<usize>,

    /// Maximum number of pages to crawl
    #[arg(long)]
    max_pages: Option<usize>,

    /// Number of concurrent requests
    #[arg(long)]
    concurrency: Option<usize>,

    // -----------------------------------------------------------------------
    // v0.2.0 — JS rendering flags
    // -----------------------------------------------------------------------

    /// Enable JavaScript rendering via headless Chrome/Chromium (v0.2.0+).
    ///
    /// Requires the `js` feature flag and a Chrome/Chromium installation.
    /// Falls back to static fetching when Chrome is unavailable.
    #[arg(long)]
    js: bool,

    /// Wait for a CSS selector to appear before extracting content (implies --js).
    ///
    /// Example: --wait-for ".content-loaded"
    #[arg(long, value_name = "SELECTOR")]
    wait_for: Option<String>,

    /// Timeout in seconds for the JS wait strategy (default: 10).
    #[arg(long, default_value_t = 10)]
    js_timeout: u64,

    /// Number of scroll operations to perform for infinite-scroll pages (implies --js).
    ///
    /// Scrolls the page by one viewport height up to N times, waiting for new
    /// content to load after each scroll.
    #[arg(long, value_name = "N", default_value_t = 0)]
    scroll: usize,

    /// Automatically detect and dismiss cookie consent banners, GDPR modals,
    /// and newsletter popups before extracting content (implies --js).
    #[arg(long)]
    dismiss_popups: bool,

    /// Capture a full-page screenshot after JS rendering (implies --js).
    ///
    /// Screenshots are written to the directory specified by --screenshot-dir
    /// (or the current directory if not set).
    #[arg(long)]
    screenshot: bool,

    /// Image format for captured screenshots.
    #[arg(long, value_enum, default_value_t = CliScreenshotFormat::Png)]
    screenshot_format: CliScreenshotFormat,

    /// Directory where screenshots are saved (default: current directory).
    #[arg(long, value_name = "DIR")]
    screenshot_dir: Option<PathBuf>,

    /// Browser viewport size profile (default: desktop).
    #[arg(long, value_enum, default_value_t = CliViewport::Desktop)]
    viewport: CliViewport,

    /// Automatically detect the SPA framework (React, Vue, Angular, …) and
    /// log it alongside the crawl result (implies --js).
    #[arg(long)]
    detect_spa: bool,

    /// Randomize browser fingerprint details (viewport noise, WebGL strings,
    /// navigator.languages) to reduce bot-detection signals (implies --js).
    #[arg(long)]
    randomize_fingerprint: bool,

    // -----------------------------------------------------------------------
    // Output
    // -----------------------------------------------------------------------

    /// Write individual .md files to this directory
    #[arg(long)]
    output_dir: Option<PathBuf>,

    /// Write all output to a single file
    #[arg(short = 'o', long)]
    output: Option<PathBuf>,

    /// File structure when using --output-dir
    #[arg(long, value_enum, default_value_t = CliFileStructure::Flat)]
    structure: CliFileStructure,

    // -----------------------------------------------------------------------
    // Network
    // -----------------------------------------------------------------------

    /// Proxy URL (can be specified multiple times)
    #[arg(long = "proxy", value_name = "URL")]
    proxies: Vec<String>,

    /// Random delay between requests in milliseconds (e.g. "500-2000")
    #[arg(long, value_name = "MIN-MAX")]
    delay: Option<String>,

    /// Exclude URLs matching this regex (can be specified multiple times)
    #[arg(long = "exclude", value_name = "PATTERN")]
    exclude_patterns: Vec<String>,

    /// Only crawl URLs matching this regex (can be specified multiple times)
    #[arg(long = "include", value_name = "PATTERN")]
    include_patterns: Vec<String>,

    /// Ignore robots.txt directives
    #[arg(long)]
    no_robots: bool,

    /// Remove navigation elements from extracted content
    #[arg(long)]
    clean_nav: bool,

    /// Remove footer elements from extracted content
    #[arg(long)]
    clean_footer: bool,

    /// Remove advertisement elements from extracted content
    #[arg(long)]
    clean_ads: bool,

    /// Per-request timeout in seconds
    #[arg(long)]
    timeout: Option<u64>,

    /// Number of retry attempts for failed requests
    #[arg(long)]
    retries: Option<u32>,

    /// Custom User-Agent header
    #[arg(long)]
    user_agent: Option<String>,

    // -----------------------------------------------------------------------
    // Misc
    // -----------------------------------------------------------------------

    /// Enable verbose (debug-level) logging
    #[arg(short = 'v', long)]
    verbose: bool,

    /// Output format
    #[arg(long, value_enum, default_value_t = CliOutputFormat::Markdown)]
    format: CliOutputFormat,
}

// ---------------------------------------------------------------------------
// Clap value enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliScanMode {
    Lite,
    Full,
    Deep,
}

impl From<CliScanMode> for ScanMode {
    fn from(mode: CliScanMode) -> Self {
        match mode {
            CliScanMode::Lite => ScanMode::Lite,
            CliScanMode::Full => ScanMode::Full,
            CliScanMode::Deep => ScanMode::Deep,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliFileStructure {
    Flat,
    Mirror,
}

impl From<CliFileStructure> for FileStructure {
    fn from(s: CliFileStructure) -> Self {
        match s {
            CliFileStructure::Flat => FileStructure::Flat,
            CliFileStructure::Mirror => FileStructure::Mirror,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliOutputFormat {
    Markdown,
    Json,
}

/// v0.2.0 — viewport profile
#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliViewport {
    Desktop,
    Tablet,
    Mobile,
}

impl From<CliViewport> for Viewport {
    fn from(v: CliViewport) -> Self {
        match v {
            CliViewport::Desktop => Viewport::Desktop,
            CliViewport::Tablet => Viewport::Tablet,
            CliViewport::Mobile => Viewport::Mobile,
        }
    }
}

/// v0.2.0 — screenshot format
#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliScreenshotFormat {
    Png,
    Jpeg,
}

impl From<CliScreenshotFormat> for ScreenshotFormat {
    fn from(f: CliScreenshotFormat) -> Self {
        match f {
            CliScreenshotFormat::Png => ScreenshotFormat::Png,
            CliScreenshotFormat::Jpeg => ScreenshotFormat::Jpeg,
        }
    }
}

// ---------------------------------------------------------------------------
// Config building
// ---------------------------------------------------------------------------

fn build_config(cli: &Cli) -> Result<CrawlConfig> {
    let mode: ScanMode = cli.mode.into();

    // Any JS-adjacent flag implicitly enables JS rendering.
    let enable_js = cli.js
        || cli.wait_for.is_some()
        || cli.scroll > 0
        || cli.dismiss_popups
        || cli.screenshot
        || cli.detect_spa
        || cli.randomize_fingerprint;

    let mut builder = CrawlConfigBuilder::new().mode(mode).enable_js(enable_js);

    // Override mode defaults if the user explicitly provided them.
    if let Some(depth) = cli.max_depth {
        builder = builder.max_depth(depth);
    }
    if let Some(pages) = cli.max_pages {
        builder = builder.max_pages(pages);
    }
    if let Some(conc) = cli.concurrency {
        builder = builder.concurrency(conc);
    }

    // -----------------------------------------------------------------------
    // v0.2.0 — JS / browser flags
    // -----------------------------------------------------------------------

    // Wait strategy.
    if let Some(ref selector) = cli.wait_for {
        builder = builder.js_wait_strategy(WaitStrategy::Selector {
            selector: selector.clone(),
        });
    } else if enable_js {
        builder = builder.js_wait_strategy(WaitStrategy::NetworkIdle);
    }

    builder = builder
        .js_wait_timeout(Duration::from_secs(cli.js_timeout))
        .js_scroll_count(cli.scroll)
        .dismiss_popups(cli.dismiss_popups)
        .screenshot(cli.screenshot)
        .screenshot_format(cli.screenshot_format.into())
        .viewport(cli.viewport.into())
        .detect_spa(cli.detect_spa)
        .randomize_fingerprint(cli.randomize_fingerprint);

    if let Some(ref dir) = cli.screenshot_dir {
        builder = builder.screenshot_output_dir(dir.clone());
    }

    // -----------------------------------------------------------------------
    // Output mode
    // -----------------------------------------------------------------------

    if let Some(ref dir) = cli.output_dir {
        builder = builder.output(OutputMode::Files {
            output_dir: dir.clone(),
            structure: cli.structure.into(),
        });
    } else if let Some(ref file) = cli.output {
        builder = builder.output(OutputMode::SingleFile {
            output_path: file.clone(),
        });
    }

    // -----------------------------------------------------------------------
    // Network
    // -----------------------------------------------------------------------

    if !cli.proxies.is_empty() {
        let proxy_configs: Vec<ProxyConfig> = cli
            .proxies
            .iter()
            .map(|url| ProxyConfig {
                url: url.clone(),
                proxy_type: if url.starts_with("socks5") {
                    ProxyType::Socks5
                } else if url.starts_with("https") {
                    ProxyType::Https
                } else {
                    ProxyType::Http
                },
                auth: None,
                region: None,
            })
            .collect();
        builder = builder.proxies(proxy_configs);
    }

    if let Some(ref delay_str) = cli.delay {
        let delay_strategy = parse_delay(delay_str)
            .with_context(|| format!("invalid --delay value: '{}'", delay_str))?;
        builder = builder.delay_strategy(delay_strategy);
    }

    if !cli.exclude_patterns.is_empty() {
        builder = builder.exclude_patterns(cli.exclude_patterns.clone());
    }
    if !cli.include_patterns.is_empty() {
        builder = builder.include_patterns(cli.include_patterns.clone());
    }

    if cli.no_robots {
        builder = builder.respect_robots_txt(false);
    }

    builder = builder.clean_navigation(cli.clean_nav || true);
    builder = builder.clean_footers(cli.clean_footer || true);
    builder = builder.clean_ads(cli.clean_ads || true);

    if let Some(secs) = cli.timeout {
        builder = builder.timeout(Duration::from_secs(secs));
    }

    if let Some(retries) = cli.retries {
        let mut retry_config = RetryConfig::default();
        retry_config.max_retries = retries;
        builder = builder.retry_config(retry_config);
    }

    if let Some(ref ua) = cli.user_agent {
        builder = builder.user_agent(ua.clone());
    }

    Ok(builder.build())
}

/// Parse a delay specification like "500-2000" into a [`DelayStrategy`].
fn parse_delay(s: &str) -> Result<DelayStrategy> {
    if let Some((min_str, max_str)) = s.split_once('-') {
        let min_ms: u64 = min_str
            .trim()
            .parse()
            .with_context(|| "min delay is not a valid number")?;
        let max_ms: u64 = max_str
            .trim()
            .parse()
            .with_context(|| "max delay is not a valid number")?;
        Ok(DelayStrategy::Random {
            min: Duration::from_millis(min_ms),
            max: Duration::from_millis(max_ms),
        })
    } else {
        let ms: u64 = s
            .trim()
            .parse()
            .with_context(|| "delay is not a valid number")?;
        Ok(DelayStrategy::Fixed {
            delay: Duration::from_millis(ms),
        })
    }
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Set up tracing subscriber.
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level)),
        )
        .with_target(false)
        .init();

    // Print banner.
    eprintln!(
        "{} {} {}",
        style("rehyke").cyan().bold(),
        style(env!("CARGO_PKG_VERSION")).dim(),
        style("-- Crawl Everything. Miss Nothing.").dim()
    );
    eprintln!();

    // Build config.
    let config = build_config(&cli)?;

    // Print crawl plan summary.
    eprintln!(
        "  {} {}",
        style("Target:").bold(),
        style(&cli.url).underlined()
    );
    eprintln!(
        "  {} {:?}  |  depth: {}  |  pages: {}  |  concurrency: {}",
        style("Mode:").bold(),
        config.mode,
        config.max_depth,
        config.max_pages,
        config.concurrency
    );
    if config.enable_js {
        let (vw, vh) = config.viewport.dimensions();
        eprintln!(
            "  {} enabled  |  viewport: {}x{}  |  wait: {:?}  |  timeout: {}s",
            style("JS rendering:").bold(),
            vw,
            vh,
            config.js_wait_strategy,
            config.js_wait_timeout.as_secs()
        );
        if config.js_scroll_count > 0 {
            eprintln!(
                "  {} {} scrolls",
                style("Infinite scroll:").bold(),
                config.js_scroll_count
            );
        }
        if config.dismiss_popups {
            eprintln!("  {} enabled", style("Popup dismissal:").bold());
        }
        if config.screenshot {
            let ext = match config.screenshot_format {
                ScreenshotFormat::Png => "png",
                ScreenshotFormat::Jpeg => "jpeg",
            };
            eprintln!(
                "  {} {} format",
                style("Screenshots:").bold(),
                ext.to_uppercase()
            );
        }
        if config.detect_spa {
            eprintln!("  {} enabled", style("SPA detection:").bold());
        }
        if config.randomize_fingerprint {
            eprintln!("  {} enabled", style("Fingerprint randomisation:").bold());
        }
    }
    match &config.output {
        OutputMode::Memory => {
            eprintln!("  {} stdout", style("Output:").bold());
        }
        OutputMode::Files {
            output_dir,
            structure,
        } => {
            eprintln!(
                "  {} {} ({:?})",
                style("Output dir:").bold(),
                output_dir.display(),
                structure
            );
        }
        OutputMode::SingleFile { output_path } => {
            eprintln!(
                "  {} {}",
                style("Output file:").bold(),
                output_path.display()
            );
        }
    }
    eprintln!();

    // Set up progress bar.
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} [{elapsed_precise}] {msg}")
            .unwrap()
            .tick_chars("/-\\|"),
    );
    pb.set_message(format!("Crawling {}...", &cli.url));
    pb.enable_steady_tick(Duration::from_millis(120));

    // Run crawl.
    let start = Instant::now();
    let crawler = Rehyke::new(config);
    let results = crawler.run(&cli.url).await;
    let elapsed = start.elapsed();

    pb.finish_and_clear();

    match results {
        Ok(pages) => {
            let error_count = pages.iter().filter(|p| p.status_code >= 400).count();
            let js_count = pages
                .iter()
                .filter(|p| matches!(p.render_method, rehyke_core::output::RenderMethod::JavaScript))
                .count();

            // Output results.
            match cli.format {
                CliOutputFormat::Markdown => {
                    for page in &pages {
                        println!("{}", page.markdown);
                    }
                }
                CliOutputFormat::Json => {
                    let json = serde_json::to_string_pretty(&pages)
                        .context("failed to serialize results to JSON")?;
                    println!("{}", json);
                }
            }

            // Print summary.
            eprintln!();
            eprintln!("{} Crawl complete!", style("✓").green().bold());
            eprintln!(
                "  Pages crawled: {}  |  JS-rendered: {}  |  Errors: {}  |  Time: {:.2}s",
                style(pages.len()).cyan().bold(),
                style(js_count).cyan(),
                if error_count > 0 {
                    style(error_count).red().bold()
                } else {
                    style(error_count).green().bold()
                },
                elapsed.as_secs_f64()
            );

            info!(
                pages = pages.len(),
                js_rendered = js_count,
                errors = error_count,
                elapsed_ms = elapsed.as_millis(),
                "crawl finished"
            );
        }
        Err(e) => {
            eprintln!(
                "{} Crawl failed: {}",
                style("✗").red().bold(),
                style(&e).red()
            );
            eprintln!("  Time: {:.2}s", elapsed.as_secs_f64());
            return Err(e.into());
        }
    }

    Ok(())
}
