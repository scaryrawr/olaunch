use std::fs;

use toml_edit::{DocumentMut, Item, Table, value};

use crate::error::Result;
use crate::integrations::{
    ConfigEdit, EnvChange, Integration, IntegrationSpec, LaunchContext, LaunchPlan,
    provider_base_url, resolve_binary,
};
use crate::paths::Paths;

const PROFILE: &str = "olaunch";

pub struct Codex;

impl Integration for Codex {
    fn spec(&self) -> IntegrationSpec {
        IntegrationSpec {
            name: "codex",
            display_name: "Codex",
            aliases: &[],
            description: "OpenAI's open-source coding agent",
            install_hint: "Install Codex CLI from https://developers.openai.com/codex/cli/.",
        }
    }

    fn plan_launch(&self, context: &LaunchContext) -> Result<LaunchPlan> {
        let config_path = context.paths.codex_config();
        let existing = fs::read_to_string(&config_path).unwrap_or_default();
        let content = codex_config(&existing, context)?;

        let mut args = vec!["--profile".into(), PROFILE.into()];
        if !context.model.id.is_empty() {
            args.extend(["-m".into(), context.model.id.clone()]);
        }
        args.extend(context.extra_args.clone());

        let mut plan = LaunchPlan::new("codex", resolve_binary("codex", None), args);
        if let Some(token) = context.model.provider.token() {
            plan.env
                .push(EnvChange::set_secret("OPENAI_API_KEY", token));
        }
        plan.config_edits.push(ConfigEdit {
            path: config_path,
            content,
            description: "configure Codex olaunch profile".into(),
            integration: "codex".into(),
        });
        plan.restore_hints.push("olaunch restore codex".into());
        Ok(plan)
    }

    fn installed(&self, _paths: &Paths) -> bool {
        which::which("codex").is_ok()
    }
}

fn codex_config(existing: &str, context: &LaunchContext) -> Result<String> {
    let mut doc = existing
        .parse::<DocumentMut>()
        .unwrap_or_else(|_| DocumentMut::new());

    ensure_table(&mut doc, "profiles");
    ensure_nested_table(&mut doc, "profiles", PROFILE);
    ensure_table(&mut doc, "model_providers");
    ensure_nested_table(&mut doc, "model_providers", PROFILE);

    doc["profiles"][PROFILE]["model"] = value(context.model.id.clone());
    doc["profiles"][PROFILE]["model_provider"] = value(PROFILE);
    doc["profiles"][PROFILE]["openai_base_url"] =
        value(format!("{}/", provider_base_url(&context.model.provider)));
    doc["profiles"][PROFILE]["forced_login_method"] = value("api");

    doc["model_providers"][PROFILE]["name"] = value("olaunch");
    doc["model_providers"][PROFILE]["base_url"] =
        value(format!("{}/", provider_base_url(&context.model.provider)));
    doc["model_providers"][PROFILE]["wire_api"] = value("responses");

    Ok(doc.to_string())
}

fn ensure_table(doc: &mut DocumentMut, key: &str) {
    if !matches!(doc.get(key), Some(Item::Table(_))) {
        doc.insert(key, Item::Table(Table::new()));
    }
}

fn ensure_nested_table(doc: &mut DocumentMut, parent: &str, child: &str) {
    let Some(Item::Table(table)) = doc.get_mut(parent) else {
        return;
    };
    if !matches!(table.get(child), Some(Item::Table(_))) {
        table.insert(child, Item::Table(Table::new()));
    }
}

#[cfg(test)]
mod tests {
    use crate::integrations::{Codex, Integration, LaunchContext};
    use crate::paths::Paths;
    use crate::providers::{ModelInfo, ProviderInfo};

    #[test]
    fn writes_codex_profile_without_losing_existing_text() {
        let tmp = tempfile::tempdir().unwrap();
        let path = Paths::new(tmp.path());
        std::fs::create_dir_all(tmp.path().join(".codex")).unwrap();
        std::fs::write(
            path.codex_config(),
            "# keep me\napproval_policy = \"never\"\n",
        )
        .unwrap();

        let plan = Codex
            .plan_launch(&LaunchContext {
                model: ModelInfo {
                    id: "qwen".into(),
                    provider: ProviderInfo::ollama(),
                    context_window: None,
                    max_output_tokens: None,
                    loaded: None,
                },
                paths: path,
                extra_args: vec![],
                dry_run: true,
            })
            .unwrap();

        let content = &plan.config_edits[0].content;
        assert!(content.contains("# keep me"));
        assert!(content.contains("[profiles.olaunch]"));
        assert!(content.contains("model = \"qwen\""));
        assert!(content.contains("[model_providers.olaunch]"));
    }
}
