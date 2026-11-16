use url::Url;

pub fn normalize(url: &Url) -> Url {
    let mut out = url.clone();
    out.set_fragment(None);
    if let Some(host) = out.host_str() {
        let lower = host.to_ascii_lowercase();
        let _ = out.set_host(Some(&lower));
    }
    if default_port(&out) {
        let _ = out.set_port(None);
    }
    if out.path().is_empty() {
        out.set_path("/");
    }
    let kept: Vec<(String, String)> = out
        .query_pairs()
        .filter(|(key, _)| !is_tracking(key))
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    if kept.is_empty() {
        out.set_query(None);
    } else {
        let mut serial = url::form_urlencoded::Serializer::new(String::new());
        for (key, value) in &kept {
            serial.append_pair(key, value);
        }
        out.set_query(Some(&serial.finish()));
    }
    out
}

fn default_port(url: &Url) -> bool {
    matches!(
        (url.scheme(), url.port()),
        ("http", Some(80)) | ("https", Some(443))
    )
}

fn is_tracking(key: &str) -> bool {
    key.starts_with("utm_") || key == "gclid" || key == "fbclid"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_drops_fragment_and_tracking() {
        let url = Url::parse("https://Example.com/a?utm_source=x&id=2#top").unwrap();
        assert_eq!(normalize(&url).as_str(), "https://example.com/a?id=2");
    }

    #[test]
    fn normalize_keeps_meaningful_query() {
        let url = Url::parse("https://example.com:443/docs?q=a").unwrap();
        assert_eq!(normalize(&url).as_str(), "https://example.com/docs?q=a");
    }
}
