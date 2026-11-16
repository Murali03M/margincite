//! Does an answer mention or cite the site, and who else does it name.
//! Matching is on whole words and case-insensitive, so "Apple" is not found
//! in "pineapple" and "cbuddy" is not found in "cbuddyai".

use super::engines::{bare_host, Citation};

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Whether `needle` is in `haystack` (both already lower case) with no word
/// character right before or after it. `extra_before` and `extra_after` say
/// what else may not touch the match.
fn find_bounded(
    haystack: &str,
    needle: &str,
    blocks_before: impl Fn(char) -> bool,
    blocks_after: impl Fn(&str) -> bool,
) -> bool {
    if needle.is_empty() {
        return false;
    }
    let mut from = 0;
    while let Some(found) = haystack[from..].find(needle) {
        let start = from + found;
        let end = start + needle.len();
        let before_ok = haystack[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !blocks_before(c));
        let after_ok = !blocks_after(&haystack[end..]);
        if before_ok && after_ok {
            return true;
        }
        // Move on by one character, never into the middle of one.
        from = start + haystack[start..].chars().next().map_or(1, char::len_utf8);
    }
    false
}

/// A name (brand, alias, competitor name): whole words only.
fn word_in(text: &str, name: &str) -> bool {
    let name = name.trim().to_lowercase();
    find_bounded(&text.to_lowercase(), &name, is_word_char, |rest| {
        rest.chars().next().is_some_and(is_word_char)
    })
}

/// A host name such as `example.com`: nothing word-like or `-` before it, and
/// nothing after it that would make it a longer name (`example.com.au`,
/// `example.community`). A `.` that ends a sentence is fine.
fn host_in(text: &str, host: &str) -> bool {
    let host = bare_host(host.trim());
    find_bounded(
        &text.to_lowercase(),
        &host,
        |c| is_word_char(c) || c == '-',
        |rest| {
            let mut chars = rest.chars();
            match chars.next() {
                Some(c) if is_word_char(c) || c == '-' => true,
                Some('.') => chars.next().is_some_and(is_word_char),
                _ => false,
            }
        },
    )
}

/// Whether the answer text names the brand or an alias, or the site's host
/// (with or without `www.`) appears in it.
pub fn mentioned(text: &str, brand: &str, aliases: &[String], site_host: &str) -> bool {
    std::iter::once(brand)
        .chain(aliases.iter().map(String::as_str))
        .any(|name| word_in(text, name))
        || host_in(text, site_host)
}

pub fn on_domain(domain: &str, host: &str) -> bool {
    let (domain, host) = (bare_host(domain), bare_host(host));
    !domain.is_empty()
        && !host.is_empty()
        && (domain == host || domain.ends_with(&format!(".{host}")))
}

/// Whether a citation is the site or one of its subdomains, and the 1-based
/// position of the first such citation among all citations.
pub fn cited(citations: &[Citation], site_host: &str) -> (bool, Option<i32>) {
    citations
        .iter()
        .position(|citation| on_domain(&citation.domain, site_host))
        .map_or((false, None), |index| (true, Some(index as i32 + 1)))
}

/// The competitors (domains) that the answer names or cites, in the order
/// they were configured. A competitor is named when its domain appears in the
/// text or its first label appears as a whole word (`stripe` for `stripe.com`,
/// three letters or more).
pub fn competitors(text: &str, citations: &[Citation], competitors: &[String]) -> Vec<String> {
    competitors
        .iter()
        .filter(|domain| {
            let label = bare_host(domain)
                .split('.')
                .next()
                .unwrap_or("")
                .to_string();
            citations.iter().any(|c| on_domain(&c.domain, domain))
                || host_in(text, domain)
                || (label.chars().count() >= 3 && word_in(text, &label))
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cite(url: &str, domain: &str) -> Citation {
        Citation {
            url: url.into(),
            title: None,
            domain: domain.into(),
        }
    }

    fn mention(text: &str, brand: &str) -> bool {
        mentioned(text, brand, &[], "example.com")
    }

    #[test]
    fn a_brand_is_found_as_a_whole_word_in_any_case() {
        assert!(mention("Try Acme for this.", "acme"));
        assert!(mention("try ACME, then others", "Acme"));
        assert!(mention("(Acme)", "Acme"));
        assert!(mention("a good one: Acme.", "Acme"));
    }

    #[test]
    fn a_brand_inside_another_word_is_not_a_mention() {
        assert!(!mention("I ate a pineapple.", "Apple"));
        assert!(!mention("Try cbuddyai today.", "cbuddy"));
        assert!(!mention("The acmes of the world", "acme"));
        assert!(!mention("snake_case_acme", "acme"));
    }

    #[test]
    fn multi_word_brands_and_unicode_are_matched() {
        assert!(mention("We like Acme Widgets a lot", "Acme Widgets"));
        assert!(mention("Café Münster is great", "café münster"));
        assert!(!mention("Café Münsters", "Café Münster"));
    }

    #[test]
    fn aliases_count_and_blank_names_never_match() {
        let aliases = vec!["The Widget Co".to_string(), "  ".to_string(), String::new()];
        assert!(mentioned(
            "see the widget co.",
            "Acme",
            &aliases,
            "example.com"
        ));
        assert!(!mentioned("nothing here", "", &aliases, "example.com"));
        assert!(!mentioned("anything", "   ", &[], ""));
    }

    #[test]
    fn the_site_host_counts_with_or_without_www() {
        assert!(mentioned(
            "Visit example.com for more",
            "",
            &[],
            "example.com"
        ));
        assert!(mentioned(
            "Visit www.example.com/pricing",
            "",
            &[],
            "example.com"
        ));
        assert!(mentioned(
            "Visit https://EXAMPLE.com.",
            "",
            &[],
            "www.example.com"
        ));
        assert!(mentioned("see blog.example.com", "", &[], "example.com"));
    }

    #[test]
    fn a_host_inside_a_longer_name_is_not_a_mention() {
        assert!(!mentioned("see notexample.com", "", &[], "example.com"));
        assert!(!mentioned("see my-example.com", "", &[], "example.com"));
        assert!(!mentioned(
            "see example.com.evil.net",
            "",
            &[],
            "example.com"
        ));
        assert!(!mentioned("see example.community", "", &[], "example.com"));
    }

    #[test]
    fn a_citation_on_the_host_or_a_subdomain_counts_with_its_rank() {
        let list = vec![
            cite("https://other.com/a", "other.com"),
            cite("https://blog.example.com/x", "blog.example.com"),
            cite("https://example.com/y", "example.com"),
        ];
        assert_eq!(cited(&list, "example.com"), (true, Some(2)));
        assert_eq!(cited(&list, "www.example.com"), (true, Some(2)));
        assert_eq!(cited(&list[..1], "example.com"), (false, None));
        assert_eq!(cited(&[], "example.com"), (false, None));
    }

    #[test]
    fn a_lookalike_domain_is_not_a_citation() {
        let list = vec![
            cite("https://notexample.com/", "notexample.com"),
            cite("https://example.com.evil.net/", "example.com.evil.net"),
            cite("https://x.com/", ""),
        ];
        assert_eq!(cited(&list, "example.com"), (false, None));
        assert_eq!(cited(&list, ""), (false, None));
    }

    #[test]
    fn competitors_are_found_by_citation_domain_text_or_name() {
        let list = vec![cite("https://rival.com/p", "rival.com")];
        let configured = vec![
            "rival.com".to_string(),
            "stripe.com".to_string(),
            "other.io".to_string(),
            "ab.com".to_string(),
        ];
        let found = competitors("We suggest Stripe. Also ab is short.", &list, &configured);
        assert_eq!(found, vec!["rival.com", "stripe.com"]);
        assert!(competitors("nothing", &[], &configured).is_empty());
    }
}
