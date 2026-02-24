# Rehyke User Guide

**Crawl Everything. Miss Nothing.**

The definitive guide to mastering Rehyke -- an ultra-high-performance, regex-powered
autonomous web crawler built in Rust with Python bindings.

---

## Table of Contents

1. [Quick Start Guide](#1-quick-start-guide)
2. [Configuration Deep Dive](#2-configuration-deep-dive)
3. [Regex Mastery for Crawling](#3-regex-mastery-for-crawling)
4. [Anti-Detection Strategies](#4-anti-detection-strategies)
5. [Content Processing Pipeline](#5-content-processing-pipeline)
6. [Advanced Crawling Patterns](#6-advanced-crawling-patterns)
7. [Python Integration Guide](#7-python-integration-guide)
8. [CLI Power User Guide](#8-cli-power-user-guide)
9. [Performance Tuning](#9-performance-tuning)
10. [Troubleshooting](#10-troubleshooting)

---

## 1. Quick Start Guide

### Installation

Rehyke ships as three packages: a Rust library (`rehyke-core`), a command-line tool
(`rehyke-cli`), and Python bindings (`rehyke-python`). Pick the one that fits your
workflow.

#### Install the CLI via Cargo

If you have a Rust toolchain (1.75 or later), install the CLI directly:

```bash
cargo install rehyke-cli
```

After installation the `rehyke` binary is available system-wide.

#### Install the Python package

The Python package requires `maturin` to build from the Rust source:

```bash
pip install maturin
git clone https://github.com/user/rehyke.git
cd rehyke/crates/rehyke-python
maturin develop --release
```

This compiles the Rust core and installs the `rehyke` module into your active Python
environment.

#### Build from source

Clone, build, and run the full workspace:

```bash
git clone https://github.com/user/rehyke.git
cd rehyke
cargo build --release

# The CLI binary is at target/release/rehyke-cli
./target/release/rehyke-cli https://example.com --mode lite
```

#### Prerequisites

| Requirement | Version | Notes |
|-------------|---------|-------|
| Rust | 1.75+ | Install via [rustup.rs](https://rustup.rs) |
| Python | 3.8+ | Only needed for Python bindings |
| Chrome/Chromium | Latest | Only needed for JavaScript rendering |

### Your First Crawl in 60 Seconds

#### CLI -- the fastest path

Open a terminal and run:

```bash
rehyke https://example.com
```

That is all. Rehyke fetches the page, converts it to Markdown, and prints the result
to stdout. The default mode is `Full`, which follows internal links up to depth 5 and
crawls up to 1,000 pages.

For a single-page fetch with no link following:

```bash
rehyke https://example.com --mode lite
```

To save the output to a file:

```bash
rehyke https://example.com --mode lite -o output.md
```

To crawl an entire site into a mirrored directory tree:

```bash
rehyke https://docs.example.com --mode full --output-dir ./crawled --structure mirror
```

#### Rust -- programmatic access

Add `rehyke-core` to your `Cargo.toml`:

```toml
[dependencies]
rehyke-core = { path = "crates/rehyke-core" }
tokio = { version = "1", features = ["full"] }
```

Then run a crawl:

```rust
use rehyke_core::{Rehyke, ScanMode};

#[tokio::main]
async fn main() {
    // One-liner: crawl a single URL with default Full-mode settings
    let results = Rehyke::crawl("https://example.com", ScanMode::Full)
        .await
        .unwrap();

    for page in &results {
        println!("Title: {}", page.title);
        println!("URL:   {}", page.url);
        println!("---");
        println!("{}", &page.markdown[..200.min(page.markdown.len())]);
    }
}
```

#### Python -- quick scripting

```python
import rehyke

# One-liner
results = rehyke.crawl("https://example.com")

for page in results:
    print(f"Title: {page.title}")
    print(f"URL:   {page.url}")
    print(f"Words: {len(page.markdown.split())}")
    print("---")
```

### Understanding Output Formats

Every crawled page produces a `CrawlResult` containing clean Markdown with YAML
frontmatter. Here is a representative example:

```markdown
---
url: https://example.com/blog/building-crawlers
title: "How to Build a Web Crawler"
description: "A comprehensive guide to building fast web crawlers in Rust"
author: Jane Smith
published: 2024-06-15
language: en
canonical: https://example.com/blog/building-crawlers
---

# How to Build a Web Crawler

Main content converted to clean Markdown with all formatting preserved...

## Getting Started

Paragraph text with [links](https://example.com) and **bold** formatting.

| Feature   | Status    |
|-----------|-----------|
| HTML      | Supported |
| RSS       | Supported |

> Blockquotes are preserved with proper formatting
```

**Output modes** control where this Markdown goes:

| Mode | CLI Flag | Description |
|------|----------|-------------|
| Memory | (default) | Results held in memory, printed to stdout |
| Files (Flat) | `--output-dir ./out --structure flat` | One `.md` file per page in a single directory |
| Files (Mirror) | `--output-dir ./out --structure mirror` | Directory tree mirrors the website structure |
| Single File | `-o output.md` | All pages concatenated into one file with `---` separators |

**Output formats** for the CLI:

| Format | CLI Flag | Description |
|--------|----------|-------------|
| Markdown | `--format markdown` (default) | Clean Markdown with YAML frontmatter |
| JSON | `--format json` | Structured JSON with all metadata, links, and content |

---

## 2. Configuration Deep Dive

### The CrawlConfig Builder Pattern

Rehyke uses a builder pattern for configuration. Start with sensible defaults and
override only what you need:

```rust
use rehyke_core::{CrawlConfigBuilder, ScanMode, DelayStrategy, OutputMode, FileStructure};
use std::path::PathBuf;
use std::time::Duration;

let config = CrawlConfigBuilder::new()
    // Scope
    .mode(ScanMode::Full)           // Preset: Lite, Full, or Deep
    .max_depth(10)                   // Override mode's default depth
    .max_pages(5_000)                // Override mode's default page limit
    .concurrency(20)                 // Override mode's default concurrency

    // Output
    .output(OutputMode::Files {
        output_dir: PathBuf::from("./crawled"),
        structure: FileStructure::Mirror,
    })

    // Politeness
    .delay_strategy(DelayStrategy::Random {
        min: Duration::from_millis(500),
        max: Duration::from_secs(2),
    })
    .respect_robots_txt(true)

    // URL filtering
    .exclude_patterns(vec![
        r"\.pdf$".into(),
        r"\.zip$".into(),
        r"/login".into(),
        r"/admin".into(),
    ])
    .include_patterns(vec![r"/blog/".into()])

    // Content cleaning
    .clean_navigation(true)
    .clean_footers(true)
    .clean_ads(true)
    .extract_metadata(true)

    // Network
    .timeout(Duration::from_secs(60))
    .user_agent("MyBot/1.0 (https://mysite.com/bot)")
    .header("Accept-Language", "en-US,en;q=0.9")
    .cookie("session", "abc123")

    // URL normalization
    .remove_www(true)

    // JavaScript rendering
    .enable_js(false)

    .build();
```

**Important:** Calling `.mode()` resets `max_depth`, `max_pages`, and `concurrency`
to the mode's defaults. Always call `.mode()` first, then override individual values
afterward.

### Complete Configuration Reference

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `mode` | `ScanMode` | `Full` | High-level scan preset |
| `max_depth` | `usize` | 5 (mode-dependent) | Maximum link-follow depth from seed URL |
| `max_pages` | `usize` | 1,000 (mode-dependent) | Maximum pages to crawl |
| `concurrency` | `usize` | 10 (mode-dependent) | Simultaneous HTTP connections |
| `enable_js` | `bool` | `false` | Enable headless browser rendering |
| `js_wait_strategy` | `WaitStrategy` | `Auto` | When to consider JS page "ready" |
| `output` | `OutputMode` | `Memory` | Where to write results |
| `user_agent` | `String` | `"rehyke/{version}"` | User-Agent header |
| `proxies` | `Vec<ProxyConfig>` | `[]` | Proxy endpoints |
| `proxy_strategy` | `ProxyStrategy` | `RoundRobin` | Proxy selection strategy |
| `delay_strategy` | `DelayStrategy` | `None` | Inter-request delay |
| `exclude_patterns` | `Vec<String>` | `[]` | Regex exclusion patterns |
| `include_patterns` | `Vec<String>` | `[]` | Regex inclusion patterns |
| `respect_robots_txt` | `bool` | `true` | Honor robots.txt |
| `extract_metadata` | `bool` | `true` | Extract title, description, etc. |
| `clean_navigation` | `bool` | `true` | Remove `<nav>` elements |
| `clean_footers` | `bool` | `true` | Remove `<footer>` elements |
| `clean_ads` | `bool` | `true` | Remove ad containers |
| `timeout` | `Duration` | 30s | Per-request timeout |
| `retry_config` | `RetryConfig` | 3 retries, 500ms initial, 30s max | Retry behavior |
| `custom_headers` | `HashMap` | `{}` | Extra HTTP headers |
| `cookies` | `HashMap` | `{}` | Cookies per request |
| `remove_www` | `bool` | `true` | Strip www. in URL dedup |

### ScanMode Selection Guide

Rehyke provides three presets that control how aggressively the crawler explores:

#### Lite Mode -- Quick Single-Page Extraction

```rust
let config = CrawlConfigBuilder::new().mode(ScanMode::Lite).build();
```

| Property | Value |
|----------|-------|
| Max Depth | 2 |
| Max Pages | 100 |
| Concurrency | 5 |
| Link Following | None -- single page only |
| Typical Speed | < 200ms (static), < 3s (JS) |

**When to use Lite:**
- You need content from one specific page
- Quick content extraction for LLM context
- Testing your configuration before a larger crawl
- API documentation pages where each URL is self-contained

#### Full Mode -- Domain-Scoped Crawling (Default)

```rust
let config = CrawlConfigBuilder::new().mode(ScanMode::Full).build();
```

| Property | Value |
|----------|-------|
| Max Depth | 5 |
| Max Pages | 1,000 |
| Concurrency | 10 |
| Link Following | Same-domain (internal) only |
| Typical Speed | < 5 min for 1,000 pages |

**When to use Full:**
- Crawling documentation sites
- Building datasets from a single domain
- Archiving a blog or news site
- General-purpose site scraping

#### Deep Mode -- Exhaustive Cross-Domain Crawling

```rust
let config = CrawlConfigBuilder::new().mode(ScanMode::Deep).build();
```

| Property | Value |
|----------|-------|
| Max Depth | 50 |
| Max Pages | 50,000 |
| Concurrency | 25 |
| Link Following | Internal AND external |
| Typical Speed | < 30 min for 10,000 pages |

**When to use Deep:**
- Research and competitive analysis
- Building large training datasets
- Link graph analysis across domains
- Discovering all resources linked from a site

### Timeout and Retry Strategies

#### Per-Request Timeout

The `timeout` field controls how long to wait for each individual HTTP request:

```rust
// Short timeout for fast sites
let config = CrawlConfigBuilder::new()
    .timeout(Duration::from_secs(10))
    .build();

// Long timeout for slow APIs or large pages
let config = CrawlConfigBuilder::new()
    .timeout(Duration::from_secs(120))
    .build();
```

#### Retry Configuration

Failed requests are automatically retried with exponential backoff:

```rust
use rehyke_core::RetryConfig;

let config = CrawlConfigBuilder::new()
    .retry_config(RetryConfig {
        max_retries: 5,                          // Try up to 5 times
        initial_delay: Duration::from_secs(1),   // Wait 1s before first retry
        max_delay: Duration::from_secs(60),      // Never wait more than 60s
    })
    .build();
```

**Backoff formula:** `delay = min(initial_delay * 2^attempt, max_delay)`

Example with defaults (3 retries, 500ms initial, 30s max):

| Attempt | Delay |
|---------|-------|
| 1st retry | 500ms |
| 2nd retry | 1,000ms |
| 3rd retry | 2,000ms |

**Retryable errors:** Network timeouts, DNS failures, HTTP 429 (Too Many Requests),
HTTP 500/502/503/504 (server errors).

**Non-retryable errors:** HTTP 403 (Forbidden), HTTP 404 (Not Found), TLS errors,
parse errors, malformed URLs.

**Retry-After header:** When the server sends a `Retry-After` header with an integer
value on a 429 response, Rehyke uses that delay instead of its computed backoff
(still capped at `max_delay`).

### Output Modes and File Structures

#### Memory Mode (Default)

Results are collected in a `Vec<CrawlResult>` and returned when the crawl completes.
Best for programmatic use when you want to process results in code.

```rust
let config = CrawlConfigBuilder::new()
    .output(OutputMode::Memory)
    .build();

let crawler = Rehyke::new(config);
let results = crawler.run("https://example.com").await?;
// results: Vec<CrawlResult>
```

#### Files Mode -- Flat Structure

Each page is written to a separate `.md` file with a sanitized filename:

```rust
let config = CrawlConfigBuilder::new()
    .output(OutputMode::Files {
        output_dir: PathBuf::from("./output"),
        structure: FileStructure::Flat,
    })
    .build();
```

Result on disk:

```
output/
  example-com.md
  example-com-blog-post-1.md
  example-com-blog-post-2.md
  example-com-about.md
```

#### Files Mode -- Mirror Structure

The directory tree mirrors the website's URL hierarchy:

```rust
let config = CrawlConfigBuilder::new()
    .output(OutputMode::Files {
        output_dir: PathBuf::from("./output"),
        structure: FileStructure::Mirror,
    })
    .build();
```

Result on disk:

```
output/
  example.com/
    index.md
    blog/
      post-1/
        index.md
      post-2/
        index.md
    about/
      index.md
```

#### Single File Mode

All pages are concatenated into a single file with `---` separators. Useful for
building a single document for LLM context:

```rust
let config = CrawlConfigBuilder::new()
    .output(OutputMode::SingleFile {
        output_path: PathBuf::from("./site.md"),
    })
    .build();
```

### JSON Configuration

`CrawlConfig` implements `Serialize` and `Deserialize`, so you can load
configuration from JSON files:

```json
{
  "mode": "full",
  "max_depth": 10,
  "max_pages": 5000,
  "concurrency": 20,
  "enable_js": false,
  "output": {
    "type": "files",
    "output_dir": "./crawled",
    "structure": "mirror"
  },
  "user_agent": "MyBot/1.0",
  "delay_strategy": {
    "type": "random",
    "min": 500,
    "max": 2000
  },
  "exclude_patterns": ["\\.pdf$", "/login"],
  "include_patterns": ["/blog/"],
  "respect_robots_txt": true,
  "timeout": 30000,
  "retry_config": {
    "max_retries": 3,
    "initial_delay": 500,
    "max_delay": 30000
  }
}
```

Duration fields accept both integer milliseconds and human-readable strings like
`"30s"`, `"500ms"`, `"1m30s"`, or `"2h"`.

Loading a JSON config in Rust:

```rust
use std::fs;
use rehyke_core::CrawlConfig;

let json = fs::read_to_string("config.json")?;
let config: CrawlConfig = serde_json::from_str(&json)?;
let crawler = Rehyke::new(config);
```

---

## 3. Regex Mastery for Crawling

Rehyke uses Rust's `regex` crate -- one of the fastest regex engines in the world,
guaranteeing O(n) execution with no catastrophic backtracking. Regex patterns are
used throughout the system for URL filtering, content cleaning, link extraction, and
more.

### URL Filtering with Regex Patterns

#### Exclude Patterns

Skip URLs that match any of the provided regex patterns:

```rust
let config = CrawlConfigBuilder::new()
    .exclude_patterns(vec![
        r"\.pdf$".into(),           // Skip PDF files
        r"\.zip$".into(),           // Skip ZIP files
        r"\.exe$".into(),           // Skip executables
        r"/login".into(),           // Skip login pages
        r"/admin".into(),           // Skip admin panels
        r"/wp-admin".into(),        // Skip WordPress admin
        r"\?.*utm_".into(),         // Skip UTM-tagged URLs
        r"/tag/".into(),            // Skip tag pages
        r"/category/".into(),       // Skip category archives
        r"/page/\d+".into(),        // Skip pagination pages
    ])
    .build();
```

CLI equivalent:

```bash
rehyke https://example.com \
    --exclude '\.pdf$' \
    --exclude '\.zip$' \
    --exclude '/login' \
    --exclude '/admin'
```

#### Include Patterns

Only crawl URLs that match at least one of the provided regex patterns. When
`include_patterns` is non-empty, every discovered URL must match at least one
pattern to be enqueued:

```rust
let config = CrawlConfigBuilder::new()
    .include_patterns(vec![
        r"/blog/".into(),           // Only crawl blog posts
        r"/docs/".into(),           // And documentation pages
    ])
    .build();
```

CLI equivalent:

```bash
rehyke https://example.com \
    --include '/blog/' \
    --include '/docs/'
```

#### Combining Include and Exclude

You can use both together. Exclude patterns are checked first:

```rust
let config = CrawlConfigBuilder::new()
    .include_patterns(vec![r"/products/".into()])   // Only product pages
    .exclude_patterns(vec![r"/products/archive".into()])  // But not archived ones
    .build();
```

### Content Extraction Using Regex

Rehyke uses regex patterns internally for content cleaning. The parser module uses
14 regex patterns for ad detection and 5 patterns for comment section identification.
Here are the kinds of patterns used internally:

**Ad container detection patterns (class/id matching):**

```
ad, ads, advert, advertisement, banner, sponsor, promoted,
sidebar-ad, ad-container, ad-wrapper, dfp, google-ad, adsense
```

**Comment section patterns:**

```
comments, comment-section, disqus, discuss, responses
```

These patterns are applied during the HTML cleaning stage, before the DOM is walked
for Markdown conversion.

### Pattern Matching for Structured Data

While Rehyke's primary regex usage is URL filtering and content cleaning, you can
apply regex patterns to the extracted Markdown output for structured data extraction.
Here is a practical pipeline:

```rust
use regex::Regex;

// After crawling, extract structured data from results
let results = Rehyke::crawl("https://shop.example.com", ScanMode::Full).await?;

let email_re = Regex::new(r"[\w.+-]+@[\w-]+\.[\w.]+").unwrap();
let phone_re = Regex::new(r"\+?1?\s*\(?\d{3}\)?[\s.-]*\d{3}[\s.-]*\d{4}").unwrap();
let price_re = Regex::new(r"\$\d{1,3}(?:,\d{3})*(?:\.\d{2})?").unwrap();

for page in &results {
    let emails: Vec<&str> = email_re
        .find_iter(&page.markdown)
        .map(|m| m.as_str())
        .collect();

    let phones: Vec<&str> = phone_re
        .find_iter(&page.markdown)
        .map(|m| m.as_str())
        .collect();

    let prices: Vec<&str> = price_re
        .find_iter(&page.markdown)
        .map(|m| m.as_str())
        .collect();

    if !emails.is_empty() || !phones.is_empty() || !prices.is_empty() {
        println!("Page: {}", page.url);
        println!("  Emails: {:?}", emails);
        println!("  Phones: {:?}", phones);
        println!("  Prices: {:?}", prices);
    }
}
```

### Building Custom Extraction Pipelines

You can chain Rehyke's crawl output with regex processing to build powerful
extraction workflows:

```rust
use regex::Regex;
use std::collections::HashMap;

struct ExtractionPipeline {
    rules: Vec<(String, Regex)>,
}

impl ExtractionPipeline {
    fn new() -> Self {
        Self { rules: Vec::new() }
    }

    fn add_rule(mut self, name: &str, pattern: &str) -> Self {
        self.rules.push((
            name.to_string(),
            Regex::new(pattern).expect("invalid regex"),
        ));
        self
    }

    fn extract(&self, text: &str) -> HashMap<String, Vec<String>> {
        let mut results = HashMap::new();
        for (name, re) in &self.rules {
            let matches: Vec<String> = re
                .find_iter(text)
                .map(|m| m.as_str().to_string())
                .collect();
            if !matches.is_empty() {
                results.insert(name.clone(), matches);
            }
        }
        results
    }
}

// Usage
let pipeline = ExtractionPipeline::new()
    .add_rule("emails", r"[\w.+-]+@[\w-]+\.[\w.]+")
    .add_rule("phones", r"\+?1?\s*\(?\d{3}\)?[\s.-]*\d{3}[\s.-]*\d{4}")
    .add_rule("prices", r"\$\d{1,3}(?:,\d{3})*(?:\.\d{2})?")
    .add_rule("dates_iso", r"\d{4}-\d{2}-\d{2}")
    .add_rule("dates_us", r"\d{2}/\d{2}/\d{4}")
    .add_rule("urls", r"https?://[^\s\)]+");

let results = Rehyke::crawl("https://example.com", ScanMode::Full).await?;
for page in &results {
    let extracted = pipeline.extract(&page.markdown);
    println!("{}: {:?}", page.url, extracted);
}
```

The same pipeline in Python:

```python
import re
from rehyke import Rehyke, CrawlConfig, ScanMode

class ExtractionPipeline:
    def __init__(self):
        self.rules = {}

    def add_rule(self, name, pattern):
        self.rules[name] = re.compile(pattern)
        return self

    def extract(self, text):
        results = {}
        for name, regex in self.rules.items():
            matches = regex.findall(text)
            if matches:
                results[name] = matches
        return results

pipeline = (ExtractionPipeline()
    .add_rule("emails", r"[\w.+-]+@[\w-]+\.[\w.]+")
    .add_rule("phones", r"\+?1?\s*\(?\d{3}\)?[\s.-]*\d{3}[\s.-]*\d{4}")
    .add_rule("prices", r"\$\d{1,3}(?:,\d{3})*(?:\.\d{2})?")
    .add_rule("dates", r"\d{4}-\d{2}-\d{2}"))

config = CrawlConfig(mode=ScanMode.FULL)
results = Rehyke(config).crawl("https://example.com")

for page in results:
    extracted = pipeline.extract(page.markdown)
    if extracted:
        print(f"{page.url}: {extracted}")
```

### 10+ Practical Regex Examples for Common Crawling Tasks

Here are ready-to-use regex patterns for common data extraction scenarios:

#### 1. Email Addresses

```
[\w.+-]+@[\w-]+\.[\w.]+
```

Matches: `user@example.com`, `john.doe+tag@company.co.uk`

#### 2. Phone Numbers (North American)

```
\+?1?\s*\(?\d{3}\)?[\s.-]*\d{3}[\s.-]*\d{4}
```

Matches: `(555) 123-4567`, `+1 555.123.4567`, `555-123-4567`

#### 3. Prices (USD)

```
\$\d{1,3}(?:,\d{3})*(?:\.\d{2})?
```

Matches: `$29.99`, `$1,499.00`, `$5`

#### 4. Dates (ISO 8601)

```
\d{4}-\d{2}-\d{2}(?:T\d{2}:\d{2}:\d{2})?
```

Matches: `2024-06-15`, `2024-06-15T14:30:00`

#### 5. Dates (US Format)

```
\d{1,2}/\d{1,2}/\d{2,4}
```

Matches: `6/15/2024`, `06/15/24`

#### 6. Social Media Profile Links

```
(?:https?://)?(?:www\.)?twitter\.com/[\w]+
(?:https?://)?(?:www\.)?x\.com/[\w]+
(?:https?://)?(?:www\.)?github\.com/[\w-]+
(?:https?://)?(?:www\.)?linkedin\.com/in/[\w-]+
(?:https?://)?(?:www\.)?instagram\.com/[\w.]+
(?:https?://)?(?:www\.)?facebook\.com/[\w.]+
```

Combined pattern for any social link:

```
(?:https?://)?(?:www\.)?(?:twitter|x|github|linkedin|instagram|facebook)\.com/[\w.-]+
```

#### 7. API Endpoints

```
(?:https?://)?[\w.-]+/api/v\d+/[\w/.-]+
```

Matches: `https://api.example.com/api/v2/users/123`

REST-style API pattern:

```
/api/(?:v\d+/)?[\w]+(?:/[\w]+)*(?:\?[\w=&]+)?
```

#### 8. IP Addresses (IPv4)

```
\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b
```

Matches: `192.168.1.1`, `10.0.0.255`

#### 9. Hex Color Codes

```
#[0-9a-fA-F]{3,8}\b
```

Matches: `#fff`, `#FF5733`, `#00AABBCC`

#### 10. Semantic Versioning

```
v?\d+\.\d+\.\d+(?:-[\w.]+)?
```

Matches: `v1.2.3`, `2.0.0-beta.1`, `0.22.0`

#### 11. URLs in Text

```
https?://[^\s\)\]>]+
```

Matches any URL starting with `http://` or `https://`, stopping at whitespace
or closing brackets.

#### 12. ISBN Numbers

```
(?:ISBN[-: ]?)?(?:97[89][-\s]?)?\d{1,5}[-\s]?\d{1,7}[-\s]?\d{1,7}[-\s]?\d
```

#### 13. Credit Card Numbers (for detection/redaction)

```
\b(?:\d{4}[-\s]?){3}\d{4}\b
```

Use this to detect and redact sensitive data in crawled content before storing.

#### 14. Markdown Headers (for TOC extraction)

```
^#{1,6}\s+(.+)$
```

Apply to the Markdown output to build a table of contents from crawled content.

### URL Filtering Recipes

Common URL filter patterns for the `exclude_patterns` field:

```rust
let exclude = vec![
    // Binary files
    r"\.(pdf|zip|tar|gz|rar|exe|dmg|iso|mp3|mp4|avi|mov)$".into(),

    // Image files (already captured as resources)
    r"\.(jpg|jpeg|png|gif|svg|webp|ico|bmp|tiff)$".into(),

    // Authentication and admin paths
    r"/(login|logout|signin|signup|register|admin|wp-admin)".into(),

    // Tracking and analytics
    r"\?.*utm_".into(),
    r"/(analytics|tracking|pixel)".into(),

    // Social sharing widgets
    r"/(share|tweet|pin)\?".into(),

    // Print and feed versions
    r"/(print|feed|rss|atom)(/|$)".into(),

    // Calendar and date-only archives
    r"/\d{4}/\d{2}/\d{2}/$".into(),

    // Language/locale variants you do not need
    r"/(fr|de|es|ja|zh)/".into(),

    // Common CMS noise
    r"/(wp-content|wp-includes|wp-json)".into(),
    r"/xmlrpc\.php".into(),

    // Query strings with session IDs
    r"\?(.*&)?session_?id=".into(),
    r"\?(.*&)?jsessionid=".into(),

    // Sorting and filtering parameters
    r"\?(.*&)?sort=".into(),
    r"\?(.*&)?order=".into(),
];
```

---

## 4. Anti-Detection Strategies

Rehyke includes a comprehensive anti-detection suite designed to make crawl traffic
blend in with normal browser activity. This section covers all available strategies.

### User Agent Rotation

Rehyke ships with a pool of 57 realistic, modern user agent strings covering:

| Browser | Versions | Platforms | Count |
|---------|----------|-----------|-------|
| Chrome | 120 - 128 | Windows, macOS, Linux | 21 |
| Firefox | 120 - 127 | Windows, macOS, Linux | 13 |
| Safari | 17.0 - 17.5 | macOS 13.x, 14.x | 8 |
| Edge | 120 - 125 | Windows, macOS, Linux | 9 |

A different user agent is randomly selected for each request, making the crawl look
like traffic from many different users.

**Custom user agent:** If you need a specific identity (for example, to identify
your bot to website operators):

```rust
let config = CrawlConfigBuilder::new()
    .user_agent("MyResearchBot/1.0 (+https://mysite.com/bot)")
    .build();
```

```bash
rehyke https://example.com --user-agent "MyResearchBot/1.0"
```

**When to use a custom user agent:**
- You have permission to crawl and want to identify yourself
- The site whitelists specific bots
- You want to be transparent about your crawling

**When to use the default rotation:**
- General-purpose crawling where you want to blend in
- Sites that block known bot user agents
- When you need to look like normal browser traffic

### Request Delay Strategies

Inter-request delays prevent the crawler from overwhelming target servers and reduce
the chance of being rate-limited or blocked.

#### Fixed Delay

Constant delay between every request to the same domain:

```rust
let config = CrawlConfigBuilder::new()
    .delay_strategy(DelayStrategy::Fixed {
        delay: Duration::from_millis(500),
    })
    .build();
```

```bash
rehyke https://example.com --delay 500
```

**When to use:** Predictable servers with known rate limits. Good for APIs with
documented request-per-second limits.

#### Random Delay

Random delay uniformly distributed between a minimum and maximum:

```rust
let config = CrawlConfigBuilder::new()
    .delay_strategy(DelayStrategy::Random {
        min: Duration::from_millis(500),
        max: Duration::from_secs(2),
    })
    .build();
```

```bash
rehyke https://example.com --delay 500-2000
```

**When to use:** General-purpose crawling. The randomness makes traffic look more
human. This is the recommended strategy for most crawls.

#### Adaptive Delay

Starts at an initial delay and increases when rate-limiting is detected:

```rust
let config = CrawlConfigBuilder::new()
    .delay_strategy(DelayStrategy::Adaptive {
        initial: Duration::from_millis(200),
    })
    .build();
```

**When to use:** When you want to crawl as fast as the server allows. Starts
aggressive and backs off automatically when it gets 429 or 503 responses.

#### No Delay

Zero inter-request delay. Fastest possible crawling:

```rust
let config = CrawlConfigBuilder::new()
    .delay_strategy(DelayStrategy::None)
    .build();
```

**When to use:** Only for sites you control, internal documentation, or when you
have explicit permission. Using no delay on external sites will likely get you
blocked quickly.

#### Strategy Comparison

| Strategy | Speed | Stealth | Predictability | Best For |
|----------|-------|---------|----------------|----------|
| `None` | Fastest | Lowest | -- | Owned sites, testing |
| `Fixed` | Moderate | Low | High | APIs with rate limits |
| `Random` | Moderate | High | Low | General crawling |
| `Adaptive` | Variable | Highest | Low | Unknown rate limits |

### Proxy Configuration and Rotation

Rehyke supports HTTP, HTTPS, and SOCKS5 proxies with four rotation strategies.

#### Single Proxy

```rust
use rehyke_core::{ProxyConfig, ProxyType};

let config = CrawlConfigBuilder::new()
    .proxies(vec![ProxyConfig {
        url: "http://proxy.example.com:8080".into(),
        proxy_type: ProxyType::Http,
        auth: None,
        region: None,
    }])
    .build();
```

```bash
rehyke https://example.com --proxy http://proxy.example.com:8080
```

#### Authenticated Proxy

```rust
use rehyke_core::{ProxyConfig, ProxyType, ProxyAuth};

let config = CrawlConfigBuilder::new()
    .proxies(vec![ProxyConfig {
        url: "http://proxy.example.com:8080".into(),
        proxy_type: ProxyType::Http,
        auth: Some(ProxyAuth {
            username: "user".into(),
            password: "pass".into(),
        }),
        region: None,
    }])
    .build();
```

#### SOCKS5 Proxy

```rust
let config = CrawlConfigBuilder::new()
    .proxies(vec![ProxyConfig {
        url: "socks5://proxy.example.com:1080".into(),
        proxy_type: ProxyType::Socks5,
        auth: None,
        region: None,
    }])
    .build();
```

```bash
rehyke https://example.com --proxy socks5://proxy.example.com:1080
```

#### Multiple Proxies with Rotation

```rust
use rehyke_core::ProxyStrategy;

let proxies = vec![
    ProxyConfig {
        url: "http://proxy1.example.com:8080".into(),
        proxy_type: ProxyType::Http,
        auth: None,
        region: Some("US".into()),
    },
    ProxyConfig {
        url: "http://proxy2.example.com:8080".into(),
        proxy_type: ProxyType::Http,
        auth: None,
        region: Some("EU".into()),
    },
    ProxyConfig {
        url: "socks5://proxy3.example.com:1080".into(),
        proxy_type: ProxyType::Socks5,
        auth: None,
        region: Some("APAC".into()),
    },
];

let config = CrawlConfigBuilder::new()
    .proxies(proxies)
    .proxy_strategy(ProxyStrategy::RoundRobin)
    .build();
```

```bash
rehyke https://example.com \
    --proxy http://proxy1.example.com:8080 \
    --proxy http://proxy2.example.com:8080 \
    --proxy socks5://proxy3.example.com:1080
```

#### Proxy Rotation Strategies

| Strategy | Behavior | Best For |
|----------|----------|----------|
| `RoundRobin` | Cycle through proxies in order | Even distribution, predictable |
| `Random` | Random proxy per request | Avoiding patterns |
| `LeastUsed` | Proxy with fewest active requests | Load balancing |
| `FailoverOnly` | Stay on current proxy until it fails | Reliability, minimizing switches |

### Header Randomization

Rehyke sends a complete set of realistic browser headers with every request:

```
User-Agent:                  (randomly rotated from pool of 57)
Accept:                      text/html,application/xhtml+xml,...
Accept-Language:             en-US,en;q=0.9
Accept-Encoding:             gzip, deflate, br, zstd
Connection:                  keep-alive
Upgrade-Insecure-Requests:   1
Sec-Fetch-Dest:              document
Sec-Fetch-Mode:              navigate
Sec-Fetch-Site:              none
Sec-Fetch-User:              ?1
DNT:                         1
Sec-Ch-Ua-Platform:          "Windows"
```

A `Cache-Control: max-age=0` header is randomly included with 30% probability to add
per-request variance and mimic real browsing patterns.

You can add custom headers on top of the defaults:

```rust
let config = CrawlConfigBuilder::new()
    .header("X-Custom-Header", "value")
    .header("Authorization", "Bearer token123")
    .header("Referer", "https://google.com")
    .build();
```

### Rate Limiting and Politeness

Rehyke implements per-domain rate limiting to prevent overwhelming any single server:

- **Per-domain cooldown:** Each domain's last request timestamp is tracked in a
  `DashMap<String, Instant>`. Requests to the same domain are delayed until the
  cooldown has elapsed.
- **robots.txt compliance:** Enabled by default. Set `respect_robots_txt(false)` to
  disable.
- **Automatic backoff on 429:** When a server returns HTTP 429 (Too Many Requests),
  Rehyke uses exponential backoff and respects the `Retry-After` header.
- **Deferred task scheduling:** When a domain is in cooldown, its task is deferred
  and the next eligible task from a different domain is tried instead.

**Recommended settings for polite crawling:**

```rust
let config = CrawlConfigBuilder::new()
    .delay_strategy(DelayStrategy::Random {
        min: Duration::from_secs(1),
        max: Duration::from_secs(3),
    })
    .respect_robots_txt(true)
    .concurrency(5)
    .build();
```

```bash
rehyke https://example.com \
    --delay 1000-3000 \
    --concurrency 5
```

---

## 5. Content Processing Pipeline

Rehyke's content processing pipeline transforms raw HTTP responses into clean,
structured Markdown. This section explains each stage.

### HTML to Markdown Conversion

The converter maps every HTML element to its Markdown equivalent:

| HTML Element | Markdown Output |
|-------------|-----------------|
| `<h1>` - `<h6>` | `#` to `######` |
| `<p>` | Paragraph with blank lines |
| `<a href="url">text</a>` | `[text](url)` |
| `<img src="url" alt="text">` | `![text](url)` |
| `<strong>` / `<b>` | `**text**` |
| `<em>` / `<i>` | `*text*` |
| `<code>` | `` `code` `` |
| `<pre><code>` | Fenced code block with language |
| `<ul>` / `<li>` | `- item` |
| `<ol>` / `<li>` | `1. item` |
| `<blockquote>` | `> text` |
| `<table>` | GFM pipe table |
| `<hr>` | `---` |
| `<del>` / `<s>` | `~~text~~` |
| `<dl>` / `<dt>` / `<dd>` | `**term:** definition` |
| `<figure>` | `![caption](src)` |
| `<video>` / `<audio>` | `[Media: title](src)` |
| `<iframe>` | `[Embedded: title](src)` |

**Code block language detection:** The parser uses regex to extract language
identifiers from `class="language-*"` attributes on `<code>` elements, producing
properly tagged fenced code blocks:

```markdown
```python
def hello():
    print("world")
```
```

**GFM table formatting:** Tables are rendered with aligned pipe-delimited columns.
Column widths are computed from the maximum cell width in each column (minimum 3
characters for the separator dashes).

### Content Cleaning and Normalization

Before conversion, the parser cleans the HTML document in two phases:

**Phase 1: Element removal (always applied)**
- `<script>` elements
- `<style>` elements
- `<noscript>` elements

**Phase 2: Configurable removal (all enabled by default)**
- `<nav>` elements (`clean_navigation`)
- `<footer>` elements (`clean_footers`)
- Ad containers matching class/id patterns (`clean_ads`)
- Comment sections (`clean_comments` in ParseConfig)

**Phase 3: Post-processing pipeline**

1. **HTML tag stripping** -- Removes any residual `<tags>` that survived parsing
2. **Blank line collapsing** -- Collapses runs of blank lines to at most 2
3. **Trailing whitespace trimming** -- Strips trailing spaces from every line
4. **Final newline** -- Ensures the file ends with exactly one newline

To disable cleaning for raw extraction:

```rust
let config = CrawlConfigBuilder::new()
    .clean_navigation(false)
    .clean_footers(false)
    .clean_ads(false)
    .build();
```

### Metadata Extraction

When `extract_metadata` is enabled (default), Rehyke extracts structured metadata
from HTML `<meta>` tags and other elements:

| Field | Source |
|-------|--------|
| `title` | `<title>` tag |
| `description` | `<meta name="description">` or `og:description` |
| `author` | `<meta name="author">` |
| `published_date` | `article:published_time` |
| `language` | `<html lang="...">` |
| `canonical_url` | `<link rel="canonical">` |
| `og_image` | `og:image` meta tag |
| `keywords` | `<meta name="keywords">` |

This metadata appears in the YAML frontmatter of the Markdown output and is also
available programmatically via `CrawlResult.metadata`:

```rust
for page in &results {
    if let Some(author) = &page.metadata.author {
        println!("Author: {}", author);
    }
    if let Some(date) = &page.metadata.published_date {
        println!("Published: {}", date);
    }
    println!("Keywords: {:?}", page.metadata.keywords);
}
```

Python equivalent:

```python
for page in results:
    if page.metadata.author:
        print(f"Author: {page.metadata.author}")
    if page.metadata.published_date:
        print(f"Published: {page.metadata.published_date}")
    print(f"Keywords: {page.metadata.keywords}")
```

**YAML frontmatter escaping:** Values containing special YAML characters (`:`, `#`,
`'`, `"`, `\n`, `{`, `[`, `*`, `&`, `!`, `%`, `@`) are automatically double-quoted
and escaped.

### Universal Content Type Support

Rehyke detects and parses eight content types automatically:

| Input Format | Detection Method | Parser | Output |
|-------------|-----------------|--------|--------|
| HTML / XHTML | Content-Type header, body sniffing | scraper + DOM walker | Clean Markdown |
| RSS 2.0 | `application/rss+xml`, `<rss>` root | quick-xml + CDATA | Feed Markdown |
| Atom | `application/atom+xml`, `<feed>` root | quick-xml + namespaces | Feed Markdown |
| XML (generic) | `text/xml`, `application/xml` | quick-xml event-driven | Structured Markdown |
| JSON / JSON-LD | `application/json`, `application/ld+json` | serde_json | Code block Markdown |
| SVG | `image/svg+xml` | quick-xml | Description + metadata |
| Sitemap XML | `<urlset>` or `<sitemapindex>` root | quick-xml | URL list |
| Plain Text | `text/plain` | passthrough | Wrapped Markdown |

**Content type detection pipeline (three-stage cascade):**

1. **HTTP `Content-Type` header** -- Highest priority
2. **URL file extension** -- `.html`, `.xml`, `.rss`, `.json`, etc.
3. **Body sniffing** -- Inspects first characters (`<!DOCTYPE html`, `<rss>`, etc.)

### Feed Parsing (RSS/Atom)

Rehyke automatically detects and parses RSS 2.0 and Atom feeds:

```bash
rehyke https://blog.example.com/feed.xml --mode lite
```

The parser extracts:
- Feed title and description
- Individual item titles, links, descriptions
- Author and publication dates
- Content (including CDATA sections)

Feed items are converted to Markdown with each item as a section:

```markdown
---
url: https://blog.example.com/feed.xml
title: Example Blog RSS Feed
---

# Example Blog RSS Feed

## Post Title One

**Published:** 2024-06-15
**Author:** Jane Smith

Post description text...

[Read more](https://blog.example.com/post-1)

---

## Post Title Two

**Published:** 2024-06-10

Another post description...

[Read more](https://blog.example.com/post-2)
```

### Sitemap Discovery and Parsing

Rehyke discovers sitemaps through multiple channels:

1. **robots.txt:** Parses `Sitemap:` directives
2. **HTML link tags:** Detects `<link>` elements pointing to sitemaps
3. **Common paths:** Probes `/sitemap.xml`, `/sitemap_index.xml`,
   `/sitemap/sitemap.xml`, and `/sitemaps/sitemap.xml`
4. **Sitemap index files:** Follows `<sitemapindex>` references recursively

Discovered sitemap URLs are enqueued with `Priority::Critical`, ensuring they are
processed early in the crawl.

**Sitemap XML format support:**

- Standard sitemap: `<urlset>` with `<url><loc>` entries
- Sitemap index: `<sitemapindex>` with `<sitemap><loc>` references
- Entries include `<lastmod>`, `<changefreq>`, and `<priority>` metadata

---

## 6. Advanced Crawling Patterns

### Domain-Scoped vs Cross-Domain Crawling

**Domain-scoped (Full mode):** Only follows links on the same domain. Subdomains
are treated as external unless the seed URL is on a subdomain.

```rust
// Only crawls pages on docs.example.com
let config = CrawlConfigBuilder::new()
    .mode(ScanMode::Full)
    .build();
let results = Rehyke::new(config).run("https://docs.example.com").await?;
```

**Cross-domain (Deep mode):** Follows all links, including those to external domains.
Use `max_depth` and `max_pages` to keep the scope manageable.

```rust
// Follows links to any domain, up to depth 10 and 5,000 pages
let config = CrawlConfigBuilder::new()
    .mode(ScanMode::Deep)
    .max_depth(10)
    .max_pages(5_000)
    .build();
```

**Hybrid approach:** Use Deep mode with include patterns to crawl specific external
domains while filtering out the rest:

```rust
let config = CrawlConfigBuilder::new()
    .mode(ScanMode::Deep)
    .include_patterns(vec![
        r"docs\.example\.com".into(),
        r"api\.example\.com".into(),
        r"blog\.example\.com".into(),
    ])
    .build();
```

### Depth-Limited Crawling

Control how far from the seed URL the crawler ventures:

```rust
// Only crawl the seed page and pages directly linked from it
let config = CrawlConfigBuilder::new()
    .mode(ScanMode::Full)
    .max_depth(1)
    .build();

// Crawl 3 levels deep -- enough for most documentation sites
let config = CrawlConfigBuilder::new()
    .mode(ScanMode::Full)
    .max_depth(3)
    .build();
```

**Depth semantics:**
- Depth 0: The seed URL itself
- Depth 1: Pages directly linked from the seed
- Depth 2: Pages linked from depth-1 pages
- And so on...

**Practical depth recommendations:**

| Site Type | Recommended Depth | Rationale |
|-----------|-------------------|-----------|
| Single page | 0 (use Lite mode) | No link following needed |
| Landing page + links | 1 | Just the immediate neighborhood |
| Documentation site | 3 - 5 | Docs are typically 3-4 levels deep |
| Blog with archives | 5 - 10 | Posts may be nested under date paths |
| E-commerce catalog | 3 - 5 | Category > subcategory > product |
| Full site archive | 10 - 20 | Captures deeply nested content |

### Priority-Based URL Scheduling

Rehyke's scheduler uses a priority queue to decide which URLs to fetch next:

| Priority | Numeric Value | Assigned To |
|----------|---------------|-------------|
| `Critical` | 3 | Seed URLs, sitemaps, feed discoveries |
| `High` | 2 | Internal links at depth <= 1 |
| `Normal` | 1 | Internal links at depth >= 2 |
| `Low` | 0 | External links |

The crawler always processes the highest-priority URLs first, ensuring that important
pages (sitemaps, top-level pages) are crawled before deep or external links.

**URL lifecycle in the scheduler:**

```
seed URL
   |
   v
add_seed() --> visited set + frontier queue  [Priority: Critical]
   |
   v
next_task() --> in_progress set              [being fetched]
   |
   +--> mark_completed() --> visited set     [done]
   |
   +--> mark_failed()    --> visited set     [skip]
```

### Handling Pagination

For sites with paginated content, use include patterns to capture all pages:

```bash
# Crawl all pages of a paginated blog
rehyke https://blog.example.com \
    --mode full \
    --include '/blog/' \
    --include '/blog/page/\d+' \
    --max-depth 3
```

```rust
let config = CrawlConfigBuilder::new()
    .mode(ScanMode::Full)
    .include_patterns(vec![
        r"/blog/$".into(),
        r"/blog/page/\d+".into(),
    ])
    .max_depth(3)
    .build();
```

**Common pagination patterns and their regex:**

| Pattern | URL Example | Regex |
|---------|-------------|-------|
| Query string | `?page=2` | `\?.*page=\d+` |
| Path segment | `/page/2` | `/page/\d+` |
| Offset-based | `?offset=20` | `\?.*offset=\d+` |
| Cursor-based | `?cursor=abc123` | `\?.*cursor=[\w]+` |

**Tip:** To crawl paginated content without following every single page link, set a
moderate `max_pages` limit and let the priority scheduler do its work. Sitemap
discovery (which runs at `Critical` priority) will often find all the important pages
without needing to traverse pagination.

### Crawling JavaScript-Rendered Pages

Rehyke has a renderer module designed for headless Chromium integration. While JS
rendering is currently a stub that falls back to static fetching, the configuration
API is in place:

```rust
use rehyke_core::WaitStrategy;

let config = CrawlConfigBuilder::new()
    .enable_js(true)
    .js_wait_strategy(WaitStrategy::NetworkIdle)
    .build();

// Alternative: wait for a specific element
let config = CrawlConfigBuilder::new()
    .enable_js(true)
    .js_wait_strategy(WaitStrategy::Selector {
        selector: "div.content-loaded".into(),
    })
    .build();

// Alternative: wait a fixed duration
let config = CrawlConfigBuilder::new()
    .enable_js(true)
    .js_wait_strategy(WaitStrategy::Duration {
        duration: Duration::from_secs(5),
    })
    .build();
```

**Wait strategy guide:**

| Strategy | When to Use |
|----------|-------------|
| `NetworkIdle` | SPAs that load data via XHR/fetch |
| `Selector` | Pages where you know the target element |
| `Duration` | Pages with complex animations or delayed rendering |
| `Auto` | Let Rehyke decide (default) |

**Fallback behavior:** When `enable_js` is true but rendering fails, the system
automatically falls back to a static HTTP fetch. The `render_method` field in
`CrawlResult` records which path was taken (`Static` or `JavaScript`).

### Link Classification

Rehyke classifies every discovered link into one of six buckets, available in the
`CrawlResult.links` field:

| Bucket | Description | Example |
|--------|-------------|---------|
| `internal` | Same host as the seed URL | `https://example.com/about` |
| `external` | Different root domain | `https://other-site.com/page` |
| `subdomains` | Same root domain, different subdomain | `https://blog.example.com/post` |
| `resources` | CSS, JS, images, fonts, media | `https://cdn.example.com/style.css` |
| `feeds` | RSS / Atom feed URLs | `https://example.com/feed.xml` |
| `sitemaps` | Sitemap XML references | `https://example.com/sitemap.xml` |

Use these classifications to build link graphs, find broken links, discover feeds,
or identify external dependencies:

```rust
for page in &results {
    println!("Page: {}", page.url);
    println!("  Internal links: {}", page.links.internal.len());
    println!("  External links: {}", page.links.external.len());
    println!("  Subdomains:     {}", page.links.subdomains.len());
    println!("  Resources:      {}", page.links.resources.len());
    println!("  Feeds:          {:?}", page.links.feeds);
    println!("  Sitemaps:       {:?}", page.links.sitemaps);
}
```

---

## 7. Python Integration Guide

### Installation with Maturin

The Python bindings are built using PyO3 and compiled with maturin:

```bash
# Install maturin
pip install maturin

# Clone the repository
git clone https://github.com/user/rehyke.git
cd rehyke

# Build and install in development mode
cd crates/rehyke-python
maturin develop --release

# Or build a wheel for distribution
maturin build --release
pip install target/wheels/rehyke-*.whl

# Verify installation
python -c "import rehyke; print('rehyke ready')"
```

### Sync Usage

```python
import rehyke
from rehyke import Rehyke, CrawlConfig, ScanMode

# Quick one-liner
results = rehyke.crawl("https://example.com")

# With configuration
config = CrawlConfig(
    mode=ScanMode.FULL,
    max_depth=5,
    max_pages=1000,
    concurrency=10,
    clean_navigation=True,
    clean_ads=True,
    exclude_patterns=[r"\.pdf$", r"/login"],
)

crawler = Rehyke(config)
results = crawler.crawl("https://example.com")

# Access results
for page in results:
    print(f"URL:    {page.url}")
    print(f"Title:  {page.title}")
    print(f"Status: {page.status_code}")
    print(f"Depth:  {page.depth}")
    print(f"Words:  {len(page.markdown.split())}")
    print()
```

### Async Usage

Rehyke's Python API supports async/await through tokio's async runtime:

```python
import asyncio
from rehyke import Rehyke, CrawlConfig, ScanMode

async def main():
    config = CrawlConfig(
        mode=ScanMode.FULL,
        max_depth=3,
    )
    crawler = Rehyke(config)
    results = await crawler.crawl_async("https://example.com")

    for page in results:
        print(f"{page.title}: {len(page.markdown)} chars")

asyncio.run(main())
```

### Integration with Pandas

Build structured datasets from crawl results:

```python
import pandas as pd
from rehyke import Rehyke, CrawlConfig, ScanMode

config = CrawlConfig(mode=ScanMode.FULL, max_depth=3)
crawler = Rehyke(config)
results = crawler.crawl("https://docs.example.com")

# Convert to DataFrame
data = []
for page in results:
    data.append({
        "url": page.url,
        "title": page.title,
        "word_count": len(page.markdown.split()),
        "status": page.status_code,
        "depth": page.depth,
        "content_type": page.content_type,
        "internal_links": len(page.links.internal),
        "external_links": len(page.links.external),
        "has_feeds": len(page.links.feeds) > 0,
    })

df = pd.DataFrame(data)
print(df.describe())
print(df.sort_values("word_count", ascending=False).head(10))
df.to_csv("crawl_results.csv", index=False)
```

### Integration with BeautifulSoup

Use Rehyke for fast fetching and anti-detection, then process results with
BeautifulSoup for detailed analysis:

```python
from bs4 import BeautifulSoup
from rehyke import Rehyke, CrawlConfig, ScanMode

config = CrawlConfig(mode=ScanMode.LITE)
crawler = Rehyke(config)

# Rehyke handles fetching, retries, and anti-detection
results = crawler.crawl("https://shop.example.com/products")

for page in results:
    # Use the Markdown output for text content
    print(f"Markdown preview: {page.markdown[:200]}")

    # Access extracted links for further processing
    for link in page.links.internal:
        print(f"  Internal: {link}")
    for feed in page.links.feeds:
        print(f"  Feed: {feed}")
```

### Building Data Pipelines

Combine Rehyke with other tools for end-to-end data pipelines:

```python
import json
import re
from pathlib import Path
from rehyke import Rehyke, CrawlConfig, ScanMode

def crawl_and_extract(seed_url, output_dir):
    """Crawl a site and extract structured data."""

    # Step 1: Crawl the site
    config = CrawlConfig(
        mode=ScanMode.FULL,
        max_depth=5,
        max_pages=500,
        concurrency=10,
        clean_navigation=True,
        clean_ads=True,
        exclude_patterns=[r"\.(pdf|zip|exe)$"],
    )
    crawler = Rehyke(config)
    results = crawler.crawl(seed_url)

    # Step 2: Extract structured data with regex
    email_pattern = re.compile(r"[\w.+-]+@[\w-]+\.[\w.]+")
    phone_pattern = re.compile(r"\+?1?\s*\(?\d{3}\)?[\s.-]*\d{3}[\s.-]*\d{4}")

    extracted = []
    for page in results:
        emails = email_pattern.findall(page.markdown)
        phones = phone_pattern.findall(page.markdown)

        extracted.append({
            "url": page.url,
            "title": page.title,
            "emails": list(set(emails)),
            "phones": list(set(phones)),
            "word_count": len(page.markdown.split()),
            "internal_links": len(page.links.internal),
        })

    # Step 3: Save results
    output_path = Path(output_dir)
    output_path.mkdir(parents=True, exist_ok=True)

    with open(output_path / "extracted.json", "w") as f:
        json.dump(extracted, f, indent=2)

    # Step 4: Save Markdown files
    for page in results:
        slug = re.sub(r"[^\w-]", "_", page.url.split("//")[1])
        with open(output_path / f"{slug}.md", "w") as f:
            f.write(page.markdown)

    return extracted

# Run the pipeline
data = crawl_and_extract("https://example.com", "./output")
print(f"Extracted data from {len(data)} pages")
```

### Exporting to CSV/JSON/Databases

#### Export to CSV

```python
import csv
from rehyke import Rehyke, CrawlConfig, ScanMode

results = Rehyke(CrawlConfig(mode=ScanMode.FULL)).crawl("https://example.com")

with open("crawl.csv", "w", newline="") as f:
    writer = csv.DictWriter(f, fieldnames=[
        "url", "title", "status", "depth", "word_count",
    ])
    writer.writeheader()
    for page in results:
        writer.writerow({
            "url": page.url,
            "title": page.title,
            "status": page.status_code,
            "depth": page.depth,
            "word_count": len(page.markdown.split()),
        })
```

#### Export to JSON

```python
import json
from rehyke import Rehyke, CrawlConfig, ScanMode

results = Rehyke(CrawlConfig(mode=ScanMode.FULL)).crawl("https://example.com")

data = [
    {
        "url": p.url,
        "title": p.title,
        "markdown": p.markdown,
        "metadata": {
            "description": p.metadata.description,
            "author": p.metadata.author,
            "language": p.metadata.language,
        },
        "links": {
            "internal": p.links.internal,
            "external": p.links.external,
            "feeds": p.links.feeds,
        },
    }
    for p in results
]

with open("crawl.json", "w") as f:
    json.dump(data, f, indent=2)
```

#### Export to SQLite

```python
import sqlite3
from rehyke import Rehyke, CrawlConfig, ScanMode

results = Rehyke(CrawlConfig(mode=ScanMode.FULL)).crawl("https://example.com")

conn = sqlite3.connect("crawl.db")
conn.execute("""
    CREATE TABLE IF NOT EXISTS pages (
        url TEXT PRIMARY KEY,
        title TEXT,
        markdown TEXT,
        status_code INTEGER,
        depth INTEGER,
        content_type TEXT,
        crawled_at TEXT,
        word_count INTEGER
    )
""")

for page in results:
    conn.execute(
        "INSERT OR REPLACE INTO pages VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        (
            page.url,
            page.title,
            page.markdown,
            page.status_code,
            page.depth,
            page.content_type,
            str(page.crawled_at),
            len(page.markdown.split()),
        ),
    )

conn.commit()
conn.close()
print(f"Saved {len(results)} pages to crawl.db")
```

---

## 8. CLI Power User Guide

### All Command-Line Options Explained

```
USAGE:
    rehyke [OPTIONS] <URL>

ARGUMENTS:
    <URL>    URL to crawl (must include scheme: https://...)

OPTIONS:
    --mode <MODE>              Scan mode preset [default: full]
                               Values: lite, full, deep

    --max-depth <N>            Maximum crawl depth from the seed URL
                               Overrides the mode's default depth

    --max-pages <N>            Maximum number of pages to crawl
                               Overrides the mode's default page limit

    --concurrency <N>          Number of concurrent requests
                               Overrides the mode's default concurrency

    --js                       Enable JavaScript rendering via headless browser
                               Requires Chrome/Chromium installed

    --output-dir <DIR>         Write individual .md files to this directory
                               Creates the directory if it does not exist

    -o, --output <FILE>        Write all output to a single file
                               All pages concatenated with --- separators

    --structure <STRUCTURE>    File structure when using --output-dir
                               [default: flat]
                               Values: flat, mirror

    --proxy <URL>              Proxy URL (can be specified multiple times)
                               Supports http://, https://, socks5://

    --delay <MIN-MAX>          Delay between requests in milliseconds
                               Single number for fixed: "1000"
                               Range for random: "500-2000"

    --exclude <PATTERN>        Exclude URLs matching this regex
                               (can be specified multiple times)

    --include <PATTERN>        Only crawl URLs matching this regex
                               (can be specified multiple times)

    --no-robots                Ignore robots.txt directives

    --clean-nav                Remove navigation elements from content

    --clean-footer             Remove footer elements from content

    --clean-ads                Remove advertisement elements from content

    --timeout <SECONDS>        Per-request timeout in seconds
                               [default: 30]

    --retries <N>              Number of retry attempts for failed requests
                               [default: 3]

    --user-agent <STRING>      Custom User-Agent header
                               Overrides the rotating UA pool

    --format <FORMAT>          Output format [default: markdown]
                               Values: markdown, json

    -v, --verbose              Enable verbose (debug-level) logging

    -h, --help                 Print help information

    -V, --version              Print version information
```

### Combining Flags for Common Workflows

#### Documentation site archival

```bash
rehyke https://docs.example.com \
    --mode full \
    --max-depth 10 \
    --output-dir ./docs-archive \
    --structure mirror \
    --delay 500-1500 \
    --exclude '\.(pdf|zip)$' \
    --clean-nav \
    --clean-footer
```

#### Blog content extraction

```bash
rehyke https://blog.example.com \
    --mode full \
    --include '/blog/' \
    --include '/posts/' \
    --exclude '/tag/' \
    --exclude '/category/' \
    --exclude '/page/\d+' \
    --output-dir ./blog-content \
    --structure flat \
    --clean-nav \
    --clean-footer \
    --clean-ads
```

#### Quick content grab for LLM context

```bash
rehyke https://example.com/important-page --mode lite -o context.md
```

#### Full site to single file for search indexing

```bash
rehyke https://docs.example.com \
    --mode full \
    -o full-site.md \
    --clean-nav \
    --clean-footer \
    --clean-ads
```

#### Deep research crawl through a proxy

```bash
rehyke https://research-site.com \
    --mode deep \
    --max-depth 15 \
    --max-pages 10000 \
    --concurrency 50 \
    --proxy socks5://proxy.example.com:1080 \
    --delay 1000-3000 \
    --output-dir ./research \
    --structure mirror \
    --timeout 60 \
    --retries 5 \
    -v
```

#### JSON metadata analysis

```bash
rehyke https://example.com --mode full --format json > results.json

# Then process with jq
cat results.json | jq '.[].title'
cat results.json | jq '[.[] | {url: .url, links: (.links.internal | length)}]'
cat results.json | jq '[.[] | select(.status_code >= 400)]'
```

#### E-commerce product scraping

```bash
rehyke https://shop.example.com \
    --mode full \
    --include '/products/' \
    --include '/product/' \
    --exclude '/cart' \
    --exclude '/checkout' \
    --exclude '/wishlist' \
    --max-depth 4 \
    --delay 1000-3000 \
    --output-dir ./products \
    --structure flat \
    --clean-nav \
    --clean-footer \
    --clean-ads
```

### Output Formatting and Piping

Rehyke writes Markdown (or JSON) to stdout and status messages to stderr. This
means you can pipe the output cleanly:

```bash
# Pipe to a pager
rehyke https://example.com --mode lite | less

# Count words
rehyke https://example.com --mode lite | wc -w

# Search the output
rehyke https://example.com --mode full | grep -i "important topic"

# Extract all URLs from Markdown links
rehyke https://example.com --mode full | grep -oP '\[.*?\]\(\K[^)]+' | sort -u

# Feed into another tool
rehyke https://docs.example.com --mode full -o - | my-search-indexer --input -

# JSON processing pipeline
rehyke https://example.com --mode full --format json | \
    jq '.[] | select(.status_code == 200) | .url'

# Count pages per depth level
rehyke https://example.com --mode full --format json | \
    jq '[group_by(.depth)[] | {depth: .[0].depth, count: length}]'
```

### Batch Crawling Scripts

#### Crawl multiple sites from a file

```bash
#!/bin/bash
# sites.txt contains one URL per line

while IFS= read -r site; do
    domain=$(echo "$site" | sed 's|https\?://||; s|/.*||')
    echo "Crawling $domain..."
    rehyke "$site" \
        --mode full \
        --output-dir "./output/$domain" \
        --structure mirror \
        --delay 1000-2000 \
        --clean-nav \
        --clean-footer 2>"./logs/$domain.log"
    echo "Done: $domain"
    sleep 5  # Pause between sites
done < sites.txt
```

#### Scheduled daily crawl

```bash
#!/bin/bash
# Add to crontab: 0 2 * * * /path/to/daily-crawl.sh

SITE="https://news.example.com"
DATE=$(date +%Y-%m-%d)
OUTPUT_DIR="./crawls/$DATE"

mkdir -p "$OUTPUT_DIR"

rehyke "$SITE" \
    --mode full \
    --max-depth 3 \
    --output-dir "$OUTPUT_DIR" \
    --structure flat \
    --delay 500-1500 \
    --clean-nav \
    --clean-footer \
    --clean-ads \
    --format json > "$OUTPUT_DIR/metadata.json" 2>"$OUTPUT_DIR/crawl.log"

PAGE_COUNT=$(cat "$OUTPUT_DIR/metadata.json" | jq '. | length')
echo "$(date): Crawled $PAGE_COUNT pages. Output in $OUTPUT_DIR" >> crawl-history.log
```

#### Parallel multi-site crawling with GNU parallel

```bash
#!/bin/bash
# Crawl multiple sites in parallel (4 at a time)

cat sites.txt | parallel -j 4 \
    'domain=$(echo {} | sed "s|https\?://||; s|/.*||"); \
     mkdir -p "./output/$domain"; \
     rehyke {} --mode full \
         --output-dir "./output/$domain" \
         --structure mirror \
         --delay 1000-2000 \
         2>"./logs/$domain.log"'
```

---

## 9. Performance Tuning

### Concurrency Settings

Concurrency controls how many HTTP requests are in flight simultaneously. The right
setting depends on your hardware, network bandwidth, and the target server's
capacity.

**Guidelines:**

| Scenario | Recommended Concurrency |
|----------|------------------------|
| Polite crawling of external sites | 5 - 10 |
| Fast crawling with permission | 20 - 50 |
| Internal/owned sites | 50 - 100 |
| Maximum throughput (localhost) | 100 - 200 |

```rust
// Conservative: 5 concurrent requests
let config = CrawlConfigBuilder::new()
    .mode(ScanMode::Full)
    .concurrency(5)
    .build();

// Aggressive: 100 concurrent requests (use with caution)
let config = CrawlConfigBuilder::new()
    .mode(ScanMode::Deep)
    .concurrency(100)
    .build();
```

```bash
# CLI: set concurrency
rehyke https://example.com --concurrency 20
```

**Warning:** Setting concurrency too high can overwhelm target servers, get your IP
blocked, or exhaust your system's file descriptors. Always pair high concurrency with
appropriate delay strategies.

### Memory Management

Rehyke is designed for low memory consumption (< 5MB per page). Key strategies:

**Use file output for large crawls:**

```rust
// Bad: holding 50,000 pages in memory
let config = CrawlConfigBuilder::new()
    .mode(ScanMode::Deep)
    .output(OutputMode::Memory)  // All results in RAM
    .build();

// Good: streaming to disk as pages arrive
let config = CrawlConfigBuilder::new()
    .mode(ScanMode::Deep)
    .output(OutputMode::Files {
        output_dir: PathBuf::from("./output"),
        structure: FileStructure::Flat,
    })
    .build();
```

**Lock-free data structures:** The scheduler uses `DashSet` and `DashMap` (from the
`dashmap` crate v6) for URL deduplication and per-domain timestamps, avoiding mutex
contention under high concurrency.

**Atomic counters:** Crawl statistics use `AtomicUsize` with relaxed ordering,
providing fast, lock-free progress tracking without synchronization overhead.

**Minimal critical sections:** The priority queue (`BinaryHeap`) is wrapped in a
`std::sync::Mutex`, but the critical section is kept minimal: `next_task()` pops one
item and immediately releases the lock.

### Network Optimization

**HTTP/2 multiplexing:** Rehyke uses reqwest with HTTP/2 support via ALPN
negotiation. Multiple requests to the same host can share a single TCP connection,
reducing connection overhead.

**Compression:** All four compression algorithms are enabled:
- gzip
- brotli
- deflate
- zstd

This significantly reduces bandwidth usage, especially for text-heavy content.

**Connection pooling:** The reqwest client automatically pools connections per host,
reusing TCP connections across requests.

**Cookie persistence:** The cookie store is persistent across requests within a crawl,
maintaining session state without explicit configuration.

**TLS via rustls:** No OpenSSL dependency needed. The rustls backend is fast, secure,
and cross-platform.

### When to Use Different Scan Modes

| Scenario | Mode | Custom Settings |
|----------|------|-----------------|
| Grab one page for LLM context | Lite | Default |
| Archive documentation site | Full | `max_depth(10)`, mirror output |
| Build training dataset | Full | `max_pages(10000)`, no cleaning |
| Research competitor sites | Deep | `include_patterns`, `max_depth(5)` |
| Monitor for changes | Lite | Run on schedule, compare output |
| Extract all feeds from a domain | Full | Check `results.links.feeds` |
| Sitemap-driven crawl | Full | Sitemap auto-discovery enabled |
| Cross-domain link analysis | Deep | `max_pages(5000)`, JSON output |

### Benchmarking Your Crawls

Use the verbose flag and JSON output to measure performance:

```bash
# Time the crawl
time rehyke https://example.com --mode full --format json > /dev/null

# Verbose logging shows per-request timing
rehyke https://example.com --mode full -v 2>&1 | grep "elapsed"

# JSON output includes crawl timestamps
rehyke https://example.com --mode full --format json | \
    jq '[.[] | .crawled_at] | sort | {first: .[0], last: .[-1]}'
```

In Rust, measure programmatically:

```rust
use std::time::Instant;

let start = Instant::now();
let results = crawler.run("https://example.com").await?;
let elapsed = start.elapsed();

println!(
    "Crawled {} pages in {:.2}s ({:.1} pages/sec)",
    results.len(),
    elapsed.as_secs_f64(),
    results.len() as f64 / elapsed.as_secs_f64()
);

// Breakdown by status code
let ok_count = results.iter().filter(|p| p.status_code == 200).count();
let error_count = results.iter().filter(|p| p.status_code >= 400).count();
println!("  200 OK: {}  |  Errors: {}", ok_count, error_count);
```

In Python:

```python
import time
from rehyke import Rehyke, CrawlConfig, ScanMode

config = CrawlConfig(mode=ScanMode.FULL)
crawler = Rehyke(config)

start = time.time()
results = crawler.crawl("https://example.com")
elapsed = time.time() - start

print(f"Crawled {len(results)} pages in {elapsed:.2f}s")
print(f"  {len(results) / elapsed:.1f} pages/sec")
```

### Performance Targets

| Metric | Target |
|--------|--------|
| Single page (Lite, no JS) | < 200ms |
| Single page (Lite, with JS) | < 3s |
| 100 pages (Full, parallel) | < 30s |
| 1,000 pages (Full, parallel) | < 5 min |
| 10,000 pages (Deep, parallel) | < 30 min |
| Memory per page | < 5MB |
| Max concurrent connections | Up to 200 |

---

## 10. Troubleshooting

### Common Errors and Solutions

#### ConfigError: "invalid seed URL"

**Cause:** The URL you provided cannot be parsed.

**Solution:** Ensure the URL includes a scheme (`http://` or `https://`):

```bash
# Wrong
rehyke example.com

# Correct
rehyke https://example.com
```

#### HttpError 403 (Forbidden)

**Cause:** The server is blocking your request, likely due to user agent detection
or IP-based blocking.

**Solutions:**

1. Use a realistic user agent:
   ```bash
   rehyke https://example.com \
       --user-agent "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"
   ```

2. Add delays to avoid triggering rate limits:
   ```bash
   rehyke https://example.com --delay 2000-5000
   ```

3. Use a proxy:
   ```bash
   rehyke https://example.com --proxy http://proxy.example.com:8080
   ```

4. Add custom headers (Rust):
   ```rust
   let config = CrawlConfigBuilder::new()
       .header("Referer", "https://google.com")
       .header("Accept-Language", "en-US,en;q=0.9")
       .build();
   ```

#### HttpError 429 (Too Many Requests)

**Cause:** You are sending requests too fast.

**Solutions:**

1. Increase delays:
   ```bash
   rehyke https://example.com --delay 3000-6000
   ```

2. Reduce concurrency:
   ```bash
   rehyke https://example.com --concurrency 3
   ```

3. Use adaptive delay strategy (Rust):
   ```rust
   let config = CrawlConfigBuilder::new()
       .delay_strategy(DelayStrategy::Adaptive {
           initial: Duration::from_secs(1),
       })
       .concurrency(5)
       .build();
   ```

4. Increase retry count to wait out rate limits:
   ```bash
   rehyke https://example.com --retries 5 --delay 2000-4000
   ```

#### Timeout Errors

**Cause:** Requests are taking longer than the configured timeout.

**Solutions:**

1. Increase the timeout:
   ```bash
   rehyke https://slow-site.com --timeout 120
   ```

2. Check your network connection.

3. If using a proxy, verify the proxy is responsive.

4. In Rust:
   ```rust
   let config = CrawlConfigBuilder::new()
       .timeout(Duration::from_secs(120))
       .build();
   ```

#### TLS/SSL Errors

**Cause:** The server's TLS certificate is invalid, expired, or uses an unsupported
cipher.

**Solution:** This is a hard error and cannot be retried. Check that:
- The URL uses the correct hostname
- The server's certificate is valid
- Your system's root certificates are up to date

#### DNS Resolution Failures

**Cause:** The hostname cannot be resolved to an IP address.

**Solutions:**
- Verify the URL is correct
- Check your DNS configuration
- Try using a different DNS resolver (e.g., `8.8.8.8`)
- These errors are retryable -- Rehyke will automatically retry them

#### Parse Errors

**Cause:** The fetched content could not be parsed (malformed HTML, invalid XML,
etc.).

**Solution:** Parse errors are non-retryable. The content is returned as raw text
when possible. Check the content type of the URL -- you may be trying to parse a
binary file. Add binary extensions to your exclude patterns:

```bash
rehyke https://example.com --exclude '\.(pdf|doc|xls|ppt|zip)$'
```

#### ProxyError

**Cause:** The proxy connection failed or authentication was rejected.

**Solutions:**
- Verify the proxy URL and credentials
- Test the proxy independently: `curl -x http://proxy:8080 https://example.com`
- If using multiple proxies, switch to `FailoverOnly` strategy
- Check that the proxy supports the protocol you are using (HTTP vs SOCKS5)

#### MaxPagesReached

**Cause:** The configured page limit has been reached.

**This is not an error** -- it is expected behavior. The crawl stops gracefully
when the limit is hit. If you need more pages:

```bash
rehyke https://example.com --max-pages 10000
```

### Debug Logging

Enable verbose logging to see detailed information about every request:

```bash
# CLI verbose mode
rehyke https://example.com --mode full -v

# Or set the RUST_LOG environment variable for fine-grained control
RUST_LOG=debug rehyke https://example.com --mode full

# Even more detail
RUST_LOG=trace rehyke https://example.com --mode lite

# Module-specific logging
RUST_LOG=rehyke_core::fetcher=debug rehyke https://example.com
RUST_LOG=rehyke_core::scheduler=debug rehyke https://example.com
RUST_LOG=rehyke_core::parser=debug rehyke https://example.com

# Multiple modules
RUST_LOG=rehyke_core::fetcher=debug,rehyke_core::scheduler=info rehyke https://example.com
```

In Rust code:

```rust
// Initialize tracing subscriber
tracing_subscriber::fmt()
    .with_env_filter("rehyke_core=debug")
    .init();

let results = crawler.run("https://example.com").await?;
```

Debug output includes:
- Request URLs, status codes, and timing
- Content type detection results
- URL normalization decisions
- Scheduler queue operations (enqueue, dequeue, dedup)
- Retry attempts and backoff delays
- Link extraction counts per page

### Network Issues

#### Behind a corporate proxy

```bash
rehyke https://example.com --proxy http://corporate-proxy.internal:3128
```

If the proxy requires authentication, configure it in Rust:

```rust
let config = CrawlConfigBuilder::new()
    .proxies(vec![ProxyConfig {
        url: "http://corporate-proxy.internal:3128".into(),
        proxy_type: ProxyType::Http,
        auth: Some(ProxyAuth {
            username: "your_username".into(),
            password: "your_password".into(),
        }),
        region: None,
    }])
    .build();
```

#### Through a VPN

No special configuration needed. If the VPN changes your DNS, make sure the target
site is reachable through the VPN.

#### IPv6 issues

If your network has IPv6 connectivity problems, the reqwest client will fall back
to IPv4 automatically.

### Rate Limiting Detection

Signs that you are being rate-limited:

1. **HTTP 429 responses** -- Rehyke handles these automatically with backoff
2. **Increasing response times** -- The server is throttling you
3. **HTTP 503 responses** -- The server is overloaded or blocking you
4. **Connection resets** -- The server is dropping your connections
5. **CAPTCHAs in HTML** -- The server is challenging you (visible in Markdown output)

**Recommended approach when rate-limited:**

```bash
rehyke https://example.com \
    --delay 3000-8000 \
    --concurrency 3 \
    --retries 5 \
    --timeout 60 \
    -v
```

### robots.txt Compliance

Rehyke respects robots.txt by default. The parser supports:
- `User-agent` directives (matches `rehyke` and `*`)
- `Disallow` paths with wildcard (`*`) matching
- `Allow` paths (overrides Disallow)
- `Sitemap` directives (auto-discovers sitemaps)
- `$` anchor for end-of-URL matching
- `Crawl-delay` directives

To check what robots.txt says about a site before crawling:

```bash
curl https://example.com/robots.txt
```

To disable robots.txt compliance (use responsibly):

```bash
rehyke https://example.com --no-robots
```

```rust
let config = CrawlConfigBuilder::new()
    .respect_robots_txt(false)
    .build();
```

**Note:** Even with robots.txt disabled, you should still use delays and reasonable
concurrency to avoid overwhelming servers. robots.txt compliance is a courtesy, but
rate limiting is a practical necessity.

### Error Classification Reference

| Error | Retryable | Recovery |
|-------|-----------|----------|
| `HttpError{403}` | No | Rotate UA/proxy, add delays |
| `HttpError{404}` | No | Skip URL, mark as failed |
| `HttpError{429}` | Yes | Exponential backoff, Retry-After |
| `HttpError{5xx}` | Yes | Exponential backoff |
| `Timeout` | Yes | Retry with backoff |
| `DnsError` | Yes | Retry (may be transient) |
| `TlsError` | No | Check certificate, skip URL |
| `RenderError` | Depends | Fall back to static fetch |
| `BrowserError` | No | Disable JS rendering |
| `ParseError` | No | Return raw text fallback |
| `ProxyError` | No | Try next proxy or direct |
| `RateLimited` | Yes | Wait and retry (scheduler-level) |
| `MaxPagesReached` | No | Stop crawl (expected behavior) |
| `IoError` | No | Check disk space/permissions |
| `ConfigError` | No | Fix configuration and restart |
| `UrlParseError` | No | Skip URL |
| `RequestError` | Depends | Classified by `is_network_error()` |

---

## Appendix: Quick Reference Card

### CLI Commands at a Glance

```bash
# Single page to stdout
rehyke https://example.com --mode lite

# Full site to directory (mirrored)
rehyke https://example.com --mode full --output-dir ./out --structure mirror

# Polite crawl with delays
rehyke https://example.com --delay 1000-3000 --concurrency 5

# Blog-only extraction
rehyke https://example.com --include '/blog/' --exclude '\.(pdf|zip)$'

# JSON output for processing
rehyke https://example.com --format json > results.json

# Through a proxy
rehyke https://example.com --proxy socks5://proxy:1080

# Debug mode
rehyke https://example.com -v
```

### Rust Quick Reference

```rust
use rehyke_core::{Rehyke, CrawlConfigBuilder, ScanMode, DelayStrategy};
use std::time::Duration;

// One-liner
let results = Rehyke::crawl("https://example.com", ScanMode::Full).await?;

// Custom config
let config = CrawlConfigBuilder::new()
    .mode(ScanMode::Full)
    .max_depth(5)
    .concurrency(10)
    .delay_strategy(DelayStrategy::Random {
        min: Duration::from_millis(500),
        max: Duration::from_secs(2),
    })
    .exclude_patterns(vec![r"\.pdf$".into()])
    .build();

let crawler = Rehyke::new(config);
let results = crawler.run("https://example.com").await?;

// Process results
for page in &results {
    println!("{}: {} words", page.title, page.markdown.split_whitespace().count());
}
```

### Python Quick Reference

```python
import rehyke
from rehyke import Rehyke, CrawlConfig, ScanMode

# One-liner
results = rehyke.crawl("https://example.com")

# Custom config
config = CrawlConfig(
    mode=ScanMode.FULL,
    max_depth=5,
    concurrency=10,
    exclude_patterns=[r"\.pdf$"],
)
crawler = Rehyke(config)
results = crawler.crawl("https://example.com")

# Process results
for page in results:
    print(f"{page.title}: {len(page.markdown.split())} words")
```

### Regex Quick Reference

```
# URL filtering
\.pdf$                          Skip PDF files
/login|/admin                   Skip auth pages
\?.*utm_                        Skip tracking URLs
/blog/                          Include blog pages only

# Data extraction (apply to Markdown output)
[\w.+-]+@[\w-]+\.[\w.]+         Email addresses
\$\d{1,3}(?:,\d{3})*(?:\.\d{2})?   USD prices
\d{4}-\d{2}-\d{2}              ISO dates
\+?1?\s*\(?\d{3}\)?[\s.-]*\d{3}[\s.-]*\d{4}   Phone numbers
https?://[^\s\)\]>]+            URLs in text
```

---

*Built with Rust. Powered by Regex. Ready for Everything.*
