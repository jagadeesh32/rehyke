"""Rehyke -- Crawl Everything. Miss Nothing.

Ultra-high-performance web crawler that converts web pages to clean Markdown.

Usage:
    import rehyke

    # Simple one-liner
    results = rehyke.crawl("https://example.com")

    # With mode
    results = rehyke.crawl("https://example.com", mode="full")

    # Access results
    for page in results:
        print(page.url)
        print(page.title)
        print(page.markdown)
"""

from .rehyke import CrawlConfig, CrawlResult, Rehyke, ScanMode, crawl

__all__ = [
    "crawl",
    "CrawlConfig",
    "CrawlResult",
    "Rehyke",
    "ScanMode",
]

__version__ = "0.1.0"
