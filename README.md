# olaunch

`olaunch` is an open launcher for local and OpenAI-compatible model workflows. It discovers local providers, selects a model, prepares the right environment or config for a coding agent, then launches the tool.

Initial integrations:

- `copilot` / `copilot-cli`
- `claude` / `claude-code`
- `codex`
- `hermes` / `hermes-agent`

Droid is intentionally deferred until the core registry and config-editing patterns settle.

## Usage

Launch with the explicit command:

```bash
olaunch run copilot --model qwen3 -- --allow-all-tools
```

Or use the short alias:

```bash
olaunch copilot --model qwen3
```

Inspect the launch plan without writing config files or starting the tool:

```bash
olaunch codex --model qwen3 --provider ollama --dry-run
```

List supported integrations and discovered models:

```bash
olaunch list integrations
olaunch list models
```

Check local provider and integration status:

```bash
olaunch doctor
```

Restore the latest `olaunch` backup for integrations that edit config:

```bash
olaunch restore codex
olaunch restore hermes
```

## Providers

Default provider priority follows `copilito`:

1. LM Studio at `http://localhost:1234/v1`
2. Ollama at `http://localhost:11434/v1`
3. oMLX from `OMLX_BASE_URL` or `http://localhost:8000/v1`

Use `--provider`, `--base-url`, and `--api-key-env` to target a specific OpenAI-compatible endpoint. Raw API keys are not accepted as command-line flags; pass secrets through environment variables.
