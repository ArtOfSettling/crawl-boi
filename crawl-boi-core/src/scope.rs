use url::Url;

pub struct CrawlScope {
    allowed_host: String,
    seed_scheme: String,
}

impl CrawlScope {
    pub fn new(seed: &Url) -> Self {
        CrawlScope {
            allowed_host: seed.host_str().unwrap_or("").to_owned(),
            seed_scheme: seed.scheme().to_owned(),
        }
    }

    /// Returns true iff the URL uses http or https and its host exactly matches the seed host.
    pub fn is_allowed(&self, url: &Url) -> bool {
        // Strip fragment by working with scheme and host directly; the Url type already
        // excludes fragments from host()/scheme() accessors, but we are explicit here.
        let scheme = url.scheme();
        if scheme != "http" && scheme != "https" {
            return false;
        }
        let host = url.host_str().unwrap_or("");
        scheme == self.seed_scheme && host == self.allowed_host
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use url::Url;

    /// Generate a simple ASCII hostname like "example.com" or "sub.host.org".
    fn arb_hostname() -> impl Strategy<Value = String> {
        // label: 1-8 lowercase ascii letters
        let label = "[a-z]{1,8}";
        // hostname: 1-3 dot-separated labels
        (label, proptest::option::of((label, proptest::option::of(label))))
            .prop_map(|(first, rest)| match rest {
                None => first.to_owned(),
                Some((second, None)) => format!("{}.{}", first, second),
                Some((second, Some(third))) => format!("{}.{}.{}", first, second, third),
            })
    }

    fn arb_http_scheme() -> impl Strategy<Value = &'static str> {
        prop_oneof![Just("http"), Just("https")]
    }

    fn arb_non_http_scheme() -> impl Strategy<Value = &'static str> {
        prop_oneof![Just("ftp"), Just("ftps"), Just("file"), Just("ws"), Just("wss")]
    }

    proptest! {
        #[test]
        fn prop_same_scheme_same_host_allowed(
            scheme in arb_http_scheme(),
            host in arb_hostname(),
            path in "/[a-z0-9/]{0,20}",
        ) {
            let seed_str = format!("{}://{}/seed", scheme, host);
            let candidate_str = format!("{}://{}{}", scheme, host, path);

            let seed = Url::parse(&seed_str).unwrap();
            let candidate = Url::parse(&candidate_str).unwrap();
            let scope = CrawlScope::new(&seed);

            prop_assert!(scope.is_allowed(&candidate));
        }

        #[test]
        fn prop_different_host_rejected(
            scheme in arb_http_scheme(),
            seed_host in arb_hostname(),
            candidate_host in arb_hostname(),
        ) {
            prop_assume!(seed_host != candidate_host);

            let seed = Url::parse(&format!("{}://{}/", scheme, seed_host)).unwrap();
            let candidate = Url::parse(&format!("{}://{}/page", scheme, candidate_host)).unwrap();
            let scope = CrawlScope::new(&seed);

            prop_assert!(!scope.is_allowed(&candidate));
        }

        #[test]
        fn prop_non_http_scheme_rejected(
            seed_scheme in arb_http_scheme(),
            host in arb_hostname(),
            bad_scheme in arb_non_http_scheme(),
        ) {
            let seed = Url::parse(&format!("{}://{}/", seed_scheme, host)).unwrap();
            let candidate_str = format!("{}://{}/page", bad_scheme, host);
            if let Ok(candidate) = Url::parse(&candidate_str) {
                let scope = CrawlScope::new(&seed);
                prop_assert!(!scope.is_allowed(&candidate));
            }
        }

        #[test]
        fn prop_fragment_does_not_affect_scope(
            scheme in arb_http_scheme(),
            host in arb_hostname(),
            fragment in "[a-z]{1,10}",
        ) {
            let seed = Url::parse(&format!("{}://{}/", scheme, host)).unwrap();
            let candidate = Url::parse(&format!("{}://{}/page#{}", scheme, host, fragment)).unwrap();
            let scope = CrawlScope::new(&seed);

            prop_assert!(scope.is_allowed(&candidate));
        }
    }

    #[test]
    fn same_host_allowed() {
        let seed = Url::parse("http://example.com/start").unwrap();
        let scope = CrawlScope::new(&seed);
        let candidate = Url::parse("http://example.com/page").unwrap();
        assert!(scope.is_allowed(&candidate));
    }

    #[test]
    fn different_host_rejected() {
        let seed = Url::parse("http://example.com").unwrap();
        let scope = CrawlScope::new(&seed);
        let candidate = Url::parse("http://other.com/page").unwrap();
        assert!(!scope.is_allowed(&candidate));
    }

    #[test]
    fn mailto_scheme_rejected() {
        let seed = Url::parse("http://example.com").unwrap();
        let scope = CrawlScope::new(&seed);
        let candidate = Url::parse("mailto:user@example.com").unwrap();
        assert!(!scope.is_allowed(&candidate));
    }

    #[test]
    fn ftp_scheme_rejected() {
        let seed = Url::parse("http://example.com").unwrap();
        let scope = CrawlScope::new(&seed);
        let candidate = Url::parse("ftp://example.com/file.txt").unwrap();
        assert!(!scope.is_allowed(&candidate));
    }

    #[test]
    fn https_seed_rejects_http_candidate() {
        let seed = Url::parse("https://example.com").unwrap();
        let scope = CrawlScope::new(&seed);
        let candidate = Url::parse("http://example.com/page").unwrap();
        assert!(!scope.is_allowed(&candidate));
    }

    #[test]
    fn http_seed_rejects_https_candidate() {
        let seed = Url::parse("http://example.com").unwrap();
        let scope = CrawlScope::new(&seed);
        let candidate = Url::parse("https://example.com/page").unwrap();
        assert!(!scope.is_allowed(&candidate));
    }

    #[test]
    fn fragment_stripped_before_comparison() {
        let seed = Url::parse("http://example.com").unwrap();
        let scope = CrawlScope::new(&seed);
        let candidate = Url::parse("http://example.com/page#section").unwrap();
        assert!(scope.is_allowed(&candidate));
    }
}
