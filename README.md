# olaunch

`olaunch` launches coding-agent CLIs with local or OpenAI-compatible model providers. It can discover local models, prepare the environment or config an integration needs, and then start the selected tool.

## Supported integrations

- `copilot` (`copilot-cli`)
- `claude` (`claude-code`)
- `codex`
- `hermes` (`hermes-agent`)

## Install

From a checkout of this repository:

```bash
cargo install --path .
```

## Quick start

Run an integration with a specific model:

```bash
olaunch copilot --model qwen3
```

Use the explicit `run` command when you prefer the full form:

```bash
olaunch run copilot --model qwen3 -- --allow-all-tools
```

Arguments after `--` are passed through to the launched integration.

Preview what `olaunch` would do without writing config files or starting the tool:

```bash
olaunch codex --model qwen3 --provider ollama --dry-run
```

## Commands

```bash
olaunch list integrations
olaunch list models
olaunch doctor
olaunch restore codex
```

- `list integrations` shows the available integrations and aliases.
- `list models` discovers models from configured local providers.
- `doctor` checks provider reachability and integration availability.
- `restore <integration>` restores the latest `olaunch` backup for integrations that edit config files.

## Providers

By default, `olaunch` checks these local providers:

1. LM Studio at `http://localhost:1234/v1`
2. Ollama at `http://localhost:11434/v1`
3. oMLX from `OMLX_BASE_URL` or `http://localhost:8000/v1`

Use `--provider` to choose a known provider:

```bash
olaunch codex --provider ollama --model qwen3
```

Use `--base-url` for any OpenAI-compatible endpoint:

```bash
olaunch claude --base-url http://localhost:8000/v1 --model qwen3
```

If the endpoint needs a key, put the key in an environment variable and pass the variable name:

```bash
export LOCAL_API_KEY=...
olaunch copilot --base-url http://localhost:8000/v1 --api-key-env LOCAL_API_KEY --model qwen3
```

`olaunch` does not accept raw API keys as command-line flags.

## Config changes and backups

Some integrations need config file changes before launch. `olaunch` creates a backup before writing those files and prints restore hints when changes are made.

To apply config changes without starting the integration, use:

```bash
olaunch hermes --model qwen3 --configure-only
```

To restore the latest backup:

```bash
olaunch restore hermes
```
