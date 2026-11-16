//! Where a setting's value comes from: the workspace's saved value, then the
//! environment, then the built-in default. A sealed value that cannot be
//! opened (the key changed) counts as not saved, so one bad row never stops a
//! feature that has another source.

use std::collections::HashMap;

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::keys;
use super::repo::{self, Row};
use crate::ai::engines::EngineId;
use crate::config::{
    provider_spec, Config, EngineSetting, EngineSettings, GoogleCredentials, PROVIDERS,
};
use crate::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Workspace,
    Environment,
    None,
}

/// A provider's key and model for one workspace. `Debug` hides the key.
#[derive(Clone)]
pub struct ResolvedProvider {
    pub api_key: Option<String>,
    pub model: String,
    pub source: Source,
}

impl std::fmt::Debug for ResolvedProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedProvider")
            .field("api_key", &self.api_key.as_ref().map(|_| ".."))
            .field("model", &self.model)
            .field("source", &self.source)
            .finish()
    }
}

/// What a workspace has saved, read once so several settings resolve from one
/// query.
pub struct Saved<'a> {
    rows: HashMap<String, Row>,
    config: &'a Config,
}

pub async fn load<'a>(
    pool: &PgPool,
    config: &'a Config,
    workspace_id: Uuid,
) -> Result<Saved<'a>, AppError> {
    Ok(Saved {
        rows: repo::all(pool, workspace_id).await?,
        config,
    })
}

impl Saved<'_> {
    fn plain(&self, key: &str) -> Option<String> {
        self.rows
            .get(key)
            .and_then(|row| row.plain.clone())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    }

    fn sealed(&self, key: &str) -> Option<String> {
        let sealed = self.rows.get(key)?.sealed.as_deref()?;
        let secrets = self.config.secret_key.as_ref()?;
        match secrets.open(sealed) {
            Ok(value) if !value.is_empty() => Some(value),
            Ok(_) => None,
            Err(_) => {
                tracing::warn!(key, "a saved secret could not be read and is ignored");
                None
            }
        }
    }

    pub fn provider(&self, id: EngineId) -> ResolvedProvider {
        let name = id.as_str();
        let spec = provider_spec(name).expect("every engine has a spec");
        let env = self
            .config
            .engines
            .get(name)
            .cloned()
            .unwrap_or(EngineSetting {
                api_key: None,
                model: spec.default_model.to_string(),
            });
        let own_key = self.sealed(&keys::provider_key(name));
        let (api_key, source) = match (own_key, env.api_key) {
            (Some(key), _) => (Some(key), Source::Workspace),
            (None, Some(key)) => (Some(key), Source::Environment),
            (None, None) => (None, Source::None),
        };
        let model = self.plain(&keys::provider_model(name)).unwrap_or(env.model);
        ResolvedProvider {
            api_key,
            model,
            source,
        }
    }

    pub fn engine_settings(&self) -> EngineSettings {
        let mut settings = self.config.engines.clone();
        for spec in &PROVIDERS {
            if let Some(id) = EngineId::from_name(spec.name) {
                let resolved = self.provider(id);
                settings = settings.with(
                    spec.name,
                    EngineSetting {
                        api_key: resolved.api_key,
                        model: resolved.model,
                    },
                );
            }
        }
        settings
    }

    /// Both values or neither, from one source: a workspace id is never
    /// paired with an environment secret.
    pub fn google(&self) -> (Option<GoogleCredentials>, Source) {
        let own = (
            self.plain(keys::GOOGLE_CLIENT_ID),
            self.sealed(keys::GOOGLE_CLIENT_SECRET),
        );
        if let (Some(client_id), Some(client_secret)) = own {
            return (
                Some(GoogleCredentials {
                    client_id,
                    client_secret,
                }),
                Source::Workspace,
            );
        }
        match &self.config.google {
            Some(env) => (Some(env.clone()), Source::Environment),
            None => (None, Source::None),
        }
    }

    pub fn dataforseo(&self) -> (Option<(String, String)>, Source) {
        let own = (
            self.plain(keys::DATAFORSEO_LOGIN),
            self.sealed(keys::DATAFORSEO_PASSWORD),
        );
        if let (Some(login), Some(password)) = own {
            return (Some((login, password)), Source::Workspace);
        }
        match &self.config.dataforseo {
            Some(env) => (
                Some((env.login.clone(), env.password.clone())),
                Source::Environment,
            ),
            None => (None, Source::None),
        }
    }

    pub fn pagespeed_key(&self) -> (Option<String>, Source) {
        if let Some(key) = self.sealed(keys::PAGESPEED_KEY) {
            return (Some(key), Source::Workspace);
        }
        match &self.config.pagespeed_key {
            Some(key) => (Some(key.expose().to_string()), Source::Environment),
            None => (None, Source::None),
        }
    }

    pub fn crawler_contact(&self) -> (Option<String>, Source) {
        if let Some(contact) = self.plain(keys::CRAWLER_CONTACT) {
            return (Some(contact), Source::Workspace);
        }
        match &self.config.crawler_contact {
            Some(contact) => (Some(contact.clone()), Source::Environment),
            None => (None, Source::None),
        }
    }
}

pub async fn provider(
    pool: &PgPool,
    config: &Config,
    workspace_id: Uuid,
    id: EngineId,
) -> Result<ResolvedProvider, AppError> {
    Ok(load(pool, config, workspace_id).await?.provider(id))
}

/// Every engine's key and model for the workspace, as `EngineSettings`.
pub async fn engine_settings(
    pool: &PgPool,
    config: &Config,
    workspace_id: Uuid,
) -> Result<EngineSettings, AppError> {
    Ok(load(pool, config, workspace_id).await?.engine_settings())
}

pub async fn google(
    pool: &PgPool,
    config: &Config,
    workspace_id: Uuid,
) -> Result<Option<GoogleCredentials>, AppError> {
    Ok(load(pool, config, workspace_id).await?.google().0)
}

pub async fn dataforseo(
    pool: &PgPool,
    config: &Config,
    workspace_id: Uuid,
) -> Result<Option<(String, String)>, AppError> {
    Ok(load(pool, config, workspace_id).await?.dataforseo().0)
}

pub async fn pagespeed_key(
    pool: &PgPool,
    config: &Config,
    workspace_id: Uuid,
) -> Result<Option<String>, AppError> {
    Ok(load(pool, config, workspace_id).await?.pagespeed_key().0)
}

/// The address sent with every crawler request. `None` when none is set
/// anywhere, which stops crawling.
pub async fn crawler_contact(
    pool: &PgPool,
    config: &Config,
    workspace_id: Uuid,
) -> Result<Option<String>, AppError> {
    Ok(load(pool, config, workspace_id).await?.crawler_contact().0)
}
