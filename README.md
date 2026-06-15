# Crawl Boi

## Overview

Crawl Boi is a concurrent web crawler written in Rust. Given a starting URL, it visits every reachable page on the same subdomain, printing each visited URL and the links discovered on that page. It will not follow links to external domains or sibling subdomains (Though it could with minor changes).

The project is a Cargo workspace split into two crates: `crawl-boi-core` (a library containing all crawling logic, testable in isolation with no network access) and `crawl-boi-cli` (a binary handling argument parsing and output formatting). This separation means the core can be embedded in other programs or driven entirely through tests using a mock fetcher.

### Design decisions

- Concurrency is driven by `FuturesUnordered` with batch-size limiting rather than spawning unbounded tasks. The crawler drains at most `concurrency` URLs from the frontier at once, feeds them into the pool, and refills as tasks complete. This gives natural back-pressure and keeps the concurrency model simple to reason about.
- The visited set uses `Arc<Mutex<HashSet<Url>>>`. At the default concurrency of 8 (appropriate for a polite crawler), sharded maps like `DashMap` offer no measurable benefit and add a dependency.
- `CrawlScope` is a first-class type consulted by the crawler loop. Scope decisions are encapsulated there so that future extensions (multi-domain support, per-domain caps) require no changes to the orchestrator.
- The `Fetcher` trait abstracts HTTP access. `HttpFetcher` is the production implementation; tests use a `MockFetcher` backed by an in-memory `HashMap<Url, Result<String, FetchError>>`, giving full crawler integration tests with zero network access.
- `robots.txt` is fetched once before the crawl begins. If it is absent or unreachable the crawler proceeds with no restrictions and logs a warning to stderr.
- URLs are normalised by stripping fragment identifiers before they enter the frontier or visited set. `http` and `https` on the same host are treated as distinct origins.

### Trade-offs

- HTML parsing uses the `scraper` crate (a thin wrapper around `html5ever`). It keeps the parser module concise without sacrificing correctness. Using `html5ever` directly would add boilerplate for no material gain here.

## Getting started

You will need a Rust toolchain (stable channel). If you do not already have one, install it via [rustup](https://rustup.rs/):

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Follow the on-screen prompts, then restart your shell or run `source $HOME/.cargo/env` to make `cargo` available.

Build the workspace:

```sh
cargo build
```

Run the tests:

```sh
cargo test
```

## Use cases

Crawl a site with default settings (concurrency 8, no budget, plain-text output):

```sh
cargo run -p crawl-boi-cli -- --url https://books.toscrape.com crawl
```

The output prints each visited URL followed by the links found on that page, indented:

```
https://books.toscrape.com/
  https://books.toscrape.com/catalogue/page-2.html
  https://books.toscrape.com/catalogue/a-light-in-the-attic_1000/index.html
  https://books.toscrape.com/catalogue/tipping-the-velvet_999/index.html
https://books.toscrape.com/catalogue/page-2.html
  https://books.toscrape.com/catalogue/page-3.html
  https://books.toscrape.com/catalogue/its-only-the-himalayas_981/index.html
```

Limit the crawl to 50 pages:

```sh
cargo run -p crawl-boi-cli -- --url https://books.toscrape.com crawl --max-pages 50
```

## Fun, extra Use-Cases
Restrict how many pages are fetched under the catalogue path (the site has 1000 books across 50 pages of listings plus individual product pages):

```sh
cargo run -p crawl-boi-cli -- --url https://books.toscrape.com crawl --path-budget "/catalogue/,20"
```

Combine a global cap with multiple path budgets:

```sh
cargo run -p crawl-boi-cli -- --url https://books.toscrape.com crawl --max-pages 30 --path-budget "/catalogue/,4"  --path-budget "/catalogue/category,1"
```

## Stress testing

Running against the target site with concurrency cranked up:

```sh
time cargo run --release -p crawl-boi-cli -- --url YOUR_URL_THAT_HAS_LOTS_OF_BLOG_AND_PRODUCT_PAGES crawl -c 5000 2>/dev/null | wc -l
```

On a Mac Mini this crawls all 382,104 output lines (every reachable page on the subdomain) in about 64 seconds wall-clock. The process spends roughly 5.5s in user time and 3.1s in system time, with CPU sitting at 13% since the bottleneck is network I/O, not compute.

To verify correctness under concurrency, the property-based tests in `crawl-boi-core` generate random site graphs (including cycles) and assert invariants hold across hundreds of iterations:

```sh
cargo test -p crawl-boi-core -- --nocapture
```

The property tests cover things like: URLs never get fetched twice even when the link graph has cycles, page and path budgets are respected, scope checks reject anything off-host, fragments get stripped, JSONL round-trips cleanly, robots parsing is deterministic, and bad input gets caught at the CLI boundary.

To run a longer property test session (useful for shaking out edge cases):

```sh
PROPTEST_CASES=10000 cargo test -p crawl-boi-core
```

## Natural extensions

The following didn't make it into this version, but the code is shaped so they slot in without reworking the core loop include javascript rendering (though I started building the feature and you cansee it exists), crawl-delay and allow directives, meta robots tags and x-robots-tag (though robots parsing is in), 429 back-off, streaming to disk, multi-domain crawling (I ran out of time to add this as a cli toggle).
