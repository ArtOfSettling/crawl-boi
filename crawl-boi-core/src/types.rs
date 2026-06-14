use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageResult {
    pub url: Url,
    pub links: Vec<Url>,
}

#[derive(Debug, Clone)]
pub struct CrawlConfig {
    pub seed: Url,
    pub concurrency: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FetchError {
    Http { status: u16 },
    Network(String),
    NonUtf8Body,
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchError::Http { status } => write!(f, "HTTP error: status {status}"),
            FetchError::Network(msg) => write!(f, "Network error: {msg}"),
            FetchError::NonUtf8Body => write!(f, "Response body is not valid UTF-8"),
        }
    }
}

impl std::error::Error for FetchError {}
