use std::ffi::OsString;
use std::io::{self, IsTerminal};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use dialoguer::Select;

use crate::config;
use crate::error::{OlaunchError, Result};
use crate::integrations::{self, LaunchContext};
use crate::paths::Paths;
use crate::process;
use crate::providers::{self, ModelInfo, ProviderInfo};

#[derive(Parser, Debug)]
#[command(
    name = "olaunch",
    version,
    about = "Open launcher for local/open model coding agents"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Launch an integration.
    Run(RunArgs),
    /// List integrations or models.
    List(ListArgs),
    /// Check providers and integrations.
    Doctor,
    /// Restore the latest olaunch backup for an integration.
    Restore { integration: String },
}

#[derive(Args, Debug)]
struct ListArgs {
    #[command(subcommand)]
    command: ListCommand,
}

#[derive(Subcommand, Debug)]
enum ListCommand {
    Integrations,
    Models {
        #[arg(long)]
        provider: Option<String>,
        #[arg(long)]
        base_url: Option<String>,
        #[arg(long = "api-key-env")]
        api_key_env: Option<String>,
    },
}

#[derive(Args, Debug)]
struct RunArgs {
    integration: String,
    #[arg(long)]
    model: Option<String>,
    #[arg(long)]
    provider: Option<String>,
    #[arg(long)]
    base_url: Option<String>,
    #[arg(long = "api-key-env")]
    api_key_env: Option<String>,
    #[arg(long)]
    dry_run: bool,
    #[arg(long = "configure-only")]
    configure_only: bool,
    #[arg(long)]
    yes: bool,
    #[arg(num_args = 0.., trailing_var_arg = true, allow_hyphen_values = true)]
    extra_args: Vec<String>,
}

pub fn run_env() -> ExitCode {
    init_tracing();
    match run_from(std::env::args_os()) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(err.exit_code() as u8)
        }
    }
}

pub fn run_from<I, T>(args: I) -> Result<ExitCode>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let args = normalize_alias_args(args);
    let cli = Cli::try_parse_from(args).map_err(|err| OlaunchError::Message(err.to_string()))?;
    dispatch(cli)
}

fn dispatch(cli: Cli) -> Result<ExitCode> {
    match cli.command {
        Command::Run(args) => run_integration(args),
        Command::List(args) => list(args),
        Command::Doctor => doctor(),
        Command::Restore { integration } => restore(&integration),
    }
}

fn run_integration(args: RunArgs) -> Result<ExitCode> {
    let paths = Paths::detect()?;
    let integration = integrations::get(&args.integration)?;
    let model = resolve_model(&args)?;
    let context = LaunchContext {
        model,
        paths,
        extra_args: args.extra_args,
        dry_run: args.dry_run,
    };
    let plan = integration.plan_launch(&context)?;
    process::execute(&plan, args.configure_only, args.dry_run)
}

fn list(args: ListArgs) -> Result<ExitCode> {
    match args.command {
        ListCommand::Integrations => {
            for spec in integrations::specs() {
                let aliases = if spec.aliases.is_empty() {
                    String::new()
                } else {
                    format!(" (aliases: {})", spec.aliases.join(", "))
                };
                println!("{} - {}{}", spec.name, spec.description, aliases);
            }
        }
        ListCommand::Models {
            provider,
            base_url,
            api_key_env,
        } => {
            let models = if let Some(provider) = provider {
                let provider = providers::provider_by_name(&provider, base_url, api_key_env)?;
                providers::discover_provider_models(provider)?
            } else if let Some(base_url) = base_url {
                providers::discover_provider_models(ProviderInfo::generic(base_url, api_key_env))?
            } else {
                providers::discover_models()?
            };
            for model in models {
                println!("{}\t{}", model.provider.display_name, model.id);
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn doctor() -> Result<ExitCode> {
    let paths = Paths::detect()?;
    println!("Integrations:");
    for spec in integrations::specs() {
        let integration = integrations::get(spec.name)?;
        let status = if integration.installed(&paths) {
            "installed"
        } else {
            "missing"
        };
        println!("  {:<8} {}", spec.name, status);
        if status == "missing" {
            println!("           {}", spec.install_hint);
        }
    }

    println!("\nProviders:");
    for provider in providers::default_providers() {
        match providers::discover_provider_models(provider.clone()) {
            Ok(models) => println!("  {:<9} reachable ({} models)", provider.name, models.len()),
            Err(err) => println!("  {:<9} unavailable ({err})", provider.name),
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn restore(name: &str) -> Result<ExitCode> {
    let paths = Paths::detect()?;
    match name {
        "codex" => restore_path(&paths.codex_config(), "codex")?,
        "codex-app" | "codex-desktop" => {
            restore_path(&paths.codex_config(), "codex-app")?;
            restore_path(&paths.codex_app_model_catalog(), "codex-app")?;
        }
        "hermes" => restore_path(&paths.hermes_config(), "hermes")?,
        "opencode" => restore_path(&paths.opencode_model_state(), "opencode")?,
        other => {
            return Err(OlaunchError::Message(format!(
                "`{other}` does not have restore-managed config files yet"
            )));
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn restore_path(path: &std::path::Path, integration: &str) -> Result<()> {
    match config::restore_latest(path, integration)? {
        Some(backup) => println!("restored {} from {}", path.display(), backup.display()),
        None => println!("no olaunch backup found for {}", path.display()),
    }
    Ok(())
}

fn resolve_model(args: &RunArgs) -> Result<ModelInfo> {
    if let Some(model_id) = &args.model {
        let provider = if let Some(provider) = &args.provider {
            providers::provider_by_name(provider, args.base_url.clone(), args.api_key_env.clone())?
        } else {
            providers::default_provider(args.base_url.clone(), args.api_key_env.clone())
        };
        return Ok(ModelInfo {
            id: model_id.clone(),
            provider,
            context_window: None,
            max_output_tokens: None,
            loaded: None,
        });
    }

    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(OlaunchError::MissingModelNonInteractive);
    }

    let models = if let Some(provider) = &args.provider {
        let provider =
            providers::provider_by_name(provider, args.base_url.clone(), args.api_key_env.clone())?;
        providers::discover_provider_models(provider)?
    } else if let Some(base_url) = &args.base_url {
        providers::discover_provider_models(ProviderInfo::generic(
            base_url.clone(),
            args.api_key_env.clone(),
        ))?
    } else {
        providers::discover_models()?
    };
    pick_model(models)
}

fn pick_model(models: Vec<ModelInfo>) -> Result<ModelInfo> {
    if models.len() == 1 {
        return Ok(models.into_iter().next().unwrap());
    }
    let items = models
        .iter()
        .map(|model| format!("{} ({})", model.id, model.provider.display_name))
        .collect::<Vec<_>>();
    let selected = Select::new()
        .with_prompt("Select a model")
        .items(&items)
        .default(0)
        .interact_opt()
        .map_err(|err| OlaunchError::Message(format!("model selection failed: {err}")))?
        .ok_or_else(|| OlaunchError::Message("no model selected".into()))?;
    Ok(models[selected].clone())
}

fn normalize_alias_args<I, T>(args: I) -> Vec<OsString>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let mut args = args.into_iter().map(Into::into).collect::<Vec<OsString>>();
    if args.len() < 2 {
        return args;
    }
    let Some(first) = args[1].to_str() else {
        return args;
    };
    if first.starts_with('-') || is_top_level_command(first) {
        return args;
    }
    if integrations::names_and_aliases().contains(&first) {
        args.insert(1, OsString::from("run"));
    }
    args
}

fn is_top_level_command(value: &str) -> bool {
    matches!(
        value,
        "run" | "list" | "doctor" | "restore" | "help" | "--help" | "-h" | "--version" | "-V"
    )
}

fn init_tracing() {
    if std::env::var_os("OLAUNCH_LOG").is_none() {
        return;
    }
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::run_from;

    #[test]
    fn short_alias_parses_as_run() {
        let result = run_from(["olaunch", "copilot", "--model", "qwen", "--dry-run"]);
        assert!(result.is_ok());
    }

    #[test]
    fn unknown_integration_errors() {
        let result = run_from(["olaunch", "run", "missing", "--model", "qwen", "--dry-run"]);
        assert!(result.is_err());
    }
}
