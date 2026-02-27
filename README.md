<p align="center">
  <img src="https://img.shields.io/badge/built%20with-Rust-e43717?style=for-the-badge&logo=rust&logoColor=white" alt="Built with Rust"/>
  <img src="https://img.shields.io/badge/python-3.8+-blue?style=for-the-badge&logo=python&logoColor=white" alt="Python 3.8+"/>
  <img src="https://img.shields.io/badge/license-MIT%20%7C%20Apache--2.0-green?style=for-the-badge" alt="License"/>
  <img src="https://img.shields.io/badge/version-0.2.0-blueviolet?style=for-the-badge" alt="Version 0.2.0"/>
  <img src="https://img.shields.io/badge/tests-473%20passing-brightgreen?style=for-the-badge" alt="Tests"/>
  <img src="https://img.shields.io/badge/lines-13k+-orange?style=for-the-badge" alt="Lines of Code"/>
</p>

<h1 align="center">
  <br>
  REHYKE
  <br>
</h1>

<h3 align="center">Crawl Everything. Miss Nothing.</h3>

<p align="center">
  <strong>Ultra-high-performance, regex-powered autonomous web crawler</strong><br>
  Built in Rust &bull; Python-ready &bull; Markdown output &bull; Headless Chrome
  <br><em>The last crawler you'll ever need.</em>
</p>

<p align="center">
  <strong>🆕 v0.2.0 — "Chrome Eyes"</strong> &bull;
  Headless Chromium &bull; SPA detection &bull; Infinite scroll &bull; Screenshots &bull; Fingerprint diversity
</p>

---

<p align="center">
  <a href="#why-rehyke">Why Rehyke?</a> &bull;
  <a href="#key-features">Features</a> &bull;
  <a href="#quick-start">Quick Start</a> &bull;
  <a href="#javascript-rendering-v020">JS Rendering</a> &bull;
  <a href="#scan-modes">Scan Modes</a> &bull;
  <a href="#crawlresult-fields">CrawlResult</a> &bull;
  <a href="#python-api">Python API</a> &bull;
  <a href="#cli-reference">CLI Reference</a> &bull;
  <a href="#output-modes">Output Modes</a> &bull;
  <a href="#architecture">Architecture</a> &bull;
  <a href="#installation">Installation</a> &bull;
  <a href="#roadmap">Roadmap</a>
</p>

---

## Why Rehyke?

> Most crawlers fetch pages. **Rehyke understands them.**

Rehyke is not just another scraper. It's an **autonomous crawling agent** that uses advanced regex pattern matching, intelligent content extraction, and Rust-native concurrency to crawl entire websites — including JavaScript-heavy SPAs — and produce **clean, structured Markdown**.

| Feature | crawl4ai | Scrapy | **Rehyke** |
|---------|----------|--------|--------|
| Language | Python | Python | **Rust + Python** |
| JS Rendering | Partial | Plugin | **Native Chromium (v0.2.0)** |
| SPA Support | Partial | No | **React/Vue/Angular/Svelte/Next/Nuxt** |
| Output Format | Raw HTML | Items | **Clean Markdown + YAML frontmatter** |
| Concurrency | Threads | Twisted | **tokio async + work-stealing** |
| Anti-Detection | Basic | Manual | **57 UAs + fingerprint diversity** |
| Regex Engine | `re` | `re` | **Rust regex (DFA, O(n) guaranteed)** |
| Memory per page | ~50 MB | ~20 MB | **< 5 MB** |
| Speed (1K pages) | ~15 min | ~8 min | **< 5 min** |

---

## Key Features

### JavaScript Rendering (v0.2.0)
- **Headless Chromium** via Chrome DevTools Protocol — optional `js` feature flag, graceful static fallback
- **Four wait strategies** — `NetworkIdle`, `Selector { css }`, `Duration`, `Auto`
- **SPA framework detection** — auto-identifies React, Vue, Angular, Svelte, Next.js, Nuxt, SvelteKit
- **Infinite scroll** — scrolls N viewport-heights, triggering Intersection-Observer loaders
- **Popup & cookie banner dismissal** — 30+ CSS selectors + JS text-matching fallback
- **Browser fingerprint diversity** — randomised UA, WebGL vendor/renderer, languages, timezone, canvas noise
- **Viewport profiles** — Desktop (1920×1080), Tablet (768×1024, 2×), Mobile (390×844, 3× DPR, touch)
- **Full-page screenshots** — PNG (lossless) or JPEG per crawled page, named by URL slug

### Autonomous Crawling
- **Self-navigating** — discovers and follows links across entire domains automatically
- **Priority-based scheduling** — sitemaps and indexes crawled first via BinaryHeap
- **Intelligent deduplication** — URL normalization with 7 rules prevents re-crawling
- **Adaptive throttling** — automatically backs off on 429/503 responses
- **Auto-recovery** — retries with exponential backoff; falls back to static fetch on JS failure

### Regex-Powered Intelligence
- **URL filtering** — include/exclude patterns via full Rust regex syntax
- **Content cleaning** — regex-based removal of ads, navigation, footers, cookie banners
- **Link extraction** — 12+ HTML element types, srcset parsing, meta refresh detection
- **robots.txt** — wildcard `*` and `$` anchor support
- **Sitemap discovery** — auto-probes 4 common sitemap paths per domain

### Universal Content Parser

| Input | Parser | Output |
|-------|--------|--------|
| HTML / XHTML | scraper + custom DOM walker | Clean Markdown |
| XML (generic) | quick-xml event-driven | Structured Markdown |
| RSS 2.0 | quick-xml + CDATA | Feed Markdown |
| Atom | quick-xml + namespaces | Feed Markdown |
| JSON / JSON-LD | serde_json + schema detection | Markdown tables |
| SVG | quick-xml | Description + metadata |
| Sitemap XML | quick-xml | URL list |
| Plain Text | passthrough | Wrapped Markdown |

### Anti-Detection Suite
- **57 rotating user agents** (Chrome, Firefox, Safari, Edge — Windows/macOS/Linux)
- **Realistic browser headers** (Accept, Accept-Language, Sec-Fetch-*, DNT)
- **Per-domain rate limiting** with configurable delays
- **Proxy rotation** — HTTP, HTTPS, SOCKS5 with round-robin/random/failover

---

## Quick Start

### Rust

```rust
use rehyke_core::{CrawlConfigBuilder, Rehyke, ScanMode};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── One-liner ──────────────────────────────────────────────────────────
    let results = Rehyke::crawl("https://example.com", ScanMode::Lite).await?;
    println!("{}", results[0].markdown);

    // ── Static crawl with configuration ────────────────────────────────────
    let config = CrawlConfigBuilder::new()
        .mode(ScanMode::Full)
        .max_pages(500)
        .concurrency(20)
        .clean_navigation(true)
        .clean_ads(true)
        .exclude_patterns(vec![r"\.pdf$".into(), r"/login".into()])
        .include_patterns(vec![r"/blog/".into()])
        .respect_robots_txt(true)
        .build();

    let results = Rehyke::new(config).run("https://example.com").await?;

    for page in &results {
        println!("[{}] {} — {} words",
            page.status_code, page.title,
            page.markdown.split_whitespace().count());
    }

    Ok(())
}
```

### Python

```python
import rehyke

# ── One-liner ──────────────────────────────────────────────────────────────
results = rehyke.crawl("https://example.com", mode="lite")
print(results[0].markdown)

# ── Static crawl with configuration ───────────────────────────────────────
from rehyke import Rehyke, CrawlConfig, ScanMode

config = CrawlConfig(
    mode=ScanMode.FULL,
    max_pages=500,
    concurrency=20,
    clean_navigation=True,
    clean_ads=True,
    exclude_patterns=[r"\.pdf$", r"/login"],
    include_patterns=[r"/blog/"],
    respect_robots_txt=True,
)

crawler = Rehyke(config)
results = crawler.crawl("https://example.com")

for page in results:
    print(f"[{page.status_code}] {page.title} — {len(page.markdown.split())} words")
    print(f"  render: {page.render_method}  depth: {page.depth}")

# Save all pages to a single Markdown file
crawler.crawl_to_file("https://example.com", "site.md")
```

### CLI

```bash
# Basic crawl — prints Markdown to stdout
rehyke https://example.com

# Lite: single page only
rehyke https://example.com --mode lite

# Full site to a mirrored directory
rehyke https://example.com --mode full --output-dir ./output --structure mirror

# Deep crawl with all tuning knobs
rehyke https://example.com \
    --mode deep \
    --max-depth 20 \
    --max-pages 100000 \
    --concurrency 100 \
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
```

---

## JavaScript Rendering (v0.2.0)

> Requires Chrome/Chromium installed and the `js` feature flag.
> Without Chrome, Rehyke **automatically falls back** to static fetch — no crash, no error.

### Rust — JS Rendering

```rust
use rehyke_core::{CrawlConfigBuilder, Rehyke, ScanMode, ScreenshotFormat, Viewport, WaitStrategy};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── React / Next.js — network idle wait ────────────────────────────────
    let config = CrawlConfigBuilder::new()
        .mode(ScanMode::Full)
        .enable_js(true)
        .js_wait_strategy(WaitStrategy::NetworkIdle)   // wait until XHR/fetch settle
        .js_wait_timeout(Duration::from_secs(12))
        .js_scroll_count(8)          // scroll 8 viewport-heights for lazy content
        .dismiss_popups(true)        // auto-click Accept on cookie banners
        .detect_spa(true)            // result.detected_framework = Some("React")
        .viewport(Viewport::Desktop) // 1920×1080
        .randomize_fingerprint(true) // randomise UA, WebGL, timezone per crawl
        .max_pages(200)
        .build();

    let results = Rehyke::new(config).run("https://my-react-app.com").await?;
    for page in &results {
        println!("[{:?}] {} — {} words",
            page.render_method, page.title,
            page.markdown.split_whitespace().count());
    }

    // ── Vue / Nuxt — wait for a specific DOM element ───────────────────────
    let config = CrawlConfigBuilder::new()
        .mode(ScanMode::Full)
        .enable_js(true)
        .js_wait_strategy(WaitStrategy::Selector {
            selector: "#app, [data-v-app], #__nuxt".into(),
        })
        .js_wait_timeout(Duration::from_secs(10))
        .js_scroll_count(5)
        .viewport(Viewport::Desktop)
        .detect_spa(true)
        .build();
    let _ = Rehyke::new(config).run("https://my-vue-app.com").await?;

    // ── Angular — fixed settle time after network idle ─────────────────────
    let config = CrawlConfigBuilder::new()
        .mode(ScanMode::Full)
        .enable_js(true)
        .js_wait_strategy(WaitStrategy::Duration {
            duration: Duration::from_millis(1500),
        })
        .js_wait_timeout(Duration::from_secs(15))
        .viewport(Viewport::Desktop)
        .build();
    let _ = Rehyke::new(config).run("https://my-angular-app.com").await?;

    // ── Mobile viewport ────────────────────────────────────────────────────
    let config = CrawlConfigBuilder::new()
        .mode(ScanMode::Lite)
        .enable_js(true)
        .js_wait_strategy(WaitStrategy::Auto)
        .viewport(Viewport::Mobile) // 390×844, 3× DPR, touch enabled
        .randomize_fingerprint(true)
        .build();
    let _ = Rehyke::new(config).run("https://example.com").await?;

    // ── Screenshot every page ──────────────────────────────────────────────
    let config = CrawlConfigBuilder::new()
        .mode(ScanMode::Lite)
        .enable_js(true)
        .js_wait_strategy(WaitStrategy::Auto)
        .screenshot(true)
        .screenshot_format(ScreenshotFormat::Png)       // or Jpeg
        .screenshot_output_dir("/tmp/shots".into())     // {dir}/{url-slug}.png
        .viewport(Viewport::Desktop)
        .build();
    let _ = Rehyke::new(config).run("https://example.com").await?;

    Ok(())
}
```

### Python — JS Rendering

```python
import rehyke

# ── React / Next.js ────────────────────────────────────────────────────────
config = rehyke.CrawlConfig(
    enable_js=True,
    js_wait_strategy="network_idle",   # "auto" | "network_idle" | "selector:<CSS>" | float
    js_wait_timeout=12.0,
    scroll_count=8,          # scroll 8 viewport-heights for lazy-loaded content
    dismiss_popups=True,     # auto-dismiss cookie/GDPR banners
    detect_spa=True,         # page.render_method == "javascript"
    viewport="desktop",      # "desktop" | "tablet" | "mobile"
    randomize_fingerprint=True,
    max_pages=200,
)
results = rehyke.Rehyke(config).crawl("https://my-react-app.com")
for page in results:
    print(f"[{page.render_method}] {page.title}")

# ── Vue / Nuxt — wait for the app root element ────────────────────────────
config = rehyke.CrawlConfig(
    enable_js=True,
    js_wait_strategy="selector:#app",  # poll until #app is in the DOM
    viewport="desktop",
)
results = rehyke.Rehyke(config).crawl("https://my-vue-app.com")

# ── Angular — fixed 1.5 s settle period ───────────────────────────────────
config = rehyke.CrawlConfig(
    enable_js=True,
    js_wait_strategy=1.5,   # float = seconds (Duration wait)
    viewport="desktop",
)
results = rehyke.Rehyke(config).crawl("https://my-angular-app.com")

# ── Mobile viewport ────────────────────────────────────────────────────────
config = rehyke.CrawlConfig(
    enable_js=True,
    js_wait_strategy="auto",
    viewport="mobile",       # 390×844, 3× DPR, touch emulation
    randomize_fingerprint=True,
)
results = rehyke.Rehyke(config).crawl("https://example.com")

# ── Screenshot capture ─────────────────────────────────────────────────────
config = rehyke.CrawlConfig(
    enable_js=True,
    js_wait_strategy="auto",
    screenshot=True,
    screenshot_format="png",        # "png" | "jpeg"
    screenshot_dir="/tmp/shots",    # files named {url-slug}.png
    viewport="desktop",
)
rehyke.Rehyke(config).crawl("https://example.com")
```

### CLI — JS Rendering

```bash
# React SPA: network-idle wait, popup dismissal, fingerprint randomisation
rehyke https://my-react-app.com \
    --js \
    --wait-for '#root' \
    --js-timeout 12 \
    --scroll 8 \
    --dismiss-popups \
    --detect-spa \
    --viewport desktop \
    --randomize-fingerprint \
    --mode full

# Vue/Nuxt: wait for DOM selector
rehyke https://my-vue-app.com --js --wait-for '#app' --mode full

# Angular: fixed 1.5 s settle time (no --wait-for needed)
rehyke https://my-angular-app.com --js --js-timeout 15 --mode full

# Mobile viewport
rehyke https://example.com --js --viewport mobile --mode lite

# Desktop PNG screenshot
rehyke https://example.com \
    --js --screenshot --screenshot-format png \
    --screenshot-dir ./shots --viewport desktop --mode lite

# Run bundled examples (requires --features js build)
cargo run --example js_render          --features js
cargo run --example spa_crawl          --features js
cargo run --example screenshot_capture --features js
python examples/python_js_render.py
```

---

## Scan Modes

| Mode | Max depth | Max pages | Concurrency | Scope |
|------|-----------|-----------|-------------|-------|
| **Lite** | 2 | 100 | 5 | Seed URL only — no link following |
| **Full** | 5 | 1 000 | 10 | Entire domain (default) |
| **Deep** | 50 | 50 000 | 25 | Internal + all external links |

### Lite — Single Page

```bash
rehyke https://example.com --mode lite          # CLI
```
```rust
Rehyke::crawl("https://example.com", ScanMode::Lite).await?;
```
```python
rehyke.crawl("https://example.com", mode="lite")
```

Best for: quick content extraction, screenshot of a single page, API scraping.

### Full — Entire Domain (Default)

```bash
rehyke https://example.com --mode full
```
```rust
CrawlConfigBuilder::new().mode(ScanMode::Full).build()
```
```python
rehyke.CrawlConfig(mode=rehyke.ScanMode.FULL)
```

Best for: documentation sites, blogs, product catalogs, complete site backup.

### Deep — Cross-Domain

```bash
rehyke https://example.com --mode deep
```
```rust
CrawlConfigBuilder::new().mode(ScanMode::Deep).build()
```
```python
rehyke.CrawlConfig(mode=rehyke.ScanMode.DEEP)
```

Best for: research, competitive analysis, link graph construction, broad topic crawling.

> JS rendering is **off by default** in all modes. Enable it with `enable_js=True` / `--js`.

---

## CrawlResult Fields

Every crawled page returns a `CrawlResult` with these fields:

| Field | Type | Description |
|-------|------|-------------|
| `url` | `String` | Final URL after any redirects |
| `title` | `String` | Page `<title>` tag content |
| `markdown` | `String` | Full page content as clean Markdown |
| `metadata` | `PageMetadata` | Author, description, published date, language, canonical, OG tags |
| `links` | `ExtractedLinks` | `.internal`, `.external`, `.subdomains` — all as `Vec<String>` |
| `status_code` | `u16` | HTTP response status (200, 301, 404, …) |
| `content_type` | `String` | Raw `Content-Type` header value |
| `render_method` | `RenderMethod` | `Static` or `JavaScript` (v0.2.0) |
| `depth` | `u32` | Link depth from the seed URL (0 = seed) |
| `crawled_at` | `DateTime<Utc>` | Timestamp when the page was fetched |

### Rust usage

```rust
let results = Rehyke::crawl("https://example.com", ScanMode::Lite).await?;
let page = &results[0];

println!("URL:     {}", page.url);
println!("Title:   {}", page.title);
println!("Status:  {}", page.status_code);
println!("Render:  {:?}", page.render_method);   // Static | JavaScript
println!("Depth:   {}", page.depth);
println!("Words:   {}", page.markdown.split_whitespace().count());
println!("Author:  {:?}", page.metadata.author);
println!("Internal links: {}", page.links.internal.len());
println!("External links: {}", page.links.external.len());
```

### Python usage

```python
results = rehyke.crawl("https://example.com", mode="full")
for page in results:
    print(f"url:     {page.url}")
    print(f"title:   {page.title}")
    print(f"status:  {page.status_code}")
    print(f"render:  {page.render_method}")   # "static" or "javascript"
    print(f"depth:   {page.depth}")
    print(f"words:   {len(page.markdown.split())}")
    print(page.markdown[:500])
```

### Markdown Output Format

Every page produces structured Markdown with YAML frontmatter:

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

Content converted to clean Markdown...

| Feature   | Status    |
|-----------|-----------|
| HTML      | ✅        |
| RSS/Atom  | ✅        |

> Blockquotes preserved with proper nesting

```python
# Code blocks with language detection
print("hello world")
```
```

---

## Python API

### Installation

```bash
# Build from source (recommended during development)
pip install maturin
git clone https://github.com/user/rehyke.git && cd rehyke
maturin develop --release                        # static fetch only
maturin develop --release --features js          # + headless Chrome
```

### `rehyke.crawl(url, mode)` — one-shot function

```python
import rehyke

# mode: "lite" | "full" | "deep"
results = rehyke.crawl("https://example.com", mode="full")
for page in results:
    print(page.title, page.markdown[:200])
```

### `CrawlConfig` — full parameter reference

```python
config = rehyke.CrawlConfig(
    # ── Scope ──────────────────────────────────────────────────────────────
    mode              = rehyke.ScanMode.FULL,   # LITE | FULL | DEEP
    max_depth         = 5,                      # max link-follow depth
    max_pages         = 1000,                   # hard page cap
    concurrency       = 10,                     # parallel requests

    # ── Network ────────────────────────────────────────────────────────────
    user_agent        = None,                   # custom User-Agent string
    timeout_secs      = 30,                     # per-request timeout
    max_retries       = 3,                      # retry attempts on failure
    respect_robots_txt= True,

    # ── Content cleaning ───────────────────────────────────────────────────
    clean_navigation  = True,                   # strip <nav> elements
    clean_footers     = True,                   # strip <footer> elements
    clean_ads         = True,                   # strip ad containers

    # ── URL filtering ──────────────────────────────────────────────────────
    exclude_patterns  = [r"\.pdf$", r"/login"], # skip matching URLs
    include_patterns  = [r"/blog/"],            # only follow matching URLs

    # ── Delays ─────────────────────────────────────────────────────────────
    delay_min_ms      = 300,                    # min delay between requests
    delay_max_ms      = 1200,                   # max delay (random in range)

    # ── JavaScript rendering (v0.2.0) ──────────────────────────────────────
    enable_js         = True,                   # enable headless Chrome
    js_wait_strategy  = "network_idle",         # "auto" | "network_idle"
                                                # | "selector:<CSS>" | float
    js_wait_timeout   = 10.0,                   # seconds before giving up
    scroll_count      = 5,                      # viewport scrolls for lazy content
    dismiss_popups    = True,                   # dismiss cookie/GDPR banners
    viewport          = "desktop",              # "desktop" | "tablet" | "mobile"
    detect_spa        = True,                   # auto-detect JS framework
    randomize_fingerprint = True,               # randomise UA, WebGL, timezone

    # ── Screenshots (v0.2.0) ───────────────────────────────────────────────
    screenshot        = True,
    screenshot_format = "png",                  # "png" | "jpeg"
    screenshot_dir    = "/tmp/shots",           # output directory
)
```

### `ScanMode` enum

```python
rehyke.ScanMode.LITE   # single page, max 100 pages, depth 2
rehyke.ScanMode.FULL   # full domain, max 1 000 pages, depth 5  (default)
rehyke.ScanMode.DEEP   # cross-domain, max 50 000 pages, depth 50
```

### `Rehyke` class

```python
crawler = rehyke.Rehyke(config)            # create with config
results = crawler.crawl("https://...")     # returns list[CrawlResult]
crawler.crawl_to_file("https://...", "output.md")  # write to file
```

### Common patterns

```python
import rehyke, json, csv

# ── Save each page as a separate file ─────────────────────────────────────
import os
config = rehyke.CrawlConfig(mode=rehyke.ScanMode.FULL)
results = rehyke.Rehyke(config).crawl("https://docs.example.com")

os.makedirs("docs_output", exist_ok=True)
for page in results:
    slug = page.url.replace("https://", "").replace("/", "_")[:80]
    with open(f"docs_output/{slug}.md", "w") as f:
        f.write(page.markdown)

# ── Export metadata to JSON ────────────────────────────────────────────────
meta = [{"url": p.url, "title": p.title, "words": len(p.markdown.split()),
         "render": p.render_method, "status": p.status_code}
        for p in results]
with open("metadata.json", "w") as f:
    json.dump(meta, f, indent=2)

# ── Filter JS-rendered pages ───────────────────────────────────────────────
config = rehyke.CrawlConfig(enable_js=True, js_wait_strategy="auto")
results = rehyke.Rehyke(config).crawl("https://spa.example.com")
js_pages   = [p for p in results if p.render_method == "javascript"]
stat_pages = [p for p in results if p.render_method == "static"]
print(f"JS: {len(js_pages)}  Static fallback: {len(stat_pages)}")

# ── Multi-target crawl ─────────────────────────────────────────────────────
targets = ["https://site-a.com", "https://site-b.com", "https://site-c.com"]
all_results = []
crawler = rehyke.Rehyke(rehyke.CrawlConfig(mode=rehyke.ScanMode.LITE))
for url in targets:
    all_results.extend(crawler.crawl(url))
print(f"Total pages: {len(all_results)}")
```

---

## CLI Reference

```bash
rehyke [OPTIONS] <URL>
```

### Core options

| Flag | Default | Description |
|------|---------|-------------|
| `--mode lite\|full\|deep` | `full` | Scan mode preset |
| `--max-depth N` | mode default | Maximum link-follow depth |
| `--max-pages N` | mode default | Hard page cap |
| `--concurrency N` | mode default | Parallel requests |
| `--timeout N` | `30` | Per-request timeout (seconds) |
| `--retries N` | `3` | Retry attempts on failure |
| `--user-agent STR` | rotating | Custom User-Agent string |
| `--no-robots` | — | Ignore robots.txt |
| `--verbose` / `-v` | — | Debug logging |

### URL filtering

| Flag | Description |
|------|-------------|
| `--exclude REGEX` | Skip URLs matching pattern (repeatable) |
| `--include REGEX` | Only follow URLs matching pattern (repeatable) |

### Content cleaning

| Flag | Description |
|------|-------------|
| `--clean-nav` | Strip `<nav>` elements |
| `--clean-footer` | Strip `<footer>` elements |
| `--clean-ads` | Strip ad containers |

### Output

| Flag | Default | Description |
|------|---------|-------------|
| `--output-dir DIR` | stdout | Write pages to directory |
| `--structure flat\|mirror` | `flat` | File naming strategy |
| `--single-file PATH` | — | Write all pages to one `.md` file |

### Delays & proxy

| Flag | Description |
|------|-------------|
| `--delay MS` or `--delay MIN-MAX` | Fixed or random inter-request delay (ms) |
| `--proxy URL` | Proxy URL (`http://`, `https://`, `socks5://`) |

### JavaScript rendering (v0.2.0)

| Flag | Description |
|------|-------------|
| `--js` | Enable headless Chrome rendering |
| `--wait-for SELECTOR` | Wait for CSS selector before extracting (implies `--js`) |
| `--js-timeout N` | Max wait time in seconds (default `10`) |
| `--scroll N` | Scroll N viewport-heights for infinite scroll (implies `--js`) |
| `--dismiss-popups` | Auto-dismiss cookie/GDPR banners (implies `--js`) |
| `--viewport desktop\|tablet\|mobile` | Browser viewport profile (default `desktop`) |
| `--detect-spa` | Auto-detect SPA framework (implies `--js`) |
| `--randomize-fingerprint` | Randomise UA, WebGL, timezone (implies `--js`) |

### Screenshots (v0.2.0)

| Flag | Description |
|------|-------------|
| `--screenshot` | Capture full-page screenshot (implies `--js`) |
| `--screenshot-format png\|jpeg` | Image format (default `png`) |
| `--screenshot-dir DIR` | Output directory for screenshots |

### Example commands

```bash
# Quick single-page extraction
rehyke https://example.com --mode lite

# Full blog crawl, save to mirrored directory
rehyke https://blog.example.com \
    --mode full --output-dir ./blog --structure mirror

# SPA crawl with JS rendering and screenshots
rehyke https://my-app.com \
    --js --wait-for '#app' --scroll 5 --dismiss-popups \
    --screenshot --screenshot-dir ./shots --viewport desktop \
    --mode full

# Deep research crawl with delays and proxy
rehyke https://example.com \
    --mode deep --delay 500-2000 \
    --proxy socks5://proxy.local:1080 \
    --exclude '\.pdf$' --exclude '/admin' \
    --concurrency 20 --verbose

# Mobile screenshot of a single page
rehyke https://example.com \
    --mode lite --js --viewport mobile \
    --screenshot --screenshot-format jpeg --screenshot-dir ./mobile-shots
```

---

## Output Modes

### Memory (default)

Results returned in-process. Use for scripting and programmatic access.

```rust
// Rust — default
let results = Rehyke::new(config).run("https://example.com").await?;
```
```python
# Python — always in-memory
results = crawler.crawl("https://example.com")
```

### Files

Write one Markdown file per page to a directory.

```rust
use rehyke_core::{CrawlConfigBuilder, FileStructure, OutputMode};
use std::path::PathBuf;

let config = CrawlConfigBuilder::new()
    .output(OutputMode::Files {
        output_dir: PathBuf::from("./output"),
        structure: FileStructure::Mirror,   // or Flat
    })
    .build();
```
```bash
rehyke https://example.com --output-dir ./output --structure mirror
```

`FileStructure::Mirror` preserves the URL path hierarchy:
```
./output/
  example.com/
    index.md
    blog/
      post-title.md
```

`FileStructure::Flat` puts everything in one directory:
```
./output/
  example-com-index.md
  example-com-blog-post-title.md
```

### SingleFile

Append all pages to a single `.md` file, separated by `---`.

```rust
use rehyke_core::{CrawlConfigBuilder, OutputMode};
use std::path::PathBuf;

let config = CrawlConfigBuilder::new()
    .output(OutputMode::SingleFile {
        output_path: PathBuf::from("./site.md"),
    })
    .build();
```
```bash
rehyke https://example.com --single-file site.md
```

---

## Architecture

```
              Python API (PyO3)          CLI (clap)
              rehyke.crawl(url)          rehyke <url> --js
                      |                        |
                      +----------+-------------+
                                 |
                    +------------v--------------+
                    |    Rust Core Engine        |
                    |                           |
                    |  +---------+  +---------+ |
                    |  |Scheduler|->| Fetcher | |  ← static path
                    |  | (tokio) |  |(reqwest)| |
                    |  +----+----+  +----+----+ |
                    |       |            |      |
                    |       |   +--------v----+ |
                    |       |   | JS Renderer | |  ← v0.2.0 path
                    |       |   |chromiumoxide| |    wait strategies
                    |       |   |SPA detection| |    popup dismissal
                    |       |   |scroll/shots | |    screenshots
                    |       |   +--------+----+ |
                    |       |            |      |
                    |  +----v------------v----+ |
                    |  |    Link Extractor    | |
                    |  |  (regex + scraper)   | |
                    |  +--------+-------------+ |
                    |           |               |
                    |  +--------v-------------+ |
                    |  |  Content Processor   | |
                    |  | HTML→MD  XML→MD      | |
                    |  | RSS→MD   JSON→MD     | |
                    |  +--------+-------------+ |
                    |           |               |
                    |  +--------v-------------+ |
                    |  |   Output Handler     | |
                    |  | Memory | Files | .md | |
                    |  +---------------------+ |
                    +---------------------------+
```

### Module Map (15 modules, 13 k+ lines)

| Module | Purpose |
|--------|---------|
| `config.rs` | Configuration, builder, 10 enums (Viewport, ScreenshotFormat, WaitStrategy, …) |
| `error.rs` | 14 error variants with `thiserror` |
| `utils.rs` | URL normalization (7 rules), domain comparison, slug generation |
| `fetcher.rs` | HTTP/2 client, retry, gzip/brotli/zstd, proxy, content-type detection |
| `parser.rs` | HTML/XML/RSS/Atom/JSON/SVG/Sitemap parsing, metadata extraction |
| `converter.rs` | Markdown conversion, GFM tables, YAML frontmatter |
| `extractor.rs` | Link extraction from 12+ element types, srcset, meta refresh |
| `scheduler.rs` | BinaryHeap priority queue, dedup, per-domain rate limiting |
| `output.rs` | Memory / Files / SingleFile output handler |
| `renderer.rs` | Headless Chrome (CDP) — wait strategies, SPA detection, popup dismissal, screenshots |
| `browser_fingerprint.rs` | Desktop/Tablet/Mobile profiles, randomisation, CDP injection script |
| `anti_detect.rs` | 57 rotating UAs, adaptive throttling, realistic headers |
| `robots.rs` | robots.txt parser with `*` wildcard and `$` anchor |
| `sitemap.rs` | Sitemap XML and SitemapIndex parser |
| `proxy.rs` | Proxy pool with round-robin / random / failover strategies |

---

## Installation

### Prerequisites

- **Rust 1.75+** — install via [rustup.rs](https://rustup.rs)
- **Python 3.8+** — for Python bindings only
- **Chrome/Chromium** — optional, for JS rendering (`--features js`)

### Rust library

```bash
# Add to Cargo.toml
[dependencies]
rehyke-core = "0.2"                   # static fetch only
rehyke-core = { version = "0.2", features = ["js"] }  # + headless Chrome
```

### CLI

```bash
cargo install rehyke-cli              # static fetch
cargo install rehyke-cli --features js  # + headless Chrome
```

### Python (via maturin)

```bash
pip install maturin
git clone https://github.com/user/rehyke.git && cd rehyke/crates/rehyke-python
maturin develop --release                    # static fetch only
maturin develop --release --features js      # + headless Chrome
```

### Build from source

```bash
git clone https://github.com/user/rehyke.git
cd rehyke
cargo build --release                        # all crates, static fetch
cargo build --release --features js          # with headless Chrome support
```

---

## Roadmap

### ✅ v0.1.0 — Foundation (Shipped)

- [x] Full crawl engine: HTTP/2, retry, content-type detection
- [x] Universal parser: HTML, XML, RSS, Atom, JSON, SVG, Sitemap
- [x] HTML → Markdown converter with GFM tables and YAML frontmatter
- [x] Priority scheduler, URL dedup, per-domain rate limiting
- [x] robots.txt and sitemap support
- [x] 57 rotating UAs, adaptive throttling, proxy rotation
- [x] Python bindings (PyO3/maturin), CLI (clap)
- [x] 369 tests

### ✅ v0.2.0 — "Chrome Eyes" (Shipped)

- [x] Headless Chromium via `chromiumoxide` — optional `js` feature, graceful static fallback
- [x] Four JS wait strategies: `NetworkIdle`, `Selector`, `Duration`, `Auto`
- [x] SPA framework detection: React, Vue, Angular, Svelte, Next.js, Nuxt, SvelteKit
- [x] Infinite scroll with bottom-detection early stop
- [x] Popup & overlay dismissal: 30+ selectors + JS text fallback
- [x] Browser fingerprint diversity: UA, WebGL, languages, timezone, canvas noise
- [x] Viewport profiles: Desktop / Tablet / Mobile with correct DPR and touch
- [x] Full-page screenshots: PNG / JPEG per page, slug-named
- [x] CLI: `--js`, `--wait-for`, `--scroll`, `--dismiss-popups`, `--screenshot`, `--viewport`, `--detect-spa`, `--randomize-fingerprint`
- [x] Python: 10 new `CrawlConfig` kwargs, `render_method` + `depth` on `CrawlResult`
- [x] 4 new examples: `js_render.rs`, `spa_crawl.rs`, `screenshot_capture.rs`, `python_js_render.py`
- [x] 473 tests (unit + integration + doctests)

### v0.3.0 — Autonomous Agent Mode

- [ ] **Autonomous Crawl Planner** — AI-driven traversal strategy
- [ ] **Regex Rule Engine** — user-defined extraction/routing pipelines
- [ ] **Content Fingerprinting** — near-dedup with simhash
- [ ] **Smart Pagination Detection** — `?page=N`, `/page/N`, infinite scroll
- [ ] **Form Discovery & Auto-Submit**

### v0.4.0 — Regex Superpowers

- [ ] **Named Capture Groups Pipeline** — structured data extraction
- [ ] **Regex-Based Content Scoring** — relevance ranking
- [ ] **URL Pattern Learning** — auto-suggest include/exclude rules
- [ ] **Custom Markdown Templates**

### v0.5.0 — Distributed Crawling

- [ ] Worker pool architecture, Redis-backed frontier
- [ ] S3/GCS streaming output
- [ ] Resume/checkpoint for long crawls
- [ ] Webhook callbacks on crawl events

### v1.0.0 — Production Ready

- [ ] WASM support, plugin system, GUI dashboard
- [ ] Scheduled crawls, OAuth2 authentication
- [ ] GDPR-aware compliance mode

---

## Regex at the Core

Rehyke uses Rust's `regex` crate — DFA-based, O(n) guaranteed — as a fundamental building block:

| Module | Regex usage |
|--------|-------------|
| URL Filtering | Include/exclude URL patterns |
| Link Extraction | `srcset` parsing, meta refresh URL extraction |
| Content Cleaning | Ad container class/id detection, comment section removal |
| robots.txt | Wildcard `*` and `$` anchor path matching |
| Code Detection | Language class extraction (`class="language-*"`) |
| URL Normalization | Percent-encoding, query sorting |
| Anti-Detection | CAPTCHA/block response analysis |
| Output | Filename sanitization |

---

## Performance

| Metric | Target | Status |
|--------|--------|--------|
| Single page — no JS | < 200 ms | ✅ |
| Single page — with JS | < 3 s | ✅ v0.2.0 |
| 100 pages (full, parallel) | < 30 s | ✅ |
| 1 000 pages (full, parallel) | < 5 min | ✅ |
| 10 000 pages (deep, parallel) | < 30 min | ✅ |
| Memory per page | < 5 MB | ✅ |
| Concurrent connections | Up to 200 | ✅ |

---

## Contributing

```bash
# Clone and build
git clone https://github.com/user/rehyke.git && cd rehyke
cargo build

# Run all tests (offline)
cargo test --workspace

# Run tests including live-network tests
REHYKE_LIVE=1 cargo test --package rehyke-core --test integration

# Run JS examples (requires Chrome)
cargo run --example js_render          --features js
cargo run --example spa_crawl          --features js
cargo run --example screenshot_capture --features js

# Run with verbose logging
RUST_LOG=debug cargo run -p rehyke-cli -- https://example.com --mode lite -v

# Build Python wheel
cd crates/rehyke-python
pip install maturin
maturin develop --features js

# Lint and format
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
  <sub>v0.2.0 "Chrome Eyes" &bull; 13k+ lines &bull; 473 tests &bull; 15 modules &bull; 3 crates</sub>
</p>
