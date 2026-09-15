# herdr-http-plugin

Herdr plugin that turns any OpenAI-compatible HTTP `/v1` model server into a model backend — no Pi, OpenCode, or other harness required.

It validates `MODEL_BASE_URL`, streams tokens when possible, falls back to a single non-stream response when streaming is unsupported, and auto-disables model tools when the backend is missing or invalid.

| Feature | Behavior |
|---------|----------|
| Auto-disable | Empty/invalid `MODEL_BASE_URL` → no generation tools |
| Streaming | SSE / chunked HTTP → live `model.stream` deltas |
| Fallback | Unsupported stream → one non-stream completion |
| Auth | Optional `MODEL_API_KEY` → `Authorization: Bearer …` |

## Requirements

- [Herdr](https://herdr.dev/) `0.7.0+`
- [Rust](https://rustup.rs/) / `cargo` (for install-time build)
- **Linux, macOS, or WSL2** — native Windows MSVC/GNU does not compile yet (`UnixListener` is Unix-only)

On Windows hosts, install and run inside WSL2.

## Install with Herdr (recommended)

Standard Herdr GitHub install — clones the repo, runs `cargo build --release`, and registers the plugin:

```bash
herdr plugin install pve-homelab/herdr-bridge
```

Useful follow-ups:

```bash
herdr plugin list
herdr plugin config-dir pve-homelab.herdr-http-plugin
herdr plugin action invoke pve-homelab.herdr-http-plugin.status
```

Pin a revision if you want:

```bash
herdr plugin install pve-homelab/herdr-bridge --ref main
herdr plugin install pve-homelab/herdr-bridge --yes   # skip interactive trust preview
```

Reinstall to refresh a managed checkout (there is no separate `plugin update` in Herdr v1):

```bash
herdr plugin install pve-homelab/herdr-bridge
```

Uninstall:

```bash
herdr plugin uninstall pve-homelab.herdr-http-plugin
# or:
herdr plugin uninstall pve-homelab/herdr-bridge
```

### Local development link

`plugin link` does **not** run `[[build]]` — build first, then link:

```bash
git clone https://github.com/pve-homelab/herdr-bridge.git
cd herdr-bridge
cargo build --release
herdr plugin link .
herdr plugin action invoke pve-homelab.herdr-http-plugin.status
```

## Configure

| Variable | Required | Description |
|----------|----------|-------------|
| `MODEL_BASE_URL` | For generation | e.g. `http://localhost:8000/v1` |
| `MODEL_API_KEY` | No | Bearer token for the model server |
| `HERDR_PLUGIN_SOCKET` | When launched as a socket model bridge | Set by the host that speaks the NDJSON `model.*` protocol |
| `RUST_LOG` | No | Tracing filter (default `info`) |

Put user-editable secrets under the plugin config dir (not the managed Git checkout):

```bash
herdr plugin config-dir pve-homelab.herdr-http-plugin
```

Point `MODEL_BASE_URL` at Cursor-API, LM Studio, vLLM, or any server that exposes `/v1/generate` or `/v1/chat/completions`.

Example:

```bash
export MODEL_BASE_URL="http://localhost:8000/v1"
export MODEL_API_KEY="..."   # optional
```

## Alternative: install the binary with Cargo

If you only want the binary on `PATH` (without Herdr’s plugin registry):

```bash
cargo install --git https://github.com/pve-homelab/herdr-bridge.git --locked
# or from a clone:
cargo install --path . --locked
```

Binary: `~/.cargo/bin/herdr-http-plugin` (or `./target/release/herdr-http-plugin` after `cargo build --release`).

## Marketplace

Repos tagged with the GitHub topic [`herdr-plugin`](https://github.com/topics/herdr-plugin) show up in Herdr’s marketplace index. This plugin’s manifest is [`herdr-plugin.toml`](herdr-plugin.toml) at the repo root.

## Documentation

- [docs/README.md](docs/README.md) — overview, auto-disable, streaming, fallback  
- [ARCHITECTURE](docs/ARCHITECTURE.md) · [TUI](docs/TUI_USAGE.md) · [FEATURES](docs/FEATURES.md) · [DIAGRAMS](docs/DIAGRAMS.md)
- Herdr plugin docs: https://herdr.dev/docs/plugins/

## License

MIT
