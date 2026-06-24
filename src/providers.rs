use std::cmp::Ordering;
use std::env;
use std::thread;
use std::time::Duration;

use serde::Deserialize;

use crate::error::{OlaunchError, Result};

const LM_STUDIO_BASE_URL: &str = "http://localhost:1234/v1";
const OLLAMA_BASE_URL: &str = "http://localhost:11434/v1";
const OSAURUS_BASE_URL: &str = "http://localhost:1337/v1";
const OMLX_BASE_URL_DEFAULT: &str = "http://localhost:8000";
const OMLX_API_KEY_ENV: &str = "OMLX_API_KEY";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderKind {
    LmStudio,
    Ollama,
    Osaurus,
    Omlx,
    Generic,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderInfo {
    pub kind: ProviderKind,
    pub name: String,
    pub display_name: String,
    pub base_url: String,
    pub api_key_env: Option<String>,
    pub local_placeholder_token: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelInfo {
    pub id: String,
    pub provider: ProviderInfo,
    pub context_window: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub loaded: Option<bool>,
}

impl ProviderInfo {
    pub fn lm_studio() -> Self {
        Self {
            kind: ProviderKind::LmStudio,
            name: "lmstudio".into(),
            display_name: "LM Studio".into(),
            base_url: LM_STUDIO_BASE_URL.into(),
            api_key_env: None,
            local_placeholder_token: None,
        }
    }

    pub fn ollama() -> Self {
        Self {
            kind: ProviderKind::Ollama,
            name: "ollama".into(),
            display_name: "Ollama".into(),
            base_url: OLLAMA_BASE_URL.into(),
            api_key_env: None,
            local_placeholder_token: Some("ollama".into()),
        }
    }

    pub fn osaurus() -> Self {
        Self {
            kind: ProviderKind::Osaurus,
            name: "osaurus".into(),
            display_name: "Osaurus".into(),
            base_url: OSAURUS_BASE_URL.into(),
            api_key_env: None,
            local_placeholder_token: Some("osaurus".into()),
        }
    }

    pub fn omlx() -> Self {
        Self {
            kind: ProviderKind::Omlx,
            name: "omlx".into(),
            display_name: "oMLX".into(),
            base_url: normalize_omlx_base_url(
                &env::var("OMLX_BASE_URL").unwrap_or_else(|_| OMLX_BASE_URL_DEFAULT.into()),
            ),
            api_key_env: Some(OMLX_API_KEY_ENV.into()),
            local_placeholder_token: Some("omlx".into()),
        }
    }

    pub fn generic(base_url: String, api_key_env: Option<String>) -> Self {
        Self {
            kind: ProviderKind::Generic,
            name: "generic".into(),
            display_name: "OpenAI-compatible".into(),
            base_url: normalize_v1_base_url(&base_url),
            api_key_env,
            local_placeholder_token: None,
        }
    }

    pub fn token(&self) -> Option<String> {
        self.api_key_env
            .as_deref()
            .and_then(|key| env::var(key).ok())
            .filter(|value| !value.is_empty())
            .or_else(|| self.local_placeholder_token.clone())
    }
}

pub fn provider_by_name(
    name: &str,
    base_url: Option<String>,
    api_key_env: Option<String>,
) -> Result<ProviderInfo> {
    match name {
        "lmstudio" | "lm-studio" | "lm_studio" => Ok(ProviderInfo::lm_studio()),
        "ollama" => Ok(ProviderInfo::ollama()),
        "osaurus" => Ok(ProviderInfo::osaurus()),
        "omlx" => Ok(ProviderInfo::omlx()),
        "generic" | "openai" | "openai-compatible" => base_url
            .map(|url| ProviderInfo::generic(url, api_key_env))
            .ok_or_else(|| {
                OlaunchError::Message("generic provider requires --base-url <URL>".into())
            }),
        _ => Err(OlaunchError::UnknownProvider { name: name.into() }),
    }
}

pub fn default_provider(base_url: Option<String>, api_key_env: Option<String>) -> ProviderInfo {
    base_url
        .map(|url| ProviderInfo::generic(url, api_key_env))
        .unwrap_or_else(ProviderInfo::lm_studio)
}

pub fn default_providers() -> Vec<ProviderInfo> {
    vec![
        ProviderInfo::lm_studio(),
        ProviderInfo::ollama(),
        ProviderInfo::osaurus(),
        ProviderInfo::omlx(),
    ]
}

pub fn normalize_v1_base_url(raw: &str) -> String {
    let trimmed = raw.trim_end_matches('/');
    if trimmed.ends_with("/v1") {
        trimmed.into()
    } else {
        format!("{trimmed}/v1")
    }
}

pub fn normalize_omlx_base_url(raw: &str) -> String {
    normalize_v1_base_url(raw)
}

#[derive(Deserialize)]
struct OpenAiModelsResponse {
    data: Vec<OpenAiModel>,
}

#[derive(Deserialize)]
struct OpenAiModel {
    id: String,
}

#[derive(Deserialize)]
struct OmlxStatusResponse {
    #[serde(default)]
    models: Vec<OmlxStatusModel>,
}

#[derive(Deserialize)]
struct OmlxStatusModel {
    #[serde(alias = "name", alias = "model")]
    id: Option<String>,
    #[serde(alias = "context_window")]
    max_context_window: Option<u32>,
    #[serde(alias = "max_output_tokens")]
    max_tokens: Option<u32>,
    loaded: Option<bool>,
    engine_type: Option<String>,
}

pub fn discover_models() -> Result<Vec<ModelInfo>> {
    let handles = default_providers()
        .into_iter()
        .map(|provider| thread::spawn(move || discover_provider_models(provider)))
        .collect::<Vec<_>>();

    let mut models = Vec::new();
    let mut issues = Vec::new();

    for handle in handles {
        match handle.join() {
            Ok(Ok(found)) => models.extend(found),
            Ok(Err(err)) => issues.push(err.to_string()),
            Err(_) => issues.push("provider discovery thread panicked".into()),
        }
    }

    sort_models(&mut models);
    models.dedup_by(|left, right| left.provider.name == right.provider.name && left.id == right.id);

    if models.is_empty() {
        return Err(OlaunchError::NoModelsDiscovered(issues.join(" | ")));
    }

    Ok(models)
}

pub fn discover_provider_models(provider: ProviderInfo) -> Result<Vec<ModelInfo>> {
    if provider.kind == ProviderKind::Omlx
        && let Ok(models) = discover_omlx_status_models(provider.clone())
        && !models.is_empty()
    {
        return Ok(models);
    }

    discover_openai_models(provider)
}

fn discover_omlx_status_models(provider: ProviderInfo) -> Result<Vec<ModelInfo>> {
    let url = format!("{}/models/status", provider.base_url);
    let agent = http_agent();
    let mut request = agent.get(&url);
    if let Some(token) = provider.token() {
        request = request.header("Authorization", &format!("Bearer {token}"));
    }

    let response: OmlxStatusResponse = request
        .call()
        .map_err(|err| OlaunchError::Message(format!("{}: {err}", provider.display_name)))?
        .body_mut()
        .read_json()
        .map_err(|err| OlaunchError::Message(format!("{}: {err}", provider.display_name)))?;

    Ok(response
        .models
        .into_iter()
        .filter_map(|model| {
            let id = model.id?.trim().to_string();
            if id.is_empty() {
                return None;
            }
            // Only expose LLM and VLM models; filter out other engine types
            let engine_type = model.engine_type?;
            if engine_type != "llm" && engine_type != "vlm" {
                return None;
            }
            Some(ModelInfo {
                id,
                provider: provider.clone(),
                context_window: model.max_context_window,
                max_output_tokens: model.max_tokens,
                loaded: model.loaded,
            })
        })
        .collect())
}

fn discover_openai_models(provider: ProviderInfo) -> Result<Vec<ModelInfo>> {
    let url = format!("{}/models", provider.base_url);
    let agent = http_agent();
    let mut request = agent.get(&url);
    if let Some(token) = provider.token()
        && provider.api_key_env.is_some()
    {
        request = request.header("Authorization", &format!("Bearer {token}"));
    }

    let response: OpenAiModelsResponse = request
        .call()
        .map_err(|err| OlaunchError::Message(format!("{}: {err}", provider.display_name)))?
        .body_mut()
        .read_json()
        .map_err(|err| OlaunchError::Message(format!("{}: {err}", provider.display_name)))?;

    Ok(response
        .data
        .into_iter()
        .map(|model| model.id.trim().to_string())
        .filter(|id| !id.is_empty())
        .map(|id| ModelInfo {
            id,
            provider: provider.clone(),
            context_window: None,
            max_output_tokens: None,
            loaded: None,
        })
        .collect())
}

fn http_agent() -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_millis(300)))
        .timeout_global(Some(Duration::from_millis(700)))
        .build();
    config.into()
}

fn sort_models(models: &mut [ModelInfo]) {
    models.sort_by(|left, right| {
        provider_rank(&left.provider)
            .cmp(&provider_rank(&right.provider))
            .then_with(|| loaded_rank(left).cmp(&loaded_rank(right)))
            .then_with(|| left.id.cmp(&right.id))
    });
}

fn provider_rank(provider: &ProviderInfo) -> u8 {
    match provider.kind {
        ProviderKind::LmStudio => 0,
        ProviderKind::Ollama => 1,
        ProviderKind::Osaurus => 2,
        ProviderKind::Omlx => 3,
        ProviderKind::Generic => 4,
    }
}

fn loaded_rank(model: &ModelInfo) -> Ordering {
    match model.loaded {
        Some(true) => Ordering::Less,
        Some(false) | None => Ordering::Equal,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ProviderInfo, ProviderKind, normalize_omlx_base_url, normalize_v1_base_url,
        provider_by_name,
    };

    #[test]
    fn normalizes_v1_urls() {
        assert_eq!(
            normalize_v1_base_url("http://localhost:8000"),
            "http://localhost:8000/v1"
        );
        assert_eq!(
            normalize_v1_base_url("http://localhost:8000/v1/"),
            "http://localhost:8000/v1"
        );
        assert_eq!(
            normalize_omlx_base_url("http://localhost:8000"),
            "http://localhost:8000/v1"
        );
    }

    #[test]
    fn osaurus_provider_defaults() {
        let provider = ProviderInfo::osaurus();
        assert_eq!(provider.kind, ProviderKind::Osaurus);
        assert_eq!(provider.name, "osaurus");
        assert_eq!(provider.base_url, "http://localhost:1337/v1");
        assert_eq!(provider.local_placeholder_token.as_deref(), Some("osaurus"));
        assert_eq!(provider.token().as_deref(), Some("osaurus"));
    }

    #[test]
    fn resolves_osaurus_by_name() {
        let provider = provider_by_name("osaurus", None, None).expect("osaurus provider");
        assert_eq!(provider.kind, ProviderKind::Osaurus);
    }
}
