# herdr-http-plugin

Herdr plugin: Unix-socket NDJSON `model.*` RPCs → HTTP `/v1` model server.

## Supported platforms

**Build and run targets:** Linux, macOS, and WSL2 on Windows.

This plugin binds a Unix domain socket (`HERDR_PLUGIN_SOCKET`) via `tokio::net::UnixListener`. Native Windows MSVC/GNU targets do **not** compile today — Tokio 1.x gates `UnixListener` behind `cfg(unix)` and does not yet expose AF_UNIX on Windows.

**Recommended on Windows hosts:** build and run inside WSL2, where Herdr runs and UDS is available.

See [docs/README.md](docs/README.md) for install, config, streaming, and auto-disable. Full suite: [ARCHITECTURE](docs/ARCHITECTURE.md) · [TUI](docs/TUI_USAGE.md) · [FEATURES](docs/FEATURES.md) · [DIAGRAMS](docs/DIAGRAMS.md).

```bash
cargo build
export MODEL_BASE_URL="http://localhost:8000/v1"
export MODEL_API_KEY="..."   # optional
herdr --plugin ./target/debug/herdr-http-plugin
```
