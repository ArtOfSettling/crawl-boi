// crawl-boi-core library root

pub mod crawler;
pub mod fetcher;
#[cfg(feature = "js-rendering")]
mod js_fetcher;
pub mod parser;
pub mod robots;
pub mod scope;
mod types;

pub use crawler::Crawler;
pub use fetcher::{Fetcher, HttpFetcher};
pub use parser::extract_links;
pub use robots::{RobotsParser, RobotsRules};
pub use scope::CrawlScope;
pub use types::{CrawlBudget, CrawlConfig, FetchError, PageResult};
