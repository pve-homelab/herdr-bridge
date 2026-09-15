# herdr-http-plugin

Herdr plugin that turns any OpenAI-compatible HTTP `/v1` model server into Herdr’s model backend — no Pi, OpenCode, or other harness required.

Herdr speaks NDJSON `model.*` RPCs over a Unix domain socket. This plugin validates `MODEL_BASE_URL`, streams tokens into the TUI when possible, and falls back to a single non-stream response when streaming is unsupported. If the backend is missing or invalid, it auto-disables model tools so Herdr ignores it for generation.

| Feature | Behavior |
|---------|----------|
| Auto-disable | Empty/invalid `MODEL_BASE_URL` → no generation tools |
| Streaming | SSE / chunked HTTP → live `model.stream` deltas |
| Fallback | Unsupported stream → one non-stream completion |
| Auth | Optional `MODEL_API_KEY` → `Authorization: Bearer …` |

## Requirements

- [Rust](https://rustup.rs/) (stable)
- Herdr with `--plugin` support
- **Linux, macOS, or WSL2** — native Windows targets do not compile yet (`tokio::net::UnixListener` is Unix-only)

On Windows, build and run inside WSL2 (where Herdr typically runs).

## Install from GitHub

Install the release binary with Cargo (no clone required):

```bash
cargo install --git https://github.com/pve-homelab/herdr-bridge.git --locked
```

Binary lands on your `PATH` as `herdr-http-plugin` (usually `~/.cargo/bin`).

Or clone and install from a local checkout:

```bash
git clone https://github.com/pve-homelab/herdr-bridge.git
cd herdr-bridge
cargo install --path . --locked
```

## Build from source

```bash
git clone https://github.com/pve-homelab/herdr-bridge.git
cd herdr-bridge
cargo build --release
```

Debug build: `cargo build` → `./target/debug/herdr-http-plugin`  
Release build: `./target/release/herdr-http-plugin`

## Configure and run

| Variable | Required | Description |
|----------|----------|-------------|
| `HERDR_PLUGIN_SOCKET` | Yes | Set by Herdr when using `--plugin` |
| `MODEL_BASE_URL` | For generation | e.g. `http://localhost:8000/v1` |
| `MODEL_API_KEY` | No | Bearer token for the model server |
| `RUST_LOG` | No | Tracing filter (default `info`) |

```bash
export MODEL_BASE_URL="http://localhost:8000/v1"
export MODEL_API_KEY="..."   # optional

# after cargo install:
herdr --plugin herdr-http-plugin

# or from a local build:
herdr --plugin ./target/release/herdr-http-plugin
```

Point `MODEL_BASE_URL` at Cursor-API, LM Studio, vLLM, or any server that exposes `/v1/generate` or `/v1/chat/completions`.

## Documentation

- [docs/README.md](docs/README.md) — overview, auto-disable, streaming, fallback  
- [ARCHITECTURE](docs/ARCHITECTURE.md) · [TUI](docs/TUI_USAGE.md) · [FEATURES](docs/FEATURES.md) · [DIAGRAMS](docs/DIAGRAMS.md)

## License

MIT
