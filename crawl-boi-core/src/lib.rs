// crawl-boi-core library root

mod types;
pub mod scope;
pub mod parser;
pub mod robots;

pub use types::{CrawlConfig, FetchError, PageResult};
pub use scope::CrawlScope;
pub use parser::extract_links;
pub use robots::{RobotsParser, RobotsRules};
