use std::time::Duration;

use axum::routing::get;
use axum::Router;
use margincite::crawl::client::Network;
use margincite::crawl::render::{ChromeRenderer, Renderer};
use url::Url;

fn chrome_path() -> Option<String> {
    std::env::var("CHROME_PATH")
        .ok()
        .filter(|path| !path.is_empty())
}

async fn serve(body: &'static str) -> String {
    let app = Router::new().route(
        "/",
        get(move || async move { ([("content-type", "text/html")], body) }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}/")
}

#[tokio::test]
async fn returns_html_that_javascript_wrote() {
    let Some(path) = chrome_path() else {
        println!("skipped returns_html_that_javascript_wrote: CHROME_PATH is not set");
        return;
    };
    let url =
        serve(r#"<body><script>document.body.innerText = 'Hello from script'</script></body>"#)
            .await;
    let renderer = ChromeRenderer::launch(path.as_ref(), Network::AllowPrivate)
        .await
        .unwrap();
    let html = renderer.render(&Url::parse(&url).unwrap()).await.unwrap();
    assert!(html.contains("Hello from script"), "{html}");
}

#[tokio::test]
async fn gives_up_on_a_page_that_never_settles() {
    let Some(path) = chrome_path() else {
        println!("skipped gives_up_on_a_page_that_never_settles: CHROME_PATH is not set");
        return;
    };
    let app = Router::new().route(
        "/",
        get(|| async {
            tokio::time::sleep(Duration::from_secs(60)).await;
            "late"
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let renderer = ChromeRenderer::launch(path.as_ref(), Network::AllowPrivate)
        .await
        .unwrap();
    let started = std::time::Instant::now();
    let result = renderer
        .render(&Url::parse(&format!("http://{addr}/")).unwrap())
        .await;
    assert!(result.is_err());
    assert!(started.elapsed() < Duration::from_secs(20));
}

#[tokio::test]
async fn refuses_a_page_request_to_an_internal_address() {
    let Some(path) = chrome_path() else {
        println!("skipped refuses_a_page_request_to_an_internal_address: CHROME_PATH is not set");
        return;
    };
    let renderer = ChromeRenderer::launch(std::path::Path::new(&path), Network::Public)
        .await
        .unwrap();
    let result = renderer
        .render(&Url::parse("http://169.254.169.254/latest/meta-data/").unwrap())
        .await;
    assert_eq!(result.unwrap_err(), "blocked");
}

#[tokio::test]
async fn refuses_a_page_that_sends_the_browser_to_another_host() {
    let Some(path) = chrome_path() else {
        println!(
            "skipped refuses_a_page_that_sends_the_browser_to_another_host: CHROME_PATH is not set"
        );
        return;
    };
    let internal = serve("<body>internal document</body>").await;
    let port = Url::parse(&internal).unwrap().port().unwrap();
    let shell: &'static str = Box::leak(
        format!(r#"<body><script>location.replace("http://localhost:{port}/")</script></body>"#)
            .into_boxed_str(),
    );
    let url = serve(shell).await;
    let renderer = ChromeRenderer::launch(path.as_ref(), Network::AllowPrivate)
        .await
        .unwrap();
    let result = renderer.render(&Url::parse(&url).unwrap()).await;
    assert_eq!(result.unwrap_err(), "the page moved to another site");
}
