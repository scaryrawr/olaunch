use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::error::{OlaunchError, Result};
use crate::paths::Paths;
use crate::providers::{ModelInfo, ProviderInfo};

mod claude;
mod codex;
mod copilot;
mod hermes;

pub use claude::ClaudeCode;
pub use codex::{Codex, CodexApp};
pub use copilot::Copilot;
pub use hermes::Hermes;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegrationSpec {
    pub name: &'static str,
    pub display_name: &'static str,
    pub aliases: &'static [&'static str],
    pub description: &'static str,
    pub install_hint: &'static str,
}

#[derive(Clone, Debug)]
pub struct LaunchContext {
    pub model: ModelInfo,
    pub paths: Paths,
    pub extra_args: Vec<String>,
    pub dry_run: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvChange {
    pub key: String,
    pub value: Option<String>,
    pub secret: bool,
}

impl EnvChange {
    pub fn set(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: Some(value.into()),
            secret: false,
        }
    }

    pub fn set_secret(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: Some(value.into()),
            secret: true,
        }
    }

    pub fn remove(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: None,
            secret: false,
        }
    }

    pub fn display_value(&self) -> String {
        match (&self.value, self.secret) {
            (Some(_), true) => "<redacted>".into(),
            (Some(value), false) => value.clone(),
            (None, _) => "<removed>".into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigEdit {
    pub path: PathBuf,
    pub content: String,
    pub description: String,
    pub integration: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchPlan {
    pub integration: String,
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<EnvChange>,
    pub config_edits: Vec<ConfigEdit>,
    pub restore_hints: Vec<String>,
}

impl LaunchPlan {
    pub fn new(
        integration: impl Into<String>,
        program: impl Into<PathBuf>,
        args: Vec<String>,
    ) -> Self {
        Self {
            integration: integration.into(),
            program: program.into(),
            args,
            env: Vec::new(),
            config_edits: Vec::new(),
            restore_hints: Vec::new(),
        }
    }

    pub fn redacted_summary(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("integration: {}\n", self.integration));
        out.push_str(&format!("program: {}\n", self.program.display()));
        out.push_str(&format!("args: {}\n", shell_join(&self.args)));
        if !self.env.is_empty() {
            out.push_str("env:\n");
            for env in &self.env {
                out.push_str(&format!("  {}={}\n", env.key, env.display_value()));
            }
        }
        if !self.config_edits.is_empty() {
            out.push_str("config edits:\n");
            for edit in &self.config_edits {
                out.push_str(&format!(
                    "  {} ({})\n",
                    edit.path.display(),
                    edit.description
                ));
            }
        }
        if !self.restore_hints.is_empty() {
            out.push_str("restore hints:\n");
            for hint in &self.restore_hints {
                out.push_str(&format!("  {hint}\n"));
            }
        }
        out
    }

    pub fn env_map(&self) -> BTreeMap<String, Option<String>> {
        self.env
            .iter()
            .map(|change| (change.key.clone(), change.value.clone()))
            .collect()
    }
}

pub trait Integration {
    fn spec(&self) -> IntegrationSpec;
    fn plan_launch(&self, context: &LaunchContext) -> Result<LaunchPlan>;
    fn installed(&self, paths: &Paths) -> bool;
}

pub fn specs() -> Vec<IntegrationSpec> {
    vec![
        Copilot.spec(),
        ClaudeCode.spec(),
        Codex.spec(),
        CodexApp.spec(),
        Hermes.spec(),
    ]
}

pub fn names_and_aliases() -> Vec<&'static str> {
    let mut names = Vec::new();
    for spec in specs() {
        names.push(spec.name);
        names.extend(spec.aliases);
    }
    names
}

pub fn get(name: &str) -> Result<Box<dyn Integration>> {
    let normalized = name.to_ascii_lowercase();
    let integration: Box<dyn Integration> = match normalized.as_str() {
        "copilot" | "copilot-cli" => Box::new(Copilot),
        "claude" | "claude-code" => Box::new(ClaudeCode),
        "codex" => Box::new(Codex),
        "codex-app" | "codex-desktop" => Box::new(CodexApp),
        "hermes" | "hermes-agent" => Box::new(Hermes),
        _ => return Err(OlaunchError::UnknownIntegration { name: name.into() }),
    };
    Ok(integration)
}

pub fn provider_base_url(provider: &ProviderInfo) -> String {
    provider.base_url.trim_end_matches('/').to_string()
}

fn shell_join(args: &[String]) -> String {
    if args.is_empty() {
        return "<none>".into();
    }
    args.iter()
        .map(|arg| {
            if arg
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_./:=+".contains(c))
            {
                arg.clone()
            } else {
                format!("{arg:?}")
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn resolve_binary(name: &str, fallback: Option<PathBuf>) -> PathBuf {
    which::which(name)
        .ok()
        .or_else(|| fallback.filter(|path| path.exists()))
        .unwrap_or_else(|| PathBuf::from(name))
}

#[cfg(test)]
mod tests {
    use super::{get, specs};

    #[test]
    fn registry_contains_initial_integrations() {
        let names = specs()
            .into_iter()
            .map(|spec| spec.name)
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec!["copilot", "claude", "codex", "codex-app", "hermes"]
        );
    }

    #[test]
    fn resolves_aliases() {
        assert_eq!(get("copilot-cli").unwrap().spec().name, "copilot");
        assert_eq!(get("claude-code").unwrap().spec().name, "claude");
        assert_eq!(get("codex-desktop").unwrap().spec().name, "codex-app");
        assert_eq!(get("hermes-agent").unwrap().spec().name, "hermes");
    }
}
