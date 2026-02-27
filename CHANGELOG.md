# Changelog

All notable changes to the Rehyke project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-02-27

### Added -- Headless Browser Integration ("Chrome Eyes")

- **Headless Chromium renderer** via `chromiumoxide` (Chrome DevTools Protocol)
  - Optional `js` Cargo feature keeps the crate buildable without Chrome installed
  - Graceful runtime fallback to static HTTP fetch when Chrome is unavailable
  - Tab pooling with `Arc<Mutex<Browser>>` for safe concurrent rendering

- **JavaScript wait strategies** — `WaitStrategy` enum with four variants:
  - `NetworkIdle` — wait until no pending network requests remain (best for SPAs)
  - `Selector { selector }` — poll until a CSS selector appears in the DOM
  - `Duration { duration }` — fixed settle time after initial render
  - `Auto` — heuristic combining network idle and a short fixed delay

- **SPA framework detection** — `SpaFramework` enum identifies React, Vue, Angular,
  Svelte, Next.js, Nuxt, and SvelteKit from DOM signals and global JS objects

- **Infinite-scroll support** — `js_scroll_count` scrolls the page N viewport-heights
  after initial render, triggering Intersection-Observer pagination; stops early when
  the page bottom is reached

- **Popup & overlay dismissal** — `dismiss_popups` heuristic targets:
  - 30+ CSS selectors for OneTrust, CookieBot, CookieYes, and generic CMP providers
  - Attribute patterns (`[id*="cookie"][id*="accept"]`, `[class*="consent"]`)
  - ARIA labels (`Accept cookies`, `Accept all`, `Agree`)
  - JavaScript fallback scanning visible buttons for acceptance text

- **Browser fingerprint diversity** — `randomize_fingerprint` injects a
  `Page.addScriptToEvaluateOnNewDocument` script that randomises:
  - Viewport dimensions (within profile range)
  - `navigator.userAgent`, `navigator.platform`, `navigator.languages`
  - WebGL vendor and renderer strings
  - Timezone and UTC offset
  - Canvas noise (sub-pixel pixel-value perturbation)
  - `navigator.webdriver` hidden flag

- **Browser viewport profiles** — `Viewport` enum:
  - `Desktop` — 1920 × 1080, standard DPR
  - `Tablet` — 768 × 1024, touch enabled
  - `Mobile` — 390 × 844, 3× device pixel ratio, touch enabled

- **Screenshot & visual capture** — `screenshot` + `screenshot_format`:
  - Full-page screenshots in PNG (lossless) or JPEG (lossy, ~90% quality)
  - Per-page files named by URL slug: `{output_dir}/{url-slug}.{ext}`
  - Configurable output directory via `screenshot_output_dir`

### Added -- CLI (`rehyke-cli`)

New flags for headless rendering:

```
--js                         Enable JavaScript rendering
--wait-for <SELECTOR>        Wait for a CSS selector before extracting (implies --js)
--js-timeout <SECS>          JS wait timeout in seconds (default 10)
--scroll <N>                 Scroll N viewports for infinite scroll (implies --js)
--dismiss-popups             Dismiss cookie/GDPR popups (implies --js)
--screenshot                 Capture a full-page screenshot (implies --js)
--screenshot-format png|jpeg Image format (default png)
--screenshot-dir <DIR>       Directory to write screenshots into
--viewport desktop|tablet|mobile  Browser viewport profile (default desktop)
--detect-spa                 Auto-detect SPA framework (implies --js)
--randomize-fingerprint      Randomise browser fingerprint (implies --js)
```

### Added -- Python bindings (`rehyke-python`)

`CrawlConfig` now accepts ten new keyword arguments:

```python
config = rehyke.CrawlConfig(
    enable_js=True,
    js_wait_strategy="network_idle",  # "auto" | "network_idle" | "selector:<CSS>" | float
    js_wait_timeout=10.0,
    scroll_count=5,
    dismiss_popups=True,
    viewport="desktop",               # "desktop" | "tablet" | "mobile"
    screenshot=True,
    screenshot_format="png",          # "png" | "jpeg"
    screenshot_dir="/tmp/shots",
    detect_spa=True,
    randomize_fingerprint=True,
)
```

`CrawlResult` exposes two new fields:
- `render_method` — `"static"` or `"javascript"`
- `depth` — crawl depth at which the page was discovered

### Added -- Examples

- `examples/js_render.rs` — JS rendering with all four wait strategies, static vs JS
  comparison, and full configuration reference
- `examples/spa_crawl.rs` — Framework-specific config profiles (React/Vue/Angular),
  route pattern analysis, infinite scroll, popup dismissal, parallel multi-config crawl
- `examples/screenshot_capture.rs` — Full-page screenshots at desktop/tablet/mobile
  viewports, per-page screenshots during multi-page crawls, file listing helper
- `examples/python_js_render.py` — Python equivalent covering all v0.2.0 features

### Changed

- `CrawlConfig` extended with 9 new fields: `viewport`, `js_scroll_count`,
  `dismiss_popups`, `screenshot`, `screenshot_format`, `screenshot_output_dir`,
  `js_wait_timeout`, `detect_spa`, `randomize_fingerprint`
- `CrawlConfigBuilder` extended with 9 corresponding fluent builder methods
- `renderer.rs` rewritten: stub replaced with full `chromiumoxide` implementation
- `lib.rs` crawl loop updated to initialise the JS renderer and route pages through it
  when `enable_js = true`
- `fetcher.rs` — added `detect_content_type_from_str` for renderer integration
- `utils.rs` — added `parse_url_lossy` helper

### Removed

- "Headless Chromium integration (renderer.rs stub ready)" from the Planned section
  of v0.1.0 — it is now fully implemented

---

## [0.1.0] - 2025-02-24

### Added -- Core Engine

- Full crawl engine with 14 modules and 11,670 lines of Rust
- Three scan modes: Lite (single page), Full (domain-wide), Deep (cross-domain)
- CrawlConfig with builder pattern and 25+ configurable options
- HTTP fetcher with reqwest: HTTP/2, gzip/brotli/zstd, cookies, proxy support
- Retry with exponential backoff and Retry-After header support
- Content-type auto-detection (headers -> URL extension -> body sniffing)

### Added -- Content Processing

- Universal parser: HTML, XHTML, XML, RSS 2.0, Atom, JSON, JSON-LD, SVG, Sitemap, Plain Text
- HTML-to-Markdown converter with 18 element types and GFM table support
- YAML frontmatter generation with page metadata
- Content cleaning: scripts, styles, nav, headers, footers, ads, comments
- Link extractor scanning 12+ HTML element types

### Added -- Crawl Intelligence

- Priority-based URL scheduler with BinaryHeap
- URL normalization with 7 rules for deduplication
- Per-domain rate limiting with DashMap timestamps
- robots.txt parser with wildcard and $ anchor support
- Sitemap XML parser (urlset and sitemapindex)

### Added -- Anti-Detection

- 57 rotating user agents (Chrome, Firefox, Safari, Edge)
- Realistic browser header generation
- Delay strategies: Fixed, Random, Adaptive, None
- Proxy pool with Round-Robin, Random, LeastUsed, FailoverOnly

### Added -- Distribution

- CLI binary (rehyke-cli) with clap: 20+ options, progress bar, JSON output
- Python bindings (PyO3): crawl(), Rehyke class, CrawlConfig, ScanMode
- pyproject.toml with maturin build system

### Added -- Quality

- 369 unit tests across all modules
- Comprehensive error handling with 14 error variants
- Structured logging with tracing

### Planned

- Autonomous crawl planner
- Regex rule engine for custom extraction
- Distributed crawling support

---

*For unreleased changes, see the [commit log](https://github.com/vrinda/rehyke/commits/main).*
