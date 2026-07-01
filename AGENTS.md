# Repository Guidelines

## Project Structure & Module Organization

`olaunch` is a Rust 2024 CLI for launching local and OpenAI-compatible model workflows through coding-agent integrations. The binary entrypoint is `src/main.rs`, which calls `olaunch::cli::run_env()`.

Core modules:

- `src/cli.rs` — clap command parsing, short integration aliases, fuzzy model selection, `doctor`, and restore dispatch.
- `src/providers.rs` — provider defaults, `/v1` URL normalization, local model discovery, and OpenAI-compatible model responses.
- `src/integrations/` — integration registry and `LaunchPlan` builders for Copilot, Claude, Codex, Codex App, Hermes, and OpenCode.
- `src/config.rs` — atomic config writes, backup creation, and restore support for config-editing integrations.
- `src/process.rs` — dry-run summaries, config application, and final process execution.

## Build, Test, and Development Commands

Use these from the repository root:

```bash
cargo fmt --check
cargo test --quiet
cargo build --release
```

For a narrow Rust test while iterating, run:

```bash
cargo test <test_name> --quiet
```

CI runs `cargo build --release`, `cargo test`, `cargo clippy --quiet`, and `cargo fmt -- --check` on Linux, macOS, and Windows.

## Coding Style & Naming Conventions

Use rustfmt defaults. Prefer typed project errors in `OlaunchError` for user-facing failures with stable exit codes; use `OlaunchError::Message` for contextual errors that do not need a dedicated variant. Keep provider URL handling centralized through `normalize_v1_base_url` or `provider_base_url` instead of hand-building `/v1` strings.

## Testing Guidelines

Tests are colocated in module-level `#[cfg(test)]` blocks under `src/`; there is no top-level `tests/` directory currently. When adding an integration, update `src/integrations/mod.rs` registry functions, add the module, implement `Integration::spec`, `plan_launch`, and `installed`, and add focused tests for aliases, args, env, and config edits.

## Security & Configuration Tips

Do not add CLI arguments that accept raw API keys. Pass secrets through named environment variables and represent secret launch environment changes with `EnvChange::set_secret`. Preserve user config whenever possible: existing Codex TOML, Hermes YAML, and similar files should be parsed and amended rather than overwritten wholesale. Config writes create `.olaunch-<integration>.<timestamp>.bak` backups; keep restore behavior compatible with those filenames. `process::run_command` uses `exec` on Unix, so apply all config edits and validation before invoking it.

For Copilot BYOK launches, keep `COPILOT_OFFLINE=true` with the custom provider environment so Copilot CLI stays on the configured provider and cannot fall back to GitHub-hosted model routing.
