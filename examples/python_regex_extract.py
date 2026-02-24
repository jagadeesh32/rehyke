#!/usr/bin/env python3
"""Regex-Powered Data Extraction with Rehyke

Crawl pages with Rehyke, then mine the resulting Markdown for structured
data using Python's `re` module.  Extracts emails, URLs, phone numbers,
prices, dates, and keywords -- with CSV export.

Prerequisites:
    pip install rehyke
"""

import csv
import io
import re
from collections import Counter
from dataclasses import dataclass, field

import rehyke

# ── Compiled regex patterns (compiled once for speed) ───────────────────────

EMAIL_RE = re.compile(r"[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}")
URL_RE = re.compile(r"https?://[^\s\)\]\"'>]+")
PHONE_RE = re.compile(r"(?:\+?1[-.\s]?)?\(?\d{3}\)?[-.\s]?\d{3}[-.\s]?\d{4}")
PRICE_RE = re.compile(r"\$\d{1,3}(?:,\d{3})*(?:\.\d{2})?")
DATE_RE = re.compile(
    r"\b\d{4}[-/]\d{2}[-/]\d{2}\b"                        # 2025-01-15
    r"|\b\d{1,2}[-/]\d{1,2}[-/]\d{2,4}\b"                 # 01/15/2025
    r"|\b(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)"
    r"[a-z]*\s+\d{1,2},?\s+\d{4}\b",                      # January 15, 2025
    re.IGNORECASE,
)
HEADING_RE = re.compile(r"^#{1,6}\s+(.+)$", re.MULTILINE)

# ── Data container ──────────────────────────────────────────────────────────

@dataclass
class PageExtract:
    """All data extracted from a single crawled page."""
    url: str
    title: str
    emails: list[str] = field(default_factory=list)
    links: list[str] = field(default_factory=list)
    phones: list[str] = field(default_factory=list)
    prices: list[str] = field(default_factory=list)
    dates: list[str] = field(default_factory=list)
    headings: list[str] = field(default_factory=list)
    word_count: int = 0
    keyword_density: dict[str, float] = field(default_factory=dict)

# ── Extraction helpers ──────────────────────────────────────────────────────

def extract_all(page: rehyke.CrawlResult, keywords: list[str]) -> PageExtract:
    """Run every regex extractor on a CrawlResult."""
    text = page.markdown
    total = len(text.split()) or 1
    lower = text.lower()
    return PageExtract(
        url=page.url, title=page.title,
        emails=EMAIL_RE.findall(text), links=URL_RE.findall(text),
        phones=PHONE_RE.findall(text), prices=PRICE_RE.findall(text),
        dates=DATE_RE.findall(text),   headings=HEADING_RE.findall(text),
        word_count=total,
        keyword_density={kw: lower.count(kw.lower()) / total for kw in keywords},
    )

def score_page(ex: PageExtract) -> float:
    """Score a page by the richness of extractable data."""
    return (len(ex.emails) * 3 + len(ex.phones) * 2 + len(ex.prices) * 2
            + len(ex.dates) + len(ex.links) * 0.5
            + sum(ex.keyword_density.values()) * 100)

def extracts_to_csv(extracts: list[PageExtract]) -> str:
    """Serialize PageExtract objects to a CSV string."""
    buf = io.StringIO()
    w = csv.writer(buf)
    w.writerow(["url", "title", "words", "emails", "phones",
                "prices", "dates", "links", "score"])
    for ex in extracts:
        w.writerow([ex.url, ex.title, ex.word_count,
                    "; ".join(ex.emails), "; ".join(ex.phones),
                    "; ".join(ex.prices), "; ".join(ex.dates),
                    len(ex.links), f"{score_page(ex):.1f}"])
    return buf.getvalue()

# ── Main ────────────────────────────────────────────────────────────────────

def main() -> None:
    target_url = "https://example.com"
    keywords = ["example", "domain", "information"]

    # -- Crawl --
    print("=" * 60)
    print("Crawling with Rehyke")
    print("=" * 60)

    crawler = rehyke.Rehyke(config=rehyke.CrawlConfig(
        mode=rehyke.ScanMode.FULL, max_pages=20,
        concurrency=4, clean_navigation=True, clean_ads=True,
    ))
    try:
        results = crawler.crawl(target_url)
    except Exception as exc:
        print(f"   Crawl failed: {exc}")
        return
    print(f"   Crawled {len(results)} page(s)\n")

    # -- Extract structured data from each page --
    print("=" * 60)
    print("Extracting data with regex")
    print("=" * 60)

    extracts: list[PageExtract] = []
    for page in results:
        ex = extract_all(page, keywords)
        extracts.append(ex)
        print(f"\n   Page : {ex.url}")
        print(f"   Words: {ex.word_count}  Emails: {ex.emails or '-'}  "
              f"Phones: {ex.phones or '-'}")
        print(f"   Prices: {ex.prices or '-'}  Dates: {ex.dates or '-'}  "
              f"Links: {len(ex.links)}  Score: {score_page(ex):.1f}")
        if ex.headings:
            print(f"   Headings: {ex.headings[:5]}")

    # -- Keyword density report --
    print("\n" + "=" * 60)
    print("Keyword density analysis")
    print("=" * 60)
    for kw in keywords:
        densities = [ex.keyword_density.get(kw, 0) for ex in extracts]
        avg = sum(densities) / len(densities) if densities else 0
        print(f"   '{kw}' avg={avg:.4f}  max={max(densities, default=0):.4f}")

    # -- Aggregate statistics --
    print("\n" + "=" * 60)
    print("Aggregate statistics")
    print("=" * 60)
    all_emails = [e for ex in extracts for e in ex.emails]
    all_links = [lnk for ex in extracts for lnk in ex.links]
    print(f"   Unique emails: {len(set(all_emails))}")
    print(f"   Unique links : {len(set(all_links))}")
    top_links = Counter(all_links).most_common(5)
    if top_links:
        print(f"   Top links    : {[u for u, _ in top_links]}")

    # -- CSV export --
    csv_path = "extracted_data.csv"
    with open(csv_path, "w", newline="") as f:
        f.write(extracts_to_csv(extracts))
    print(f"\n   Exported {len(extracts)} row(s) to {csv_path}")
    print("\nDone!  Regex extraction complete.")

if __name__ == "__main__":
    main()
