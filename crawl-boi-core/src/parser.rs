use scraper::{Html, Selector};
use url::Url;

pub fn extract_links(base: &Url, html: &str) -> Vec<Url> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("a[href]").expect("valid static selector");

    let mut links: Vec<Url> = document
        .select(&selector)
        .filter_map(|el| el.value().attr("href"))
        .filter_map(|href| base.join(href).ok())
        .map(|mut url| {
            url.set_fragment(None);
            url
        })
        .collect();

    links.sort();
    links.dedup();
    links
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn base() -> Url {
        Url::parse("https://example.com/page/").unwrap()
    }

    /// Arbitrary path segment: alphanumeric, non-empty, no special chars that break URL parsing.
    fn arb_path_segment() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9]{0,9}".prop_map(|s| s)
    }

    fn arb_fragment() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9]{0,9}".prop_map(|s| s)
    }

    #[test]
    fn absolute_href_used_as_is() {
        let html = r#"<a href="https://other.com/foo">link</a>"#;
        let links = extract_links(&base(), html);
        assert_eq!(links, vec![Url::parse("https://other.com/foo").unwrap()]);
    }

    #[test]
    fn relative_href_resolved_against_base() {
        let html = r#"<a href="../bar">link</a>"#;
        let links = extract_links(&base(), html);
        assert_eq!(links, vec![Url::parse("https://example.com/bar").unwrap()]);
    }

    #[test]
    fn anchor_with_no_href_skipped() {
        let html = r#"<a name="top">no href</a>"#;
        let links = extract_links(&base(), html);
        assert!(links.is_empty());
    }

    #[test]
    fn fragment_stripped() {
        let html = r#"<a href="https://example.com/page#section">link</a>"#;
        let links = extract_links(&base(), html);
        assert_eq!(links, vec![Url::parse("https://example.com/page").unwrap()]);
    }

    #[test]
    fn duplicate_links_deduplicated() {
        let html = r#"
            <a href="https://example.com/a">first</a>
            <a href="https://example.com/a">second</a>
            <a href="https://example.com/a#frag">third</a>
        "#;
        let links = extract_links(&base(), html);
        assert_eq!(links, vec![Url::parse("https://example.com/a").unwrap()]);
    }

    #[test]
    fn links_returned_sorted() {
        let html = r#"
            <a href="https://example.com/z">z</a>
            <a href="https://example.com/a">a</a>
            <a href="https://example.com/m">m</a>
        "#;
        let links = extract_links(&base(), html);
        let mut expected = links.clone();
        expected.sort();
        assert_eq!(links, expected);
    }

    proptest! {
        // crawl-boi,  Fragment identifiers are stripped from extracted links
        #[test]
        fn prop_fragments_stripped(path in arb_path_segment(), frag in arb_fragment()) {
            let href = format!("https://example.com/{path}#{frag}");
            let html = format!(r#"<a href="{href}">link</a>"#);
            let links = extract_links(&base(), &html);
            prop_assert!(links.iter().all(|u| u.fragment().is_none()),
                "found fragment in {:?}", links);
        }

        // crawl-boi, All <a href> elements are extracted
        #[test]
        fn prop_all_hrefs_extracted(paths in proptest::collection::vec(arb_path_segment(), 1..=8)) {
            // Build HTML with one <a> per path, all distinct absolute URLs.
            let unique_paths: Vec<String> = {
                let mut v = paths.clone();
                v.sort();
                v.dedup();
                v
            };
            let html: String = unique_paths
                .iter()
                .map(|p| format!(r#"<a href="https://example.com/{p}">x</a>"#))
                .collect::<Vec<_>>()
                .join("\n");
            let links = extract_links(&base(), &html);
            for path in &unique_paths {
                let expected = Url::parse(&format!("https://example.com/{path}")).unwrap();
                prop_assert!(links.contains(&expected),
                    "missing {expected} in {links:?}");
            }
        }

        // crawl-boi, Relative hrefs are resolved against the base URL
        #[test]
        fn prop_relative_hrefs_resolved(path in arb_path_segment()) {
            let relative_href = format!("/resolved/{path}");
            let html = format!(r#"<a href="{relative_href}">link</a>"#);
            let links = extract_links(&base(), &html);
            let expected = base().join(&relative_href).unwrap();
            prop_assert!(links.contains(&expected),
                "expected {expected} in {links:?}");
        }
    }
}
