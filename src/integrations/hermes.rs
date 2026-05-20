use std::fs;

use serde_yml::{Mapping, Value};

use crate::error::Result;
use crate::integrations::{
    ConfigEdit, Integration, IntegrationSpec, LaunchContext, LaunchPlan, provider_base_url,
    resolve_binary,
};
use crate::paths::Paths;

const PROVIDER: &str = "olaunch";
const HERMES_MIN_CONTEXT_LENGTH: u32 = 64_000;

pub struct Hermes;

impl Integration for Hermes {
    fn spec(&self) -> IntegrationSpec {
        IntegrationSpec {
            name: "hermes",
            display_name: "Hermes Agent",
            aliases: &["hermes-agent"],
            description: "Self-improving AI agent built by Nous Research",
            install_hint: "Install Hermes Agent from https://hermes-agent.nousresearch.com/docs/getting-started/installation/.",
        }
    }

    fn plan_launch(&self, context: &LaunchContext) -> Result<LaunchPlan> {
        let config_path = context.paths.hermes_config();
        let existing = fs::read_to_string(&config_path).unwrap_or_default();
        let content = hermes_config(&existing, context)?;

        let mut args = vec!["--provider".into(), PROVIDER.into()];
        if !context.model.id.is_empty() {
            args.extend(["--model".into(), context.model.id.clone()]);
        }
        args.extend(context.extra_args.clone());

        let mut plan = LaunchPlan::new("hermes", resolve_binary("hermes", None), args);
        plan.config_edits.push(ConfigEdit {
            path: config_path,
            content,
            description: "configure Hermes olaunch provider".into(),
            integration: "hermes".into(),
        });
        plan.restore_hints.push("olaunch restore hermes".into());
        Ok(plan)
    }

    fn installed(&self, _paths: &Paths) -> bool {
        which::which("hermes").is_ok()
    }
}

fn hermes_config(existing: &str, context: &LaunchContext) -> Result<String> {
    let mut root = match serde_yml::from_str::<Value>(existing).ok() {
        Some(Value::Mapping(map)) => map,
        _ => Mapping::new(),
    };

    let providers_key = Value::String("providers".into());
    let mut providers = match root.remove(&providers_key) {
        Some(Value::Mapping(map)) => map,
        _ => Mapping::new(),
    };

    let mut provider = Mapping::new();
    provider.insert(
        Value::String("name".into()),
        Value::String("olaunch".into()),
    );
    provider.insert(
        Value::String("base_url".into()),
        Value::String(format!(
            "{}/v1",
            provider_base_url(&context.model.provider).trim_end_matches("/v1")
        )),
    );
    provider.insert(
        Value::String("api_key".into()),
        Value::String(
            context
                .model
                .provider
                .token()
                .unwrap_or_else(|| "olaunch".into()),
        ),
    );
    provider.insert(
        Value::String("api_mode".into()),
        Value::String("chat_completions".into()),
    );
    provider.insert(
        Value::String("default_model".into()),
        Value::String(context.model.id.clone()),
    );
    providers.insert(Value::String(PROVIDER.into()), Value::Mapping(provider));
    root.insert(providers_key, Value::Mapping(providers));

    let model_key = Value::String("model".into());
    let mut model = match root.remove(&model_key) {
        Some(Value::Mapping(map)) => map,
        _ => Mapping::new(),
    };
    model.insert(
        Value::String("provider".into()),
        Value::String(PROVIDER.into()),
    );
    model.insert(
        Value::String("default".into()),
        Value::String(context.model.id.clone()),
    );
    if let Some(context_window) = context.model.context_window {
        model.insert(
            Value::String("context_length".into()),
            Value::Number((context_window.max(HERMES_MIN_CONTEXT_LENGTH) as u64).into()),
        );
    }
    if let Some(max_tokens) = context.model.max_output_tokens {
        model.insert(
            Value::String("max_tokens".into()),
            Value::Number((max_tokens as u64).into()),
        );
    }
    root.insert(model_key, Value::Mapping(model));

    Ok(serde_yml::to_string(&Value::Mapping(root))?)
}

#[cfg(test)]
mod tests {
    use crate::integrations::{Hermes, Integration, LaunchContext};
    use crate::paths::Paths;
    use crate::providers::{ModelInfo, ProviderInfo};

    #[test]
    fn writes_hermes_provider_and_model() {
        let tmp = tempfile::tempdir().unwrap();
        let path = Paths::new(tmp.path());
        std::fs::create_dir_all(tmp.path().join(".hermes")).unwrap();
        std::fs::write(path.hermes_config(), "toolsets:\n- web\n").unwrap();

        let plan = Hermes
            .plan_launch(&LaunchContext {
                model: ModelInfo {
                    id: "qwen".into(),
                    provider: ProviderInfo::omlx(),
                    context_window: Some(32000),
                    max_output_tokens: Some(8000),
                    loaded: None,
                },
                paths: path,
                extra_args: vec![],
                dry_run: true,
            })
            .unwrap();

        let content = &plan.config_edits[0].content;
        assert!(content.contains("toolsets:"));
        assert!(content.contains("olaunch:"));
        assert!(content.contains("default: qwen"));
        assert!(content.contains("context_length: 64000"));
    }

    #[test]
    fn launches_classic_chat_by_default() {
        let tmp = tempfile::tempdir().unwrap();
        let path = Paths::new(tmp.path());

        let plan = Hermes
            .plan_launch(&LaunchContext {
                model: ModelInfo {
                    id: "qwen".into(),
                    provider: ProviderInfo::omlx(),
                    context_window: None,
                    max_output_tokens: None,
                    loaded: None,
                },
                paths: path,
                extra_args: vec![],
                dry_run: true,
            })
            .unwrap();

        assert_eq!(plan.args, vec!["--provider", "olaunch", "--model", "qwen"]);
    }

    #[test]
    fn allows_explicit_tui_passthrough() {
        let tmp = tempfile::tempdir().unwrap();
        let path = Paths::new(tmp.path());

        let plan = Hermes
            .plan_launch(&LaunchContext {
                model: ModelInfo {
                    id: "qwen".into(),
                    provider: ProviderInfo::omlx(),
                    context_window: None,
                    max_output_tokens: None,
                    loaded: None,
                },
                paths: path,
                extra_args: vec!["--tui".into()],
                dry_run: true,
            })
            .unwrap();

        assert_eq!(
            plan.args,
            vec!["--provider", "olaunch", "--model", "qwen", "--tui"]
        );
    }
}
