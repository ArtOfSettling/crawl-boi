use async_trait::async_trait;
use url::Url;

use crate::{FetchError, Fetcher};

#[allow(dead_code)]
pub struct JsFetcher;

#[async_trait]
impl Fetcher for JsFetcher {
    async fn fetch(&self, _url: &Url) -> Result<String, FetchError> {
        unimplemented!("JS rendering is not implemented in v1")
    }
}
