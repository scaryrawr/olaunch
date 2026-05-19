# Copilot instructions

This repository is a Rust 2024 CLI named `olaunch`. It launches coding-agent integrations against local or OpenAI-compatible model providers by building a `LaunchPlan`, applying any needed config edits, then executing the selected tool.

When editing:

- Keep changes surgical and preserve the existing module boundaries: CLI dispatch in `src/cli.rs`, provider discovery in `src/providers.rs`, launch planning in `src/integrations/`, config backup/restore in `src/config.rs`, and process execution in `src/process.rs`.
- Do not introduce CLI flags that carry raw secrets. Use environment variable names and `EnvChange::set_secret` for secret values.
- Use the existing provider URL helpers instead of hand-normalizing `/v1` URLs.
- Add or update focused unit tests near the changed code.
- Validate Rust changes with `cargo fmt --check` and `cargo test --quiet` from the repo root.

Copilot-specific behavior:

- Prefer concise explanations that lead with the observable behavior change.
- For path-specific Rust patterns, also follow files under `.github/instructions/` when their `applyTo` patterns match.
