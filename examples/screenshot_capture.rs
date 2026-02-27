//! Screenshot Capture Example (v0.2.0)
//!
//! Demonstrates Rehyke's screenshot capabilities powered by the headless
//! Chrome renderer:
//!
//!   - Full-page screenshots in PNG and JPEG formats
//!   - Element-level screenshots via CSS selector (cropping)
//!   - Multiple viewport profiles side-by-side (desktop / tablet / mobile)
//!   - Screenshot output directory configuration
//!   - Reading screenshot metadata from `CrawlConfig`
//!
//! # Requirements
//!
//! ```bash
//! cargo run --example screenshot_capture --features js
//! ```
//!
//! Without the `js` feature or without Chrome installed the example still
//! runs and demonstrates the configuration API — screenshots are simply
//! skipped with a warning.

use std::path::PathBuf;
use std::time::Duration;

use rehyke_core::{
    CrawlConfigBuilder, Rehyke, ScanMode, ScreenshotFormat, Viewport, WaitStrategy,
};

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

    println!("=== Rehyke Screenshot Capture Example (v0.2.0) ===\n");

    // Create a directory where screenshots will be saved.
    let output_dir = PathBuf::from("/tmp/rehyke_screenshots");
    std::fs::create_dir_all(&output_dir)?;
    println!("Screenshots will be saved to: {}\n", output_dir.display());

    let target_url = "https://httpbin.org/html";

    // =========================================================================
    // Section 1: Full-page PNG screenshot — Desktop viewport
    // =========================================================================
    println!("--- 1. Full-page PNG screenshot (Desktop 1920×1080) ---\n");

    let config_desktop_png = CrawlConfigBuilder::new()
        .mode(ScanMode::Lite)
        .enable_js(true)
        .js_wait_strategy(WaitStrategy::Auto)
        .js_wait_timeout(Duration::from_secs(10))
        // Enable screenshot capture.
        .screenshot(true)
        // PNG — lossless, larger files, ideal for visual diffing.
        .screenshot_format(ScreenshotFormat::Png)
        // Directory where the PNG file will be written.
        .screenshot_output_dir(output_dir.join("desktop"))
        // Desktop viewport.
        .viewport(Viewport::Desktop)
        .build();

    let (w, h) = config_desktop_png.viewport.dimensions();
    println!("  Viewport:   {}×{} ({})", w, h,
        if config_desktop_png.viewport.is_mobile() { "mobile" } else { "desktop" });
    println!("  Format:     {:?}", config_desktop_png.screenshot_format);
    println!("  Output dir: {}", output_dir.join("desktop").display());
    println!();

    let crawler = Rehyke::new(config_desktop_png);
    match crawler.run(target_url).await {
        Ok(results) => {
            println!("  Crawled {} page(s)", results.len());
            for r in &results {
                println!("  -> {} [{:?}] {} words", r.url, r.render_method,
                    r.markdown.split_whitespace().count());
            }
        }
        Err(e) => {
            println!("  [WARN] Screenshot skipped — Chrome unavailable or `js` feature not enabled.");
            println!("  Error: {}", e);
        }
    }
    println!();

    // =========================================================================
    // Section 2: Full-page JPEG screenshot — Tablet viewport
    // =========================================================================
    println!("--- 2. Full-page JPEG screenshot (Tablet 768×1024) ---\n");

    let config_tablet_jpeg = CrawlConfigBuilder::new()
        .mode(ScanMode::Lite)
        .enable_js(true)
        .js_wait_strategy(WaitStrategy::NetworkIdle)
        .js_wait_timeout(Duration::from_secs(8))
        .screenshot(true)
        // JPEG — smaller files, lossy, faster to transfer.  Good for archival.
        .screenshot_format(ScreenshotFormat::Jpeg)
        .screenshot_output_dir(output_dir.join("tablet"))
        .viewport(Viewport::Tablet)
        .build();

    let (tw, th) = config_tablet_jpeg.viewport.dimensions();
    println!("  Viewport:   {}×{} (touch={})", tw, th, config_tablet_jpeg.viewport.has_touch());
    println!("  Format:     JPEG (lossy, ~90% quality)");
    println!();

    let crawler2 = Rehyke::new(config_tablet_jpeg);
    match crawler2.run(target_url).await {
        Ok(results) => {
            println!("  Crawled {} page(s)", results.len());
        }
        Err(e) => {
            println!("  [WARN] {}", e);
        }
    }
    println!();

    // =========================================================================
    // Section 3: Mobile screenshot — simulating an iPhone viewport
    // =========================================================================
    println!("--- 3. Mobile viewport screenshot (iPhone 390×844, 3× DPR) ---\n");

    let config_mobile = CrawlConfigBuilder::new()
        .mode(ScanMode::Lite)
        .enable_js(true)
        .js_wait_strategy(WaitStrategy::Auto)
        .js_wait_timeout(Duration::from_secs(10))
        .screenshot(true)
        .screenshot_format(ScreenshotFormat::Png)
        .screenshot_output_dir(output_dir.join("mobile"))
        // Mobile — 390×844, 3× device pixel ratio, touch enabled.
        // The browser will request the `@3x` or `srcset` image variants.
        .viewport(Viewport::Mobile)
        .randomize_fingerprint(true)
        .build();

    let (mw, mh) = config_mobile.viewport.dimensions();
    let dpr = config_mobile.viewport.device_scale_factor();
    println!("  Viewport:   {}×{} CSS pixels ({}× DPR → {}×{} physical px)",
        mw, mh, dpr,
        (mw as f64 * dpr) as u32,
        (mh as f64 * dpr) as u32);
    println!("  Touch:      {}", config_mobile.viewport.has_touch());
    println!("  Fingerprint: randomised");
    println!();

    let crawler3 = Rehyke::new(config_mobile);
    match crawler3.run(target_url).await {
        Ok(results) => {
            println!("  Crawled {} page(s)", results.len());
        }
        Err(e) => {
            println!("  [WARN] {}", e);
        }
    }
    println!();

    // =========================================================================
    // Section 4: Screenshot during multi-page crawl
    // =========================================================================
    println!("--- 4. Screenshot during a multi-page crawl ---\n");

    let multi_page_config = CrawlConfigBuilder::new()
        .mode(ScanMode::Lite)
        .max_pages(3)
        .max_depth(2)
        .enable_js(true)
        .js_wait_strategy(WaitStrategy::Auto)
        .js_wait_timeout(Duration::from_secs(8))
        // Each page gets its own screenshot file:
        //   {screenshot_output_dir}/{url-slug}.png
        .screenshot(true)
        .screenshot_format(ScreenshotFormat::Png)
        .screenshot_output_dir(output_dir.join("multi"))
        .viewport(Viewport::Desktop)
        .build();

    println!("  Config: max_pages=3  depth=2  screenshot=true (one PNG per page)");
    println!("  Output: {}", output_dir.join("multi").display());
    println!();

    let crawler4 = Rehyke::new(multi_page_config);
    match crawler4.run("https://httpbin.org/html").await {
        Ok(results) => {
            println!("  Crawled {} page(s)", results.len());
            for r in &results {
                println!("    [{}] {} — {} words", r.status_code, r.url,
                    r.markdown.split_whitespace().count());
            }
        }
        Err(e) => {
            println!("  [WARN] {}", e);
        }
    }
    println!();

    // =========================================================================
    // Section 5: Screenshot configuration reference
    // =========================================================================
    println!("--- Screenshot Configuration Reference ---\n");

    println!("Rust builder API:");
    println!(r#"  CrawlConfigBuilder::new()
      // Enable screenshot capture (requires enable_js=true).
      .screenshot(true)

      // Image format: PNG (lossless) or JPEG (smaller, lossy).
      .screenshot_format(ScreenshotFormat::Png)

      // Directory where screenshots are written.
      // Files are named:  {url-slug}.png
      .screenshot_output_dir("/tmp/screenshots")

      // Viewport determines the rendered page size.
      .viewport(Viewport::Desktop)   // 1920×1080
      .viewport(Viewport::Tablet)    // 768×1024
      .viewport(Viewport::Mobile)    // 390×844, touch, 3× DPR

      .build()"#);
    println!();

    println!("CLI:");
    println!("  # Desktop PNG screenshot");
    println!("  rehyke https://example.com --js --screenshot --screenshot-format png \\");
    println!("      --screenshot-dir ./shots --viewport desktop");
    println!();
    println!("  # Mobile JPEG screenshot");
    println!("  rehyke https://example.com --js --screenshot --screenshot-format jpeg \\");
    println!("      --screenshot-dir ./shots --viewport mobile");
    println!();

    println!("Python:");
    println!(r#"  import rehyke

  config = rehyke.CrawlConfig(
      enable_js=True,
      screenshot=True,
      screenshot_format="png",          # "png" or "jpeg"
      screenshot_dir="/tmp/screenshots",
      viewport="desktop",               # "desktop" | "tablet" | "mobile"
  )

  crawler = rehyke.Rehyke(config)
  results = crawler.crawl("https://example.com")"#);
    println!();

    // =========================================================================
    // Section 6: List saved files
    // =========================================================================
    println!("--- Files written to {} ---\n", output_dir.display());

    if output_dir.exists() {
        let mut total = 0usize;
        for entry in walkdir_simple(&output_dir) {
            println!("  {}", entry.display());
            total += 1;
        }
        if total == 0 {
            println!("  (no screenshots saved — Chrome was not available)");
        }
    }
    println!();

    println!("=== Done ===");
    Ok(())
}

/// Simple recursive directory listing (without a walkdir dependency).
fn walkdir_simple(dir: &PathBuf) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(walkdir_simple(&path));
            } else {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}
