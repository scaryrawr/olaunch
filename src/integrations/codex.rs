use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;
use toml_edit::{DocumentMut, Item, Table, value};

use crate::error::{OlaunchError, Result};
use crate::integrations::{
    ConfigEdit, EnvChange, Integration, IntegrationSpec, LaunchContext, LaunchPlan,
    provider_base_url, resolve_binary,
};
use crate::paths::Paths;

const PROFILE: &str = "olaunch";
const CODEX_APP_PROFILE: &str = "olaunch-codex-app";
const CODEX_APP_BUNDLE_ID: &str = "com.openai.codex";
const CODEX_APP_FALLBACK_CONTEXT_WINDOW: u32 = 128_000;

pub struct Codex;
pub struct CodexApp;

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

impl Integration for CodexApp {
    fn spec(&self) -> IntegrationSpec {
        IntegrationSpec {
            name: "codex-app",
            display_name: "Codex App",
            aliases: &["codex-desktop"],
            description: "OpenAI Codex desktop app",
            install_hint: "Install Codex from https://developers.openai.com/codex/.",
        }
    }

    fn plan_launch(&self, context: &LaunchContext) -> Result<LaunchPlan> {
        if !context.extra_args.is_empty() {
            return Err(OlaunchError::Message(
                "codex-app does not accept extra arguments".into(),
            ));
        }

        let config_path = context.paths.codex_config();
        let catalog_path = context.paths.codex_app_model_catalog();
        let existing = fs::read_to_string(&config_path).unwrap_or_default();
        let content = codex_app_config(&existing, context, &catalog_path)?;
        let catalog = codex_app_model_catalog(context)?;
        let (program, args) = codex_app_launch_command(&context.paths)?;

        let mut plan = LaunchPlan::new("codex-app", program, args);
        plan.config_edits.push(ConfigEdit {
            path: config_path,
            content,
            description: "configure Codex App olaunch profile".into(),
            integration: "codex-app".into(),
        });
        plan.config_edits.push(ConfigEdit {
            path: catalog_path,
            content: catalog,
            description: "write Codex App model catalog".into(),
            integration: "codex-app".into(),
        });
        plan.restore_hints.push("olaunch restore codex-app".into());
        Ok(plan)
    }

    fn installed(&self, paths: &Paths) -> bool {
        codex_app_installed(paths)
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

fn codex_app_config(
    existing: &str,
    context: &LaunchContext,
    catalog_path: &Path,
) -> Result<String> {
    let mut doc = existing
        .parse::<DocumentMut>()
        .unwrap_or_else(|_| DocumentMut::new());
    let catalog_path = catalog_path.display().to_string();
    let base_url = format!("{}/", provider_base_url(&context.model.provider));

    ensure_table(&mut doc, "profiles");
    ensure_nested_table(&mut doc, "profiles", CODEX_APP_PROFILE);
    ensure_table(&mut doc, "model_providers");
    ensure_nested_table(&mut doc, "model_providers", CODEX_APP_PROFILE);

    doc["profile"] = value(CODEX_APP_PROFILE);
    doc["model"] = value(context.model.id.clone());
    doc["model_provider"] = value(CODEX_APP_PROFILE);
    doc["model_catalog_json"] = value(catalog_path.clone());

    doc["profiles"][CODEX_APP_PROFILE]["model"] = value(context.model.id.clone());
    doc["profiles"][CODEX_APP_PROFILE]["model_provider"] = value(CODEX_APP_PROFILE);
    doc["profiles"][CODEX_APP_PROFILE]["openai_base_url"] = value(base_url.clone());
    doc["profiles"][CODEX_APP_PROFILE]["model_catalog_json"] = value(catalog_path);

    doc["model_providers"][CODEX_APP_PROFILE]["name"] = value("olaunch");
    doc["model_providers"][CODEX_APP_PROFILE]["base_url"] = value(base_url);
    doc["model_providers"][CODEX_APP_PROFILE]["wire_api"] = value("responses");
    if let Some(api_key_env) = &context.model.provider.api_key_env {
        doc["model_providers"][CODEX_APP_PROFILE]["env_key"] = value(api_key_env.clone());
    }

    Ok(doc.to_string())
}

fn codex_app_model_catalog(context: &LaunchContext) -> Result<String> {
    let context_window = context
        .model
        .context_window
        .unwrap_or(CODEX_APP_FALLBACK_CONTEXT_WINDOW);
    let catalog = json!({
        "models": [
            {
                "slug": &context.model.id,
                "display_name": &context.model.id,
                "description": format!("{} model", &context.model.provider.display_name),
                "default_reasoning_level": null,
                "supported_reasoning_levels": [],
                "shell_type": "default",
                "visibility": "list",
                "supported_in_api": true,
                "priority": 0,
                "additional_speed_tiers": [],
                "availability_nux": null,
                "upgrade": null,
                "base_instructions": "You are Codex, a coding agent. You and the user share the same workspace and collaborate to achieve the user's goals.",
                "model_messages": null,
                "supports_reasoning_summaries": false,
                "default_reasoning_summary": "auto",
                "support_verbosity": false,
                "default_verbosity": null,
                "apply_patch_tool_type": null,
                "web_search_tool_type": "text",
                "truncation_policy": {
                    "mode": "bytes",
                    "limit": 10_000,
                },
                "supports_parallel_tool_calls": false,
                "supports_image_detail_original": false,
                "context_window": context_window,
                "max_context_window": context_window,
                "auto_compact_token_limit": null,
                "effective_context_window_percent": 95,
                "experimental_supported_tools": [],
                "input_modalities": ["text"],
                "supports_search_tool": false,
            }
        ]
    });

    Ok(format!("{}\n", serde_json::to_string_pretty(&catalog)?))
}

fn codex_app_launch_command(paths: &Paths) -> Result<(PathBuf, Vec<String>)> {
    #[cfg(target_os = "macos")]
    {
        if let Some(path) = codex_app_macos_candidates(paths)
            .into_iter()
            .find(|path| path.is_dir())
        {
            return Ok((PathBuf::from("open"), vec![path.display().to_string()]));
        }
        return Ok((
            PathBuf::from("open"),
            vec!["-b".into(), CODEX_APP_BUNDLE_ID.into()],
        ));
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(path) = codex_app_windows_candidates(paths)
            .into_iter()
            .find(|path| path.is_file())
        {
            return Ok((path, Vec::new()));
        }
        return Ok((
            PathBuf::from("powershell.exe"),
            vec![
                "-NoProfile".into(),
                "-Command".into(),
                "Start-Process Codex".into(),
            ],
        ));
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = paths;
        Err(OlaunchError::Message(
            "Codex App launch is only supported on macOS and Windows".into(),
        ))
    }
}

fn codex_app_installed(paths: &Paths) -> bool {
    #[cfg(target_os = "macos")]
    {
        return codex_app_macos_candidates(paths)
            .into_iter()
            .any(|path| path.is_dir())
            || codex_app_bundle_available();
    }

    #[cfg(target_os = "windows")]
    {
        return codex_app_windows_candidates(paths)
            .into_iter()
            .any(|path| path.is_file());
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = paths;
        false
    }
}

#[cfg(target_os = "macos")]
fn codex_app_macos_candidates(paths: &Paths) -> Vec<PathBuf> {
    vec![
        PathBuf::from("/Applications/Codex.app"),
        paths.home().join("Applications").join("Codex.app"),
    ]
}

#[cfg(target_os = "macos")]
fn codex_app_bundle_available() -> bool {
    std::process::Command::new("mdfind")
        .arg(format!(
            "kMDItemCFBundleIdentifier == {CODEX_APP_BUNDLE_ID:?}"
        ))
        .output()
        .map(|output| {
            output.status.success() && !String::from_utf8_lossy(&output.stdout).trim().is_empty()
        })
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn codex_app_windows_candidates(paths: &Paths) -> Vec<PathBuf> {
    let local_app_data = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .map(PathBuf::from)
                .map(|home| home.join("AppData").join("Local"))
        })
        .unwrap_or_else(|| paths.home().join("AppData").join("Local"));

    [
        &["Programs", "Codex", "Codex.exe"][..],
        &["Programs", "OpenAI Codex", "Codex.exe"][..],
        &["Codex", "Codex.exe"][..],
        &["OpenAI Codex", "Codex.exe"][..],
        &["OpenAI", "Codex", "Codex.exe"][..],
        &["openai-codex-electron", "Codex.exe"][..],
    ]
    .into_iter()
    .map(|parts| {
        parts
            .iter()
            .fold(local_app_data.clone(), |path, part| path.join(part))
    })
    .collect()
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
    use serde_json::Value;

    use super::{codex_app_config, codex_app_model_catalog};
    use crate::integrations::{Codex, CodexApp, Integration, LaunchContext};
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

    #[test]
    fn writes_codex_app_root_profile_and_catalog_reference() {
        let tmp = tempfile::tempdir().unwrap();
        let path = Paths::new(tmp.path());
        let catalog_path = path.codex_app_model_catalog();

        let content = codex_app_config(
            "# keep me\nprofile = \"usual\"\n",
            &LaunchContext {
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
            },
            &catalog_path,
        )
        .unwrap();

        assert!(content.contains("# keep me"));
        assert!(content.contains("profile = \"olaunch-codex-app\""));
        assert!(content.contains("model = \"qwen\""));
        assert!(content.contains("model_provider = \"olaunch-codex-app\""));
        assert!(content.contains(&format!(
            "model_catalog_json = {:?}",
            catalog_path.display().to_string()
        )));
        assert!(content.contains("[profiles.olaunch-codex-app]"));
        assert!(content.contains("[model_providers.olaunch-codex-app]"));
    }

    #[test]
    fn writes_codex_app_catalog_metadata() {
        let tmp = tempfile::tempdir().unwrap();
        let catalog = codex_app_model_catalog(&LaunchContext {
            model: ModelInfo {
                id: "qwen".into(),
                provider: ProviderInfo::omlx(),
                context_window: Some(65_536),
                max_output_tokens: None,
                loaded: None,
            },
            paths: Paths::new(tmp.path()),
            extra_args: vec![],
            dry_run: true,
        })
        .unwrap();

        let parsed: Value = serde_json::from_str(&catalog).unwrap();
        let model = &parsed["models"][0];
        assert_eq!(model["slug"], "qwen");
        assert_eq!(model["context_window"], 65_536);
        assert_eq!(model["max_context_window"], 65_536);
        assert_eq!(model["input_modalities"], serde_json::json!(["text"]));
    }

    #[test]
    fn codex_app_rejects_passthrough_args() {
        let tmp = tempfile::tempdir().unwrap();
        let result = CodexApp.plan_launch(&LaunchContext {
            model: ModelInfo {
                id: "qwen".into(),
                provider: ProviderInfo::ollama(),
                context_window: None,
                max_output_tokens: None,
                loaded: None,
            },
            paths: Paths::new(tmp.path()),
            extra_args: vec!["--foo".into()],
            dry_run: true,
        });

        assert!(result.unwrap_err().to_string().contains("extra arguments"));
    }
}
