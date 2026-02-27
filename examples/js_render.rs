//! JavaScript Rendering Example (v0.2.0)
//!
//! Demonstrates how to use Rehyke's headless-browser pipeline to fetch
//! JavaScript-heavy Single Page Applications (SPAs) and compare the result
//! against a plain static fetch.
//!
//! Features shown in this example:
//!   - Enabling JS rendering with `enable_js(true)`
//!   - Configuring wait strategies (NetworkIdle, Selector, Duration, Auto)
//!   - SPA framework auto-detection (React / Vue / Angular / …)
//!   - Popup and cookie-banner dismissal
//!   - Browser viewport profiles (desktop / tablet / mobile)
//!   - Browser fingerprint randomisation
//!   - Reading the `render_method` field from `CrawlResult`
//!
//! # Requirements
//!
//! Chrome or Chromium must be installed and discoverable, AND the crate must
//! be compiled with the `js` feature:
//!
//! ```bash
//! cargo run --example js_render --features js
//! ```
//!
//! Without Chrome the renderer logs a warning and falls back to static fetch.
//!
//! # Run (static fallback — no Chrome needed)
//!
//! ```bash
//! cargo run --example js_render
//! ```

use std::time::Duration;

use rehyke_core::{
    CrawlConfigBuilder, Rehyke, ScanMode, ScreenshotFormat, Viewport, WaitStrategy,
};
use rehyke_core::output::RenderMethod;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialise logging (set RUST_LOG=debug for verbose output).
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "info".to_string())
                .as_str(),
        )
        .with_target(false)
        .init();

    println!("=== Rehyke JS Rendering Example (v0.2.0) ===\n");

    // =========================================================================
    // Section 1: NetworkIdle wait — best for most SPAs
    // =========================================================================
    println!("--- Strategy 1: NetworkIdle (best for React/Vue/Angular SPAs) ---\n");

    let config_network_idle = CrawlConfigBuilder::new()
        .mode(ScanMode::Lite)
        // Enable headless-browser rendering.
        .enable_js(true)
        // Wait until no pending network requests remain (network idle).
        // This ensures XHR/fetch calls triggered by JS have settled.
        .js_wait_strategy(WaitStrategy::NetworkIdle)
        // Maximum wait time before the renderer gives up.
        .js_wait_timeout(Duration::from_secs(10))
        // Desktop viewport — 1920 × 1080.
        .viewport(Viewport::Desktop)
        // Auto-detect the JavaScript framework.
        .detect_spa(true)
        // Dismiss cookie-consent banners, GDPR modals, newsletter overlays.
        .dismiss_popups(true)
        .build();

    println!("Config: JS=on  strategy=NetworkIdle  viewport=Desktop");
    println!("Target: https://httpbin.org/html (static page used as a proxy)\n");

    // We use httpbin.org/html as it is always available.  In production you
    // would point this at a real SPA like https://react-app.example.com.
    let crawler = Rehyke::new(config_network_idle);
    match crawler.run("https://httpbin.org/html").await {
        Ok(results) => {
            for result in &results {
                let rendered = matches!(result.render_method, RenderMethod::JavaScript);
                println!("  URL:          {}", result.url);
                println!("  Title:        {}", result.title);
                println!("  Status:       {}", result.status_code);
                println!("  Render:       {} (JS={})", format!("{:?}", result.render_method), rendered);
                let words = result.markdown.split_whitespace().count();
                println!("  Words:        {}", words);
                println!();
            }
        }
        Err(e) => {
            println!("  [WARN] Crawl failed (Chrome may not be installed): {}", e);
            println!("  Falling back strategy demonstrated below.\n");
        }
    }

    // =========================================================================
    // Section 2: Selector wait — best when you know the target element
    // =========================================================================
    println!("--- Strategy 2: Selector wait (wait for '.herman' to appear) ---\n");

    let config_selector = CrawlConfigBuilder::new()
        .mode(ScanMode::Lite)
        .enable_js(true)
        // Wait until a specific CSS selector is present in the DOM.
        // Rehyke polls until the element appears or js_wait_timeout elapses.
        .js_wait_strategy(WaitStrategy::Selector {
            selector: "body".to_string(), // body always exists — illustrative
        })
        .js_wait_timeout(Duration::from_secs(5))
        .viewport(Viewport::Desktop)
        .build();

    println!("Config: JS=on  strategy=Selector('body')  viewport=Desktop");

    let crawler2 = Rehyke::new(config_selector);
    match crawler2.run("https://httpbin.org/html").await {
        Ok(results) => {
            if let Some(r) = results.first() {
                println!("  Rendered: {:?} — {} words", r.render_method, r.markdown.split_whitespace().count());
            }
        }
        Err(e) => println!("  [WARN] {}", e),
    }
    println!();

    // =========================================================================
    // Section 3: Duration wait — fixed settle time
    // =========================================================================
    println!("--- Strategy 3: Duration wait (fixed 2-second settle period) ---\n");

    let config_duration = CrawlConfigBuilder::new()
        .mode(ScanMode::Lite)
        .enable_js(true)
        .js_wait_strategy(WaitStrategy::Duration {
            duration: Duration::from_secs(2),
        })
        .js_wait_timeout(Duration::from_secs(15))
        .viewport(Viewport::Tablet)
        .detect_spa(true)
        .build();

    println!("Config: JS=on  strategy=Duration(2s)  viewport=Tablet (768×1024)");

    let crawler3 = Rehyke::new(config_duration);
    match crawler3.run("https://httpbin.org/html").await {
        Ok(results) => {
            if let Some(r) = results.first() {
                println!("  Rendered: {:?}", r.render_method);
            }
        }
        Err(e) => println!("  [WARN] {}", e),
    }
    println!();

    // =========================================================================
    // Section 4: Mobile viewport with fingerprint randomisation
    // =========================================================================
    println!("--- Strategy 4: Mobile viewport + fingerprint randomisation ---\n");

    let config_mobile = CrawlConfigBuilder::new()
        .mode(ScanMode::Lite)
        .enable_js(true)
        .js_wait_strategy(WaitStrategy::Auto)
        .js_wait_timeout(Duration::from_secs(8))
        // Emulate a mobile device — 390×844, touch enabled.
        .viewport(Viewport::Mobile)
        // Randomise viewport size within the mobile range, WebGL strings,
        // navigator.languages, and timezone to avoid fingerprinting.
        .randomize_fingerprint(true)
        .dismiss_popups(true)
        .detect_spa(true)
        .build();

    println!("Config: JS=on  strategy=Auto  viewport=Mobile (390×844)  fingerprint=random");

    let crawler4 = Rehyke::new(config_mobile);
    match crawler4.run("https://httpbin.org/html").await {
        Ok(results) => {
            if let Some(r) = results.first() {
                println!("  Rendered: {:?}  words: {}", r.render_method, r.markdown.split_whitespace().count());
            }
        }
        Err(e) => println!("  [WARN] {}", e),
    }
    println!();

    // =========================================================================
    // Section 5: Static vs JS comparison
    // =========================================================================
    println!("--- Section 5: Static vs JS render comparison ---\n");

    let static_results = Rehyke::crawl("https://httpbin.org/html", ScanMode::Lite).await?;

    let js_config = CrawlConfigBuilder::new()
        .mode(ScanMode::Lite)
        .enable_js(true)
        .js_wait_strategy(WaitStrategy::Auto)
        .build();
    let js_results = Rehyke::new(js_config).run("https://httpbin.org/html").await;

    println!("  Static fetch:");
    if let Some(r) = static_results.first() {
        println!("    Method: {:?}", r.render_method);
        println!("    Words:  {}", r.markdown.split_whitespace().count());
        println!("    Status: {}", r.status_code);
    }

    println!("  JS render:");
    match js_results {
        Ok(ref results) => {
            if let Some(r) = results.first() {
                println!("    Method: {:?}", r.render_method);
                println!("    Words:  {}", r.markdown.split_whitespace().count());
                println!("    Status: {}", r.status_code);
            }
        }
        Err(ref e) => {
            println!("    [WARN] JS unavailable: {}", e);
            println!("    (Chrome not installed or `js` feature not enabled)");
        }
    }
    println!();

    // =========================================================================
    // Section 6: Configuration quick-reference
    // =========================================================================
    println!("--- Configuration Reference ---\n");
    println!("Rust builder API:");
    println!(r#"  CrawlConfigBuilder::new()
      .enable_js(true)
      .js_wait_strategy(WaitStrategy::NetworkIdle)   // NetworkIdle | Selector | Duration | Auto
      .js_wait_timeout(Duration::from_secs(10))
      .js_scroll_count(5)                             // scroll N viewports for infinite scroll
      .dismiss_popups(true)                           // dismiss cookie/GDPR popups
      .viewport(Viewport::Desktop)                    // Desktop | Tablet | Mobile
      .screenshot(true)                               // capture full-page PNG
      .screenshot_format(ScreenshotFormat::Png)       // Png | Jpeg
      .screenshot_output_dir("/tmp/shots")
      .detect_spa(true)                               // React | Vue | Angular | Svelte | …
      .randomize_fingerprint(true)                    // randomise viewport, WebGL, languages
      .build()"#);
    println!();
    println!("CLI equivalent:");
    println!("  rehyke https://spa-app.com \\");
    println!("      --js \\");
    println!("      --wait-for '.content-loaded' \\");
    println!("      --scroll 10 \\");
    println!("      --dismiss-popups \\");
    println!("      --screenshot --screenshot-format png \\");
    println!("      --viewport mobile \\");
    println!("      --detect-spa \\");
    println!("      --randomize-fingerprint");
    println!();
    println!("=== Done ===");
    Ok(())
}
