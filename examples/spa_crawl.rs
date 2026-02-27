//! SPA Crawling Example (v0.2.0)
//!
//! Demonstrates Rehyke's specialised support for Single Page Applications:
//!
//!   1. **Framework detection** — identifies React, Vue, Angular, Svelte, Next.js,
//!      Nuxt, or SvelteKit from DOM signals and global JS objects.
//!
//!   2. **Routing patterns** — discovers client-side routes via hash-based
//!      (`#/about`) and history-based (`/about`) navigation by scanning the
//!      rendered page source for link patterns.
//!
//!   3. **Infinite scroll** — configures scroll count so Rehyke scrolls the page
//!      N times after initial render, triggering lazy content loads and
//!      Intersection-Observer pagination.
//!
//!   4. **Popup dismissal** — handles cookie consent banners, GDPR modals,
//!      newsletter overlays, and "load more" prompts before extracting content.
//!
//!   5. **Hydration-aware extraction** — uses `js_wait_timeout` and `detect_spa`
//!      so the framework has time to hydrate its components before the DOM is
//!      captured.
//!
//! # Requirements
//!
//! ```bash
//! cargo run --example spa_crawl --features js
//! ```
//!
//! Without Chrome the example still compiles and runs, demonstrating config
//! construction and fallback behaviour.

use std::collections::HashMap;
use std::time::Duration;

use rehyke_core::{
    CrawlConfigBuilder, DelayStrategy, Rehyke, ScanMode, Viewport, WaitStrategy,
};
use rehyke_core::output::RenderMethod;

// ---------------------------------------------------------------------------
// SPA-specific configuration profiles
// ---------------------------------------------------------------------------

/// Build a config tuned for React / Next.js applications.
fn react_config() -> rehyke_core::CrawlConfig {
    CrawlConfigBuilder::new()
        .mode(ScanMode::Full)
        .enable_js(true)
        // Next.js renders server-side and hydrates on the client; network idle
        // ensures the hydration XHRs complete before we snapshot the DOM.
        .js_wait_strategy(WaitStrategy::NetworkIdle)
        .js_wait_timeout(Duration::from_secs(12))
        // Scroll 8 viewports to trigger any lazy-loaded list items.
        .js_scroll_count(8)
        .dismiss_popups(true)
        .viewport(Viewport::Desktop)
        .detect_spa(true)
        .randomize_fingerprint(true)
        // Polite delay between pages — SPAs often have per-origin rate limits.
        .delay_strategy(DelayStrategy::Random {
            min: Duration::from_millis(300),
            max: Duration::from_millis(1200),
        })
        .max_pages(200)
        .build()
}

/// Build a config tuned for Vue / Nuxt applications.
fn vue_config() -> rehyke_core::CrawlConfig {
    CrawlConfigBuilder::new()
        .mode(ScanMode::Full)
        .enable_js(true)
        // Wait for the Vue app root (`#app` or `[data-v-app]`) to appear.
        .js_wait_strategy(WaitStrategy::Selector {
            selector: "#app, [data-v-app], #__nuxt".to_string(),
        })
        .js_wait_timeout(Duration::from_secs(10))
        .js_scroll_count(5)
        .dismiss_popups(true)
        .viewport(Viewport::Desktop)
        .detect_spa(true)
        .max_pages(200)
        .build()
}

/// Build a config tuned for Angular applications.
fn angular_config() -> rehyke_core::CrawlConfig {
    CrawlConfigBuilder::new()
        .mode(ScanMode::Full)
        .enable_js(true)
        // Angular bootstraps and then fires a change-detection cycle.
        // A short fixed delay after network idle is usually sufficient.
        .js_wait_strategy(WaitStrategy::Duration {
            duration: Duration::from_millis(1500),
        })
        .js_wait_timeout(Duration::from_secs(15))
        .js_scroll_count(6)
        .dismiss_popups(true)
        .viewport(Viewport::Desktop)
        .detect_spa(true)
        .max_pages(150)
        .build()
}

/// Build a config tuned for general SPA / unknown framework.
fn generic_spa_config() -> rehyke_core::CrawlConfig {
    CrawlConfigBuilder::new()
        .mode(ScanMode::Full)
        .enable_js(true)
        .js_wait_strategy(WaitStrategy::Auto)
        .js_wait_timeout(Duration::from_secs(10))
        .js_scroll_count(5)
        .dismiss_popups(true)
        .viewport(Viewport::Desktop)
        .detect_spa(true)
        .randomize_fingerprint(true)
        .max_pages(100)
        .build()
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "info".to_string())
                .as_str(),
        )
        .with_target(false)
        .init();

    println!("=== Rehyke SPA Crawling Example (v0.2.0) ===\n");

    // =========================================================================
    // Section 1: Show framework-specific configurations
    // =========================================================================
    println!("--- Framework-specific configuration profiles ---\n");

    let configs: Vec<(&str, rehyke_core::CrawlConfig)> = vec![
        ("React / Next.js", react_config()),
        ("Vue / Nuxt", vue_config()),
        ("Angular", angular_config()),
        ("Generic SPA", generic_spa_config()),
    ];

    for (name, cfg) in &configs {
        println!("  {}:", name);
        println!("    enable_js:       {}", cfg.enable_js);
        println!("    wait_strategy:   {:?}", cfg.js_wait_strategy);
        println!("    wait_timeout:    {:?}", cfg.js_wait_timeout);
        println!("    scroll_count:    {}", cfg.js_scroll_count);
        println!("    dismiss_popups:  {}", cfg.dismiss_popups);
        println!("    viewport:        {:?}", cfg.viewport);
        println!("    detect_spa:      {}", cfg.detect_spa);
        println!();
    }

    // =========================================================================
    // Section 2: Route pattern analysis
    // =========================================================================
    println!("--- SPA route pattern analysis ---\n");

    // After crawling a SPA's entry point the markdown will contain links.
    // We scan those links to classify them into routing patterns.

    let seed = "https://httpbin.org/html";
    println!("Crawling seed URL to analyse routes: {}\n", seed);

    let results = Rehyke::crawl(seed, ScanMode::Lite).await?;

    if let Some(result) = results.first() {
        println!("  Rendered via: {:?}", result.render_method);
        println!("  Total links found:");
        println!("    Internal:   {}", result.links.internal.len());
        println!("    External:   {}", result.links.external.len());
        println!("    Subdomains: {}", result.links.subdomains.len());

        // Classify internal links into routing patterns.
        let mut hash_routes: Vec<&str> = Vec::new();
        let mut history_routes: Vec<&str> = Vec::new();

        for link in &result.links.internal {
            if link.contains('#') {
                hash_routes.push(link);
            } else {
                history_routes.push(link);
            }
        }

        println!("\n  Routing pattern breakdown:");
        println!(
            "    Hash-based routes (#/path):     {}",
            hash_routes.len()
        );
        println!(
            "    History-based routes (/path):   {}",
            history_routes.len()
        );

        if !hash_routes.is_empty() {
            println!("\n  Sample hash routes:");
            for r in hash_routes.iter().take(5) {
                println!("    - {}", r);
            }
        }
        if !history_routes.is_empty() {
            println!("\n  Sample history routes:");
            for r in history_routes.iter().take(5) {
                println!("    - {}", r);
            }
        }
    }
    println!();

    // =========================================================================
    // Section 3: Infinite scroll simulation
    // =========================================================================
    println!("--- Infinite scroll configuration ---\n");

    // This configuration demonstrates how Rehyke handles infinite-scroll
    // pages (e.g. news feeds, social media timelines, product lists).
    let infinite_scroll_config = CrawlConfigBuilder::new()
        .mode(ScanMode::Lite)
        .enable_js(true)
        .js_wait_strategy(WaitStrategy::NetworkIdle)
        .js_wait_timeout(Duration::from_secs(15))
        // Scroll 10 viewports.  Each scroll triggers an Intersection Observer
        // which loads the next batch of items.
        .js_scroll_count(10)
        .dismiss_popups(true)
        .viewport(Viewport::Desktop)
        .build();

    println!("Infinite-scroll config:");
    println!("  scroll_count = {}", infinite_scroll_config.js_scroll_count);
    println!("  Rehyke scrolls the page {} times after initial render,", infinite_scroll_config.js_scroll_count);
    println!("  waiting 800 ms after each scroll for new content to load.");
    println!("  If the page stops moving (reached the bottom) it stops early.");
    println!();

    let crawler = Rehyke::new(infinite_scroll_config);
    match crawler.run("https://httpbin.org/html").await {
        Ok(results) => {
            println!("  Crawled {} page(s)", results.len());
            for r in &results {
                let words = r.markdown.split_whitespace().count();
                println!("  -> {} | {:?} | {} words", r.url, r.render_method, words);
            }
        }
        Err(e) => println!("  [WARN] Crawl failed (Chrome may not be installed): {}", e),
    }
    println!();

    // =========================================================================
    // Section 4: Popup / overlay dismissal showcase
    // =========================================================================
    println!("--- Popup dismissal showcase ---\n");
    println!("Rehyke's popup dismissal heuristic targets:");
    println!("  1. CSS selectors for known CMP providers (OneTrust, CookieBot, CookieYes)");
    println!("  2. Attribute patterns: [id*='cookie'][id*='accept'], [class*='consent']");
    println!("  3. ARIA labels: 'Accept cookies', 'Accept all', 'Agree'");
    println!("  4. JavaScript fallback: scans visible buttons for acceptance text");
    println!("     ('accept', 'agree', 'allow', 'ok', 'got it', 'i understand')");
    println!();
    println!("  dismiss_popups = true in CrawlConfigBuilder enables all of the above.");
    println!("  A 300 ms pause follows a successful click to allow animation completion.");
    println!();

    // =========================================================================
    // Section 5: Multi-framework crawl statistics
    // =========================================================================
    println!("--- Parallel multi-config crawl (same URL, different configs) ---\n");

    let test_url = "https://httpbin.org/html";
    let results_by_config = tokio::join!(
        async {
            let cfg = CrawlConfigBuilder::new()
                .mode(ScanMode::Lite)
                .enable_js(false)
                .build();
            ("Static (no JS)", Rehyke::new(cfg).run(test_url).await)
        },
        async {
            let cfg = CrawlConfigBuilder::new()
                .mode(ScanMode::Lite)
                .enable_js(true)
                .js_wait_strategy(WaitStrategy::Auto)
                .viewport(Viewport::Desktop)
                .build();
            ("JS + Desktop", Rehyke::new(cfg).run(test_url).await)
        },
        async {
            let cfg = CrawlConfigBuilder::new()
                .mode(ScanMode::Lite)
                .enable_js(true)
                .js_wait_strategy(WaitStrategy::Auto)
                .viewport(Viewport::Mobile)
                .build();
            ("JS + Mobile", Rehyke::new(cfg).run(test_url).await)
        },
    );

    let all_results = [results_by_config.0, results_by_config.1, results_by_config.2];

    println!("  {:<25} {:<15} {:<10}", "Config", "Render Method", "Words");
    println!("  {}", "-".repeat(55));
    for (label, result) in &all_results {
        match result {
            Ok(pages) => {
                if let Some(page) = pages.first() {
                    let words = page.markdown.split_whitespace().count();
                    println!(
                        "  {:<25} {:<15} {:<10}",
                        label,
                        format!("{:?}", page.render_method),
                        words
                    );
                }
            }
            Err(e) => {
                println!("  {:<25} ERROR: {}", label, e);
            }
        }
    }
    println!();

    // =========================================================================
    // Section 6: Real-world usage pattern
    // =========================================================================
    println!("--- Real-world usage pattern ---\n");
    println!("To crawl a production React app with full SPA support:\n");
    println!(r#"  let config = CrawlConfigBuilder::new()
      .mode(ScanMode::Full)
      .enable_js(true)
      .js_wait_strategy(WaitStrategy::NetworkIdle)
      .js_wait_timeout(Duration::from_secs(15))
      .js_scroll_count(10)
      .dismiss_popups(true)
      .viewport(Viewport::Desktop)
      .detect_spa(true)
      .randomize_fingerprint(true)
      .delay_strategy(DelayStrategy::Random {{
          min: Duration::from_millis(500),
          max: Duration::from_millis(2000),
      }})
      .max_pages(500)
      .output(OutputMode::Files {{
          output_dir: PathBuf::from("./output"),
          structure: FileStructure::Mirror,
      }})
      .build();

  let crawler = Rehyke::new(config);
  let results = crawler.run("https://my-react-app.com").await?;"#);
    println!();

    println!("=== Done ===");
    Ok(())
}
