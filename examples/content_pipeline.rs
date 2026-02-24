//! Content Pipeline Example
//!
//! Demonstrates using Rehyke's internal modules directly to build a custom
//! content processing pipeline. This example shows:
//!   - Crawling multiple URLs sequentially
//!   - Using the parser module for custom HTML parsing
//!   - Using the converter module for custom Markdown generation
//!   - URL normalization via the utils module
//!   - Parsing robots.txt files
//!   - Parsing sitemap XML
//!   - Link extraction and classification
//!
//! Run with:
//!   cargo run --example content_pipeline

use url::Url;

use rehyke_core::converter::{self, ConverterConfig};
use rehyke_core::extractor;
use rehyke_core::parser::{self, ParseConfig};
use rehyke_core::robots::RobotsTxt;
use rehyke_core::sitemap::Sitemap;
use rehyke_core::utils;
use rehyke_core::{Rehyke, ScanMode};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== Rehyke Content Pipeline Example ===\n");

    // =====================================================================
    // Section 1: URL normalization utilities
    // =====================================================================
    println!("--- URL Normalization ---\n");

    let raw_urls = [
        "HTTP://WWW.Example.COM:80/Path/?b=2&a=1#fragment",
        "https://example.com/blog/../about/",
        "  example.com/page  ",
        "//cdn.example.com/assets/style.css",
        "https://EXAMPLE.COM/%41%42%43",
    ];

    for raw in &raw_urls {
        // sanitize_url handles missing schemes, whitespace, and validation.
        match utils::sanitize_url(raw) {
            Some(sanitized) => {
                let parsed = Url::parse(&sanitized)?;
                // normalize_url deduplicates by lowering host, removing default
                // ports, stripping fragments, sorting query params, etc.
                let normalized = utils::normalize_url(&parsed, true);
                println!("  Input:      {}", raw);
                println!("  Sanitized:  {}", sanitized);
                println!("  Normalized: {}", normalized);
                // Generate filesystem-safe names from URLs.
                println!("  Filename:   {}", utils::url_to_filename(&parsed));
                println!("  Slug:       {}", utils::url_to_slug(&parsed));
                println!();
            }
            None => {
                println!("  Input:      {} -> INVALID", raw);
                println!();
            }
        }
    }

    // Domain comparison utilities.
    let base = Url::parse("https://example.com/docs")?;
    let same = Url::parse("https://example.com/blog")?;
    let sub = Url::parse("https://blog.example.com/post")?;
    let ext = Url::parse("https://other.com/page")?;

    println!("Domain comparison (base = {}):", base);
    println!(
        "  {} is_same_domain: {}",
        same,
        utils::is_same_domain(&same, &base)
    );
    println!(
        "  {} is_subdomain:   {}",
        sub,
        utils::is_subdomain(&sub, &base)
    );
    println!(
        "  {} is_same_domain: {}",
        ext,
        utils::is_same_domain(&ext, &base)
    );
    println!(
        "  root_domain(base): {:?}",
        utils::root_domain(&base)
    );
    println!();

    // Resolve relative URLs against a base.
    let page_base = Url::parse("https://example.com/docs/v2/guide")?;
    let relatives = ["../api", "/home", "sibling", "//cdn.example.com/file.js"];
    println!("URL resolution (base = {}):", page_base);
    for rel in &relatives {
        match utils::resolve_url(&page_base, rel) {
            Some(resolved) => println!("  {} -> {}", rel, resolved),
            None => println!("  {} -> FAILED", rel),
        }
    }
    println!();

    // =====================================================================
    // Section 2: Robots.txt parsing
    // =====================================================================
    println!("--- Robots.txt Parsing ---\n");

    let robots_content = "\
User-agent: *
Disallow: /admin/
Disallow: /private/
Allow: /admin/public

User-agent: Rehyke
Disallow: /slow/
Allow: /

Sitemap: https://example.com/sitemap.xml
Sitemap: https://example.com/sitemap-blog.xml
Crawl-delay: 2
";

    let robots = RobotsTxt::parse(robots_content);

    let test_paths = [
        "/",
        "/about",
        "/admin/settings",
        "/admin/public",
        "/private/data",
        "/slow/endpoint",
    ];

    println!("Robots.txt rules (Rehyke user-agent):");
    for path in &test_paths {
        let allowed = robots.is_allowed(path);
        let symbol = if allowed { "ALLOW" } else { "DENY " };
        println!("  {} {}", symbol, path);
    }
    println!();

    println!("Sitemaps declared in robots.txt:");
    for sitemap_url in robots.sitemaps() {
        println!("  - {}", sitemap_url);
    }
    if let Some(delay) = robots.crawl_delay() {
        println!("Crawl-delay: {} seconds", delay);
    }
    println!(
        "Robots.txt URL for example.com: {}",
        RobotsTxt::robots_url(&Url::parse("https://example.com/any/page")?)
    );
    println!();

    // =====================================================================
    // Section 3: Sitemap XML parsing
    // =====================================================================
    println!("--- Sitemap Parsing ---\n");

    let sitemap_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url>
    <loc>https://example.com/</loc>
    <lastmod>2025-06-15</lastmod>
    <changefreq>daily</changefreq>
    <priority>1.0</priority>
  </url>
  <url>
    <loc>https://example.com/blog</loc>
    <lastmod>2025-06-14</lastmod>
    <changefreq>weekly</changefreq>
    <priority>0.8</priority>
  </url>
  <url>
    <loc>https://example.com/docs/api</loc>
    <priority>0.6</priority>
  </url>
</urlset>"#;

    let sitemap = Sitemap::parse(sitemap_xml)?;
    println!("Sitemap entries: {}", sitemap.entries.len());
    for entry in &sitemap.entries {
        println!(
            "  {} (lastmod={}, freq={}, priority={})",
            entry.loc,
            entry.lastmod.as_deref().unwrap_or("n/a"),
            entry.changefreq.as_deref().unwrap_or("n/a"),
            entry
                .priority
                .map(|p| format!("{:.1}", p))
                .unwrap_or_else(|| "n/a".to_string()),
        );
    }
    println!();

    // Sitemap index example.
    let index_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <sitemap><loc>https://example.com/sitemap-pages.xml</loc></sitemap>
  <sitemap><loc>https://example.com/sitemap-blog.xml</loc></sitemap>
</sitemapindex>"#;

    let index = Sitemap::parse(index_xml)?;
    println!("Sitemap index sub-sitemaps: {}", index.sub_sitemaps.len());
    for sub in &index.sub_sitemaps {
        println!("  - {}", sub);
    }
    println!();

    // Common sitemap URL probes for a domain.
    let probe_base = Url::parse("https://example.com/any/page")?;
    println!("Common sitemap URLs to probe:");
    for url in Sitemap::common_sitemap_urls(&probe_base) {
        println!("  - {}", url);
    }
    println!();

    // =====================================================================
    // Section 4: Crawl and use parser/converter directly
    // =====================================================================
    println!("--- Custom Parsing and Conversion Pipeline ---\n");

    let urls_to_crawl = [
        "https://httpbin.org/html",
        "https://httpbin.org/robots.txt",
    ];

    for seed_url in &urls_to_crawl {
        println!("Processing: {}", seed_url);

        let results = match Rehyke::crawl(seed_url, ScanMode::Lite).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("  Skipping (error): {}", e);
                continue;
            }
        };

        let result = match results.first() {
            Some(r) => r,
            None => {
                println!("  No results.");
                continue;
            }
        };

        println!("  Title:        {}", result.title);
        println!("  Status:       {}", result.status_code);
        println!("  Content-Type: {}", result.content_type);
        println!("  Render:       {:?}", result.render_method);

        // Access parsed metadata fields.
        let meta = &result.metadata;
        if let Some(ref desc) = meta.description {
            println!("  Description:  {}", desc);
        }
        if let Some(ref lang) = meta.language {
            println!("  Language:     {}", lang);
        }
        if !meta.keywords.is_empty() {
            println!("  Keywords:     {}", meta.keywords.join(", "));
        }

        // =====================================================================
        // Section 5: Link extraction and classification
        // =====================================================================
        let links = &result.links;
        println!("  Links:");
        println!("    Internal:   {}", links.internal.len());
        println!("    External:   {}", links.external.len());
        println!("    Subdomains: {}", links.subdomains.len());
        println!("    Resources:  {}", links.resources.len());
        println!("    Feeds:      {}", links.feeds.len());
        println!("    Sitemaps:   {}", links.sitemaps.len());

        // Show a preview of the Markdown output (first 200 chars).
        let preview_len = 200.min(result.markdown.len());
        let preview = &result.markdown[..preview_len];
        println!("  Markdown preview:");
        for line in preview.lines().take(8) {
            println!("    {}", line);
        }

        println!();
    }

    // =====================================================================
    // Section 6: Direct parser and converter usage
    // =====================================================================
    println!("--- Direct Parser + Converter Usage ---\n");

    // Parse raw HTML directly without going through the full crawl pipeline.
    let raw_html = r#"
    <html>
    <head>
        <title>Manual Parse Example</title>
        <meta name="description" content="A hand-crafted HTML snippet">
        <meta name="author" content="Rehyke Team">
    </head>
    <body>
        <h1>Welcome to Rehyke</h1>
        <p>This is a <strong>manually parsed</strong> HTML document.</p>
        <ul>
            <li>Fast and async</li>
            <li>Configurable depth</li>
            <li>Regex-powered filtering</li>
        </ul>
        <a href="https://github.com/user/rehyke">GitHub</a>
        <a href="/docs/api">API Docs</a>
    </body>
    </html>
    "#;

    let parse_config = ParseConfig {
        clean_navigation: true,
        clean_footers: true,
        clean_ads: true,
        clean_comments: true,
        extract_metadata: true,
    };

    let parsed = parser::parse(
        raw_html,
        &rehyke_core::fetcher::ContentType::Html,
        &parse_config,
    )?;

    println!("Parsed metadata:");
    println!("  Title:       {:?}", parsed.metadata.title);
    println!("  Description: {:?}", parsed.metadata.description);
    println!("  Author:      {:?}", parsed.metadata.author);
    println!("  Nodes:       {} content nodes", parsed.content_nodes.len());
    println!();

    // Convert to Markdown with custom config.
    let converter_config = ConverterConfig {
        include_frontmatter: true,
        include_footer: false,
        max_blank_lines: 1,
    };
    let markdown = converter::to_markdown_with_url(
        &parsed,
        "https://example.com/manual",
        &converter_config,
    );

    println!("Generated Markdown ({} chars):", markdown.len());
    println!("---");
    println!("{}", markdown);
    println!("---");

    // Extract and classify links from the raw HTML.
    let html_doc = scraper::Html::parse_document(raw_html);
    let base_url = Url::parse("https://example.com/")?;
    let extracted = extractor::extract_links(&html_doc, &base_url);

    println!("Links extracted from raw HTML:");
    println!("  Internal: {:?}", extracted.internal);
    println!("  External: {:?}", extracted.external);
    println!();

    println!("=== Pipeline Complete ===");
    Ok(())
}
