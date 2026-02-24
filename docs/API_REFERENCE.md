# Rehyke API Reference

Comprehensive reference for the Rehyke web crawler library. Rehyke is a high-performance
web crawler written in Rust that converts web pages to clean Markdown, with Python bindings
via PyO3.

---

## Table of Contents

1. [Core Types](#1-core-types)
   - [Rehyke](#rehyke)
   - [CrawlConfig](#crawlconfig)
   - [CrawlConfigBuilder](#crawlconfigbuilder)
   - [ScanMode](#scanmode)
2. [Output Types](#2-output-types)
   - [CrawlResult](#crawlresult)
   - [PageMetadata](#pagemetadata)
   - [ExtractedLinks](#extractedlinks)
   - [RenderMethod](#rendermethod)
   - [OutputMode](#outputmode)
   - [FileStructure](#filestructure)
3. [Configuration Enums](#3-configuration-enums)
   - [DelayStrategy](#delaystrategy)
   - [WaitStrategy](#waitstrategy)
   - [ProxyStrategy](#proxystrategy)
   - [ProxyType](#proxytype)
   - [ProxyConfig](#proxyconfig)
   - [ProxyAuth](#proxyauth)
   - [RetryConfig](#retryconfig)
4. [Content Processing](#4-content-processing)
   - [ContentType](#contenttype)
   - [Parser Functions](#parser-functions)
   - [ParseConfig](#parseconfig)
   - [ContentNode](#contentnode)
   - [Converter Functions](#converter-functions)
   - [ConverterConfig](#converterconfig)
5. [URL Utilities](#5-url-utilities)
   - [normalize_url](#normalize_url)
   - [is_same_domain](#is_same_domain)
   - [is_subdomain](#is_subdomain)
   - [root_domain](#root_domain)
   - [resolve_url](#resolve_url)
   - [sanitize_url](#sanitize_url)
   - [url_to_filename](#url_to_filename)
   - [url_to_slug](#url_to_slug)
6. [Network and Anti-Detection](#6-network-and-anti-detection)
   - [Fetcher](#fetcher)
   - [FetchResult](#fetchresult)
   - [AntiDetect](#antidetect)
   - [ProxyPool](#proxypool)
   - [Renderer](#renderer)
   - [RendererConfig](#rendererconfig)
   - [RenderResult](#renderresult)
7. [Crawl Management](#7-crawl-management)
   - [Scheduler](#scheduler)
   - [Priority](#priority)
   - [CrawlTask](#crawltask)
   - [TaskSource](#tasksource)
   - [CrawlStats / CrawlStatsSnapshot](#crawlstats--crawlstatssnapshot)
8. [Auxiliary Parsers](#8-auxiliary-parsers)
   - [RobotsTxt](#robotstxt)
   - [Sitemap](#sitemap)
   - [SitemapEntry](#sitemapentry)
9. [Error Handling](#9-error-handling)
   - [RehykeError](#rehykeerror)
   - [Error Recovery Strategy](#error-recovery-strategy)
10. [Python API](#10-python-api)
    - [Module-level crawl()](#module-level-crawl-function)
    - [Rehyke class](#python-rehyke-class)
    - [CrawlConfig class](#python-crawlconfig-class)
    - [CrawlResult class](#python-crawlresult-class)
    - [ScanMode enum](#python-scanmode-enum)

---

## 1. Core Types

### Rehyke

The main entry point for the crawler. Provides both a simple one-shot API and a
configurable multi-page crawl API.

```rust
pub struct Rehyke {
    config: CrawlConfig,
}
```

#### `Rehyke::new`

```rust
pub fn new(config: CrawlConfig) -> Self
```

Create a new crawler instance with the given configuration.

| Parameter | Type | Description |
|-----------|------|-------------|
| `config` | `CrawlConfig` | Complete configuration for the crawl job |

**Returns:** `Rehyke`

```rust
use rehyke_core::config::{CrawlConfig, CrawlConfigBuilder, ScanMode};

let config = CrawlConfigBuilder::new()
    .mode(ScanMode::Full)
    .concurrency(20)
    .build();
let crawler = Rehyke::new(config);
```

#### `Rehyke::crawl` (static async)

```rust
pub async fn crawl(url: &str, mode: ScanMode) -> Result<Vec<CrawlResult>>
```

Simple one-shot crawl API. Creates a default `CrawlConfig` with the given `ScanMode`,
fetches the URL, parses the content, converts it to Markdown, and returns the results.

| Parameter | Type | Description |
|-----------|------|-------------|
| `url` | `&str` | The seed URL to crawl |
| `mode` | `ScanMode` | Crawl preset (Lite, Full, or Deep) |

**Returns:** `Result<Vec<CrawlResult>>`

```rust
let results = Rehyke::crawl("https://example.com", ScanMode::Full).await?;
for page in &results {
    println!("{}: {}", page.title, page.url);
}
```

#### `Rehyke::run` (async)

```rust
pub async fn run(&self, url: &str) -> Result<Vec<CrawlResult>>
```

Full crawl API using the configured options. Executes the following pipeline:

1. Create a `Fetcher` from the config
2. Fetch the URL (with retry logic)
3. Parse the HTML/XML/JSON content
4. Convert to Markdown
5. Extract links
6. Return a `CrawlResult`

| Parameter | Type | Description |
|-----------|------|-------------|
| `url` | `&str` | The seed URL to crawl |

**Returns:** `Result<Vec<CrawlResult>>`

```rust
let config = CrawlConfigBuilder::new()
    .mode(ScanMode::Deep)
    .max_depth(3)
    .enable_js(true)
    .build();
let crawler = Rehyke::new(config);
let results = crawler.run("https://docs.example.com").await?;
```

---

### CrawlConfig

Complete configuration for a crawl job. Use `CrawlConfig::default()` for sensible defaults
or `CrawlConfigBuilder` for a fluent builder API.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlConfig { /* fields */ }
```

#### Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `mode` | `ScanMode` | `ScanMode::Full` | High-level scan preset |
| `max_depth` | `usize` | `5` (mode-dependent) | Maximum link-follow depth from the seed URL |
| `max_pages` | `usize` | `1_000` (mode-dependent) | Maximum number of pages to crawl |
| `concurrency` | `usize` | `10` (mode-dependent) | Number of concurrent requests |
| `enable_js` | `bool` | `false` | Whether to run pages through a headless browser |
| `js_wait_strategy` | `WaitStrategy` | `WaitStrategy::Auto` | How to wait for JS-rendered content to settle |
| `output` | `OutputMode` | `OutputMode::Memory` | Where to store crawl results |
| `user_agent` | `String` | `"rehyke/{version}"` | User-Agent header sent with every request |
| `proxies` | `Vec<ProxyConfig>` | `[]` | List of proxy endpoints |
| `proxy_strategy` | `ProxyStrategy` | `ProxyStrategy::RoundRobin` | Strategy for selecting among proxies |
| `delay_strategy` | `DelayStrategy` | `DelayStrategy::None` | Inter-request delay strategy |
| `exclude_patterns` | `Vec<String>` | `[]` | Regex patterns; URLs matching any pattern are skipped |
| `include_patterns` | `Vec<String>` | `[]` | Regex patterns; only URLs matching at least one are crawled |
| `respect_robots_txt` | `bool` | `true` | Whether to honour robots.txt directives |
| `extract_metadata` | `bool` | `true` | Extract structured metadata (title, description, etc.) |
| `clean_navigation` | `bool` | `true` | Remove navigation elements from extracted content |
| `clean_footers` | `bool` | `true` | Remove footer elements from extracted content |
| `clean_ads` | `bool` | `true` | Remove advertisement elements from extracted content |
| `timeout` | `Duration` | `30s` | Per-request timeout |
| `retry_config` | `RetryConfig` | See RetryConfig defaults | Retry configuration for failed requests |
| `custom_headers` | `HashMap<String, String>` | `{}` | Extra HTTP headers sent with every request |
| `cookies` | `HashMap<String, String>` | `{}` | Cookies injected into every request |
| `remove_www` | `bool` | `true` | Strip the `www.` prefix when comparing/deduplicating URLs |

Serialization note: Duration fields are serialized as milliseconds (integer) and can be
deserialized from integers or human-readable strings such as `"30s"`, `"500ms"`, `"2m"`,
`"1m30s"`, or `"2h"`.

---

### CrawlConfigBuilder

Fluent builder for `CrawlConfig`. Start from defaults and override only what you need.

```rust
#[derive(Debug, Clone)]
pub struct CrawlConfigBuilder { /* ... */ }
```

#### `CrawlConfigBuilder::new`

```rust
pub fn new() -> Self
```

Create a builder pre-filled with `CrawlConfig::default()` (Full mode).

#### Builder Methods

All builder methods consume `self` and return `Self` for chaining.

| Method | Parameter Type | Description |
|--------|---------------|-------------|
| `mode(mode)` | `ScanMode` | Set scan mode and update depth/pages/concurrency to mode defaults |
| `max_depth(n)` | `usize` | Override maximum crawl depth |
| `max_pages(n)` | `usize` | Override maximum page count |
| `concurrency(n)` | `usize` | Override concurrent request count |
| `enable_js(flag)` | `bool` | Enable/disable JavaScript rendering |
| `js_wait_strategy(s)` | `WaitStrategy` | Set JS page-ready strategy |
| `output(mode)` | `OutputMode` | Set output mode (memory, files, single file) |
| `user_agent(ua)` | `impl Into<String>` | Set User-Agent header |
| `proxies(list)` | `Vec<ProxyConfig>` | Set proxy list |
| `proxy_strategy(s)` | `ProxyStrategy` | Set proxy selection strategy |
| `delay_strategy(s)` | `DelayStrategy` | Set inter-request delay strategy |
| `exclude_patterns(p)` | `Vec<String>` | Set URL exclusion patterns (regex) |
| `include_patterns(p)` | `Vec<String>` | Set URL inclusion patterns (regex) |
| `respect_robots_txt(flag)` | `bool` | Enable/disable robots.txt |
| `extract_metadata(flag)` | `bool` | Enable/disable metadata extraction |
| `clean_navigation(flag)` | `bool` | Enable/disable nav element removal |
| `clean_footers(flag)` | `bool` | Enable/disable footer removal |
| `clean_ads(flag)` | `bool` | Enable/disable ad removal |
| `timeout(d)` | `Duration` | Set per-request timeout |
| `retry_config(r)` | `RetryConfig` | Set retry configuration |
| `custom_headers(h)` | `HashMap<String, String>` | Set all custom headers |
| `header(key, value)` | `impl Into<String>, impl Into<String>` | Insert a single custom header |
| `cookies(c)` | `HashMap<String, String>` | Set all cookies |
| `cookie(name, value)` | `impl Into<String>, impl Into<String>` | Insert a single cookie |
| `remove_www(flag)` | `bool` | Enable/disable www. stripping |
| `build()` | -- | Consume the builder, return `CrawlConfig` |

```rust
use rehyke_core::config::*;
use std::time::Duration;

let config = CrawlConfigBuilder::new()
    .mode(ScanMode::Lite)
    .max_depth(3)
    .concurrency(20)
    .enable_js(true)
    .user_agent("MyBot/1.0")
    .timeout(Duration::from_secs(60))
    .header("X-Custom", "value")
    .cookie("session", "abc123")
    .exclude_patterns(vec![r"\.pdf$".to_string()])
    .remove_www(false)
    .build();
```

---

### ScanMode

High-level crawl presets that set sensible defaults for depth, page count, and concurrency.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScanMode {
    Lite,
    Full,  // default
    Deep,
}
```

#### Mode Comparison

| Property | Lite | Full | Deep |
|----------|------|------|------|
| **Max Depth** | 2 | 5 | 50 |
| **Max Pages** | 100 | 1,000 | 50,000 |
| **Concurrency** | 5 | 10 | 25 |
| **Link Following** | None (single page) | Same-domain only | Internal + external |
| **Use Case** | Quick single-page extraction | Balanced site crawl | Exhaustive deep crawl |

#### Methods

```rust
pub fn default_max_depth(self) -> usize
pub fn default_max_pages(self) -> usize
pub fn default_concurrency(self) -> usize
```

---

## 2. Output Types

### CrawlResult

Result of crawling a single page. Serializable with serde.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub render_method: RenderMethod,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `url` | `String` | The URL that was crawled (after redirects) |
| `title` | `String` | Page title extracted from the document |
| `markdown` | `String` | Markdown representation of the page content |
| `metadata` | `PageMetadata` | Structured metadata parsed from the page |
| `links` | `ExtractedLinks` | Links discovered on the page, classified by type |
| `crawled_at` | `DateTime<Utc>` | Timestamp when the page was crawled |
| `status_code` | `u16` | HTTP status code of the response |
| `content_type` | `String` | Content-Type header value from the response |
| `depth` | `u32` | Crawl depth at which this page was discovered (0 = seed) |
| `render_method` | `RenderMethod` | How the page was rendered (Static or JavaScript) |

---

### PageMetadata

Metadata extracted from a page's `<meta>` tags, `<title>`, and `<link>` elements.

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PageMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub published_date: Option<String>,
    pub language: Option<String>,
    pub canonical_url: Option<String>,
    pub og_image: Option<String>,
    pub keywords: Vec<String>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `title` | `Option<String>` | Page title from `<title>` tag |
| `description` | `Option<String>` | From `<meta name="description">` or `og:description` |
| `author` | `Option<String>` | From `<meta name="author">` |
| `published_date` | `Option<String>` | From `article:published_time` or similar |
| `language` | `Option<String>` | From `<html lang="...">` |
| `canonical_url` | `Option<String>` | From `<link rel="canonical">` |
| `og_image` | `Option<String>` | From `og:image` meta tag |
| `keywords` | `Vec<String>` | From `<meta name="keywords">` |

---

### ExtractedLinks

All links extracted from a document, classified by type.

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtractedLinks {
    pub internal: Vec<String>,
    pub external: Vec<String>,
    pub subdomains: Vec<String>,
    pub resources: Vec<String>,
    pub feeds: Vec<String>,
    pub sitemaps: Vec<String>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `internal` | `Vec<String>` | Links pointing to pages on the same domain |
| `external` | `Vec<String>` | Links pointing to pages on a different domain |
| `subdomains` | `Vec<String>` | Links pointing to a different subdomain of the same root domain |
| `resources` | `Vec<String>` | Resource URLs: CSS, JS, images, fonts, media |
| `feeds` | `Vec<String>` | RSS / Atom feed URLs |
| `sitemaps` | `Vec<String>` | Sitemap XML references |

The `extract_links` function inspects 12 distinct HTML element types including `<a>`, `<link>`,
`<script>`, `<iframe>`, `<form>`, `<area>`, `<img>`, `<video>`, `<audio>`, `<source>`, and
`<meta>` tags.

```rust
pub fn extract_links(html: &Html, base_url: &Url) -> ExtractedLinks
```

---

### RenderMethod

How a page was rendered.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RenderMethod {
    Static,
    JavaScript,
}
```

| Variant | Description |
|---------|-------------|
| `Static` | Page was fetched as static HTML without JavaScript execution |
| `JavaScript` | Page was rendered via a headless browser with JavaScript execution |

---

### OutputMode

Where and how crawl results are stored.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputMode {
    Memory,
    Files { output_dir: PathBuf, structure: FileStructure },
    SingleFile { output_path: PathBuf },
}
```

| Variant | Fields | Description |
|---------|--------|-------------|
| `Memory` | -- | Keep everything in memory and return it at the end (default) |
| `Files` | `output_dir: PathBuf`, `structure: FileStructure` | Write individual `.md` files to a directory |
| `SingleFile` | `output_path: PathBuf` | Write all output into a single file with `---` separators |

---

### FileStructure

How files are laid out on disk when using `OutputMode::Files`.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileStructure {
    Flat,
    Mirror,
}
```

| Variant | Description | Example Path |
|---------|-------------|--------------|
| `Flat` | All files in a single directory with slugified names (default) | `output/example-com-blog-post.md` |
| `Mirror` | Mirror the site's URL path hierarchy | `output/example.com/blog/post/index.md` |

---

## 3. Configuration Enums

### DelayStrategy

How to introduce delays between requests to the same domain.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DelayStrategy {
    Fixed { delay: Duration },
    Random { min: Duration, max: Duration },
    Adaptive { initial: Duration },
    None,
}
```

| Variant | Fields | Description |
|---------|--------|-------------|
| `Fixed` | `delay: Duration` | Constant delay between requests |
| `Random` | `min: Duration`, `max: Duration` | Random delay within [min, max] per request |
| `Adaptive` | `initial: Duration` | Adaptive delay that backs off when rate-limited; starts at `initial` |
| `None` | -- | No delay (default) |

```rust
use rehyke_core::config::DelayStrategy;
use std::time::Duration;

// Fixed 1-second delay
let fixed = DelayStrategy::Fixed { delay: Duration::from_secs(1) };

// Random delay between 500ms and 2s
let random = DelayStrategy::Random {
    min: Duration::from_millis(500),
    max: Duration::from_secs(2),
};
```

---

### WaitStrategy

Strategy used to decide when a JavaScript-rendered page is "ready".

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WaitStrategy {
    NetworkIdle,
    Selector { selector: String },
    Duration { duration: Duration },
    Auto,
}
```

| Variant | Fields | Description |
|---------|--------|-------------|
| `NetworkIdle` | -- | Wait until there are no pending network requests |
| `Selector` | `selector: String` | Wait until a specific CSS selector is present in the DOM |
| `Duration` | `duration: Duration` | Wait a fixed duration after page load |
| `Auto` | -- | Automatically determine the best strategy (default) |

```rust
// Wait until a specific element appears
let wait = WaitStrategy::Selector {
    selector: "div.content-loaded".to_string(),
};
```

---

### ProxyStrategy

Strategy for choosing among multiple proxies.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProxyStrategy {
    RoundRobin,
    Random,
    LeastUsed,
    FailoverOnly,
}
```

| Variant | Description |
|---------|-------------|
| `RoundRobin` | Cycle through proxies in order (default) |
| `Random` | Pick a random proxy for each request |
| `LeastUsed` | Use the proxy with the fewest in-flight requests |
| `FailoverOnly` | Only switch to another proxy when the current one fails |

---

### ProxyType

Protocol used by a proxy.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProxyType {
    Http,    // default
    Https,
    Socks5,
}
```

---

### ProxyConfig

Configuration for a single proxy endpoint.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub url: String,
    pub proxy_type: ProxyType,
    pub auth: Option<ProxyAuth>,
    pub region: Option<String>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `url` | `String` | The proxy URL (e.g., `http://proxy.example.com:8080`) |
| `proxy_type` | `ProxyType` | Protocol type of the proxy (default: `Http`) |
| `auth` | `Option<ProxyAuth>` | Optional authentication credentials |
| `region` | `Option<String>` | Optional region label (for geo-routing proxies) |

#### `ProxyConfig::to_reqwest_proxy`

```rust
pub fn to_reqwest_proxy(&self) -> Result<reqwest::Proxy, reqwest::Error>
```

Convert this proxy configuration into a `reqwest::Proxy` instance. Applies authentication
if credentials are present.

---

### ProxyAuth

Authentication credentials for a proxy.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyAuth {
    pub username: String,
    pub password: String,
}
```

---

### RetryConfig

Controls automatic retry behaviour for failed requests.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_retries` | `u32` | `3` | Maximum number of retry attempts |
| `initial_delay` | `Duration` | `500ms` | Delay before the first retry (doubles on each subsequent retry) |
| `max_delay` | `Duration` | `30s` | Upper bound on retry delay |

Backoff is exponential: `initial_delay * 2^attempt`, capped at `max_delay`. A `Retry-After`
header on HTTP 429 responses is respected when present.

---

## 4. Content Processing

### ContentType

Detected content type of a fetched response. Determined from (in priority order):
the `Content-Type` header, the URL file extension, and body sniffing.

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ContentType {
    Html,
    Xhtml,
    Xml,
    Rss,
    Atom,
    Json,
    JsonLd,
    Svg,
    PlainText,
    Sitemap,
    Other(String),
}
```

| Variant | MIME Type(s) | Description |
|---------|-------------|-------------|
| `Html` | `text/html` | Standard HTML document |
| `Xhtml` | `application/xhtml+xml` | XHTML document |
| `Xml` | `text/xml`, `application/xml` | Generic XML document |
| `Rss` | `application/rss+xml` | RSS 2.0 feed |
| `Atom` | `application/atom+xml` | Atom feed |
| `Json` | `application/json`, `text/json` | JSON data |
| `JsonLd` | `application/ld+json` | JSON-LD structured data |
| `Svg` | `image/svg+xml` | SVG image |
| `PlainText` | `text/plain` | Plain text |
| `Sitemap` | (detected from `<urlset>` / `<sitemapindex>` root) | Sitemap XML |
| `Other(String)` | Any other MIME | Unknown content type (stores the MIME essence) |

#### `detect_content_type`

```rust
pub fn detect_content_type(headers: &HeaderMap, url: &Url, body: &str) -> ContentType
```

Detect the `ContentType` from response headers, URL extension, and body content, in that
order of priority. Falls back to `ContentType::Html` when no signal is available.

---

### Parser Functions

#### `parse`

```rust
pub fn parse(body: &str, content_type: &ContentType, config: &ParseConfig) -> Result<ParsedDocument>
```

Top-level dispatcher. Parse raw content into a `ParsedDocument` based on its content type.
Dispatches to the appropriate format-specific parser.

| Parameter | Type | Description |
|-----------|------|-------------|
| `body` | `&str` | Raw response body |
| `content_type` | `&ContentType` | Detected content type |
| `config` | `&ParseConfig` | Parser configuration |

**Returns:** `Result<ParsedDocument>`

#### `parse_html`

```rust
pub fn parse_html(html: &str, config: &ParseConfig) -> Result<ParsedDocument>
```

Parse an HTML document. Uses the `scraper` crate to build a DOM, extracts metadata from
`<meta>` tags and `<link>` elements, cleans unwanted elements according to `config`, then
walks the remaining DOM tree to produce `ContentNode`s.

#### `parse_rss`

```rust
pub fn parse_rss(body: &str) -> Result<ParsedDocument>
```

Parse an RSS 2.0 feed into a `ParsedDocument`. Extracts feed metadata and individual items.

#### `parse_atom`

```rust
pub fn parse_atom(body: &str) -> Result<ParsedDocument>
```

Parse an Atom feed into a `ParsedDocument`. Extracts feed metadata and individual entries.

#### `parse_xml`

```rust
pub fn parse_xml(body: &str) -> Result<ParsedDocument>
```

Parse a generic XML document into a `ParsedDocument`. Produces raw text nodes.

#### `parse_json`

```rust
pub fn parse_json(body: &str) -> Result<ParsedDocument>
```

Parse JSON or JSON-LD content into a `ParsedDocument`. Pretty-prints the JSON into a
code block.

---

### ParseConfig

Configuration for the parser. Controls which parts of a document are cleaned before
content extraction.

```rust
#[derive(Debug, Clone)]
pub struct ParseConfig {
    pub clean_navigation: bool,
    pub clean_footers: bool,
    pub clean_ads: bool,
    pub clean_comments: bool,
    pub extract_metadata: bool,
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `clean_navigation` | `bool` | `true` | Remove `<nav>` elements |
| `clean_footers` | `bool` | `true` | Remove `<footer>` elements |
| `clean_ads` | `bool` | `true` | Remove elements whose class or id matches common ad patterns |
| `clean_comments` | `bool` | `true` | Remove elements that look like comment sections |
| `extract_metadata` | `bool` | `true` | Whether to extract `<meta>` tag metadata |

---

### ContentNode

A parsed content node for conversion. Each variant represents a semantic element that can
be rendered to Markdown or another output format by the converter module.

```rust
#[derive(Debug, Clone)]
pub enum ContentNode {
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

| Variant | Fields | Markdown Output |
|---------|--------|-----------------|
| `Heading` | `level: u8`, `text: String` | `# text` through `###### text` (clamped to 1-6) |
| `Paragraph` | `String` | `text\n\n` |
| `Link` | `text: String`, `href: String` | `[text](href)` |
| `Image` | `alt: String`, `src: String` | `![alt](src)` |
| `Bold` | `String` | `**text**` |
| `Italic` | `String` | `*text*` |
| `Code` | `String` | `` `text` `` |
| `CodeBlock` | `language: Option<String>`, `code: String` | ` ```lang\ncode\n``` ` |
| `UnorderedList` | `Vec<String>` | `- item\n- item` |
| `OrderedList` | `Vec<String>` | `1. item\n2. item` |
| `Blockquote` | `String` | `> line\n> line` |
| `Table` | `headers: Vec<String>`, `rows: Vec<Vec<String>>` | GFM table with alignment |
| `HorizontalRule` | -- | `---` |
| `LineBreak` | -- | `\n` |
| `Strikethrough` | `String` | `~~text~~` |
| `DefinitionList` | `Vec<(String, String)>` | `**term:** definition` |
| `Media` | `media_type`, `title`, `src` | `[Media: title](src)` |
| `RawText` | `String` | Verbatim text |

---

### Converter Functions

#### `to_markdown`

```rust
pub fn to_markdown(doc: &ParsedDocument, config: &ConverterConfig) -> String
```

Convert a `ParsedDocument` into a Markdown string.

| Parameter | Type | Description |
|-----------|------|-------------|
| `doc` | `&ParsedDocument` | The parsed document to convert |
| `config` | `&ConverterConfig` | Output formatting options |

**Returns:** `String` -- the Markdown representation.

#### `to_markdown_with_url`

```rust
pub fn to_markdown_with_url(doc: &ParsedDocument, url: &str, config: &ConverterConfig) -> String
```

Convert a `ParsedDocument` into a Markdown string with an explicit URL for the YAML
frontmatter.

#### `escape_markdown`

```rust
pub fn escape_markdown(text: &str) -> String
```

Escape special Markdown characters in text content. Escapes: `\`, `*`, `_`, `[`, `]`,
`(`, `)`, `#`, `+`, `-`, `.`, `!`, `|`, `` ` ``.

```rust
let escaped = escape_markdown("Hello *world* and _underscores_");
// "Hello \\*world\\* and \\_underscores\\_"
```

#### `format_table`

```rust
pub fn format_table(headers: &[String], rows: &[Vec<String>]) -> String
```

Format a GFM (GitHub Flavored Markdown) table with proper column alignment and padding.

```rust
let headers = vec!["Name".to_string(), "Age".to_string()];
let rows = vec![
    vec!["Alice".to_string(), "30".to_string()],
    vec!["Bob".to_string(), "25".to_string()],
];
let table = format_table(&headers, &rows);
// | Name  | Age |
// |-------|-----|
// | Alice | 30  |
// | Bob   | 25  |
```

#### `collapse_blank_lines`

```rust
pub fn collapse_blank_lines(text: &str, max: usize) -> String
```

Collapse consecutive blank lines to at most `max` blank lines. A blank line is a line
containing only whitespace.

#### `strip_html_tags`

```rust
pub fn strip_html_tags(text: &str) -> String
```

Remove any remaining HTML tags from text. Strips tags like `<p>`, `</p>`, `<br/>`,
`<div class="foo">`, etc. Self-closing tags and tags with attributes are also removed.

---

### ConverterConfig

Configuration for Markdown output.

```rust
#[derive(Debug, Clone)]
pub struct ConverterConfig {
    pub include_frontmatter: bool,
    pub include_footer: bool,
    pub max_blank_lines: usize,
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `include_frontmatter` | `bool` | `true` | Include YAML frontmatter with metadata |
| `include_footer` | `bool` | `true` | Include footer with source attribution |
| `max_blank_lines` | `usize` | `2` | Maximum consecutive blank lines allowed |

---

## 5. URL Utilities

All URL utility functions are in the `rehyke_core::utils` module.

### `normalize_url`

```rust
pub fn normalize_url(url: &Url, remove_www: bool) -> String
```

Normalize a URL for deduplication purposes. Applied rules:

1. Lowercase scheme and host
2. Remove default ports (80 for http, 443 for https)
3. Remove fragment (`#`)
4. Remove trailing slash (except for root path `/`)
5. Sort query parameters alphabetically
6. Decode unnecessary percent-encoding
7. Optionally remove `www.` prefix

| Parameter | Type | Description |
|-----------|------|-------------|
| `url` | `&Url` | The URL to normalize |
| `remove_www` | `bool` | Whether to strip the `www.` prefix |

**Returns:** `String`

```rust
let url = Url::parse("HTTP://WWW.Example.COM:80/path/?b=2&a=1#frag").unwrap();
let normalized = normalize_url(&url, true);
// "http://example.com/path?a=1&b=2"
```

---

### `is_same_domain`

```rust
pub fn is_same_domain(url: &Url, base: &Url) -> bool
```

Check if a URL is internal (same domain) relative to a base URL. Compares host components
case-insensitively.

```rust
let a = Url::parse("https://example.com/page").unwrap();
let b = Url::parse("https://Example.COM/other").unwrap();
assert!(is_same_domain(&a, &b)); // true
```

---

### `is_subdomain`

```rust
pub fn is_subdomain(url: &Url, base: &Url) -> bool
```

Check if a URL is a subdomain of the base domain. Returns `true` when the URL's host ends
with `.<base_host>`. The base host itself is **not** considered a subdomain of itself.

```rust
let url = Url::parse("https://blog.example.com/").unwrap();
let base = Url::parse("https://example.com/").unwrap();
assert!(is_subdomain(&url, &base));  // true
assert!(!is_subdomain(&base, &base)); // false -- same host
```

---

### `root_domain`

```rust
pub fn root_domain(url: &Url) -> Option<String>
```

Extract the root domain from a URL (e.g., `"blog.example.com"` becomes `"example.com"`).
Uses a simple heuristic: take the last two labels of the host.

```rust
let url = Url::parse("https://a.b.c.example.com/page").unwrap();
assert_eq!(root_domain(&url), Some("example.com".to_string()));
```

---

### `resolve_url`

```rust
pub fn resolve_url(base: &Url, relative: &str) -> Option<Url>
```

Resolve a potentially relative URL against a base URL. Handles relative paths (`../page`),
absolute paths (`/page`), protocol-relative URLs (`//cdn.example.com/file`), and full URLs.
Returns `None` when parsing fails or the input is empty.

```rust
let base = Url::parse("https://example.com/dir/page").unwrap();
assert_eq!(
    resolve_url(&base, "../other").unwrap().as_str(),
    "https://example.com/other"
);
```

---

### `sanitize_url`

```rust
pub fn sanitize_url(raw: &str) -> Option<String>
```

Sanitize a URL string. Performs the following clean-up steps:

- Trim leading/trailing whitespace and control characters
- Collapse internal whitespace/newlines
- Prepend `https://` when no scheme is present
- Validate by attempting to parse

Returns `None` when the result is not a valid URL.

```rust
assert_eq!(sanitize_url("example.com/page"), Some("https://example.com/page".to_string()));
assert_eq!(sanitize_url("//cdn.example.com/file"), Some("https://cdn.example.com/file".to_string()));
assert_eq!(sanitize_url(""), None);
```

---

### `url_to_filename`

```rust
pub fn url_to_filename(url: &Url) -> String
```

Convert a URL path to a filesystem-safe filename. The result contains only ASCII
alphanumerics, hyphens, underscores, and dots. Query and fragment are excluded. Returns
`"index"` if the path would produce an empty filename.

```rust
let url = Url::parse("https://example.com/blog/post").unwrap();
assert_eq!(url_to_filename(&url), "example.com_blog_post");
```

---

### `url_to_slug`

```rust
pub fn url_to_slug(url: &Url) -> String
```

Generate a slug from a URL for use as a filename. Produces a human-readable, kebab-case
identifier. The scheme is omitted, the host and path are converted to lowercase, and
non-alphanumeric characters are replaced with hyphens.

```rust
let url = Url::parse("https://example.com/Blog/Post").unwrap();
assert_eq!(url_to_slug(&url), "example-com-blog-post");
```

---

## 6. Network and Anti-Detection

### Fetcher

HTTP fetcher with retry, rate limiting, and user agent support.

```rust
pub struct Fetcher {
    client: Client,
    retry_config: RetryConfig,
    user_agent: Option<String>,
    custom_headers: HeaderMap,
    timeout: Duration,
}
```

#### `Fetcher::new`

```rust
pub fn new(config: &CrawlConfig) -> Result<Self>
```

Build a new `Fetcher` from the given `CrawlConfig`. The underlying `reqwest::Client` is
configured with:

- HTTP/1.1 and HTTP/2 support (via ALPN negotiation)
- Gzip, Brotli, Deflate, and Zstd decompression
- A maximum of 10 redirect hops
- Connection/read/total timeouts drawn from the config
- A persistent cookie store
- Custom headers from the config
- Optional proxy (uses the first entry in `config.proxies`)
- TLS via rustls

#### `Fetcher::fetch`

```rust
pub async fn fetch(&self, url: &Url) -> Result<FetchResult>
```

Fetch a single URL without any retry logic. The caller is responsible for retries.

#### `Fetcher::fetch_with_retry`

```rust
pub async fn fetch_with_retry(&self, url: &Url) -> Result<FetchResult>
```

Fetch a URL with automatic retries on transient failures. Retries on:

- Network / IO errors
- HTTP 429, 500, 502, 503, 504

Backoff is exponential: `initial_delay * 2^attempt`, capped at `retry_config.max_delay`.
A `Retry-After` header on 429 responses is respected when present. HTTP 403 is **not**
retried.

---

### FetchResult

Result of fetching a single URL.

```rust
#[derive(Debug, Clone)]
pub struct FetchResult {
    pub url: Url,
    pub status: u16,
    pub headers: HeaderMap,
    pub body: String,
    pub content_type: ContentType,
    pub elapsed: Duration,
    pub final_url: Url,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `url` | `Url` | The originally requested URL |
| `status` | `u16` | HTTP status code returned by the server |
| `headers` | `HeaderMap` | Response headers |
| `body` | `String` | Response body decoded to a string |
| `content_type` | `ContentType` | Detected content type of the response |
| `elapsed` | `Duration` | Wall-clock time the request took |
| `final_url` | `Url` | The URL after following all redirects |

---

### AntiDetect

Manages anti-detection measures: user agent rotation, browser header profiles, and
per-request delay calculation.

```rust
pub struct AntiDetect {
    ua_pool: Vec<String>,
    delay_strategy: DelayStrategy,
}
```

#### `AntiDetect::new`

```rust
pub fn new(delay_strategy: DelayStrategy) -> Self
```

Create a new `AntiDetect` instance with a built-in pool of 50+ realistic user agent strings
(Chrome, Firefox, Safari, Edge across Windows, macOS, Linux) and the supplied delay strategy.

#### `AntiDetect::random_ua`

```rust
pub fn random_ua(&self) -> &str
```

Return a randomly selected user agent string from the pool.

#### `AntiDetect::get_delay`

```rust
pub fn get_delay(&self) -> Duration
```

Compute the delay duration before the next request based on the configured strategy.

| Strategy | Behavior |
|----------|----------|
| `Fixed` | Returns the configured fixed delay |
| `Random` | Returns a random duration within [min, max] |
| `Adaptive` | Returns the initial delay (full adaptive scaling is engine-level) |
| `None` | Returns `Duration::ZERO` |

#### `AntiDetect::browser_headers`

```rust
pub fn browser_headers(&self) -> Vec<(String, String)>
```

Return a set of realistic browser headers including User-Agent, Accept, Accept-Language,
Accept-Encoding, Connection, Upgrade-Insecure-Requests, Sec-Fetch-*, DNT, and
Sec-Ch-Ua-Platform. A Cache-Control header is randomly included for per-request variance.

---

### ProxyPool

A pool of proxies with configurable rotation strategies.

```rust
pub struct ProxyPool {
    proxies: Vec<ProxyConfig>,
    strategy: ProxyStrategy,
    current_index: AtomicUsize,
}
```

#### `ProxyPool::new`

```rust
pub fn new(proxies: Vec<ProxyConfig>, strategy: ProxyStrategy) -> Self
```

Create a new proxy pool with the given list of proxies and selection strategy.

#### `ProxyPool::next_proxy`

```rust
pub fn next_proxy(&self) -> Option<&ProxyConfig>
```

Select the next proxy according to the configured strategy. Returns `None` when the pool
is empty.

#### `ProxyPool::advance_failover`

```rust
pub fn advance_failover(&self)
```

Advance the failover index to the next proxy. Intended to be called when the current proxy
fails and the strategy is `FailoverOnly`.

#### `ProxyPool::is_empty`

```rust
pub fn is_empty(&self) -> bool
```

Return `true` if the pool contains no proxies.

#### `ProxyPool::len`

```rust
pub fn len(&self) -> usize
```

Return the number of proxies in the pool.

---

### Renderer

Headless browser renderer for JavaScript-heavy pages. Uses a headless Chromium browser to
execute JavaScript and extract the final rendered DOM.

```rust
pub struct Renderer {
    config: RendererConfig,
    initialized: bool,
}
```

> **Note:** JavaScript rendering is currently a stub implementation. The `render()` method
> returns a `RenderError` indicating that JS rendering is not yet implemented. The crawler
> falls back to static fetching.

#### `Renderer::new`

```rust
pub fn new(config: RendererConfig) -> Self
```

Create a new renderer with the given configuration.

#### `Renderer::initialize`

```rust
pub async fn initialize(&mut self) -> Result<()>
```

Initialize the browser (launch Chromium process). This is separate from `new()` because
it is an async operation that can fail.

#### `Renderer::render`

```rust
pub async fn render(&self, url: &Url) -> Result<RenderResult>
```

Render a page by navigating to the URL and executing JavaScript. Returns
`Err(BrowserError)` if the renderer is not initialized.

#### `Renderer::render_html`

```rust
pub async fn render_html(&self, html: &str, base_url: &Url) -> Result<RenderResult>
```

Render a page from already-fetched HTML (re-render with JS).

#### `Renderer::is_available`

```rust
pub fn is_available() -> bool
```

Check if the renderer is available (Chromium is installed) by probing common binary paths.

#### `Renderer::shutdown`

```rust
pub async fn shutdown(&mut self) -> Result<()>
```

Shutdown the browser.

#### `Renderer::is_initialized`

```rust
pub fn is_initialized(&self) -> bool
```

Whether the renderer has been initialized.

---

### RendererConfig

Configuration for the headless browser renderer.

```rust
#[derive(Debug, Clone)]
pub struct RendererConfig {
    pub render_timeout: Duration,
    pub wait_strategy: WaitStrategy,
    pub block_resources: bool,
    pub tab_pool_size: usize,
    pub headless: bool,
    pub max_scrolls: usize,
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `render_timeout` | `Duration` | `30s` | Timeout for rendering a page |
| `wait_strategy` | `WaitStrategy` | `Auto` | Wait strategy for page load |
| `block_resources` | `bool` | `true` | Block unnecessary resources (images, fonts, media) for speed |
| `tab_pool_size` | `usize` | `4` | Number of browser tabs to pool |
| `headless` | `bool` | `true` | Whether to run in headless mode |
| `max_scrolls` | `usize` | `0` | Maximum number of scroll operations for infinite scroll pages |

---

### RenderResult

Result of rendering a page with JavaScript.

```rust
#[derive(Debug, Clone)]
pub struct RenderResult {
    pub html: String,
    pub final_url: String,
    pub elapsed: Duration,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `html` | `String` | The rendered HTML after JavaScript execution |
| `final_url` | `String` | The final URL (after any JS-driven navigation) |
| `elapsed` | `Duration` | Time taken to render |

---

## 7. Crawl Management

### Scheduler

The main scheduler that manages the crawl frontier. It owns a priority queue of pending
`CrawlTask`s, tracks visited URLs for deduplication, enforces per-domain rate limits, and
respects the configured `ScanMode` to decide which discovered URLs should be enqueued.

```rust
pub struct Scheduler {
    frontier: Mutex<BinaryHeap<CrawlTask>>,
    visited: DashSet<String>,
    in_progress: DashSet<String>,
    domain_delays: DashMap<String, Instant>,
    pub stats: Arc<CrawlStats>,
    max_pages: usize,
    max_depth: u32,
    mode: ScanMode,
    domain_delay: Duration,
    remove_www: bool,
    done: AtomicBool,
    seed_url: Mutex<Option<Url>>,
}
```

#### `Scheduler::new`

```rust
pub fn new(config: &CrawlConfig, mode: ScanMode) -> Self
```

Create a new `Scheduler` from the given `CrawlConfig` and `ScanMode`.

#### `Scheduler::add_seed`

```rust
pub fn add_seed(&self, url: Url)
```

Add the initial seed URL with `Priority::Critical`.

#### `Scheduler::add_urls`

```rust
pub fn add_urls(&self, urls: Vec<Url>, depth: u32, source: TaskSource)
```

Add a batch of discovered URLs to the frontier. Each URL is normalised and deduplicated.
Behavior depends on the current `ScanMode`:

| Mode | Behavior |
|------|----------|
| `Lite` | No URLs are added (single-page mode) |
| `Full` | Only same-domain (internal) URLs are added |
| `Deep` | Both internal and external URLs are added |

#### `Scheduler::next_task`

```rust
pub fn next_task(&self) -> Option<CrawlTask>
```

Get the next URL to crawl from the priority queue. Returns `None` when the queue is empty.
If the highest-priority task's domain was accessed too recently (rate limiting), the task
is re-queued and the method attempts the next task.

#### `Scheduler::mark_completed`

```rust
pub fn mark_completed(&self, url: &Url)
```

Mark a URL as successfully crawled. Removes it from the in-progress set and updates the
per-domain timestamp for rate limiting.

#### `Scheduler::mark_failed`

```rust
pub fn mark_failed(&self, url: &Url)
```

Mark a URL as failed (will not be retried). Inserts it into the visited set to prevent
re-processing.

#### `Scheduler::is_done`

```rust
pub fn is_done(&self) -> bool
```

Returns `true` when the crawl is complete: the frontier is empty **and** there are no
tasks in progress.

#### `Scheduler::pending_count`

```rust
pub fn pending_count(&self) -> usize
```

Number of URLs currently waiting in the priority queue.

#### `Scheduler::should_crawl`

```rust
pub fn should_crawl(&self, url: &Url) -> bool
```

Check if a URL should be crawled given the current mode, depth limits, and deduplication
state. This is a read-only query that does **not** mutate any internal state.

#### `Scheduler::set_done`

```rust
pub fn set_done(&self)
```

Explicitly mark the scheduler as done (e.g., on cancellation).

---

### Priority

Priority levels for crawl tasks. Higher numeric value = higher priority. The `BinaryHeap`
is a max-heap, so `Critical` tasks are dequeued before `Low`.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}
```

| Priority | Value | Assigned When |
|----------|-------|---------------|
| `Critical` | 3 | Seed URL, sitemap URLs, feed URLs |
| `High` | 2 | Internal links at depth <= 1 |
| `Normal` | 1 | Internal links at depth > 1 |
| `Low` | 0 | External links |

---

### CrawlTask

A single crawl task in the queue.

```rust
#[derive(Debug, Clone)]
pub struct CrawlTask {
    pub url: Url,
    pub depth: u32,
    pub priority: Priority,
    pub source: TaskSource,
    pub requires_js: bool,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `url` | `Url` | The URL to crawl |
| `depth` | `u32` | The crawl depth (distance from seed) |
| `priority` | `Priority` | Priority level for queue ordering |
| `source` | `TaskSource` | How this URL was discovered |
| `requires_js` | `bool` | Whether JavaScript rendering is needed |

Tasks are ordered by `priority` in the `BinaryHeap` (higher priority dequeued first).

---

### TaskSource

Source of how a URL was discovered.

```rust
#[derive(Debug, Clone)]
pub enum TaskSource {
    Seed,
    InternalLink,
    ExternalLink,
    Sitemap,
    Feed,
}
```

| Variant | Description |
|---------|-------------|
| `Seed` | Initial URL provided by the user |
| `InternalLink` | Found on an internal page |
| `ExternalLink` | Found as an external link |
| `Sitemap` | Found in sitemap.xml |
| `Feed` | Found in an RSS/Atom feed |

---

### CrawlStats / CrawlStatsSnapshot

Atomic counters tracking crawl progress. `CrawlStats` uses `AtomicUsize` fields for
lock-free concurrent updates. `CrawlStatsSnapshot` is an immutable, cloneable snapshot.

```rust
#[derive(Debug, Default)]
pub struct CrawlStats {
    pub total_discovered: AtomicUsize,
    pub total_crawled: AtomicUsize,
    pub total_errors: AtomicUsize,
    pub total_skipped: AtomicUsize,
}
```

#### `CrawlStats::snapshot`

```rust
pub fn snapshot(&self) -> CrawlStatsSnapshot
```

Take a consistent snapshot of the current counters.

```rust
#[derive(Debug, Clone)]
pub struct CrawlStatsSnapshot {
    pub total_discovered: usize,
    pub total_crawled: usize,
    pub total_errors: usize,
    pub total_skipped: usize,
}
```

| Field | Description |
|-------|-------------|
| `total_discovered` | Total unique URLs discovered and added to the frontier |
| `total_crawled` | Total URLs successfully crawled |
| `total_errors` | Total URLs that failed permanently |
| `total_skipped` | Total URLs skipped (filtered by mode, depth, or page limit) |

---

## 8. Auxiliary Parsers

### RobotsTxt

Parsed robots.txt rules for one or more user agents.

```rust
#[derive(Debug, Clone, Default)]
pub struct RobotsTxt {
    rules: Vec<RobotsRule>,
    sitemaps: Vec<String>,
    crawl_delay: Option<f64>,
}
```

#### `RobotsTxt::parse`

```rust
pub fn parse(content: &str) -> Self
```

Parse the text content of a robots.txt file into structured rules. Recognised directives
(case-insensitive): `User-agent`, `Allow`, `Disallow`, `Sitemap`, `Crawl-delay`.

```rust
let robots = RobotsTxt::parse(r#"
User-agent: *
Disallow: /admin
Allow: /admin/public

Sitemap: https://example.com/sitemap.xml
Crawl-delay: 2
"#);
```

#### `RobotsTxt::is_allowed`

```rust
pub fn is_allowed(&self, path: &str) -> bool
```

Check whether the given URL path is allowed for the Rehyke crawler. Matching logic:

1. First look for rules targeting `"rehyke"` (case-insensitive)
2. If none exist, fall back to the wildcard `"*"` rules
3. If no matching rules exist at all, allow by default
4. Among matched rules, the most specific (longest) pattern wins
5. If both an allow and a disallow match with the same specificity, the allow takes precedence

Supports `*` wildcards and `$` end-of-path anchors.

```rust
assert!(!robots.is_allowed("/admin"));
assert!(robots.is_allowed("/admin/public"));
assert!(robots.is_allowed("/public"));
```

#### `RobotsTxt::sitemaps`

```rust
pub fn sitemaps(&self) -> &[String]
```

Return the sitemap URLs listed in the robots.txt file.

#### `RobotsTxt::crawl_delay`

```rust
pub fn crawl_delay(&self) -> Option<f64>
```

Return the crawl delay (in seconds) if one was specified.

#### `RobotsTxt::robots_url`

```rust
pub fn robots_url(base: &Url) -> String
```

Build the canonical robots.txt URL for the given base URL.

```rust
let base = Url::parse("https://example.com/some/page").unwrap();
assert_eq!(RobotsTxt::robots_url(&base), "https://example.com/robots.txt");
```

---

### Sitemap

A parsed sitemap document (either a URL set or a sitemap index).

```rust
#[derive(Debug, Clone)]
pub struct Sitemap {
    pub entries: Vec<SitemapEntry>,
    pub sub_sitemaps: Vec<String>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `entries` | `Vec<SitemapEntry>` | URL entries from `<urlset>` documents |
| `sub_sitemaps` | `Vec<String>` | Sub-sitemap URLs from `<sitemapindex>` documents |

#### `Sitemap::parse`

```rust
pub fn parse(xml: &str) -> Result<Self>
```

Parse a sitemap XML string. Handles both `<urlset>` sitemaps (containing `<url>` elements)
and `<sitemapindex>` sitemaps (containing `<sitemap>` elements with `<loc>` sub-elements).
Supports namespace-prefixed tags (e.g., `<ns:url>`).

```rust
let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url>
    <loc>https://example.com/</loc>
    <lastmod>2024-01-15</lastmod>
    <changefreq>daily</changefreq>
    <priority>1.0</priority>
  </url>
</urlset>"#;

let sitemap = Sitemap::parse(xml)?;
assert_eq!(sitemap.entries.len(), 1);
```

#### `Sitemap::is_sitemap_index`

```rust
pub fn is_sitemap_index(xml: &str) -> bool
```

Check whether the given XML content looks like a sitemap index (contains `<sitemapindex`).

#### `Sitemap::urls`

```rust
pub fn urls(&self) -> Vec<&str>
```

Return references to all URL strings contained in this sitemap's entries.

#### `Sitemap::common_sitemap_urls`

```rust
pub fn common_sitemap_urls(base: &Url) -> Vec<String>
```

Build a list of common sitemap URLs to probe for a given domain. Returns 4 URLs:

1. `{origin}/sitemap.xml`
2. `{origin}/sitemap_index.xml`
3. `{origin}/sitemap/sitemap.xml`
4. `{origin}/sitemaps.xml`

---

### SitemapEntry

A single entry extracted from a `<url>` element in a sitemap.

```rust
#[derive(Debug, Clone)]
pub struct SitemapEntry {
    pub loc: String,
    pub lastmod: Option<String>,
    pub changefreq: Option<String>,
    pub priority: Option<f64>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `loc` | `String` | The URL location (`<loc>`) |
| `lastmod` | `Option<String>` | The last modification date (`<lastmod>`) |
| `changefreq` | `Option<String>` | The change frequency (`<changefreq>`) |
| `priority` | `Option<f64>` | The priority value (`<priority>`, 0.0 to 1.0) |

---

## 9. Error Handling

### RehykeError

All errors in Rehyke are represented by the `RehykeError` enum. Implements
`std::error::Error` and `Display` via the `thiserror` crate.

```rust
#[derive(Debug, thiserror::Error)]
pub enum RehykeError { /* variants */ }

pub type Result<T> = std::result::Result<T, RehykeError>;
```

#### Error Variants

| Variant | Fields | Display Message | Description |
|---------|--------|-----------------|-------------|
| `HttpError` | `url: String`, `status: u16` | `"HTTP error for {url}: {status}"` | Non-success HTTP status code |
| `Timeout` | `url: String` | `"Connection timeout for {url}"` | Request timed out |
| `DnsError` | `domain: String` | `"DNS resolution failed for {domain}"` | DNS lookup failure |
| `TlsError` | `url: String`, `message: String` | `"TLS/SSL error for {url}: {message}"` | TLS certificate or handshake error |
| `RenderError` | `url: String`, `message: String` | `"JavaScript rendering failed for {url}: {message}"` | Headless browser rendering failure |
| `BrowserError` | `message: String` | `"Browser launch failed: {message}"` | Failed to launch or initialize browser |
| `ParseError` | `url: String`, `message: String` | `"Parse error for {url}: {message}"` | Content parsing failure |
| `ProxyError` | `message: String` | `"Proxy error: {message}"` | Proxy configuration or connection error |
| `RateLimited` | `domain: String` | `"Rate limited by {domain}"` | Server returned 429 or equivalent |
| `MaxPagesReached` | `limit: usize` | `"Max pages limit reached: {limit}"` | Crawl hit the configured page limit |
| `IoError` | `#[from] std::io::Error` | `"IO error: {0}"` | File system or IO operation error |
| `ConfigError` | `message: String` | `"Configuration error: {message}"` | Invalid configuration value |
| `RequestError` | `#[from] reqwest::Error` | `"Request error: {0}"` | Low-level HTTP client error |
| `UrlParseError` | `#[from] url::ParseError` | `"URL parse error: {0}"` | Malformed URL |

---

### Error Recovery Strategy

How the crawler handles different error types during a crawl:

| Error Type | Retried? | Recovery Action |
|------------|----------|-----------------|
| `HttpError` (429, 500, 502, 503, 504) | Yes | Exponential backoff; respects `Retry-After` header |
| `HttpError` (403) | No | Hard failure; anti-detect module handles UA/proxy rotation separately |
| `HttpError` (other 4xx/5xx) | No | Hard failure; URL marked as failed |
| `Timeout` | Yes | Exponential backoff up to `max_retries` |
| `DnsError` | Yes | Exponential backoff up to `max_retries` |
| `TlsError` | No | Hard failure; usually a certificate issue |
| `RenderError` | No | Falls back to static fetching |
| `BrowserError` | No | Hard failure; browser not available |
| `ParseError` | No | URL marked as failed; crawl continues |
| `ProxyError` | No | Hard failure for the request; caller may switch proxy |
| `RateLimited` | Yes | Respects server-indicated delay |
| `MaxPagesReached` | N/A | Crawl terminates gracefully |
| `IoError` | No | File write failure; logged and continued |
| `ConfigError` | N/A | Crawl does not start |
| `RequestError` | Yes | Network-level retries with backoff |
| `UrlParseError` | N/A | URL skipped |

---

## 10. Python API

Rehyke provides Python bindings via PyO3. Install with:

```bash
pip install rehyke
```

Or build from source:

```bash
cd /path/to/rehyke
pip install -e .
```

### Module-level `crawl()` Function

```python
rehyke.crawl(url: str, mode: str = "full") -> list[CrawlResult]
```

Simple one-shot crawl function.

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `url` | `str` | (required) | The URL to crawl |
| `mode` | `str` | `"full"` | Scan mode: `"lite"`, `"full"`, or `"deep"` |

**Returns:** `list[CrawlResult]`

**Raises:** `ValueError` for invalid mode, `TimeoutError` for timeouts, `RuntimeError`
for other crawl errors.

```python
import rehyke

results = rehyke.crawl("https://example.com", mode="full")
for page in results:
    print(page.title)
    print(page.markdown)
```

---

### Python Rehyke Class

```python
class Rehyke:
    def __init__(self, config: CrawlConfig | None = None) -> None: ...
    def crawl(self, url: str) -> list[CrawlResult]: ...
    def crawl_to_file(self, url: str, path: str) -> None: ...
```

#### `Rehyke.__init__`

```python
Rehyke(config=None)
```

Create a new crawler instance with optional configuration. Uses defaults if no config
is provided.

#### `Rehyke.crawl`

```python
Rehyke.crawl(url: str) -> list[CrawlResult]
```

Crawl a URL and return a list of `CrawlResult` objects.

#### `Rehyke.crawl_to_file`

```python
Rehyke.crawl_to_file(url: str, path: str) -> None
```

Crawl a URL and write the Markdown output to a file. Multiple pages are separated by
`---` dividers.

```python
crawler = rehyke.Rehyke()
crawler.crawl_to_file("https://example.com", "output.md")
```

---

### Python CrawlConfig Class

```python
class CrawlConfig:
    def __init__(
        self,
        mode: ScanMode | None = None,
        max_depth: int | None = None,
        max_pages: int | None = None,
        concurrency: int | None = None,
        enable_js: bool = False,
        user_agent: str | None = None,
        timeout_secs: int | None = None,
        max_retries: int | None = None,
        respect_robots_txt: bool = True,
        clean_navigation: bool = True,
        clean_footers: bool = True,
        clean_ads: bool = True,
        exclude_patterns: list[str] | None = None,
        include_patterns: list[str] | None = None,
        delay_min_ms: int | None = None,
        delay_max_ms: int | None = None,
    ) -> None: ...
```

All parameters are optional. When omitted, sensible defaults are used based on the
selected mode.

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `mode` | `ScanMode \| None` | `ScanMode.FULL` | Scan mode preset |
| `max_depth` | `int \| None` | Mode default | Maximum crawl depth |
| `max_pages` | `int \| None` | Mode default | Maximum page count |
| `concurrency` | `int \| None` | Mode default | Concurrent requests |
| `enable_js` | `bool` | `False` | Enable JavaScript rendering |
| `user_agent` | `str \| None` | `"rehyke/{version}"` | Custom User-Agent |
| `timeout_secs` | `int \| None` | `30` | Per-request timeout in seconds |
| `max_retries` | `int \| None` | `3` | Maximum retry attempts |
| `respect_robots_txt` | `bool` | `True` | Honour robots.txt |
| `clean_navigation` | `bool` | `True` | Remove nav elements |
| `clean_footers` | `bool` | `True` | Remove footer elements |
| `clean_ads` | `bool` | `True` | Remove ad elements |
| `exclude_patterns` | `list[str] \| None` | `None` | URL exclusion regex patterns |
| `include_patterns` | `list[str] \| None` | `None` | URL inclusion regex patterns |
| `delay_min_ms` | `int \| None` | `None` | Minimum delay between requests (ms) |
| `delay_max_ms` | `int \| None` | `None` | Maximum delay between requests (ms) |

Delay strategy is automatically determined:
- Both `delay_min_ms` and `delay_max_ms` set: `Random` strategy
- Only one set: `Fixed` strategy using that value
- Neither set: `None` strategy (no delay)

```python
config = rehyke.CrawlConfig(
    mode=rehyke.ScanMode.LITE,
    max_depth=3,
    concurrency=20,
    timeout_secs=60,
    exclude_patterns=[r"\.pdf$", r"/admin/"],
    delay_min_ms=500,
    delay_max_ms=2000,
)
crawler = rehyke.Rehyke(config)
results = crawler.crawl("https://example.com")
```

---

### Python CrawlResult Class

```python
class CrawlResult:
    url: str
    title: str
    markdown: str
    status_code: int
    content_type: str
```

| Property | Type | Description |
|----------|------|-------------|
| `url` | `str` | The URL that was crawled |
| `title` | `str` | Page title extracted from the document |
| `markdown` | `str` | Markdown representation of the page content |
| `status_code` | `int` | HTTP status code of the response |
| `content_type` | `str` | Content-Type header value |

Special methods:
- `__repr__()` returns `CrawlResult(url='...', title='...', status_code=...)`
- `__str__()` returns the Markdown content

```python
result = results[0]
print(result)           # prints the markdown
print(repr(result))     # CrawlResult(url='https://example.com/', title='Example', status_code=200)
print(result.url)       # https://example.com/
print(result.markdown)  # full markdown content
```

---

### Python ScanMode Enum

```python
class ScanMode:
    LITE = 0
    FULL = 1
    DEEP = 2
```

| Value | Constant | Description |
|-------|----------|-------------|
| `0` | `ScanMode.LITE` | Quick surface-level crawl |
| `1` | `ScanMode.FULL` | Balanced crawl for most use-cases (default) |
| `2` | `ScanMode.DEEP` | Exhaustive deep crawl |

```python
import rehyke

# Using enum values
results = rehyke.crawl("https://example.com", mode="lite")

# Or with CrawlConfig
config = rehyke.CrawlConfig(mode=rehyke.ScanMode.DEEP)
```

---

## Appendix: OutputHandler

The `OutputHandler` manages output of crawl results to memory, individual files, or a
single file.

```rust
pub struct OutputHandler {
    mode: OutputMode,
    results: Vec<CrawlResult>,
}
```

#### `OutputHandler::new`

```rust
pub fn new(mode: OutputMode) -> Self
```

Create a new output handler for the given output mode.

#### `OutputHandler::handle_result`

```rust
pub fn handle_result(&mut self, result: CrawlResult) -> Result<()>
```

Process a single crawl result. Depending on the configured `OutputMode`:

- **Memory** -- the result is stored in an internal buffer
- **Files** -- the markdown is written to an individual file derived from the crawled URL
- **SingleFile** -- the markdown is appended to a single output file with `---` separators

#### `OutputHandler::finalize`

```rust
pub fn finalize(self) -> Result<Vec<CrawlResult>>
```

Finish processing and return all collected crawl results. Results are tracked regardless
of the output mode.

#### `OutputHandler::results`

```rust
pub fn results(&self) -> &[CrawlResult]
```

Get a reference to the currently collected results.

---

## Appendix: ParsedDocument and Feed Types

### ParsedDocument

```rust
#[derive(Debug, Clone)]
pub struct ParsedDocument {
    pub metadata: PageMetadata,
    pub content_nodes: Vec<ContentNode>,
    pub content_type: ContentType,
}
```

### FeedItem

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedItem {
    pub title: Option<String>,
    pub link: Option<String>,
    pub description: Option<String>,
    pub pub_date: Option<String>,
    pub author: Option<String>,
}
```

### ParsedFeed

```rust
#[derive(Debug, Clone)]
pub struct ParsedFeed {
    pub title: Option<String>,
    pub link: Option<String>,
    pub description: Option<String>,
    pub items: Vec<FeedItem>,
}
```
