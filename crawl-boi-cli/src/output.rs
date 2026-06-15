use std::io::{self, Write};

use crawl_boi_core::PageResult;
use serde_json;

/// Prints a page result in plain-text format to stdout.
/// The visited URL appears on its own line, followed by each link indented with two spaces.
pub fn print_plain(result: &PageResult) {
    println!("{}", result.url);
    for link in &result.links {
        println!("  {link}");
    }
}

/// Formats a page result as plain text, writing to the provided writer.
/// Useful for testing without capturing stdout.
pub fn format_plain<W: Write>(writer: &mut W, result: &PageResult) -> io::Result<()> {
    writeln!(writer, "{}", result.url)?;
    for link in &result.links {
        writeln!(writer, "  {link}")?;
    }
    Ok(())
}

/// Prints a page result as a single JSONL line to stdout.
/// On serialisation failure, writes an error record to stderr.
pub fn print_jsonl(result: &PageResult) {
    match serde_json::to_string(result) {
        Ok(line) => println!("{line}"),
        Err(e) => eprintln!("{{\"error\":\"serialisation failed\",\"url\":\"{}\",\"detail\":\"{e}\"}}", result.url),
    }
}

/// Formats a page result as a single JSONL line, writing to the provided writer.
/// On serialisation failure, writes an error record to `err_writer`.
pub fn format_jsonl<W: Write, E: Write>(
    writer: &mut W,
    err_writer: &mut E,
    result: &PageResult,
) -> io::Result<()> {
    match serde_json::to_string(result) {
        Ok(line) => writeln!(writer, "{line}"),
        Err(e) => writeln!(
            err_writer,
            "{{\"error\":\"serialisation failed\",\"url\":\"{}\",\"detail\":\"{e}\"}}",
            result.url
        ),
    }
}

/// Formats a fetch error, writing to the provided writer.
pub fn format_error<W: Write>(writer: &mut W, url: &url::Url, error: &str) -> io::Result<()> {
    writeln!(writer, "{url}: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crawl_boi_core::PageResult;
    use proptest::prelude::*;
    use url::Url;

    #[test]
    fn page_with_links_formats_correctly() {
        let result = PageResult {
            url: Url::parse("https://example.com/").unwrap(),
            links: vec![
                Url::parse("https://example.com/a").unwrap(),
                Url::parse("https://example.com/b").unwrap(),
            ],
        };
        let mut buf = Vec::new();
        format_plain(&mut buf, &result).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(
            output,
            "https://example.com/\n  https://example.com/a\n  https://example.com/b\n"
        );
    }

    #[test]
    fn page_with_no_links_produces_single_line() {
        let result = PageResult {
            url: Url::parse("https://example.com/lonely").unwrap(),
            links: vec![],
        };
        let mut buf = Vec::new();
        format_plain(&mut buf, &result).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output, "https://example.com/lonely\n");
    }

    #[test]
    fn error_written_to_writer() {
        let url = Url::parse("https://example.com/broken").unwrap();
        let mut buf = Vec::new();
        format_error(&mut buf, &url, "HTTP error: status 500").unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output, "https://example.com/broken: HTTP error: status 500\n");
    }

    #[test]
    fn jsonl_correct_json_structure() {
        let result = PageResult {
            url: Url::parse("https://example.com/page").unwrap(),
            links: vec![
                Url::parse("https://example.com/a").unwrap(),
                Url::parse("https://example.com/b").unwrap(),
            ],
        };
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        format_jsonl(&mut stdout, &mut stderr, &result).unwrap();
        let output = String::from_utf8(stdout).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
        assert_eq!(parsed["url"], "https://example.com/page");
        assert!(parsed["links"].is_array());
        assert_eq!(parsed["links"].as_array().unwrap().len(), 2);
        assert!(stderr.is_empty());
    }

    #[test]
    fn jsonl_links_array_present_when_empty() {
        let result = PageResult {
            url: Url::parse("https://example.com/alone").unwrap(),
            links: vec![],
        };
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        format_jsonl(&mut stdout, &mut stderr, &result).unwrap();
        let output = String::from_utf8(stdout).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
        assert_eq!(parsed["url"], "https://example.com/alone");
        assert!(parsed["links"].is_array());
        assert_eq!(parsed["links"].as_array().unwrap().len(), 0);
        assert!(stderr.is_empty());
    }

    #[test]
    fn jsonl_output_is_single_line() {
        let result = PageResult {
            url: Url::parse("https://example.com/x").unwrap(),
            links: vec![
                Url::parse("https://example.com/a").unwrap(),
                Url::parse("https://example.com/b").unwrap(),
                Url::parse("https://example.com/c").unwrap(),
            ],
        };
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        format_jsonl(&mut stdout, &mut stderr, &result).unwrap();
        let output = String::from_utf8(stdout).unwrap();
        // JSONL: exactly one newline at the end
        assert_eq!(output.lines().count(), 1);
    }

    fn arb_path_segment() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9]{0,8}".prop_map(|s| s)
    }

    fn arb_url() -> impl Strategy<Value = Url> {
        (1..5usize, prop::collection::vec(arb_path_segment(), 1..4)).prop_map(
            |(_, segments)| {
                let path = segments.join("/");
                Url::parse(&format!("https://example.com/{path}")).unwrap()
            },
        )
    }

    fn arb_page_result() -> impl Strategy<Value = PageResult> {
        (arb_url(), prop::collection::vec(arb_url(), 0..10)).prop_map(|(url, links)| {
            PageResult { url, links }
        })
    }

    proptest! {
        // Plain-text format structure
        #[test]
        fn plain_text_format_structure(result in arb_page_result()) {
            let mut buf = Vec::new();
            format_plain(&mut buf, &result).unwrap();
            let output = String::from_utf8(buf).unwrap();
            let lines: Vec<&str> = output.lines().collect();

            // First line is the visited URL
            prop_assert_eq!(lines[0], result.url.as_str());

            if result.links.is_empty() {
                // No links means exactly one line
                prop_assert_eq!(lines.len(), 1);
            } else {
                // Each link appears indented on its own line
                prop_assert_eq!(lines.len(), 1 + result.links.len());
                for (i, link) in result.links.iter().enumerate() {
                    prop_assert!(lines[i + 1].starts_with("  "));
                    prop_assert_eq!(lines[i + 1].trim(), link.as_str());
                }
            }
        }

        // JSONL serialisation round-trip
        #[test]
        fn jsonl_round_trip(result in arb_page_result()) {
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            format_jsonl(&mut stdout, &mut stderr, &result).unwrap();
            let output = String::from_utf8(stdout).unwrap();
            let decoded: PageResult = serde_json::from_str(output.trim()).unwrap();
            prop_assert_eq!(result, decoded);
        }
    }
}
