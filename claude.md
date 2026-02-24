# CLAUDE.md — Rehyke: Advanced Web Crawler

## Project Identity

**Name:** Rehyke
**Tagline:** "Crawl Everything. Miss Nothing."
**Version:** 0.1.0
**Language:** Rust core + Python bindings (PyO3/maturin)
**Distribution:** crates.io (Rust) + PyPI (Python)
**License:** MIT OR Apache-2.0

---

## 1. Project Overview

Rehyke is an ultra-high-performance, JavaScript-rendering web crawler built in Rust with first-class Python bindings. It crawls any website — including JS-heavy SPAs (React, Vue, Angular, Svelte), XML sitemaps, RSS feeds, and arbitrary markup — and outputs clean, structured Markdown.

It is designed to be **faster, more resilient, and more capable** than crawl4ai, Scrapy, or any existing crawl tool.

### Core Differentiators
- **Rust-native performance** with zero-cost Python interop via PyO3
- **Headless Chromium** integration for full JavaScript rendering
- **Three scan modes**: Lite, Full, Deep — user controls depth/breadth
- **Concurrent architecture**: tokio async runtime + work-stealing task pool
- **Anti-detection**: rotating user agents, request throttling, proxy support, TLS fingerprint randomization
- **Universal parser**: HTML, XHTML, XML, RSS, Atom, JSON-LD, SVG, sitemap.xml
- **Output flexibility**: return Markdown strings or save to `.md` files (user's choice)

---

## 2. Architecture

### 2.1 High-Level Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Python API (PyO3)                     │
│  rehyke.crawl(url, mode="full", output="markdown")      │
├─────────────────────────────────────────────────────────┤
│                   Rust Core Engine                       │
│                                                         │
│  ┌──────────┐  ┌──────────┐  ┌───────────┐             │
│  │ Scheduler│──│  Fetcher  │──│  Renderer │             │
│  │ (tokio)  │  │(reqwest + │  │(headless  │             │
│  │          │  │ hyper)    │  │ chromium) │             │
│  └────┬─────┘  └────┬─────┘  └─────┬─────┘             │
│       │              │              │                    │
│  ┌────▼──────────────▼──────────────▼─────┐             │
│  │           Link Extractor               │             │
│  │   (scraper + select.rs + custom XML)   │             │
│  └────────────────┬───────────────────────┘             │
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
├── Cargo.toml                    # Workspace root
├── pyproject.toml                # Python package config (maturin)
├── README.md
├── LICENSE-MIT
├── LICENSE-APACHE
├── CLAUDE.md                     # This file
│
├── crates/
│   ├── rehyke-core/              # Core crawler engine
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs            # Public API
│   │       ├── config.rs         # CrawlConfig, ScanMode enum
│   │       ├── scheduler.rs      # URL frontier, priority queue, dedup
│   │       ├── fetcher.rs        # HTTP client (reqwest), retry, proxy
│   │       ├── renderer.rs       # Headless Chromium (chromiumoxide)
│   │       ├── extractor.rs      # Link extraction (all formats)
│   │       ├── parser.rs         # HTML/XML/RSS/Atom/JSON-LD parsing
│   │       ├── converter.rs      # Content → Markdown conversion
│   │       ├── robots.rs         # robots.txt parser (optional respect)
│   │       ├── sitemap.rs        # sitemap.xml parser and crawler
│   │       ├── anti_detect.rs    # UA rotation, fingerprinting, delays
│   │       ├── proxy.rs          # Proxy pool management
│   │       ├── output.rs         # File writer / string collector
│   │       ├── error.rs          # Error types
│   │       └── utils.rs          # URL normalization, helpers
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
├── tests/
│   ├── integration/
│   │   ├── test_lite_scan.rs
│   │   ├── test_full_scan.rs
│   │   ├── test_deep_scan.rs
│   │   └── test_js_rendering.rs
│   └── python/
│       ├── test_basic.py
│       ├── test_modes.py
│       └── test_output.py
│
├── benches/
│   └── crawl_benchmark.rs
│
└── examples/
    ├── basic_crawl.rs
    ├── deep_crawl.rs
    ├── python_basic.py
    └── python_advanced.py
```

---

## 3. Scan Modes

### 3.1 Lite Scan
- **Scope:** Single page only (the given URL)
- **JS Rendering:** Disabled by default (optional enable)
- **Links:** Extracts but does NOT follow
- **Speed:** Fastest — single request
- **Use case:** Quick content extraction from a known page

### 3.2 Full Scan
- **Scope:** Given domain — all internal links (same domain/subdomain)
- **JS Rendering:** Enabled
- **Links:** Follows all internal links recursively (respects max_depth)
- **Concurrency:** Up to 50 concurrent requests (configurable)
- **Speed:** Fast — parallel crawling with dedup
- **Max Depth:** 10 levels (configurable)
- **Use case:** Complete site extraction

### 3.3 Deep Scan
- **Scope:** Given domain + ALL discovered external links
- **JS Rendering:** Enabled for all pages
- **Links:** Follows internal AND external links recursively
- **Concurrency:** Up to 100 concurrent requests (configurable)
- **Speed:** Thorough — maximum coverage
- **Max Depth:** 20 levels (configurable)
- **External Depth:** 2 levels deep into external sites (configurable)
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

### 4.2 Renderer (`renderer.rs`)

```rust
pub struct Renderer {
    browser: Browser,              // chromiumoxide
    tab_pool: TabPool,             // Reusable browser tabs
    render_timeout: Duration,
    wait_strategy: WaitStrategy,
}

pub enum WaitStrategy {
    NetworkIdle(Duration),         // Wait for network to be idle
    Selector(String),              // Wait for CSS selector to appear
    Duration(Duration),            // Fixed wait time
    Auto,                          // Smart detection
}
```

**Requirements:**
- Headless Chromium via `chromiumoxide` crate
- Tab pooling (reuse tabs, don't spawn new for each page)
- Configurable wait strategies per page
- JavaScript execution support
- Screenshot capability (optional, for debugging)
- Intercept and block unnecessary resources (images, fonts, media) for speed
- Extract final rendered DOM as HTML string
- Handle SPAs: React, Vue, Angular, Svelte, Next.js, Nuxt, SvelteKit
- Handle infinite scroll detection and execution (configurable scroll count)
- Handle popups/modals/cookie banners (auto-dismiss)

### 4.3 Link Extractor (`extractor.rs`)

```rust
pub struct LinkExtractor;

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
- `<a href>` tags
- `<link>` tags (stylesheets, canonical, alternate)
- `<script src>` tags
- `<iframe src>` tags
- `<form action>` tags
- `<area href>` tags (image maps)
- `<meta http-equiv="refresh">` redirects
- Inline JavaScript (`window.location`, `document.location`)
- CSS `url()` references
- XML/RSS/Atom `<link>` elements
- JSON-LD `@id` and `url` fields
- `srcset` attributes
- `data-*` attributes containing URLs
- Sitemap XML `<loc>` elements
- Open Graph / Twitter Card URLs

### 4.4 Content Parser & Converter (`parser.rs` + `converter.rs`)

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

**Content cleaning (remove before conversion):**
- `<script>` tags and content
- `<style>` tags and content
- `<noscript>` tags
- Navigation elements (`<nav>`)
- Header/footer boilerplate (`<header>`, `<footer>`) — configurable
- Cookie consent banners
- Ad containers (common ad class/id patterns)
- Hidden elements (`display: none`, `visibility: hidden`)
- Comment sections — configurable
- Social media share buttons
- Popups and overlays

### 4.5 Scheduler (`scheduler.rs`)

```rust
pub struct Scheduler {
    frontier: PriorityQueue<CrawlTask>,  // URL priority queue
    visited: DashSet<String>,            // Thread-safe dedup (normalized URLs)
    in_progress: DashSet<String>,        // Currently being fetched
    domain_delays: DashMap<String, Instant>, // Per-domain rate limiting
    stats: CrawlStats,
}

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
- Max URLs limit (configurable, default: 10,000 for Full, 100,000 for Deep)
- Progress tracking and statistics

### 4.6 Anti-Detection (`anti_detect.rs`)

```rust
pub struct AntiDetect {
    ua_pool: Vec<String>,              // 500+ real user agents
    header_profiles: Vec<HeaderProfile>, // Realistic browser header sets
    tls_configs: Vec<TlsProfile>,      // Different TLS fingerprints
    delay_strategy: DelayStrategy,
}

pub enum DelayStrategy {
    Fixed(Duration),
    Random { min: Duration, max: Duration },
    Adaptive,                           // Slow down on 429/503
    None,                               // No delay (use with proxies)
}
```

**Capabilities:**
- 500+ rotating user agents (Chrome, Firefox, Safari, Edge — all platforms)
- Realistic header ordering (Accept, Accept-Language, Accept-Encoding, etc.)
- TLS fingerprint variation (JA3 hash diversity)
- Configurable request delays per domain
- Adaptive throttling on rate limit detection (429, 503, CAPTCHA pages)
- Proxy rotation support (HTTP, SOCKS5, rotating residential)
- Automatic retry with different identity on block
- Referrer chain simulation
- Cookie acceptance automation (via renderer)
- Respect `Retry-After` headers

### 4.7 Proxy Support (`proxy.rs`)

```rust
pub struct ProxyPool {
    proxies: Vec<ProxyConfig>,
    strategy: ProxyStrategy,
    health_checker: HealthChecker,
}

pub struct ProxyConfig {
    pub url: String,
    pub proxy_type: ProxyType,         // HTTP, HTTPS, SOCKS5
    pub auth: Option<(String, String)>, // username, password
    pub region: Option<String>,
}

pub enum ProxyStrategy {
    RoundRobin,
    Random,
    LeastUsed,
    FailoverOnly,                       // Use proxy only after direct fails
}
```

### 4.8 Output Handler (`output.rs`)

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
    /// Stream results via callback
    Streaming(Box<dyn Fn(CrawlResult) + Send + Sync>),
}

pub enum FileStructure {
    /// Flat: all files in one directory
    Flat,
    /// Mirror: replicate URL path structure
    Mirror,
}

pub struct CrawlResult {
    pub url: String,
    pub title: String,
    pub markdown: String,
    pub metadata: PageMetadata,
    pub links: ExtractedLinks,
    pub crawled_at: DateTime<Utc>,
    pub status_code: u16,
    pub content_type: String,
    pub depth: u32,
    pub render_method: RenderMethod,    // Static or JS-rendered
}

pub struct PageMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub published_date: Option<String>,
    pub language: Option<String>,
    pub canonical_url: Option<String>,
    pub og_image: Option<String>,
    pub keywords: Vec<String>,
    pub word_count: usize,
    pub reading_time_minutes: f32,
}
```

---

## 5. Public API

### 5.1 Rust API

```rust
use rehyke::{Rehyke, CrawlConfig, ScanMode, OutputMode};

// Simple usage
let results = Rehyke::crawl("https://example.com", ScanMode::Full).await?;

// Advanced usage
let config = CrawlConfig::builder()
    .mode(ScanMode::Deep)
    .max_depth(15)
    .max_pages(50_000)
    .concurrency(80)
    .enable_js(true)
    .js_wait_strategy(WaitStrategy::NetworkIdle(Duration::from_secs(3)))
    .output(OutputMode::Files {
        output_dir: "output/".into(),
        structure: FileStructure::Mirror,
    })
    .user_agent("custom-agent/1.0")
    .proxies(vec!["socks5://proxy1:1080", "http://proxy2:8080"])
    .proxy_strategy(ProxyStrategy::RoundRobin)
    .delay(DelayStrategy::Random {
        min: Duration::from_millis(500),
        max: Duration::from_secs(2),
    })
    .exclude_patterns(vec![r"\.pdf$", r"/login", r"/admin"])
    .include_patterns(vec![r"/blog/", r"/docs/"])
    .respect_robots_txt(false)           // User's choice
    .extract_metadata(true)
    .clean_navigation(true)
    .clean_footers(true)
    .clean_ads(true)
    .timeout(Duration::from_secs(30))
    .max_retries(3)
    .headers(vec![("Authorization", "Bearer token123")])
    .cookies(vec![("session", "abc123", ".example.com")])
    .build()?;

let crawler = Rehyke::new(config);

// With progress callback
crawler.on_progress(|stats| {
    println!("Crawled: {}/{} pages", stats.completed, stats.total_discovered);
});

// With per-page callback
crawler.on_page(|result| {
    println!("Got: {} ({} words)", result.url, result.metadata.word_count);
});

let results = crawler.run().await?;

println!("Total pages: {}", results.len());
println!("Total time: {:?}", results.elapsed());
```

### 5.2 Python API

```python
import rehyke
from rehyke import Rehyke, ScanMode, OutputMode, CrawlConfig

# Simple one-liner
results = rehyke.crawl("https://example.com")

# With mode
results = rehyke.crawl("https://example.com", mode="full")

# Full configuration
config = CrawlConfig(
    mode=ScanMode.DEEP,
    max_depth=15,
    max_pages=50_000,
    concurrency=80,
    enable_js=True,
    js_wait_seconds=3,
    output=OutputMode.FILES,
    output_dir="./output",
    file_structure="mirror",          # "flat" or "mirror"
    proxies=["socks5://proxy1:1080"],
    proxy_strategy="round_robin",
    delay_min_ms=500,
    delay_max_ms=2000,
    exclude_patterns=[r"\.pdf$", r"/login"],
    include_patterns=[r"/blog/"],
    respect_robots_txt=False,
    extract_metadata=True,
    clean_navigation=True,
    clean_footers=True,
    clean_ads=True,
    timeout_seconds=30,
    max_retries=3,
    custom_headers={"Authorization": "Bearer token123"},
    cookies={"session": "abc123"},
)

crawler = Rehyke(config)

# Sync usage
results = crawler.crawl("https://example.com")

# Access results
for page in results:
    print(f"URL: {page.url}")
    print(f"Title: {page.title}")
    print(f"Words: {page.metadata.word_count}")
    print(f"Markdown length: {len(page.markdown)}")
    print(f"Links found: {len(page.links.internal) + len(page.links.external)}")
    print("---")

# Async usage
import asyncio

async def main():
    results = await crawler.crawl_async("https://example.com")
    print(f"Crawled {len(results)} pages")

asyncio.run(main())

# Save all results to single file
crawler.crawl_to_file("https://example.com", "output.md")

# Stream results
for page in crawler.crawl_stream("https://example.com"):
    print(f"Got: {page.url}")
    with open(f"pages/{page.slug}.md", "w") as f:
        f.write(page.markdown)
```

### 5.3 CLI

```bash
# Install
cargo install rehyke-cli

# Basic usage
rehyke https://example.com

# With mode
rehyke https://example.com --mode full

# Deep scan with all options
rehyke https://example.com \
    --mode deep \
    --max-depth 20 \
    --max-pages 100000 \
    --concurrency 100 \
    --js \
    --output-dir ./output \
    --structure mirror \
    --proxy socks5://proxy:1080 \
    --delay 500-2000 \
    --exclude '\.pdf$' \
    --include '/blog/' \
    --no-robots \
    --clean-nav \
    --clean-footer \
    --clean-ads \
    --timeout 30 \
    --retries 3 \
    --verbose

# Output to single file
rehyke https://example.com --mode full -o site.md

# Lite scan (single page, fast)
rehyke https://example.com --mode lite

# JSON output (metadata only)
rehyke https://example.com --mode full --format json
```

---

## 6. Rust Dependencies (Cargo.toml)

### rehyke-core

```toml
[dependencies]
# Async runtime
tokio = { version = "1", features = ["full"] }
futures = "0.3"

# HTTP client
reqwest = { version = "0.12", features = ["gzip", "brotli", "deflate", "zstd", "cookies", "socks", "rustls-tls"] }

# HTML parsing
scraper = "0.20"
ego-tree = "0.9"
html5ever = "0.29"
markup5ever = "0.14"

# XML parsing
quick-xml = { version = "0.36", features = ["serialize"] }

# URL handling
url = "2"

# JSON
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Headless browser (JS rendering)
chromiumoxide = { version = "0.7", features = ["tokio-runtime"] }

# Concurrent data structures
dashmap = "6"
crossbeam = "0.8"
crossbeam-queue = "0.3"

# Regex
regex = "1"

# Time
chrono = { version = "0.4", features = ["serde"] }

# Logging
tracing = "0.1"
tracing-subscriber = "0.3"

# Error handling
thiserror = "2"
anyhow = "1"

# Markdown generation
# (custom implementation — no external dep needed)

# Rate limiting
governor = "0.7"

# Robots.txt
robotstxt = "0.3"

# Content type detection
mime = "0.3"
mime_guess = "2"

# Encoding detection
encoding_rs = "0.8"

# Random
rand = "0.8"
```

### rehyke-python

```toml
[dependencies]
pyo3 = { version = "0.22", features = ["extension-module", "abi3-py38"] }
pyo3-asyncio-0-22 = { version = "0.22", features = ["tokio-runtime"] }
rehyke-core = { path = "../rehyke-core" }
tokio = { version = "1", features = ["full"] }
```

### rehyke-cli

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
rehyke-core = { path = "../rehyke-core" }
tokio = { version = "1", features = ["full"] }
indicatif = "0.17"          # Progress bars
console = "0.15"            # Terminal colors
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
word_count: 1523
reading_time: 6.5 min
depth: 2
render_method: js
---

# Page Title

Main content converted to clean Markdown...

## Section Heading

Paragraph text with [links](https://example.com) and **formatting** preserved.

| Table | Data |
|-------|------|
| Cell  | Cell |

> Blockquotes preserved

```python
# Code blocks with language detection
print("hello")
```

---

*Crawled by [Rehyke](https://github.com/user/rehyke) — [Source](https://example.com/page)*
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
- TLS errors → try different TLS config, then skip
- All unrecoverable errors → log, skip URL, continue crawling

---

## 9. Performance Targets

| Metric | Target |
|--------|--------|
| Single page (lite, no JS) | < 200ms |
| Single page (lite, with JS) | < 3s |
| 100 pages (full, parallel) | < 30s |
| 1,000 pages (full, parallel) | < 5 min |
| 10,000 pages (deep, parallel) | < 30 min |
| Memory usage per page | < 5MB |
| Peak memory (10K pages) | < 2GB |
| Concurrent connections | Up to 200 |

---

## 10. Testing Requirements

### Unit Tests
- URL normalization (20+ edge cases)
- HTML → Markdown conversion (all element types)
- XML/RSS/Atom parsing
- Link extraction from all source types
- Robot.txt parsing
- Sitemap parsing
- Anti-detection UA rotation
- Proxy pool management

### Integration Tests
- Lite scan: single page crawl and verify markdown output
- Full scan: multi-page same-domain crawl
- Deep scan: cross-domain crawl with depth limits
- JS rendering: React/Vue SPA page extraction
- File output: verify directory structure and file content
- Error recovery: simulate timeouts, 403s, 429s
- Concurrent crawl: verify no duplicate processing

### Benchmarks
- Compare against crawl4ai on same target set
- Measure pages/second at various concurrency levels
- Memory profiling under load
- JS rendering overhead measurement

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

## 12. Implementation Priority

### Phase 1: Core Engine (MVP)
1. `config.rs` — Configuration structs and ScanMode enum
2. `error.rs` — Error types
3. `utils.rs` — URL normalization
4. `fetcher.rs` — HTTP client with basic headers
5. `parser.rs` — HTML parsing
6. `converter.rs` — HTML → Markdown
7. `extractor.rs` — Link extraction from HTML
8. `scheduler.rs` — URL frontier with dedup
9. `output.rs` — Memory and file output
10. `lib.rs` — Public API (Rehyke::crawl)

### Phase 2: Advanced Features
11. `renderer.rs` — Headless Chromium JS rendering
12. `anti_detect.rs` — UA rotation, header profiles
13. `robots.rs` — robots.txt parser
14. `sitemap.rs` — sitemap.xml discovery and parsing
15. XML/RSS/Atom support in parser + converter

### Phase 3: Distribution
16. `rehyke-cli` — CLI binary with clap
17. `rehyke-python` — PyO3 bindings
18. Tests and benchmarks
19. Documentation
20. Publish to crates.io and PyPI

---

## 13. Code Style & Conventions

- **Rust edition:** 2021
- **MSRV:** 1.75.0
- **Formatting:** `rustfmt` with default config
- **Linting:** `clippy` with `-D warnings`
- **Documentation:** All public items documented with `///` doc comments
- **Error handling:** Use `thiserror` for library errors, `anyhow` in CLI/tests only
- **Async:** All I/O operations are async (tokio)
- **Safety:** No `unsafe` code except in PyO3 bindings (required by PyO3)
- **Naming:** snake_case for functions/variables, PascalCase for types, SCREAMING_SNAKE for constants
- **Testing:** Every module has a `#[cfg(test)] mod tests` section
- **Logging:** Use `tracing` macros (`info!`, `debug!`, `warn!`, `error!`)

---

## 14. Important Implementation Notes

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
- Proper escaping of Markdown special characters in text content

### Memory Management
- Stream pages to disk when `OutputMode::Files` to avoid memory buildup
- Use `String::with_capacity` for known-size allocations
- Drop DOM trees immediately after extraction
- Limit in-memory results buffer size

### Concurrency Safety
- All shared state via `DashMap`/`DashSet` (lock-free)
- URL frontier is a concurrent priority queue
- Per-domain rate limiters are independent
- No global locks on the hot path

---

## 15. Environment Setup

### Prerequisites
- Rust 1.75+ (install via rustup)
- Python 3.8+ (for Python bindings)
- Chromium/Chrome browser (for JS rendering)
- maturin (`pip install maturin`)

### Development
```bash
# Clone and build
git clone <repo>
cd rehyke
cargo build

# Run tests
cargo test --workspace

# Build Python wheel
cd crates/rehyke-python
maturin develop

# Run benchmarks
cargo bench

# Check formatting and lints
cargo fmt --check
cargo clippy -- -D warnings
```
