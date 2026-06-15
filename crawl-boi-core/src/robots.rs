#[derive(Debug, Clone, PartialEq)]
pub struct RobotsRules {
    pub(crate) disallowed_prefixes: Vec<String>,
}

impl RobotsRules {
    pub fn is_allowed(&self, path: &str) -> bool {
        !self
            .disallowed_prefixes
            .iter()
            .any(|prefix| path.starts_with(prefix.as_str()))
    }
}

pub struct RobotsParser;

impl RobotsParser {
    /// Parses body and returns the RobotsRules that apply to this crawler.
    ///
    /// If a crawl-boi-specific stanza exists it takes full precedence over *. An empty
    /// or absent Disallow: value is treated as "allow all" and is not recorded.
    pub fn parse(body: &str) -> RobotsRules {
        // Each stanza is a group of user-agents followed by directives.
        // We walk line-by-line, accumulating agents then directives.
        let mut crawl_boi_prefixes: Option<Vec<String>> = None;
        let mut wildcard_prefixes: Option<Vec<String>> = None;

        // Current stanza state.
        let mut current_agents: Vec<AgentKind> = Vec::new();
        let mut current_prefixes: Vec<String> = Vec::new();
        let mut in_directive_block = false;

        for raw_line in body.lines() {
            let line = strip_comment(raw_line).trim();

            if line.is_empty() {
                // Blank line ends a stanza, flush if we were in a directive block.
                if in_directive_block {
                    flush_stanza(
                        &current_agents,
                        &current_prefixes,
                        &mut crawl_boi_prefixes,
                        &mut wildcard_prefixes,
                    );
                    current_agents.clear();
                    current_prefixes.clear();
                    in_directive_block = false;
                } else {
                    // Blank line before any directive: discard accumulated agents.
                    current_agents.clear();
                }
                continue;
            }

            if let Some(agent_value) = parse_field(line, "User-agent") {
                if in_directive_block {
                    // New user-agent after directives, flush previous stanza first.
                    flush_stanza(
                        &current_agents,
                        &current_prefixes,
                        &mut crawl_boi_prefixes,
                        &mut wildcard_prefixes,
                    );
                    current_agents.clear();
                    current_prefixes.clear();
                    in_directive_block = false;
                }
                if let Some(kind) = classify_agent(agent_value) {
                    current_agents.push(kind);
                }
            } else if let Some(path_value) = parse_field(line, "Disallow") {
                in_directive_block = true;
                if !path_value.is_empty() {
                    current_prefixes.push(path_value.to_owned());
                }
            } else {
                // Other directives (Allow, Crawl-delay, Sitemap, …) are ignored.
                if !current_agents.is_empty() {
                    in_directive_block = true;
                }
            }
        }

        // Flush the final stanza if the file doesn't end with a blank line.
        if in_directive_block || !current_prefixes.is_empty() {
            flush_stanza(
                &current_agents,
                &current_prefixes,
                &mut crawl_boi_prefixes,
                &mut wildcard_prefixes,
            );
        }

        // crawl-boi-specific rules take full precedence over wildcard.
        let prefixes = crawl_boi_prefixes.or(wildcard_prefixes).unwrap_or_default();

        RobotsRules {
            disallowed_prefixes: prefixes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentKind {
    CrawlBoi,
    Wildcard,
}

fn classify_agent(agent: &str) -> Option<AgentKind> {
    let lower = agent.trim().to_lowercase();
    if lower == "crawl-boi" {
        Some(AgentKind::CrawlBoi)
    } else if lower == "*" {
        Some(AgentKind::Wildcard)
    } else {
        None
    }
}

fn parse_field<'a>(line: &'a str, field: &str) -> Option<&'a str> {
    let prefix = field.to_owned() + ":";
    line.get(..prefix.len())
        .filter(|p| p.eq_ignore_ascii_case(&prefix))
        .map(|_| line[prefix.len()..].trim())
}

fn strip_comment(line: &str) -> &str {
    line.find('#').map(|i| &line[..i]).unwrap_or(line)
}

fn flush_stanza(
    agents: &[AgentKind],
    prefixes: &[String],
    crawl_boi_prefixes: &mut Option<Vec<String>>,
    wildcard_prefixes: &mut Option<Vec<String>>,
) {
    let has_crawl_boi = agents.contains(&AgentKind::CrawlBoi);
    let has_wildcard = agents.contains(&AgentKind::Wildcard);

    if has_crawl_boi {
        let entry = crawl_boi_prefixes.get_or_insert_with(Vec::new);
        entry.extend_from_slice(prefixes);
    }
    if has_wildcard {
        let entry = wildcard_prefixes.get_or_insert_with(Vec::new);
        entry.extend_from_slice(prefixes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn crawl_boi_stanza_disallows_path() {
        let robots = "\
User-agent: crawl-boi\n\
Disallow: /private\n";
        let rules = RobotsParser::parse(robots);
        assert!(!rules.is_allowed("/private/page"));
        assert!(rules.is_allowed("/public/page"));
    }

    #[test]
    fn wildcard_stanza_disallows_path() {
        let robots = "\
User-agent: *\n\
Disallow: /secret\n";
        let rules = RobotsParser::parse(robots);
        assert!(!rules.is_allowed("/secret/stuff"));
        assert!(rules.is_allowed("/open"));
    }

    #[test]
    fn crawl_boi_stanza_overrides_wildcard() {
        let robots = "\
User-agent: *\n\
Disallow: /wildcard-only\n\
\n\
User-agent: crawl-boi\n\
Disallow: /crawl-boi-only\n";
        let rules = RobotsParser::parse(robots);
        // crawl-boi stanza in effect — its disallow applies
        assert!(!rules.is_allowed("/crawl-boi-only/page"));
        // wildcard disallow does NOT apply because crawl-boi stanza takes precedence
        assert!(rules.is_allowed("/wildcard-only/page"));
    }

    #[test]
    fn empty_disallow_means_allow_all() {
        let robots = "\
User-agent: crawl-boi\n\
Disallow:\n";
        let rules = RobotsParser::parse(robots);
        assert!(rules.is_allowed("/anything"));
        assert!(rules.is_allowed("/"));
    }

    #[test]
    fn missing_robots_means_allow_all() {
        let rules = RobotsParser::parse("");
        assert!(rules.is_allowed("/anything"));
        assert!(rules.is_allowed("/private"));
    }

    #[test]
    fn is_allowed_prefix_semantics() {
        let rules = RobotsRules {
            disallowed_prefixes: vec!["/admin".to_owned(), "/internal".to_owned()],
        };
        assert!(!rules.is_allowed("/admin"));
        assert!(!rules.is_allowed("/admin/users"));
        assert!(!rules.is_allowed("/admins")); // starts_with("/admin") is true
        assert!(!rules.is_allowed("/internal/api"));
        assert!(rules.is_allowed("/public"));
        assert!(rules.is_allowed("/"));
    }

    /// Generates a simple path starting with '/' followed by lowercase alphanum chars.
    /// Generates a non-empty string suitable for use as a disallowed prefix (starts with '/').
    fn arb_prefix() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9]{0,10}".prop_map(|s| format!("/{s}"))
    }

    proptest! {
        // robots.txt parsing extracts correct Disallow rules
        #[test]
        fn prop_parse_extracts_disallow_rules(
            paths in proptest::collection::vec(arb_prefix(), 1..=5),
            use_crawl_boi in proptest::bool::ANY,
        ) {
            let agent = if use_crawl_boi { "crawl-boi" } else { "*" };
            let disallow_lines: String = paths
                .iter()
                .map(|p| format!("Disallow: {p}\n"))
                .collect();
            let robots = format!("User-agent: {agent}\n{disallow_lines}");

            let rules = RobotsParser::parse(&robots);

            // Every generated path must be disallowed by the resulting rules.
            for path in &paths {
                prop_assert!(
                    !rules.is_allowed(path),
                    "expected {path} to be disallowed, rules: {:?}",
                    rules.disallowed_prefixes
                );
            }
        }

        // RobotsRules disallows matching paths
        #[test]
        fn prop_is_allowed_prefix_semantics(
            prefix in arb_prefix(),
            suffix in "[a-z0-9]{0,10}",
        ) {
            let rules = RobotsRules {
                disallowed_prefixes: vec![prefix.clone()],
            };

            // A path that starts with the prefix must be disallowed.
            let disallowed_path = format!("{prefix}{suffix}");
            prop_assert!(
                !rules.is_allowed(&disallowed_path),
                "{disallowed_path} should be disallowed by prefix {prefix}"
            );

            // A path that does NOT start with the prefix must be allowed.
            // Construct such a path by prepending something that cannot match.
            let unrelated_path = format!("/zzzunrelated{suffix}");
            if !unrelated_path.starts_with(&*prefix) {
                prop_assert!(
                    rules.is_allowed(&unrelated_path),
                    "{unrelated_path} should be allowed; prefix is {prefix}"
                );
            }
        }

        // robots.txt parsing is deterministic
        #[test]
        fn prop_parse_is_deterministic(
            paths in proptest::collection::vec(arb_prefix(), 0..=4),
            use_crawl_boi in proptest::bool::ANY,
        ) {
            let agent = if use_crawl_boi { "crawl-boi" } else { "*" };
            let disallow_lines: String = paths
                .iter()
                .map(|p| format!("Disallow: {p}\n"))
                .collect();
            let body = format!("User-agent: {agent}\n{disallow_lines}");

            let first = RobotsParser::parse(&body);
            let second = RobotsParser::parse(&body);

            prop_assert_eq!(first, second);
        }
    }
}
