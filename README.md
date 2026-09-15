# herdr-http-plugin

Herdr plugin: Unix-socket NDJSON `model.*` RPCs → HTTP `/v1` model server.

See [docs/README.md](docs/README.md) for install, config, streaming, and auto-disable.

```bash
export MODEL_BASE_URL="http://localhost:8000/v1"
herdr --plugin ./target/debug/herdr-http-plugin
```
