use async_trait::async_trait;
use url::Url;

#[async_trait]
pub trait Renderer: Send + Sync {
    async fn render(&self, url: &Url) -> Result<String, String>;
}

pub struct NoRenderer;

pub struct ChromeRenderer {
    browser: chromiumoxide::Browser,
    network: super::client::Network,
    gate: tokio::sync::Mutex<()>,
}

impl ChromeRenderer {
    pub async fn launch(
        chrome_path: &std::path::Path,
        network: super::client::Network,
    ) -> anyhow::Result<Self> {
        let profile =
            std::env::temp_dir().join(format!("margincite-chrome-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&profile)?;
        let config = chromiumoxide::BrowserConfig::builder()
            .chrome_executable(chrome_path)
            .user_data_dir(&profile)
            .build()
            .map_err(|err| anyhow::anyhow!(err))?;
        let (browser, mut handler) = chromiumoxide::Browser::launch(config).await?;
        tokio::spawn(async move {
            use futures::StreamExt;
            while let Some(event) = handler.next().await {
                if let Err(err) = event {
                    tracing::debug!(error = %err, "chrome handler");
                    break;
                }
            }
        });
        Ok(Self {
            browser,
            network,
            gate: tokio::sync::Mutex::new(()),
        })
    }
}

#[async_trait]
impl Renderer for NoRenderer {
    async fn render(&self, _url: &Url) -> Result<String, String> {
        Err("no browser is configured".into())
    }
}

#[async_trait]
impl Renderer for ChromeRenderer {
    async fn render(&self, url: &Url) -> Result<String, String> {
        permit(self.network, url)?;
        let _guard = self.gate.lock().await;
        let page = self
            .browser
            .new_page("about:blank")
            .await
            .map_err(|err| err.to_string())?;
        let loaded =
            tokio::time::timeout(std::time::Duration::from_secs(15), page.goto(url.as_str())).await;
        let html = match loaded {
            Ok(Ok(_)) => read_checked(&page, self.network, url).await,
            Ok(Err(err)) => Err(err.to_string()),
            Err(_) => Err("the page did not settle".into()),
        };
        page.close().await.ok();
        html
    }
}

/// Reads the page, then refuses it when the browser ended up somewhere other
/// than the requested site. The address is read after the content so a late
/// script redirect is seen too.
async fn read_checked(
    page: &chromiumoxide::Page,
    network: super::client::Network,
    requested: &Url,
) -> Result<String, String> {
    let html = page.content().await.map_err(|err| err.to_string())?;
    let landed = page.url().await.map_err(|err| err.to_string())?;
    check_final(network, requested, landed.as_deref())?;
    Ok(html)
}

/// The browser's final address must pass the same guard as any fetch and stay
/// on the host that was asked for.
fn check_final(
    network: super::client::Network,
    requested: &Url,
    landed: Option<&str>,
) -> Result<(), String> {
    let landed = landed
        .and_then(|raw| Url::parse(raw).ok())
        .ok_or_else(|| "the browser did not report where it ended up".to_string())?;
    permit(network, &landed)?;
    if landed.host_str() != requested.host_str() {
        return Err("the page moved to another site".into());
    }
    Ok(())
}

fn permit(network: super::client::Network, url: &Url) -> Result<(), String> {
    let loopback = matches!(
        url.host(),
        Some(url::Host::Ipv4(ip)) if ip.is_loopback()
    ) || matches!(url.host(), Some(url::Host::Ipv6(ip)) if ip.is_loopback());
    if network == super::client::Network::AllowPrivate
        && loopback
        && matches!(url.scheme(), "http" | "https")
    {
        return Ok(());
    }
    super::guard::check_url(url).map_err(|_| "blocked".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crawl::client::Network;

    fn url(raw: &str) -> Url {
        Url::parse(raw).unwrap()
    }

    #[test]
    fn a_final_address_on_the_same_host_passes() {
        let page = url("https://example.com/a");
        assert!(check_final(Network::Public, &page, Some("https://example.com/b")).is_ok());
    }

    #[test]
    fn a_final_internal_address_is_refused() {
        let page = url("https://example.com/a");
        for landed in [
            "http://169.254.169.254/latest/meta-data/",
            "http://127.0.0.1:8787/api/v1/me",
            "http://10.0.0.5/",
            "file:///etc/passwd",
            "chrome-error://chromewebdata/",
        ] {
            assert!(
                check_final(Network::Public, &page, Some(landed)).is_err(),
                "{landed}"
            );
        }
    }

    #[test]
    fn a_final_address_on_another_host_is_refused() {
        let page = url("https://example.com/a");
        let err = check_final(Network::Public, &page, Some("https://other.example/")).unwrap_err();
        assert_eq!(err, "the page moved to another site");
        assert!(check_final(Network::Public, &page, Some("https://www.example.com/")).is_err());
    }

    #[test]
    fn a_missing_final_address_is_refused() {
        let page = url("https://example.com/a");
        assert!(check_final(Network::Public, &page, None).is_err());
    }

    #[test]
    fn allow_private_still_guards_non_loopback_internal_addresses() {
        let page = url("http://127.0.0.1:3000/");
        assert!(check_final(
            Network::AllowPrivate,
            &page,
            Some("http://127.0.0.1:3000/x")
        )
        .is_ok());
        assert!(check_final(
            Network::AllowPrivate,
            &page,
            Some("http://169.254.169.254/")
        )
        .is_err());
    }
}
