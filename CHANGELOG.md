# Changelog

All notable changes to the Rehyke project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

- Headless Chromium integration (renderer.rs stub ready)
- Autonomous crawl planner
- Regex rule engine for custom extraction
- Distributed crawling support

---

*For unreleased changes, see the [commit log](https://github.com/vrinda/rehyke/commits/main).*
