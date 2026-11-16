use std::net::SocketAddr;
use std::path::PathBuf;

use url::Url;

use crate::secrets::Secrets;

#[derive(Debug)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: SocketAddr,
    pub dev_tokens: Vec<(String, String)>,
    /// `CRAWLER_CONTACT`: the fallback for a workspace that has not saved its
    /// own contact in Settings. None is fine at start-up; crawling is refused
    /// until one is set somewhere.
    pub crawler_contact: Option<String>,
    pub chrome_path: Option<PathBuf>,
    /// `RENDER_SITES_I_TRUST=yes`. Rendering lets the crawled pages make this
    /// machine fetch internal addresses, so it is a separate, explicit opt-in.
    pub render_sites_i_trust: bool,
    /// `MARGINCITE_SECRET_KEY`: encrypts stored Google and Bing secrets.
    pub secret_key: Option<Secrets>,
    /// `PUBLIC_BASE_URL`: where the web app lives, for OAuth redirects.
    pub public_base_url: Url,
    pub google: Option<GoogleCredentials>,
    pub engines: EngineSettings,
    /// `DATAFORSEO_LOGIN` and `DATAFORSEO_PASSWORD`, both or neither.
    pub dataforseo: Option<DataForSeoCredentials>,
    /// `PAGESPEED_API_KEY`.
    pub pagespeed_key: Option<Redacted>,
    /// `MARGINCITE_DATA_DIR`: where the generated secret key lives. Relative
    /// to the folder the server starts in; `.data` by default.
    pub data_dir: PathBuf,
}

/// Text that must never reach a log line: its `Debug` output hides it.
#[derive(Clone)]
pub struct Redacted(String);

impl Redacted {
    pub fn new(value: impl Into<String>) -> Self {
        Redacted(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Redacted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Redacted(..)")
    }
}

/// A DataForSEO login and password.
#[derive(Clone)]
pub struct DataForSeoCredentials {
    pub login: String,
    pub password: String,
}

impl std::fmt::Debug for DataForSeoCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataForSeoCredentials")
            .field("login", &"..")
            .field("password", &"..")
            .finish()
    }
}

/// The OAuth client Margincite uses to ask for Search Console access.
#[derive(Clone)]
pub struct GoogleCredentials {
    pub client_id: String,
    pub client_secret: String,
}

impl std::fmt::Debug for GoogleCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleCredentials")
            .field("client_id", &self.client_id)
            .field("client_secret", &"..")
            .finish()
    }
}

/// One AI engine: an optional server-side key and the model to ask.
#[derive(Clone)]
pub struct EngineSetting {
    pub api_key: Option<String>,
    pub model: String,
}

impl EngineSetting {
    pub fn configured(&self) -> bool {
        self.api_key.is_some()
    }
}

impl std::fmt::Debug for EngineSetting {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EngineSetting")
            .field("api_key", &self.api_key.as_ref().map(|_| ".."))
            .field("model", &self.model)
            .finish()
    }
}

/// One AI provider as the environment knows it: the variables that can set
/// its key and model, and the model used when nobody says. A new provider is
/// one more row in `PROVIDERS`.
pub struct ProviderSpec {
    pub name: &'static str,
    pub env_key: &'static str,
    pub env_model: &'static str,
    pub default_model: &'static str,
}

/// Every AI provider, in the fixed order the API lists them.
pub const PROVIDERS: [ProviderSpec; 5] = [
    ProviderSpec {
        name: "claude",
        env_key: "ANTHROPIC_API_KEY",
        env_model: "CLAUDE_MODEL",
        default_model: "claude-opus-5-5",
    },
    ProviderSpec {
        name: "chatgpt",
        env_key: "OPENAI_API_KEY",
        env_model: "OPENAI_MODEL",
        default_model: "gpt-5.6",
    },
    ProviderSpec {
        name: "grok",
        env_key: "XAI_API_KEY",
        env_model: "GROK_MODEL",
        default_model: "grok-4.7",
    },
    ProviderSpec {
        name: "gemini",
        env_key: "GEMINI_API_KEY",
        env_model: "GEMINI_MODEL",
        default_model: "gemini-3.5-flash",
    },
    ProviderSpec {
        name: "perplexity",
        env_key: "PERPLEXITY_API_KEY",
        env_model: "PERPLEXITY_MODEL",
        default_model: "sonar",
    },
];

/// The spec of a provider by its API name.
pub fn provider_spec(name: &str) -> Option<&'static ProviderSpec> {
    PROVIDERS.iter().find(|spec| spec.name == name)
}

/// The AI engines as the environment sets them up: the fallback for a
/// workspace that has saved nothing of its own. An engine without a key is off.
#[derive(Debug, Clone)]
pub struct EngineSettings {
    pub claude: EngineSetting,
    pub chatgpt: EngineSetting,
    pub grok: EngineSetting,
    pub gemini: EngineSetting,
    pub perplexity: EngineSetting,
}

impl EngineSettings {
    /// Every engine by its API name, in the order of `PROVIDERS`.
    pub fn all(&self) -> [(&'static str, &EngineSetting); 5] {
        [
            ("claude", &self.claude),
            ("chatgpt", &self.chatgpt),
            ("grok", &self.grok),
            ("gemini", &self.gemini),
            ("perplexity", &self.perplexity),
        ]
    }

    pub fn get(&self, name: &str) -> Option<&EngineSetting> {
        self.all()
            .into_iter()
            .find(|(engine, _)| *engine == name)
            .map(|(_, setting)| setting)
    }

    /// The same settings with one engine replaced.
    pub fn with(mut self, name: &str, setting: EngineSetting) -> EngineSettings {
        match name {
            "claude" => self.claude = setting,
            "chatgpt" => self.chatgpt = setting,
            "grok" => self.grok = setting,
            "gemini" => self.gemini = setting,
            "perplexity" => self.perplexity = setting,
            _ => {}
        }
        self
    }

    fn from_env(vars: &impl Fn(&str) -> Option<String>) -> EngineSettings {
        let engine = |name: &str| {
            let spec = provider_spec(name).expect("every engine has a spec");
            EngineSetting {
                api_key: optional(vars, spec.env_key),
                model: optional(vars, spec.env_model)
                    .unwrap_or_else(|| spec.default_model.to_string()),
            }
        };
        EngineSettings {
            claude: engine("claude"),
            chatgpt: engine("chatgpt"),
            grok: engine("grok"),
            gemini: engine("gemini"),
            perplexity: engine("perplexity"),
        }
    }
}

const DEFAULT_PUBLIC_BASE_URL: &str = "http://localhost:5174";
pub const DEFAULT_DATA_DIR: &str = ".data";

#[derive(Debug)]
pub struct ConfigError(String);

impl ConfigError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        ConfigError(message.into())
    }
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ConfigError {}

impl Config {
    pub fn from_env(vars: impl Fn(&str) -> Option<String>) -> Result<Config, ConfigError> {
        let database_url = required(&vars, "DATABASE_URL")?;
        let crawler_contact = optional(&vars, "CRAWLER_CONTACT");
        let bind_addr = parse_bind(vars("BIND_ADDR"))?;
        let dev_tokens = parse_tokens(vars("DEV_API_TOKENS").as_deref().unwrap_or(""))?;
        let chrome_path = vars("CHROME_PATH")
            .filter(|p| !p.is_empty())
            .map(PathBuf::from);
        let render_sites_i_trust = vars("RENDER_SITES_I_TRUST").is_some_and(|v| v.trim() == "yes");
        let secret_key = optional(&vars, "MARGINCITE_SECRET_KEY")
            .map(|key| Secrets::from_base64(&key))
            .transpose()?;
        Ok(Config {
            database_url,
            bind_addr,
            dev_tokens,
            crawler_contact,
            chrome_path,
            render_sites_i_trust,
            secret_key,
            public_base_url: parse_public_base_url(&vars)?,
            google: parse_google(&vars)?,
            engines: EngineSettings::from_env(&vars),
            dataforseo: parse_dataforseo(&vars)?,
            pagespeed_key: optional(&vars, "PAGESPEED_API_KEY").map(Redacted::new),
            data_dir: optional(&vars, "MARGINCITE_DATA_DIR")
                .map_or_else(|| PathBuf::from(DEFAULT_DATA_DIR), PathBuf::from),
        })
    }

    /// Makes sure there is a sealing key: `MARGINCITE_SECRET_KEY` when it is
    /// set (nothing is read or written on disk then), otherwise the key in the
    /// data folder, made the first time. The server calls this at start-up;
    /// `from_env` itself never touches the disk.
    pub fn with_secret_key_file(mut self) -> Result<Config, ConfigError> {
        if self.secret_key.is_none() {
            self.secret_key = Some(Secrets::load_or_create(&self.data_dir)?);
        }
        Ok(self)
    }

    /// The browser to render with: only when a browser is set and the operator
    /// has said the crawled sites are trusted.
    pub fn rendering_browser(&self) -> Option<&std::path::Path> {
        self.chrome_path
            .as_deref()
            .filter(|_| self.render_sites_i_trust)
    }
}

fn required(vars: &impl Fn(&str) -> Option<String>, name: &str) -> Result<String, ConfigError> {
    vars(name)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| ConfigError(format!("{name} is required")))
}

/// A setting that is unset or empty is `None`.
fn optional(vars: &impl Fn(&str) -> Option<String>, name: &str) -> Option<String> {
    vars(name)
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn parse_public_base_url(vars: &impl Fn(&str) -> Option<String>) -> Result<Url, ConfigError> {
    let raw = optional(vars, "PUBLIC_BASE_URL").unwrap_or_else(|| DEFAULT_PUBLIC_BASE_URL.into());
    let url = Url::parse(&raw)
        .ok()
        .filter(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
        .ok_or_else(|| {
            ConfigError::new(format!("PUBLIC_BASE_URL is not an http(s) address: {raw}"))
        })?;
    Ok(url)
}

/// Both Google values or neither; one alone is a mistake worth stopping for.
fn parse_google(
    vars: &impl Fn(&str) -> Option<String>,
) -> Result<Option<GoogleCredentials>, ConfigError> {
    match (
        optional(vars, "GOOGLE_CLIENT_ID"),
        optional(vars, "GOOGLE_CLIENT_SECRET"),
    ) {
        (Some(client_id), Some(client_secret)) => Ok(Some(GoogleCredentials {
            client_id,
            client_secret,
        })),
        (None, None) => Ok(None),
        (Some(_), None) => Err(ConfigError::new(
            "GOOGLE_CLIENT_SECRET is required when GOOGLE_CLIENT_ID is set",
        )),
        (None, Some(_)) => Err(ConfigError::new(
            "GOOGLE_CLIENT_ID is required when GOOGLE_CLIENT_SECRET is set",
        )),
    }
}

/// Both DataForSEO values or neither.
fn parse_dataforseo(
    vars: &impl Fn(&str) -> Option<String>,
) -> Result<Option<DataForSeoCredentials>, ConfigError> {
    match (
        optional(vars, "DATAFORSEO_LOGIN"),
        optional(vars, "DATAFORSEO_PASSWORD"),
    ) {
        (Some(login), Some(password)) => Ok(Some(DataForSeoCredentials { login, password })),
        (None, None) => Ok(None),
        (Some(_), None) => Err(ConfigError::new(
            "DATAFORSEO_PASSWORD is required when DATAFORSEO_LOGIN is set",
        )),
        (None, Some(_)) => Err(ConfigError::new(
            "DATAFORSEO_LOGIN is required when DATAFORSEO_PASSWORD is set",
        )),
    }
}

fn parse_bind(raw: Option<String>) -> Result<SocketAddr, ConfigError> {
    let raw = raw.unwrap_or_else(|| "127.0.0.1:8787".to_string());
    raw.parse()
        .map_err(|_| ConfigError(format!("BIND_ADDR is not a socket address: {raw}")))
}

fn parse_tokens(raw: &str) -> Result<Vec<(String, String)>, ConfigError> {
    let mut tokens = Vec::new();
    for pair in raw.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (token, email) = pair
            .split_once('=')
            .ok_or_else(|| ConfigError(format!("DEV_API_TOKENS pair is malformed: {pair}")))?;
        if token.is_empty() || email.is_empty() {
            return Err(ConfigError(format!(
                "DEV_API_TOKENS pair is malformed: {pair}"
            )));
        }
        tokens.push((token.to_string(), email.to_string()));
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(key: &str) -> Option<String> {
        match key {
            "DATABASE_URL" => Some("postgres://localhost/margincite_dev".into()),
            "BIND_ADDR" => Some("0.0.0.0:9000".into()),
            "DEV_API_TOKENS" => Some("tok=ada@example.com".into()),
            "CRAWLER_CONTACT" => Some("https://example.com/margincite-bot".into()),
            "CHROME_PATH" => Some("/Applications/Google Chrome.app".into()),
            "RENDER_SITES_I_TRUST" => Some("yes".into()),
            _ => None,
        }
    }

    #[test]
    fn reads_every_setting() {
        let config = Config::from_env(set).unwrap();
        assert_eq!(config.database_url, "postgres://localhost/margincite_dev");
        assert_eq!(config.bind_addr.to_string(), "0.0.0.0:9000");
        assert_eq!(
            config.dev_tokens,
            vec![("tok".into(), "ada@example.com".into())]
        );
        assert_eq!(
            config.crawler_contact.as_deref(),
            Some("https://example.com/margincite-bot")
        );
        assert_eq!(
            config.chrome_path.unwrap().to_string_lossy(),
            "/Applications/Google Chrome.app"
        );
        assert!(config.render_sites_i_trust);
    }

    fn with_render(trust: Option<&'static str>, chrome: Option<&'static str>) -> Config {
        Config::from_env(|key| match key {
            "DATABASE_URL" => Some("postgres://localhost/margincite_dev".into()),
            "CRAWLER_CONTACT" => Some("https://example.com/margincite-bot".into()),
            "CHROME_PATH" => chrome.map(str::to_string),
            "RENDER_SITES_I_TRUST" => trust.map(str::to_string),
            _ => None,
        })
        .unwrap()
    }

    #[test]
    fn rendering_needs_both_a_browser_and_the_trust_setting() {
        let on = with_render(Some("yes"), Some("/bin/chrome"));
        assert!(on.rendering_browser().is_some());
        let no_trust = with_render(None, Some("/bin/chrome"));
        assert!(no_trust.chrome_path.is_some());
        assert!(no_trust.rendering_browser().is_none());
        let no_browser = with_render(Some("yes"), None);
        assert!(no_browser.rendering_browser().is_none());
    }

    #[test]
    fn only_yes_turns_trust_on() {
        for value in ["no", "true", "1", "", "YES please"] {
            let config = with_render(Some(value), Some("/bin/chrome"));
            assert!(!config.render_sites_i_trust, "{value}");
        }
    }

    #[test]
    fn bind_addr_defaults_to_local_8787() {
        let config = Config::from_env(|key| match key {
            "DATABASE_URL" => Some("postgres://localhost/margincite_dev".into()),
            "CRAWLER_CONTACT" => Some("https://example.com/margincite-bot".into()),
            _ => None,
        })
        .unwrap();
        assert_eq!(config.bind_addr.to_string(), "127.0.0.1:8787");
    }

    #[test]
    fn missing_database_url_names_the_variable() {
        let err = Config::from_env(|_| None).unwrap_err();
        assert!(err.to_string().contains("DATABASE_URL"));
    }

    #[test]
    fn a_missing_crawler_contact_is_not_a_start_up_error() {
        let config = Config::from_env(|key| match key {
            "DATABASE_URL" => Some("postgres://localhost/margincite_dev".into()),
            _ => None,
        })
        .unwrap();
        assert!(config.crawler_contact.is_none());
        assert_eq!(config.data_dir, PathBuf::from(".data"));
    }

    #[test]
    fn the_key_file_is_only_used_when_the_environment_has_no_key() {
        let dir = std::env::temp_dir().join(format!("margincite-cfg-{}", uuid::Uuid::new_v4()));
        let dir_text = dir.to_string_lossy().to_string();
        let env_key = "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=".to_string();
        let with = |key: Option<String>| {
            let dir_text = dir_text.clone();
            Config::from_env(move |name| match name {
                "DATABASE_URL" => Some("postgres://localhost/margincite_dev".into()),
                "MARGINCITE_DATA_DIR" => Some(dir_text.clone()),
                "MARGINCITE_SECRET_KEY" => key.clone(),
                _ => None,
            })
            .unwrap()
        };
        let config = with(Some(env_key)).with_secret_key_file().unwrap();
        assert!(config.secret_key.is_some());
        assert!(!dir.exists(), "an environment key touches no disk");
        let config = with(None).with_secret_key_file().unwrap();
        assert!(config.secret_key.is_some());
        assert!(dir.join("secret.key").is_file());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dataforseo_and_pagespeed_come_from_the_environment() {
        let config = Config::from_env(base(&[
            ("DATAFORSEO_LOGIN", "me@example.com"),
            ("DATAFORSEO_PASSWORD", "pw"),
            ("PAGESPEED_API_KEY", "psi"),
            ("MARGINCITE_DATA_DIR", "/tmp/x"),
        ]))
        .unwrap();
        let seo = config.dataforseo.unwrap();
        assert_eq!(
            (seo.login.as_str(), seo.password.as_str()),
            ("me@example.com", "pw")
        );
        assert_eq!(config.pagespeed_key.unwrap().expose(), "psi");
        assert_eq!(config.data_dir, PathBuf::from("/tmp/x"));
        let err = Config::from_env(base(&[("DATAFORSEO_LOGIN", "a")])).unwrap_err();
        assert!(err.to_string().contains("DATAFORSEO_PASSWORD"));
        let err = Config::from_env(base(&[("DATAFORSEO_PASSWORD", "a")])).unwrap_err();
        assert!(err.to_string().contains("DATAFORSEO_LOGIN"));
    }

    #[test]
    fn malformed_dev_token_pair_is_rejected() {
        let err = Config::from_env(|key| match key {
            "DATABASE_URL" => Some("postgres://localhost/margincite_dev".into()),
            "CRAWLER_CONTACT" => Some("https://example.com/margincite-bot".into()),
            "DEV_API_TOKENS" => Some("abc".into()),
            _ => None,
        })
        .unwrap_err();
        assert!(err.to_string().contains("DEV_API_TOKENS"));
    }

    fn base(extra: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |key| match key {
            "DATABASE_URL" => Some("postgres://localhost/margincite_dev".into()),
            "CRAWLER_CONTACT" => Some("https://example.com/margincite-bot".into()),
            other => extra
                .iter()
                .find(|(name, _)| *name == other)
                .map(|(_, value)| value.to_string()),
        }
    }

    #[test]
    fn integration_settings_default_to_off_with_default_models() {
        let config = Config::from_env(base(&[])).unwrap();
        assert!(config.secret_key.is_none());
        assert!(config.google.is_none());
        assert_eq!(config.public_base_url.as_str(), "http://localhost:5174/");
        let models = config
            .engines
            .all()
            .map(|(name, e)| (name, e.model.clone(), e.configured()));
        assert_eq!(
            models,
            [
                ("claude", "claude-opus-5-5".to_string(), false),
                ("chatgpt", "gpt-5.6".to_string(), false),
                ("grok", "grok-4.7".to_string(), false),
                ("gemini", "gemini-3.5-flash".to_string(), false),
                ("perplexity", "sonar".to_string(), false),
            ]
        );
    }

    #[test]
    fn integration_settings_are_read() {
        let config = Config::from_env(base(&[
            (
                "MARGINCITE_SECRET_KEY",
                "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=",
            ),
            ("PUBLIC_BASE_URL", "https://app.example.com"),
            ("GOOGLE_CLIENT_ID", "id"),
            ("GOOGLE_CLIENT_SECRET", "shh"),
            ("ANTHROPIC_API_KEY", "a"),
            ("CLAUDE_MODEL", "claude-x"),
            ("OPENAI_API_KEY", "o"),
            ("OPENAI_MODEL", "gpt-x"),
            ("PERPLEXITY_API_KEY", "p"),
            ("PERPLEXITY_MODEL", "sonar-x"),
            ("GEMINI_API_KEY", "g"),
            ("GEMINI_MODEL", "gem-x"),
            ("XAI_API_KEY", "x"),
            ("GROK_MODEL", "grok-x"),
        ]))
        .unwrap();
        assert!(config.secret_key.is_some());
        assert_eq!(config.public_base_url.as_str(), "https://app.example.com/");
        let google = config.google.unwrap();
        assert_eq!(
            (google.client_id.as_str(), google.client_secret.as_str()),
            ("id", "shh")
        );
        let engines = config
            .engines
            .all()
            .map(|(name, e)| (name, e.api_key.clone().unwrap(), e.model.clone()));
        assert_eq!(
            engines[0],
            ("claude", "a".to_string(), "claude-x".to_string())
        );
        assert_eq!(
            engines[1],
            ("chatgpt", "o".to_string(), "gpt-x".to_string())
        );
        assert_eq!(engines[2], ("grok", "x".to_string(), "grok-x".to_string()));
        assert_eq!(engines[3], ("gemini", "g".to_string(), "gem-x".to_string()));
        assert_eq!(
            engines[4],
            ("perplexity", "p".to_string(), "sonar-x".to_string())
        );
    }

    #[test]
    fn empty_values_count_as_unset() {
        let config = Config::from_env(base(&[
            ("ANTHROPIC_API_KEY", "  "),
            ("GOOGLE_CLIENT_ID", ""),
            ("MARGINCITE_SECRET_KEY", ""),
        ]))
        .unwrap();
        assert!(!config.engines.claude.configured());
        assert!(config.google.is_none());
        assert!(config.secret_key.is_none());
    }

    #[test]
    fn half_set_google_credentials_are_refused_by_name() {
        let err = Config::from_env(base(&[("GOOGLE_CLIENT_ID", "id")])).unwrap_err();
        assert!(err.to_string().contains("GOOGLE_CLIENT_SECRET"));
        let err = Config::from_env(base(&[("GOOGLE_CLIENT_SECRET", "s")])).unwrap_err();
        assert!(err.to_string().contains("GOOGLE_CLIENT_ID"));
    }

    #[test]
    fn a_bad_secret_key_or_base_url_is_refused_by_name() {
        let err = Config::from_env(base(&[("MARGINCITE_SECRET_KEY", "c2hvcnQ=")])).unwrap_err();
        assert!(err.to_string().contains("MARGINCITE_SECRET_KEY"));
        for bad in ["nonsense", "ftp://example.com"] {
            let bad: &'static str = bad;
            let err = Config::from_env(move |key| match key {
                "PUBLIC_BASE_URL" => Some(bad.into()),
                other => base(&[])(other),
            })
            .unwrap_err();
            assert!(err.to_string().contains("PUBLIC_BASE_URL"), "{bad}");
        }
    }

    #[test]
    fn debug_output_never_shows_keys() {
        let config = Config::from_env(base(&[
            (
                "MARGINCITE_SECRET_KEY",
                "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=",
            ),
            ("GOOGLE_CLIENT_ID", "id"),
            ("GOOGLE_CLIENT_SECRET", "very-secret"),
            ("ANTHROPIC_API_KEY", "sk-hidden"),
            ("DATAFORSEO_LOGIN", "login-hidden"),
            ("DATAFORSEO_PASSWORD", "pw-hidden"),
            ("PAGESPEED_API_KEY", "psi-hidden"),
        ]))
        .unwrap();
        let shown = format!("{config:?}");
        for secret in [
            "very-secret",
            "sk-hidden",
            "BwcHBw",
            "login-hidden",
            "pw-hidden",
            "psi-hidden",
        ] {
            assert!(!shown.contains(secret), "{secret}");
        }
    }
}
