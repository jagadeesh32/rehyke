# Architecture Guide

## Overview

Rehyke is an ultra-high-performance web crawler written in Rust, designed around the
philosophy of **"Crawl Everything. Miss Nothing."** It converts arbitrary web content
into clean, structured Markdown suitable for feeding into LLMs, building datasets,
archiving documentation, and general-purpose web scraping.

The project is organized as a Cargo workspace with three crates:

| Crate            | Role                                                      |
|------------------|-----------------------------------------------------------|
| `rehyke-core`    | Core crawl engine: fetching, parsing, converting, scheduling |
| `rehyke-cli`     | Command-line interface built on `clap`                    |
| `rehyke-python`  | Python bindings via `pyo3` / `maturin`                    |

**Design principles:**

1. **Rust core, polyglot surface** -- All performance-critical logic lives in Rust.
   Python users get the same engine through zero-copy FFI bindings.
2. **Pipeline architecture** -- Each crawl stage is a distinct module with clear
   inputs and outputs, making it easy to test, replace, or extend individual stages.
3. **Async-first** -- Built on `tokio` with `reqwest` for non-blocking I/O, enabling
   thousands of concurrent connections on a single machine.
4. **Configurable politeness** -- Delay strategies, robots.txt compliance, and
   per-domain rate limiting are first-class citizens, not afterthoughts.

---

## System Architecture Diagram

```
                        +--------------------+
                        |   User / Config    |
                        |  (CLI / Python /   |
                        |   CrawlConfig)     |
                        +---------+----------+
                                  |
                                  v
                        +--------------------+
                   +--->|    Scheduler       |<---+
                   |    |  (Priority Queue)  |    |
                   |    |  (Dedup / Depth)   |    |
                   |    +--------+-----------+    |
                   |             |                 |
                   |             | next_task()     |
                   |             v                 |
                   |    +--------------------+     |
                   |    |    Anti-Detect      |    |
                   |    | (UA Pool / Headers) |    |
                   |    +--------+-----------+     |
                   |             |                 |
                   |             | rotate UA,      |
                   |             | compute delay   |
                   |             v                 |
                   |    +--------------------+     |
                   |    |     Fetcher        |     |
                   |    | (reqwest + retry)  |     |
                   |    | (proxy, TLS, H/2)  |     |
                   |    +--------+-----------+     |
                   |             |                 |
                   |             | FetchResult     |
                   |             v                 |
                   |    +--------------------+     |
                   |    | Content Detection  |     |
                   |    | (headers/URL/body) |     |
                   |    +--------+-----------+     |
                   |             |                 |
                   |             | ContentType     |
                   |             v                 |
                   |    +--------------------+     |
                   |    |      Parser        |     |
                   |    | (HTML/RSS/Atom/    |     |
                   |    |  XML/JSON/Text)    |     |
                   |    +--------+-----------+     |
                   |             |                 |
                   |             | ParsedDocument  |
                   |             v                 |
                   |    +--------------------+     |
                   |    |    Converter       |     |
                   |    | (Markdown + YAML   |     |
                   |    |  frontmatter)      |     |
                   |    +--------+-----------+     |
                   |             |                 |
                   |             | Markdown        |
                   |             v                 |
                   |    +--------------------+     |
                   |    |    Extractor       |     |
          new URLs |    | (links, feeds,     |     | discovered
          fed back |    |  sitemaps, etc.)   |     | URLs
                   |    +--------+-----------+     |
                   |             |                 |
                   |             v                 |
                   |    +--------------------+     |
                   +----|  Output Handler    |-----+
                        | (Memory / Files /  |
                        |  SingleFile)       |
                        +--------------------+
                                  |
                                  v
                        +--------------------+
                        |   CrawlResult[]    |
                        | (URL, title, MD,   |
                        |  metadata, links)  |
                        +--------------------+
```

---

## Module Deep Dive

### Scheduler (`scheduler.rs`)

The scheduler is the brain of the crawl. It owns the **frontier** (the set of URLs
waiting to be fetched) and enforces all scope, depth, deduplication, and rate-limiting
rules.

**Core data structures:**

```rust
pub struct Scheduler {
    frontier:      Mutex<BinaryHeap<CrawlTask>>,   // priority queue
    visited:       DashSet<String>,                  // dedup set (normalized URLs)
    in_progress:   DashSet<String>,                  // currently being fetched
    domain_delays: DashMap<String, Instant>,          // per-domain timestamps
    stats:         Arc<CrawlStats>,                  // atomic counters
    seed_url:      Mutex<Option<Url>>,               // for same-domain checks
    // ... configuration fields ...
}
```

**Priority queue design.** The frontier is a `std::collections::BinaryHeap<CrawlTask>`,
which is a max-heap. Each `CrawlTask` carries a `Priority` enum:

| Priority   | Numeric | Assigned to                                     |
|------------|---------|--------------------------------------------------|
| `Critical` | 3       | Seed URLs, sitemap/feed discoveries              |
| `High`     | 2       | Internal links at depth <= 1                     |
| `Normal`   | 1       | Internal links at depth >= 2                     |
| `Low`      | 0       | External links                                   |

The `Ord` implementation on `CrawlTask` delegates entirely to `Priority`, so the
heap always dequeues the highest-priority task first.

**URL normalization and deduplication.** Every URL passes through
`utils::normalize_url()` before being stored in the `visited` DashSet. Normalization
strips fragments (`#section`), optionally removes the `www.` prefix, and lowercases
the scheme and host. The DashSet is lock-free for concurrent reads and writes,
allowing multiple worker tasks to check and insert URLs without contention.

**Per-domain rate limiting.** The `domain_delays` DashMap stores the `Instant` of the
last completed request for each domain. When `next_task()` pops a task from the heap,
it checks whether the domain's cooldown has elapsed. If not, the task is deferred and
the next one is tried (up to `queue.len()` attempts to avoid busy-looping). This
ensures politeness without blocking other domains.

**Scan mode behavior:**

| Mode   | Depth | Pages  | Concurrency | Link following                |
|--------|-------|--------|-------------|-------------------------------|
| `Lite` | 2     | 100    | 5           | None -- single page only      |
| `Full` | 5     | 1,000  | 10          | Same-domain (internal) only   |
| `Deep` | 50    | 50,000 | 25          | Both internal and external    |

- **Lite** mode calls `add_urls()` with an early return, so discovered links are
  never enqueued.
- **Full** mode checks `utils::is_same_domain()` and rejects external URLs.
- **Deep** mode allows everything through the domain filter.

**CrawlTask lifecycle:**

```
   seed URL
      |
      v
  add_seed() ---> visited set + frontier queue  [status: queued]
      |
      v
  next_task() --> in_progress set               [status: in_progress]
      |
      +---> mark_completed() --> visited set    [status: completed]
      |
      +---> mark_failed()    --> visited set    [status: failed]
```

Failed URLs are also added to the `visited` set to prevent retries at the
scheduler level (retries happen inside the fetcher).

---

### Fetcher (`fetcher.rs`)

The fetcher is responsible for downloading a single URL and returning a
`FetchResult` that includes the response body, headers, status code, and detected
content type.

**reqwest client configuration.** The `Client` is built once per crawl with:

- **HTTP/2 support** via ALPN negotiation (the reqwest default when using rustls)
- **Compression**: gzip, brotli, deflate, and zstd decompression enabled
- **Redirect policy**: follows up to 10 hops
- **Cookie store**: persistent across requests in the same crawl
- **TLS**: rustls backend (no OpenSSL dependency)
- **Proxy**: first entry from `config.proxies` is applied if present

**Retry with exponential backoff.** `fetch_with_retry()` wraps `fetch()` and
retries on transient failures:

```
Retryable conditions:
  - Network / IO errors (timeout, DNS, connection reset)
  - HTTP 429 (Too Many Requests)
  - HTTP 500, 502, 503, 504 (server errors)

Non-retryable:
  - HTTP 403 (Forbidden) -- hard error, anti-detect should handle rotation
  - HTTP 404, 410, etc. -- hard error
  - TLS errors -- hard error

Backoff formula:
  delay = min(initial_delay * 2^attempt, max_delay)

Retry-After header:
  If the server sends Retry-After with an integer value (seconds),
  that value is used instead of the computed backoff (capped at max_delay).
```

Default retry parameters: 3 retries, 500ms initial delay, 30s maximum delay.

**Content-type detection pipeline.** The `detect_content_type()` function uses a
three-stage cascade:

1. **HTTP `Content-Type` header** -- Highest priority. Matches against known MIME
   types (text/html, application/rss+xml, application/json, etc.).
2. **URL file extension** -- If no header is present, the URL path extension is
   checked (.html, .xml, .rss, .json, .svg, .txt).
3. **Body sniffing** -- As a last resort, the first few characters of the body are
   inspected (`<!DOCTYPE html`, `<rss`, `<feed`, `<urlset`, `{`, `[`, etc.).

For generic XML content-types (`application/xml`, `text/xml`), a sub-type detector
inspects the body to distinguish RSS, Atom, Sitemap, SVG, and generic XML.

Detected content types:

```
Html | Xhtml | Xml | Rss | Atom | Json | JsonLd | Svg | PlainText | Sitemap | Other(String)
```

---

### Parser (`parser.rs`)

The parser transforms raw response bodies into a unified `ParsedDocument`
representation that the converter can render to Markdown.

**Content type dispatch.** The top-level `parse()` function dispatches based on the
`ContentType` enum:

```rust
match content_type {
    Html | Xhtml       => parse_html(body, config),
    Rss                => parse_rss(body),
    Atom               => parse_atom(body),
    Xml | Sitemap      => parse_xml(body),
    Json | JsonLd      => parse_json(body),
    Svg | PlainText    => wrap as RawText,
    Other(_)           => wrap as RawText (with warning),
}
```

**HTML DOM walking algorithm.** The HTML parser uses the `scraper` crate to build a
DOM tree, then:

1. **Extract metadata** from `<title>`, `<meta>` (description, author, og:image,
   keywords, published date, language), and `<link rel="canonical">`.
2. **Clean unwanted elements** according to `ParseConfig`:
   - `<nav>` elements when `clean_navigation` is true
   - `<footer>` elements when `clean_footers` is true
   - Elements with class/id matching ad patterns (`ad`, `banner`, `sponsor`, etc.)
     when `clean_ads` is true
   - Comment sections when `clean_comments` is true
   - `<script>`, `<style>`, and `<noscript>` elements (always removed)
3. **Walk the cleaned DOM** to produce `ContentNode` variants: headings, paragraphs,
   links, images, lists, tables, code blocks, blockquotes, definition lists, media
   elements, and more.

**ContentNode variants:**

```rust
enum ContentNode {
    Heading { level: u8, text: String },
    Paragraph(String),
    Link { text: String, href: String },
    Image { alt: String, src: String },
    Bold(String),
    Italic(String),
    Code(String),
    CodeBlock { language: Option<String>, code: String },
    UnorderedList(Vec<String>),
    OrderedList(Vec<String>),
    Blockquote(String),
    Table { headers: Vec<String>, rows: Vec<Vec<String>> },
    HorizontalRule,
    LineBreak,
    Strikethrough(String),
    DefinitionList(Vec<(String, String)>),
    Media { media_type: String, title: String, src: String },
    RawText(String),
}
```

**RSS/Atom/XML/JSON parsing.** Feed parsers use `quick-xml` to extract items with
title, link, description, author, and publication date. JSON content is parsed with
`serde_json` and rendered as formatted text or code blocks.

---

### Converter (`converter.rs`)

The converter transforms a `ParsedDocument` into a single Markdown string, ready for
output.

**ContentNode to Markdown mapping:**

| ContentNode        | Markdown output                              |
|--------------------|----------------------------------------------|
| `Heading{1}`       | `# Title`                                    |
| `Heading{3}`       | `### Title`                                  |
| `Paragraph`        | `text\n\n`                                   |
| `Link`             | `[text](href)`                               |
| `Image`            | `![alt](src)`                                |
| `Bold`             | `**text**`                                   |
| `Italic`           | `*text*`                                     |
| `Code`             | `` `code` ``                                 |
| `CodeBlock`        | ` ```lang\ncode\n``` `                       |
| `UnorderedList`    | `- item\n- item`                             |
| `OrderedList`      | `1. item\n2. item`                           |
| `Blockquote`       | `> line\n> line`                             |
| `Table`            | GFM pipe table with alignment                |
| `HorizontalRule`   | `---`                                        |
| `Strikethrough`    | `~~text~~`                                   |
| `DefinitionList`   | `**term:** definition`                       |
| `Media`            | `[Media: title](src)`                        |

**YAML frontmatter generation.** When `include_frontmatter` is true, the converter
prepends a `---` delimited YAML block containing:

```yaml
---
url: https://example.com/page
title: Page Title
description: A description of the page
author: Author Name
published: 2024-01-15
language: en
canonical: https://example.com/page
---
```

Values containing YAML-special characters (`:`, `#`, `'`, `"`, `\n`, leading
whitespace, `{`, `[`, `*`, `&`, `!`, `%`, `@`, `` ` ``) are automatically
double-quoted and escaped.

**GFM table formatting.** Tables are rendered with aligned pipe-delimited columns:

```markdown
| Name  | Age |
|-------|-----|
| Alice | 30  |
| Bob   | 25  |
```

Column widths are computed from the maximum cell width in each column (minimum 3 for
the separator dashes).

**Post-processing pipeline:**

1. **HTML tag stripping** -- Removes any residual `<tags>` that survived parsing.
2. **Blank line collapsing** -- Collapses runs of blank lines to at most
   `max_blank_lines` (default: 2).
3. **Trailing whitespace trimming** -- Strips trailing spaces from every line.
4. **Final newline** -- Ensures the file ends with exactly one newline.

---

### Extractor (`extractor.rs`)

The extractor scans HTML documents for every URL reference and classifies each one.

**HTML element types scanned (12+ sources):**

| # | Element / Attribute                      | Link Type  |
|---|------------------------------------------|------------|
| 1 | `<a href>`                               | Page       |
| 2 | `<link href>` (stylesheet)               | Resource   |
| 3 | `<link href>` (canonical, alternate)     | Page       |
| 4 | `<link href>` (RSS/Atom `type=`)         | Feed       |
| 5 | `<script src>`                           | Resource   |
| 6 | `<iframe src>`                           | Page       |
| 7 | `<form action>`                          | Page       |
| 8 | `<area href>` (image maps)               | Page       |
| 9 | `<img src>` and `<img srcset>`           | Resource   |
|10 | `<video src>`, `<video poster>`          | Resource   |
|11 | `<audio src>`, `<source src>`            | Resource   |
|12 | `<meta http-equiv="refresh" content>`    | Page       |
|13 | `<meta property="og:url|og:image|...">`  | Page/Resource |

**Link classification.** After extracting a raw URL and its link type, each URL is
classified into one of six buckets in `ExtractedLinks`:

```rust
pub struct ExtractedLinks {
    pub internal:   Vec<String>,   // same domain
    pub external:   Vec<String>,   // different domain
    pub subdomains: Vec<String>,   // same root, different subdomain
    pub resources:  Vec<String>,   // CSS, JS, images, fonts, media
    pub feeds:      Vec<String>,   // RSS / Atom
    pub sitemaps:   Vec<String>,   // sitemap XML
}
```

Classification logic:

- **Feed** and **Sitemap** link types go directly to their respective buckets.
- **Resource** link types go to `resources`.
- **Page** link types are compared against the base URL:
  - Same host -> `internal`
  - Same root domain, different subdomain -> `subdomains`
  - Different root domain -> `external`

**srcset parsing.** The extractor handles the `srcset` attribute on `<img>` elements,
which contains comma-separated entries like `image-480w.jpg 480w, image-800w.jpg 800w`.
Each URL is extracted and resolved against the base URL.

**Meta refresh extraction.** The extractor parses `<meta http-equiv="refresh"
content="0;url=https://...">` tags, extracting the redirect target URL.

**XML link extraction.** A separate `extract_links_from_xml()` function handles
RSS/Atom/Sitemap XML documents, extracting URLs from `<link>`, `<loc>`, and similar
elements. It detects the document context (sitemap vs feed) from root elements like
`<urlset>`, `<sitemapindex>`, `<feed>`, `<channel>`, and `<rss>`.

---

### Anti-Detection (`anti_detect.rs`)

The anti-detection module helps the crawler blend in with normal browser traffic.

**User agent pool design.** The `build_ua_pool()` function returns a vector of 57
unique, realistic user agent strings spanning:

| Browser | Versions   | Platforms                      | Count |
|---------|------------|--------------------------------|-------|
| Chrome  | 120 - 128  | Windows, macOS, Linux          | 21    |
| Firefox | 120 - 127  | Windows, macOS, Linux (Ubuntu) | 13    |
| Safari  | 17.0 - 17.5| macOS 13.x and 14.x            | 8     |
| Edge    | 120 - 125  | Windows, macOS, Linux          | 9     |

All UAs are modern (Chrome 120+, Firefox 120+, Safari 17+, Edge 120+), unique, and
include realistic platform tokens.

**Browser header simulation.** The `browser_headers()` method returns a complete set
of headers that real browsers send:

```
User-Agent:                  (randomly selected from pool)
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

**Delay strategies.** The `get_delay()` method computes inter-request delays:

| Strategy   | Behavior                                                        |
|------------|-----------------------------------------------------------------|
| `Fixed`    | Constant delay (e.g., 500ms between every request)             |
| `Random`   | Uniform random delay in `[min, max]` range                     |
| `Adaptive` | Starts at `initial` delay; intended to scale with server load  |
| `None`     | Zero delay (fastest, but most detectable)                      |

---

## Concurrency Model

Rehyke's concurrency model is built on `tokio`'s async runtime with careful use of
lock-free data structures for shared state.

**Async runtime.** All I/O (HTTP requests, file writes) runs on the tokio
multi-threaded runtime. The `Fetcher::fetch()` method is fully async, and the
scheduler is designed to be called from multiple concurrent tasks.

**Lock-free shared state:**

| Data Structure              | Type                   | Purpose                        |
|-----------------------------|------------------------|--------------------------------|
| `visited`                   | `DashSet<String>`      | URL deduplication              |
| `in_progress`               | `DashSet<String>`      | Tracking active fetches        |
| `domain_delays`             | `DashMap<String, Instant>` | Per-domain rate limiting   |
| `stats.total_discovered`    | `AtomicUsize`          | Counter: discovered URLs       |
| `stats.total_crawled`       | `AtomicUsize`          | Counter: completed fetches     |
| `stats.total_errors`        | `AtomicUsize`          | Counter: failed fetches        |
| `stats.total_skipped`       | `AtomicUsize`          | Counter: filtered URLs         |

DashMap and DashSet (from the `dashmap` crate, version 6) use sharded locking
internally, providing near-lock-free performance under high concurrency. Multiple
worker tasks can read and write the visited set simultaneously without blocking.

**Mutex-protected frontier.** The priority queue (`BinaryHeap`) is wrapped in a
`std::sync::Mutex` because `BinaryHeap` does not support concurrent access.
The critical section is kept minimal: `next_task()` pops one item and immediately
releases the lock. Insertion via `add_urls()` briefly locks to push new tasks.

**AtomicUsize for stats.** All crawl statistics use `AtomicUsize` with `Relaxed`
ordering. Exact consistency is not required for stats -- approximate counts are
sufficient for progress reporting and limit enforcement. The `snapshot()` method
reads all counters into an immutable struct for display.

**Per-domain rate limiting with Instant timestamps.** The `domain_delays` DashMap
stores the wall-clock `Instant` when each domain was last accessed. Before dequeuing
a task, `next_task()` checks `last_access.elapsed() < domain_delay`. If the domain
is still in cooldown, the task is deferred and the next candidate is tried. This
approach avoids global serialization -- domains that are not rate-limited proceed
immediately.

---

## Data Flow

Here is a step-by-step walkthrough of what happens when a URL is crawled:

### Step 1: URL enters the scheduler

```rust
scheduler.add_seed(Url::parse("https://example.com")?);
```

The seed URL is normalized (fragment stripped, www removed, lowercased), inserted
into the `visited` DashSet, and pushed onto the priority queue with
`Priority::Critical`. The `total_discovered` counter is incremented.

### Step 2: Fetcher downloads content

```rust
let task = scheduler.next_task().unwrap();
let fetch_result = fetcher.fetch_with_retry(&task.url).await?;
```

The fetcher sends an HTTP GET request with the configured user agent, custom headers,
and proxy. If the request fails with a retryable error (timeout, 429, 5xx), it waits
with exponential backoff and retries up to `max_retries` times. On success, it returns
a `FetchResult` containing the body, headers, status code, final URL (after
redirects), and elapsed time.

### Step 3: Content type detected

```rust
let content_type = detect_content_type(&result.headers, &result.url, &result.body);
```

The three-stage detection pipeline (header -> URL extension -> body sniffing)
determines whether the content is HTML, RSS, Atom, JSON, XML, or another format.

### Step 4: Parser extracts structured data

```rust
let parsed = parser::parse(&body, &content_type, &parse_config)?;
```

The parser dispatches to the appropriate format handler. For HTML, it builds a DOM
with `scraper`, extracts metadata from `<meta>` tags, cleans unwanted elements
(scripts, styles, nav, footer, ads), and walks the remaining DOM to produce a vector
of `ContentNode` values.

### Step 5: Converter produces Markdown

```rust
let markdown = converter::to_markdown_with_url(&parsed, url, &converter_config);
```

Each `ContentNode` is rendered to its Markdown representation. YAML frontmatter is
prepended with the URL, title, description, and other metadata. Post-processing
strips residual HTML tags, collapses blank lines, and trims trailing whitespace.

### Step 6: Links extracted and fed back to scheduler

```rust
let links = extractor::extract_links(&html, &final_url);
scheduler.add_urls(internal_urls, depth + 1, TaskSource::InternalLink);
```

The extractor scans the HTML for all link sources (anchors, link tags, images,
scripts, meta refreshes, etc.) and classifies them. Internal page links are fed
back to the scheduler at `depth + 1`. The scheduler applies its mode-based
filtering (Lite drops all, Full drops external, Deep keeps all), deduplication,
and depth/page limits before enqueuing.

### Step 7: Output handler writes result

```rust
let result = CrawlResult { url, title, markdown, metadata, links, ... };
output_handler.handle_result(result)?;
```

The `CrawlResult` is dispatched to the configured output mode:

- **Memory** -- stored in an in-memory vector, returned at the end.
- **Files (Flat)** -- written to `{output_dir}/{slug}.md`.
- **Files (Mirror)** -- written to `{output_dir}/{host}/{path}/index.md`.
- **SingleFile** -- appended to a single file with `---` separators.

---

## Error Handling Strategy

Rehyke uses a typed error enum (`RehykeError`) with `thiserror` for ergonomic error
reporting. Each error variant carries context (URL, domain, status code, or message)
to aid debugging.

### Error classification table

| Error Variant      | Cause                                      | Retryable? | Recovery                           |
|--------------------|--------------------------------------------|------------|------------------------------------|
| `HttpError{403}`   | Forbidden / blocked by server              | No         | Rotate UA/proxy, skip URL          |
| `HttpError{404}`   | Page not found                             | No         | Mark failed, skip URL              |
| `HttpError{429}`   | Rate limited                               | Yes        | Exponential backoff, Retry-After   |
| `HttpError{5xx}`   | Server error                               | Yes        | Exponential backoff                |
| `Timeout`          | Request exceeded timeout                   | Yes        | Retry with backoff                 |
| `DnsError`         | DNS resolution failed                      | Yes        | Retry (may be transient)           |
| `TlsError`         | SSL/TLS handshake failure                  | No         | Log and skip                       |
| `RenderError`      | JS rendering failed                        | Depends    | Fall back to static fetch          |
| `BrowserError`     | Headless browser launch failed             | No         | Disable JS rendering               |
| `ParseError`       | Content could not be parsed                | No         | Return raw text as fallback        |
| `ProxyError`       | Proxy connection or auth failure           | No         | Try next proxy or direct           |
| `RateLimited`      | Domain-level rate limit hit                | Yes        | Wait and retry (scheduler-level)   |
| `MaxPagesReached`  | Configured page limit exceeded             | No         | Stop crawl                         |
| `IoError`          | File system error                          | No         | Report and fail                    |
| `ConfigError`      | Invalid configuration                      | No         | Fail fast at startup               |
| `RequestError`     | Generic reqwest error                      | Depends    | Classified by `is_network_error()` |
| `UrlParseError`    | Malformed URL                              | No         | Skip URL                           |

### Recovery behavior

The `classify_reqwest_error()` function maps low-level reqwest errors to the most
specific `RehykeError` variant by inspecting error properties:

- `is_timeout()` -> `Timeout`
- `is_connect()` with DNS-related message -> `DnsError`
- Message contains "tls"/"ssl"/"certificate" -> `TlsError`
- Has a status code -> `HttpError`
- Fallback -> `ConfigError` with descriptive message

The `is_network_error()` function identifies errors that are safe to retry:
`Timeout`, `DnsError`, `RequestError`, and the `ConfigError` fallback (which
wraps unknown reqwest errors).

### Fallback chain for rendering

```
JavaScript render attempt
    |
    +--> Success: use rendered HTML
    |
    +--> Failure (RenderError): fall back to static fetch
              |
              +--> Success: use static HTML
              |
              +--> Failure: report error, skip page
```

When `enable_js` is true but rendering fails, the system can fall back to a static
HTTP fetch. The `RenderMethod` enum in `CrawlResult` records which path was taken.

---

## Configuration Reference

The `CrawlConfig` struct is the single source of truth for all crawl parameters.
It can be constructed via `CrawlConfig::default()`, the `CrawlConfigBuilder` fluent
API, or deserialized from JSON/YAML.

```rust
let config = CrawlConfigBuilder::new()
    .mode(ScanMode::Full)
    .max_depth(3)
    .concurrency(20)
    .enable_js(true)
    .delay_strategy(DelayStrategy::Random {
        min: Duration::from_millis(500),
        max: Duration::from_secs(2),
    })
    .exclude_patterns(vec![r"\.pdf$".into()])
    .header("X-Custom", "value")
    .cookie("session", "abc123")
    .build();
```

Duration fields support both integer milliseconds and human-readable strings
(`"30s"`, `"500ms"`, `"1m30s"`, `"2h"`) when deserializing from JSON.

---

## Dependency Overview

| Dependency           | Version | Purpose                                     |
|----------------------|---------|---------------------------------------------|
| `tokio`              | 1.x     | Async runtime (full features)               |
| `reqwest`            | 0.12    | HTTP client (gzip, brotli, zstd, cookies, socks, rustls) |
| `scraper`            | 0.20    | HTML parsing and CSS selector queries       |
| `quick-xml`          | 0.36    | Fast XML/RSS/Atom/Sitemap parsing           |
| `url`                | 2.x     | URL parsing and normalization               |
| `dashmap`            | 6.x     | Lock-free concurrent HashMap/HashSet        |
| `serde` / `serde_json` | 1.x  | Serialization / deserialization             |
| `regex`              | 1.x     | URL include/exclude pattern matching        |
| `chrono`             | 0.4     | Timestamps on crawl results                 |
| `tracing`            | 0.1     | Structured logging                          |
| `thiserror`          | 2.x     | Ergonomic error type derivation             |
| `rand`               | 0.8     | User agent selection, random delays         |
| `pyo3`               | 0.22    | Python FFI bindings (rehyke-python crate)   |
| `clap`               | 4.x     | CLI argument parsing (rehyke-cli crate)     |
