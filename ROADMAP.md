# Rehyke Roadmap: v0.1.0 → v1.0.0

> **"Crawl Everything. Miss Nothing."**
>
> A version-by-version evolution from alpha crawler to the most advanced
> web intelligence platform ever built in Rust.

---

## Version Timeline

```
  0.1.0          0.3.0          0.5.0          0.7.0          0.9.0    1.0.0
    │              │              │              │              │        │
    ▼              ▼              ▼              ▼              ▼        ▼
 ┌──────┐  ┌───────────┐  ┌───────────┐  ┌───────────┐  ┌─────────┐ ┌──────┐
 │ALPHA │──│ RENDERING │──│INTELLIGENCE──│DISTRIBUTED│──│  CLOUD  │─│STABLE│
 │ENGINE│  │  & STEALTH│  │  & EXTRACT│  │  & SCALE  │  │ & WASM  │ │ 1.0  │
 └──────┘  └───────────┘  └───────────┘  └───────────┘  └─────────┘ └──────┘
    │              │              │              │              │        │
  PHASE 1       PHASE 2       PHASE 3       PHASE 4       PHASE 5   RELEASE
  Foundation    Browser       Brains        Scale          Edge      Production
```

---

## v0.1.0 — "Groundwork" (Current Release)

**Status:** Released
**Theme:** Core crawl engine with Rust performance

### What Shipped

- [x] Full crawl engine — 14 modules, 11,670 lines of Rust
- [x] Three scan modes: Lite (single page), Full (domain-wide), Deep (cross-domain)
- [x] CrawlConfig builder with 25+ configurable options
- [x] HTTP/2 fetcher via reqwest with gzip/brotli/zstd compression
- [x] Universal parser: HTML, XHTML, XML, RSS 2.0, Atom, JSON, JSON-LD, SVG, Sitemap, Plain Text
- [x] HTML-to-Markdown converter with 18 element types + GFM tables
- [x] YAML frontmatter generation with rich page metadata
- [x] Content cleaning: scripts, styles, nav, headers, footers, ads, comments
- [x] Link extractor scanning 12+ HTML element types + srcset + meta refresh
- [x] Priority-based URL scheduler with BinaryHeap + DashSet dedup
- [x] URL normalization with 7 rules
- [x] robots.txt parser with wildcard and `$` anchor support
- [x] Sitemap XML parser (urlset + sitemapindex) with auto-probing
- [x] 57 rotating user agents with realistic browser headers
- [x] Delay strategies: Fixed, Random, Adaptive, None
- [x] Proxy pool: HTTP/HTTPS/SOCKS5 with 4 rotation strategies
- [x] Retry with exponential backoff + Retry-After header support
- [x] CLI binary (rehyke-cli): 20+ options, progress bar, JSON output
- [x] Python bindings (PyO3): crawl(), Rehyke class, CrawlConfig, ScanMode
- [x] 369 unit tests across all modules
- [x] Comprehensive error handling with 14 error variants
- [x] Structured logging with tracing

### Performance Baselines

| Metric | Achieved |
|--------|----------|
| Single page (no JS) | < 200ms |
| 100 pages parallel | < 30s |
| 1,000 pages parallel | < 5 min |
| 10,000 pages parallel | < 30 min |
| Memory per page | < 5MB |
| Concurrent connections | Up to 200 |

---

## v0.2.0 — "Chrome Eyes"

**Theme:** Full JavaScript rendering and headless browser integration
**Expected:** Completing the renderer.rs stub into a production system

### Features

- [ ] **Headless Chromium Integration**
  - Complete `renderer.rs` implementation via `chromiumoxide` crate
  - Tab pooling — reuse browser tabs instead of spawning per page
  - Resource interception — block images, fonts, media for speed
  - Configurable wait strategies: NetworkIdle, Selector, Duration, Auto
  - Extract final rendered DOM as HTML string for parser pipeline

- [ ] **SPA Handling**
  - React, Vue, Angular, Svelte, Next.js, Nuxt, SvelteKit detection
  - Automatic client-side routing detection and page state extraction
  - Hash-based and history-based route discovery
  - Hydration-aware extraction — wait for framework to mount before scraping

- [ ] **Infinite Scroll & Pagination**
  - Auto-detect infinite scroll patterns (Intersection Observer, scroll events)
  - Configurable scroll count and scroll-to-bottom behavior
  - "Load More" button detection and auto-click
  - Pagination link discovery (next/prev, page numbers, cursor-based)
  - Configurable max pages per paginated resource

- [ ] **Popup & Overlay Dismissal**
  - Cookie consent banner auto-detection and auto-dismiss
  - Newsletter/subscription popup handling
  - Login wall detection with configurable behavior (skip or authenticate)
  - GDPR consent modal handling for EU sites
  - Paywall detection and reporting (not bypass)

- [ ] **Browser Fingerprint Diversity**
  - Randomized viewport sizes (desktop/tablet/mobile profiles)
  - Randomized WebGL renderer strings
  - Canvas fingerprint noise injection
  - Navigator.plugins and Navigator.languages variation
  - Timezone and locale randomization matching proxy geo

- [ ] **Screenshot & Visual Capture**
  - Full-page screenshot on demand (PNG/JPEG)
  - Element-level screenshot by CSS selector
  - Visual diff capability — detect layout changes between crawls
  - PDF export of rendered pages

### Python API Additions

```python
config = CrawlConfig(
    enable_js=True,
    js_wait_strategy="network_idle",
    js_wait_timeout=5.0,
    scroll_count=10,
    dismiss_popups=True,
    viewport="desktop",       # "desktop", "tablet", "mobile"
    screenshot=True,
    screenshot_format="png",
)
```

### CLI Additions

```bash
rehyke https://spa-app.com --js --wait-for ".content-loaded" \
    --scroll 10 --dismiss-popups --screenshot --viewport mobile
```

---

## v0.3.0 — "Shadow Protocol"

**Theme:** Advanced anti-detection, stealth, and authentication
**Expected:** Making rehyke undetectable by even the most aggressive bot protection

### Features

- [ ] **TLS Fingerprint Engine**
  - JA3/JA4 fingerprint rotation across 20+ real browser profiles
  - Cipher suite ordering randomization matching target browser
  - TLS extension mimicry (SNI, ALPN, signature algorithms)
  - HTTP/2 SETTINGS frame fingerprint matching (Akamai fingerprint)
  - Per-request TLS profile selection based on target UA

- [ ] **Behavioral Simulation**
  - Mouse movement simulation with Bezier curve trajectories
  - Realistic scroll patterns (variable speed, pauses at content)
  - Typing simulation for form fields (variable keystroke timing)
  - Click delay randomization (150ms-600ms, normal distribution)
  - Tab focus/blur event simulation
  - Referrer chain construction (search engine → landing → target)

- [ ] **CAPTCHA Awareness**
  - reCAPTCHA v2/v3, hCaptcha, Cloudflare Turnstile detection
  - Configurable behavior: skip, flag, callback to user-provided solver
  - CAPTCHA solver integration API (plug in 2Captcha, Anti-Captcha, etc.)
  - Challenge page detection and reporting
  - Automatic retry with different identity on soft blocks

- [ ] **Session & Authentication Support**
  - Cookie jar persistence across crawl sessions (SQLite-backed)
  - Form-based login automation (username/password fields)
  - OAuth2 bearer token injection
  - API key header injection
  - Session rotation — re-authenticate when session expires
  - Multi-account rotation for distributed identity

- [ ] **Residential Proxy Intelligence**
  - Geo-targeted proxy selection (match proxy country to site language)
  - Sticky sessions — keep same IP for same domain across requests
  - Proxy health scoring (latency, success rate, block rate)
  - Automatic proxy ban detection and rotation
  - Bandwidth-aware rotation (prefer cheaper proxies for static content)
  - SOCKS5 with authentication, HTTP CONNECT tunneling

- [ ] **Request Fingerprint Randomization**
  - HTTP header ordering randomization per browser profile
  - Accept-Language variation matching proxy geo
  - DNT/Sec-Fetch-* header consistency with UA profile
  - Connection keep-alive behavior matching real browsers
  - Cookie ordering and format matching browser engine

### Configuration

```rust
let config = CrawlConfig::builder()
    .stealth_mode(StealthLevel::Maximum)
    .tls_fingerprint(TlsProfile::Chrome128)
    .simulate_behavior(true)
    .captcha_handler(CaptchaHandler::Callback(my_solver))
    .session_file("session.db")
    .login_url("https://example.com/login")
    .login_credentials("user", "pass")
    .proxy_geo("US")
    .sticky_sessions(true)
    .build()?;
```

---

## v0.4.0 — "Regex Superpowers"

**Theme:** Custom extraction rules, structured data mining, and content intelligence
**Expected:** Transform rehyke from a crawler into a data extraction platform

### Features

- [ ] **Regex Rule Engine**
  - Named capture groups with typed output: `(?P<price>\d+\.\d{2})`
  - Multi-pattern extraction chains: extract → transform → validate
  - Rule sets loadable from YAML/JSON config files
  - Built-in rule library for common patterns (emails, phones, prices, dates, addresses)
  - Regex compilation caching — compile once, match millions of times
  - PCRE2-compatible syntax via `fancy-regex` for lookaheads/lookbehinds

- [ ] **CSS Selector Extraction**
  - Target specific elements: `article.post > .content`
  - Attribute extraction: `a[href]`, `img[src]`, `meta[name="author"][content]`
  - Pseudo-selector support: `:first-child`, `:nth-of-type(2n)`
  - Multi-selector batch extraction in single DOM pass
  - Selector-to-field mapping: `{ "title": "h1.title", "price": ".price-tag" }`

- [ ] **XPath Extraction**
  - Full XPath 1.0 support for complex document traversal
  - XPath-to-field mapping alongside CSS selectors
  - Namespace-aware XPath for XML/XHTML documents
  - XPath functions: `contains()`, `starts-with()`, `normalize-space()`

- [ ] **Structured Data Extraction**
  - JSON-LD / Schema.org automatic extraction and normalization
  - Microdata (itemscope/itemprop) extraction
  - RDFa extraction
  - Open Graph / Twitter Card metadata
  - Output as typed JSON with schema validation

- [ ] **Content Scoring & Relevance**
  - Readability scoring (Flesch-Kincaid, Coleman-Liau, SMOG)
  - Content density analysis — text-to-HTML ratio per block
  - Main content detection (Readability-like article extraction)
  - Boilerplate ratio calculation
  - Language detection per page (CLD2 or whatlang-rs)
  - Duplicate content detection via SimHash/MinHash fingerprints

- [ ] **Table Extraction**
  - HTML table → structured JSON/CSV conversion
  - Colspan/rowspan normalization
  - Header row detection and column typing
  - Nested table flattening
  - Table-to-DataFrame output (Python: pandas DataFrame)

- [ ] **Custom Pipeline API**
  - User-defined transform stages: `fetch → parse → extract → transform → output`
  - Pipeline defined in code or YAML config
  - Stage-level error handling and fallbacks
  - Pipeline result aggregation across pages

### Python API Additions

```python
from rehyke import Rehyke, ExtractionRule, Pipeline

# Regex extraction
rules = [
    ExtractionRule.regex("price", r"\$(?P<amount>\d+\.\d{2})"),
    ExtractionRule.css("title", "h1.product-title"),
    ExtractionRule.xpath("rating", "//div[@class='rating']/@data-score"),
    ExtractionRule.schema_org("Product"),
]

results = rehyke.crawl("https://shop.example.com", rules=rules)

for page in results:
    print(page.extracted["price"])     # [{"amount": "29.99"}, ...]
    print(page.extracted["title"])     # "Product Name"
    print(page.extracted["rating"])    # "4.5"
    print(page.extracted["Product"])   # Schema.org Product object
```

### CLI Additions

```bash
rehyke https://shop.example.com --mode full \
    --extract-regex 'price:\$(?P<amount>\d+\.\d{2})' \
    --extract-css 'title:h1.product-title' \
    --extract-schema Product \
    --format json \
    --output products.json
```

---

## v0.5.0 — "Autonomous Agent"

**Theme:** AI-driven crawl planning, adaptive behavior, and self-healing
**Expected:** The crawler that thinks for itself

### Features

- [ ] **Crawl Planner**
  - Seed URL analysis — detect site type (blog, e-commerce, docs, forum, wiki)
  - Automatic mode selection based on site structure
  - Depth/concurrency auto-tuning based on server response times
  - Sitemap-first strategy — discover sitemap before blind crawling
  - Priority inference from URL patterns (product pages > category pages > legal pages)

- [ ] **URL Pattern Learning**
  - Automatic URL pattern discovery from crawled URLs
  - Pattern-based priority boosting (e.g., `/product/*` → High)
  - Duplicate template detection (same layout, different data)
  - Pagination pattern recognition and unrolling
  - Query parameter significance detection (which params change content vs tracking)

- [ ] **Adaptive Rate Control**
  - Server response time monitoring with sliding window
  - Automatic concurrency scaling: speed up on fast servers, back off on slow
  - 429/503 response → immediate slowdown with graduated recovery
  - Bandwidth estimation and throttling per domain
  - Politeness scoring — adjust aggressiveness to match site tolerance

- [ ] **Content Change Detection**
  - Page fingerprinting via content hash (xxHash for speed)
  - Differential crawling — only re-fetch changed pages
  - Change frequency estimation per URL pattern
  - ETag and Last-Modified header tracking
  - Conditional requests (If-None-Match, If-Modified-Since)
  - Change notification callbacks

- [ ] **Self-Healing Crawl**
  - Automatic checkpoint/resume — survive process restarts
  - Crawl state serialization to disk (MessagePack or bincode format)
  - Partial result recovery on crash
  - Stale URL detection and re-queuing
  - Dead-end detection — stop wasting time on fruitless branches
  - Configurable crawl budgets (time, pages, bandwidth)

- [ ] **Multi-Seed Crawling**
  - Accept multiple seed URLs in single crawl job
  - Cross-site deduplication (same content on different domains)
  - Per-seed configuration overrides
  - Seed priority ordering
  - Seed group results segregation

### Configuration

```rust
let config = CrawlConfig::builder()
    .autonomous(true)
    .auto_tune_concurrency(true)
    .change_detection(true)
    .checkpoint_file("crawl_state.bin")
    .checkpoint_interval(Duration::from_secs(60))
    .crawl_budget(CrawlBudget {
        max_time: Duration::from_secs(3600),
        max_pages: 50_000,
        max_bandwidth: ByteSize::gb(5),
    })
    .seeds(vec![
        "https://docs.example.com",
        "https://blog.example.com",
        "https://api.example.com",
    ])
    .build()?;
```

---

## v0.6.0 — "Intelligence Layer"

**Theme:** Analytics, SEO auditing, link graph analysis, and reporting
**Expected:** Rehyke becomes a web intelligence platform, not just a crawler

### Features

- [ ] **Link Graph Engine**
  - Build complete internal link graph (directed, weighted)
  - PageRank calculation for internal pages
  - Orphan page detection (no inbound internal links)
  - Redirect chain mapping (301/302 chains → final destination)
  - Broken link detection (404s, 5xxs, DNS failures)
  - External link inventory with status codes
  - Link graph export: DOT (Graphviz), GEXF (Gephi), JSON

- [ ] **SEO Audit Module**
  - Title tag analysis (length, uniqueness, keyword presence)
  - Meta description audit (length, uniqueness, presence)
  - Heading hierarchy validation (H1 count, H1→H6 order)
  - Image alt text audit (missing, empty, generic)
  - Canonical tag validation (self-referencing, cross-domain)
  - Hreflang tag validation for multilingual sites
  - Open Graph / Twitter Card completeness
  - robots.txt / meta robots conflict detection
  - Sitemap coverage analysis (pages in sitemap vs discovered pages)
  - Page speed indicators (HTML size, resource count, render-blocking scripts)
  - Mobile viewport meta tag check
  - HTTPS/mixed content detection
  - Structured data validation (JSON-LD schema errors)

- [ ] **Content Analytics**
  - Word count and reading time per page
  - Vocabulary richness (type-token ratio)
  - Content similarity matrix (TF-IDF based)
  - Thin content detection (< 300 words with boilerplate)
  - Duplicate/near-duplicate content detection across site
  - Content freshness scoring (date-based decay)
  - Topic clustering via keyword extraction (RAKE/TextRank algorithm)

- [ ] **Crawl Report Generation**
  - HTML report with interactive charts (embedded, no server needed)
  - PDF report export
  - JSON/CSV data export for all findings
  - Executive summary with top issues and recommendations
  - Comparison reports between crawl runs (what changed?)
  - Custom report templates

- [ ] **Monitoring & Alerts**
  - Webhook notifications on crawl events (complete, error, threshold)
  - Slack/Discord integration for crawl status updates
  - Email report delivery
  - Configurable alert thresholds (> N broken links, > N% 5xx errors)

### CLI Additions

```bash
# Full SEO audit
rehyke https://example.com --mode full --audit seo \
    --report html --report-output audit.html

# Broken link check
rehyke https://example.com --mode full --check-links \
    --report json --report-output links.json

# Content analytics
rehyke https://example.com --mode full --analytics content \
    --detect-duplicates --thin-content-threshold 300
```

### Python API Additions

```python
from rehyke import Rehyke, AuditConfig

crawler = Rehyke(CrawlConfig(mode="full"))
results = crawler.crawl("https://example.com")

# SEO audit
audit = results.seo_audit()
print(f"Missing titles: {len(audit.missing_titles)}")
print(f"Broken links: {len(audit.broken_links)}")
print(f"Orphan pages: {len(audit.orphan_pages)}")

# Link graph
graph = results.link_graph()
graph.export_dot("site_graph.dot")
top_pages = graph.pagerank(top_n=20)

# Content analytics
analytics = results.content_analytics()
duplicates = analytics.find_duplicates(threshold=0.85)
```

---

## v0.7.0 — "Distributed Swarm"

**Theme:** Multi-node distributed crawling for massive scale
**Expected:** Crawl the entire internet (or at least try)

### Features

- [ ] **Worker Architecture**
  - Coordinator/Worker model with gRPC communication
  - Coordinator: URL frontier management, work distribution, result aggregation
  - Workers: fetch + parse + extract (stateless, horizontally scalable)
  - Worker auto-discovery via mDNS or manual registration
  - Worker health monitoring with heartbeat and automatic failover
  - Dynamic worker scaling — add/remove workers during crawl

- [ ] **Distributed URL Frontier**
  - Redis-backed URL queue for cross-worker deduplication
  - Consistent hashing — same domain always routes to same worker
  - Priority preservation across distributed queue
  - Bloom filter for memory-efficient visited URL tracking (billions of URLs)
  - Frontier partitioning by domain for locality

- [ ] **Result Storage Backends**
  - Local filesystem (existing)
  - Amazon S3 / Google Cloud Storage / Azure Blob
  - PostgreSQL for structured metadata + search
  - Elasticsearch for full-text indexing
  - Apache Parquet for columnar analytics
  - Custom backend trait — implement your own storage

- [ ] **Crawl Job Management**
  - Job queue with priority and scheduling
  - Cron-like scheduled crawls
  - Job pause/resume/cancel
  - Job dependency chains (crawl A then crawl B with A's data)
  - Job templates for recurring crawl configurations
  - Multi-tenant isolation (separate jobs, separate quotas)

- [ ] **Metrics & Observability**
  - Prometheus metrics export (pages/sec, errors/sec, queue depth, latency p50/p95/p99)
  - OpenTelemetry tracing for distributed request tracing
  - Grafana dashboard templates (shipped with rehyke)
  - Per-domain crawl statistics
  - Real-time crawl progress via WebSocket

- [ ] **Fault Tolerance**
  - Worker crash recovery — re-queue in-progress URLs
  - Coordinator failover with state replication
  - Network partition handling (split-brain protection)
  - Graceful degradation under memory/CPU pressure
  - Circuit breaker per domain (stop hitting dead servers)

### Architecture

```
                    ┌─────────────────────┐
                    │    Coordinator      │
                    │  ┌───────────────┐  │
                    │  │ URL Frontier  │  │
                    │  │ (Redis)       │  │
                    │  ├───────────────┤  │
                    │  │ Job Manager   │  │
                    │  ├───────────────┤  │
                    │  │ Metrics       │  │
                    │  └───────────────┘  │
                    └──────────┬──────────┘
                               │ gRPC
              ┌────────────────┼────────────────┐
              ▼                ▼                 ▼
     ┌────────────────┐ ┌────────────────┐ ┌────────────────┐
     │   Worker #1    │ │   Worker #2    │ │   Worker #N    │
     │  Fetch → Parse │ │  Fetch → Parse │ │  Fetch → Parse │
     │  → Extract     │ │  → Extract     │ │  → Extract     │
     └────────┬───────┘ └────────┬───────┘ └────────┬───────┘
              │                  │                   │
              ▼                  ▼                   ▼
     ┌─────────────────────────────────────────────────────┐
     │              Storage Backend (S3/PG/ES)             │
     └─────────────────────────────────────────────────────┘
```

### Configuration

```rust
// Coordinator
let coordinator = RehykeCoordinator::builder()
    .redis_url("redis://cluster:6379")
    .storage(StorageBackend::S3 {
        bucket: "rehyke-results",
        region: "us-east-1",
    })
    .metrics_port(9090)
    .max_workers(50)
    .build()?;

coordinator.submit_job(CrawlJob {
    seeds: vec!["https://example.com"],
    config: CrawlConfig::builder().mode(ScanMode::Deep).build()?,
    schedule: Some("0 0 * * *"),  // Daily at midnight
    priority: JobPriority::High,
})?;

coordinator.run().await?;

// Worker (separate process/machine)
let worker = RehykeWorker::builder()
    .coordinator_url("http://coordinator:50051")
    .concurrency(100)
    .enable_js(true)
    .build()?;

worker.run().await?;
```

---

## v0.8.0 — "Deep Web Cartographer"

**Theme:** Advanced web graph analysis, dark web support, and specialized crawlers
**Expected:** Go where other crawlers can't

### Features

- [ ] **Tor / .onion Crawling**
  - SOCKS5 proxy routing through Tor network
  - .onion URL discovery and resolution
  - Tor circuit rotation for anonymity
  - Hidden service directory traversal
  - Clearnet ↔ darknet link mapping
  - Configurable Tor entry/exit node selection

- [ ] **API Discovery & Crawling**
  - REST API endpoint detection from JavaScript source analysis
  - GraphQL introspection and schema discovery
  - OpenAPI/Swagger spec detection and endpoint enumeration
  - API response crawling (JSON → structured Markdown)
  - Rate limit detection and respect per API endpoint
  - Authentication token refresh during API crawling

- [ ] **Deep Link Discovery**
  - JavaScript source code analysis for hidden URLs/endpoints
  - Webpack/Vite chunk analysis for lazy-loaded routes
  - Source map parsing for original route definitions
  - Parameter fuzzing for URL pattern discovery
  - Wordlist-based directory discovery (configurable wordlists)
  - Form action and AJAX endpoint enumeration

- [ ] **Archive & Versioning**
  - Wayback Machine integration — fetch historical versions
  - Content versioning — track changes over time with diffs
  - Git-like content history per URL
  - Archive.org CDX API integration for URL history
  - Side-by-side diff view (HTML report)
  - Content regression detection (removed pages, changed content)

- [ ] **Protocol Support Expansion**
  - FTP/SFTP directory listing and file crawling
  - WebSocket message capture during page rendering
  - Server-Sent Events (SSE) stream capture
  - gRPC-Web reflection and service discovery
  - RSS/Atom feed following with update checking

- [ ] **Web Graph Analytics**
  - Community detection in link graphs (Louvain algorithm)
  - Hub and authority scoring (HITS algorithm)
  - Strongly connected component identification
  - Cross-domain reference mapping
  - Link neighborhood analysis (what sites link to what)
  - Temporal graph analysis (how links change over time)

### CLI Additions

```bash
# Tor crawling
rehyke http://example.onion --tor --tor-circuits 3

# API discovery
rehyke https://app.example.com --discover-api \
    --api-auth "Bearer token123" --format json

# Historical crawl
rehyke https://example.com --archive --wayback \
    --from "2020-01-01" --to "2025-01-01" --snapshots monthly

# Web graph analysis
rehyke https://example.com --mode deep --graph \
    --community-detection --pagerank --export gephi
```

---

## v0.9.0 — "Rehyke Everywhere"

**Theme:** WebAssembly, edge deployment, embeddable engine, and platform expansion
**Expected:** Run rehyke anywhere — browser, edge, embedded, serverless

### Features

- [ ] **WebAssembly Build**
  - Compile rehyke-core to WASM (wasm32-wasi target)
  - Browser-based crawling via WASM + fetch API
  - Cloudflare Workers / Deno Deploy / Vercel Edge deployment
  - WASM-compatible HTTP client (no reqwest → use wasm-compatible fetch)
  - In-browser demo/playground on project website
  - < 2MB WASM binary size target

- [ ] **Embeddable Engine**
  - C FFI bindings via `cbindgen` for any language
  - Node.js bindings via NAPI-RS
  - Go bindings via CGo
  - Ruby bindings via Magnus
  - Java/Kotlin bindings via JNI
  - Shared library (.so/.dll/.dylib) distribution

- [ ] **Plugin System**
  - Plugin trait for custom extraction/transform/output stages
  - Plugin discovery and loading (dynamic .so/.dll or WASM plugins)
  - Plugin marketplace/registry concept
  - Built-in plugins: CSV export, SQL insert generation, Notion import
  - Plugin sandboxing via WASM for untrusted plugins
  - Plugin configuration via TOML/YAML

- [ ] **Serverless Deployment**
  - AWS Lambda handler (Rust native, cold start < 100ms)
  - Google Cloud Functions adapter
  - Azure Functions adapter
  - Event-driven crawling (trigger on webhook, schedule, queue message)
  - Stateless worker mode for serverless (all state in external store)

- [ ] **Mobile & Embedded**
  - Android library (.aar) via JNI
  - iOS framework via Swift bridging
  - Embedded Linux support (ARM/RISC-V cross-compilation)
  - Minimal memory footprint mode (< 10MB total for constrained environments)

- [ ] **Configuration as Code**
  - `rehyke.toml` project configuration file
  - Composable config profiles (base + override)
  - Environment variable interpolation: `proxy = "${PROXY_URL}"`
  - Config validation with helpful error messages
  - Config schema generation (JSON Schema for IDE autocomplete)

### Example: WASM in Browser

```javascript
import init, { Rehyke } from 'rehyke-wasm';

await init();

const crawler = new Rehyke({
  mode: 'lite',
  timeout: 5000,
});

const result = await crawler.crawl('https://example.com');
console.log(result.markdown);
console.log(result.metadata);
```

### Example: rehyke.toml

```toml
[project]
name = "my-crawl-project"
version = "1.0.0"

[defaults]
mode = "full"
concurrency = 50
enable_js = true
respect_robots_txt = true

[output]
format = "markdown"
structure = "mirror"
directory = "./output"

[stealth]
level = "medium"
rotate_ua = true
delay = { min = 500, max = 2000 }

[extraction.rules]
title = { css = "h1.page-title" }
price = { regex = '\\$(?P<amount>\\d+\\.\\d{2})' }
author = { schema_org = "Article.author.name" }

[[seeds]]
url = "https://docs.example.com"
mode = "full"
max_depth = 5

[[seeds]]
url = "https://blog.example.com"
mode = "full"
max_depth = 3
include = ["/posts/"]
```

---

## v1.0.0 — "Production Ready"

**Theme:** Stability, ecosystem, GUI, and enterprise features
**Expected:** The definitive web crawling platform

### Stability & Quality

- [ ] **Semantic Versioning Guarantee**
  - No breaking API changes in 1.x releases
  - Deprecation warnings for at least 2 minor versions before removal
  - Migration guides for every breaking change

- [ ] **Comprehensive Test Suite**
  - 1,000+ unit tests (up from 369)
  - Integration test suite against real-world sites (containerized)
  - Property-based testing (proptest) for parsers and normalizers
  - Fuzz testing for HTML/XML/JSON parsers
  - Performance regression tests (criterion benchmarks in CI)
  - Cross-platform CI (Linux, macOS, Windows, ARM64)

- [ ] **Security Hardening**
  - SSRF prevention — block private IP ranges by default (10.x, 172.16.x, 192.168.x, 127.x)
  - URL validation and sanitization
  - Response size limits (prevent memory exhaustion from malicious servers)
  - Certificate validation with pinning option
  - Audit logging for all network operations
  - SBOM (Software Bill of Materials) generation
  - Regular dependency vulnerability scanning

### GUI Dashboard

- [ ] **Rehyke Studio (Desktop App)**
  - Cross-platform desktop app (Tauri — Rust + WebView)
  - Visual crawl configuration builder
  - Real-time crawl progress with live statistics
  - Interactive link graph visualization (force-directed graph)
  - Result browser with search and filtering
  - Side-by-side page comparison (crawled vs live)
  - Export to multiple formats (Markdown, JSON, CSV, PDF, HTML)
  - Project management (save/load crawl configurations)
  - Dark/light theme

- [ ] **Web Dashboard (Optional)**
  - Lightweight web UI for distributed crawl monitoring
  - Job management interface
  - Worker status and health overview
  - Result exploration and search
  - Report viewer
  - REST API for programmatic access to dashboard data

### Enterprise Features

- [ ] **Role-Based Access Control**
  - User authentication for multi-user deployments
  - Role-based permissions (admin, operator, viewer)
  - API key management
  - Audit trail for all operations

- [ ] **Compliance & Governance**
  - GDPR-aware crawling (respect DNT, honor data deletion requests)
  - PII detection and redaction in crawled content
  - Crawl policy enforcement (allowed domains, rate limits, time windows)
  - Data retention policies with automatic cleanup
  - Export for legal/compliance review

- [ ] **SaaS-Ready Components**
  - Multi-tenant architecture
  - Usage metering and quota management
  - Billing event hooks
  - Tenant isolation for data and compute

### Ecosystem & Community

- [ ] **Documentation Site**
  - Interactive tutorial with live playground
  - API reference (auto-generated from doc comments)
  - Cookbook with 50+ real-world recipes
  - Architecture deep-dives
  - Video walkthroughs
  - Multi-language docs (EN, ES, ZH, JA, DE, FR)

- [ ] **CI/CD Integration**
  - GitHub Actions for automated releases
  - Multi-platform binary builds (Linux x86_64/ARM64, macOS x86_64/ARM64, Windows)
  - Automated PyPI publishing via maturin
  - Docker images (slim Alpine-based, < 50MB)
  - Homebrew formula, APT/RPM packages, Chocolatey package
  - Nix flake

- [ ] **Ecosystem Packages**
  - `rehyke-extract` — extraction rule library
  - `rehyke-store` — storage backend adapters
  - `rehyke-report` — report generation engine
  - `rehyke-schedule` — cron-like job scheduler
  - `rehyke-studio` — GUI application
  - `rehyke-cloud` — cloud deployment helpers

### Performance Targets (v1.0.0)

| Metric | Target |
|--------|--------|
| Single page (no JS) | < 100ms |
| Single page (with JS) | < 2s |
| 1,000 pages parallel | < 2 min |
| 10,000 pages parallel | < 15 min |
| 100,000 pages (distributed) | < 1 hour |
| 1,000,000 pages (distributed) | < 8 hours |
| Memory per page | < 3MB |
| WASM binary size | < 2MB |
| Docker image size | < 50MB |
| Cold start (serverless) | < 100ms |

---

## What Makes Rehyke Unique

A summary of differentiators no other crawler combines:

| Capability | Rehyke | crawl4ai | Scrapy | Colly | wget |
|-----------|--------|----------|--------|-------|------|
| Rust-native performance | **Yes** | No (Python) | No (Python) | Go | C |
| Python bindings (zero-copy) | **Yes** | Native | Native | No | No |
| JS rendering (headless) | **v0.2+** | Yes | Plugin | No | No |
| Universal format parser | **Yes** | HTML only | HTML only | HTML only | HTML only |
| TLS fingerprint rotation | **v0.3+** | No | No | No | No |
| Behavioral simulation | **v0.3+** | No | No | No | No |
| Regex + CSS + XPath extraction | **v0.4+** | CSS only | CSS+XPath | No | No |
| Autonomous crawl planning | **v0.5+** | No | No | No | No |
| Change detection / diff crawl | **v0.5+** | No | No | No | No |
| SEO audit built-in | **v0.6+** | No | No | No | No |
| Link graph + PageRank | **v0.6+** | No | No | No | No |
| Distributed multi-node | **v0.7+** | No | Scrapy Cloud | No | No |
| Tor / .onion support | **v0.8+** | No | No | No | No |
| API discovery | **v0.8+** | No | No | No | No |
| WASM / browser deployment | **v0.9+** | No | No | No | No |
| Plugin system | **v0.9+** | No | Yes | No | No |
| GUI dashboard | **v1.0** | No | No | No | No |
| Enterprise / multi-tenant | **v1.0** | No | Cloud only | No | No |

---

## Release Cadence

| Version | Estimated Timeline | Focus |
|---------|-------------------|-------|
| v0.1.0 | **Released** | Core engine |
| v0.2.0 | +6 weeks | JS rendering |
| v0.3.0 | +6 weeks | Stealth & auth |
| v0.4.0 | +8 weeks | Extraction engine |
| v0.5.0 | +8 weeks | Autonomous agent |
| v0.6.0 | +8 weeks | Intelligence layer |
| v0.7.0 | +10 weeks | Distributed crawling |
| v0.8.0 | +10 weeks | Deep web & analytics |
| v0.9.0 | +10 weeks | WASM & plugins |
| v1.0.0 | +12 weeks | Production stable |

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, coding standards, and how to submit patches. Every feature above is tracked as a milestone — pick one and start building.

---

*This roadmap is a living document. Priorities may shift based on community feedback and real-world usage patterns. Open an issue to suggest features or vote on priorities.*

*Built with Rust. Powered by ambition.*
