//! Basic Crawl Example
//!
//! Demonstrates the simplest way to use Rehyke: the one-shot `Rehyke::crawl()`
//! API. This example fetches a single URL, converts it to Markdown, and prints
//! a summary including the page title, word count, and a short content preview.
//!
//! Run with:
//!   cargo run --example basic_crawl

use rehyke_core::{Rehyke, ScanMode};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // =====================================================================
    // Step 1: Choose a URL to crawl
    // =====================================================================
    // You can change this to any publicly accessible URL.
    let url = "https://httpbin.org/html";

    println!("=== Rehyke Basic Crawl Example ===\n");
    println!("Target URL: {}\n", url);

    // =====================================================================
    // Step 2: Perform a one-shot crawl
    // =====================================================================
    // `Rehyke::crawl()` is the simplest API. It creates a default config
    // with the chosen ScanMode, fetches the page, parses the HTML, and
    // converts it to Markdown -- all in one call.
    //
    // ScanMode options:
    //   - Lite: shallow crawl (max_depth=2, max_pages=100)
    //   - Full: balanced crawl (max_depth=5, max_pages=1000)  [default]
    //   - Deep: exhaustive crawl (max_depth=50, max_pages=50000)
    let results = match Rehyke::crawl(url, ScanMode::Lite).await {
        Ok(results) => {
            println!("Crawl succeeded! Got {} result(s).\n", results.len());
            results
        }
        Err(e) => {
            // The error type is `RehykeError`, which covers HTTP errors,
            // timeouts, DNS failures, parse errors, and more.
            eprintln!("Crawl failed: {}", e);
            return Err(e.into());
        }
    };

    // =====================================================================
    // Step 3: Inspect the results
    // =====================================================================
    // Each `CrawlResult` contains the URL, title, Markdown content,
    // structured metadata, extracted links, HTTP status, and more.
    for (i, result) in results.iter().enumerate() {
        println!("--- Result {} ---", i + 1);
        println!("  URL:          {}", result.url);
        println!("  Title:        {}", result.title);
        println!("  Status:       {}", result.status_code);
        println!("  Content-Type: {}", result.content_type);
        println!("  Crawl Depth:  {}", result.depth);
        println!("  Crawled At:   {}", result.crawled_at);

        // Word count: split the markdown on whitespace and count tokens.
        let word_count = result.markdown.split_whitespace().count();
        println!("  Word Count:   {}", word_count);

        // Show a short preview of the Markdown (first 300 characters).
        let preview_len = 300.min(result.markdown.len());
        let preview = &result.markdown[..preview_len];
        println!("\n  Markdown Preview:\n  {}", preview.replace('\n', "\n  "));

        // =====================================================================
        // Step 4: Inspect extracted links
        // =====================================================================
        let links = &result.links;
        println!("\n  Links Found:");
        println!("    Internal:   {}", links.internal.len());
        println!("    External:   {}", links.external.len());
        println!("    Subdomains: {}", links.subdomains.len());
        println!("    Resources:  {}", links.resources.len());

        if !links.external.is_empty() {
            println!("\n  First 5 external links:");
            for link in links.external.iter().take(5) {
                println!("    - {}", link);
            }
        }
    }

    println!("\n=== Done ===");
    Ok(())
}
