use std::collections::HashSet;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use futures::stream::{FuturesUnordered, StreamExt};
use url::Url;

use crate::fetcher::Fetcher;
use crate::parser::extract_links;
use crate::robots::RobotsParser;
use crate::scope::CrawlScope;
use crate::types::{CrawlConfig, FetchError, PageResult};

/// A FuturesUnordered is a bog of tasks running at the same time. The boxed future is a wrapper that
/// homogenises those tasks so they can sit in the same pool.
type FetchTask = Pin<Box<dyn std::future::Future<Output = (Url, Result<String, FetchError>)> + Send>>;

pub struct Crawler<F: Fetcher> {
    config: CrawlConfig,
    scope: CrawlScope,
    fetcher: F,
}

impl<F: Fetcher + 'static> Crawler<F> {
    pub fn new(config: CrawlConfig, fetcher: F) -> Self {
        let scope = CrawlScope::new(&config.seed);
        Self { config, scope, fetcher }
    }

    pub async fn run(self) -> Vec<PageResult> {
        let robots_rules = self.fetch_robots().await;

        let visited: Arc<Mutex<HashSet<Url>>> = Arc::new(Mutex::new(HashSet::new()));
        let mut frontier: Vec<Url> = Vec::new();
        let mut results: Vec<PageResult> = Vec::new();

        // Seed the frontier and visited set.
        // This does a BFS with concurrent fetching layered on top.
        // The frontier is the queue, we keep the visited set and process level-by-level. Instead of visiting 
        // one at a time, we drain up to 'concurrency' URLs at once into a pool and fetch them in parallel.
        // When a fetch completes, newly discovered links are added to the back of the frontier.
        // Essentially BFS ordering within each 'wave'.
        //
        // Risks : 
        // * no crawl budget implemented yet, so, yeah.... Memroy explosion
        // * No server backpressure can in theory result in an endlessly growing frontier
        // * If the robots changes during processing ... we just keep processing as it is fetched once (unlikely, but possible).
        // * Current implementation does not surface failures to the caller.
        {
            let mut v = visited.lock().unwrap();
            v.insert(self.config.seed.clone());
        }
        frontier.push(self.config.seed.clone());

        let fetcher = Arc::new(self.fetcher);

        while !frontier.is_empty() {
            let mut tasks: FuturesUnordered<FetchTask> = FuturesUnordered::new();

            // Drain up to concurrency URLs into in-flight tasks.
            let batch: Vec<Url> = frontier
                .drain(..frontier.len().min(self.config.concurrency))
                .collect();

            for url in batch {
                let fetcher = Arc::clone(&fetcher);
                tasks.push(Box::pin(async move {
                    let result = fetcher.fetch(&url).await;
                    (url, result)
                }));
            }

            // Collect completed tasks, replenishing from the frontier as slots open.
            while let Some((url, fetch_result)) = tasks.next().await {
                match fetch_result {
                    Ok(body) => {
                        let links = extract_links(&url, &body);
                        let mut new_urls: Vec<Url> = Vec::new();

                        for link in &links {
                            if !self.scope.is_allowed(link) {
                                continue;
                            }
                            if !robots_rules.is_allowed(link.path()) {
                                continue;
                            }
                            let mut v = visited.lock().unwrap();
                            if v.insert(link.clone()) {
                                new_urls.push(link.clone());
                            }
                        }

                        frontier.extend(new_urls);
                        results.push(PageResult { url, links });
                    }
                    Err(err) => {
                        eprintln!("error fetching {url}: {err}");
                    }
                }

                // If the frontier has grown and we have spare capacity, dispatch more tasks.
                while tasks.len() < self.config.concurrency && !frontier.is_empty() {
                    let next_url = frontier.remove(0);
                    let fetcher = Arc::clone(&fetcher);
                    tasks.push(Box::pin(async move {
                        let result = fetcher.fetch(&next_url).await;
                        (next_url, result)
                    }));
                }
            }
        }

        results
    }

    async fn fetch_robots(&self) -> crate::robots::RobotsRules {
        let mut robots_url = self.config.seed.clone();
        robots_url.set_path("/robots.txt");
        robots_url.set_query(None);
        robots_url.set_fragment(None);

        match self.fetcher.fetch(&robots_url).await {
            Ok(body) => RobotsParser::parse(&body),
            Err(FetchError::Http { status: 404 }) => {
                eprintln!("warning: robots.txt not found at {robots_url}, proceeding without restrictions");
                RobotsParser::parse("")
            }
            Err(err) => {
                eprintln!("warning: could not fetch robots.txt at {robots_url}: {err}, proceeding without restrictions");
                RobotsParser::parse("")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::collections::HashMap;

    struct MockFetcher {
        responses: HashMap<Url, Result<String, FetchError>>,
    }

    impl MockFetcher {
        fn new(responses: HashMap<Url, Result<String, FetchError>>) -> Self {
            Self { responses }
        }
    }

    #[async_trait]
    impl Fetcher for MockFetcher {
        async fn fetch(&self, url: &Url) -> Result<String, FetchError> {
            // Strip fragment before lookup so the mock matches how the crawler uses URLs.
            let mut lookup = url.clone();
            lookup.set_fragment(None);
            self.responses
                .get(&lookup)
                .cloned()
                .unwrap_or(Err(FetchError::Http { status: 404 }))
        }
    }

    fn config(seed: &str) -> CrawlConfig {
        CrawlConfig {
            seed: Url::parse(seed).unwrap(),
            concurrency: 4,
        }
    }

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    fn html_with_links(links: &[&str]) -> String {
        let anchors: String = links
            .iter()
            .map(|href| format!(r#"<a href="{href}">link</a>"#))
            .collect::<Vec<_>>()
            .join("\n");
        format!("<html><body>{anchors}</body></html>")
    }

    fn sorted_urls(results: &[PageResult]) -> Vec<Url> {
        let mut urls: Vec<Url> = results.iter().map(|r| r.url.clone()).collect();
        urls.sort();
        urls
    }

    #[tokio::test]
    async fn single_page_site() {
        let seed = "http://example.com/";
        let mut responses = HashMap::new();
        responses.insert(url(seed), Ok(html_with_links(&[])));
        // robots.txt → 404 by default (not in map)

        let results = Crawler::new(config(seed), MockFetcher::new(responses))
            .run()
            .await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, url(seed));
        assert!(results[0].links.is_empty());
    }

    #[tokio::test]
    async fn multi_page_site_with_links() {
        let seed = "http://example.com/";
        let page_a = "http://example.com/a";
        let page_b = "http://example.com/b";

        let mut responses = HashMap::new();
        responses.insert(url(seed), Ok(html_with_links(&[page_a, page_b])));
        responses.insert(url(page_a), Ok(html_with_links(&[])));
        responses.insert(url(page_b), Ok(html_with_links(&[])));

        let results = Crawler::new(config(seed), MockFetcher::new(responses))
            .run()
            .await;

        let visited = sorted_urls(&results);
        assert_eq!(visited.len(), 3);
        assert!(visited.contains(&url(seed)));
        assert!(visited.contains(&url(page_a)));
        assert!(visited.contains(&url(page_b)));
    }

    #[tokio::test]
    async fn site_graph_with_cycle() {
        // a → b → a (cycle); crawler must not fetch any URL more than once.
        let seed = "http://example.com/a";
        let page_b = "http://example.com/b";

        let mut responses = HashMap::new();
        responses.insert(url(seed), Ok(html_with_links(&[page_b])));
        responses.insert(url(page_b), Ok(html_with_links(&[seed])));

        let results = Crawler::new(config(seed), MockFetcher::new(responses))
            .run()
            .await;

        let visited = sorted_urls(&results);
        assert_eq!(visited.len(), 2, "each URL fetched exactly once; got {visited:?}");
        assert!(visited.contains(&url(seed)));
        assert!(visited.contains(&url(page_b)));
    }

    #[tokio::test]
    async fn fetch_error_crawl_continues() {
        // The seed links to /a (error) and /b (ok). Crawl must still complete /b.
        let seed = "http://example.com/";
        let page_a = "http://example.com/a";
        let page_b = "http://example.com/b";

        let mut responses = HashMap::new();
        responses.insert(url(seed), Ok(html_with_links(&[page_a, page_b])));
        responses.insert(url(page_a), Err(FetchError::Http { status: 500 }));
        responses.insert(url(page_b), Ok(html_with_links(&[])));

        let results = Crawler::new(config(seed), MockFetcher::new(responses))
            .run()
            .await;

        // Only successful fetches produce PageResults.
        let visited = sorted_urls(&results);
        assert!(!visited.contains(&url(page_a)), "errored page should not appear in results");
        assert!(visited.contains(&url(seed)));
        assert!(visited.contains(&url(page_b)));
    }

    #[tokio::test]
    async fn out_of_scope_links_discarded() {
        let seed = "http://example.com/";
        let external = "http://other.com/page";
        let internal = "http://example.com/internal";

        let mut responses = HashMap::new();
        responses.insert(url(seed), Ok(html_with_links(&[external, internal])));
        responses.insert(url(internal), Ok(html_with_links(&[])));

        let results = Crawler::new(config(seed), MockFetcher::new(responses))
            .run()
            .await;

        let visited = sorted_urls(&results);
        assert!(!visited.contains(&url(external)), "external URL must not be visited");
        assert!(visited.contains(&url(seed)));
        assert!(visited.contains(&url(internal)));
    }

    #[tokio::test]
    async fn robots_disallowed_urls_not_visited() {
        let seed = "http://example.com/";
        let allowed = "http://example.com/allowed";
        let disallowed = "http://example.com/private/secret";

        let robots_body =
            "User-agent: *\nDisallow: /private\n".to_owned();

        let mut responses = HashMap::new();
        responses.insert(
            url("http://example.com/robots.txt"),
            Ok(robots_body),
        );
        responses.insert(
            url(seed),
            Ok(html_with_links(&[allowed, disallowed])),
        );
        responses.insert(url(allowed), Ok(html_with_links(&[])));
        // /private/secret is NOT in the map; any fetch would return 404.

        let results = Crawler::new(config(seed), MockFetcher::new(responses))
            .run()
            .await;

        let visited = sorted_urls(&results);
        assert!(
            !visited.contains(&url(disallowed)),
            "robots-disallowed URL must not be visited"
        );
        assert!(visited.contains(&url(seed)));
        assert!(visited.contains(&url(allowed)));
    }

    use proptest::prelude::*;

    /// Generates a mock site graph as a HashMap<Url, Result<String, FetchError>>.
    ///
    /// The graph has a fixed seed http://example.com/ and up to max_pages additional  pages under the 
    // same host. Each page links to a random subset of the other pages. This produces graphs that may contain cycles, dead ends, and disconnected nodes.
    fn arb_site_graph(
        max_pages: usize,
    ) -> impl Strategy<Value = HashMap<Url, Result<String, FetchError>>> {
        // Generate between 0 and max_pages additional path segments.
        proptest::collection::vec(
            proptest::string::string_regex("[a-z]{1,6}").unwrap(),
            0..=max_pages,
        )
        .prop_map(|paths| {
            // Deduplicate to avoid duplicate URLs.
            let mut unique_paths: Vec<String> = paths;
            unique_paths.sort();
            unique_paths.dedup();

            let seed_str = "http://example.com/".to_owned();
            let all_urls: Vec<String> = std::iter::once(seed_str.clone())
                .chain(
                    unique_paths
                        .iter()
                        .map(|p| format!("http://example.com/{p}")),
                )
                .collect();

            // Build each page's HTML so that it links to every other page in the graph.
            // This maximises connectivity and therefore maximises opportunities for the crawler to 
            // revisit a URL (which it must not do).
            let mut responses: HashMap<Url, Result<String, FetchError>> = HashMap::new();
            for page_url in &all_urls {
                let link_strs: Vec<&str> = all_urls
                    .iter()
                    .filter(|u| *u != page_url)
                    .map(|u| u.as_str())
                    .collect();
                let html = {
                    let anchors: String = link_strs
                        .iter()
                        .map(|href| format!(r#"<a href="{href}">link</a>"#))
                        .collect::<Vec<_>>()
                        .join("\n");
                    format!("<html><body>{anchors}</body></html>")
                };
                responses.insert(Url::parse(page_url).unwrap(), Ok(html));
            }
            responses
        })
    }

    proptest! {
        // No URL is fetched more than once
        #[test]
        fn no_url_fetched_more_than_once(responses in arb_site_graph(8)) {
            let seed = "http://example.com/";
            let cfg = CrawlConfig {
                seed: Url::parse(seed).unwrap(),
                concurrency: 4,
            };

            let results = tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(Crawler::new(cfg, MockFetcher::new(responses)).run());

            let mut seen = HashSet::new();
            for r in &results {
                prop_assert!(
                    seen.insert(r.url.clone()),
                    "URL fetched more than once: {}",
                    r.url
                );
            }
        }
    }
}
