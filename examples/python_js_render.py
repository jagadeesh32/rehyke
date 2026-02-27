#!/usr/bin/env python3
"""
Python JS Rendering Example (v0.2.0)
=====================================

Demonstrates Rehyke's headless-browser pipeline from Python:

  1. NetworkIdle wait strategy — best for React / Vue / Angular SPAs
  2. Selector-based wait — wait for a specific DOM element to appear
  3. Duration-based wait — fixed settle time after page load
  4. Mobile viewport + fingerprint randomisation
  5. Static vs JS render comparison
  6. SPA detection and popup dismissal
  7. Screenshot capture (PNG and JPEG)
  8. Multi-page crawl with JS rendering

Requirements
------------
Install the Rehyke Python package::

    pip install rehyke            # from PyPI
    # or build locally:
    cd crates/rehyke-python && maturin develop --features js

Chrome or Chromium must be installed and discoverable on PATH.

Usage
-----
::

    python examples/python_js_render.py

Set ``RUST_LOG=debug`` for verbose output.
"""

import os
import sys

try:
    import rehyke
except ImportError:
    print("ERROR: rehyke is not installed.")
    print("  Build it with: cd crates/rehyke-python && maturin develop --features js")
    sys.exit(1)


def section(title: str) -> None:
    print(f"\n--- {title} ---\n")


def main() -> None:
    print("=== Rehyke Python JS Rendering Example (v0.2.0) ===\n")

    target = "https://httpbin.org/html"

    # =========================================================================
    # 1. NetworkIdle wait — best for most SPAs
    # =========================================================================
    section("1. NetworkIdle wait (best for React / Vue / Angular SPAs)")

    config_idle = rehyke.CrawlConfig(
        enable_js=True,
        js_wait_strategy="network_idle",
        js_wait_timeout=10.0,
        viewport="desktop",
        detect_spa=True,
        dismiss_popups=True,
    )

    print(f"Config: {config_idle!r}")
    print(f"Target: {target}\n")

    crawler = rehyke.Rehyke(config_idle)
    try:
        results = crawler.crawl(target)
        for r in results:
            print(f"  URL:     {r.url}")
            print(f"  Title:   {r.title}")
            print(f"  Status:  {r.status_code}")
            print(f"  Render:  {r.render_method}")
            print(f"  Words:   {len(r.markdown.split())}")
            print()
    except Exception as exc:
        print(f"  [WARN] Crawl failed (Chrome may not be installed): {exc}\n")

    # =========================================================================
    # 2. Selector-based wait
    # =========================================================================
    section("2. Selector-based wait (wait for 'body' to appear)")

    config_selector = rehyke.CrawlConfig(
        enable_js=True,
        # "selector:<CSS>" tells Rehyke to poll until the element appears.
        js_wait_strategy="selector:body",
        js_wait_timeout=5.0,
        viewport="desktop",
    )

    print(f"Config: {config_selector!r}")

    crawler2 = rehyke.Rehyke(config_selector)
    try:
        results2 = crawler2.crawl(target)
        if results2:
            r = results2[0]
            print(f"  Render: {r.render_method}  Words: {len(r.markdown.split())}")
    except Exception as exc:
        print(f"  [WARN] {exc}")
    print()

    # =========================================================================
    # 3. Duration-based wait
    # =========================================================================
    section("3. Duration-based wait (fixed 2-second settle period)")

    config_duration = rehyke.CrawlConfig(
        enable_js=True,
        # A numeric string (or float) is interpreted as seconds of fixed wait.
        js_wait_strategy="2.0",
        js_wait_timeout=15.0,
        viewport="tablet",
        detect_spa=True,
    )

    print(f"Config: {config_duration!r}")

    crawler3 = rehyke.Rehyke(config_duration)
    try:
        results3 = crawler3.crawl(target)
        if results3:
            r = results3[0]
            print(f"  Render: {r.render_method}")
    except Exception as exc:
        print(f"  [WARN] {exc}")
    print()

    # =========================================================================
    # 4. Mobile viewport + fingerprint randomisation
    # =========================================================================
    section("4. Mobile viewport + fingerprint randomisation")

    config_mobile = rehyke.CrawlConfig(
        enable_js=True,
        js_wait_strategy="auto",
        js_wait_timeout=8.0,
        viewport="mobile",               # 390×844, 3× DPR, touch enabled
        randomize_fingerprint=True,      # randomise UA, languages, WebGL, timezone
        dismiss_popups=True,
        detect_spa=True,
    )

    print(f"Config: {config_mobile!r}")

    crawler4 = rehyke.Rehyke(config_mobile)
    try:
        results4 = crawler4.crawl(target)
        if results4:
            r = results4[0]
            print(f"  Render: {r.render_method}  Words: {len(r.markdown.split())}")
    except Exception as exc:
        print(f"  [WARN] {exc}")
    print()

    # =========================================================================
    # 5. Static vs JS render comparison
    # =========================================================================
    section("5. Static vs JS render comparison")

    print("  Static fetch:")
    try:
        static_results = rehyke.crawl(target, mode="lite")
        if static_results:
            r = static_results[0]
            print(f"    Method: {r.render_method}")
            print(f"    Words:  {len(r.markdown.split())}")
            print(f"    Status: {r.status_code}")
    except Exception as exc:
        print(f"    [WARN] {exc}")

    print("\n  JS render:")
    config_js = rehyke.CrawlConfig(
        enable_js=True,
        js_wait_strategy="auto",
    )
    try:
        js_results = rehyke.Rehyke(config_js).crawl(target)
        if js_results:
            r = js_results[0]
            print(f"    Method: {r.render_method}")
            print(f"    Words:  {len(r.markdown.split())}")
            print(f"    Status: {r.status_code}")
    except Exception as exc:
        print(f"    [WARN] JS unavailable: {exc}")
        print("    (Chrome not installed or `js` feature not enabled)")
    print()

    # =========================================================================
    # 6. SPA detection + popup dismissal
    # =========================================================================
    section("6. SPA detection + popup dismissal")

    config_spa = rehyke.CrawlConfig(
        enable_js=True,
        js_wait_strategy="network_idle",
        js_wait_timeout=12.0,
        scroll_count=5,        # scroll 5 viewports (triggers infinite-scroll loaders)
        dismiss_popups=True,   # dismiss cookie banners, GDPR modals, newsletter overlays
        viewport="desktop",
        detect_spa=True,       # identify React / Vue / Angular / Svelte / Next.js / …
        max_pages=3,
    )

    print(f"Config: {config_spa!r}")
    print(f"  scroll_count = {config_spa._CrawlConfig__inner_scroll_count() if hasattr(config_spa, '_CrawlConfig__inner_scroll_count') else 5}")

    crawler5 = rehyke.Rehyke(config_spa)
    try:
        results5 = crawler5.crawl(target)
        print(f"  Crawled {len(results5)} page(s)")
        for r in results5:
            print(f"    [{r.status_code}] {r.url}  ({len(r.markdown.split())} words, {r.render_method})")
    except Exception as exc:
        print(f"  [WARN] {exc}")
    print()

    # =========================================================================
    # 7. Screenshot capture
    # =========================================================================
    section("7. Screenshot capture (PNG — Desktop)")

    screenshot_dir = "/tmp/rehyke_py_screenshots"
    os.makedirs(screenshot_dir, exist_ok=True)
    print(f"Screenshots will be saved to: {screenshot_dir}\n")

    config_shot = rehyke.CrawlConfig(
        enable_js=True,
        js_wait_strategy="auto",
        js_wait_timeout=10.0,
        viewport="desktop",
        screenshot=True,
        screenshot_format="png",
        screenshot_dir=screenshot_dir,
    )

    print(f"Config: {config_shot!r}")

    crawler6 = rehyke.Rehyke(config_shot)
    try:
        results6 = crawler6.crawl(target)
        print(f"  Crawled {len(results6)} page(s)")
        # List saved files
        saved = sorted(
            os.path.join(dp, f)
            for dp, _, files in os.walk(screenshot_dir)
            for f in files
        )
        if saved:
            print(f"  Saved {len(saved)} screenshot(s):")
            for path in saved:
                size = os.path.getsize(path)
                print(f"    {path}  ({size:,} bytes)")
        else:
            print("  (no screenshots saved — Chrome was not available)")
    except Exception as exc:
        print(f"  [WARN] {exc}")
    print()

    # =========================================================================
    # 8. Configuration reference
    # =========================================================================
    section("Configuration Reference")

    print("CrawlConfig parameters (v0.2.0):")
    print("""
    rehyke.CrawlConfig(
        # Core crawl settings (v0.1.0)
        mode              = rehyke.ScanMode.FULL,   # LITE | FULL | DEEP
        max_depth         = 3,
        max_pages         = 100,
        concurrency       = 8,
        user_agent        = None,                    # custom UA string
        timeout_secs      = 30,
        respect_robots_txt= True,

        # JavaScript rendering (v0.2.0)
        enable_js         = True,
        js_wait_strategy  = "network_idle",   # "auto" | "network_idle" |
                                              # "selector:<CSS>" | float (secs)
        js_wait_timeout   = 10.0,             # max seconds to wait
        scroll_count      = 5,                # viewport scrolls for infinite scroll
        dismiss_popups    = True,             # dismiss cookie/GDPR banners
        viewport          = "desktop",        # "desktop" | "tablet" | "mobile"
        detect_spa        = True,             # identify React/Vue/Angular/…
        randomize_fingerprint = True,         # randomise UA, WebGL, languages

        # Screenshot capture (v0.2.0)
        screenshot        = True,
        screenshot_format = "png",            # "png" | "jpeg"
        screenshot_dir    = "/tmp/shots",
    )
""")

    print("One-shot convenience function:")
    print("    results = rehyke.crawl('https://example.com', mode='full')")
    print()

    print("CrawlResult fields:")
    print("    r.url           # str  — the crawled URL")
    print("    r.title         # str  — page <title>")
    print("    r.markdown      # str  — page content as Markdown")
    print("    r.status_code   # int  — HTTP status code")
    print("    r.content_type  # str  — Content-Type header")
    print("    r.render_method # str  — 'static' or 'javascript'")
    print("    r.depth         # int  — crawl depth")
    print()

    print("=== Done ===")


if __name__ == "__main__":
    main()
