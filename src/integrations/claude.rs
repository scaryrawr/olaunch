use crate::error::Result;
use crate::integrations::{
    EnvChange, Integration, IntegrationSpec, LaunchContext, LaunchPlan, provider_base_url,
    resolve_binary,
};
use crate::paths::Paths;

pub struct ClaudeCode;

impl Integration for ClaudeCode {
    fn spec(&self) -> IntegrationSpec {
        IntegrationSpec {
            name: "claude",
            display_name: "Claude Code",
            aliases: &["claude-code"],
            description: "Anthropic's coding tool with subagents",
            install_hint: "Install Claude Code from https://code.claude.com/docs/en/quickstart.",
        }
    }

    fn plan_launch(&self, context: &LaunchContext) -> Result<LaunchPlan> {
        // Claude Code validates `--model` against its Anthropic-facing model
        // selector. Local provider model IDs belong in env defaults instead.
        let mut args = Vec::new();
        args.extend(context.extra_args.clone());

        let mut plan = LaunchPlan::new(
            "claude",
            resolve_binary("claude", Some(context.paths.claude_local_binary())),
            args,
        );
        let anthropic_base_url = provider_base_url(&context.model.provider)
            .trim_end_matches("/v1")
            .to_string();
        plan.env.extend([
            EnvChange::set("ANTHROPIC_BASE_URL", anthropic_base_url),
            EnvChange::remove("ANTHROPIC_API_KEY"),
            EnvChange::set("CLAUDE_CODE_ATTRIBUTION_HEADER", "0"),
            EnvChange::set("API_TIMEOUT_MS", "3000000"),
            EnvChange::set("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1"),
            EnvChange::set("ANTHROPIC_DEFAULT_OPUS_MODEL", context.model.id.clone()),
            EnvChange::set("ANTHROPIC_DEFAULT_SONNET_MODEL", context.model.id.clone()),
            EnvChange::set("ANTHROPIC_DEFAULT_HAIKU_MODEL", context.model.id.clone()),
            EnvChange::set("CLAUDE_CODE_SUBAGENT_MODEL", context.model.id.clone()),
        ]);
        if let Some(token) = context.model.provider.token() {
            plan.env
                .push(EnvChange::set_secret("ANTHROPIC_AUTH_TOKEN", token));
        }
        if let Some(context_window) = context.model.context_window {
            plan.env.push(EnvChange::set(
                "CLAUDE_CODE_AUTO_COMPACT_WINDOW",
                context_window.to_string(),
            ));
        }
        Ok(plan)
    }

    fn installed(&self, paths: &Paths) -> bool {
        which::which("claude").is_ok() || paths.claude_local_binary().exists()
    }
}

#[cfg(test)]
mod tests {
    use crate::integrations::{ClaudeCode, Integration, LaunchContext};
    use crate::paths::Paths;
    use crate::providers::{ModelInfo, ProviderInfo};

    #[test]
    fn configures_model_with_env_not_model_arg() {
        let plan = ClaudeCode
            .plan_launch(&LaunchContext {
                model: ModelInfo {
                    id: "Qwen3.5-122B-A10B-heretic-mxfp4".into(),
                    provider: ProviderInfo::ollama(),
                    context_window: Some(128000),
                    max_output_tokens: None,
                    loaded: None,
                },
                paths: Paths::new("/tmp"),
                extra_args: vec!["--debug".into()],
                dry_run: true,
            })
            .unwrap();

        assert_eq!(plan.args, vec!["--debug"]);
        let env = plan.env_map();
        assert_eq!(
            env.get("ANTHROPIC_BASE_URL").unwrap().as_deref(),
            Some("http://localhost:11434")
        );
        assert_eq!(
            env.get("ANTHROPIC_DEFAULT_SONNET_MODEL")
                .unwrap()
                .as_deref(),
            Some("Qwen3.5-122B-A10B-heretic-mxfp4")
        );
        assert_eq!(
            env.get("CLAUDE_CODE_AUTO_COMPACT_WINDOW")
                .unwrap()
                .as_deref(),
            Some("128000")
        );
    }
}
