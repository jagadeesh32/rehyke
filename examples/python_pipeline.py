#!/usr/bin/env python3
"""Content Processing Pipeline with Rehyke

Multi-stage pipeline: crawl several URLs, deduplicate by content hash,
extract a link graph, compute word frequencies, and write organized output
(one Markdown file per page, an index, and a site report).

Prerequisites:
    pip install rehyke
"""

import hashlib
import json
import re
import time
from collections import Counter
from pathlib import Path

import rehyke

# ── Configuration ───────────────────────────────────────────────────────────

TARGETS: list[str] = [
    "https://example.com",
    "https://httpbin.org/html",
]
OUTPUT_DIR = Path("pipeline_output")
STOPWORDS = {"the", "a", "an", "and", "or", "but", "in", "on", "at", "to",
             "for", "of", "with", "by", "is", "was", "are", "were", "be",
             "it", "its", "this", "that", "from", "as", "not", "so", "if",
             "all", "can", "has", "have", "had", "do", "did", "will"}
MD_LINK_RE = re.compile(r"\[([^\]]*)\]\(([^)]+)\)")
WORD_RE = re.compile(r"[a-zA-Z]{3,}")

# ── Stage 1: Crawl ─────────────────────────────────────────────────────────

def stage_crawl(targets: list[str]) -> list[rehyke.CrawlResult]:
    """Crawl every target URL and collect results."""
    print("=" * 60, "\nStage 1 -- Crawling targets\n" + "=" * 60)
    crawler = rehyke.Rehyke(config=rehyke.CrawlConfig(
        mode=rehyke.ScanMode.LITE, max_pages=20, concurrency=4,
        timeout_secs=15, clean_navigation=True, clean_footers=True,
        clean_ads=True, delay_min_ms=100, delay_max_ms=500,
    ))
    all_results: list[rehyke.CrawlResult] = []
    for url in targets:
        t0 = time.perf_counter()
        try:
            pages = crawler.crawl(url)
        except Exception as exc:
            print(f"   SKIP {url}: {exc}"); continue
        all_results.extend(pages)
        print(f"   {url} -> {len(pages)} page(s) in {time.perf_counter()-t0:.2f}s")
    print(f"   Total raw pages: {len(all_results)}\n")
    return all_results

# ── Stage 2: Deduplicate ───────────────────────────────────────────────────

def stage_deduplicate(pages: list[rehyke.CrawlResult]) -> list[rehyke.CrawlResult]:
    """Remove duplicate pages by SHA-256 content fingerprint."""
    print("=" * 60, "\nStage 2 -- Deduplication\n" + "=" * 60)
    seen: set[str] = set()
    unique: list[rehyke.CrawlResult] = []
    for page in pages:
        h = hashlib.sha256(page.markdown.encode()).hexdigest()
        if h not in seen:
            seen.add(h); unique.append(page)
    print(f"   {len(pages)} input -> {len(unique)} unique "
          f"({len(pages)-len(unique)} duplicates removed)\n")
    return unique

# ── Stage 3: Regex post-processing ─────────────────────────────────────────

def clean_markdown(md: str) -> str:
    """Normalize whitespace and bullet styles in Markdown."""
    md = re.sub(r"\n{3,}", "\n\n", md)                      # collapse blank lines
    md = re.sub(r"^[*+]\s", "- ", md, flags=re.MULTILINE)   # unify bullets
    md = re.sub(r"[ \t]+$", "", md, flags=re.MULTILINE)      # trim trailing ws
    return md.strip()

# ── Stage 4: Link graph ────────────────────────────────────────────────────

def stage_link_graph(pages: list[rehyke.CrawlResult]) -> dict[str, list[str]]:
    """Build adjacency list: source URL -> list of outgoing URLs."""
    print("=" * 60, "\nStage 4 -- Link graph\n" + "=" * 60)
    graph: dict[str, list[str]] = {}
    for page in pages:
        hrefs = [href for _, href in MD_LINK_RE.findall(page.markdown)]
        graph[page.url] = hrefs
        print(f"   {page.url} -> {len(hrefs)} outgoing link(s)")
    edges = sum(len(v) for v in graph.values())
    print(f"   Graph: {len(graph)} node(s), {edges} edge(s)\n")
    return graph

# ── Stage 5: Word frequency ────────────────────────────────────────────────

def stage_word_frequency(pages: list[rehyke.CrawlResult], top_n: int = 20) -> Counter:
    """Count word frequencies across all pages, minus stopwords."""
    print("=" * 60, "\nStage 5 -- Word frequency\n" + "=" * 60)
    counter: Counter = Counter()
    for page in pages:
        counter.update(w for w in WORD_RE.findall(page.markdown.lower())
                       if w not in STOPWORDS)
    print(f"   Vocabulary: {len(counter)} unique words")
    for word, n in counter.most_common(top_n):
        print(f"      {word:20s} {n:5d}  {'#' * min(n, 40)}")
    print()
    return counter

# ── Stage 6: Write output ──────────────────────────────────────────────────

def stage_write(pages: list[rehyke.CrawlResult],
                graph: dict[str, list[str]], freq: Counter) -> None:
    """Write per-page .md files, an index, and a site report."""
    print("=" * 60, "\nStage 6 -- Writing output\n" + "=" * 60)
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    manifest: list[dict[str, str]] = []
    for i, page in enumerate(pages):
        slug = re.sub(r"[^a-zA-Z0-9]+", "_", page.url)[:80].strip("_")
        fname = f"{i:03d}_{slug}.md"
        header = f"---\nurl: {page.url}\ntitle: {page.title}\n---\n\n"
        (OUTPUT_DIR / fname).write_text(header + clean_markdown(page.markdown))
        manifest.append({"url": page.url, "title": page.title, "file": fname})
        print(f"   Wrote {OUTPUT_DIR / fname}")

    # Index file
    idx = ["# Crawl Index\n"] + [
        f"- [{e['title'] or '(untitled)'}]({e['file']})  \n  {e['url']}"
        for e in manifest]
    (OUTPUT_DIR / "index.md").write_text("\n".join(idx) + "\n")

    # Site report: word frequency table + link graph
    rpt = [f"# Site Report\n\nPages crawled: {len(pages)}\n",
           "## Top Words\n", "| Word | Count |", "|------|-------|"]
    rpt += [f"| {w} | {c} |" for w, c in freq.most_common(15)]
    rpt.append("\n## Link Graph\n")
    for src, dsts in graph.items():
        rpt.append(f"**{src}**: " + ", ".join(dsts[:10]) +
                   (f" (+{len(dsts)-10} more)" if len(dsts) > 10 else ""))
    (OUTPUT_DIR / "report.md").write_text("\n".join(rpt) + "\n")

    (OUTPUT_DIR / "manifest.json").write_text(json.dumps(manifest, indent=2))
    print(f"   Wrote index.md, report.md, manifest.json\n")

# ── Main ────────────────────────────────────────────────────────────────────

def main() -> None:
    t0 = time.perf_counter()
    pages = stage_crawl(TARGETS)
    if not pages:
        print("No pages crawled. Exiting."); return
    pages = stage_deduplicate(pages)
    graph = stage_link_graph(pages)
    freq = stage_word_frequency(pages)
    stage_write(pages, graph, freq)
    print("=" * 60)
    print(f"Pipeline complete in {time.perf_counter()-t0:.2f}s")
    print(f"Output directory: {OUTPUT_DIR.resolve()}")
    print("=" * 60)

if __name__ == "__main__":
    main()
