use std::time::{Duration, Instant};

use futures::StreamExt;
use reqwest::header::{HeaderMap, LOCATION};
use url::Url;

use super::guard;
use super::resolver::AddressRefused;
pub use super::resolver::GuardedResolver;

const HTML: &[&str] = &["text/html", "application/xhtml+xml"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Network {
    Public,
    AllowPrivate,
}

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub timeout: Duration,
    pub max_bytes: usize,
    pub max_redirects: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(20),
            max_bytes: 5 * 1024 * 1024,
            max_redirects: 10,
        }
    }
}

#[derive(Debug)]
pub struct Fetched {
    pub final_url: Url,
    pub status: Option<u16>,
    pub headers: HeaderMap,
    pub body: Option<String>,
    pub content_type: Option<String>,
    pub redirects: Vec<Url>,
    pub elapsed_ms: u32,
    pub bytes: u32,
    pub error: Option<FetchError>,
}

#[derive(Debug)]
pub enum FetchError {
    Blocked,
    Timeout,
    TooLarge,
    TooManyRedirects,
    Network(String),
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchError::Blocked => write!(f, "blocked"),
            FetchError::Timeout => write!(f, "timed out"),
            FetchError::TooLarge => write!(f, "too large"),
            FetchError::TooManyRedirects => write!(f, "too many redirects"),
            FetchError::Network(message) => write!(f, "{message}"),
        }
    }
}

#[derive(Clone)]
pub struct CrawlClient {
    http: reqwest::Client,
    network: Network,
    limits: Limits,
    user_agent: String,
    pins: Vec<(String, std::net::SocketAddr)>,
}

impl CrawlClient {
    pub fn new(contact: &str, network: Network) -> Self {
        Self::with_limits(contact, network, Limits::default())
    }

    pub fn with_limits(contact: &str, network: Network, limits: Limits) -> Self {
        let contact: String = contact.chars().filter(char::is_ascii_graphic).collect();
        let user_agent = format!("MarginciteBot/0.1 (+{contact})");
        let http = builder(network, limits, &user_agent)
            .build()
            .expect("crawler client");
        Self {
            http,
            network,
            limits,
            user_agent,
            pins: Vec::new(),
        }
    }

    /// The same client announcing a different contact address. Pinned hosts
    /// stay pinned.
    pub fn with_contact(&self, contact: &str) -> Self {
        let mut client = Self::with_limits(contact, self.network, self.limits);
        for (host, addr) in &self.pins {
            client = client.pin_host(host, *addr);
        }
        client
    }

    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    /// Sends one host to a fixed address. Tests use it so a local site can have a public-looking name.
    pub fn pin_host(mut self, host: &str, addr: std::net::SocketAddr) -> Self {
        self.pins.push((host.to_string(), addr));
        let mut http = builder(self.network, self.limits, &self.user_agent);
        for (host, addr) in &self.pins {
            http = http.resolve(host, *addr);
        }
        self.http = http.build().expect("crawler client");
        self
    }

    pub async fn fetch(&self, url: &Url) -> Fetched {
        self.fetch_mode(url, false).await
    }

    /// Reads the body of any response, not only HTML. Used for robots.txt and sitemaps.
    pub async fn fetch_text(&self, url: &Url) -> Fetched {
        self.fetch_mode(url, true).await
    }

    async fn fetch_mode(&self, url: &Url, read_any: bool) -> Fetched {
        let started = Instant::now();
        let mut current = url.clone();
        let mut redirects = Vec::new();
        loop {
            if let Err(error) = self.permit(&current) {
                return finish(current, redirects, started, error);
            }
            let response = match self.http.get(current.clone()).send().await {
                Ok(response) => response,
                Err(err) => return finish(current, redirects, started, classify(err)),
            };
            if is_redirect(response.status().as_u16()) {
                if redirects.len() >= self.limits.max_redirects {
                    return finish(current, redirects, started, FetchError::TooManyRedirects);
                }
                let Some(next) = next_url(&current, &response) else {
                    return finish(
                        current,
                        redirects,
                        started,
                        FetchError::Network("bad redirect".into()),
                    );
                };
                redirects.push(current);
                current = next;
                continue;
            }
            return self
                .read_body(current, redirects, response, started, read_any)
                .await;
        }
    }

    fn permit(&self, url: &Url) -> Result<(), FetchError> {
        if url.scheme() != "http" && url.scheme() != "https" {
            return Err(FetchError::Blocked);
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(FetchError::Blocked);
        }
        if let Some(ip) = literal_ip(url) {
            if self.network == Network::AllowPrivate && ip.is_loopback() {
                return Ok(());
            }
            return guard::check_ip(ip).map_err(|_| FetchError::Blocked);
        }
        Ok(())
    }

    async fn read_body(
        &self,
        final_url: Url,
        redirects: Vec<Url>,
        response: reqwest::Response,
        started: Instant,
        read_any: bool,
    ) -> Fetched {
        let status = Some(response.status().as_u16());
        let headers = response.headers().clone();
        let content_type = headers
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        if !read_any && !is_html(content_type.as_deref()) {
            return Fetched {
                final_url,
                status,
                headers,
                body: None,
                content_type,
                redirects,
                elapsed_ms: elapsed(started),
                bytes: 0,
                error: None,
            };
        }
        match read_limited(response, self.limits.max_bytes).await {
            Ok(bytes) => Fetched {
                final_url,
                status,
                headers,
                body: Some(String::from_utf8_lossy(&bytes).into_owned()),
                content_type,
                redirects,
                elapsed_ms: elapsed(started),
                bytes: bytes.len() as u32,
                error: None,
            },
            Err(error) => Fetched {
                final_url,
                status,
                headers,
                body: None,
                content_type,
                redirects,
                elapsed_ms: elapsed(started),
                bytes: 0,
                error: Some(error),
            },
        }
    }
}

/// Every request resolves names through the guarded resolver. Proxies are off
/// so no other party resolves a name for the crawler.
fn builder(network: Network, limits: Limits, user_agent: &str) -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(limits.timeout)
        .user_agent(user_agent)
        .no_proxy()
        .dns_resolver(GuardedResolver::new(network))
}

fn literal_ip(url: &Url) -> Option<std::net::IpAddr> {
    match url.host()? {
        url::Host::Ipv4(ip) => Some(ip.into()),
        url::Host::Ipv6(ip) => Some(ip.into()),
        url::Host::Domain(_) => None,
    }
}

fn is_html(content_type: Option<&str>) -> bool {
    let bare = content_type
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim();
    HTML.iter().any(|kind| bare.eq_ignore_ascii_case(kind))
}

fn is_redirect(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

fn next_url(current: &Url, response: &reqwest::Response) -> Option<Url> {
    let location = response.headers().get(LOCATION)?.to_str().ok()?;
    current.join(location).ok()
}

async fn read_limited(
    response: reqwest::Response,
    max_bytes: usize,
) -> Result<Vec<u8>, FetchError> {
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(classify)?;
        if body.len() + chunk.len() > max_bytes {
            return Err(FetchError::TooLarge);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn classify(err: reqwest::Error) -> FetchError {
    if refused_by_resolver(&err) {
        FetchError::Blocked
    } else if err.is_timeout() {
        FetchError::Timeout
    } else {
        FetchError::Network("the page could not be fetched".into())
    }
}

fn refused_by_resolver(err: &reqwest::Error) -> bool {
    let mut source = std::error::Error::source(err);
    while let Some(inner) = source {
        if inner.is::<AddressRefused>() {
            return true;
        }
        source = inner.source();
    }
    false
}

fn finish(final_url: Url, redirects: Vec<Url>, started: Instant, error: FetchError) -> Fetched {
    Fetched {
        final_url,
        status: None,
        headers: HeaderMap::new(),
        body: None,
        content_type: None,
        redirects,
        elapsed_ms: elapsed(started),
        bytes: 0,
        error: Some(error),
    }
}

fn elapsed(started: Instant) -> u32 {
    u32::try_from(started.elapsed().as_millis())
        .unwrap_or(u32::MAX)
        .max(1)
}
