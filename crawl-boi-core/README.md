# crawl-boi-core

This crate is a work in progress.

`crawl-boi-core` is the library crate for the Crawl Boi web crawler. It provides all core
crawling logic as a public API with no dependency on any CLI framework, so that it can be
used and tested in isolation.

## Purpose

The crate will expose the components needed to crawl a website: an HTTP fetcher, an HTML
link parser, a robots.txt parser, a crawl scope filter, and the crawler orchestrator itself.
The CLI crate (`crawl-boi-cli`) depends on this crate and handles argument parsing and output
formatting.

## Status

The public API surface is not yet available. Subsequent tasks will introduce types and
functions incrementally, each leaving `cargo build` and `cargo test` passing.
