# Agent instructions

## Project overview

`olaunch` is a Rust 2024 CLI for launching local and OpenAI-compatible model workflows through coding-agent integrations. The binary entry point is `src/main.rs`, which delegates to `olaunch::cli::run_env()`.

Core responsibilities live in:

- `src/cli.rs` - clap command parsing, short integration aliases, model resolution, `doctor`, and restore dispatch.
- `src/providers.rs` - provider defaults, `/v1` URL normalization, local model discovery, and OpenAI-compatible model responses.
- `src/integrations/` - integration registry and per-tool `LaunchPlan` builders for Copilot, Claude, Codex, and Hermes.
- `src/config.rs` - atomic config writes, backup creation, and restore support for config-editing integrations.
- `src/process.rs` - dry-run summaries, config application, and final process execution.

## Validation commands

Use these from the repository root:

```bash
cargo fmt --check
cargo test --quiet
```

For a narrow Rust test while iterating, run:

```bash
cargo test <test_name> --quiet
```

Run `cargo clippy --all-targets -- -D warnings` only when you intend to address the current lint baseline.

## Conventions and patterns

- Keep provider API keys out of command-line flags. Pass secrets through named environment variables and represent secret launch environment changes with `EnvChange::set_secret`.
- Normalize OpenAI-compatible provider base URLs through `normalize_v1_base_url` or `provider_base_url` instead of hand-building `/v1` strings.
- Build integrations through `LaunchPlan` rather than invoking tools directly. Config-editing integrations should add `ConfigEdit` entries and a matching restore hint.
- Preserve user config whenever possible. Existing Codex TOML and Hermes YAML are parsed and amended instead of overwritten wholesale.
- When adding an integration, update `src/integrations/mod.rs` registry functions, add the integration module, implement `Integration::spec`, `plan_launch`, and `installed`, and add focused tests for aliases, args, env, and config edits.
- Prefer typed project errors in `OlaunchError` for user-facing failures with stable exit codes; use `OlaunchError::Message` for contextual errors that do not need a dedicated variant.

## Safety notes

- Do not accept raw API keys as new CLI arguments; use `--api-key-env` or provider-specific environment variables.
- `process::run_command` uses `exec` on Unix, so code after launch will not run on success. Apply all required config edits and validation before invoking it.
- Config writes create `.olaunch-<integration>.<timestamp>.bak` backups. Keep restore behavior compatible with those filenames.
