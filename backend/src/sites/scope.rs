use std::net::IpAddr;

use url::{Host, Url};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteScope {
    pub root: Url,
    pub host: String,
    pub path_prefix: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ScopeError {
    #[error("Enter a website address.")]
    Empty,
    #[error("That address is too long.")]
    TooLong,
    #[error("That address could not be read.")]
    BadUrl,
    #[error("Only http and https addresses are allowed.")]
    Scheme,
    #[error("Addresses with a username or password are not allowed.")]
    Credentials,
    #[error("That address is not a public website.")]
    InternalHost,
}

impl SiteScope {
    pub fn parse(input: &str) -> Result<SiteScope, ScopeError> {
        let input = input.trim();
        if input.is_empty() {
            return Err(ScopeError::Empty);
        }
        if input.len() > 2048 {
            return Err(ScopeError::TooLong);
        }
        let raw = if input.contains("://") {
            input.to_string()
        } else {
            format!("https://{input}")
        };
        let url = Url::parse(&raw).map_err(|_| ScopeError::BadUrl)?;
        check_scheme_and_credentials(&url)?;
        let host = public_host(&url)?;
        let path_prefix = path_prefix(url.path());
        // Crawl the host the person typed. `www` and the bare name are one site,
        // but only one of them may exist in DNS.
        let fetch_host = url
            .host_str()
            .ok_or(ScopeError::BadUrl)?
            .trim_end_matches('.')
            .to_ascii_lowercase();
        let root = root_url(url.scheme(), &fetch_host, &path_prefix)?;
        Ok(SiteScope {
            root,
            host,
            path_prefix,
        })
    }

    /// Whether `url` is on this site's host, ignoring a leading `www.`.
    pub fn same_host(&self, url: &Url) -> bool {
        url.host_str()
            .is_some_and(|host| bare_host(host) == self.host)
    }

    pub fn contains(&self, url: &Url) -> bool {
        if !self.same_host(url) {
            return false;
        }
        let path = url.path().trim_end_matches('/');
        let path = if path.is_empty() { "/" } else { path };
        if self.path_prefix == "/" {
            return true;
        }
        path == self.path_prefix || path.starts_with(&format!("{}/", self.path_prefix))
    }
}

fn check_scheme_and_credentials(url: &Url) -> Result<(), ScopeError> {
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err(ScopeError::Scheme);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(ScopeError::Credentials);
    }
    Ok(())
}

fn public_host(url: &Url) -> Result<String, ScopeError> {
    let host = url.host_str().ok_or(ScopeError::BadUrl)?;
    if matches!(url.host(), Some(Host::Ipv4(_) | Host::Ipv6(_))) {
        return Err(ScopeError::InternalHost);
    }
    let host = bare_host(host);
    if host == "localhost" || host.ends_with(".localhost") || host.parse::<IpAddr>().is_ok() {
        return Err(ScopeError::InternalHost);
    }
    Ok(host)
}

fn bare_host(host: &str) -> String {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    host.strip_prefix("www.").unwrap_or(&host).to_string()
}

fn path_prefix(path: &str) -> String {
    let path = path.trim_end_matches('/');
    if path.is_empty() {
        "/".to_string()
    } else {
        path.to_string()
    }
}

fn root_url(scheme: &str, host: &str, path_prefix: &str) -> Result<Url, ScopeError> {
    let path = if path_prefix == "/" {
        "/".to_string()
    } else {
        format!("{path_prefix}/")
    };
    Url::parse(&format!("{scheme}://{host}{path}")).map_err(|_| ScopeError::BadUrl)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_https_when_no_scheme() {
        let scope = SiteScope::parse("example.com").unwrap();
        assert_eq!(scope.root.as_str(), "https://example.com/");
    }

    #[test]
    fn www_and_bare_host_are_the_same_site() {
        let with_www = SiteScope::parse("www.example.com").unwrap();
        assert_eq!(with_www.host, "example.com");
        assert_eq!(with_www.root.as_str(), "https://www.example.com/");
        assert_eq!(SiteScope::parse("EXAMPLE.com").unwrap().host, "example.com");
    }

    #[test]
    fn keeps_a_sub_path_as_the_prefix() {
        let scope = SiteScope::parse("example.com/docs").unwrap();
        assert_eq!(scope.path_prefix, "/docs");
    }

    #[test]
    fn contains_pages_under_the_prefix_only() {
        let scope = SiteScope::parse("example.com/docs").unwrap();
        assert!(scope.contains(&Url::parse("https://example.com/docs/a").unwrap()));
        assert!(scope.contains(&Url::parse("https://www.example.com/docs").unwrap()));
        assert!(!scope.contains(&Url::parse("https://example.com/blog").unwrap()));
        assert!(!scope.contains(&Url::parse("https://example.com/docsx").unwrap()));
    }

    #[test]
    fn other_subdomains_are_outside() {
        let scope = SiteScope::parse("example.com").unwrap();
        assert!(!scope.contains(&Url::parse("https://blog.example.com/").unwrap()));
    }

    #[test]
    fn rejects_non_http_schemes() {
        assert!(SiteScope::parse("ftp://x").is_err());
        assert!(SiteScope::parse("javascript:alert(1)").is_err());
        assert!(SiteScope::parse("file:///etc/passwd").is_err());
    }

    #[test]
    fn rejects_addresses_with_credentials() {
        assert!(SiteScope::parse("https://user:pw@example.com").is_err());
    }

    #[test]
    fn rejects_ip_literals_and_localhost() {
        assert!(SiteScope::parse("http://127.0.0.1").is_err());
        assert!(SiteScope::parse("http://[::1]").is_err());
        assert!(SiteScope::parse("localhost").is_err());
    }

    #[test]
    fn rejects_empty_and_overlong_input() {
        assert!(SiteScope::parse("").is_err());
        assert!(SiteScope::parse("   ").is_err());
        assert!(SiteScope::parse(&"a".repeat(2049)).is_err());
    }
}
