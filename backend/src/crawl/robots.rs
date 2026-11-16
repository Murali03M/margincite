use std::time::Duration;

use texting_robots::Robot;
use url::Url;

use super::client::CrawlClient;
use crate::sites::scope::SiteScope;

pub struct Robots {
    robot: Option<Robot>,
    raw: Option<String>,
    deny_all: bool,
}

pub struct RobotsOutcome {
    pub rules: Robots,
    pub raw: Option<String>,
    pub unreachable: bool,
}

impl Robots {
    pub fn parse(text: &str) -> Self {
        match Robot::new("MarginciteBot", text.as_bytes()) {
            Ok(robot) => Self {
                robot: Some(robot),
                raw: Some(text.to_string()),
                deny_all: false,
            },
            Err(_) => Self::allow_all(),
        }
    }

    pub fn allow_all() -> Self {
        Self {
            robot: None,
            raw: None,
            deny_all: false,
        }
    }

    pub fn deny_all() -> Self {
        Self {
            robot: None,
            raw: None,
            deny_all: true,
        }
    }

    pub fn allows(&self, url: &Url) -> bool {
        if self.deny_all {
            return false;
        }
        self.robot
            .as_ref()
            .is_none_or(|robot| robot.allowed(url.as_str()))
    }

    /// Whether robots.txt lets the named crawler fetch `url`, judged by the
    /// group written for that crawler, or the `*` group when it has none.
    pub fn allows_agent(&self, agent: &str, url: &Url) -> bool {
        if self.deny_all {
            return false;
        }
        match &self.raw {
            None => true,
            Some(raw) => {
                Robot::new(agent, raw.as_bytes()).map_or(true, |robot| robot.allowed(url.as_str()))
            }
        }
    }

    pub fn crawl_delay(&self) -> Option<Duration> {
        self.robot
            .as_ref()
            .and_then(|robot| robot.delay)
            .map(Duration::from_secs_f32)
    }

    pub fn sitemaps(&self) -> Vec<Url> {
        self.robot
            .as_ref()
            .map(|robot| {
                robot
                    .sitemaps
                    .iter()
                    .filter_map(|raw| Url::parse(raw).ok())
                    .collect()
            })
            .unwrap_or_default()
    }
}

pub async fn load_robots(client: &CrawlClient, scope: &SiteScope) -> RobotsOutcome {
    let mut url = scope.root.clone();
    url.set_path("/robots.txt");
    url.set_query(None);
    url.set_fragment(None);
    let fetched = client.fetch_text(&url).await;
    match fetched.status {
        Some(code) if (200..300).contains(&code) => {
            let raw = fetched.body.unwrap_or_default();
            RobotsOutcome {
                rules: Robots::parse(&raw),
                raw: Some(raw),
                unreachable: false,
            }
        }
        Some(code) if (400..500).contains(&code) => RobotsOutcome {
            rules: Robots::allow_all(),
            raw: None,
            unreachable: false,
        },
        _ => RobotsOutcome {
            rules: Robots::deny_all(),
            raw: None,
            unreachable: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::crawl::client::{CrawlClient, Network};
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn page(path: &str) -> Url {
        Url::parse(&format!("https://example.com{path}")).unwrap()
    }

    #[test]
    fn robots_disallow_applies_to_our_agent() {
        let rules = Robots::parse("User-agent: MarginciteBot\nDisallow: /private\n");
        assert!(!rules.allows(&page("/private")));
        assert!(rules.allows(&page("/public")));
    }

    #[test]
    fn robots_specific_group_beats_wildcard() {
        let rules =
            Robots::parse("User-agent: *\nDisallow: /\n\nUser-agent: MarginciteBot\nAllow: /\n");
        assert!(rules.allows(&page("/anything")));
    }

    #[test]
    fn robots_reads_crawl_delay_and_sitemaps() {
        let rules = Robots::parse(
            "User-agent: MarginciteBot\nCrawl-delay: 2\nSitemap: https://example.com/sitemap.xml\n",
        );
        assert_eq!(rules.crawl_delay(), Some(Duration::from_secs(2)));
        assert_eq!(
            rules.sitemaps(),
            vec![Url::parse("https://example.com/sitemap.xml").unwrap()]
        );
    }

    #[test]
    fn allows_agent_uses_that_agents_group() {
        let rules = Robots::parse("User-agent: *\nAllow: /\n\nUser-agent: GPTBot\nDisallow: /\n");
        assert!(!rules.allows_agent("GPTBot", &page("/")));
        assert!(rules.allows_agent("ClaudeBot", &page("/")));
        assert!(Robots::allow_all().allows_agent("GPTBot", &page("/")));
        assert!(!Robots::deny_all().allows_agent("GPTBot", &page("/")));
    }

    fn scope_for(server: &MockServer) -> SiteScope {
        let root = Url::parse(&format!("{}/", server.uri())).unwrap();
        SiteScope {
            host: root.host_str().unwrap().to_string(),
            path_prefix: "/".into(),
            root,
        }
    }

    #[tokio::test]
    async fn missing_robots_allows_everything() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let client = CrawlClient::new("https://example.com/bot", Network::AllowPrivate);
        let outcome = load_robots(&client, &scope_for(&server)).await;
        assert!(!outcome.unreachable);
        assert!(outcome.rules.allows(&page("/anywhere")));
    }

    #[tokio::test]
    async fn a_500_on_robots_allows_nothing() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let client = CrawlClient::new("https://example.com/bot", Network::AllowPrivate);
        let outcome = load_robots(&client, &scope_for(&server)).await;
        assert!(outcome.unreachable);
        assert!(!outcome.rules.allows(&page("/")));
    }
}
