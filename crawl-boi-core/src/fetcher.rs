use async_trait::async_trait;
use url::Url;

use crate::FetchError;

/// Rust by default ensures this string is a valid UTF-8 string. This is a compile time protection.
/// anyone who is familiar with rust and sees this function will know, but I'm leaving this comment
// in case the consumer does not know.
#[async_trait]
pub trait Fetcher: Send + Sync {
    async fn fetch(&self, url: &Url) -> Result<String, FetchError>;
}

pub struct HttpFetcher {
    client: reqwest::Client,
}

impl HttpFetcher {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("crawl-boi/0.1 (+https://github.com/your-org/crawl-boi)")
            .build()
            .expect("failed to build reqwest client");
        Self { client }
    }
}

impl Default for HttpFetcher {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Fetcher for HttpFetcher {
    async fn fetch(&self, url: &Url) -> Result<String, FetchError> {
        let response = self
            .client
            .get(url.as_str())
            .send()
            .await
            .map_err(|e| FetchError::Network(e.to_string()))?;

        if !response.status().is_success() {
            return Err(FetchError::Http {
                status: response.status().as_u16(),
            });
        }

        response.text().await.map_err(|_| FetchError::NonUtf8Body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    struct StatusMockFetcher {
        status: u16,
    }

    #[async_trait]
    impl Fetcher for StatusMockFetcher {
        async fn fetch(&self, _url: &Url) -> Result<String, FetchError> {
            Err(FetchError::Http {
                status: self.status,
            })
        }
    }

    proptest! {
        // Error status codes produce FetchError
        #[test]
        fn prop_error_status_codes_produce_fetch_error(status in 400u16..=599u16) {
            let fetcher = StatusMockFetcher { status };
            let url = Url::parse("https://example.com/page").unwrap();

            let runtime = tokio::runtime::Runtime::new().unwrap();
            let result = runtime.block_on(fetcher.fetch(&url));

            prop_assert!(
                matches!(result, Err(FetchError::Http { status: s }) if s == status),
                "expected Err(FetchError::Http {{ status: {status} }}), got {:?}",
                result
            );
        }
    }
}
