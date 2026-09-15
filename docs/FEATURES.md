# Features

Reference for `herdr-http-plugin` behavior. See [DIAGRAMS.md](DIAGRAMS.md) for visual flows.

## Auto-disable

Startup never aborts solely because the model backend is bad — only because `HERDR_PLUGIN_SOCKET` is missing or UDS bind fails.

When `MODEL_BASE_URL` is empty or both endpoint probes fail:

- State becomes `BackendState::Disabled { reason }`
- Log: `backend disabled; serving socket without model tools`
- `model.info` exposes `generation_enabled: false`, empty `tools`, and `disabled_reason`
- Generate methods return `model tools disabled: <reason>`

Herdr skips this plugin for model generation until configuration and backend are fixed and the plugin is restarted.

## Streaming

Implemented in `streaming.rs` for Active backends only.

1. Build POST body with `"stream": true` (Generate or ChatCompletions shape from `model.rs`)
2. Send request with optional Bearer auth
3. Consume `bytes_stream()`, buffer UTF-8 across chunk boundaries
4. Parse SSE `data:` lines (skip `[DONE]`) or raw NDJSON lines
5. Extract text from `choices[0].delta.content`, `text`, or fallback raw line
6. For each non-empty delta: write `{"event":"model.stream","delta":"..."}` and **flush**
7. Accumulate full text; emit final `{"id", "result":{"completion":"<full>"}}`

Herdr renders deltas in the TUI in real time.

## Fallback

If streaming cannot produce usable partial output, the plugin performs exactly **one** non-streaming POST (`stream: false`) and returns a single final completion (no deltas).

Fallback triggers:

- Stream request connection error
- HTTP status 400, 405, 415, or any non-success status on stream POST
- Stream completes with zero deltas (empty stream)

A warning is logged; the TUI may show the full reply at once instead of token-by-token.

## Endpoint validation

`validation.rs` runs once at startup:

| Step | Action |
|------|--------|
| 1 | Trim whitespace and trailing `/` from `MODEL_BASE_URL` |
| 2 | Empty → Disabled (`MODEL_BASE_URL is empty`) |
| 3 | `GET {base}/generate` — 404 or unreachable → try next |
| 4 | `GET {base}/chat/completions` — same rules |
| 5 | First success → Active with that URL and kind (`Generate` or `ChatCompletions`) |
| 6 | Both fail → Disabled (`no valid endpoint under {base}`) |

**Probe success:** TCP/HTTP completes and status is not 404. Any non-404 status counts as path exists (including 405 on GET).

Generate is preferred when both paths would succeed; chat is used when `/generate` is missing.

## Tool registration

Via `model.info` (`rpc.rs`):

**Active:**

```json
{
  "name": "herdr-http-plugin",
  "generation_enabled": true,
  "streaming": true,
  "endpoint": "http://…/generate",
  "endpoint_kind": "generate",
  "tools": ["model.generate", "model.stream_generate", "model.info"]
}
```

**Disabled:**

```json
{
  "name": "herdr-http-plugin",
  "generation_enabled": false,
  "streaming": false,
  "tools": [],
  "disabled_reason": "…"
}
```

Herdr uses this to decide whether to expose model tools and select the plugin as provider.

## Socket lifecycle

1. **Startup:** Remove stale socket file if present; bind `HERDR_PLUGIN_SOCKET` (failure → exit with error, no TCP fallback)
2. **Accept loop:** Each connection spawned on its own task
3. **Per connection:** Read one NDJSON line; if EOF/empty, close quietly
4. **RPC:** Write one or more NDJSON lines (many for streaming)
5. **Close:** Stream dropped after RPC completes; no persistent session

Platform note: UDS requires Linux, macOS, or WSL2. Native Windows MSVC/GNU does not compile until Tokio exposes AF_UNIX on Windows.

## NDJSON framing

Defined in `ndjson.rs` and used across RPC/streaming:

- **Request:** one JSON object per line: `{"id", "method", "params"}`
- **Success:** `{"id", "result": {…}}`
- **Stream event:** `{"event":"model.stream","delta":"…"}` (no `id` on delta lines)
- **Error:** `{"id", "error":{"message":"…"}}`

Each written line ends with `\n`. Stream deltas use `write_json_line_flush` so Herdr sees tokens immediately.

## HTTP mapping (brief)

**Generate kind** — POST `{model?, prompt, temperature?, max_tokens?, stream}`

**ChatCompletions kind** — POST `{model?, messages:[{role:user, content:prompt}], temperature?, max_tokens?, stream}`

Optional fields omitted when not provided. Response parsing tries chat `choices[0].message.content`, then `text` / `completion` / `output` / `choices[0].text`.

## Related docs

- [README.md](README.md) — quick start
- [ARCHITECTURE.md](ARCHITECTURE.md) — module layout
- [TUI_USAGE.md](TUI_USAGE.md) — user-visible behavior
