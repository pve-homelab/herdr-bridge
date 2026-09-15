# herdr-http-plugin

Herdr plugin that forwards `model.*` RPCs over a Unix domain socket (NDJSON) to an HTTP model backend (`/generate` or `/chat/completions`).

## Supported platforms

**Build and run targets:** Linux, macOS, and WSL2 on Windows.

This plugin binds `HERDR_PLUGIN_SOCKET` via `tokio::net::UnixListener`. Native Windows MSVC/GNU targets do **not** compile today — Tokio 1.x gates `UnixListener` behind `cfg(unix)` and does not yet expose AF_UNIX on Windows.

**Recommended on Windows hosts:** build and run inside WSL2, where Herdr runs and UDS is available.

## Overview

`herdr-http-plugin` is a single Tokio binary. Herdr launches it with `--plugin`, sets `HERDR_PLUGIN_SOCKET`, and speaks one NDJSON RPC per connection. The plugin:

- Probes `MODEL_BASE_URL` at startup and **auto-disables** model tools when the backend is missing or invalid
- Registers as Herdr's model provider when a valid endpoint is found
- Supports **streaming** (`model.stream_generate`) with live deltas to the TUI
- **Falls back** to a single non-streaming POST when streaming is unsupported or fails

No harness (Pi, OpenCode, etc.) is required.

## Install

**Herdr (recommended)** — clones from GitHub, runs `cargo build --release`, registers the plugin:

```bash
herdr plugin install pve-homelab/herdr-bridge
herdr plugin list
herdr plugin config-dir pve-homelab.herdr-http-plugin
herdr plugin action invoke pve-homelab.herdr-http-plugin.status
```

**Local link** (build yourself first; `plugin link` skips `[[build]]`):

```bash
cargo build --release
herdr plugin link .
```

**Cargo only** (binary on `PATH`, no Herdr registry):

```bash
cargo install --git https://github.com/pve-homelab/herdr-bridge.git --locked
# or:
cargo install --path . --locked
cargo build --release   # → ./target/release/herdr-http-plugin
```

See the root [README.md](../README.md) for uninstall, `--ref` / `--yes`, and marketplace notes.

## Configuration

| Variable | Required | Behavior |
|----------|----------|----------|
| `MODEL_BASE_URL` | For Active | Base URL (trimmed). Empty → auto-disable. Example: `http://localhost:8000/v1` |
| `MODEL_API_KEY` | No | If set, sends `Authorization: Bearer <key>` on probes and generate requests |
| `HERDR_PLUGIN_SOCKET` | When run as the NDJSON model bridge | Unix socket path from the host; missing → process exit |
| `RUST_LOG` | No | Tracing filter (default `info`) |

Use `herdr plugin config-dir pve-homelab.herdr-http-plugin` for user-editable config (do not write into the managed Git checkout).

## Run with Herdr

```bash
herdr plugin install pve-homelab/herdr-bridge
export MODEL_BASE_URL="http://localhost:8000/v1"
export MODEL_API_KEY="..."   # optional
herdr plugin action invoke pve-homelab.herdr-http-plugin.status
```

Point `MODEL_BASE_URL` at any OpenAI-compatible `/v1` server (e.g. Cursor-API, LM Studio) or a simple `/generate` endpoint.

## Auto-disable

At startup the plugin probes endpoints under `MODEL_BASE_URL`:

1. `GET {base}/generate`
2. If that fails (404 or unreachable), `GET {base}/chat/completions`

If both fail, the plugin still binds the socket but enters **Disabled** state:

- `model.info` reports `generation_enabled: false` and empty `tools`
- `model.generate` / `model.stream_generate` return structured errors citing the disable reason
- Herdr does not select this plugin as the active model provider

See [FEATURES.md](FEATURES.md) and [DIAGRAMS.md](DIAGRAMS.md) for the full decision tree.

## Streaming

When Active, `model.stream_generate`:

1. POSTs with `"stream": true`
2. Parses SSE (`data:` lines) or NDJSON chunks
3. Emits `{"event":"model.stream","delta":"..."}` lines (flushed after each delta)
4. Sends a final `{"id", "result":{"completion":"<full text>"}}`

Tokens appear live in the Herdr TUI as deltas arrive. See [TUI_USAGE.md](TUI_USAGE.md).

## Fallback

If streaming fails (connection error, HTTP 400/405/415, non-success status, or empty stream), the plugin logs a warning and performs one non-streaming POST (`stream: false`), returning a single final completion with no partial deltas.

## Further reading

| Doc | Contents |
|-----|----------|
| [ARCHITECTURE.md](ARCHITECTURE.md) | Modules, data flow, sequence diagrams |
| [TUI_USAGE.md](TUI_USAGE.md) | What you see in Herdr when streaming, errors, or disabled |
| [FEATURES.md](FEATURES.md) | Feature reference by topic |
| [DIAGRAMS.md](DIAGRAMS.md) | Mermaid and ASCII diagrams |

Design spec: [superpowers/specs/2026-09-15-herdr-http-plugin-design.md](superpowers/specs/2026-09-15-herdr-http-plugin-design.md)
