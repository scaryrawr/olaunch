use crate::error::Result;
use crate::integrations::{
    EnvChange, Integration, IntegrationSpec, LaunchContext, LaunchPlan, provider_base_url,
    resolve_binary,
};
use crate::paths::Paths;

pub struct Copilot;

impl Integration for Copilot {
    fn spec(&self) -> IntegrationSpec {
        IntegrationSpec {
            name: "copilot",
            display_name: "GitHub Copilot CLI",
            aliases: &["copilot-cli"],
            description: "GitHub's AI coding agent for the terminal",
            install_hint: "Install GitHub Copilot CLI, then ensure `copilot` is on PATH.",
        }
    }

    fn plan_launch(&self, context: &LaunchContext) -> Result<LaunchPlan> {
        let mut args = Vec::new();
        if !context.model.id.is_empty() {
            args.extend(["--model".into(), context.model.id.clone()]);
        }
        args.extend(context.extra_args.clone());

        let mut plan = LaunchPlan::new("copilot", resolve_binary("copilot", None), args);
        plan.env.extend([
            EnvChange::set(
                "COPILOT_PROVIDER_BASE_URL",
                provider_base_url(&context.model.provider),
            ),
            EnvChange::set("COPILOT_PROVIDER_TYPE", "openai"),
            EnvChange::set("COPILOT_OFFLINE", "true"),
            EnvChange::set("COPILOT_PROVIDER_WIRE_API", "responses"),
            EnvChange::set("COPILOT_MODEL", context.model.id.clone()),
            EnvChange::set("COPILOT_PROVIDER_MODEL_ID", context.model.id.clone()),
            EnvChange::set("COPILOT_PROVIDER_WIRE_MODEL", context.model.id.clone()),
        ]);
        if let Some(token) = context.model.provider.token() {
            plan.env.push(EnvChange::set_secret(
                "COPILOT_PROVIDER_BEARER_TOKEN",
                token,
            ));
        }
        if let Some(context_window) = context.model.context_window {
            plan.env.push(EnvChange::set(
                "COPILOT_PROVIDER_MAX_PROMPT_TOKENS",
                context_window.to_string(),
            ));
        }
        if let Some(max_output) = context.model.max_output_tokens {
            plan.env.push(EnvChange::set(
                "COPILOT_PROVIDER_MAX_OUTPUT_TOKENS",
                max_output.to_string(),
            ));
        }
        Ok(plan)
    }

    fn installed(&self, _paths: &Paths) -> bool {
        which::which("copilot").is_ok()
    }
}

#[cfg(test)]
mod tests {
    use crate::integrations::{Copilot, Integration, LaunchContext};
    use crate::paths::Paths;
    use crate::providers::{ModelInfo, ProviderInfo};

    #[test]
    fn builds_copilot_env_and_args() {
        let model = ModelInfo {
            id: "qwen".into(),
            provider: ProviderInfo::lm_studio(),
            context_window: Some(128000),
            max_output_tokens: Some(32000),
            loaded: None,
        };
        let plan = Copilot
            .plan_launch(&LaunchContext {
                model,
                paths: Paths::new("/tmp"),
                extra_args: vec!["--allow-all-tools".into()],
                dry_run: true,
            })
            .unwrap();
        assert_eq!(plan.args, vec!["--model", "qwen", "--allow-all-tools"]);
        let env = plan.env_map();
        assert_eq!(
            env.get("COPILOT_PROVIDER_BASE_URL").unwrap().as_deref(),
            Some("http://localhost:1234/v1")
        );
        assert_eq!(env.get("COPILOT_OFFLINE").unwrap().as_deref(), Some("true"));
        assert_eq!(env.get("COPILOT_MODEL").unwrap().as_deref(), Some("qwen"));
    }
}
