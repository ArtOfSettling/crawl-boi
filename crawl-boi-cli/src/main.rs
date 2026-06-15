mod args;
mod output;

use clap::Parser;
use crawl_boi_core::{CrawlBudget, CrawlConfig, Crawler, HttpFetcher};

use args::{Args, Commands, parse_path_budget, validate_url};
use output::print_plain;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let seed = match validate_url(&args.url) {
        Ok(url) => url,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };

    let (concurrency, max_pages, path_budget_raw) = match args.command {
        Commands::Crawl {
            concurrency,
            max_pages,
            path_budget,
        } => (concurrency.unwrap_or(8), max_pages, path_budget),
    };

    let mut path_limits = Vec::with_capacity(path_budget_raw.len());
    for raw in &path_budget_raw {
        match parse_path_budget(raw) {
            Ok(entry) => path_limits.push(entry),
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
    }

    let budget = CrawlBudget {
        max_pages,
        path_limits,
    };

    let config = CrawlConfig { seed, concurrency };
    let fetcher = HttpFetcher::new();
    let crawler = Crawler::new(config, fetcher, budget);
    let mut rx = crawler.run();

    while let Some(result) = rx.recv().await {
        print_plain(&result);
    }
}
