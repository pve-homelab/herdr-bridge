# TUI usage

How `herdr-http-plugin` behaves inside the Herdr terminal UI when you generate text, hit errors, or run without a valid backend.

## Prerequisites

Run Herdr with the plugin on a supported platform (Linux, macOS, or WSL2):

```bash
export MODEL_BASE_URL="http://localhost:8000/v1"
herdr --plugin ./target/debug/herdr-http-plugin
```

Herdr sets `HERDR_PLUGIN_SOCKET` and connects to the plugin for `model.*` RPCs.

## Active backend — streaming

When startup validation succeeds, Herdr selects this plugin as the model provider (when configured to use plugin models). During `model.stream_generate`:

1. You invoke a model action in the TUI (prompt / generate).
2. Herdr sends `model.stream_generate` over the plugin socket.
3. The plugin POSTs to your HTTP backend with `stream: true`.
4. As each delta arrives, the plugin writes `{"event":"model.stream","delta":"..."}` to the socket.
5. **Tokens appear live** in the TUI as Herdr renders each delta.
6. When the stream completes, a final result line carries the full `completion` string.

If the backend does not support streaming, the plugin falls back silently (warn log only) to one non-streaming request — you see the full reply at once with no incremental tokens.

## Errors in the TUI

Failures surface as plugin error messages (NDJSON `error.message`), which Herdr displays in the UI:

| Cause | Typical TUI experience |
|-------|------------------------|
| Missing `params.prompt` | Error before any HTTP call |
| HTTP / parse failure on `model.generate` | Error message with backend details |
| Stream failure after fallback also fails | Error from fallback POST |
| Unknown RPC method | `unknown method: ...` |

Errors do not crash the plugin process; only that RPC's connection closes.

## Auto-disable — provider not selected

When `MODEL_BASE_URL` is empty or both probe URLs fail:

- `model.info` returns `generation_enabled: false`, `streaming: false`, `tools: []`, and `disabled_reason`
- Herdr **does not** register model generation tools from this plugin
- The TUI shows **no model provider** from this plugin (tools missing / generation unavailable)
- Direct generate attempts that still target the plugin receive `model tools disabled: <reason>`

The plugin process keeps running and the socket stays bound so Herdr can query `model.info`; fixing `MODEL_BASE_URL` and restarting the plugin re-enables tools.

## ASCII: TUI interaction (streaming)

```text
┌──────────────────────────────────────────────────────────────┐
│ Herdr TUI                                                    │
│  User: "Explain NDJSON"                                      │
│  ┌────────────────────────────────────────────────────────┐  │
│  │ NDJSON is a format where each line is one JSON object… │  │  ◄── deltas append live
│  └────────────────────────────────────────────────────────┘  │
└───────────────────────────────┬──────────────────────────────┘
                                │ model.stream_generate
                                ▼
                    ┌───────────────────────┐
                    │   herdr-http-plugin   │
                    │  stream → HTTP backend│
                    └───────────────────────┘
                                │
              {"event":"model.stream","delta":"NDJSON"}
              {"event":"model.stream","delta":" is"}
              …
              {"id":N,"result":{"completion":"…full…"}}
```

## ASCII: TUI interaction (disabled)

```text
┌──────────────────────────────────────────────────────────────┐
│ Herdr TUI                                                    │
│  Model: (none — plugin tools disabled)                       │
│  disabled_reason: no valid endpoint under http://…         │
│  [Generate unavailable until MODEL_BASE_URL is valid]        │
└───────────────────────────────┬──────────────────────────────┘
                                │ model.info
                                ▼
                    generation_enabled: false
                    tools: []
```

## ASCII: TUI interaction (error)

```text
┌──────────────────────────────────────────────────────────────┐
│ Herdr TUI                                                    │
│  ⚠ generate HTTP 502: {"error":"upstream down"}              │
└───────────────────────────────┬──────────────────────────────┘
                                │ model.generate
                                ▼
                    {"id":1,"error":{"message":"generate HTTP 502: …"}}
```

## Tips

- Set `RUST_LOG=debug` to correlate TUI behavior with plugin logs in the terminal where Herdr launched the plugin.
- Use a local OpenAI-compatible server on the same host as Herdr (WSL) to avoid probe failures from wrong interface or firewall rules.

## Related docs

- [README.md](README.md) — install and env vars
- [FEATURES.md](FEATURES.md) — auto-disable, streaming, fallback details
- [DIAGRAMS.md](DIAGRAMS.md) — TUI rendering flow diagram
