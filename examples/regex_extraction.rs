//! Regex Extraction Example
//!
//! Demonstrates how to use regex patterns to extract structured data from
//! crawled content. After fetching a page with Rehyke, this example applies
//! multiple regex patterns to find email addresses, phone numbers, URLs,
//! prices, dates, and social media handles.
//!
//! Also shows how to use regex for URL filtering (include/exclude patterns
//! in the config) and for content relevance scoring via keyword matching.
//!
//! Run with:
//!   cargo run --example regex_extraction

use regex::Regex;

use rehyke_core::{CrawlConfigBuilder, Rehyke, ScanMode};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== Rehyke Regex Extraction Example ===\n");

    // =====================================================================
    // Section 1: URL filtering with regex (config-level)
    // =====================================================================
    // Rehyke's CrawlConfig supports include_patterns and exclude_patterns,
    // which are regex strings evaluated against each discovered URL.

    let config = CrawlConfigBuilder::new()
        .mode(ScanMode::Lite)
        // Only crawl pages under /html or /anything paths.
        .include_patterns(vec![
            r"https://[^/]+/html.*".to_string(),
            r"https://[^/]+/anything.*".to_string(),
        ])
        // Skip image and asset URLs.
        .exclude_patterns(vec![
            r".*\.(png|jpg|gif|css|js)(\?.*)?$".to_string(),
            r".*/static/.*".to_string(),
        ])
        .build();

    println!("URL include patterns: {:?}", config.include_patterns);
    println!("URL exclude patterns: {:?}", config.exclude_patterns);
    println!();

    // Demonstrate pattern matching against sample URLs.
    let test_urls = [
        "https://example.com/html/page1",
        "https://example.com/anything/test",
        "https://example.com/static/logo.png",
        "https://example.com/about",
    ];

    let include_regexes: Vec<Regex> = config
        .include_patterns
        .iter()
        .filter_map(|p| Regex::new(p).ok())
        .collect();
    let exclude_regexes: Vec<Regex> = config
        .exclude_patterns
        .iter()
        .filter_map(|p| Regex::new(p).ok())
        .collect();

    println!("URL filter results:");
    for url in &test_urls {
        let included = include_regexes.is_empty()
            || include_regexes.iter().any(|re| re.is_match(url));
        let excluded = exclude_regexes.iter().any(|re| re.is_match(url));
        let status = if excluded {
            "EXCLUDED"
        } else if included {
            "INCLUDED"
        } else {
            "SKIPPED (no include match)"
        };
        println!("  {} -> {}", url, status);
    }
    println!();

    // =====================================================================
    // Section 2: Crawl a page and extract data with regex
    // =====================================================================
    let url = "https://httpbin.org/html";
    println!("Crawling: {}\n", url);

    let results = Rehyke::crawl(url, ScanMode::Lite).await?;

    let content = if let Some(result) = results.first() {
        println!("Fetched: {} (status {})", result.title, result.status_code);
        println!(
            "Content length: {} chars, {} words\n",
            result.markdown.len(),
            result.markdown.split_whitespace().count()
        );
        result.markdown.clone()
    } else {
        println!("No results returned.");
        return Ok(());
    };

    // For demonstration, we augment the crawled content with sample data
    // that showcases all our regex patterns.
    let sample_data = format!(
        "{}\n\n\
         --- Sample Data for Regex Demo ---\n\
         Contact us at support@example.com or sales@company.org\n\
         Call +1 (555) 123-4567 or 800.555.9876\n\
         Visit [our blog](https://blog.example.com) and [docs](https://docs.example.com/api)\n\
         Prices: $19.99, $1,299.00, $5.00\n\
         Dates: 2025-01-15, 2024-12-31, 2026-06-01\n\
         Follow us: @rehyke @rustlang @tokikitten\n",
        content
    );

    // =====================================================================
    // Section 3: Define regex patterns for extraction
    // =====================================================================

    // Email addresses
    let email_re = Regex::new(r"[\w.\-]+@[\w.\-]+\.\w+")?;

    // Markdown-style links: [text](url)
    let md_link_re = Regex::new(r"\[([^\]]+)\]\(([^)]+)\)")?;

    // Prices in USD format
    let price_re = Regex::new(r"\$\d{1,3}(?:,\d{3})*(?:\.\d{2})?")?;

    // ISO dates (YYYY-MM-DD)
    let date_re = Regex::new(r"\d{4}-\d{2}-\d{2}")?;

    // Social media handles (@username, 1-15 chars)
    let handle_re = Regex::new(r"@[\w]{1,15}")?;

    // =====================================================================
    // Section 4: Extract and display results
    // =====================================================================

    println!("=== Extracted Data ===\n");

    // --- Emails ---
    let emails: Vec<&str> = email_re
        .find_iter(&sample_data)
        .map(|m| m.as_str())
        .collect();
    println!("Email Addresses ({} found):", emails.len());
    for email in &emails {
        println!("  - {}", email);
    }
    println!();

    // --- Phone Numbers (using named captures) ---
    // We rebuild with named captures for a richer demonstration.
    let phone_named_re = Regex::new(
        r"\+?1?\s*\(?(?P<area>\d{3})\)?[\s.\-]*(?P<exchange>\d{3})[\s.\-]*(?P<subscriber>\d{4})",
    )?;
    let phones: Vec<String> = phone_named_re
        .captures_iter(&sample_data)
        .map(|cap| {
            format!(
                "({}) {}-{}",
                &cap["area"], &cap["exchange"], &cap["subscriber"]
            )
        })
        .collect();
    println!("Phone Numbers ({} found):", phones.len());
    for phone in &phones {
        println!("  - {}", phone);
    }
    println!();

    // --- Markdown Links ---
    println!("Markdown Links:");
    for cap in md_link_re.captures_iter(&sample_data) {
        let text = &cap[1];
        let href = &cap[2];
        println!("  - text: {:30} url: {}", format!("\"{}\"", text), href);
    }
    println!();

    // --- Prices ---
    let prices: Vec<&str> = price_re
        .find_iter(&sample_data)
        .map(|m| m.as_str())
        .collect();
    println!("Prices ({} found):", prices.len());
    for price in &prices {
        println!("  - {}", price);
    }
    println!();

    // --- Dates ---
    let dates: Vec<&str> = date_re
        .find_iter(&sample_data)
        .map(|m| m.as_str())
        .collect();
    println!("ISO Dates ({} found):", dates.len());
    for date in &dates {
        println!("  - {}", date);
    }
    println!();

    // --- Social Media Handles ---
    let handles: Vec<&str> = handle_re
        .find_iter(&sample_data)
        .map(|m| m.as_str())
        .collect();
    println!("Social Media Handles ({} found):", handles.len());
    for handle in &handles {
        println!("  - {}", handle);
    }
    println!();

    // =====================================================================
    // Section 5: Content relevance scoring with regex
    // =====================================================================
    // Count how many times certain keywords appear to score content
    // relevance. This is useful for filtering or ranking crawled pages.

    println!("=== Content Relevance Scoring ===\n");

    let keywords = [
        ("rust", Regex::new(r"(?i)\brust\b")?),
        ("web", Regex::new(r"(?i)\bweb\b")?),
        ("crawler", Regex::new(r"(?i)\bcrawler?\b")?),
        ("html", Regex::new(r"(?i)\bhtml\b")?),
        ("data", Regex::new(r"(?i)\bdata\b")?),
        ("example", Regex::new(r"(?i)\bexample\b")?),
    ];

    let mut total_score = 0usize;
    for (keyword, re) in &keywords {
        let count = re.find_iter(&sample_data).count();
        total_score += count;
        let bar = "#".repeat(count.min(40));
        println!("  {:12} {:3} matches  {}", keyword, count, bar);
    }
    println!("\n  Total relevance score: {}", total_score);

    // =====================================================================
    // Section 6: Summary table
    // =====================================================================
    println!("\n=== Extraction Summary ===\n");
    println!("  +-----------------------+-------+");
    println!("  | Data Type             | Count |");
    println!("  +-----------------------+-------+");
    println!("  | Emails                | {:>5} |", emails.len());
    println!("  | Phone Numbers         | {:>5} |", phones.len());
    println!("  | Markdown Links        | {:>5} |", md_link_re.find_iter(&sample_data).count());
    println!("  | Prices                | {:>5} |", prices.len());
    println!("  | ISO Dates             | {:>5} |", dates.len());
    println!("  | Social Media Handles  | {:>5} |", handles.len());
    println!("  +-----------------------+-------+");

    println!("\n=== Done ===");
    Ok(())
}
