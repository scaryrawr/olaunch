---
applyTo: "**/*.rs"
name: Rust project patterns
description: Rust-specific conventions for olaunch source and tests.
---

# Rust project patterns

- Keep CLI parsing declarative with `clap` derives in `src/cli.rs`; route behavior through small helper functions instead of growing match arms inline.
- Add tests in the same module with `#[cfg(test)]` unless a scenario requires an integration test harness.
- Use `tempfile` for filesystem tests that touch home-relative config paths.
- Use `Result<T>` from `crate::error` for fallible project code and preserve typed `OlaunchError` variants for stable user-facing errors.
- Redact sensitive values in dry-run or summary output by marking env changes as secret.
