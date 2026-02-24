//! Deep Crawl Example
//!
//! Demonstrates an advanced crawl configuration using `CrawlConfigBuilder`.
//! This example sets up a deep crawl with custom delay strategies, proxy
//! configuration, URL filtering via regex patterns, content cleaning, custom
//! headers/cookies, and file output with a mirror directory structure.
//!
//! Run with:
//!   cargo run --example deep_crawl

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use rehyke_core::{
    CrawlConfigBuilder, DelayStrategy, FileStructure, OutputMode, ProxyConfig, ProxyStrategy,
    ProxyType, Rehyke, RetryConfig, ScanMode,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== Rehyke Deep Crawl Example ===\n");

    // =====================================================================
    // Step 1: Build a comprehensive crawl configuration
    // =====================================================================
    // CrawlConfigBuilder starts with sensible defaults and lets you
    // override only what you need. Calling `.mode(Deep)` sets max_depth,
    // max_pages, and concurrency to aggressive values automatically.

    let output_dir = PathBuf::from("/tmp/rehyke_deep_crawl");
    println!("Output directory: {}\n", output_dir.display());

    let config = CrawlConfigBuilder::new()
        // -- Scan scope --
        // Deep mode: max_depth=50, max_pages=50000, concurrency=25.
        // We override to more conservative values for this example.
        .mode(ScanMode::Deep)
        .max_depth(15)
        .max_pages(5000)
        .concurrency(10)
        // -- Delay strategy --
        // Random delay between 500ms and 2000ms to be polite to servers.
        // Other options: Fixed { delay }, Adaptive { initial }, None.
        .delay_strategy(DelayStrategy::Random {
            min: Duration::from_millis(500),
            max: Duration::from_millis(2000),
        })
        // -- Proxy configuration --
        // Configure proxies for requests. In production you would use real
        // proxy endpoints. Using an empty list here since these are
        // illustrative.
        .proxies(vec![
            ProxyConfig {
                url: "http://proxy1.example.com:8080".to_string(),
                proxy_type: ProxyType::Http,
                auth: None,
                region: Some("us-east".to_string()),
            },
            ProxyConfig {
                url: "socks5://proxy2.example.com:1080".to_string(),
                proxy_type: ProxyType::Socks5,
                auth: None,
                region: Some("eu-west".to_string()),
            },
        ])
        .proxy_strategy(ProxyStrategy::RoundRobin)
        // -- URL filtering with regex patterns --
        // Only crawl URLs under /docs or /blog sections.
        .include_patterns(vec![
            r"https://[^/]+/docs/.*".to_string(),
            r"https://[^/]+/blog/.*".to_string(),
        ])
        // Skip URLs matching these patterns (images, PDFs, login pages).
        .exclude_patterns(vec![
            r".*\.(png|jpg|jpeg|gif|svg|ico)$".to_string(),
            r".*\.(pdf|zip|tar\.gz)$".to_string(),
            r".*/login.*".to_string(),
            r".*/admin.*".to_string(),
        ])
        // -- Content cleaning --
        // Strip navigation, footers, and ads for cleaner markdown output.
        .clean_navigation(true)
        .clean_footers(true)
        .clean_ads(true)
        .extract_metadata(true)
        // -- Output configuration --
        // Write files mirroring the site's URL structure on disk.
        // Each page becomes: output_dir/{host}/{path}/index.md
        .output(OutputMode::Files {
            output_dir: output_dir.clone(),
            structure: FileStructure::Mirror,
        })
        // -- Network settings --
        .timeout(Duration::from_secs(45))
        .retry_config(RetryConfig {
            max_retries: 5,
            initial_delay: Duration::from_millis(1000),
            max_delay: Duration::from_secs(60),
        })
        .user_agent("rehyke-example/1.0 (deep-crawl demo)")
        // -- Custom headers --
        // Add any extra HTTP headers needed for the target site.
        .header("Accept-Language", "en-US,en;q=0.9")
        .header("X-Requested-With", "Rehyke")
        // -- Cookies --
        // Inject cookies for authenticated or session-aware crawling.
        .cookie("consent", "accepted")
        .cookie("theme", "dark")
        // -- Robots.txt and URL normalization --
        .respect_robots_txt(true)
        .remove_www(true)
        .build();

    // =====================================================================
    // Step 2: Print configuration summary
    // =====================================================================
    println!("Configuration Summary:");
    println!("  Mode:            {:?}", config.mode);
    println!("  Max Depth:       {}", config.max_depth);
    println!("  Max Pages:       {}", config.max_pages);
    println!("  Concurrency:     {}", config.concurrency);
    println!("  Delay Strategy:  {:?}", config.delay_strategy);
    println!("  Proxies:         {} configured", config.proxies.len());
    println!("  Include:         {} pattern(s)", config.include_patterns.len());
    println!("  Exclude:         {} pattern(s)", config.exclude_patterns.len());
    println!("  Timeout:         {:?}", config.timeout);
    println!("  User-Agent:      {}", config.user_agent);
    println!("  Custom Headers:  {}", config.custom_headers.len());
    println!("  Cookies:         {}", config.cookies.len());
    println!("  Respect Robots:  {}", config.respect_robots_txt);
    println!();

    // =====================================================================
    // Step 3: Run the crawl
    // =====================================================================
    // Note: we use a public test URL for this example. In a real deep crawl
    // you would point this at a documentation site or blog.
    let seed_url = "https://httpbin.org/html";

    println!("Starting deep crawl of: {}\n", seed_url);

    let crawler = Rehyke::new(config);
    let results = match crawler.run(seed_url).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Crawl failed: {}", e);
            return Err(e.into());
        }
    };

    // =====================================================================
    // Step 4: Progress tracking and result statistics
    // =====================================================================
    println!("=== Crawl Statistics ===\n");

    let total_pages = results.len();
    println!("Total pages crawled: {}", total_pages);

    if total_pages == 0 {
        println!("No pages were crawled.");
        return Ok(());
    }

    // Aggregate content type distribution.
    let mut content_types: HashMap<String, usize> = HashMap::new();
    let mut total_words: usize = 0;
    let mut total_links: usize = 0;

    for result in &results {
        *content_types.entry(result.content_type.clone()).or_insert(0) += 1;
        total_words += result.markdown.split_whitespace().count();
        total_links += result.links.internal.len() + result.links.external.len();
    }

    let avg_words = total_words as f64 / total_pages as f64;

    println!("Total words:         {}", total_words);
    println!("Average word count:  {:.1}", avg_words);
    println!("Total links found:   {}", total_links);
    println!("\nContent types:");
    for (ct, count) in &content_types {
        println!("  {}: {}", ct, count);
    }

    println!("\nPer-page breakdown:");
    for (i, result) in results.iter().enumerate() {
        let words = result.markdown.split_whitespace().count();
        let internal = result.links.internal.len();
        let external = result.links.external.len();
        println!(
            "  [{}] {} (status={}, words={}, links={}+{})",
            i + 1,
            result.url,
            result.status_code,
            words,
            internal,
            external
        );
    }

    println!("\nOutput written to: {}", output_dir.display());
    println!("\n=== Done ===");
    Ok(())
}
