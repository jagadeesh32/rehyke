#[allow(unused_imports)]
use crate::fetcher::ContentType;
use crate::parser::{ContentNode, FeedItem, PageMetadata, ParsedDocument};

/// Configuration for Markdown output
#[derive(Debug, Clone)]
pub struct ConverterConfig {
    /// Include YAML frontmatter with metadata
    pub include_frontmatter: bool,
    /// Include footer with source attribution
    pub include_footer: bool,
    /// Maximum consecutive blank lines
    pub max_blank_lines: usize,
}

impl Default for ConverterConfig {
    fn default() -> Self {
        Self {
            include_frontmatter: true,
            include_footer: true,
            max_blank_lines: 2,
        }
    }
}

/// Convert a ParsedDocument into a Markdown string
pub fn to_markdown(doc: &ParsedDocument, config: &ConverterConfig) -> String {
    to_markdown_with_url(doc, "", config)
}

/// Convert a ParsedDocument into a Markdown string with an explicit URL for frontmatter.
pub fn to_markdown_with_url(doc: &ParsedDocument, url: &str, config: &ConverterConfig) -> String {
    let mut output = String::with_capacity(4096);

    // 1. Build YAML frontmatter from metadata
    if config.include_frontmatter {
        output.push_str(&build_frontmatter(url, &doc.metadata));
        output.push('\n');
    }

    // 2. Convert each ContentNode to Markdown
    for node in &doc.content_nodes {
        let rendered = render_node(node);
        output.push_str(&rendered);
    }

    // 4. Post-process
    output = strip_html_tags(&output);
    output = collapse_blank_lines(&output, config.max_blank_lines);
    output = trim_trailing_whitespace(&output);

    // 5. Add footer if configured
    if config.include_footer {
        // Ensure there is a blank line before the footer separator
        if !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str("\n---\n\n*Crawled by [Rehyke](https://github.com/user/rehyke)*\n");
    }

    // Ensure the file ends with a single newline
    let trimmed = output.trim_end().to_string();
    let mut final_output = trimmed;
    final_output.push('\n');

    final_output
}

/// Build YAML frontmatter from the document URL and metadata
fn build_frontmatter(url: &str, meta: &PageMetadata) -> String {
    let mut fm = String::from("---\n");

    fm.push_str(&format!("url: {}\n", url));

    if let Some(ref title) = meta.title {
        fm.push_str(&format!("title: {}\n", yaml_escape(title)));
    }
    if let Some(ref description) = meta.description {
        fm.push_str(&format!("description: {}\n", yaml_escape(description)));
    }
    if let Some(ref author) = meta.author {
        fm.push_str(&format!("author: {}\n", yaml_escape(author)));
    }
    if let Some(ref published) = meta.published_date {
        fm.push_str(&format!("published: {}\n", published));
    }
    if let Some(ref language) = meta.language {
        fm.push_str(&format!("language: {}\n", language));
    }
    if let Some(ref canonical) = meta.canonical_url {
        fm.push_str(&format!("canonical: {}\n", canonical));
    }

    fm.push_str("---\n");
    fm
}

/// Escape a YAML value string if it contains special characters
fn yaml_escape(value: &str) -> String {
    if value.contains(':')
        || value.contains('#')
        || value.contains('\'')
        || value.contains('"')
        || value.contains('\n')
        || value.starts_with(' ')
        || value.ends_with(' ')
        || value.starts_with('{')
        || value.starts_with('[')
        || value.starts_with('*')
        || value.starts_with('&')
        || value.starts_with('!')
        || value.starts_with('%')
        || value.starts_with('@')
        || value.starts_with('`')
    {
        // Wrap in double quotes and escape internal double quotes and backslashes
        let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{}\"", escaped)
    } else {
        value.to_string()
    }
}

/// Render a single ContentNode into its Markdown representation
fn render_node(node: &ContentNode) -> String {
    match node {
        ContentNode::Heading { level, text } => {
            let clamped = (*level).clamp(1, 6);
            let prefix = "#".repeat(clamped as usize);
            format!("{} {}\n\n", prefix, text.trim())
        }
        ContentNode::Paragraph(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                String::from("\n")
            } else {
                format!("{}\n\n", trimmed)
            }
        }
        ContentNode::Link { text, href } => {
            format!("[{}]({})", text, href)
        }
        ContentNode::Image { alt, src } => {
            format!("![{}]({})\n\n", alt, src)
        }
        ContentNode::Bold(text) => {
            format!("**{}**", text)
        }
        ContentNode::Italic(text) => {
            format!("*{}*", text)
        }
        ContentNode::Code(text) => {
            format!("`{}`", text)
        }
        ContentNode::CodeBlock { language, code } => {
            let lang = language.as_deref().unwrap_or("");
            format!("```{}\n{}\n```\n\n", lang, code)
        }
        ContentNode::UnorderedList(items) => {
            let mut result = String::new();
            for item in items {
                result.push_str(&format!("- {}\n", item));
            }
            result.push('\n');
            result
        }
        ContentNode::OrderedList(items) => {
            let mut result = String::new();
            for (i, item) in items.iter().enumerate() {
                result.push_str(&format!("{}. {}\n", i + 1, item));
            }
            result.push('\n');
            result
        }
        ContentNode::Blockquote(text) => {
            let mut result = String::new();
            for line in text.lines() {
                result.push_str(&format!("> {}\n", line));
            }
            result.push('\n');
            result
        }
        ContentNode::Table { headers, rows } => {
            let mut result = format_table(headers, rows);
            result.push('\n');
            result
        }
        ContentNode::HorizontalRule => String::from("---\n\n"),
        ContentNode::LineBreak => String::from("\n"),
        ContentNode::Strikethrough(text) => {
            format!("~~{}~~", text)
        }
        ContentNode::DefinitionList(items) => {
            let mut result = String::new();
            for (term, definition) in items {
                result.push_str(&format!("**{}:** {}\n\n", term, definition));
            }
            result
        }
        ContentNode::Media {
            media_type: _,
            title,
            src,
        } => {
            format!("[Media: {}]({})\n\n", title, src)
        }
        ContentNode::RawText(text) => text.clone(),
    }
}

/// Render a feed item into Markdown
#[allow(dead_code)]
fn render_feed_item(item: &FeedItem) -> String {
    let mut result = String::new();

    let title = item.title.as_deref().unwrap_or("Untitled");
    if let Some(ref link) = item.link {
        result.push_str(&format!("### [{}]({})\n\n", title, link));
    } else {
        result.push_str(&format!("### {}\n\n", title));
    }

    if let Some(ref description) = item.description {
        result.push_str(&format!("{}\n\n", description));
    }

    let mut meta_parts = Vec::new();
    if let Some(ref author) = item.author {
        meta_parts.push(format!("By {}", author));
    }
    if let Some(ref pub_date) = item.pub_date {
        meta_parts.push(pub_date.clone());
    }
    if !meta_parts.is_empty() {
        result.push_str(&format!("*{}*\n\n", meta_parts.join(" | ")));
    }

    result
}

/// Escape special Markdown characters in text content.
///
/// This escapes: `\`, `*`, `_`, `[`, `]`, `(`, `)`, `#`, `+`, `-`, `.`, `!`, `|`, `` ` ``
///
/// Note: This should NOT be used on code blocks, which should remain unescaped.
pub fn escape_markdown(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len() + text.len() / 4);
    for ch in text.chars() {
        match ch {
            '\\' | '*' | '_' | '[' | ']' | '(' | ')' | '#' | '+' | '-' | '.' | '!' | '|'
            | '`' => {
                escaped.push('\\');
                escaped.push(ch);
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}

/// Format a GFM (GitHub Flavored Markdown) table with proper alignment.
///
/// Produces output like:
/// ```text
/// | Header1 | Header2 |
/// |---------|---------|
/// | Cell1   | Cell2   |
/// ```
pub fn format_table(headers: &[String], rows: &[Vec<String>]) -> String {
    if headers.is_empty() {
        return String::new();
    }

    // Calculate the maximum width for each column
    let num_cols = headers.len();
    let mut col_widths = vec![0usize; num_cols];

    for (i, header) in headers.iter().enumerate() {
        col_widths[i] = col_widths[i].max(header.len());
    }

    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < num_cols {
                col_widths[i] = col_widths[i].max(cell.len());
            }
        }
    }

    // Ensure minimum width of 3 for the separator
    for w in &mut col_widths {
        *w = (*w).max(3);
    }

    let mut result = String::new();

    // Header row
    result.push('|');
    for (i, header) in headers.iter().enumerate() {
        result.push_str(&format!(" {:<width$} |", header, width = col_widths[i]));
    }
    result.push('\n');

    // Separator row
    result.push('|');
    for width in &col_widths {
        result.push_str(&format!("-{}-|", "-".repeat(*width)));
    }
    result.push('\n');

    // Data rows
    for row in rows {
        result.push('|');
        for i in 0..num_cols {
            let cell = row.get(i).map(|s| s.as_str()).unwrap_or("");
            result.push_str(&format!(" {:<width$} |", cell, width = col_widths[i]));
        }
        result.push('\n');
    }

    result
}

/// Collapse consecutive blank lines in the text to at most `max` blank lines.
///
/// A blank line is a line containing only whitespace. Consecutive blank lines
/// beyond the limit are removed.
pub fn collapse_blank_lines(text: &str, max: usize) -> String {
    let mut result = String::with_capacity(text.len());
    let mut consecutive_blanks = 0usize;
    let lines: Vec<&str> = text.split('\n').collect();
    let last_idx = lines.len().saturating_sub(1);

    for (i, line) in lines.iter().enumerate() {
        // The trailing empty element from split('\n') on "...\n" is not a real
        // blank line — skip it to avoid injecting an extra newline.
        if i == last_idx && line.is_empty() {
            break;
        }

        if line.trim().is_empty() {
            consecutive_blanks += 1;
            if consecutive_blanks <= max {
                result.push('\n');
            }
        } else {
            consecutive_blanks = 0;
            result.push_str(line);
            result.push('\n');
        }
    }

    result
}

/// Remove any remaining HTML tags from text.
///
/// This strips tags like `<p>`, `</p>`, `<br/>`, `<div class="foo">`, etc.
/// Self-closing tags and tags with attributes are also removed.
pub fn strip_html_tags(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut inside_tag = false;

    for ch in text.chars() {
        if ch == '<' {
            inside_tag = true;
        } else if ch == '>' {
            if inside_tag {
                inside_tag = false;
            } else {
                // Stray '>' not preceded by '<', keep it
                result.push(ch);
            }
        } else if !inside_tag {
            result.push(ch);
        }
    }

    result
}

/// Trim trailing whitespace from each line in the text
fn trim_trailing_whitespace(text: &str) -> String {
    text.lines()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{ContentNode, FeedItem, PageMetadata, ParsedDocument};

    fn make_metadata() -> PageMetadata {
        PageMetadata {
            title: Some("Test Page".to_string()),
            description: Some("A test page description".to_string()),
            author: Some("Author Name".to_string()),
            published_date: Some("2024-01-15".to_string()),
            language: Some("en".to_string()),
            canonical_url: Some("https://example.com/page".to_string()),
            og_image: None,
            keywords: vec!["test".to_string(), "page".to_string()],
        }
    }

    fn make_doc(content: Vec<ContentNode>) -> ParsedDocument {
        ParsedDocument {
            metadata: make_metadata(),
            content_nodes: content,
            content_type: ContentType::Html,
        }
    }

    /// URL used for frontmatter in tests.
    const TEST_URL: &str = "https://example.com/page";

    // -------------------------------------------------------------------------
    // Heading conversion (all levels)
    // -------------------------------------------------------------------------
    #[test]
    fn test_heading_levels() {
        for level in 1..=6u8 {
            let node = ContentNode::Heading {
                level,
                text: format!("Heading {}", level),
            };
            let rendered = render_node(&node);
            let prefix = "#".repeat(level as usize);
            assert_eq!(rendered, format!("{} Heading {}\n\n", prefix, level));
        }
    }

    #[test]
    fn test_heading_level_clamped() {
        let node = ContentNode::Heading {
            level: 7,
            text: "Deep heading".to_string(),
        };
        let rendered = render_node(&node);
        assert!(rendered.starts_with("######"));
        assert!(!rendered.starts_with("#######"));
    }

    // -------------------------------------------------------------------------
    // Paragraph conversion
    // -------------------------------------------------------------------------
    #[test]
    fn test_paragraph() {
        let node = ContentNode::Paragraph("Hello, world!".to_string());
        let rendered = render_node(&node);
        assert_eq!(rendered, "Hello, world!\n\n");
    }

    #[test]
    fn test_paragraph_trims_whitespace() {
        let node = ContentNode::Paragraph("  Hello  ".to_string());
        let rendered = render_node(&node);
        assert_eq!(rendered, "Hello\n\n");
    }

    #[test]
    fn test_empty_paragraph() {
        let node = ContentNode::Paragraph("   ".to_string());
        let rendered = render_node(&node);
        assert_eq!(rendered, "\n");
    }

    // -------------------------------------------------------------------------
    // Link and image conversion
    // -------------------------------------------------------------------------
    #[test]
    fn test_link() {
        let node = ContentNode::Link {
            text: "Example".to_string(),
            href: "https://example.com".to_string(),
        };
        let rendered = render_node(&node);
        assert_eq!(rendered, "[Example](https://example.com)");
    }

    #[test]
    fn test_image() {
        let node = ContentNode::Image {
            alt: "An image".to_string(),
            src: "https://example.com/img.png".to_string(),
        };
        let rendered = render_node(&node);
        assert_eq!(rendered, "![An image](https://example.com/img.png)\n\n");
    }

    // -------------------------------------------------------------------------
    // Code and code block conversion
    // -------------------------------------------------------------------------
    #[test]
    fn test_inline_code() {
        let node = ContentNode::Code("let x = 42;".to_string());
        let rendered = render_node(&node);
        assert_eq!(rendered, "`let x = 42;`");
    }

    #[test]
    fn test_code_block_with_language() {
        let node = ContentNode::CodeBlock {
            language: Some("rust".to_string()),
            code: "fn main() {\n    println!(\"hello\");\n}".to_string(),
        };
        let rendered = render_node(&node);
        assert_eq!(
            rendered,
            "```rust\nfn main() {\n    println!(\"hello\");\n}\n```\n\n"
        );
    }

    #[test]
    fn test_code_block_without_language() {
        let node = ContentNode::CodeBlock {
            language: None,
            code: "some code".to_string(),
        };
        let rendered = render_node(&node);
        assert_eq!(rendered, "```\nsome code\n```\n\n");
    }

    // -------------------------------------------------------------------------
    // List conversion (ordered and unordered)
    // -------------------------------------------------------------------------
    #[test]
    fn test_unordered_list() {
        let node = ContentNode::UnorderedList(vec![
            "Item one".to_string(),
            "Item two".to_string(),
            "Item three".to_string(),
        ]);
        let rendered = render_node(&node);
        assert_eq!(rendered, "- Item one\n- Item two\n- Item three\n\n");
    }

    #[test]
    fn test_ordered_list() {
        let node = ContentNode::OrderedList(vec![
            "First".to_string(),
            "Second".to_string(),
            "Third".to_string(),
        ]);
        let rendered = render_node(&node);
        assert_eq!(rendered, "1. First\n2. Second\n3. Third\n\n");
    }

    // -------------------------------------------------------------------------
    // Table formatting
    // -------------------------------------------------------------------------
    #[test]
    fn test_format_table_basic() {
        let headers = vec!["Name".to_string(), "Age".to_string()];
        let rows = vec![
            vec!["Alice".to_string(), "30".to_string()],
            vec!["Bob".to_string(), "25".to_string()],
        ];
        let table = format_table(&headers, &rows);
        let lines: Vec<&str> = table.lines().collect();

        assert_eq!(lines.len(), 4);
        assert!(lines[0].contains("Name"));
        assert!(lines[0].contains("Age"));
        assert!(lines[1].contains("---"));
        assert!(lines[2].contains("Alice"));
        assert!(lines[2].contains("30"));
        assert!(lines[3].contains("Bob"));
        assert!(lines[3].contains("25"));
    }

    #[test]
    fn test_format_table_empty_headers() {
        let headers: Vec<String> = vec![];
        let rows: Vec<Vec<String>> = vec![];
        let table = format_table(&headers, &rows);
        assert_eq!(table, "");
    }

    #[test]
    fn test_format_table_missing_cells() {
        let headers = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let rows = vec![vec!["1".to_string()]]; // row has fewer cells than headers
        let table = format_table(&headers, &rows);
        let lines: Vec<&str> = table.lines().collect();
        assert_eq!(lines.len(), 3);
        // The row should have 3 columns, with empty cells padded
        assert!(lines[2].contains("1"));
    }

    #[test]
    fn test_table_node_rendering() {
        let node = ContentNode::Table {
            headers: vec!["Col1".to_string(), "Col2".to_string()],
            rows: vec![vec!["A".to_string(), "B".to_string()]],
        };
        let rendered = render_node(&node);
        assert!(rendered.contains("| Col1"));
        assert!(rendered.contains("| A"));
        assert!(rendered.contains("---"));
    }

    // -------------------------------------------------------------------------
    // Frontmatter generation
    // -------------------------------------------------------------------------
    #[test]
    fn test_frontmatter_full_metadata() {
        let meta = make_metadata();
        let fm = build_frontmatter("https://example.com/page", &meta);

        assert!(fm.starts_with("---\n"));
        assert!(fm.ends_with("---\n"));
        assert!(fm.contains("url: https://example.com/page"));
        assert!(fm.contains("title: Test Page"));
        assert!(fm.contains("description: A test page description"));
        assert!(fm.contains("author: Author Name"));
        assert!(fm.contains("published: 2024-01-15"));
        assert!(fm.contains("language: en"));
        assert!(fm.contains("canonical: https://example.com/page"));
    }

    #[test]
    fn test_frontmatter_minimal_metadata() {
        let meta = PageMetadata::default();
        let fm = build_frontmatter("https://example.com", &meta);

        assert!(fm.starts_with("---\n"));
        assert!(fm.contains("url: https://example.com"));
        // Optional fields should not appear
        assert!(!fm.contains("title:"));
        assert!(!fm.contains("description:"));
        assert!(!fm.contains("author:"));
    }

    #[test]
    fn test_frontmatter_yaml_escaping() {
        let meta = PageMetadata {
            title: Some("Title: with colon".to_string()),
            description: Some("Normal description".to_string()),
            ..Default::default()
        };
        let fm = build_frontmatter("https://example.com", &meta);
        // Title should be escaped because it contains ':'
        assert!(fm.contains("title: \"Title: with colon\""));
    }

    // -------------------------------------------------------------------------
    // Blank line collapsing
    // -------------------------------------------------------------------------
    #[test]
    fn test_collapse_blank_lines_basic() {
        let input = "Hello\n\n\n\n\nWorld\n";
        let result = collapse_blank_lines(input, 2);
        // Should have at most 2 blank lines (3 newlines between content)
        assert_eq!(result, "Hello\n\n\nWorld\n");
    }

    #[test]
    fn test_collapse_blank_lines_within_limit() {
        let input = "A\n\nB\n";
        let result = collapse_blank_lines(input, 2);
        assert_eq!(result, "A\n\nB\n");
    }

    #[test]
    fn test_collapse_blank_lines_zero_max() {
        let input = "A\n\n\nB\n";
        let result = collapse_blank_lines(input, 0);
        assert_eq!(result, "A\nB\n");
    }

    // -------------------------------------------------------------------------
    // Markdown escaping
    // -------------------------------------------------------------------------
    #[test]
    fn test_escape_markdown_special_chars() {
        let input = "Hello *world* and _underscores_";
        let escaped = escape_markdown(input);
        assert_eq!(escaped, "Hello \\*world\\* and \\_underscores\\_");
    }

    #[test]
    fn test_escape_markdown_brackets() {
        let input = "[link](url)";
        let escaped = escape_markdown(input);
        assert_eq!(escaped, "\\[link\\]\\(url\\)");
    }

    #[test]
    fn test_escape_markdown_all_chars() {
        let input = r"\*_[]()#+-.!|`";
        let escaped = escape_markdown(input);
        assert_eq!(escaped, r"\\\*\_\[\]\(\)\#\+\-\.\!\|\`");
    }

    #[test]
    fn test_escape_markdown_plain_text() {
        let input = "Just normal text here";
        let escaped = escape_markdown(input);
        assert_eq!(escaped, input);
    }

    // -------------------------------------------------------------------------
    // strip_html_tags
    // -------------------------------------------------------------------------
    #[test]
    fn test_strip_html_tags_basic() {
        let input = "<p>Hello <strong>world</strong></p>";
        let result = strip_html_tags(input);
        assert_eq!(result, "Hello world");
    }

    #[test]
    fn test_strip_html_tags_self_closing() {
        let input = "Line one<br/>Line two";
        let result = strip_html_tags(input);
        assert_eq!(result, "Line oneLine two");
    }

    #[test]
    fn test_strip_html_tags_with_attributes() {
        let input = r#"<div class="container">Content</div>"#;
        let result = strip_html_tags(input);
        assert_eq!(result, "Content");
    }

    #[test]
    fn test_strip_html_tags_no_tags() {
        let input = "No HTML here";
        let result = strip_html_tags(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_strip_html_tags_nested() {
        let input = "<div><p>Nested <em>tags</em></p></div>";
        let result = strip_html_tags(input);
        assert_eq!(result, "Nested tags");
    }

    // -------------------------------------------------------------------------
    // Blockquote
    // -------------------------------------------------------------------------
    #[test]
    fn test_blockquote_single_line() {
        let node = ContentNode::Blockquote("A quote".to_string());
        let rendered = render_node(&node);
        assert_eq!(rendered, "> A quote\n\n");
    }

    #[test]
    fn test_blockquote_multiline() {
        let node = ContentNode::Blockquote("Line one\nLine two\nLine three".to_string());
        let rendered = render_node(&node);
        assert_eq!(rendered, "> Line one\n> Line two\n> Line three\n\n");
    }

    // -------------------------------------------------------------------------
    // Inline formatting
    // -------------------------------------------------------------------------
    #[test]
    fn test_bold() {
        let node = ContentNode::Bold("important".to_string());
        assert_eq!(render_node(&node), "**important**");
    }

    #[test]
    fn test_italic() {
        let node = ContentNode::Italic("emphasis".to_string());
        assert_eq!(render_node(&node), "*emphasis*");
    }

    #[test]
    fn test_strikethrough() {
        let node = ContentNode::Strikethrough("removed".to_string());
        assert_eq!(render_node(&node), "~~removed~~");
    }

    // -------------------------------------------------------------------------
    // Other nodes
    // -------------------------------------------------------------------------
    #[test]
    fn test_horizontal_rule() {
        let node = ContentNode::HorizontalRule;
        assert_eq!(render_node(&node), "---\n\n");
    }

    #[test]
    fn test_line_break() {
        let node = ContentNode::LineBreak;
        assert_eq!(render_node(&node), "\n");
    }

    #[test]
    fn test_definition_list() {
        let node = ContentNode::DefinitionList(vec![
            ("Term1".to_string(), "Definition1".to_string()),
            ("Term2".to_string(), "Definition2".to_string()),
        ]);
        let rendered = render_node(&node);
        assert_eq!(
            rendered,
            "**Term1:** Definition1\n\n**Term2:** Definition2\n\n"
        );
    }

    #[test]
    fn test_media() {
        let node = ContentNode::Media {
            media_type: "video".to_string(),
            title: "My Video".to_string(),
            src: "https://example.com/video.mp4".to_string(),
        };
        let rendered = render_node(&node);
        assert_eq!(
            rendered,
            "[Media: My Video](https://example.com/video.mp4)\n\n"
        );
    }

    #[test]
    fn test_raw_text() {
        let node = ContentNode::RawText("just raw text".to_string());
        assert_eq!(render_node(&node), "just raw text");
    }

    // -------------------------------------------------------------------------
    // Full document conversion end-to-end
    // -------------------------------------------------------------------------
    #[test]
    fn test_full_document_with_frontmatter_and_footer() {
        let doc = make_doc(vec![
            ContentNode::Heading {
                level: 1,
                text: "Welcome".to_string(),
            },
            ContentNode::Paragraph("This is a test document.".to_string()),
            ContentNode::UnorderedList(vec!["Alpha".to_string(), "Beta".to_string()]),
        ]);

        let config = ConverterConfig::default();
        let md = to_markdown_with_url(&doc, TEST_URL, &config);

        // Frontmatter present
        assert!(md.starts_with("---\n"));
        assert!(md.contains("url: https://example.com/page"));
        assert!(md.contains("title: Test Page"));

        // Content present
        assert!(md.contains("# Welcome"));
        assert!(md.contains("This is a test document."));
        assert!(md.contains("- Alpha"));
        assert!(md.contains("- Beta"));

        // Footer present
        assert!(md.contains("*Crawled by [Rehyke](https://github.com/user/rehyke)*"));

        // Ends with single newline
        assert!(md.ends_with('\n'));
        assert!(!md.ends_with("\n\n"));
    }

    #[test]
    fn test_full_document_no_frontmatter_no_footer() {
        let doc = make_doc(vec![ContentNode::Paragraph(
            "Just a paragraph.".to_string(),
        )]);

        let config = ConverterConfig {
            include_frontmatter: false,
            include_footer: false,
            max_blank_lines: 2,
        };
        let md = to_markdown(&doc, &config);

        // No frontmatter
        assert!(!md.starts_with("---"));
        // No footer
        assert!(!md.contains("Crawled by"));
        // Content present
        assert!(md.contains("Just a paragraph."));
        // Ends with single newline
        assert!(md.ends_with('\n'));
    }

    #[test]
    fn test_full_document_blank_line_collapsing() {
        let doc = make_doc(vec![
            ContentNode::Paragraph("First".to_string()),
            ContentNode::Paragraph("".to_string()),
            ContentNode::Paragraph("".to_string()),
            ContentNode::Paragraph("".to_string()),
            ContentNode::Paragraph("Second".to_string()),
        ]);

        let config = ConverterConfig {
            include_frontmatter: false,
            include_footer: false,
            max_blank_lines: 2,
        };
        let md = to_markdown(&doc, &config);

        // Count consecutive newlines - should not exceed 3 (which means 2 blank lines)
        let max_consecutive = md
            .as_bytes()
            .windows(4)
            .filter(|w| w == b"\n\n\n\n")
            .count();
        assert_eq!(max_consecutive, 0, "Should not have more than 2 consecutive blank lines");
    }

    #[test]
    fn test_full_document_strips_html() {
        let doc = make_doc(vec![ContentNode::RawText(
            "Hello <b>world</b> and <i>more</i>".to_string(),
        )]);

        let config = ConverterConfig {
            include_frontmatter: false,
            include_footer: false,
            max_blank_lines: 2,
        };
        let md = to_markdown(&doc, &config);

        assert!(!md.contains('<'));
        assert!(!md.contains('>'));
        assert!(md.contains("Hello world and more"));
    }

    #[test]
    fn test_full_document_with_table() {
        let doc = make_doc(vec![
            ContentNode::Heading {
                level: 2,
                text: "Data Table".to_string(),
            },
            ContentNode::Table {
                headers: vec!["Name".to_string(), "Value".to_string()],
                rows: vec![
                    vec!["foo".to_string(), "42".to_string()],
                    vec!["bar".to_string(), "99".to_string()],
                ],
            },
        ]);

        let config = ConverterConfig {
            include_frontmatter: false,
            include_footer: false,
            max_blank_lines: 2,
        };
        let md = to_markdown(&doc, &config);

        assert!(md.contains("## Data Table"));
        assert!(md.contains("| Name"));
        assert!(md.contains("| foo"));
        assert!(md.contains("| bar"));
    }

    #[test]
    fn test_full_document_with_code_block() {
        let doc = make_doc(vec![ContentNode::CodeBlock {
            language: Some("python".to_string()),
            code: "print(\"hello\")".to_string(),
        }]);

        let config = ConverterConfig {
            include_frontmatter: false,
            include_footer: false,
            max_blank_lines: 2,
        };
        let md = to_markdown(&doc, &config);

        assert!(md.contains("```python"));
        assert!(md.contains("print(\"hello\")"));
        assert!(md.contains("```\n"));
    }

    #[test]
    fn test_render_feed_item_with_all_fields() {
        let item = FeedItem {
            title: Some("Article One".to_string()),
            link: Some("https://example.com/1".to_string()),
            description: Some("First article".to_string()),
            pub_date: Some("2024-01-01".to_string()),
            author: Some("Jane".to_string()),
        };

        let rendered = render_feed_item(&item);

        assert!(rendered.contains("[Article One](https://example.com/1)"));
        assert!(rendered.contains("First article"));
        assert!(rendered.contains("By Jane"));
        assert!(rendered.contains("2024-01-01"));
    }

    #[test]
    fn test_render_feed_item_minimal() {
        let item = FeedItem {
            title: Some("Article Two".to_string()),
            link: None,
            description: None,
            pub_date: None,
            author: None,
        };

        let rendered = render_feed_item(&item);
        assert!(rendered.contains("### Article Two"));
    }
}
