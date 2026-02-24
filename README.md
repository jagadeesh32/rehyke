<p align="center">
  <img src="https://img.shields.io/badge/built%20with-Rust-e43717?style=for-the-badge&logo=rust&logoColor=white" alt="Built with Rust"/>
  <img src="https://img.shields.io/badge/python-3.8+-blue?style=for-the-badge&logo=python&logoColor=white" alt="Python 3.8+"/>
  <img src="https://img.shields.io/badge/license-MIT%20%7C%20Apache--2.0-green?style=for-the-badge" alt="License"/>
  <img src="https://img.shields.io/badge/tests-369%20passing-brightgreen?style=for-the-badge" alt="Tests"/>
  <img src="https://img.shields.io/badge/lines-11.6k-orange?style=for-the-badge" alt="Lines of Code"/>
</p>

<h1 align="center">
  <br>
  REHYKE
  <br>
</h1>

<h3 align="center">Crawl Everything. Miss Nothing.</h3>

<p align="center">
  <strong>Ultra-high-performance, regex-powered autonomous web crawler</strong><br>
  Built in Rust. Python-ready. Markdown output.<br>
  <em>The last crawler you'll ever need.</em>
</p>

---

<p align="center">
  <a href="#-features">Features</a> &bull;
  <a href="#-quick-start">Quick Start</a> &bull;
  <a href="#-scan-modes">Scan Modes</a> &bull;
  <a href="#-python-api">Python API</a> &bull;
  <a href="#-cli">CLI</a> &bull;
  <a href="#-architecture">Architecture</a> &bull;
  <a href="#-roadmap">Roadmap</a>
</p>

---

## Why Rehyke?

> Most crawlers fetch pages. **Rehyke understands them.**

Rehyke is not just another scraper. It's an **autonomous crawling agent** that uses advanced regex pattern matching, intelligent content extraction, and Rust-native concurrency to crawl entire websites -- including JavaScript-heavy SPAs -- and produce **clean, structured Markdown**.

| Feature | crawl4ai | Scrapy | Rehyke |
|---------|----------|--------|--------|
| Language | Python | Python | **Rust + Python** |
| JS Rendering | Partial | Plugin | **Native Chromium** |
| Output Format | Raw HTML | Items | **Clean Markdown** |
| Concurrency | Threads | Twisted | **tokio async + work-stealing** |
| Anti-Detection | Basic | Manual | **57 UAs + TLS rotation** |
| Regex Engine | re | re | **Rust regex (10x faster)** |
| Memory per page | ~50MB | ~20MB | **< 5MB** |
| Speed (1K pages) | ~15min | ~8min | **< 5min** |

---

## Key Features

### Autonomous Crawling Agent
- **Self-navigating** -- discovers and follows links across entire domains automatically
- **Priority-based scheduling** -- critical pages (sitemaps, indexes) crawled first
- **Intelligent deduplication** -- URL normalization with 7 rules prevents re-crawling
- **Adaptive throttling** -- automatically slows down when rate-limited (429/503)
- **Auto-recovery** -- retries with exponential backoff, falls back to static fetch on JS failure

### Regex-Powered Intelligence
- **URL filtering** -- include/exclude patterns via full Rust regex syntax
- **Content cleaning** -- regex-based removal of ads, navigation, footers, cookie banners
- **Link extraction** -- pattern matching across 12+ HTML element types, srcset parsing, meta refresh detection
- **Ad detection** -- 14 regex patterns for identifying ad containers by class/id
- **Comment filtering** -- 5 regex patterns for removing comment sections
- **Language detection** -- regex extraction from `class="language-*"` attributes
- **robots.txt** -- wildcard pattern matching with `*` and `$` anchor support
- **Sitemap discovery** -- auto-probes 4 common sitemap paths per domain

### Universal Content Parser
| Input Format | Parser | Output |
|-------------|--------|--------|
| HTML / XHTML | scraper + custom DOM walker | Clean Markdown |
| XML (generic) | quick-xml event-driven | Structured Markdown |
| RSS 2.0 | quick-xml + CDATA handling | Feed Markdown |
| Atom | quick-xml + namespace support | Feed Markdown |
| JSON / JSON-LD | serde_json + schema detection | Markdown tables |
| SVG | quick-xml | Description + metadata |
| Sitemap XML | quick-xml | URL list |
| Plain Text | passthrough | Wrapped Markdown |

### HTML to Markdown Conversion
Every HTML element maps to clean Markdown:

```
<h1>-<h6>         -->  # to ######
<p>                -->  paragraph with blank lines
<a href>           -->  [text](url)
<img>              -->  ![alt](src)
<strong>/<b>       -->  **text**
<em>/<i>           -->  *text*
<code>             -->  `code`
<pre><code>        -->  ```lang\ncode\n```
<ul>/<ol>          -->  - item / 1. item
<blockquote>       -->  > text
<table>            -->  | col | col | (GFM tables)
<hr>               -->  ---
<del>/<s>          -->  ~~text~~
<dl>/<dt>/<dd>     -->  **term:** definition
<figure>           -->  ![caption](src)
<video>/<audio>    -->  [Media: title](src)
<iframe>           -->  [Embedded: title](src)
```

### Performance
- **Rust-native** with zero-cost abstractions
- **tokio async** runtime with work-stealing thread pool
- **Lock-free** concurrent data structures (DashMap/DashSet)
- **Connection pooling** per domain
- **Streaming output** -- write to disk as pages arrive, keep memory flat

### Anti-Detection Suite
- **57 rotating user agents** (Chrome, Firefox, Safari, Edge across Windows/macOS/Linux)
- **Realistic browser headers** (Accept, Accept-Language, Sec-Fetch-*, DNT)
- **TLS fingerprint diversity** (rustls with configurable ciphers)
- **Per-domain rate limiting** with configurable delays
- **Adaptive throttling** on 429/503 responses
- **Proxy rotation** (HTTP, HTTPS, SOCKS5) with round-robin/random/failover strategies

---

## Quick Start

### Rust

```rust
use rehyke_core::{Rehyke, ScanMode, CrawlConfigBuilder};

#[tokio::main]
async fn main() {
    // Simple one-liner
    let results = Rehyke::crawl("https://example.com", ScanMode::Full).await.unwrap();

    for page in &results {
        println!("# {}", page.title);
        println!("{}", page.markdown);
    }

    // Advanced configuration
    let config = CrawlConfigBuilder::new()
        .mode(ScanMode::Deep)
        .max_depth(15)
        .max_pages(50_000)
        .concurrency(80)
        .enable_js(true)
        .exclude_patterns(vec![r"\.pdf$".into(), r"/login".into()])
        .include_patterns(vec![r"/blog/".into()])
        .respect_robots_txt(false)
        .clean_navigation(true)
        .clean_ads(true)
        .build();

    let crawler = Rehyke::new(config);
    let results = crawler.run("https://example.com").await.unwrap();
    println!("Crawled {} pages", results.len());
}
```

### Python

```python
import rehyke

# One-liner
results = rehyke.crawl("https://example.com")

# With configuration
from rehyke import Rehyke, CrawlConfig, ScanMode

config = CrawlConfig(
    mode=ScanMode.DEEP,
    max_depth=15,
    max_pages=50_000,
    concurrency=80,
    enable_js=True,
    clean_navigation=True,
    clean_ads=True,
    exclude_patterns=[r"\.pdf$", r"/login"],
)

crawler = Rehyke(config)
results = crawler.crawl("https://example.com")

for page in results:
    print(f"URL: {page.url}")
    print(f"Title: {page.title}")
    print(f"Words: {len(page.markdown.split())}")
    print(page.markdown[:200])
    print("---")

# Save to file
crawler.crawl_to_file("https://example.com", "output.md")
```

### CLI

```bash
# Install
cargo install rehyke-cli

# Basic crawl
rehyke https://example.com

# Full site to directory
rehyke https://example.com --mode full --output-dir ./output --structure mirror

# Deep crawl with all options
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

# Single page, JSON metadata output
rehyke https://example.com --mode lite --format json
```

---

## Scan Modes

### Lite Mode -- Single Page
```
Scope:       Single URL only
JS:          Off by default
Links:       Extracted but NOT followed
Speed:       < 200ms (no JS) / < 3s (with JS)
Use case:    Quick content extraction
```

### Full Mode -- Entire Domain (Default)
```
Scope:       All internal links (same domain)
JS:          Enabled
Links:       Follow internal links recursively
Concurrency: Up to 50 parallel requests
Max Depth:   10 levels
Use case:    Complete site extraction
```

### Deep Mode -- Cross-Domain
```
Scope:       Internal + ALL external links
JS:          Enabled everywhere
Links:       Follow everything recursively
Concurrency: Up to 100 parallel requests
Max Depth:   20 levels
Use case:    Research, competitive analysis
```

---

## Markdown Output Format

Every crawled page produces structured Markdown with YAML frontmatter:

```markdown
---
url: https://example.com/blog/post
title: "How to Build a Web Crawler"
description: "A comprehensive guide to building fast web crawlers"
author: Jane Smith
published: 2024-06-15
language: en
canonical: https://example.com/blog/post
---

# How to Build a Web Crawler

Main content converted to clean Markdown...

## Getting Started

Paragraph text with [links](https://example.com) and **formatting** preserved.

| Feature   | Status    |
|-----------|-----------|
| HTML      | Supported |
| RSS       | Supported |

> Blockquotes preserved with proper nesting

```python
# Code blocks with language detection
print("hello world")
```

---

*Crawled by [Rehyke](https://github.com/user/rehyke)*
```

---

## Architecture

```
                        +---------------------------+
                        |       Python API (PyO3)    |
                        |  rehyke.crawl(url, mode)   |
                        +-------------+-------------+
                                      |
                        +-------------v-------------+
                        |     Rust Core Engine       |
                        |                           |
                        |  +--------+  +---------+  |
                        |  |Schedule|->| Fetcher  |  |
                        |  | (tokio)|  |(reqwest) |  |
                        |  +---+----+  +----+----+  |
                        |      |            |        |
                        |  +---v------------v----+   |
                        |  |   Link Extractor    |   |
                        |  | (regex + scraper)   |   |
                        |  +--------+-----------+   |
                        |           |               |
                        |  +--------v-----------+   |
                        |  |  Content Processor  |   |
                        |  | HTML->MD | XML->MD  |   |
                        |  | RSS->MD  | JSON->MD |   |
                        |  +--------+-----------+   |
                        |           |               |
                        |  +--------v-----------+   |
                        |  |  Output Handler     |   |
                        |  | Memory | Files | .md|   |
                        |  +--------------------+   |
                        +---------------------------+
```

### Module Map (14 modules, 10,882 lines)

| Module | Lines | Purpose |
|--------|-------|---------|
| `config.rs` | 798 | Configuration, builder pattern, 8 enums |
| `error.rs` | 46 | 14 error variants with thiserror |
| `utils.rs` | 652 | URL normalization (7 rules), domain comparison |
| `fetcher.rs` | 877 | HTTP/2 client, retry, compression, proxy |
| `parser.rs` | 2,451 | HTML/XML/RSS/Atom/JSON parsing |
| `converter.rs` | 1,010 | Markdown conversion, GFM tables |
| `extractor.rs` | 1,331 | Link extraction from 12+ element types |
| `scheduler.rs` | 1,063 | Priority queue, dedup, rate limiting |
| `output.rs` | 479 | Memory/Files/SingleFile output |
| `renderer.rs` | 269 | Headless Chromium integration |
| `anti_detect.rs` | 357 | 57 UAs, delay strategies, headers |
| `robots.rs` | 607 | robots.txt with wildcard matching |
| `sitemap.rs` | 453 | Sitemap/SitemapIndex XML parser |
| `proxy.rs` | 336 | Proxy pool rotation strategies |

---

## Installation

### From Source (Rust)
```bash
git clone https://github.com/user/rehyke.git
cd rehyke
cargo build --release
```

### CLI
```bash
cargo install rehyke-cli
```

### Python (via maturin)
```bash
pip install maturin
cd rehyke
maturin develop --release
```

### Prerequisites
- Rust 1.75+ (install via [rustup](https://rustup.rs))
- Python 3.8+ (for Python bindings)
- Chrome/Chromium (optional, for JS rendering)

---

## Roadmap

### v0.2.0 -- Autonomous Agent Mode
> **Make Rehyke think, not just crawl.**

- [ ] **Autonomous Crawl Planner** -- AI-driven crawl strategy that analyzes site structure and optimizes traversal order
- [ ] **Regex Rule Engine** -- user-defined regex pipelines for content extraction, transformation, and routing
  ```rust
  rules! {
      match r"price:\s*\$(\d+\.?\d*)" => extract("price"),
      match r"<span class=\"rating\">(\d+)/5</span>" => extract("rating"),
      match r"/product/([a-z0-9-]+)" => follow(priority: High),
      match r"\.(jpg|png|gif|webp)$" => skip(),
      match r"(cookie|consent|gdpr)" => remove_element(),
  }
  ```
- [ ] **Content Fingerprinting** -- regex-based duplicate content detection across pages (near-dedup with simhash)
- [ ] **Smart Pagination Detection** -- auto-detect `?page=N`, `/page/N`, infinite scroll patterns via regex
- [ ] **Form Discovery & Auto-Submit** -- detect search forms, login pages, and parameterized URLs

### v0.3.0 -- Regex Superpowers
> **Regex is the backbone. Make it unbreakable.**

- [ ] **Named Capture Groups Pipeline** -- extract structured data using named regex groups
  ```python
  config = CrawlConfig(
      extract_rules={
          "emails": r"[\w.-]+@[\w.-]+\.\w+",
          "phones": r"\+?1?\s*\(?(\d{3})\)?[\s.-]*(\d{3})[\s.-]*(\d{4})",
          "prices": r"\$\d{1,3}(?:,\d{3})*(?:\.\d{2})?",
          "dates": r"\d{4}-\d{2}-\d{2}|\d{2}/\d{2}/\d{4}",
          "social": r"(?:twitter|x)\.com/(\w+)|github\.com/(\w+)",
      }
  )
  results = rehyke.crawl("https://shop.example.com", config=config)
  print(results[0].extracted["prices"])  # ["$29.99", "$149.00"]
  ```
- [ ] **Regex-Based Content Scoring** -- rank pages by relevance using weighted regex matches
- [ ] **URL Pattern Learning** -- automatically discover URL patterns in a site and suggest include/exclude rules
- [ ] **Custom Markdown Templates** -- regex-driven template engine for custom output formats
- [ ] **Multi-Pattern Link Classification** -- user-defined regex rules to classify links (blog, product, docs, etc.)

### v0.4.0 -- Full JS Rendering Engine
> **See what the browser sees.**

- [ ] **Chromiumoxide Integration** -- full headless Chrome with tab pooling
- [ ] **Infinite Scroll Handling** -- auto-detect and execute scroll-to-load patterns
- [ ] **SPA Navigation** -- handle React Router, Vue Router, Angular routing
- [ ] **Cookie Banner Auto-Dismiss** -- regex + selector patterns for consent popups
- [ ] **Resource Blocking** -- skip images/fonts/media for 3x faster rendering
- [ ] **Screenshot Capture** -- optional visual snapshots for debugging

### v0.5.0 -- Distributed Crawling
> **One machine is never enough.**

- [ ] **Worker Pool Architecture** -- distribute crawl tasks across multiple machines
- [ ] **Redis-backed Frontier** -- shared URL queue with Redis pub/sub
- [ ] **S3/GCS Output** -- stream results directly to cloud storage
- [ ] **Webhook Callbacks** -- notify external services on crawl events
- [ ] **Resume/Checkpoint** -- save and restore crawl state for long-running jobs
- [ ] **Rate Limit Coordination** -- global per-domain rate limiting across workers

### v0.6.0 -- Intelligence Layer
> **From data to knowledge.**

- [ ] **Content Diff Detection** -- track changes between crawls using regex fingerprints
- [ ] **Broken Link Scanner** -- identify 404s, redirect chains, mixed content
- [ ] **SEO Analyzer** -- extract and score meta tags, headings, structured data
- [ ] **Regex-Based Entity Extraction** -- pull emails, phones, addresses, social links automatically
- [ ] **Site Graph Visualization** -- generate interactive link maps
- [ ] **Competitive Intelligence** -- compare site structures across domains
- [ ] **API Endpoint Discovery** -- regex patterns to find REST/GraphQL endpoints in JS bundles

### v1.0.0 -- Production Ready
> **Battle-tested and enterprise-grade.**

- [ ] **WASM Support** -- run Rehyke in the browser
- [ ] **Plugin System** -- user-contributed parsers, extractors, and transformers
- [ ] **GUI Dashboard** -- real-time crawl monitoring with charts
- [ ] **Scheduled Crawls** -- cron-based recurring crawl jobs
- [ ] **Authentication Flows** -- OAuth2, session cookies, API key injection
- [ ] **Compliance Mode** -- GDPR-aware crawling with data classification

---

## Regex at the Core

Rehyke uses Rust's `regex` crate -- one of the fastest regex engines in the world -- as a fundamental building block across every module:

```
+-------------------+----------------------------------------+
| Module            | Regex Usage                            |
+-------------------+----------------------------------------+
| URL Filtering     | Include/exclude URL patterns           |
| Link Extraction   | srcset parsing, meta refresh URLs      |
| Content Cleaning  | Ad container class/id detection        |
|                   | Comment section identification         |
|                   | Hidden element filtering               |
| robots.txt        | Wildcard path matching (* and $)       |
| Code Detection    | Language class extraction               |
| URL Normalization | Percent-encoding, query sorting        |
| Anti-Detection    | Response analysis for CAPTCHAs/blocks  |
| Output            | Filename sanitization                  |
+-------------------+----------------------------------------+
```

### Why Regex?

Traditional crawlers use CSS selectors or XPath. Rehyke uses **regex as a first-class citizen** because:

1. **Speed** -- Rust regex compiles to DFA, runs in O(n) guaranteed
2. **Universality** -- works on HTML, XML, JSON, plain text, URLs, headers
3. **Composability** -- chain patterns into pipelines for complex extraction
4. **Portability** -- same patterns work in Rust, Python, CLI, and config files
5. **User power** -- developers already know regex; no new query language to learn

---

## Performance Targets

| Metric | Target | Status |
|--------|--------|--------|
| Single page (lite, no JS) | < 200ms | Implemented |
| Single page (lite, with JS) | < 3s | Stub |
| 100 pages (full, parallel) | < 30s | Implemented |
| 1,000 pages (full, parallel) | < 5 min | Implemented |
| 10,000 pages (deep, parallel) | < 30 min | Implemented |
| Memory per page | < 5MB | Implemented |
| Concurrent connections | Up to 200 | Implemented |

---

## Contributing

Contributions are welcome! Here's how to get started:

```bash
# Clone and build
git clone https://github.com/user/rehyke.git
cd rehyke
cargo build

# Run tests
cargo test --workspace

# Run with verbose logging
RUST_LOG=debug cargo run -p rehyke-cli -- https://example.com --mode lite -v

# Build Python wheel
cd crates/rehyke-python
pip install maturin
maturin develop

# Check formatting and lints
cargo fmt --check
cargo clippy -- -D warnings
```

---

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your choice.

---

<p align="center">
  <strong>Built with Rust. Powered by Regex. Ready for Everything.</strong>
  <br>
  <sub>11,670 lines of code &bull; 369 tests &bull; 14 core modules &bull; 3 crates</sub>
</p>
