use crate::config::{FileStructure, OutputMode};
use crate::error::{RehykeError, Result};
use crate::extractor::ExtractedLinks;
use crate::parser::PageMetadata;
use crate::utils;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{debug, info};
use url::Url;

/// Method used to render the page
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RenderMethod {
    /// Page was fetched as static HTML without JavaScript execution
    Static,
    /// Page was rendered via a headless browser with JavaScript execution
    JavaScript,
}

/// Result of crawling a single page
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlResult {
    /// The URL that was crawled
    pub url: String,
    /// Page title extracted from the document
    pub title: String,
    /// Markdown representation of the page content
    pub markdown: String,
    /// Structured metadata parsed from the page
    pub metadata: PageMetadata,
    /// Links discovered on the page
    pub links: ExtractedLinks,
    /// Timestamp when the page was crawled
    pub crawled_at: DateTime<Utc>,
    /// HTTP status code of the response
    pub status_code: u16,
    /// Content-Type header value from the response
    pub content_type: String,
    /// Crawl depth at which this page was discovered
    pub depth: u32,
    /// How the page was rendered
    pub render_method: RenderMethod,
}

/// Manages output of crawl results to memory, individual files, or a single file.
pub struct OutputHandler {
    mode: OutputMode,
    results: Vec<CrawlResult>,
}

impl OutputHandler {
    /// Create a new output handler for the given output mode.
    pub fn new(mode: OutputMode) -> Self {
        Self {
            mode,
            results: Vec::new(),
        }
    }

    /// Process a single crawl result.
    ///
    /// Depending on the configured [`OutputMode`]:
    /// - **Memory** -- the result is stored in an internal buffer.
    /// - **Files** -- the markdown is written to an individual file derived from
    ///   the crawled URL (flat slug or mirrored path structure).
    /// - **SingleFile** -- the markdown is appended to a single output file with
    ///   `---` separators between pages.
    pub fn handle_result(&mut self, result: CrawlResult) -> Result<()> {
        match &self.mode {
            OutputMode::Memory => {
                debug!(url = %result.url, "Storing crawl result in memory");
            }
            OutputMode::Files {
                output_dir,
                structure,
            } => {
                let parsed_url = Url::parse(&result.url).map_err(|e| {
                    RehykeError::ConfigError {
                        message: format!("Invalid URL in crawl result: {}", e),
                    }
                })?;
                let file_path = url_to_file_path(&parsed_url, output_dir, structure);
                let content = format_page_content(&result);
                write_to_file(&file_path, &content)?;
                info!(
                    url = %result.url,
                    path = %file_path.display(),
                    "Wrote crawl result to file"
                );
            }
            OutputMode::SingleFile { output_path } => {
                let content = format_page_content(&result);
                // If the file already exists and is non-empty, prepend a separator.
                let needs_separator = output_path.exists()
                    && std::fs::metadata(output_path)
                        .map(|m| m.len() > 0)
                        .unwrap_or(false);
                let to_append = if needs_separator {
                    format!("\n---\n\n{}", content)
                } else {
                    content
                };
                append_to_file(output_path, &to_append)?;
                info!(
                    url = %result.url,
                    path = %output_path.display(),
                    "Appended crawl result to single file"
                );
            }
        }

        self.results.push(result);
        Ok(())
    }

    /// Finish processing and return all collected crawl results.
    ///
    /// Results are tracked regardless of the output mode, so the caller always
    /// receives the full list.
    pub fn finalize(self) -> Result<Vec<CrawlResult>> {
        info!(count = self.results.len(), "Finalizing output handler");
        Ok(self.results)
    }

    /// Get a reference to the currently collected results.
    pub fn results(&self) -> &[CrawlResult] {
        &self.results
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Format a crawl result as markdown page content with a YAML-style front-matter
/// header containing the URL, title, and crawl timestamp.
fn format_page_content(result: &CrawlResult) -> String {
    format!(
        "# {}\n\nURL: {}\nCrawled at: {}\n\n{}\n",
        result.title, result.url, result.crawled_at, result.markdown
    )
}

/// Write `content` to `path`, creating parent directories as needed.
fn write_to_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    debug!(path = %path.display(), "Wrote file");
    Ok(())
}

/// Append `content` to `path`, creating parent directories and the file as
/// needed.
fn append_to_file(path: &Path, content: &str) -> Result<()> {
    use std::io::Write;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(content.as_bytes())?;
    debug!(path = %path.display(), "Appended to file");
    Ok(())
}

/// Convert a URL into an output file path based on the chosen
/// [`FileStructure`].
///
/// - **Flat** -- `{output_dir}/{slug}.md` where the slug is derived from
///   [`utils::url_to_slug`].
/// - **Mirror** -- `{output_dir}/{host}/{url_path}/index.md`, mirroring the
///   URL's path hierarchy on disk.
fn url_to_file_path(url: &Url, output_dir: &Path, structure: &FileStructure) -> PathBuf {
    match structure {
        FileStructure::Flat => {
            let slug = utils::url_to_slug(url);
            output_dir.join(format!("{}.md", slug))
        }
        FileStructure::Mirror => {
            let host = url.host_str().unwrap_or("unknown");
            let path = url.path();

            // Strip the leading slash so we don't create an empty path
            // component when joining.
            let trimmed = path.strip_prefix('/').unwrap_or(path);

            let mut file_path = output_dir.join(host);

            if trimmed.is_empty() {
                // Root page -> output_dir/host/index.md
                file_path.push("index.md");
            } else {
                // Push each segment of the URL path.
                for segment in trimmed.split('/') {
                    if !segment.is_empty() {
                        file_path.push(segment);
                    }
                }
                file_path.push("index.md");
            }

            file_path
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::FileStructure;
    use crate::extractor::ExtractedLinks;
    use crate::parser::PageMetadata;
    use chrono::Utc;
    use std::fs;
    use url::Url;

    /// Build a minimal [`CrawlResult`] for testing purposes.
    fn make_result(url: &str, title: &str, markdown: &str) -> CrawlResult {
        CrawlResult {
            url: url.to_string(),
            title: title.to_string(),
            markdown: markdown.to_string(),
            metadata: PageMetadata::default(),
            links: ExtractedLinks::default(),
            crawled_at: Utc::now(),
            status_code: 200,
            content_type: "text/html".to_string(),
            depth: 0,
            render_method: RenderMethod::Static,
        }
    }

    // -----------------------------------------------------------------------
    // Memory mode
    // -----------------------------------------------------------------------

    #[test]
    fn test_memory_mode_collects_results() {
        let mut handler = OutputHandler::new(OutputMode::Memory);
        let r1 = make_result("https://example.com/a", "Page A", "Content A");
        let r2 = make_result("https://example.com/b", "Page B", "Content B");

        handler.handle_result(r1).unwrap();
        handler.handle_result(r2).unwrap();

        assert_eq!(handler.results().len(), 2);
        assert_eq!(handler.results()[0].url, "https://example.com/a");
        assert_eq!(handler.results()[1].url, "https://example.com/b");
    }

    #[test]
    fn test_memory_mode_finalize_returns_all() {
        let mut handler = OutputHandler::new(OutputMode::Memory);
        handler
            .handle_result(make_result("https://example.com", "Home", "# Home"))
            .unwrap();

        let results = handler.finalize().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Home");
    }

    // -----------------------------------------------------------------------
    // File path generation
    // -----------------------------------------------------------------------

    #[test]
    fn test_url_to_file_path_flat() {
        let url = Url::parse("https://example.com/blog/post").unwrap();
        let output_dir = Path::new("/tmp/output");
        let path = url_to_file_path(&url, output_dir, &FileStructure::Flat);

        let slug = utils::url_to_slug(&url);
        assert_eq!(path, output_dir.join(format!("{}.md", slug)));
        assert!(path.to_string_lossy().ends_with(".md"));
    }

    #[test]
    fn test_url_to_file_path_flat_root() {
        let url = Url::parse("https://example.com/").unwrap();
        let output_dir = Path::new("/tmp/output");
        let path = url_to_file_path(&url, output_dir, &FileStructure::Flat);

        assert!(path.to_string_lossy().ends_with(".md"));
    }

    #[test]
    fn test_url_to_file_path_mirror() {
        let url = Url::parse("https://example.com/blog/post").unwrap();
        let output_dir = Path::new("/tmp/output");
        let path = url_to_file_path(&url, output_dir, &FileStructure::Mirror);

        let expected = output_dir.join("example.com/blog/post/index.md");
        assert_eq!(path, expected);
    }

    #[test]
    fn test_url_to_file_path_mirror_root() {
        let url = Url::parse("https://example.com/").unwrap();
        let output_dir = Path::new("/tmp/output");
        let path = url_to_file_path(&url, output_dir, &FileStructure::Mirror);

        let expected = output_dir.join("example.com/index.md");
        assert_eq!(path, expected);
    }

    #[test]
    fn test_url_to_file_path_mirror_deep() {
        let url = Url::parse("https://docs.example.com/api/v2/reference").unwrap();
        let output_dir = Path::new("/tmp/output");
        let path = url_to_file_path(&url, output_dir, &FileStructure::Mirror);

        let expected = output_dir.join("docs.example.com/api/v2/reference/index.md");
        assert_eq!(path, expected);
    }

    // -----------------------------------------------------------------------
    // URL to slug for filenames
    // -----------------------------------------------------------------------

    #[test]
    fn test_slug_used_for_flat_filename() {
        let url = Url::parse("https://example.com/About-Us/Team").unwrap();
        let slug = utils::url_to_slug(&url);

        // Slugs should be lowercase and use hyphens.
        assert_eq!(slug, slug.to_lowercase());
        assert!(!slug.contains('/'));
        assert!(!slug.is_empty());
    }

    #[test]
    fn test_slug_no_special_characters() {
        let url = Url::parse("https://example.com/path?query=1&other=2#frag").unwrap();
        let slug = utils::url_to_slug(&url);

        assert!(!slug.contains('?'));
        assert!(!slug.contains('&'));
        assert!(!slug.contains('#'));
        assert!(!slug.contains('='));
    }

    // -----------------------------------------------------------------------
    // CrawlResult serialization
    // -----------------------------------------------------------------------

    #[test]
    fn test_crawl_result_serialization_roundtrip() {
        let result = make_result(
            "https://example.com/page",
            "Test Page",
            "Some markdown content",
        );

        let json = serde_json::to_string(&result).expect("serialization should succeed");
        let deserialized: CrawlResult =
            serde_json::from_str(&json).expect("deserialization should succeed");

        assert_eq!(deserialized.url, result.url);
        assert_eq!(deserialized.title, result.title);
        assert_eq!(deserialized.markdown, result.markdown);
        assert_eq!(deserialized.status_code, result.status_code);
        assert_eq!(deserialized.content_type, result.content_type);
        assert_eq!(deserialized.depth, result.depth);
    }

    #[test]
    fn test_crawl_result_render_method_serialization() {
        let static_result = make_result("https://example.com", "Static", "body");
        let json = serde_json::to_string(&static_result).unwrap();
        assert!(json.contains("Static"));

        let mut js_result = make_result("https://example.com", "JS", "body");
        js_result.render_method = RenderMethod::JavaScript;
        let json = serde_json::to_string(&js_result).unwrap();
        assert!(json.contains("JavaScript"));
    }

    // -----------------------------------------------------------------------
    // File mode (integration-style tests using temp directories)
    // -----------------------------------------------------------------------

    #[test]
    fn test_files_mode_writes_flat() {
        let tmp = tempdir();
        let mut handler = OutputHandler::new(OutputMode::Files {
            output_dir: tmp.clone(),
            structure: FileStructure::Flat,
        });

        let result = make_result("https://example.com/page", "Page", "# Hello");
        handler.handle_result(result).unwrap();

        // Verify a .md file was created.
        let entries: Vec<_> = fs::read_dir(&tmp)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(entries.len(), 1);
        let path = entries[0].path();
        assert!(path.extension().map(|e| e == "md").unwrap_or(false));

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("# Page"));
        assert!(content.contains("# Hello"));
    }

    #[test]
    fn test_files_mode_writes_mirror() {
        let tmp = tempdir();
        let mut handler = OutputHandler::new(OutputMode::Files {
            output_dir: tmp.clone(),
            structure: FileStructure::Mirror,
        });

        let result = make_result("https://example.com/docs/guide", "Guide", "# Guide");
        handler.handle_result(result).unwrap();

        let expected = tmp.join("example.com/docs/guide/index.md");
        assert!(expected.exists(), "Expected file at {:?}", expected);

        let content = fs::read_to_string(&expected).unwrap();
        assert!(content.contains("# Guide"));
    }

    #[test]
    fn test_single_file_mode_appends() {
        let tmp = tempdir();
        let output_path = tmp.join("all.md");

        let mut handler = OutputHandler::new(OutputMode::SingleFile {
            output_path: output_path.clone(),
        });

        handler
            .handle_result(make_result("https://example.com/a", "Page A", "Content A"))
            .unwrap();
        handler
            .handle_result(make_result("https://example.com/b", "Page B", "Content B"))
            .unwrap();

        let content = fs::read_to_string(&output_path).unwrap();
        assert!(content.contains("Page A"));
        assert!(content.contains("Page B"));
        assert!(content.contains("---"), "Pages should be separated by ---");
    }

    #[test]
    fn test_results_ref_returns_current_state() {
        let mut handler = OutputHandler::new(OutputMode::Memory);
        assert!(handler.results().is_empty());

        handler
            .handle_result(make_result("https://example.com", "Home", "body"))
            .unwrap();
        assert_eq!(handler.results().len(), 1);
    }

    // -----------------------------------------------------------------------
    // Helper: create a temporary directory that is cleaned up on drop-ish
    // -----------------------------------------------------------------------

    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rehyke_test_{}", std::process::id()));
        let unique = dir.join(format!("{}", rand::random::<u64>()));
        fs::create_dir_all(&unique).expect("failed to create temp dir");
        unique
    }
}
