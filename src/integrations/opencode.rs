use std::{env, fs};

use serde_json::{Map, Value, json};

use crate::error::{OlaunchError, Result};
use crate::integrations::{
    ConfigEdit, EnvChange, Integration, IntegrationSpec, LaunchContext, LaunchPlan,
    provider_base_url, resolve_binary,
};
use crate::paths::Paths;

const PROVIDER: &str = "olaunch";
const MAX_RECENT_MODELS: usize = 10;

pub struct OpenCode;

impl Integration for OpenCode {
    fn spec(&self) -> IntegrationSpec {
        IntegrationSpec {
            name: "opencode",
            display_name: "OpenCode",
            aliases: &[],
            description: "AI coding agent built for the terminal",
            install_hint: "Install OpenCode from https://opencode.ai/docs/installation/.",
        }
    }

    fn plan_launch(&self, context: &LaunchContext) -> Result<LaunchPlan> {
        let config_content = opencode_inline_config(context)?;
        let state_path = context.paths.opencode_model_state();
        let existing_state = fs::read_to_string(&state_path).unwrap_or_default();
        let state_content = opencode_model_state(&existing_state, &context.model.id)?;

        let mut plan = LaunchPlan::new(
            "opencode",
            resolve_binary("opencode", Some(context.paths.opencode_local_binary())),
            context.extra_args.clone(),
        );
        let config_env = if inline_config_contains_secret(context) {
            EnvChange::set_secret("OPENCODE_CONFIG_CONTENT", config_content)
        } else {
            EnvChange::set("OPENCODE_CONFIG_CONTENT", config_content)
        };
        plan.env.push(config_env);
        plan.config_edits.push(ConfigEdit {
            path: state_path,
            content: state_content,
            description: "update OpenCode olaunch model picker state".into(),
            integration: "opencode".into(),
        });
        plan.restore_hints.push("olaunch restore opencode".into());
        Ok(plan)
    }

    fn installed(&self, paths: &Paths) -> bool {
        which::which("opencode").is_ok() || paths.opencode_local_binary().exists()
    }
}

fn opencode_inline_config(context: &LaunchContext) -> Result<String> {
    if context.model.id.trim().is_empty() {
        return Err(OlaunchError::Message(
            "opencode requires a non-empty model id".into(),
        ));
    }

    let mut options = Map::new();
    options.insert(
        "baseURL".into(),
        Value::String(provider_base_url(&context.model.provider)),
    );
    if context.model.provider.api_key_env.is_some()
        && let Some(token) = context.model.provider.token()
    {
        options.insert("apiKey".into(), Value::String(token));
    }

    let mut model_entry = Map::new();
    model_entry.insert("name".into(), Value::String(context.model.id.clone()));

    let mut limit = Map::new();
    if let Some(context_window) = context.model.context_window {
        limit.insert("context".into(), json!(context_window));
    }
    if let Some(max_output_tokens) = context.model.max_output_tokens {
        limit.insert("output".into(), json!(max_output_tokens));
    }
    if !limit.is_empty() {
        model_entry.insert("limit".into(), Value::Object(limit));
    }

    let mut models = Map::new();
    models.insert(context.model.id.clone(), Value::Object(model_entry));

    let provider_config = json!({
        "npm": "@ai-sdk/openai-compatible",
        "name": context.model.provider.display_name,
        "options": Value::Object(options),
        "models": Value::Object(models),
    });
    let mut providers = Map::new();
    providers.insert(PROVIDER.into(), provider_config);

    let config = json!({
        "$schema": "https://opencode.ai/config.json",
        "provider": Value::Object(providers),
        "model": format!("{PROVIDER}/{}", context.model.id),
    });
    Ok(serde_json::to_string(&config)?)
}

fn opencode_model_state(existing: &str, model_id: &str) -> Result<String> {
    let mut state = serde_json::from_str::<Value>(existing)
        .ok()
        .and_then(|value| match value {
            Value::Object(map) => Some(map),
            _ => None,
        })
        .unwrap_or_default();

    let recent = state
        .remove("recent")
        .and_then(|value| match value {
            Value::Array(values) => Some(values),
            _ => None,
        })
        .unwrap_or_default();

    let mut next_recent = Vec::with_capacity(MAX_RECENT_MODELS.min(recent.len() + 1));
    next_recent.push(json!({
        "providerID": PROVIDER,
        "modelID": model_id,
    }));
    for entry in recent {
        if is_opencode_model(&entry, model_id) {
            continue;
        }
        next_recent.push(entry);
        if next_recent.len() == MAX_RECENT_MODELS {
            break;
        }
    }

    if !state.contains_key("favorite") {
        state.insert("favorite".into(), Value::Array(Vec::new()));
    }
    if !state.contains_key("variant") {
        state.insert("variant".into(), Value::Object(Map::new()));
    }
    state.insert("recent".into(), Value::Array(next_recent));

    Ok(format!(
        "{}\n",
        serde_json::to_string_pretty(&Value::Object(state))?
    ))
}

fn is_opencode_model(entry: &Value, model_id: &str) -> bool {
    entry.as_object().is_some_and(|entry| {
        entry.get("providerID").and_then(Value::as_str) == Some(PROVIDER)
            && entry.get("modelID").and_then(Value::as_str) == Some(model_id)
    })
}

fn inline_config_contains_secret(context: &LaunchContext) -> bool {
    context
        .model
        .provider
        .api_key_env
        .as_deref()
        .and_then(|key| env::var(key).ok())
        .is_some_and(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{OpenCode, opencode_model_state};
    use crate::integrations::{Integration, LaunchContext};
    use crate::paths::Paths;
    use crate::providers::{ModelInfo, ProviderInfo};

    #[test]
    fn builds_inline_config_env_and_model_state_edit() {
        let tmp = tempfile::tempdir().unwrap();
        let plan = OpenCode
            .plan_launch(&LaunchContext {
                model: ModelInfo {
                    id: "qwen".into(),
                    provider: ProviderInfo::ollama(),
                    context_window: Some(128_000),
                    max_output_tokens: Some(8_192),
                    loaded: None,
                },
                paths: Paths::new(tmp.path()),
                extra_args: vec!["run".into(), "hello".into()],
                dry_run: true,
            })
            .unwrap();

        assert_eq!(plan.args, vec!["run", "hello"]);
        let config_env = plan
            .env
            .iter()
            .find(|env| env.key == "OPENCODE_CONFIG_CONTENT")
            .unwrap();
        assert!(!config_env.secret);
        let config: Value = serde_json::from_str(config_env.value.as_deref().unwrap()).unwrap();
        assert_eq!(config["model"], "olaunch/qwen");
        assert_eq!(
            config["provider"]["olaunch"]["npm"],
            "@ai-sdk/openai-compatible"
        );
        assert_eq!(
            config["provider"]["olaunch"]["options"]["baseURL"],
            "http://localhost:11434/v1"
        );
        assert_eq!(
            config["provider"]["olaunch"]["models"]["qwen"]["limit"]["context"],
            128_000
        );
        assert_eq!(
            config["provider"]["olaunch"]["models"]["qwen"]["limit"]["output"],
            8_192
        );
        assert_eq!(
            plan.config_edits[0].path,
            tmp.path().join(".local/state/opencode/model.json")
        );
        assert_eq!(plan.restore_hints, vec!["olaunch restore opencode"]);
    }

    #[test]
    fn updates_model_state_without_losing_other_recent_entries() {
        let existing = r#"{
  "recent": [
    { "providerID": "anthropic", "modelID": "claude-sonnet" },
    { "providerID": "olaunch", "modelID": "qwen" },
    { "providerID": "olaunch", "modelID": "llama" }
  ],
  "favorite": ["keep"]
}"#;

        let content = opencode_model_state(existing, "qwen").unwrap();
        let parsed: Value = serde_json::from_str(&content).unwrap();
        let recent = parsed["recent"].as_array().unwrap();

        assert_eq!(recent[0]["providerID"], "olaunch");
        assert_eq!(recent[0]["modelID"], "qwen");
        assert_eq!(recent[1]["providerID"], "anthropic");
        assert_eq!(recent[1]["modelID"], "claude-sonnet");
        assert_eq!(recent[2]["providerID"], "olaunch");
        assert_eq!(recent[2]["modelID"], "llama");
        assert_eq!(parsed["favorite"][0], "keep");
        assert!(parsed["variant"].is_object());
    }

    #[test]
    fn detects_curl_installer_binary_location() {
        let tmp = tempfile::tempdir().unwrap();
        let binary = Paths::new(tmp.path()).opencode_local_binary();
        std::fs::create_dir_all(binary.parent().unwrap()).unwrap();
        std::fs::write(&binary, "").unwrap();

        assert!(OpenCode.installed(&Paths::new(tmp.path())));
    }
}
