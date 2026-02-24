# Contributing to Rehyke

## Welcome

Rehyke is a high-performance async web crawling engine built in Rust with Python
bindings. Its goal is to provide a fast, reliable, and configurable foundation
for web scraping, content extraction, and site analysis.

We welcome contributions of all kinds: bug reports, feature requests, documentation
improvements, code patches, and test cases. Whether you are fixing a typo or adding
a major feature, your help is valued.

## Development Setup

### Prerequisites

| Tool       | Minimum Version | Purpose                        |
|------------|-----------------|--------------------------------|
| Rust       | 1.75+           | Core engine and CLI            |
| Cargo      | (bundled)       | Build system and package mgr   |
| Python     | 3.8+            | Python bindings (optional)     |
| maturin    | 1.0+            | Build Python wheel (optional)  |
| Chrome     | 120+            | Headless rendering (optional)  |

### Clone and Build

```bash
git clone https://github.com/vrinda/rehyke.git
cd rehyke

# Build the entire workspace (core library + CLI)
cargo build --workspace

# Build in release mode
cargo build --workspace --release
```

### Running Tests

```bash
# Run all tests across the workspace
cargo test --workspace

# Run tests for a specific crate
cargo test -p rehyke-core

# Run a single test by name
cargo test -p rehyke-core -- test_normalize_url

# Run tests with output visible
cargo test --workspace -- --nocapture
```

### Building the Python Wheel

```bash
# Install maturin if you haven't already
pip install maturin

# Build and install the Python package in development mode
cd crates/rehyke-python
maturin develop --release

# Build a distributable wheel
maturin build --release
```

## Project Structure

The workspace is organized into three crates:

```
rehyke/
  Cargo.toml              # Workspace root
  crates/
    rehyke-core/          # Core crawl engine (library)
      src/
        lib.rs            # Public API and re-exports
        config.rs         # CrawlConfig builder
        crawler.rs        # Main crawl loop orchestration
        fetcher.rs        # HTTP client (reqwest)
        parser.rs         # Content type detection and routing
        html.rs           # HTML parsing and cleaning
        markdown.rs       # HTML-to-Markdown conversion
        links.rs          # Link extraction and resolution
        robots.rs         # robots.txt parsing
        sitemap.rs        # Sitemap XML parsing
        scheduler.rs      # Priority URL queue
        normalize.rs      # URL normalization
        rate_limiter.rs   # Per-domain throttling
        user_agent.rs     # User agent rotation
        delay.rs          # Delay strategies
        proxy.rs          # Proxy pool management
        renderer.rs       # Headless browser (stub)
        error.rs          # Error types (thiserror)
    rehyke-cli/           # CLI binary
      src/
        main.rs           # Argument parsing and entry point
    rehyke-python/        # Python bindings (PyO3)
      src/
        lib.rs            # PyO3 module definition
```

## Code Style

### General Rules

- Rust edition 2021 with a minimum supported Rust version (MSRV) of 1.75.
- Format all code with `rustfmt` using the default configuration.
- Run `clippy` with `-D warnings` -- all warnings are treated as errors.
- Write doc comments (`///`) for every public item: functions, structs, enums,
  traits, and their fields or variants.

### Naming Conventions

- Functions and methods: `snake_case`
- Types (structs, enums, traits): `PascalCase`
- Constants and statics: `SCREAMING_SNAKE_CASE`
- Module files: `snake_case.rs`

### Error Handling

- Use `thiserror` for error types in library crates.
- Use `anyhow` only in the CLI binary and test code.
- Propagate errors with `?` -- never call `.unwrap()` or `.expect()` in library
  code unless the invariant is provably guaranteed and documented.

### Logging

- Use `tracing` macros throughout: `info!`, `debug!`, `warn!`, `error!`.
- Log at `debug!` level for routine operations (fetching, parsing).
- Log at `info!` level for significant milestones (crawl start/end, page count).
- Log at `warn!` level for recoverable problems (retry, rate limit hit).
- Log at `error!` level only for unrecoverable failures.

### Safety

- No `unsafe` code anywhere except the PyO3 bindings crate, where it is required
  by the FFI boundary.
- Every module must contain a `#[cfg(test)] mod tests` block.

## How to Contribute

### Reporting Bugs

Open an issue on the GitHub tracker. A good bug report includes:

- **Operating system** and version (e.g., Ubuntu 22.04, macOS 14.2, Windows 11)
- **Rust version** (`rustc --version`)
- **Rehyke version** or commit hash
- **Steps to reproduce** the problem, including the target URL if applicable
- **Expected behavior** versus **actual behavior**
- **Logs or error output** (run with `RUST_LOG=debug` for verbose output)

If the issue involves a specific website, please confirm that the site is publicly
accessible so maintainers can reproduce it.

### Suggesting Features

Open a feature request issue. Include:

- A clear description of the feature and the problem it solves.
- The use case: who benefits and how.
- Any consideration of backwards compatibility -- will existing users need to
  change their code or configuration?
- If possible, a rough sketch of the API or configuration surface you envision.

### Pull Requests

1. **Fork** the repository and create a branch from `main`.
2. **Branch naming**: use a prefix that describes the change type:
   - `feature/` for new functionality (e.g., `feature/headless-renderer`)
   - `fix/` for bug fixes (e.g., `fix/robots-wildcard-match`)
   - `docs/` for documentation-only changes (e.g., `docs/add-proxy-examples`)
3. **Write your code** following the style guidelines above.
4. **Add or update tests** to cover your changes.
5. **Run the full check suite** before pushing:
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace -- -D warnings
   cargo test --workspace
   ```
6. **Push** your branch and open a pull request against `main`.
7. **PR description**: explain what changed and why. Use this template:
   ```
   ## What
   Brief summary of the change.

   ## Why
   Motivation or issue number (Closes #123).

   ## How
   Key implementation details, if non-obvious.

   ## Testing
   How you verified the change works.
   ```
8. All CI checks must pass before a PR will be reviewed.

### Commit Messages

- Use the imperative mood: "Add parser for Atom feeds", not "Added parser".
- Keep the subject line under 72 characters.
- Reference issue numbers where applicable: "Fix robots.txt wildcard matching (#42)".
- Separate subject from body with a blank line if more detail is needed.

## Architecture Guidelines

### Async-First Design

All I/O operations use `tokio` async runtime. Blocking calls must be wrapped in
`tokio::task::spawn_blocking`. The crawl loop is driven by `tokio::select!` over
the URL queue and in-flight request futures.

### Concurrency

- Use `DashMap` and `DashSet` for shared concurrent state (visited URLs, rate
  limit timestamps, proxy stats). These are lock-free and avoid the overhead of
  `Mutex` or `RwLock`.
- Limit concurrency through `tokio::sync::Semaphore` based on the configured
  `max_concurrent_requests`.

### Error Handling

- Never panic in library code. Every fallible operation returns `Result`.
- The `RehykeError` enum in `error.rs` has 14 variants covering all failure
  modes. Add new variants there rather than using string errors.
- Propagate errors with `?`. Let the caller decide how to handle them.

### Memory Management

- Stream large responses to disk rather than buffering them entirely in memory.
- Drop DOM trees as soon as content extraction is complete.
- Use `String::with_capacity` when the approximate size is known.
- Prefer iterators over collecting into intermediate `Vec`s where possible.

### Regex

- Compile regular expressions once and reuse them. Use `lazy_static!` or
  `std::sync::OnceLock` for patterns that are used across multiple calls.
- Never compile a regex inside a loop.

## Testing

### Unit Tests

Every module must have a `#[cfg(test)] mod tests` block with at least basic
coverage of its public API. Tests should be deterministic -- avoid depending on
external network calls in unit tests. Use mock data or recorded responses.

### Integration Tests

Integration tests that perform real HTTP requests belong in `tests/` at the
crate root. Mark them with `#[ignore]` so they do not run in the default
`cargo test` invocation. Run them explicitly with:

```bash
cargo test --workspace -- --ignored
```

### Adding a New Test

1. Identify the module your change affects.
2. Add a test function inside the existing `mod tests` block.
3. Name the test descriptively: `test_<function>_<scenario>` (e.g.,
   `test_normalize_url_removes_fragment`).
4. Use `assert_eq!` for value comparisons, `assert!` for boolean conditions.

### Running Specific Tests

```bash
# All tests matching a pattern
cargo test -p rehyke-core -- normalize

# A single exact test
cargo test -p rehyke-core -- test_normalize_url_removes_default_port --exact
```

## Areas We Need Help

The following areas are actively looking for contributors:

- **Headless Chromium integration** -- the `renderer.rs` module is stubbed out
  and ready for implementation. We need Chrome DevTools Protocol support for
  JavaScript-rendered pages.
- **User agent strings** -- expand the current pool of 57 user agents with
  newer browser versions and mobile variants.
- **Content type parsers** -- add support for additional formats like YAML,
  TOML, CSV, and iCalendar.
- **Performance benchmarks** -- set up `criterion` benchmarks for the hot paths
  (URL normalization, HTML parsing, Markdown conversion).
- **Documentation improvements** -- more examples, tutorials, and API docs.
- **Python type stubs** -- create `.pyi` files for the PyO3 bindings so that
  Python users get autocomplete and type checking.
- **CI/CD pipeline** -- set up GitHub Actions for automated testing, linting,
  and wheel building across Linux, macOS, and Windows.
- **Ad and tracker detection** -- add more regex patterns and heuristics for
  identifying and removing advertising and tracking elements during content
  cleaning.

## License

Rehyke is dual-licensed under the MIT License and the Apache License 2.0, at
your option. Any contribution you submit for inclusion in the project will be
licensed under these same terms, without any additional conditions.

See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) for the full
license texts.
