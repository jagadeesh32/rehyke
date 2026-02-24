#!/usr/bin/env python3
"""Advanced Rehyke Configuration and Usage

Explores every CrawlConfig knob, delay strategies, URL filtering with regex
patterns, content-cleaning toggles, multi-target crawling, result analysis,
JSON export, Markdown file generation, and performance measurement.

Prerequisites:
    pip install rehyke
"""

import json
import re
import time
from pathlib import Path

import rehyke

# ── 1. Full CrawlConfig ────────────────────────────────────────────────────

print("=" * 60)
print("1. Building a fully-customized CrawlConfig")
print("=" * 60)

config = rehyke.CrawlConfig(
    mode=rehyke.ScanMode.FULL,       # LITE | FULL | DEEP
    max_depth=3,                      # follow links up to 3 levels deep
    max_pages=50,                     # stop after 50 pages
    concurrency=4,                    # at most 4 parallel requests
    enable_js=False,                  # skip JS rendering (faster)
    user_agent="RehykeBot/1.0 (advanced-example)",
    timeout_secs=15,                  # per-request timeout
    max_retries=2,                    # retry failed requests twice
    respect_robots_txt=True,          # honour robots.txt
    clean_navigation=True,            # strip nav bars
    clean_footers=True,               # strip footers
    clean_ads=True,                   # strip ad blocks
    exclude_patterns=[r"/tag/", r"\?sort=", r"\.pdf$"],
    include_patterns=[r"/blog/", r"/docs/"],
    delay_min_ms=200,                 # random delay between requests
    delay_max_ms=800,
)
print(f"   Config: {config!r}\n")

# ── 2. Fixed delay strategy ─────────────────────────────────────────────────
# Passing only one delay value creates a fixed (non-random) delay.

print("=" * 60)
print("2. Fixed delay strategy (500 ms)")
print("=" * 60)

fixed_config = rehyke.CrawlConfig(mode=rehyke.ScanMode.LITE, delay_min_ms=500)
print(f"   Config: {fixed_config!r}\n")

# ── 3. Multi-target crawling with timing ────────────────────────────────────

print("=" * 60)
print("3. Crawling multiple targets with performance timing")
print("=" * 60)

targets: list[str] = [
    "https://example.com",
    "https://httpbin.org/html",
]

crawler = rehyke.Rehyke(config=rehyke.CrawlConfig(
    mode=rehyke.ScanMode.LITE, max_pages=5, timeout_secs=10,
))

all_results: list[rehyke.CrawlResult] = []
for url in targets:
    start = time.perf_counter()
    try:
        results = crawler.crawl(url)
    except Exception as exc:
        print(f"   [{url}] ERROR: {exc}")
        continue
    elapsed = time.perf_counter() - start
    all_results.extend(results)
    print(f"   [{url}] {len(results)} page(s) in {elapsed:.2f}s")
print(f"   Total pages collected: {len(all_results)}\n")

# ── 4. Result analysis ──────────────────────────────────────────────────────

print("=" * 60)
print("4. Analyzing crawl results")
print("=" * 60)

link_re = re.compile(r"\[([^\]]*)\]\(([^)]+)\)")
content_type_counts: dict[str, int] = {}
total_words = 0

for page in all_results:
    words = len(page.markdown.split())
    total_words += words
    ct = page.content_type.split(";")[0].strip()
    content_type_counts[ct] = content_type_counts.get(ct, 0) + 1
    links = link_re.findall(page.markdown)
    print(f"   {page.url}  words={words}  links={len(links)}  status={page.status_code}")

print(f"   Total words: {total_words}")
print(f"   Content-types: {json.dumps(content_type_counts)}\n")

# ── 5. JSON export ──────────────────────────────────────────────────────────

print("=" * 60)
print("5. Exporting results to JSON")
print("=" * 60)

records = [
    {
        "url": p.url, "title": p.title, "status_code": p.status_code,
        "content_type": p.content_type, "word_count": len(p.markdown.split()),
    }
    for p in all_results
]
Path("crawl_results.json").write_text(json.dumps(records, indent=2))
print(f"   Wrote {len(records)} record(s) to crawl_results.json\n")

# ── 6. Markdown file generation ─────────────────────────────────────────────

print("=" * 60)
print("6. Generating individual Markdown files")
print("=" * 60)

output_dir = Path("crawl_output")
output_dir.mkdir(exist_ok=True)

for i, page in enumerate(all_results):
    slug = re.sub(r"[^a-zA-Z0-9]+", "_", page.url)[:80].strip("_")
    filepath = output_dir / f"{i:03d}_{slug}.md"
    header = f"# {page.title or '(untitled)'}\n\n> Source: {page.url}\n\n---\n\n"
    filepath.write_text(header + page.markdown)
    print(f"   {filepath}")

print(f"   {len(all_results)} file(s) written to {output_dir}/\n")
print("Done!  Advanced crawl complete.")
