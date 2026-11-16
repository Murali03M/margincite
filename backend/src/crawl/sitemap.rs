use quick_xml::events::Event;
use quick_xml::Reader;
use url::Url;

use super::client::CrawlClient;
use super::robots::Robots;
use super::urls;
use crate::sites::scope::SiteScope;

const URL_CAP: usize = 50_000;
/// Sitemap documents fetched in one crawl, the index and its children together.
const DOCUMENT_CAP: usize = 50;

pub enum SitemapDoc {
    Urls(Vec<Url>),
    Index(Vec<Url>),
}

pub struct SitemapOutcome {
    pub found: bool,
    pub urls: Vec<Url>,
}

pub fn parse(xml: &str) -> SitemapDoc {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut in_loc = false;
    let mut loc = String::new();
    let mut locs = Vec::new();
    let mut index = false;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) => {
                let name = event.name().as_ref().to_string();
                if name == "sitemapindex" {
                    index = true;
                }
                in_loc = name == "loc";
                if in_loc {
                    loc.clear();
                }
            }
            Ok(Event::Text(event)) if in_loc => {
                loc.push_str(event.as_ref());
            }
            Ok(Event::End(event)) if event.name().as_ref() == "loc" => {
                if let Ok(url) = Url::parse(loc.trim()) {
                    locs.push(url);
                }
                in_loc = false;
            }
            Ok(Event::Eof) => break,
            Err(_) => return SitemapDoc::Urls(Vec::new()),
            _ => {}
        }
        buf.clear();
    }
    if index {
        SitemapDoc::Index(locs)
    } else {
        SitemapDoc::Urls(locs)
    }
}

/// How many more sitemap documents this crawl may fetch.
struct Budget {
    left: usize,
}

impl Budget {
    fn take(&mut self) -> bool {
        if self.left == 0 {
            return false;
        }
        self.left -= 1;
        true
    }
}

pub async fn load_sitemap_urls(
    client: &CrawlClient,
    scope: &SiteScope,
    robots: &Robots,
) -> SitemapOutcome {
    let mut seeds = robots.sitemaps();
    let mut fallback = scope.root.clone();
    fallback.set_path("/sitemap.xml");
    fallback.set_query(None);
    fallback.set_fragment(None);
    if !seeds.iter().any(|url| url == &fallback) {
        seeds.push(fallback);
    }
    let mut budget = Budget { left: DOCUMENT_CAP };
    let mut urls = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut found = false;
    for seed in seeds {
        if urls.len() >= URL_CAP {
            break;
        }
        if let Some(doc) = read_doc(client, scope, &seed, &mut budget).await {
            found = true;
            collect(client, scope, doc, (&mut urls, &mut seen), &mut budget).await;
        }
    }
    urls.truncate(URL_CAP);
    SitemapOutcome { found, urls }
}

async fn collect(
    client: &CrawlClient,
    scope: &SiteScope,
    doc: SitemapDoc,
    (urls, seen): (&mut Vec<Url>, &mut std::collections::HashSet<Url>),
    budget: &mut Budget,
) {
    match doc {
        SitemapDoc::Urls(found) => push_in_scope(scope, found, urls, seen),
        SitemapDoc::Index(children) => {
            for child in children {
                if urls.len() >= URL_CAP || budget.left == 0 {
                    break;
                }
                if let Some(SitemapDoc::Urls(found)) = read_doc(client, scope, &child, budget).await
                {
                    push_in_scope(scope, found, urls, seen);
                }
            }
        }
    }
}

fn push_in_scope(
    scope: &SiteScope,
    found: Vec<Url>,
    urls: &mut Vec<Url>,
    seen: &mut std::collections::HashSet<Url>,
) {
    for url in found {
        if urls.len() >= URL_CAP {
            break;
        }
        let url = urls::normalize(&url);
        if scope.contains(&url) && seen.insert(url.clone()) {
            urls.push(url);
        }
    }
}

/// Reads one sitemap document, if it is on the site's own host, is not
/// compressed and the crawl still has documents left to read.
async fn read_doc(
    client: &CrawlClient,
    scope: &SiteScope,
    url: &Url,
    budget: &mut Budget,
) -> Option<SitemapDoc> {
    if !scope.same_host(url) {
        tracing::info!(url = %url, "skipping a sitemap on another host");
        return None;
    }
    if compressed(url) {
        tracing::info!(url = %url, "skipping compressed sitemap");
        return None;
    }
    if !budget.take() {
        tracing::info!(url = %url, "skipping a sitemap past the document limit");
        return None;
    }
    let fetched = client.fetch_text(url).await;
    let status = fetched.status?;
    if !(200..300).contains(&status) {
        return None;
    }
    if compressed(url)
        || fetched
            .content_type
            .as_deref()
            .unwrap_or("")
            .contains("gzip")
    {
        tracing::info!(url = %url, "skipping compressed sitemap");
        return None;
    }
    fetched.body.map(|body| parse(&body))
}

fn compressed(url: &Url) -> bool {
    url.path().ends_with(".gz")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crawl::client::{CrawlClient, Network};
    use crate::crawl::robots::Robots;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn sitemap_reads_a_urlset() {
        let doc = parse(
            r#"<urlset><url><loc>https://example.com/a</loc></url><url><loc>https://example.com/b</loc></url></urlset>"#,
        );
        match doc {
            SitemapDoc::Urls(urls) => assert_eq!(urls.len(), 2),
            SitemapDoc::Index(_) => panic!("expected urls"),
        }
    }

    #[test]
    fn sitemap_reads_an_index() {
        let doc = parse(
            r#"<sitemapindex><sitemap><loc>https://example.com/a.xml</loc></sitemap></sitemapindex>"#,
        );
        match doc {
            SitemapDoc::Index(urls) => assert_eq!(urls.len(), 1),
            SitemapDoc::Urls(_) => panic!("expected an index"),
        }
    }

    #[test]
    fn sitemap_survives_broken_xml() {
        match parse("<<<>>>") {
            SitemapDoc::Urls(urls) => assert!(urls.is_empty()),
            SitemapDoc::Index(_) => panic!("broken xml is an empty url set"),
        }
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
    async fn sitemap_urls_outside_the_site_are_dropped() {
        let server = MockServer::start().await;
        let xml = format!(
            "<urlset><url><loc>{}/inside</loc></url><url><loc>https://other.example/no</loc></url></urlset>",
            server.uri()
        );
        Mock::given(method("GET"))
            .and(path("/sitemap.xml"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(xml, "application/xml"))
            .mount(&server)
            .await;
        let client = CrawlClient::new("https://example.com/bot", Network::AllowPrivate);
        let outcome = load_sitemap_urls(&client, &scope_for(&server), &Robots::allow_all()).await;
        assert!(outcome.found);
        assert_eq!(outcome.urls.len(), 1);
        assert!(outcome.urls[0].path().ends_with("/inside"));
    }

    #[tokio::test]
    async fn an_index_is_followed_one_level() {
        let server = MockServer::start().await;
        let index = format!(
            "<sitemapindex><sitemap><loc>{}/child.xml</loc></sitemap></sitemapindex>",
            server.uri()
        );
        let child = format!(
            "<urlset><url><loc>{}/only</loc></url></urlset>",
            server.uri()
        );
        Mock::given(method("GET"))
            .and(path("/sitemap.xml"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(index, "application/xml"))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/child.xml"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(child, "application/xml"))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/grandchild.xml"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;
        let client = CrawlClient::new("https://example.com/bot", Network::AllowPrivate);
        let outcome = load_sitemap_urls(&client, &scope_for(&server), &Robots::allow_all()).await;
        assert_eq!(outcome.urls.len(), 1);
        server.verify().await;
    }

    async fn requested_xml(server: &MockServer) -> Vec<String> {
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .map(|request| request.url.path().to_string())
            .filter(|path| path.ends_with(".xml"))
            .collect()
    }

    #[tokio::test]
    async fn at_most_50_sitemap_documents_are_read_in_one_crawl() {
        let server = MockServer::start().await;
        let children: String = (0..80)
            .map(|n| format!("<sitemap><loc>{}/c{n}.xml</loc></sitemap>", server.uri()))
            .collect();
        Mock::given(method("GET"))
            .and(path("/sitemap.xml"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                format!("<sitemapindex>{children}</sitemapindex>"),
                "application/xml",
            ))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(wiremock::matchers::path_regex(r"^/c\d+\.xml$"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                format!("<urlset><url><loc>{}/p</loc></url></urlset>", server.uri()),
                "application/xml",
            ))
            .mount(&server)
            .await;
        let client = CrawlClient::new("https://example.com/bot", Network::AllowPrivate);
        load_sitemap_urls(&client, &scope_for(&server), &Robots::allow_all()).await;
        assert_eq!(requested_xml(&server).await.len(), 50);
    }

    #[tokio::test]
    async fn the_document_limit_is_shared_by_every_sitemap_seed() {
        let server = MockServer::start().await;
        let robots = Robots::parse(&format!(
            "User-agent: *\nSitemap: {0}/s1.xml\nSitemap: {0}/s2.xml\n",
            server.uri()
        ));
        for seed in ["s1", "s2", "sitemap"] {
            let children: String = (0..40)
                .map(|n| {
                    format!(
                        "<sitemap><loc>{}/{seed}-{n}.xml</loc></sitemap>",
                        server.uri()
                    )
                })
                .collect();
            Mock::given(method("GET"))
                .and(path(format!("/{seed}.xml")))
                .respond_with(ResponseTemplate::new(200).set_body_raw(
                    format!("<sitemapindex>{children}</sitemapindex>"),
                    "application/xml",
                ))
                .mount(&server)
                .await;
        }
        Mock::given(method("GET"))
            .and(wiremock::matchers::path_regex(r"-\d+\.xml$"))
            .respond_with(ResponseTemplate::new(200).set_body_raw("<urlset/>", "application/xml"))
            .mount(&server)
            .await;
        let client = CrawlClient::new("https://example.com/bot", Network::AllowPrivate);
        load_sitemap_urls(&client, &scope_for(&server), &robots).await;
        assert!(requested_xml(&server).await.len() <= 50);
    }

    #[tokio::test]
    async fn sitemaps_on_another_host_are_never_fetched() {
        let server = MockServer::start().await;
        let port = server.address().port();
        let robots = Robots::parse(&format!(
            "User-agent: *\nSitemap: http://localhost:{port}/seed-other.xml\n"
        ));
        let index = format!(
            "<sitemapindex><sitemap><loc>http://localhost:{port}/child-other.xml</loc></sitemap></sitemapindex>"
        );
        Mock::given(method("GET"))
            .and(path("/sitemap.xml"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(index, "application/xml"))
            .mount(&server)
            .await;
        for route in ["/seed-other.xml", "/child-other.xml"] {
            Mock::given(method("GET"))
                .and(path(route))
                .respond_with(ResponseTemplate::new(200).set_body_raw("<urlset/>", "text/xml"))
                .expect(0)
                .mount(&server)
                .await;
        }
        let client = CrawlClient::new("https://example.com/bot", Network::AllowPrivate);
        load_sitemap_urls(&client, &scope_for(&server), &robots).await;
        server.verify().await;
    }

    #[test]
    fn a_leading_www_does_not_make_a_sitemap_foreign() {
        let scope = SiteScope {
            root: Url::parse("https://example.com/").unwrap(),
            host: "example.com".into(),
            path_prefix: "/".into(),
        };
        assert!(scope.same_host(&Url::parse("https://www.example.com/s.xml").unwrap()));
        assert!(scope.same_host(&Url::parse("https://example.com/s.xml").unwrap()));
        assert!(!scope.same_host(&Url::parse("https://evil.example/s.xml").unwrap()));
    }
}
