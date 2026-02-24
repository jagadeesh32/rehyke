//! Content parser for the Rehyke web crawler.
//!
//! This module parses raw HTML/XML/RSS/Atom/JSON content into structured data
//! for the converter. It supports multiple content types and produces a unified
//! [`ParsedDocument`] representation that downstream components can transform
//! into Markdown or other output formats.

use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::error::{RehykeError, Result};
use crate::fetcher::ContentType;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the parser.
///
/// Controls which parts of a document are cleaned (removed) before content
/// extraction and whether metadata extraction is enabled.
#[derive(Debug, Clone)]
pub struct ParseConfig {
    /// Remove `<nav>` elements.
    pub clean_navigation: bool,
    /// Remove `<footer>` elements.
    pub clean_footers: bool,
    /// Remove elements whose class or id matches common ad patterns.
    pub clean_ads: bool,
    /// Remove elements that look like comment sections.
    pub clean_comments: bool,
    /// Whether to extract `<meta>` tag metadata.
    pub extract_metadata: bool,
}

impl Default for ParseConfig {
    fn default() -> Self {
        Self {
            clean_navigation: true,
            clean_footers: true,
            clean_ads: true,
            clean_comments: true,
            extract_metadata: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// Metadata extracted from a page.
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

/// A parsed content node for conversion.
///
/// Each variant represents a semantic element that can be rendered to Markdown
/// or another output format by the converter module.
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

/// Result of parsing a document.
#[derive(Debug, Clone)]
pub struct ParsedDocument {
    pub metadata: PageMetadata,
    pub content_nodes: Vec<ContentNode>,
    pub content_type: ContentType,
}

/// RSS/Atom feed item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedItem {
    pub title: Option<String>,
    pub link: Option<String>,
    pub description: Option<String>,
    pub pub_date: Option<String>,
    pub author: Option<String>,
}

/// Parsed feed (RSS or Atom).
#[derive(Debug, Clone)]
pub struct ParsedFeed {
    pub title: Option<String>,
    pub link: Option<String>,
    pub description: Option<String>,
    pub items: Vec<FeedItem>,
}

// ---------------------------------------------------------------------------
// Top-level dispatcher
// ---------------------------------------------------------------------------

/// Parse raw content into a [`ParsedDocument`] based on its content type.
///
/// Dispatches to the appropriate format-specific parser.
pub fn parse(body: &str, content_type: &ContentType, config: &ParseConfig) -> Result<ParsedDocument> {
    match content_type {
        ContentType::Html | ContentType::Xhtml => parse_html(body, config),
        ContentType::Rss => parse_rss(body),
        ContentType::Atom => parse_atom(body),
        ContentType::Xml | ContentType::Sitemap => parse_xml(body),
        ContentType::Json | ContentType::JsonLd => parse_json(body),
        ContentType::Svg => Ok(ParsedDocument {
            metadata: PageMetadata::default(),
            content_nodes: vec![ContentNode::RawText(body.to_string())],
            content_type: ContentType::Svg,
        }),
        ContentType::PlainText => Ok(ParsedDocument {
            metadata: PageMetadata::default(),
            content_nodes: vec![ContentNode::RawText(body.to_string())],
            content_type: ContentType::PlainText,
        }),
        ContentType::Other(mime) => {
            warn!("Unsupported content type: {mime}, treating as plain text");
            Ok(ParsedDocument {
                metadata: PageMetadata::default(),
                content_nodes: vec![ContentNode::RawText(body.to_string())],
                content_type: content_type.clone(),
            })
        }
    }
}

// ---------------------------------------------------------------------------
// HTML parser
// ---------------------------------------------------------------------------

/// Parse an HTML document into a [`ParsedDocument`].
///
/// Uses the `scraper` crate to build a DOM, extracts metadata from `<meta>`
/// tags and `<link>` elements, cleans unwanted elements according to `config`,
/// then walks the remaining DOM tree to produce [`ContentNode`]s.
pub fn parse_html(html: &str, config: &ParseConfig) -> Result<ParsedDocument> {
    let document = Html::parse_document(html);

    // Extract metadata before cleaning.
    let metadata = if config.extract_metadata {
        extract_metadata(&document)
    } else {
        PageMetadata::default()
    };

    // Build content nodes from the cleaned document.
    let content_nodes = extract_content_nodes(&document, config);

    debug!(
        "Parsed HTML: title={:?}, nodes={}",
        metadata.title,
        content_nodes.len()
    );

    Ok(ParsedDocument {
        metadata,
        content_nodes,
        content_type: ContentType::Html,
    })
}

/// Extract [`PageMetadata`] from the HTML document.
fn extract_metadata(document: &Html) -> PageMetadata {
    let mut meta = PageMetadata::default();

    // Title from <title> tag.
    if let Ok(sel) = Selector::parse("title") {
        if let Some(el) = document.select(&sel).next() {
            let text = collect_text(&el);
            if !text.is_empty() {
                meta.title = Some(text);
            }
        }
    }

    // Language from <html lang="...">.
    if let Ok(sel) = Selector::parse("html") {
        if let Some(el) = document.select(&sel).next() {
            if let Some(lang) = el.value().attr("lang") {
                meta.language = Some(lang.to_string());
            }
        }
    }

    // Canonical URL from <link rel="canonical">.
    if let Ok(sel) = Selector::parse(r#"link[rel="canonical"]"#) {
        if let Some(el) = document.select(&sel).next() {
            if let Some(href) = el.value().attr("href") {
                meta.canonical_url = Some(href.to_string());
            }
        }
    }

    // <meta> tags — description, author, keywords, og:*, twitter:*, article:*.
    if let Ok(sel) = Selector::parse("meta") {
        for el in document.select(&sel) {
            let name = el
                .value()
                .attr("name")
                .or_else(|| el.value().attr("property"))
                .unwrap_or("");
            let content = el.value().attr("content").unwrap_or("");

            if content.is_empty() {
                continue;
            }

            match name.to_lowercase().as_str() {
                "description" | "og:description" | "twitter:description" => {
                    if meta.description.is_none() {
                        meta.description = Some(content.to_string());
                    }
                }
                "author" | "article:author" => {
                    if meta.author.is_none() {
                        meta.author = Some(content.to_string());
                    }
                }
                "keywords" => {
                    if meta.keywords.is_empty() {
                        meta.keywords = content
                            .split(',')
                            .map(|k| k.trim().to_string())
                            .filter(|k| !k.is_empty())
                            .collect();
                    }
                }
                "og:image" | "twitter:image" => {
                    if meta.og_image.is_none() {
                        meta.og_image = Some(content.to_string());
                    }
                }
                "og:title" | "twitter:title" => {
                    if meta.title.is_none() {
                        meta.title = Some(content.to_string());
                    }
                }
                "article:published_time" | "date" | "dc.date" => {
                    if meta.published_date.is_none() {
                        meta.published_date = Some(content.to_string());
                    }
                }
                _ => {}
            }
        }
    }

    meta
}

// ---------------------------------------------------------------------------
// DOM walking — content extraction
// ---------------------------------------------------------------------------

/// CSS selectors for elements that should always be removed.
const ALWAYS_REMOVE_TAGS: &[&str] = &["script", "style", "noscript"];

/// Common class/id substrings that indicate ad containers.
const AD_PATTERNS: &[&str] = &[
    "ad-", "ad_", "ads-", "ads_", "advert", "adsense", "adslot",
    "banner-ad", "sponsor", "promoted", "dfp-", "doubleclick",
    "google_ads", "gpt-ad",
];

/// Common class/id substrings that indicate comment sections.
const COMMENT_PATTERNS: &[&str] = &[
    "comment", "disqus", "discourse", "reply", "respond",
];

/// Return `true` if the element should be skipped (not turned into content nodes).
fn should_skip_element(el: &ElementRef, config: &ParseConfig) -> bool {
    let tag = el.value().name();

    // Always remove script, style, noscript.
    if ALWAYS_REMOVE_TAGS.contains(&tag) {
        return true;
    }

    // Configurable removals by tag name.
    if config.clean_navigation && tag == "nav" {
        return true;
    }
    if config.clean_footers && tag == "footer" {
        return true;
    }
    if config.clean_navigation && tag == "header" {
        return true;
    }

    // Class / id based filtering.
    let class = el.value().attr("class").unwrap_or("");
    let id = el.value().attr("id").unwrap_or("");
    let combined = format!("{} {}", class, id).to_lowercase();

    if config.clean_ads {
        for pat in AD_PATTERNS {
            if combined.contains(pat) {
                return true;
            }
        }
    }

    if config.clean_comments {
        for pat in COMMENT_PATTERNS {
            if combined.contains(pat) {
                return true;
            }
        }
    }

    false
}

/// Walk the DOM and build a flat list of [`ContentNode`]s.
fn extract_content_nodes(document: &Html, config: &ParseConfig) -> Vec<ContentNode> {
    let mut nodes = Vec::new();

    // Find <body>; fall back to the root if missing.
    let body_selector = Selector::parse("body").expect("valid selector");
    let root_elements: Vec<ElementRef> = if let Some(body) = document.select(&body_selector).next()
    {
        body.children()
            .filter_map(ElementRef::wrap)
            .collect()
    } else {
        // No <body> — iterate top-level elements.
        document
            .root_element()
            .children()
            .filter_map(ElementRef::wrap)
            .collect()
    };

    for child in root_elements {
        walk_element(&child, config, &mut nodes);
    }

    nodes
}

/// Recursively process a single element and its children.
fn walk_element(el: &ElementRef, config: &ParseConfig, nodes: &mut Vec<ContentNode>) {
    if should_skip_element(el, config) {
        return;
    }

    let tag = el.value().name();

    match tag {
        // Headings.
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let level = tag[1..].parse::<u8>().unwrap_or(1);
            let text = collect_text(el);
            if !text.is_empty() {
                nodes.push(ContentNode::Heading { level, text });
            }
        }

        // Paragraphs.
        "p" => {
            let text = collect_text(el);
            if !text.is_empty() {
                nodes.push(ContentNode::Paragraph(text));
            }
        }

        // Links (top-level; inline links inside paragraphs are folded into text).
        "a" => {
            let href = el.value().attr("href").unwrap_or("").to_string();
            let text = collect_text(el);
            if !text.is_empty() || !href.is_empty() {
                nodes.push(ContentNode::Link {
                    text: if text.is_empty() { href.clone() } else { text },
                    href,
                });
            }
        }

        // Images.
        "img" => {
            let alt = el.value().attr("alt").unwrap_or("").to_string();
            let src = el.value().attr("src").unwrap_or("").to_string();
            if !src.is_empty() {
                nodes.push(ContentNode::Image { alt, src });
            }
        }

        // Bold.
        "strong" | "b" => {
            let text = collect_text(el);
            if !text.is_empty() {
                nodes.push(ContentNode::Bold(text));
            }
        }

        // Italic.
        "em" | "i" => {
            let text = collect_text(el);
            if !text.is_empty() {
                nodes.push(ContentNode::Italic(text));
            }
        }

        // Inline code.
        "code" => {
            // If the parent is <pre>, the <pre> handler deals with it.
            let text = collect_text(el);
            if !text.is_empty() {
                nodes.push(ContentNode::Code(text));
            }
        }

        // Preformatted / code block.
        "pre" => {
            let code_sel = Selector::parse("code").expect("valid selector");
            if let Some(code_el) = el.select(&code_sel).next() {
                let language = code_el
                    .value()
                    .attr("class")
                    .and_then(extract_language_from_class);
                let code = collect_text(&code_el);
                nodes.push(ContentNode::CodeBlock { language, code });
            } else {
                let code = collect_text(el);
                if !code.is_empty() {
                    nodes.push(ContentNode::CodeBlock {
                        language: None,
                        code,
                    });
                }
            }
        }

        // Unordered list.
        "ul" => {
            let items = collect_list_items(el);
            if !items.is_empty() {
                nodes.push(ContentNode::UnorderedList(items));
            }
        }

        // Ordered list.
        "ol" => {
            let items = collect_list_items(el);
            if !items.is_empty() {
                nodes.push(ContentNode::OrderedList(items));
            }
        }

        // Blockquote.
        "blockquote" => {
            let text = collect_text(el);
            if !text.is_empty() {
                nodes.push(ContentNode::Blockquote(text));
            }
        }

        // Table.
        "table" => {
            let (headers, rows) = parse_table(el);
            if !headers.is_empty() || !rows.is_empty() {
                nodes.push(ContentNode::Table { headers, rows });
            }
        }

        // Horizontal rule.
        "hr" => {
            nodes.push(ContentNode::HorizontalRule);
        }

        // Line break.
        "br" => {
            nodes.push(ContentNode::LineBreak);
        }

        // Strikethrough.
        "del" | "s" => {
            let text = collect_text(el);
            if !text.is_empty() {
                nodes.push(ContentNode::Strikethrough(text));
            }
        }

        // Definition list.
        "dl" => {
            let defs = parse_definition_list(el);
            if !defs.is_empty() {
                nodes.push(ContentNode::DefinitionList(defs));
            }
        }

        // Figure — try to find an image and caption.
        "figure" => {
            let img_sel = Selector::parse("img").expect("valid selector");
            let caption_sel = Selector::parse("figcaption").expect("valid selector");
            let src = el
                .select(&img_sel)
                .next()
                .and_then(|i| i.value().attr("src"))
                .unwrap_or("")
                .to_string();
            let alt = el
                .select(&img_sel)
                .next()
                .and_then(|i| i.value().attr("alt"))
                .unwrap_or("")
                .to_string();
            let caption = el
                .select(&caption_sel)
                .next()
                .map(|c| collect_text(&c))
                .unwrap_or_default();
            let effective_alt = if !caption.is_empty() {
                caption
            } else {
                alt
            };
            if !src.is_empty() {
                nodes.push(ContentNode::Image {
                    alt: effective_alt,
                    src,
                });
            }
        }

        // Video.
        "video" => {
            let src = el
                .value()
                .attr("src")
                .or_else(|| {
                    let source_sel = Selector::parse("source").expect("valid selector");
                    el.select(&source_sel)
                        .next()
                        .and_then(|s| s.value().attr("src"))
                })
                .unwrap_or("")
                .to_string();
            let title = el.value().attr("title").unwrap_or("Video").to_string();
            if !src.is_empty() {
                nodes.push(ContentNode::Media {
                    media_type: "video".to_string(),
                    title,
                    src,
                });
            }
        }

        // Audio.
        "audio" => {
            let src = el
                .value()
                .attr("src")
                .or_else(|| {
                    let source_sel = Selector::parse("source").expect("valid selector");
                    el.select(&source_sel)
                        .next()
                        .and_then(|s| s.value().attr("src"))
                })
                .unwrap_or("")
                .to_string();
            let title = el.value().attr("title").unwrap_or("Audio").to_string();
            if !src.is_empty() {
                nodes.push(ContentNode::Media {
                    media_type: "audio".to_string(),
                    title,
                    src,
                });
            }
        }

        // Iframe.
        "iframe" => {
            let src = el.value().attr("src").unwrap_or("").to_string();
            let title = el.value().attr("title").unwrap_or("Embedded").to_string();
            if !src.is_empty() {
                nodes.push(ContentNode::Media {
                    media_type: "iframe".to_string(),
                    title,
                    src,
                });
            }
        }

        // Container elements — recurse into children.
        "div" | "section" | "article" | "main" | "aside" | "span" | "details" | "summary"
        | "form" | "fieldset" | "legend" => {
            for child in el.children().filter_map(ElementRef::wrap) {
                walk_element(&child, config, nodes);
            }
        }

        // List items are handled by ul/ol parents; skip standalone processing.
        "li" => {}

        // Unknown / other elements — recurse into children so we don't lose content.
        _ => {
            for child in el.children().filter_map(ElementRef::wrap) {
                walk_element(&child, config, nodes);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helper: text collection
// ---------------------------------------------------------------------------

/// Collect all descendant text content of an element, joining with spaces and
/// collapsing whitespace.
fn collect_text(el: &ElementRef) -> String {
    let raw: String = el.text().collect::<Vec<_>>().join(" ");
    collapse_whitespace(&raw)
}

/// Collapse runs of whitespace into single spaces and trim.
fn collapse_whitespace(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut prev_ws = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_ws && !result.is_empty() {
                result.push(' ');
            }
            prev_ws = true;
        } else {
            prev_ws = false;
            result.push(ch);
        }
    }
    // Trim trailing space.
    if result.ends_with(' ') {
        result.pop();
    }
    result
}

// ---------------------------------------------------------------------------
// Helper: list items
// ---------------------------------------------------------------------------

/// Collect `<li>` text from a list element.
fn collect_list_items(list_el: &ElementRef) -> Vec<String> {
    let li_sel = Selector::parse("li").expect("valid selector");
    list_el
        .select(&li_sel)
        .map(|li| collect_text(&li))
        .filter(|t| !t.is_empty())
        .collect()
}

// ---------------------------------------------------------------------------
// Helper: table parsing
// ---------------------------------------------------------------------------

/// Parse a `<table>` element into header cells and body rows.
fn parse_table(table_el: &ElementRef) -> (Vec<String>, Vec<Vec<String>>) {
    let th_sel = Selector::parse("th").expect("valid selector");
    let tr_sel = Selector::parse("tr").expect("valid selector");
    let td_sel = Selector::parse("td").expect("valid selector");

    let mut headers: Vec<String> = Vec::new();
    let mut rows: Vec<Vec<String>> = Vec::new();

    for tr in table_el.select(&tr_sel) {
        let ths: Vec<String> = tr
            .select(&th_sel)
            .map(|th| collect_text(&th))
            .collect();
        if !ths.is_empty() && headers.is_empty() {
            headers = ths;
            continue;
        }

        let tds: Vec<String> = tr
            .select(&td_sel)
            .map(|td| collect_text(&td))
            .collect();
        if !tds.is_empty() {
            rows.push(tds);
        }
    }

    (headers, rows)
}

// ---------------------------------------------------------------------------
// Helper: definition list
// ---------------------------------------------------------------------------

/// Parse a `<dl>` element into (term, definition) pairs.
fn parse_definition_list(dl_el: &ElementRef) -> Vec<(String, String)> {
    let dt_sel = Selector::parse("dt").expect("valid selector");
    let dd_sel = Selector::parse("dd").expect("valid selector");

    let terms: Vec<String> = dl_el.select(&dt_sel).map(|e| collect_text(&e)).collect();
    let defs: Vec<String> = dl_el.select(&dd_sel).map(|e| collect_text(&e)).collect();

    terms.into_iter().zip(defs).collect()
}

// ---------------------------------------------------------------------------
// Helper: language extraction from code class
// ---------------------------------------------------------------------------

/// Extract a programming language hint from a `class` attribute.
///
/// Common conventions: `language-python`, `lang-rust`, `highlight-js`, etc.
fn extract_language_from_class(class: &str) -> Option<String> {
    for part in class.split_whitespace() {
        if let Some(lang) = part.strip_prefix("language-") {
            return Some(lang.to_string());
        }
        if let Some(lang) = part.strip_prefix("lang-") {
            return Some(lang.to_string());
        }
        // Highlight.js convention.
        if let Some(lang) = part.strip_prefix("highlight-") {
            return Some(lang.to_string());
        }
    }
    // Some libraries set the bare language name as a class, e.g. "python".
    // Only accept if the class has a single token that looks plausible.
    let trimmed = class.trim();
    if !trimmed.is_empty()
        && !trimmed.contains(' ')
        && trimmed.len() <= 20
        && trimmed
            .chars()
            .all(|c| c.is_alphanumeric() || c == '+' || c == '#')
    {
        return Some(trimmed.to_string());
    }
    None
}

// ---------------------------------------------------------------------------
// XML parser (generic)
// ---------------------------------------------------------------------------

/// Parse generic XML into a [`ParsedDocument`].
///
/// Walks the XML tree and converts elements into [`ContentNode`]s. Element
/// names become headings and text content becomes paragraphs.
pub fn parse_xml(xml: &str) -> Result<ParsedDocument> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    let mut nodes = Vec::new();
    let mut current_tag = String::new();
    let mut buf = Vec::new();
    let mut depth: u32 = 0;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                current_tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                depth += 1;
            }
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().unwrap_or_default().trim().to_string();
                if !text.is_empty() {
                    if !current_tag.is_empty() {
                        let level = std::cmp::min(depth, 6) as u8;
                        nodes.push(ContentNode::Heading {
                            level,
                            text: current_tag.clone(),
                        });
                        nodes.push(ContentNode::Paragraph(text));
                    } else {
                        nodes.push(ContentNode::RawText(text));
                    }
                }
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
                current_tag.clear();
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(RehykeError::ParseError {
                    url: String::new(),
                    message: format!("XML parse error: {e}"),
                });
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(ParsedDocument {
        metadata: PageMetadata::default(),
        content_nodes: nodes,
        content_type: ContentType::Xml,
    })
}

// ---------------------------------------------------------------------------
// RSS parser
// ---------------------------------------------------------------------------

/// Parse an RSS 2.0 feed into a [`ParsedDocument`].
///
/// Extracts channel-level metadata and each `<item>` as a content section.
pub fn parse_rss(xml: &str) -> Result<ParsedDocument> {
    let feed = parse_rss_feed(xml)?;
    let mut nodes = Vec::new();

    // Feed title as top-level heading.
    if let Some(ref title) = feed.title {
        nodes.push(ContentNode::Heading {
            level: 1,
            text: title.clone(),
        });
    }
    if let Some(ref desc) = feed.description {
        nodes.push(ContentNode::Paragraph(desc.clone()));
    }

    nodes.push(ContentNode::HorizontalRule);

    // Each item becomes a section.
    for item in &feed.items {
        if let Some(ref title) = item.title {
            nodes.push(ContentNode::Heading {
                level: 2,
                text: title.clone(),
            });
        }
        if let Some(ref link) = item.link {
            nodes.push(ContentNode::Link {
                text: item.title.clone().unwrap_or_else(|| link.clone()),
                href: link.clone(),
            });
        }
        if let Some(ref desc) = item.description {
            nodes.push(ContentNode::Paragraph(desc.clone()));
        }
        if let Some(ref date) = item.pub_date {
            nodes.push(ContentNode::Paragraph(format!("Published: {date}")));
        }
        if let Some(ref author) = item.author {
            nodes.push(ContentNode::Paragraph(format!("Author: {author}")));
        }
        nodes.push(ContentNode::HorizontalRule);
    }

    let metadata = PageMetadata {
        title: feed.title.clone(),
        description: feed.description.clone(),
        ..Default::default()
    };

    Ok(ParsedDocument {
        metadata,
        content_nodes: nodes,
        content_type: ContentType::Rss,
    })
}

/// Low-level RSS 2.0 parsing with quick-xml.
fn parse_rss_feed(xml: &str) -> Result<ParsedFeed> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();

    let mut feed = ParsedFeed {
        title: None,
        link: None,
        description: None,
        items: Vec::new(),
    };

    let mut in_channel = false;
    let mut in_item = false;
    let mut current_tag = String::new();
    let mut current_item = FeedItem {
        title: None,
        link: None,
        description: None,
        pub_date: None,
        author: None,
    };

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match name.as_str() {
                    "channel" => in_channel = true,
                    "item" => {
                        in_item = true;
                        current_item = FeedItem {
                            title: None,
                            link: None,
                            description: None,
                            pub_date: None,
                            author: None,
                        };
                    }
                    _ => current_tag = name,
                }
            }
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().unwrap_or_default().trim().to_string();
                if text.is_empty() {
                    buf.clear();
                    continue;
                }
                if in_item {
                    match current_tag.as_str() {
                        "title" => current_item.title = Some(text),
                        "link" => current_item.link = Some(text),
                        "description" => current_item.description = Some(text),
                        "pubDate" => current_item.pub_date = Some(text),
                        "author" | "dc:creator" => current_item.author = Some(text),
                        _ => {}
                    }
                } else if in_channel {
                    match current_tag.as_str() {
                        "title" => feed.title = Some(text),
                        "link" => feed.link = Some(text),
                        "description" => feed.description = Some(text),
                        _ => {}
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match name.as_str() {
                    "item" => {
                        feed.items.push(current_item.clone());
                        in_item = false;
                    }
                    "channel" => in_channel = false,
                    _ => {}
                }
                current_tag.clear();
            }
            Ok(Event::CData(ref e)) => {
                let text = String::from_utf8_lossy(e.as_ref()).trim().to_string();
                if text.is_empty() {
                    buf.clear();
                    continue;
                }
                if in_item {
                    match current_tag.as_str() {
                        "title" => current_item.title = Some(text),
                        "description" => current_item.description = Some(text),
                        _ => {}
                    }
                } else if in_channel {
                    match current_tag.as_str() {
                        "title" => feed.title = Some(text),
                        "description" => feed.description = Some(text),
                        _ => {}
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(RehykeError::ParseError {
                    url: String::new(),
                    message: format!("RSS parse error: {e}"),
                });
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(feed)
}

// ---------------------------------------------------------------------------
// Atom parser
// ---------------------------------------------------------------------------

/// Parse an Atom feed into a [`ParsedDocument`].
///
/// Extracts feed-level info and each `<entry>` as a content section.
pub fn parse_atom(xml: &str) -> Result<ParsedDocument> {
    let feed = parse_atom_feed(xml)?;
    let mut nodes = Vec::new();

    if let Some(ref title) = feed.title {
        nodes.push(ContentNode::Heading {
            level: 1,
            text: title.clone(),
        });
    }
    if let Some(ref desc) = feed.description {
        nodes.push(ContentNode::Paragraph(desc.clone()));
    }

    nodes.push(ContentNode::HorizontalRule);

    for item in &feed.items {
        if let Some(ref title) = item.title {
            nodes.push(ContentNode::Heading {
                level: 2,
                text: title.clone(),
            });
        }
        if let Some(ref link) = item.link {
            nodes.push(ContentNode::Link {
                text: item.title.clone().unwrap_or_else(|| link.clone()),
                href: link.clone(),
            });
        }
        if let Some(ref desc) = item.description {
            nodes.push(ContentNode::Paragraph(desc.clone()));
        }
        if let Some(ref date) = item.pub_date {
            nodes.push(ContentNode::Paragraph(format!("Published: {date}")));
        }
        if let Some(ref author) = item.author {
            nodes.push(ContentNode::Paragraph(format!("Author: {author}")));
        }
        nodes.push(ContentNode::HorizontalRule);
    }

    let metadata = PageMetadata {
        title: feed.title.clone(),
        description: feed.description.clone(),
        ..Default::default()
    };

    Ok(ParsedDocument {
        metadata,
        content_nodes: nodes,
        content_type: ContentType::Atom,
    })
}

/// Low-level Atom feed parsing with quick-xml.
fn parse_atom_feed(xml: &str) -> Result<ParsedFeed> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();

    let mut feed = ParsedFeed {
        title: None,
        link: None,
        description: None,
        items: Vec::new(),
    };

    let mut in_entry = false;
    let mut current_tag = String::new();
    let mut current_item = FeedItem {
        title: None,
        link: None,
        description: None,
        pub_date: None,
        author: None,
    };
    // Track if we are inside <author><name> (Atom nests author name).
    let mut in_author = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match name.as_str() {
                    "entry" => {
                        in_entry = true;
                        current_item = FeedItem {
                            title: None,
                            link: None,
                            description: None,
                            pub_date: None,
                            author: None,
                        };
                    }
                    "author" => in_author = true,
                    "link" => {
                        // Atom <link href="..." rel="alternate"> can appear as
                        // a start tag with attributes (not self-closing).
                        let mut href = None;
                        let mut rel = None;
                        for attr in e.attributes().flatten() {
                            let key =
                                String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val =
                                String::from_utf8_lossy(&attr.value).to_string();
                            match key.as_str() {
                                "href" => href = Some(val),
                                "rel" => rel = Some(val),
                                _ => {}
                            }
                        }
                        let is_alternate =
                            rel.as_deref() == Some("alternate") || rel.is_none();
                        if is_alternate {
                            if let Some(h) = href {
                                if in_entry {
                                    current_item.link = Some(h);
                                } else {
                                    feed.link = Some(h);
                                }
                            }
                        }
                        current_tag = name;
                    }
                    _ => current_tag = name,
                }
            }
            Ok(Event::Empty(ref e)) => {
                // Atom <link href="..." /> is self-closing.
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "link" {
                    let mut href = None;
                    let mut rel = None;
                    for attr in e.attributes().flatten() {
                        let key =
                            String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        let val = String::from_utf8_lossy(&attr.value).to_string();
                        match key.as_str() {
                            "href" => href = Some(val),
                            "rel" => rel = Some(val),
                            _ => {}
                        }
                    }
                    let is_alternate =
                        rel.as_deref() == Some("alternate") || rel.is_none();
                    if is_alternate {
                        if let Some(h) = href {
                            if in_entry {
                                current_item.link = Some(h);
                            } else {
                                feed.link = Some(h);
                            }
                        }
                    }
                }
            }
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().unwrap_or_default().trim().to_string();
                if text.is_empty() {
                    buf.clear();
                    continue;
                }

                if in_author && current_tag == "name" {
                    if in_entry {
                        current_item.author = Some(text);
                    }
                    buf.clear();
                    continue;
                }

                if in_entry {
                    match current_tag.as_str() {
                        "title" => current_item.title = Some(text),
                        "summary" | "content" => {
                            if current_item.description.is_none() {
                                current_item.description = Some(text);
                            }
                        }
                        "updated" | "published" => {
                            if current_item.pub_date.is_none() {
                                current_item.pub_date = Some(text);
                            }
                        }
                        _ => {}
                    }
                } else {
                    match current_tag.as_str() {
                        "title" => feed.title = Some(text),
                        "subtitle" => feed.description = Some(text),
                        _ => {}
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match name.as_str() {
                    "entry" => {
                        feed.items.push(current_item.clone());
                        in_entry = false;
                    }
                    "author" => in_author = false,
                    _ => {}
                }
                current_tag.clear();
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(RehykeError::ParseError {
                    url: String::new(),
                    message: format!("Atom parse error: {e}"),
                });
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(feed)
}

// ---------------------------------------------------------------------------
// JSON parser
// ---------------------------------------------------------------------------

/// Parse JSON content into a [`ParsedDocument`].
///
/// Handles JSON-LD (`@context`, `@type`), plain JSON objects, and JSON arrays.
/// Objects are turned into key-value paragraphs; arrays of objects become
/// sections or tables when their structure is uniform.
pub fn parse_json(json: &str) -> Result<ParsedDocument> {
    let value: serde_json::Value = serde_json::from_str(json).map_err(|e| {
        RehykeError::ParseError {
            url: String::new(),
            message: format!("JSON parse error: {e}"),
        }
    })?;

    let mut nodes = Vec::new();
    let mut metadata = PageMetadata::default();

    // Detect JSON-LD.
    if let Some(obj) = value.as_object() {
        if obj.contains_key("@context") || obj.contains_key("@type") {
            extract_json_ld(&value, &mut metadata, &mut nodes);
        } else {
            json_value_to_nodes(&value, 1, &mut nodes);
        }
    } else {
        json_value_to_nodes(&value, 1, &mut nodes);
    }

    Ok(ParsedDocument {
        metadata,
        content_nodes: nodes,
        content_type: ContentType::Json,
    })
}

/// Extract JSON-LD metadata and content.
fn extract_json_ld(
    value: &serde_json::Value,
    metadata: &mut PageMetadata,
    nodes: &mut Vec<ContentNode>,
) {
    if let Some(obj) = value.as_object() {
        // Extract common JSON-LD fields.
        if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
            metadata.title = Some(name.to_string());
            nodes.push(ContentNode::Heading {
                level: 1,
                text: name.to_string(),
            });
        } else if let Some(headline) = obj.get("headline").and_then(|v| v.as_str()) {
            metadata.title = Some(headline.to_string());
            nodes.push(ContentNode::Heading {
                level: 1,
                text: headline.to_string(),
            });
        }

        if let Some(desc) = obj.get("description").and_then(|v| v.as_str()) {
            metadata.description = Some(desc.to_string());
            nodes.push(ContentNode::Paragraph(desc.to_string()));
        }

        if let Some(author) = obj.get("author") {
            if let Some(name) = author.get("name").and_then(|v| v.as_str()) {
                metadata.author = Some(name.to_string());
            } else if let Some(name) = author.as_str() {
                metadata.author = Some(name.to_string());
            }
        }

        if let Some(date) = obj.get("datePublished").and_then(|v| v.as_str()) {
            metadata.published_date = Some(date.to_string());
        }

        if let Some(image) = obj.get("image") {
            if let Some(url) = image.as_str() {
                metadata.og_image = Some(url.to_string());
                nodes.push(ContentNode::Image {
                    alt: String::new(),
                    src: url.to_string(),
                });
            } else if let Some(url) = image.get("url").and_then(|v| v.as_str()) {
                metadata.og_image = Some(url.to_string());
                nodes.push(ContentNode::Image {
                    alt: String::new(),
                    src: url.to_string(),
                });
            }
        }

        if let Some(url_val) = obj.get("url").and_then(|v| v.as_str()) {
            metadata.canonical_url = Some(url_val.to_string());
        }

        // Remaining fields as paragraphs.
        let skip_keys: &[&str] = &[
            "@context",
            "@type",
            "name",
            "headline",
            "description",
            "author",
            "datePublished",
            "image",
            "url",
        ];
        for (key, val) in obj {
            if skip_keys.contains(&key.as_str()) {
                continue;
            }
            match val {
                serde_json::Value::String(s) => {
                    nodes.push(ContentNode::Paragraph(format!("{key}: {s}")));
                }
                serde_json::Value::Number(n) => {
                    nodes.push(ContentNode::Paragraph(format!("{key}: {n}")));
                }
                serde_json::Value::Bool(b) => {
                    nodes.push(ContentNode::Paragraph(format!("{key}: {b}")));
                }
                serde_json::Value::Array(arr) => {
                    let items: Vec<String> =
                        arr.iter().map(json_value_to_string).collect();
                    nodes.push(ContentNode::Paragraph(format!("{key}:")));
                    nodes.push(ContentNode::UnorderedList(items));
                }
                serde_json::Value::Object(_) => {
                    nodes.push(ContentNode::Heading {
                        level: 2,
                        text: key.clone(),
                    });
                    json_value_to_nodes(val, 3, nodes);
                }
                serde_json::Value::Null => {}
            }
        }
    }
}

/// Recursively convert a JSON value into content nodes.
fn json_value_to_nodes(
    value: &serde_json::Value,
    heading_level: u8,
    nodes: &mut Vec<ContentNode>,
) {
    match value {
        serde_json::Value::Object(obj) => {
            for (key, val) in obj {
                let level = std::cmp::min(heading_level, 6);
                match val {
                    serde_json::Value::String(s) => {
                        nodes.push(ContentNode::Paragraph(format!("{key}: {s}")));
                    }
                    serde_json::Value::Number(n) => {
                        nodes.push(ContentNode::Paragraph(format!("{key}: {n}")));
                    }
                    serde_json::Value::Bool(b) => {
                        nodes.push(ContentNode::Paragraph(format!("{key}: {b}")));
                    }
                    serde_json::Value::Null => {
                        nodes.push(ContentNode::Paragraph(format!("{key}: null")));
                    }
                    serde_json::Value::Array(arr) => {
                        nodes.push(ContentNode::Heading {
                            level,
                            text: key.clone(),
                        });
                        // If the array contains objects with uniform keys, render
                        // as a table; otherwise render as a list.
                        if let Some(table) = try_array_as_table(arr) {
                            nodes.push(table);
                        } else {
                            let items: Vec<String> =
                                arr.iter().map(json_value_to_string).collect();
                            nodes.push(ContentNode::UnorderedList(items));
                        }
                    }
                    serde_json::Value::Object(_) => {
                        nodes.push(ContentNode::Heading {
                            level,
                            text: key.clone(),
                        });
                        json_value_to_nodes(val, level + 1, nodes);
                    }
                }
            }
        }
        serde_json::Value::Array(arr) => {
            if let Some(table) = try_array_as_table(arr) {
                nodes.push(table);
            } else {
                let items: Vec<String> = arr.iter().map(json_value_to_string).collect();
                nodes.push(ContentNode::UnorderedList(items));
            }
        }
        _ => {
            nodes.push(ContentNode::RawText(json_value_to_string(value)));
        }
    }
}

/// Try to render a JSON array of objects as a table.
///
/// Returns `Some(ContentNode::Table { .. })` if every element is an object
/// with the same set of keys and all values are scalar.
fn try_array_as_table(arr: &[serde_json::Value]) -> Option<ContentNode> {
    if arr.is_empty() {
        return None;
    }
    // All items must be objects.
    let objects: Vec<&serde_json::Map<String, serde_json::Value>> =
        arr.iter().filter_map(|v| v.as_object()).collect();
    if objects.len() != arr.len() {
        return None;
    }
    // Collect headers from the first object.
    let headers: Vec<String> = objects[0].keys().cloned().collect();
    if headers.is_empty() {
        return None;
    }
    // All objects must have the same keys with scalar values.
    for obj in &objects {
        if obj.len() != headers.len() {
            return None;
        }
        for key in &headers {
            match obj.get(key) {
                Some(serde_json::Value::Object(_))
                | Some(serde_json::Value::Array(_))
                | None => {
                    return None;
                }
                _ => {}
            }
        }
    }

    let rows: Vec<Vec<String>> = objects
        .iter()
        .map(|obj| {
            headers
                .iter()
                .map(|k| json_value_to_string(obj.get(k).unwrap()))
                .collect()
        })
        .collect();

    Some(ContentNode::Table { headers, rows })
}

/// Convert a JSON value to a simple string representation.
fn json_value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "null".to_string(),
        other => other.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Metadata extraction ------------------------------------------------

    #[test]
    fn test_extract_title() {
        let html = r#"<html><head><title>My Page Title</title></head><body></body></html>"#;
        let doc = parse_html(html, &ParseConfig::default()).unwrap();
        assert_eq!(doc.metadata.title.as_deref(), Some("My Page Title"));
    }

    #[test]
    fn test_extract_meta_description() {
        let html = r#"
        <html>
        <head>
            <meta name="description" content="A page about testing">
        </head>
        <body></body>
        </html>"#;
        let doc = parse_html(html, &ParseConfig::default()).unwrap();
        assert_eq!(
            doc.metadata.description.as_deref(),
            Some("A page about testing")
        );
    }

    #[test]
    fn test_extract_og_metadata() {
        let html = r#"
        <html>
        <head>
            <meta property="og:title" content="OG Title">
            <meta property="og:description" content="OG Description">
            <meta property="og:image" content="https://example.com/image.jpg">
        </head>
        <body></body>
        </html>"#;
        let doc = parse_html(html, &ParseConfig::default()).unwrap();
        // og:title fills in when <title> is absent.
        assert_eq!(doc.metadata.title.as_deref(), Some("OG Title"));
        assert_eq!(
            doc.metadata.description.as_deref(),
            Some("OG Description")
        );
        assert_eq!(
            doc.metadata.og_image.as_deref(),
            Some("https://example.com/image.jpg")
        );
    }

    #[test]
    fn test_extract_language() {
        let html = r#"<html lang="en-US"><head></head><body></body></html>"#;
        let doc = parse_html(html, &ParseConfig::default()).unwrap();
        assert_eq!(doc.metadata.language.as_deref(), Some("en-US"));
    }

    #[test]
    fn test_extract_canonical_url() {
        let html = r#"
        <html>
        <head><link rel="canonical" href="https://example.com/page"></head>
        <body></body>
        </html>"#;
        let doc = parse_html(html, &ParseConfig::default()).unwrap();
        assert_eq!(
            doc.metadata.canonical_url.as_deref(),
            Some("https://example.com/page")
        );
    }

    #[test]
    fn test_extract_keywords() {
        let html = r#"
        <html>
        <head><meta name="keywords" content="rust, web, crawler, parser"></head>
        <body></body>
        </html>"#;
        let doc = parse_html(html, &ParseConfig::default()).unwrap();
        assert_eq!(
            doc.metadata.keywords,
            vec!["rust", "web", "crawler", "parser"]
        );
    }

    #[test]
    fn test_extract_author() {
        let html = r#"
        <html>
        <head><meta name="author" content="Jane Doe"></head>
        <body></body>
        </html>"#;
        let doc = parse_html(html, &ParseConfig::default()).unwrap();
        assert_eq!(doc.metadata.author.as_deref(), Some("Jane Doe"));
    }

    #[test]
    fn test_extract_published_date() {
        let html = r#"
        <html>
        <head><meta property="article:published_time" content="2024-01-15"></head>
        <body></body>
        </html>"#;
        let doc = parse_html(html, &ParseConfig::default()).unwrap();
        assert_eq!(
            doc.metadata.published_date.as_deref(),
            Some("2024-01-15")
        );
    }

    #[test]
    fn test_metadata_extraction_disabled() {
        let html = r#"
        <html>
        <head>
            <title>Should Not Extract</title>
            <meta name="description" content="Ignored">
        </head>
        <body></body>
        </html>"#;
        let config = ParseConfig {
            extract_metadata: false,
            ..Default::default()
        };
        let doc = parse_html(html, &config).unwrap();
        assert!(doc.metadata.title.is_none());
        assert!(doc.metadata.description.is_none());
    }

    // -- Content node generation --------------------------------------------

    #[test]
    fn test_heading_nodes() {
        let html = r#"
        <html><body>
            <h1>Title</h1>
            <h2>Subtitle</h2>
            <h3>Section</h3>
        </body></html>"#;
        let doc = parse_html(html, &ParseConfig::default()).unwrap();
        let headings: Vec<_> = doc
            .content_nodes
            .iter()
            .filter_map(|n| match n {
                ContentNode::Heading { level, text } => Some((*level, text.as_str())),
                _ => None,
            })
            .collect();
        assert_eq!(
            headings,
            vec![(1, "Title"), (2, "Subtitle"), (3, "Section")]
        );
    }

    #[test]
    fn test_paragraph_nodes() {
        let html =
            r#"<html><body><p>Hello world</p><p>Second paragraph</p></body></html>"#;
        let doc = parse_html(html, &ParseConfig::default()).unwrap();
        let paragraphs: Vec<_> = doc
            .content_nodes
            .iter()
            .filter_map(|n| match n {
                ContentNode::Paragraph(text) => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(paragraphs, vec!["Hello world", "Second paragraph"]);
    }

    #[test]
    fn test_link_node() {
        let html = r#"<html><body><a href="https://example.com">Click me</a></body></html>"#;
        let doc = parse_html(html, &ParseConfig::default()).unwrap();
        let links: Vec<_> = doc
            .content_nodes
            .iter()
            .filter_map(|n| match n {
                ContentNode::Link { text, href } => {
                    Some((text.as_str(), href.as_str()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(links, vec![("Click me", "https://example.com")]);
    }

    #[test]
    fn test_image_node() {
        let html =
            r#"<html><body><img src="pic.jpg" alt="A picture"></body></html>"#;
        let doc = parse_html(html, &ParseConfig::default()).unwrap();
        let images: Vec<_> = doc
            .content_nodes
            .iter()
            .filter_map(|n| match n {
                ContentNode::Image { alt, src } => {
                    Some((alt.as_str(), src.as_str()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(images, vec![("A picture", "pic.jpg")]);
    }

    #[test]
    fn test_bold_and_italic_nodes() {
        let html =
            r#"<html><body><strong>Bold</strong><em>Italic</em></body></html>"#;
        let doc = parse_html(html, &ParseConfig::default()).unwrap();
        assert!(doc
            .content_nodes
            .iter()
            .any(|n| matches!(n, ContentNode::Bold(t) if t == "Bold")));
        assert!(doc
            .content_nodes
            .iter()
            .any(|n| matches!(n, ContentNode::Italic(t) if t == "Italic")));
    }

    #[test]
    fn test_code_and_codeblock_nodes() {
        let html = r#"
        <html><body>
            <code>inline_code()</code>
            <pre><code class="language-python">print("hello")</code></pre>
        </body></html>"#;
        let doc = parse_html(html, &ParseConfig::default()).unwrap();

        assert!(doc
            .content_nodes
            .iter()
            .any(|n| matches!(n, ContentNode::Code(t) if t == "inline_code()")));

        let code_blocks: Vec<_> = doc
            .content_nodes
            .iter()
            .filter_map(|n| match n {
                ContentNode::CodeBlock { language, code } => {
                    Some((language.as_deref(), code.as_str()))
                }
                _ => None,
            })
            .collect();
        assert!(code_blocks
            .iter()
            .any(|(lang, code)| *lang == Some("python")
                && *code == r#"print("hello")"#));
    }

    #[test]
    fn test_unordered_list() {
        let html = r#"
        <html><body>
            <ul>
                <li>Item A</li>
                <li>Item B</li>
                <li>Item C</li>
            </ul>
        </body></html>"#;
        let doc = parse_html(html, &ParseConfig::default()).unwrap();
        let lists: Vec<_> = doc
            .content_nodes
            .iter()
            .filter_map(|n| match n {
                ContentNode::UnorderedList(items) => Some(items.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(lists.len(), 1);
        assert_eq!(lists[0], vec!["Item A", "Item B", "Item C"]);
    }

    #[test]
    fn test_ordered_list() {
        let html = r#"
        <html><body>
            <ol>
                <li>First</li>
                <li>Second</li>
            </ol>
        </body></html>"#;
        let doc = parse_html(html, &ParseConfig::default()).unwrap();
        let lists: Vec<_> = doc
            .content_nodes
            .iter()
            .filter_map(|n| match n {
                ContentNode::OrderedList(items) => Some(items.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(lists.len(), 1);
        assert_eq!(lists[0], vec!["First", "Second"]);
    }

    #[test]
    fn test_blockquote() {
        let html = r#"<html><body><blockquote>Wise words</blockquote></body></html>"#;
        let doc = parse_html(html, &ParseConfig::default()).unwrap();
        assert!(doc
            .content_nodes
            .iter()
            .any(|n| matches!(n, ContentNode::Blockquote(t) if t == "Wise words")));
    }

    #[test]
    fn test_table_node() {
        let html = r#"
        <html><body>
            <table>
                <tr><th>Name</th><th>Age</th></tr>
                <tr><td>Alice</td><td>30</td></tr>
                <tr><td>Bob</td><td>25</td></tr>
            </table>
        </body></html>"#;
        let doc = parse_html(html, &ParseConfig::default()).unwrap();
        let tables: Vec<_> = doc
            .content_nodes
            .iter()
            .filter_map(|n| match n {
                ContentNode::Table { headers, rows } => {
                    Some((headers.clone(), rows.clone()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].0, vec!["Name", "Age"]);
        assert_eq!(tables[0].1.len(), 2);
        assert_eq!(tables[0].1[0], vec!["Alice", "30"]);
        assert_eq!(tables[0].1[1], vec!["Bob", "25"]);
    }

    #[test]
    fn test_horizontal_rule() {
        let html =
            r#"<html><body><p>Before</p><hr><p>After</p></body></html>"#;
        let doc = parse_html(html, &ParseConfig::default()).unwrap();
        assert!(doc
            .content_nodes
            .iter()
            .any(|n| matches!(n, ContentNode::HorizontalRule)));
    }

    #[test]
    fn test_strikethrough() {
        let html = r#"<html><body><del>removed</del><s>also removed</s></body></html>"#;
        let doc = parse_html(html, &ParseConfig::default()).unwrap();
        let strikes: Vec<_> = doc
            .content_nodes
            .iter()
            .filter_map(|n| match n {
                ContentNode::Strikethrough(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(strikes, vec!["removed", "also removed"]);
    }

    #[test]
    fn test_definition_list() {
        let html = r#"
        <html><body>
            <dl>
                <dt>Term</dt><dd>Definition</dd>
                <dt>Another</dt><dd>Another def</dd>
            </dl>
        </body></html>"#;
        let doc = parse_html(html, &ParseConfig::default()).unwrap();
        let dls: Vec<_> = doc
            .content_nodes
            .iter()
            .filter_map(|n| match n {
                ContentNode::DefinitionList(pairs) => Some(pairs.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(dls.len(), 1);
        assert_eq!(
            dls[0],
            vec![
                ("Term".to_string(), "Definition".to_string()),
                ("Another".to_string(), "Another def".to_string()),
            ]
        );
    }

    #[test]
    fn test_figure_with_caption() {
        let html = r#"
        <html><body>
            <figure>
                <img src="photo.jpg" alt="Photo">
                <figcaption>My great photo</figcaption>
            </figure>
        </body></html>"#;
        let doc = parse_html(html, &ParseConfig::default()).unwrap();
        let images: Vec<_> = doc
            .content_nodes
            .iter()
            .filter_map(|n| match n {
                ContentNode::Image { alt, src } => {
                    Some((alt.as_str(), src.as_str()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0], ("My great photo", "photo.jpg"));
    }

    #[test]
    fn test_video_node() {
        let html = r#"<html><body><video src="video.mp4" title="Demo"></video></body></html>"#;
        let doc = parse_html(html, &ParseConfig::default()).unwrap();
        assert!(doc.content_nodes.iter().any(|n| matches!(
            n,
            ContentNode::Media {
                media_type,
                title,
                src
            } if media_type == "video" && title == "Demo" && src == "video.mp4"
        )));
    }

    #[test]
    fn test_iframe_node() {
        let html = r#"<html><body><iframe src="https://embed.com/v" title="Embed"></iframe></body></html>"#;
        let doc = parse_html(html, &ParseConfig::default()).unwrap();
        assert!(doc.content_nodes.iter().any(|n| matches!(
            n,
            ContentNode::Media {
                media_type,
                title,
                src
            } if media_type == "iframe" && title == "Embed" && src == "https://embed.com/v"
        )));
    }

    // -- Content cleaning ---------------------------------------------------

    #[test]
    fn test_script_removal() {
        let html = r#"
        <html><body>
            <p>Visible</p>
            <script>alert("evil")</script>
            <p>Also visible</p>
        </body></html>"#;
        let doc = parse_html(html, &ParseConfig::default()).unwrap();
        let texts: Vec<_> = doc
            .content_nodes
            .iter()
            .filter_map(|n| match n {
                ContentNode::Paragraph(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, vec!["Visible", "Also visible"]);
        // Ensure no node contains the script content.
        for node in &doc.content_nodes {
            match node {
                ContentNode::RawText(t) | ContentNode::Paragraph(t) => {
                    assert!(!t.contains("alert"));
                }
                _ => {}
            }
        }
    }

    #[test]
    fn test_style_removal() {
        let html = r#"
        <html><body>
            <style>body { color: red; }</style>
            <p>Content</p>
        </body></html>"#;
        let doc = parse_html(html, &ParseConfig::default()).unwrap();
        let texts: Vec<_> = doc
            .content_nodes
            .iter()
            .filter_map(|n| match n {
                ContentNode::Paragraph(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, vec!["Content"]);
        for node in &doc.content_nodes {
            match node {
                ContentNode::RawText(t) | ContentNode::Paragraph(t) => {
                    assert!(!t.contains("color"));
                }
                _ => {}
            }
        }
    }

    #[test]
    fn test_noscript_removal() {
        let html = r#"
        <html><body>
            <noscript>Enable JS</noscript>
            <p>Real content</p>
        </body></html>"#;
        let doc = parse_html(html, &ParseConfig::default()).unwrap();
        for node in &doc.content_nodes {
            match node {
                ContentNode::RawText(t) | ContentNode::Paragraph(t) => {
                    assert!(!t.contains("Enable JS"));
                }
                _ => {}
            }
        }
    }

    // -- Parse config options -----------------------------------------------

    #[test]
    fn test_nav_removal_enabled() {
        let html = r#"
        <html><body>
            <nav><a href="/">Home</a><a href="/about">About</a></nav>
            <p>Main content</p>
        </body></html>"#;
        let config = ParseConfig {
            clean_navigation: true,
            ..Default::default()
        };
        let doc = parse_html(html, &config).unwrap();
        // Nav links should not appear.
        assert!(!doc.content_nodes.iter().any(
            |n| matches!(n, ContentNode::Link { text, .. } if text == "Home")
        ));
        // Main content should be present.
        assert!(doc.content_nodes.iter().any(
            |n| matches!(n, ContentNode::Paragraph(t) if t == "Main content")
        ));
    }

    #[test]
    fn test_nav_removal_disabled() {
        let html = r#"
        <html><body>
            <nav><a href="/">Home</a></nav>
            <p>Main content</p>
        </body></html>"#;
        let config = ParseConfig {
            clean_navigation: false,
            clean_footers: false,
            clean_ads: false,
            clean_comments: false,
            extract_metadata: true,
        };
        let doc = parse_html(html, &config).unwrap();
        // Nav link should now appear.
        assert!(doc.content_nodes.iter().any(
            |n| matches!(n, ContentNode::Link { text, .. } if text == "Home")
        ));
    }

    #[test]
    fn test_footer_removal_enabled() {
        let html = r#"
        <html><body>
            <p>Main content</p>
            <footer><p>Copyright 2024</p></footer>
        </body></html>"#;
        let config = ParseConfig {
            clean_footers: true,
            ..Default::default()
        };
        let doc = parse_html(html, &config).unwrap();
        assert!(!doc.content_nodes.iter().any(
            |n| matches!(n, ContentNode::Paragraph(t) if t.contains("Copyright"))
        ));
    }

    #[test]
    fn test_footer_removal_disabled() {
        let html = r#"
        <html><body>
            <p>Main content</p>
            <footer><p>Copyright 2024</p></footer>
        </body></html>"#;
        let config = ParseConfig {
            clean_navigation: false,
            clean_footers: false,
            clean_ads: false,
            clean_comments: false,
            extract_metadata: true,
        };
        let doc = parse_html(html, &config).unwrap();
        assert!(doc.content_nodes.iter().any(
            |n| matches!(n, ContentNode::Paragraph(t) if t.contains("Copyright"))
        ));
    }

    #[test]
    fn test_ad_removal() {
        let html = r#"
        <html><body>
            <p>Real content</p>
            <div class="ad-banner"><p>Buy stuff!</p></div>
            <div id="google_ads_123"><p>Sponsored</p></div>
            <p>More real content</p>
        </body></html>"#;
        let config = ParseConfig {
            clean_ads: true,
            ..Default::default()
        };
        let doc = parse_html(html, &config).unwrap();
        let texts: Vec<_> = doc
            .content_nodes
            .iter()
            .filter_map(|n| match n {
                ContentNode::Paragraph(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert!(texts.contains(&"Real content"));
        assert!(texts.contains(&"More real content"));
        assert!(!texts.contains(&"Buy stuff!"));
        assert!(!texts.contains(&"Sponsored"));
    }

    #[test]
    fn test_comment_section_removal() {
        let html = r#"
        <html><body>
            <p>Article text</p>
            <div id="comments"><p>User comment</p></div>
            <div class="disqus_thread"><p>Disqus comment</p></div>
        </body></html>"#;
        let config = ParseConfig {
            clean_comments: true,
            ..Default::default()
        };
        let doc = parse_html(html, &config).unwrap();
        let texts: Vec<_> = doc
            .content_nodes
            .iter()
            .filter_map(|n| match n {
                ContentNode::Paragraph(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert!(texts.contains(&"Article text"));
        assert!(!texts.contains(&"User comment"));
        assert!(!texts.contains(&"Disqus comment"));
    }

    // -- RSS parsing --------------------------------------------------------

    #[test]
    fn test_parse_rss() {
        let rss = r#"<?xml version="1.0" encoding="UTF-8"?>
        <rss version="2.0">
            <channel>
                <title>My Blog</title>
                <link>https://example.com</link>
                <description>A test blog</description>
                <item>
                    <title>First Post</title>
                    <link>https://example.com/first</link>
                    <description>Hello world</description>
                    <pubDate>Mon, 01 Jan 2024 00:00:00 GMT</pubDate>
                    <author>alice@example.com</author>
                </item>
                <item>
                    <title>Second Post</title>
                    <link>https://example.com/second</link>
                    <description>Another post</description>
                </item>
            </channel>
        </rss>"#;

        let doc = parse_rss(rss).unwrap();
        assert_eq!(doc.metadata.title.as_deref(), Some("My Blog"));
        assert_eq!(doc.metadata.description.as_deref(), Some("A test blog"));
        assert_eq!(doc.content_type, ContentType::Rss);

        // Should have heading for feed, paragraphs for items, etc.
        assert!(doc.content_nodes.iter().any(
            |n| matches!(n, ContentNode::Heading { level: 1, text } if text == "My Blog")
        ));
        assert!(doc.content_nodes.iter().any(
            |n| matches!(n, ContentNode::Heading { level: 2, text } if text == "First Post")
        ));
        assert!(doc.content_nodes.iter().any(
            |n| matches!(n, ContentNode::Heading { level: 2, text } if text == "Second Post")
        ));
    }

    #[test]
    fn test_parse_rss_items() {
        let rss = r#"<?xml version="1.0"?>
        <rss version="2.0">
            <channel>
                <title>Feed</title>
                <item>
                    <title>Post</title>
                    <link>https://example.com/post</link>
                    <description>Content here</description>
                </item>
            </channel>
        </rss>"#;

        let feed = parse_rss_feed(rss).unwrap();
        assert_eq!(feed.items.len(), 1);
        assert_eq!(feed.items[0].title.as_deref(), Some("Post"));
        assert_eq!(
            feed.items[0].link.as_deref(),
            Some("https://example.com/post")
        );
        assert_eq!(
            feed.items[0].description.as_deref(),
            Some("Content here")
        );
    }

    // -- Atom parsing -------------------------------------------------------

    #[test]
    fn test_parse_atom() {
        let atom = r#"<?xml version="1.0" encoding="UTF-8"?>
        <feed xmlns="http://www.w3.org/2005/Atom">
            <title>Atom Feed</title>
            <subtitle>A test feed</subtitle>
            <link href="https://example.com" />
            <entry>
                <title>Entry One</title>
                <link href="https://example.com/one" />
                <summary>First entry summary</summary>
                <updated>2024-01-01T00:00:00Z</updated>
                <author><name>Alice</name></author>
            </entry>
        </feed>"#;

        let doc = parse_atom(atom).unwrap();
        assert_eq!(doc.metadata.title.as_deref(), Some("Atom Feed"));
        assert_eq!(doc.metadata.description.as_deref(), Some("A test feed"));
        assert_eq!(doc.content_type, ContentType::Atom);

        assert!(doc.content_nodes.iter().any(
            |n| matches!(n, ContentNode::Heading { level: 1, text } if text == "Atom Feed")
        ));
        assert!(doc.content_nodes.iter().any(
            |n| matches!(n, ContentNode::Heading { level: 2, text } if text == "Entry One")
        ));
    }

    // -- JSON parsing -------------------------------------------------------

    #[test]
    fn test_parse_json_object() {
        let json = r#"{"name": "Test", "value": 42}"#;
        let doc = parse_json(json).unwrap();
        assert_eq!(doc.content_type, ContentType::Json);
        assert!(!doc.content_nodes.is_empty());
    }

    #[test]
    fn test_parse_json_ld() {
        let json = r#"{
            "@context": "https://schema.org",
            "@type": "Article",
            "headline": "Test Article",
            "description": "A test",
            "author": {"name": "Bob"},
            "datePublished": "2024-06-01"
        }"#;
        let doc = parse_json(json).unwrap();
        assert_eq!(doc.metadata.title.as_deref(), Some("Test Article"));
        assert_eq!(doc.metadata.description.as_deref(), Some("A test"));
        assert_eq!(doc.metadata.author.as_deref(), Some("Bob"));
        assert_eq!(
            doc.metadata.published_date.as_deref(),
            Some("2024-06-01")
        );
    }

    #[test]
    fn test_parse_json_array_as_table() {
        let json = r#"[
            {"name": "Alice", "age": 30},
            {"name": "Bob", "age": 25}
        ]"#;
        let doc = parse_json(json).unwrap();
        assert!(doc
            .content_nodes
            .iter()
            .any(|n| matches!(n, ContentNode::Table { .. })));
    }

    // -- Dispatcher ---------------------------------------------------------

    #[test]
    fn test_parse_dispatch_html() {
        let html = "<html><body><p>Hello</p></body></html>";
        let doc =
            parse(html, &ContentType::Html, &ParseConfig::default()).unwrap();
        assert_eq!(doc.content_type, ContentType::Html);
    }

    #[test]
    fn test_parse_dispatch_plain_text() {
        let text = "Just some plain text.";
        let doc = parse(text, &ContentType::PlainText, &ParseConfig::default())
            .unwrap();
        assert_eq!(doc.content_type, ContentType::PlainText);
        assert!(doc.content_nodes.iter().any(
            |n| matches!(n, ContentNode::RawText(t) if t == "Just some plain text.")
        ));
    }

    #[test]
    fn test_parse_dispatch_unknown() {
        let data = "some binary gibberish";
        let ct = ContentType::Other("application/octet-stream".to_string());
        let doc = parse(data, &ct, &ParseConfig::default()).unwrap();
        assert!(matches!(doc.content_type, ContentType::Other(_)));
    }

    // -- Utility helpers ----------------------------------------------------

    #[test]
    fn test_collapse_whitespace() {
        assert_eq!(collapse_whitespace("  hello   world  "), "hello world");
        assert_eq!(collapse_whitespace("\n\t foo \n bar \t"), "foo bar");
        assert_eq!(collapse_whitespace(""), "");
        assert_eq!(collapse_whitespace("   "), "");
    }

    #[test]
    fn test_extract_language_from_class() {
        assert_eq!(
            extract_language_from_class("language-python"),
            Some("python".to_string())
        );
        assert_eq!(
            extract_language_from_class("lang-rust"),
            Some("rust".to_string())
        );
        assert_eq!(
            extract_language_from_class("highlight-js"),
            Some("js".to_string())
        );
        assert_eq!(
            extract_language_from_class("python"),
            Some("python".to_string())
        );
        // Multiple classes: first matching prefix wins.
        assert_eq!(
            extract_language_from_class("hljs language-go"),
            Some("go".to_string())
        );
    }

    // -- Complex HTML document ----------------------------------------------

    #[test]
    fn test_complex_html_document() {
        let html = r#"
        <html lang="en">
        <head>
            <title>Complex Page</title>
            <meta name="description" content="A complex test page">
            <meta name="author" content="Test Author">
            <link rel="canonical" href="https://example.com/complex">
        </head>
        <body>
            <nav><a href="/">Home</a></nav>
            <main>
                <h1>Main Heading</h1>
                <p>Intro paragraph with <strong>bold</strong> and <em>italic</em>.</p>
                <h2>Code Examples</h2>
                <pre><code class="language-rust">fn main() {}</code></pre>
                <h2>Data</h2>
                <table>
                    <tr><th>Col A</th><th>Col B</th></tr>
                    <tr><td>1</td><td>2</td></tr>
                </table>
                <ul>
                    <li>Bullet one</li>
                    <li>Bullet two</li>
                </ul>
                <blockquote>A wise quote</blockquote>
                <hr>
                <img src="photo.png" alt="Photo">
            </main>
            <footer><p>Footer text</p></footer>
            <script>console.log("removed")</script>
        </body>
        </html>"#;

        let doc = parse_html(html, &ParseConfig::default()).unwrap();

        // Metadata.
        assert_eq!(doc.metadata.title.as_deref(), Some("Complex Page"));
        assert_eq!(
            doc.metadata.description.as_deref(),
            Some("A complex test page")
        );
        assert_eq!(doc.metadata.author.as_deref(), Some("Test Author"));
        assert_eq!(doc.metadata.language.as_deref(), Some("en"));
        assert_eq!(
            doc.metadata.canonical_url.as_deref(),
            Some("https://example.com/complex")
        );

        // Content nodes — spot check.
        assert!(doc.content_nodes.iter().any(
            |n| matches!(n, ContentNode::Heading { level: 1, text } if text == "Main Heading")
        ));
        assert!(doc.content_nodes.iter().any(
            |n| matches!(n, ContentNode::CodeBlock { language: Some(l), .. } if l == "rust")
        ));
        assert!(doc.content_nodes.iter().any(
            |n| matches!(n, ContentNode::Table { headers, .. } if headers == &["Col A", "Col B"])
        ));
        assert!(doc.content_nodes.iter().any(
            |n| matches!(n, ContentNode::UnorderedList(items) if items.len() == 2)
        ));
        assert!(doc.content_nodes.iter().any(
            |n| matches!(n, ContentNode::Blockquote(t) if t == "A wise quote")
        ));
        assert!(doc
            .content_nodes
            .iter()
            .any(|n| matches!(n, ContentNode::HorizontalRule)));
        assert!(doc.content_nodes.iter().any(
            |n| matches!(n, ContentNode::Image { src, .. } if src == "photo.png")
        ));

        // Cleaned elements should not appear.
        for node in &doc.content_nodes {
            match node {
                ContentNode::RawText(t) | ContentNode::Paragraph(t) => {
                    assert!(!t.contains("console.log"));
                    assert!(!t.contains("Footer text"));
                }
                _ => {}
            }
        }
    }
}
