use clap::{Parser, Subcommand};
use url::Url;

/// Comments are needed in this file to visualize 'help' to users of the CLI, that is their sole purpose, they are not here
/// to document the code or the functionality in any way.

#[derive(Debug, Parser)]
#[command(name = "crawl-boi", about = "A concurrent web crawler")]
pub struct Args {
    #[arg(long)]
    pub url: String,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Crawl the target URL and print results
    Crawl {
        /// Maximum number of concurrent fetch tasks
        #[arg(short = 'c', long = "concurrency")]
        concurrency: Option<usize>,

        /// Maximum total number of pages to fetch
        #[arg(short = 'm', long = "max-pages")]
        max_pages: Option<usize>,

        /// Path-prefix budget in the form "/prefix/,N" (repeatable)
        #[arg(short = 'p', long = "path-budget")]
        path_budget: Vec<String>,
    },
}

/// Parses a `--path-budget` value of the form `"/prefix/,N"`.
///
/// Splits on the last comma so that prefixes containing commas are handled correctly.
/// Returns `(prefix, count)` on success or a descriptive error message on failure.
pub fn parse_path_budget(value: &str) -> Result<(String, usize), String> {
    let last_comma = value.rfind(',').ok_or_else(|| {
        format!("invalid path-budget \"{value}\": expected format \"/prefix/,N\"")
    })?;

    let prefix = &value[..last_comma];
    let count_str = &value[last_comma + 1..];

    if prefix.is_empty() {
        return Err(format!("invalid path-budget \"{value}\": prefix must not be empty"));
    }
    if !prefix.starts_with('/') {
        return Err(format!(
            "invalid path-budget \"{value}\": prefix must start with '/'"
        ));
    }

    let count: usize = count_str.parse().map_err(|_| {
        format!("invalid path-budget \"{value}\": count must be a non-negative integer")
    })?;

    Ok((prefix.to_owned(), count))
}

/// Validates that the URL string is a valid http or https URL.
/// Returns the parsed `Url` on success or an error message on failure.
pub fn validate_url(raw: &str) -> Result<Url, String> {
    let parsed = Url::parse(raw).map_err(|e| format!("invalid URL: {e}"))?;
    match parsed.scheme() {
        "http" | "https" => Ok(parsed),
        other => Err(format!("URL scheme must be http or https, got: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::error::ErrorKind;

    fn try_parse(args: &[&str]) -> Result<Args, clap::Error> {
        Args::try_parse_from(args)
    }

    #[test]
    fn missing_url_exits_with_error() {
        let result = try_parse(&["crawl-boi", "crawl"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn invalid_url_rejected() {
        let result = validate_url("not-a-url");
        assert!(result.is_err());
    }

    #[test]
    fn ftp_url_rejected() {
        let result = validate_url("ftp://example.com");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("http or https"));
    }

    #[test]
    fn valid_http_url_accepted() {
        let result = validate_url("http://example.com");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_str(), "http://example.com/");
    }

    #[test]
    fn valid_https_url_accepted() {
        let result = validate_url("https://example.com/page");
        assert!(result.is_ok());
    }

    #[test]
    fn concurrency_flag_parsed_correctly() {
        let result = try_parse(&["crawl-boi", "--url", "https://example.com", "crawl", "-c", "16"]);
        assert!(result.is_ok());
        let args = result.unwrap();
        match args.command {
            Commands::Crawl { concurrency, .. } => assert_eq!(concurrency, Some(16)),
        }
    }

    #[test]
    fn concurrency_defaults_to_none() {
        let result = try_parse(&["crawl-boi", "--url", "https://example.com", "crawl"]);
        assert!(result.is_ok());
        let args = result.unwrap();
        match args.command {
            Commands::Crawl { concurrency, .. } => assert_eq!(concurrency, None),
        }
    }

    #[test]
    fn parse_path_budget_valid() {
        let result = parse_path_budget("/blog/,10");
        assert_eq!(result, Ok(("/blog/".to_owned(), 10)));
    }

    #[test]
    fn parse_path_budget_rejects_missing_comma() {
        let result = parse_path_budget("/blog/10");
        assert!(result.is_err());
    }

    #[test]
    fn parse_path_budget_rejects_empty_prefix() {
        let result = parse_path_budget(",5");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn parse_path_budget_rejects_prefix_not_starting_with_slash() {
        let result = parse_path_budget("blog/,5");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("start with '/'"));
    }

    #[test]
    fn parse_path_budget_rejects_non_numeric_count() {
        let result = parse_path_budget("/blog/,abc");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("integer"));
    }

    #[test]
    fn parse_path_budget_handles_prefix_containing_commas() {
        let result = parse_path_budget("/a,b,c/,7");
        assert_eq!(result, Ok(("/a,b,c/".to_owned(), 7)));
    }

    #[test]
    fn max_pages_flag_parsed() {
        let result = try_parse(&[
            "crawl-boi", "--url", "https://example.com", "crawl", "--max-pages", "50",
        ]);
        assert!(result.is_ok());
        let args = result.unwrap();
        match args.command {
            Commands::Crawl { max_pages, .. } => assert_eq!(max_pages, Some(50)),
        }
    }

    #[test]
    fn path_budget_flag_parsed() {
        let result = try_parse(&[
            "crawl-boi", "--url", "https://example.com", "crawl",
            "--path-budget", "/blog/,10",
            "--path-budget", "/docs/,5",
        ]);
        assert!(result.is_ok());
        let args = result.unwrap();
        match args.command {
            Commands::Crawl { path_budget, .. } => {
                assert_eq!(path_budget, vec!["/blog/,10".to_owned(), "/docs/,5".to_owned()]);
            }
        }
    }
}
