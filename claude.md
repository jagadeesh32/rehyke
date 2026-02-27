# CLAUDE.md — Rehyke: Advanced Web Crawler

## Project Identity

**Name:** Rehyke
**Tagline:** "Crawl Everything. Miss Nothing."
**Version:** 0.2.0 "Chrome Eyes"
**Language:** Rust core + Python bindings (PyO3/maturin)
**Distribution:** crates.io (Rust) + PyPI (Python)
**License:** MIT OR Apache-2.0
**Test Count:** 473 (444 unit + 16 integration + 13 doctests)

---

## 1. Project Overview

Rehyke is an ultra-high-performance, JavaScript-rendering web crawler built in Rust with first-class Python bindings. It crawls any website — including JS-heavy SPAs (React, Vue, Angular, Svelte), XML sitemaps, RSS feeds, and arbitrary markup — and outputs clean, structured Markdown.

It is designed to be **faster, more resilient, and more capable** than crawl4ai, Scrapy, or any existing crawl tool.

### Core Differentiators
- **Rust-native performance** with zero-cost Python interop via PyO3
- **Headless Chromium** integration for full JavaScript rendering (optional `js` feature flag)
- **Three scan modes**: Lite, Full, Deep — user controls depth/breadth
- **Concurrent architecture**: tokio async runtime + work-stealing task pool
- **Anti-detection**: rotating user agents, request throttling, proxy support, TLS fingerprint randomization
- **Browser fingerprint diversity**: randomised WebGL, languages, timezone, canvas noise (v0.2.0)
- **Universal parser**: HTML, XHTML, XML, RSS, Atom, JSON-LD, SVG, sitemap.xml
- **Output flexibility**: return Markdown strings or save to `.md` files (user's choice)
- **Screenshots**: optional PNG/JPEG capture per page (v0.2.0)
- **SPA detection**: auto-detect React, Vue, Angular, Svelte, Next.js, Nuxt, SvelteKit (v0.2.0)
- **Infinite scroll**: configurable JS scroll simulation (v0.2.0)
- **Popup dismissal**: auto-dismiss cookie/GDPR banners (v0.2.0)

---

## 2. Architecture

### 2.1 High-Level Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Python API (PyO3)                     │
│  rehyke.crawl(url, mode="full", enable_js=True)         │
├─────────────────────────────────────────────────────────┤
│                   Rust Core Engine                       │
│                                                         │
│  ┌──────────┐  ┌──────────┐  ┌────────────────────┐    │
│  │ Scheduler│──│  Fetcher  │──│     Renderer        │    │
│  │ (tokio)  │  │(reqwest + │  │ Static (reqwest) OR │    │
│  │          │  │ hyper)    │  │ JS (chromiumoxide)  │    │
│  └────┬─────┘  └────┬─────┘  └─────────┬──────────┘    │
│       │              │                  │                 │
│  ┌────▼──────────────▼──────────────────▼─────┐         │
│  │           Link Extractor                   │         │
│  │   (scraper + select.rs + custom XML)       │         │
│  └────────────────┬───────────────────────────┘         │
│                   │                                      │
│  ┌────────────────▼───────────────────────┐             │
│  │        Content Processor               │             │
│  │  HTML→MD | XML→MD | JSON→MD | RSS→MD   │             │
│  └────────────────┬───────────────────────┘             │
│                   │                                      │
│  ┌────────────────▼───────────────────────┐             │
│  │         Output Handler                 │             │
│  │   Return String | Write .md files      │             │
│  └────────────────────────────────────────┘             │
└─────────────────────────────────────────────────────────┘
```

### 2.2 Module Structure

```
rehyke/
├── Cargo.toml                    # Workspace root (version = "0.2.0")
├── pyproject.toml                # Python package config (maturin)
├── README.md
├── ROADMAP.md
├── CHANGELOG.md
├── CONTRIBUTING.md
├── LICENSE-MIT
├── LICENSE-APACHE
├── claude.md                     # This file
│
├── crates/
│   ├── rehyke-core/              # Core crawler engine
│   │   ├── Cargo.toml            # features = ["js"] for Chrome
│   │   ├── src/
│   │   │   ├── lib.rs            # Public API (Rehyke, CrawlConfigBuilder)
│   │   │   ├── config.rs         # CrawlConfig, ScanMode, Viewport, WaitStrategy, ...
│   │   │   ├── scheduler.rs      # URL frontier, priority queue, dedup
│   │   │   ├── fetcher.rs        # HTTP client (reqwest), retry, proxy
│   │   │   ├── renderer.rs       # Static + JS renderer (chromiumoxide) with SPA/scroll/popup
│   │   │   ├── browser_fingerprint.rs  # Randomised browser fingerprint injection (v0.2.0)
│   │   │   ├── extractor.rs      # Link extraction (all formats)
│   │   │   ├── parser.rs         # HTML/XML/RSS/Atom/JSON-LD parsing
│   │   │   ├── converter.rs      # Content → Markdown conversion
│   │   │   ├── robots.rs         # robots.txt parser (optional respect)
│   │   │   ├── sitemap.rs        # sitemap.xml parser and crawler
│   │   │   ├── anti_detect.rs    # UA rotation, fingerprinting, delays
│   │   │   ├── proxy.rs          # Proxy pool management
│   │   │   ├── output.rs         # File writer / string collector, CrawlResult
│   │   │   ├── error.rs          # RehykeError enum
│   │   │   └── utils.rs          # URL normalization, helpers
│   │   └── tests/
│   │       └── integration.rs    # Integration tests (offline + REHYKE_LIVE=1 gated)
│   │
│   ├── rehyke-cli/               # CLI binary
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── main.rs           # clap-based CLI
│   │
│   └── rehyke-python/            # Python bindings
│       ├── Cargo.toml
│       └── src/
│           └── lib.rs            # PyO3 module
│
├── python/
│   └── rehyke/
│       ├── __init__.py           # Python package init
│       └── py.typed              # PEP 561 marker
│
├── examples/
│   ├── basic_crawl.rs            # Simple static crawl
│   ├── deep_crawl.rs             # Deep scan with all options
│   ├── content_pipeline.rs       # Content extraction pipeline
│   ├── regex_extraction.rs       # Pattern-based extraction
│   ├── js_render.rs              # Headless Chrome JS rendering (v0.2.0)
│   ├── spa_crawl.rs              # SPA detection and crawling (v0.2.0)
│   ├── screenshot_capture.rs     # Screenshot capture (v0.2.0)
│   ├── python_basic.py
│   ├── python_advanced.py
│   ├── python_pipeline.py
│   ├── python_regex_extract.py
│   └── python_js_render.py       # Python JS rendering examples (v0.2.0)
│
└── benches/
    └── crawl_benchmark.rs
```

---

## 3. Scan Modes

**Important:** JS rendering is **off by default** for all modes. Enable it explicitly with `enable_js(true)` in Rust, `enable_js=True` in Python, or `--js` in CLI.

### 3.1 Lite Scan
- **Scope:** Single page only (the given URL, no link following)
- **JS Rendering:** Off by default (opt-in)
- **Default max_depth:** 2
- **Default max_pages:** 100
- **Default concurrency:** 5
- **Speed:** Fastest — minimal requests
- **Use case:** Quick content extraction from a known page

### 3.2 Full Scan
- **Scope:** Given domain — all internal links (same domain/subdomain)
- **JS Rendering:** Off by default (opt-in)
- **Default max_depth:** 10
- **Default max_pages:** 10,000
- **Default concurrency:** 10
- **Speed:** Fast — parallel crawling with dedup
- **Use case:** Complete site extraction

### 3.3 Deep Scan
- **Scope:** Given domain + ALL discovered external links
- **JS Rendering:** Off by default (opt-in)
- **Default max_depth:** 50
- **Default max_pages:** 50,000
- **Default concurrency:** 25
- **Speed:** Thorough — maximum coverage
- **Use case:** Research, competitive analysis, full web graph extraction

---

## 4. Core Components — Detailed Specifications

### 4.1 Fetcher (`fetcher.rs`)

```rust
pub struct Fetcher {
    client: reqwest::Client,
    proxy_pool: ProxyPool,
    ua_rotator: UserAgentRotator,
    rate_limiter: RateLimiter,
    retry_config: RetryConfig,
}

pub struct FetchResult {
    pub url: Url,
    pub status: u16,
    pub headers: HeaderMap,
    pub body: String,              // Raw response body
    pub content_type: ContentType, // HTML, XML, JSON, RSS, etc.
    pub elapsed: Duration,
    pub final_url: Url,            // After redirects
}
```

**Requirements:**
- HTTP/1.1 and HTTP/2 support
- Automatic redirect following (max 10 hops)
- Gzip, Brotli, Deflate, Zstd decompression
- Connection pooling per domain
- Configurable timeouts (connect: 10s, read: 30s, total: 60s)
- Cookie jar support (per-domain persistence)
- Custom headers injection
- TLS 1.2/1.3 with configurable cipher suites
- Retry with exponential backoff (3 attempts default)

### 4.2 Renderer (`renderer.rs`) — v0.2.0

Two rendering paths:
1. **Static** (default): `reqwest` fetch → HTML string
2. **JavaScript** (`enable_js = true`, requires `--features js`): `chromiumoxide` CDP → rendered DOM

```rust
// Actual WaitStrategy enum as implemented:
pub enum WaitStrategy {
    NetworkIdle,               // Wait for network to be quiet (duration from js_wait_timeout)
    Selector { selector: String }, // Wait for CSS selector to appear in DOM
    Duration { duration: Duration }, // Fixed wait time
    Auto,                      // Smart detection based on SPA framework
}

// Actual Viewport enum:
pub enum Viewport {
    Desktop,   // 1920×1080, DPR 1.0
    Tablet,    // 768×1024,  DPR 2.0
    Mobile,    // 390×844,   DPR 3.0
}

// SPA detection result:
pub enum SpaFramework {
    React, Vue, Angular, Svelte, NextJs, Nuxt, SvelteKit, Unknown,
}
```

**v0.2.0 renderer capabilities:**
- Tab pooling: `Arc<Mutex<Browser>>` for concurrent Chrome tab reuse
- SPA detection: JS expressions per framework injected via CDP (`SPA_DETECTORS` constant)
- Infinite scroll: configurable `js_scroll_count` iterations with `window.scrollBy` + wait
- Popup dismissal: `POPUP_ACCEPT_SELECTORS` — 30+ CSS selectors for cookie/GDPR banners
- Browser fingerprint injection: `BrowserFingerprint` struct with randomised WebGL, UA, languages, timezone, canvas noise
- Screenshots: PNG or JPEG, saved to `screenshot_output_dir`
- Viewport emulation: Desktop / Tablet / Mobile with DPR
- Falls back to static fetch if Chrome is not installed or launch fails
- `RenderMethod` field on `CrawlResult` reports `Static` or `JavaScript`

### 4.3 Browser Fingerprint (`browser_fingerprint.rs`) — v0.2.0

```rust
pub struct BrowserFingerprint {
    pub user_agent: String,
    pub platform: String,
    pub vendor: String,
    pub webgl_vendor: String,
    pub webgl_renderer: String,
    pub languages: Vec<String>,
    pub timezone: String,
    pub screen_width: u32,
    pub screen_height: u32,
    pub color_depth: u32,
    pub hardware_concurrency: u32,
    pub device_memory: u32,
    pub canvas_noise: f64,
}

impl BrowserFingerprint {
    pub fn random_desktop(rng: &mut impl Rng) -> Self { ... }
    pub fn random_tablet(rng: &mut impl Rng) -> Self { ... }
    pub fn random_mobile(rng: &mut impl Rng) -> Self { ... }
    /// Generates the CDP addScriptToEvaluateOnNewDocument JS for injection
    pub fn to_injection_script(&self) -> String { ... }
}
```

### 4.4 Link Extractor (`extractor.rs`)

```rust
pub struct ExtractedLinks {
    pub internal: Vec<Url>,        // Same domain
    pub external: Vec<Url>,        // Different domain
    pub subdomains: Vec<Url>,      // Same root, different subdomain
    pub resources: Vec<Url>,       // CSS, JS, images
    pub feeds: Vec<Url>,           // RSS, Atom feeds
    pub sitemaps: Vec<Url>,        // sitemap.xml references
}
```

**Must extract links from:**
- `<a href>`, `<link>`, `<script src>`, `<iframe src>`, `<form action>`, `<area href>` tags
- `<meta http-equiv="refresh">` redirects
- Inline JavaScript (`window.location`, `document.location`)
- CSS `url()` references
- XML/RSS/Atom `<link>` elements
- JSON-LD `@id` and `url` fields
- `srcset` attributes, `data-*` attributes containing URLs
- Sitemap XML `<loc>` elements
- Open Graph / Twitter Card URLs

### 4.5 Content Parser & Converter (`parser.rs` + `converter.rs`)

**Supported input formats:**
| Format | Parser | Output |
|--------|--------|--------|
| HTML/XHTML | scraper + custom | Markdown |
| XML (generic) | quick-xml | Structured Markdown |
| RSS 2.0 | quick-xml | Markdown feed |
| Atom | quick-xml | Markdown feed |
| JSON-LD | serde_json | Markdown metadata |
| SVG | quick-xml | Description + metadata |
| Sitemap XML | quick-xml | URL list in Markdown |
| Plain Text | passthrough | Markdown (wrapped) |
| JSON API | serde_json | Markdown tables/structure |

**HTML → Markdown conversion rules:**

```
<h1>-<h6>         →  # to ######
<p>                →  paragraph with blank lines
<a href>           →  [text](url)
<img>              →  ![alt](src)
<strong>/<b>       →  **text**
<em>/<i>           →  *text*
<code>             →  `code`
<pre><code>        →  ```lang\ncode\n```
<ul>/<ol>          →  - item / 1. item
<blockquote>       →  > text
<table>            →  | col | col | (GFM tables)
<hr>               →  ---
<del>/<s>          →  ~~text~~
<sup>              →  ^text
<sub>              →  ~text
<details>          →  <details> (passthrough)
<br>               →  \n
<dl>/<dt>/<dd>     →  **term:** definition
<figure>           →  ![caption](src)
<video>/<audio>    →  [Media: title](src)
<iframe>           →  [Embedded: title](src)
```

### 4.6 Scheduler (`scheduler.rs`)

```rust
pub struct CrawlTask {
    pub url: Url,
    pub depth: u32,
    pub priority: Priority,
    pub source: TaskSource,              // How this URL was discovered
    pub requires_js: bool,               // Whether JS rendering is needed
}

pub enum Priority {
    Critical,   // Sitemaps, index pages
    High,       // Internal links from seed
    Normal,     // Discovered internal links
    Low,        // External links
}
```

**Requirements:**
- Lock-free concurrent priority queue
- URL normalization before dedup (strip fragments, normalize slashes, sort params)
- Domain-based rate limiting (configurable per-domain delay)
- Breadth-first by default, configurable to depth-first
- Progress tracking and statistics

### 4.7 Anti-Detection (`anti_detect.rs`)

```rust
pub enum DelayStrategy {
    Fixed(Duration),
    Random { min: Duration, max: Duration },
    Adaptive,                           // Slow down on 429/503
    None,                               // No delay (use with proxies)
}
```

**Capabilities:**
- 500+ rotating user agents (Chrome, Firefox, Safari, Edge — all platforms)
- Realistic header ordering
- TLS fingerprint variation (JA3 hash diversity)
- Configurable request delays per domain
- Adaptive throttling on rate limit detection (429, 503, CAPTCHA pages)
- Proxy rotation support (HTTP, SOCKS5, rotating residential)
- Automatic retry with different identity on block
- Referrer chain simulation

### 4.8 Proxy Support (`proxy.rs`)

```rust
pub struct ProxyConfig {
    pub url: String,
    pub proxy_type: ProxyType,         // Http, Https, Socks5
    pub auth: Option<ProxyAuth>,       // ProxyAuth { username, password }
    pub region: Option<String>,
}

pub enum ProxyStrategy {
    RoundRobin,
    Random,
    LeastUsed,
    FailoverOnly,                       // Use proxy only after direct fails
}
```

### 4.9 Output Handler (`output.rs`)

```rust
pub enum OutputMode {
    /// Return all results as a Vec<CrawlResult>
    Memory,
    /// Write each page as individual .md file
    Files {
        output_dir: PathBuf,
        structure: FileStructure,
    },
    /// Write everything into a single .md file
    SingleFile {
        output_path: PathBuf,
    },
}

pub enum FileStructure {
    Flat,    // All files in one directory
    Mirror,  // Replicate URL path structure
}

pub struct CrawlResult {
    pub url: String,
    pub title: String,
    pub markdown: String,
    pub links: Vec<String>,
    pub crawled_at: chrono::DateTime<chrono::Utc>,
    pub status_code: u16,
    pub content_type: String,
    pub depth: u32,
    pub render_method: RenderMethod,    // Static or JavaScript
    pub screenshot_path: Option<PathBuf>, // Set when screenshot = true
}

pub enum RenderMethod {
    Static,
    JavaScript,
}
// Note: RenderMethod does NOT derive PartialEq; use matches!() in tests
```

---

## 5. Public API

### 5.1 Rust API — v0.2.0

```rust
use rehyke_core::{
    CrawlConfigBuilder, Rehyke, ScanMode, Viewport, ScreenshotFormat,
    WaitStrategy, RehykeError,
};
use std::time::Duration;

// One-shot shorthand
let results = Rehyke::crawl("https://example.com", ScanMode::Full).await?;

// Full v0.2.0 config
let config = CrawlConfigBuilder::new()
    .mode(ScanMode::Full)
    .max_depth(10)
    .max_pages(5_000)
    .concurrency(20)
    // JS Rendering (requires --features js and Chrome)
    .enable_js(true)
    .viewport(Viewport::Mobile)
    .js_wait_strategy(WaitStrategy::NetworkIdle)
    .js_wait_timeout(Duration::from_secs(10))
    .js_scroll_count(3)
    .dismiss_popups(true)
    .detect_spa(true)
    .randomize_fingerprint(true)
    // Screenshots
    .screenshot(true)
    .screenshot_format(ScreenshotFormat::Jpeg)
    .screenshot_output_dir("/tmp/screenshots".into())
    // Filtering
    .exclude_patterns(vec![r"\.pdf$".to_string(), r"/login".to_string()])
    .include_patterns(vec![r"/blog/".to_string()])
    .respect_robots_txt(true)
    // Proxy
    .proxies(vec!["socks5://proxy:1080".to_string()])
    .build();

let results = Rehyke::new(config).run("https://example.com").await?;

for page in &results {
    println!("{} — {} words", page.url, page.markdown.split_whitespace().count());
    if let Some(path) = &page.screenshot_path {
        println!("  Screenshot: {}", path.display());
    }
}
```

**CrawlConfigBuilder v0.2.0 methods:**

| Method | Type | Default | Description |
|--------|------|---------|-------------|
| `.mode(ScanMode)` | — | `Full` | Lite / Full / Deep |
| `.max_depth(u32)` | — | mode-dependent | Max link depth |
| `.max_pages(usize)` | — | mode-dependent | Hard page cap |
| `.concurrency(usize)` | — | mode-dependent | Parallel fetchers |
| `.enable_js(bool)` | bool | `false` | Enable headless Chrome |
| `.viewport(Viewport)` | — | `Desktop` | Desktop / Tablet / Mobile |
| `.js_wait_strategy(WaitStrategy)` | — | `Auto` | How to wait after JS load |
| `.js_wait_timeout(Duration)` | — | 15s | Max JS wait time |
| `.js_scroll_count(u32)` | — | `0` | Infinite scroll iterations |
| `.dismiss_popups(bool)` | — | `false` | Auto-click cookie banners |
| `.detect_spa(bool)` | — | `false` | SPA framework detection |
| `.randomize_fingerprint(bool)` | — | `false` | Random browser fingerprint |
| `.screenshot(bool)` | — | `false` | Capture page screenshots |
| `.screenshot_format(ScreenshotFormat)` | — | `Png` | `Png` or `Jpeg` |
| `.screenshot_output_dir(PathBuf)` | — | `None` | Where to save screenshots |
| `.exclude_patterns(Vec<String>)` | — | `[]` | Regex URL blocklist |
| `.include_patterns(Vec<String>)` | — | `[]` | Regex URL allowlist |
| `.respect_robots_txt(bool)` | — | `true` | Honor robots.txt |
| `.proxies(Vec<String>)` | — | `[]` | Proxy URLs |
| `.user_agent(String)` | — | rotated | Custom UA string |
| `.timeout(Duration)` | — | 30s | Per-request timeout |
| `.max_retries(u32)` | — | `3` | Retry count on failure |

### 5.2 Python API

```python
import rehyke
from rehyke import Rehyke, ScanMode, CrawlConfig

# Simple one-liner
results = rehyke.crawl("https://example.com")

# With JS rendering
results = rehyke.crawl(
    "https://example.com",
    mode="full",
    enable_js=True,
    viewport="mobile",
    js_wait_strategy="network_idle",
    js_wait_seconds=10,
    dismiss_popups=True,
    detect_spa=True,
    randomize_fingerprint=True,
)

# Full configuration
config = CrawlConfig(
    mode=ScanMode.FULL,
    max_depth=10,
    max_pages=5_000,
    concurrency=20,
    enable_js=True,
    viewport="mobile",
    js_wait_strategy="network_idle",
    js_wait_seconds=10,
    js_scroll_count=3,
    dismiss_popups=True,
    detect_spa=True,
    randomize_fingerprint=True,
    screenshot=True,
    screenshot_format="jpeg",
    screenshot_output_dir="./screenshots",
    exclude_patterns=[r"\.pdf$", r"/login"],
    include_patterns=[r"/blog/"],
    respect_robots_txt=True,
    proxies=["socks5://proxy:1080"],
    timeout_seconds=30,
    max_retries=3,
)

crawler = Rehyke(config)
results = crawler.crawl("https://example.com")

for page in results:
    print(f"URL:    {page.url}")
    print(f"Status: {page.status_code}")
    print(f"Render: {page.render_method}")   # "static" or "js"
    print(f"Depth:  {page.depth}")
    print(f"Words:  {len(page.markdown.split())}")
    if page.screenshot_path:
        print(f"Screenshot: {page.screenshot_path}")
    print("---")
```

### 5.3 CLI

```bash
# Build with JS support (requires Chrome)
cargo install rehyke-cli --features js

# Basic usage
rehyke https://example.com

# JS rendering
rehyke https://example.com --js --viewport mobile --wait-strategy network-idle

# Full options
rehyke https://example.com \
    --mode full \
    --max-depth 10 \
    --max-pages 5000 \
    --concurrency 20 \
    --js \
    --viewport mobile \
    --wait-strategy network-idle \
    --wait-timeout 10 \
    --scroll-count 3 \
    --dismiss-popups \
    --detect-spa \
    --randomize-fingerprint \
    --screenshot \
    --screenshot-format jpeg \
    --screenshot-dir ./screenshots \
    --output-dir ./output \
    --structure mirror \
    --exclude '\.pdf$' \
    --include '/blog/' \
    --no-robots \
    --proxy socks5://proxy:1080 \
    --timeout 30 \
    --retries 3 \
    --verbose
```

---

## 6. Rust Dependencies (Cargo.toml)

### rehyke-core

```toml
[features]
default = []
js = ["chromiumoxide"]       # Enable headless Chrome (opt-in)

[dependencies]
tokio = { version = "1", features = ["full"] }
futures = "0.3"
reqwest = { version = "0.12", features = ["gzip", "brotli", "deflate", "zstd", "cookies", "socks", "rustls-tls"] }
scraper = "0.20"
html5ever = "0.29"
quick-xml = { version = "0.36", features = ["serialize"] }
url = "2"
percent-encoding = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
dashmap = "6"
crossbeam = "0.8"
regex = "1"
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
tracing-subscriber = "0.3"
thiserror = "2"
anyhow = "1"
governor = "0.7"
mime = "0.3"
mime_guess = "2"
encoding_rs = "0.8"
rand = "0.8"

# Optional — only compiled when features = ["js"]
chromiumoxide = { version = "0.7", default-features = false, features = ["tokio-runtime"], optional = true }
```

---

## 7. Markdown Output Format

Each crawled page produces Markdown in this format:

```markdown
---
url: https://example.com/page
title: Page Title
description: Meta description
author: Author Name
published: 2024-01-15
language: en
canonical: https://example.com/page
crawled_at: 2025-02-24T10:30:00Z
status_code: 200
content_type: text/html
depth: 2
render_method: js
---

# Page Title

Main content converted to clean Markdown...
```

---

## 8. Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum RehykeError {
    #[error("HTTP error for {url}: {status}")]
    HttpError { url: String, status: u16 },

    #[error("Connection timeout for {url}")]
    Timeout { url: String },

    #[error("DNS resolution failed for {domain}")]
    DnsError { domain: String },

    #[error("TLS/SSL error for {url}: {message}")]
    TlsError { url: String, message: String },

    #[error("JavaScript rendering failed for {url}: {message}")]
    RenderError { url: String, message: String },

    #[error("Browser launch failed: {message}")]
    BrowserError { message: String },

    #[error("Parse error for {url}: {message}")]
    ParseError { url: String, message: String },

    #[error("Proxy error: {message}")]
    ProxyError { message: String },

    #[error("Rate limited by {domain}")]
    RateLimited { domain: String },

    #[error("Max pages limit reached: {limit}")]
    MaxPagesReached { limit: usize },

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Configuration error: {message}")]
    ConfigError { message: String },
}
```

**Error Recovery Strategy:**
- Network errors → retry with exponential backoff (up to max_retries)
- 429 Too Many Requests → wait `Retry-After` header duration, then retry
- 403 Forbidden → rotate UA/proxy, retry once
- 503 Service Unavailable → wait 5s, retry
- JS render timeout → fall back to static fetch
- DNS failure → skip URL, log warning
- All unrecoverable errors → log, skip URL, continue crawling

---

## 9. Performance Targets

| Metric | Target | v0.2.0 Status |
|--------|--------|---------------|
| Single page (lite, no JS) | < 200ms | ✅ |
| Single page (lite, with JS) | < 3s | ✅ |
| 100 pages (full, parallel) | < 30s | ✅ |
| 1,000 pages (full, parallel) | < 5 min | ✅ |
| 10,000 pages (deep, parallel) | < 30 min | ✅ |
| Memory usage per page | < 5MB | ✅ |
| Peak memory (10K pages) | < 2GB | ✅ |
| Concurrent connections | Up to 200 | ✅ |

---

## 10. Testing

### Test Counts (v0.2.0)
- **Unit tests:** 444 (across config.rs, renderer.rs, lib.rs, fetcher.rs, proxy.rs, etc.)
- **Integration tests:** 16 (in `crates/rehyke-core/tests/integration.rs`)
- **Doctests:** 13 (in `///` doc comments on public API types)
- **Total:** 473

### Running Tests

```bash
# All offline tests (default, no network required)
cargo test --workspace

# With JS feature enabled
cargo test --workspace --features js

# Include live network tests (requires internet access)
REHYKE_LIVE=1 cargo test --package rehyke-core --test integration

# Integration tests only (offline)
cargo test --package rehyke-core --test integration

# Specific test module
cargo test --package rehyke-core config::tests

# Run doctests
cargo test --doc --package rehyke-core
```

### Integration Test Gating

Live-network integration tests are gated behind `REHYKE_LIVE=1`:

```rust
fn live() -> bool {
    std::env::var("REHYKE_LIVE").is_ok()
}

#[tokio::test]
async fn live_static_crawl_returns_results() {
    if !live() { return; }  // Skip in CI / offline environments
    // ... actual network test
}
```

### Unit Test Coverage
- `config.rs`: 19 tests — Viewport, ScreenshotFormat, WaitStrategy, builder methods, scan mode defaults
- `renderer.rs`: 13 tests — RenderResult, SpaFramework, popup selectors, SPA detectors, viewport dimensions
- `lib.rs`: 9 tests — compile_patterns, URL validation, config propagation
- `fetcher.rs`: retry logic, timeout, proxy configuration
- `proxy.rs`: ProxyConfig, ProxyPool, ProxyStrategy, ProxyAuth

### Key Test Patterns

```rust
// Use matches!() for enums without PartialEq (e.g., RenderMethod)
assert!(matches!(page.render_method, rehyke_core::output::RenderMethod::Static));

// WaitStrategy variants require named fields:
WaitStrategy::Selector { selector: "#app".to_string() }
WaitStrategy::Duration { duration: Duration::from_millis(800) }
// (NetworkIdle and Auto have no fields)
```

---

## 11. Publishing

### crates.io (Rust)
```bash
# Publish core crate first
cd crates/rehyke-core && cargo publish
# Then CLI
cd crates/rehyke-cli && cargo publish
```

### PyPI (Python)
```bash
# Build with maturin
maturin build --release
# Publish
maturin publish
```

### Package names
- **crates.io:** `rehyke` (core), `rehyke-cli` (CLI)
- **PyPI:** `rehyke`

---

## 12. Implementation Status

### ✅ v0.1.0 — Core Engine (shipped)
- `config.rs` — Configuration structs and ScanMode enum
- `error.rs` — Error types
- `utils.rs` — URL normalization
- `fetcher.rs` — HTTP client with retry, proxy, UA rotation
- `parser.rs` — HTML/XML/RSS/Atom parsing
- `converter.rs` — HTML → Markdown
- `extractor.rs` — Link extraction
- `scheduler.rs` — URL frontier with dedup and rate limiting
- `output.rs` — Memory and file output
- `lib.rs` — Public API
- `anti_detect.rs` — UA rotation, header profiles
- `robots.rs` — robots.txt parser
- `sitemap.rs` — sitemap.xml discovery
- `proxy.rs` — Proxy pool management
- `rehyke-cli` — CLI binary
- `rehyke-python` — PyO3 bindings

### ✅ v0.2.0 — Chrome Eyes (shipped)
- `renderer.rs` — Full headless Chrome JS rendering via `chromiumoxide`
- `browser_fingerprint.rs` — Randomised browser fingerprint injection
- SPA detection (React, Vue, Angular, Svelte, Next.js, Nuxt, SvelteKit)
- Infinite scroll simulation
- Popup/cookie banner auto-dismissal
- Viewport emulation (Desktop / Tablet / Mobile)
- Screenshot capture (PNG / JPEG)
- New config fields: `viewport`, `screenshot`, `screenshot_format`, `screenshot_output_dir`, `js_scroll_count`, `dismiss_popups`, `detect_spa`, `randomize_fingerprint`, `js_wait_timeout`, `js_wait_strategy`
- 473 tests (444 unit + 16 integration + 13 doctests)

### 🔲 v0.3.0 — Planned
- Streaming output via callback
- Authenticated crawling (OAuth, form-based login)
- Sitemap-guided crawling prioritization
- Improved SPA hydration detection
- Memory-mapped output for very large crawls

---

## 13. Code Style & Conventions

- **Rust edition:** 2021
- **MSRV:** 1.75.0
- **Formatting:** `rustfmt` with default config
- **Linting:** `clippy` with `-D warnings`
- **Documentation:** All public items documented with `///` doc comments including `# Examples` blocks
- **Error handling:** Use `thiserror` for library errors, `anyhow` in CLI/tests only
- **Async:** All I/O operations are async (tokio)
- **Safety:** No `unsafe` code except in PyO3 bindings (required by PyO3)
- **Naming:** snake_case for functions/variables, PascalCase for types, SCREAMING_SNAKE for constants
- **Testing:** Every module has a `#[cfg(test)] mod tests` section
- **Logging:** Use `tracing` macros (`info!`, `debug!`, `warn!`, `error!`)
- **Feature flags:** Use `#[cfg(feature = "js")]` for all Chrome-dependent code — crate must compile without Chrome
- **Reserved words:** Avoid Rust reserved keywords (`final`, `abstract`, `become`, `box`, `do`, `override`, `priv`, `typeof`, `unsized`, `virtual`, `yield`) as variable names

---

## 14. Important Implementation Notes

### Feature Flag: `js`
All headless Chrome code is gated behind `#[cfg(feature = "js")]`. The crate compiles and runs fully without Chrome installed. When `enable_js = true` but the `js` feature is not compiled, the renderer silently falls back to static. When the `js` feature is compiled but Chrome is not installed, `Rehyke::run()` logs a warning and falls back to static fetch.

### URL Normalization
Always normalize before dedup:
1. Lowercase scheme and host
2. Remove default ports (80, 443)
3. Remove fragment (#)
4. Remove trailing slash (except root)
5. Sort query parameters
6. Decode unnecessary percent-encoding
7. Remove `www.` prefix (configurable)

### Markdown Conversion Quality
The converter MUST produce clean, readable Markdown:
- No excessive blank lines (max 2 consecutive)
- Proper nesting of lists
- Code blocks with language detection (from `class` attribute)
- Tables must be properly aligned
- No HTML remnants in output
- UTF-8 throughout

### WaitStrategy Usage
```rust
// Correct — NetworkIdle and Auto have no fields:
.js_wait_strategy(WaitStrategy::NetworkIdle)
.js_wait_strategy(WaitStrategy::Auto)

// Correct — Selector and Duration use named fields:
.js_wait_strategy(WaitStrategy::Selector { selector: "#app".to_string() })
.js_wait_strategy(WaitStrategy::Duration { duration: Duration::from_secs(2) })
```

### Concurrency Safety
- All shared state via `DashMap`/`DashSet` (lock-free)
- URL frontier is a concurrent priority queue
- Chrome browser shared via `Arc<Mutex<Browser>>`
- Per-domain rate limiters are independent
- No global locks on the hot path

---

## 15. Environment Setup

### Prerequisites
- Rust 1.75+ (install via rustup)
- Python 3.8+ (for Python bindings)
- Chromium/Chrome browser (optional, for JS rendering feature)
- maturin (`pip install maturin`)

### Development

```bash
# Clone and build (no Chrome required)
git clone <repo>
cd rehyke
cargo build

# Build with JS/Chrome support
cargo build --features js

# Run all tests (offline, no network)
cargo test --workspace

# Run tests with live network (needs internet)
REHYKE_LIVE=1 cargo test --package rehyke-core --test integration

# Build Python wheel
cd crates/rehyke-python
maturin develop

# Run benchmarks
cargo bench

# Check formatting and lints
cargo fmt --check
cargo clippy -- -D warnings

# Run JS rendering examples (requires --features js + Chrome)
cargo run --example js_render --features js
cargo run --example spa_crawl --features js
cargo run --example screenshot_capture --features js
```
