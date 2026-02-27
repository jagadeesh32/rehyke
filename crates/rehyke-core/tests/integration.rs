//! Integration tests for rehyke-core (v0.2.0)
//!
//! These tests exercise the full crawl pipeline end-to-end.
//!
//! Tests that require live network access are gated behind the `REHYKE_LIVE`
//! environment variable so CI stays fast and offline-friendly:
//!
//! ```bash
//! # Run only offline tests (default):
//! cargo test --package rehyke-core --test integration
//!
//! # Run all tests including live-network tests:
//! REHYKE_LIVE=1 cargo test --package rehyke-core --test integration
//! ```

use rehyke_core::{
    CrawlConfigBuilder, Rehyke, RehykeError, ScanMode, ScreenshotFormat, Viewport, WaitStrategy,
};
use std::path::PathBuf;
use std::time::Duration;

/// Returns `true` when live network tests are enabled.
fn live() -> bool {
    std::env::var("REHYKE_LIVE").is_ok()
}

// ---------------------------------------------------------------------------
// URL validation — no network required
// ---------------------------------------------------------------------------

#[tokio::test]
async fn run_with_invalid_url_returns_config_error() {
    let crawler = Rehyke::new(CrawlConfigBuilder::new().mode(ScanMode::Lite).build());
    let err = crawler.run(":::bad url").await.unwrap_err();

    assert!(
        matches!(err, RehykeError::ConfigError { .. }),
        "expected ConfigError, got {:?}",
        err
    );
}

#[tokio::test]
async fn run_with_empty_url_returns_error() {
    let crawler = Rehyke::new(CrawlConfigBuilder::new().mode(ScanMode::Lite).build());
    assert!(crawler.run("").await.is_err());
}

#[tokio::test]
async fn run_with_relative_url_returns_error() {
    let crawler = Rehyke::new(CrawlConfigBuilder::new().mode(ScanMode::Lite).build());
    assert!(crawler.run("/relative/path").await.is_err());
}

// ---------------------------------------------------------------------------
// Pattern compilation — no network required
// ---------------------------------------------------------------------------

#[tokio::test]
async fn invalid_exclude_pattern_returns_config_error() {
    let cfg = CrawlConfigBuilder::new()
        .mode(ScanMode::Lite)
        .exclude_patterns(vec!["[invalid regex".to_string()])
        .build();

    let crawler = Rehyke::new(cfg);
    // Patterns are compiled during run(); the invalid pattern triggers an error
    // before any network access.
    let err = crawler.run("https://example.com").await.unwrap_err();
    assert!(
        matches!(err, RehykeError::ConfigError { .. }),
        "expected ConfigError from bad regex, got {:?}",
        err
    );
}

#[tokio::test]
async fn invalid_include_pattern_returns_config_error() {
    let cfg = CrawlConfigBuilder::new()
        .mode(ScanMode::Lite)
        .include_patterns(vec!["[also bad".to_string()])
        .build();

    let crawler = Rehyke::new(cfg);
    let err = crawler.run("https://example.com").await.unwrap_err();
    assert!(matches!(err, RehykeError::ConfigError { .. }));
}

// ---------------------------------------------------------------------------
// Config construction — no network required
// ---------------------------------------------------------------------------

#[test]
fn crawl_config_viewport_builder_methods() {
    let cfg = CrawlConfigBuilder::new()
        .viewport(Viewport::Mobile)
        .screenshot(true)
        .screenshot_format(ScreenshotFormat::Jpeg)
        .screenshot_output_dir(PathBuf::from("/tmp"))
        .js_scroll_count(5)
        .dismiss_popups(true)
        .detect_spa(true)
        .randomize_fingerprint(true)
        .js_wait_timeout(Duration::from_secs(20))
        .js_wait_strategy(WaitStrategy::NetworkIdle)
        .build();

    assert_eq!(cfg.viewport, Viewport::Mobile);
    assert!(cfg.screenshot);
    assert_eq!(cfg.screenshot_format, ScreenshotFormat::Jpeg);
    assert_eq!(cfg.screenshot_output_dir, Some(PathBuf::from("/tmp")));
    assert_eq!(cfg.js_scroll_count, 5);
    assert!(cfg.dismiss_popups);
    assert!(cfg.detect_spa);
    assert!(cfg.randomize_fingerprint);
    assert_eq!(cfg.js_wait_timeout, Duration::from_secs(20));
    assert!(matches!(cfg.js_wait_strategy, WaitStrategy::NetworkIdle));
}

#[test]
fn crawl_config_selector_wait_strategy() {
    let cfg = CrawlConfigBuilder::new()
        .js_wait_strategy(WaitStrategy::Selector {
            selector: "#nuxt-loading".to_string(),
        })
        .build();

    assert!(matches!(
        cfg.js_wait_strategy,
        WaitStrategy::Selector { ref selector } if selector == "#nuxt-loading"
    ));
}

#[test]
fn crawl_config_duration_wait_strategy() {
    let cfg = CrawlConfigBuilder::new()
        .js_wait_strategy(WaitStrategy::Duration {
            duration: Duration::from_millis(800),
        })
        .build();

    assert!(matches!(
        cfg.js_wait_strategy,
        WaitStrategy::Duration { duration } if duration == Duration::from_millis(800)
    ));
}

#[test]
fn crawl_config_scan_mode_lite_defaults() {
    let cfg = CrawlConfigBuilder::new().mode(ScanMode::Lite).build();
    assert_eq!(cfg.max_depth, 2);
    assert_eq!(cfg.max_pages, 100);
    assert_eq!(cfg.concurrency, 5);
}

#[test]
fn crawl_config_scan_mode_deep_defaults() {
    let cfg = CrawlConfigBuilder::new().mode(ScanMode::Deep).build();
    assert_eq!(cfg.max_depth, 50);
    assert_eq!(cfg.max_pages, 50_000);
    assert_eq!(cfg.concurrency, 25);
}

// ---------------------------------------------------------------------------
// Live crawl tests — only run when REHYKE_LIVE=1
// ---------------------------------------------------------------------------

/// Crawl a well-known stable static URL and verify basic result fields.
#[tokio::test]
async fn live_static_crawl_returns_results() {
    if !live() {
        return; // skip in offline / CI environments
    }

    let cfg = CrawlConfigBuilder::new()
        .mode(ScanMode::Lite)
        .respect_robots_txt(false)
        .build();

    let results = Rehyke::new(cfg)
        .run("https://httpbin.org/html")
        .await
        .expect("static crawl should succeed");

    assert!(!results.is_empty(), "expected at least one crawl result");

    let page = &results[0];
    assert!(!page.url.is_empty());
    assert!(page.status_code == 200, "expected 200 OK, got {}", page.status_code);
    assert!(!page.markdown.is_empty(), "markdown should be non-empty");
    assert!(
        matches!(page.render_method, rehyke_core::output::RenderMethod::Static),
        "expected Static render method, got {:?}",
        page.render_method
    );
    assert_eq!(page.depth, 0, "seed page should be at depth 0");
}

/// Verify `max_pages` is respected during a multi-page crawl.
#[tokio::test]
async fn live_max_pages_limits_result_count() {
    if !live() {
        return;
    }

    let cfg = CrawlConfigBuilder::new()
        .mode(ScanMode::Full)
        .max_pages(2)
        .max_depth(3)
        .respect_robots_txt(false)
        .build();

    let results = Rehyke::new(cfg)
        .run("https://httpbin.org/html")
        .await
        .expect("crawl should succeed");

    assert!(
        results.len() <= 2,
        "max_pages=2 should cap results at 2, got {}",
        results.len()
    );
}

/// Verify that an exclude pattern filters out matching URLs.
#[tokio::test]
async fn live_exclude_pattern_filters_urls() {
    if !live() {
        return;
    }

    // Exclude everything — seed URL itself won't match /html but internal links
    // to /anything will be excluded.
    let cfg = CrawlConfigBuilder::new()
        .mode(ScanMode::Lite)
        .respect_robots_txt(false)
        .exclude_patterns(vec![r"\.png$".to_string(), r"/image.*".to_string()])
        .build();

    let results = Rehyke::new(cfg)
        .run("https://httpbin.org/html")
        .await
        .expect("crawl should succeed");

    // No result URL should contain .png or /image.
    for r in &results {
        assert!(
            !r.url.ends_with(".png"),
            "excluded .png URL appeared in results: {}",
            r.url
        );
        assert!(
            !r.url.contains("/image"),
            "excluded /image URL appeared in results: {}",
            r.url
        );
    }
}

/// With robots.txt disabled the crawl should not be blocked.
#[tokio::test]
async fn live_respect_robots_txt_false_does_not_block() {
    if !live() {
        return;
    }

    let cfg = CrawlConfigBuilder::new()
        .mode(ScanMode::Lite)
        .respect_robots_txt(false)
        .build();

    let results = Rehyke::new(cfg)
        .run("https://httpbin.org/html")
        .await
        .expect("crawl should succeed even if robots.txt would block it");

    assert!(!results.is_empty());
}

/// When JS is enabled but Chrome is not installed, the crawl should fall back
/// to static fetch and still return results (no panic or fatal error).
#[tokio::test]
async fn live_js_enabled_without_chrome_falls_back_to_static() {
    if !live() {
        return;
    }

    let cfg = CrawlConfigBuilder::new()
        .mode(ScanMode::Lite)
        .enable_js(true)
        .js_wait_strategy(WaitStrategy::Auto)
        .js_wait_timeout(Duration::from_secs(5))
        .respect_robots_txt(false)
        .build();

    // This will either render with Chrome (if installed) or fall back to static.
    // Either way it should not panic and should return a non-empty result set.
    let results = Rehyke::new(cfg)
        .run("https://httpbin.org/html")
        .await
        .expect("crawl should succeed with or without Chrome");

    assert!(!results.is_empty());
}

/// The one-shot `Rehyke::crawl` convenience function should work the same as
/// constructing a crawler manually.
#[tokio::test]
async fn live_crawl_shorthand_returns_results() {
    if !live() {
        return;
    }

    let results = Rehyke::crawl("https://httpbin.org/html", ScanMode::Lite)
        .await
        .expect("one-shot crawl should succeed");

    assert!(!results.is_empty());
    assert!(results[0].status_code == 200);
}
