# crawl-boi-cli

This crate is a work in progress.

`crawl-boi-cli` is the binary crate for the Crawl Boi web crawler. It handles command-line
argument parsing and output formatting, delegating all crawling logic to `crawl-boi-core`.

## Purpose

The crate will provide a `crawl` subcommand that accepts a seed URL and optional flags,
invokes the crawler from `crawl-boi-core`, and writes results to stdout in either plain-text
or JSONL format.

## Status

The CLI is not yet wired up. The binary currently prints `crawl-boi` and exits. Subsequent
tasks will introduce argument parsing, output formatting, and full integration with the core
library.
