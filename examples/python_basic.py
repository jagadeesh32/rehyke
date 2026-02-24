#!/usr/bin/env python3
"""Basic Rehyke Usage - Your First Crawl

Demonstrates the simplest ways to use Rehyke: the one-liner convenience
function, iterating results, scan modes, error handling, and saving output.

Prerequisites:
    pip install rehyke
"""

import rehyke

# ── 1. One-liner crawl ──────────────────────────────────────────────────────
# rehyke.crawl() returns a list of CrawlResult objects, one per page visited.

print("=" * 60)
print("1. Quick crawl with rehyke.crawl()")
print("=" * 60)

results = rehyke.crawl("https://example.com")
print(f"   Crawled {len(results)} page(s)\n")

# ── 2. Inspecting results ───────────────────────────────────────────────────
# CrawlResult attributes: .url, .title, .markdown, .status_code, .content_type

print("=" * 60)
print("2. Inspecting CrawlResult attributes")
print("=" * 60)

for page in results:
    print(f"   URL          : {page.url}")
    print(f"   Title        : {page.title}")
    print(f"   Status       : {page.status_code}")
    print(f"   Content-Type : {page.content_type}")
    preview = page.markdown[:200].replace("\n", " ")
    print(f"   Preview      : {preview}...\n")

# ── 3. Scan modes ───────────────────────────────────────────────────────────
# "lite" = shallow/fast, "full" = balanced default, "deep" = exhaustive

print("=" * 60)
print("3. Scan modes: lite / full / deep")
print("=" * 60)

for mode in ("lite", "full", "deep"):
    pages = rehyke.crawl("https://example.com", mode=mode)
    print(f"   mode={mode!r:6s}  =>  {len(pages)} page(s)")
print()

# ── 4. Error handling ───────────────────────────────────────────────────────
# Rehyke maps errors to ValueError, TimeoutError, OSError, or RuntimeError.

print("=" * 60)
print("4. Error handling")
print("=" * 60)

try:
    rehyke.crawl("not-a-valid-url")
except ValueError as exc:
    print(f"   Caught ValueError: {exc}")

try:
    rehyke.crawl("https://httpstat.us/404")
except Exception as exc:
    print(f"   Caught {type(exc).__name__}: {exc}")
print()

# ── 5. Saving results to a file ────────────────────────────────────────────
# Rehyke.crawl_to_file() writes combined Markdown directly to disk.

print("=" * 60)
print("5. Saving to a file with Rehyke.crawl_to_file()")
print("=" * 60)

crawler = rehyke.Rehyke()
crawler.crawl_to_file("https://example.com", "example_output.md")
print("   Saved crawl output to example_output.md\n")

print("Done!  You have completed your first Rehyke crawl.")
