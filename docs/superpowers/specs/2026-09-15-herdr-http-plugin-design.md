# Herdr HTTP Model Plugin — Design Spec

**Date:** 2026-09-15  
**Status:** Approved for planning  
**Crate / binary:** `herdr-http-plugin`  
**Approach:** Spec-faithful single Tokio binary (no core/lib split)

## 1. Goal

A complete Herdr plugin written entirely in Rust that speaks the Herdr plugin socket protocol (NDJSON over Unix domain sockets) and forwards `model.*` RPCs to a user-provided OpenAI-compatible (or simple `/generate`) HTTP AI endpoint.

It must:

- Stream partial output to Herdr in real time
- Fall back to non-streaming when streaming is unsupported
- Auto-disable model tools when the backend is missing or invalid
- Become Herdr’s active model provider when the backend is valid
- Require no harness (Pi, OpenCode, etc.)
- Run on Windows, Linux, and macOS via AF_UNIX
- Ship a full documentation suite under `docs/`

## 2. Non-goals

- MCP / Pi / OpenCode / other harness integration
- Named pipes or TCP fallback when UDS bind fails
- Multi-turn chat history beyond a single `params.prompt`
- Implementing Herdr itself or reverse-proxying arbitrary Herdr RPCs unrelated to model generation

## 3. Platform & transport

| Decision | Choice |
|----------|--------|
| Platforms | Windows, Linux, macOS |
| Transport | Unix domain sockets (`tokio::net::UnixListener`) everywhere AF_UNIX is available |
| Bind failure | Exit with a clear error — no TCP / named-pipe fallback |
| Socket path | From env `HERDR_PLUGIN_SOCKET` (required) |

Connection lifecycle: accept → read one NDJSON RPC (streaming may write many lines) → close socket after the final response for that RPC.

## 4. Dependencies

- `tokio` — async runtime (net, io, macros)
- `serde` + `serde_json` — JSON
- `reqwest` — HTTP with streaming (`bytes_stream`)
- `tracing` (+ `tracing-subscriber`) — logging
- `anyhow` — error handling
- `futures-util` — stream helpers as needed

## 5. Project structure

```
Cargo.toml
src/
  main.rs
  socket.rs
  rpc.rs
  http_client.rs
  model.rs
  ndjson.rs
  validation.rs
  streaming.rs
docs/
  README.md
  ARCHITECTURE.md
  TUI_USAGE.md
  FEATURES.md
  DIAGRAMS.md
  superpowers/specs/2026-09-15-herdr-http-plugin-design.md
```

Root README may briefly point at `docs/README.md` and the run commands.

## 6. Architecture

```
Herdr ──UDS/NDJSON──► socket ──► rpc ──► model / streaming
                              │              │
                              │              ▼
                              │         http_client ──HTTP──► MODEL_BASE_URL
                              │              ▲
                              └─ validation ─┘ (startup BackendState)
```

| Module | Responsibility |
|--------|----------------|
| `main` | Env, tracing init, validate backend, run listener loop |
| `validation` | Trim `MODEL_BASE_URL`, probe endpoints, produce `BackendState` |
| `socket` | Bind `UnixListener`, accept, spawn per-connection handler |
| `ndjson` | Read line → `Value` / write line + flush |
| `rpc` | Parse `{id, method, params}`, dispatch, encode success/error |
| `model` | Map Herdr params → HTTP JSON by endpoint kind; parse completions |
| `http_client` | Shared `reqwest::Client`, optional Bearer auth |
| `streaming` | Stream POST, SSE/chunk parse, delta emit, non-stream fallback |

### Backend state

```text
BackendState::Disabled { reason }
BackendState::Active { endpoint_url, kind: Generate | ChatCompletions }
```

- **Disabled:** Still bind and serve the socket. `model.info` reports no generation tools / disabled capabilities so Herdr ignores this plugin for model generation. `model.generate` / `model.stream_generate` return a structured error citing `reason`.
- **Active:** Full tool registration via `model.info`; generate and stream paths enabled.

## 7. Configuration (env)

| Variable | Required | Behavior |
|----------|----------|----------|
| `HERDR_PLUGIN_SOCKET` | Yes | UDS path; missing or bind failure → process exit |
| `MODEL_BASE_URL` | For Active | Trimmed; empty/whitespace → Disabled |
| `MODEL_API_KEY` | No | If set, `Authorization: Bearer <key>` on probes and generates |
| `RUST_LOG` | No | Standard tracing filter |

Example:

```bash
export MODEL_BASE_URL="http://localhost:8000/v1"
export MODEL_API_KEY="..."   # optional
herdr --plugin ./target/debug/herdr-http-plugin
```

## 8. Startup validation (auto-disable)

1. Read and trim `MODEL_BASE_URL`.
2. If empty → `Disabled` + warning log; do not treat as Active.
3. Otherwise:
   - `GET {MODEL_BASE_URL}/generate`
   - If that fails as “missing” (notably **404**) or is unreachable in a way that does not establish an endpoint, try `GET {MODEL_BASE_URL}/chat/completions`
4. If both fail (both 404, or both unreachable / connection errors) → log warning → `Disabled`.
5. If either succeeds → store that full endpoint URL and kind (`Generate` or `ChatCompletions`).

**Success criteria for a probe:** TCP/HTTP round-trip completed and status is not “endpoint missing.” Prefer 2xx. Connection failures and 404 count as failure for that candidate. A non-404 response that indicates the route exists may be accepted so local servers that reject GET-without-body still pass validation when the path is real — implementation should prefer: **404 = try next; connection error = fail that candidate; other statuses = accept that candidate** unless clearly “not found.”

Exact probe acceptance rule (normative):

- Connection / DNS / timeout → candidate fails
- HTTP **404** → candidate fails
- Any other HTTP status → candidate **succeeds** (path exists; POST will carry the real body)

## 9. Herdr RPC protocol

### Framing

- One JSON object per line (NDJSON)
- Responses written as NDJSON lines with flush after each stream delta

### Request shape

```json
{"id": <any>, "method": "<string>", "params": { ... }}
```

### Success / stream / error

- Success: `{"id": <id>, "result": { ... }}`
- Stream partial: `{"event":"model.stream","delta":"<partial text>"}`
- Stream final: `{"id": <id>, "result":{"completion":"<full text>"}}`
- Error: `{"id": <id>, "error":{"message":"<string>"}}` (and optional code if useful)

### Methods

| Method | Active | Disabled |
|--------|--------|----------|
| `model.info` | Advertise plugin name, endpoint kind, `streaming: true`, generation tools available | Advertise tools disabled / empty generation tools |
| `model.generate` | Non-stream HTTP → `{completion}` | Error with disable reason |
| `model.stream_generate` | Stream HTTP → deltas + final `{completion}`; fallback if needed | Error with disable reason |
| other | Error: unknown method | Error: unknown method |

Close the socket after each RPC completes (after final result or error).

## 10. HTTP mapping

### Request bodies

Shared Herdr params (best-effort): `model`, `prompt`, `temperature`, `max_tokens`.

**Kind = Generate** — POST validated URL:

```json
{
  "model": "...",
  "prompt": "...",
  "temperature": ...,
  "max_tokens": ...,
  "stream": false
}
```

**Kind = ChatCompletions** — POST validated URL:

```json
{
  "model": "...",
  "messages": [{"role": "user", "content": "<prompt>"}],
  "temperature": ...,
  "max_tokens": ...,
  "stream": false
}
```

For streaming, same bodies with `"stream": true`.

Omit null/absent optional fields rather than sending JSON null when params lack them.

### Non-stream response parsing

Best-effort extraction of completion text:

1. Chat-style: `choices[0].message.content`
2. Generate-style: `text` / `completion` / `output` / `choices[0].text`
3. Else: stringify a clear error if no known field exists

Return to Herdr: `{"id", "result":{"completion":"..."}}`.

### Streaming

1. POST with `stream: true`, consume `reqwest` `bytes_stream()`.
2. Decode UTF-8 (lossy buffer across chunk boundaries as needed).
3. Parse:
   - SSE: lines starting with `data:`; skip `[DONE]`; parse JSON; take `choices[0].delta.content` (and similar fallbacks)
   - Else NDJSON / raw text chunks as partial deltas when JSON parse fails but text is present
4. For each non-empty delta, write `{"event":"model.stream","delta":"..."}` and flush.
5. Accumulate full text; final line `{"id", "result":{"completion":"<full>"}}`.

### Streaming fallback

If streaming is unsupported or fails in a recoverable way (e.g. 400/405, explicit stream-not-supported body, empty unusable stream, non-parseable stream with error status):

1. Log fallback at warn/info
2. Perform one non-streaming POST (`stream: false`)
3. Return a single final `{completion}` result (no partial deltas)

## 11. Documentation suite

| Doc | Contents |
|-----|----------|
| `docs/README.md` | Overview, install, `MODEL_BASE_URL` / API key, Herdr discovery, auto-disable, streaming, fallback |
| `docs/ARCHITECTURE.md` | Components, data flow, Mermaid sequence, ASCII diagrams |
| `docs/TUI_USAGE.md` | Behavior in Herdr TUI, streaming/errors/auto-disable appearance, interaction diagrams |
| `docs/FEATURES.md` | Auto-disable, streaming, fallback, validation, tool registration, socket lifecycle, NDJSON |
| `docs/DIAGRAMS.md` | Mermaid + ASCII + picture-style diagrams for architecture, streams, decision tree, TUI |

## 12. Testing

Unit tests (no live Herdr required):

- NDJSON read/write helpers
- URL join / base trim
- Generate vs chat body mapping
- SSE delta extraction
- Disabled-state `model.info` / generate error shape

Optional manual check: point at local `/v1` (e.g. Cursor-API / LM Studio) under Herdr.

## 13. Error handling & logging

- Use `anyhow` at boundaries; `tracing` for info/warn/error
- Never panic on bad RPC JSON — return error NDJSON when `id` is known; log and close if unparseable
- UDS bind failure → exit non-zero with explicit message

## 14. Success criteria

- [ ] Builds on Windows / Linux / macOS targets with UDS
- [ ] Validates endpoint and auto-disables cleanly when invalid
- [ ] `model.generate` returns completion from HTTP backend
- [ ] `model.stream_generate` emits deltas + final completion
- [ ] Falls back to non-stream when streaming unsupported
- [ ] Docs suite complete under `docs/` with diagrams
- [ ] Runnable via `herdr --plugin ./target/debug/herdr-http-plugin`
