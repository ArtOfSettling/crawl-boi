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

#[derive(Debug, Clone, Default)]
pub struct CrawlBudget {
    pub max_pages: Option<usize>,
    pub path_limits: Vec<(String, usize)>,
}

impl CrawlBudget {
    pub fn matching_prefix(&self, path: &str) -> Option<&str> {
        self.path_limits
            .iter()
            .filter(|(prefix, _)| path.starts_with(prefix.as_str()))
            .max_by_key(|(prefix, _)| prefix.len())
            .map(|(prefix, _)| prefix.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn overlapping_prefixes_returns_longest_match() {
        let budget = CrawlBudget {
            max_pages: None,
            path_limits: vec![
                ("/blog/".to_string(), 10),
                ("/blog/2024/".to_string(), 5),
                ("/blog/2024/june/".to_string(), 2),
            ],
        };
        assert_eq!(
            budget.matching_prefix("/blog/2024/june/post"),
            Some("/blog/2024/june/")
        );
    }

    #[test]
    fn no_matching_prefix_returns_none() {
        let budget = CrawlBudget {
            max_pages: None,
            path_limits: vec![("/blog/".to_string(), 10), ("/docs/".to_string(), 5)],
        };
        assert_eq!(budget.matching_prefix("/about"), None);
    }

    #[test]
    fn exact_path_match_works() {
        let budget = CrawlBudget {
            max_pages: None,
            path_limits: vec![("/exact".to_string(), 3)],
        };
        assert_eq!(budget.matching_prefix("/exact"), Some("/exact"));
    }

    #[test]
    fn empty_path_limits_always_returns_none() {
        let budget = CrawlBudget::default();
        assert_eq!(budget.matching_prefix("/anything/here"), None);
        assert_eq!(budget.matching_prefix("/"), None);
        assert_eq!(budget.matching_prefix(""), None);
    }

    fn arb_prefix() -> impl Strategy<Value = String> {
        proptest::collection::vec("[a-z]{1,5}", 1..=4)
            .prop_map(|segments| format!("/{}/", segments.join("/")))
    }

    fn arb_path_limits() -> impl Strategy<Value = Vec<(String, usize)>> {
        proptest::collection::vec((arb_prefix(), 1..100usize), 1..=6)
    }

    // Longest prefix match
    proptest! {
        #[test]
        fn longest_prefix_match(
            path_limits in arb_path_limits(),
            suffix in "[a-z]{0,10}",
        ) {
            // Pick a random existing prefix and extend it to form the test path
            let idx = suffix.len() % path_limits.len();
            let base_prefix = &path_limits[idx].0;
            let path = format!("{base_prefix}{suffix}");

            let budget = CrawlBudget {
                max_pages: None,
                path_limits: path_limits.clone(),
            };

            let result = budget.matching_prefix(&path);

            // Compute the expected longest matching prefix manually
            let expected = path_limits
                .iter()
                .filter(|(prefix, _)| path.starts_with(prefix.as_str()))
                .max_by_key(|(prefix, _)| prefix.len())
                .map(|(prefix, _)| prefix.as_str());

            prop_assert_eq!(result, expected);
        }
    }
}
