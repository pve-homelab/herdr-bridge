# Herdr HTTP Model Plugin Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a cross-platform Rust Herdr plugin (`herdr-http-plugin`) that speaks NDJSON over Unix domain sockets and forwards `model.*` RPCs to a user HTTP `/v1` model server with streaming, fallback, and auto-disable.

**Architecture:** Single Tokio binary. Startup probes `MODEL_BASE_URL` (`/generate` then `/chat/completions`) into `BackendState`. Each Herdr connection is one NDJSON RPC; `rpc` dispatches to `model` / `streaming` via shared `reqwest` client. Disabled backends still serve the socket but advertise no generation tools.

**Tech Stack:** Rust 2021, tokio (rt-multi-thread, macros, net, io-util), serde/serde_json, reqwest (json, stream), futures-util, tracing/tracing-subscriber, anyhow; wiremock for HTTP unit tests.

**Spec:** `docs/superpowers/specs/2026-09-15-herdr-http-plugin-design.md`

## Global Constraints

- Binary/crate name: `herdr-http-plugin`
- Language: Rust only; modules exactly: `main`, `socket`, `rpc`, `http_client`, `model`, `ndjson`, `validation`, `streaming`
- Transport: `tokio::net::UnixListener` (AF_UNIX on Windows/Linux/macOS); bind failure → exit (no TCP/named-pipe fallback)
- Env: `HERDR_PLUGIN_SOCKET` required; `MODEL_BASE_URL` for Active; optional `MODEL_API_KEY` → `Authorization: Bearer …`
- Probe rule: connection/DNS/timeout fail; HTTP 404 fail; any other HTTP status succeed
- Body by kind: Generate uses `prompt`; ChatCompletions uses `messages:[{role:user,content}]`
- Stream events: `{"event":"model.stream","delta":…}` then final `{"id", "result":{"completion":…}}`
- Docs required: `docs/README.md`, `ARCHITECTURE.md`, `TUI_USAGE.md`, `FEATURES.md`, `DIAGRAMS.md`
- No harnesses (Pi/OpenCode/MCP); no multi-turn history beyond single `prompt`

## File Structure

| Path | Responsibility |
|------|----------------|
| `Cargo.toml` | Package + deps |
| `src/main.rs` | Tracing, env, validate, run socket loop |
| `src/ndjson.rs` | Line-framed JSON read/write + flush |
| `src/validation.rs` | `BackendState`, URL join, endpoint probe |
| `src/http_client.rs` | Shared client + Bearer header helper |
| `src/model.rs` | Params → body; completion text extract; non-stream generate |
| `src/streaming.rs` | SSE/chunk parse; stream generate + fallback |
| `src/rpc.rs` | Method dispatch + response helpers |
| `src/socket.rs` | UnixListener bind/accept/per-conn |
| `README.md` | Short pointer + run commands |
| `docs/*.md` | Full documentation suite |

---

### Task 1: Scaffold crate and module stubs

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/ndjson.rs`, `src/validation.rs`, `src/http_client.rs`, `src/model.rs`, `src/streaming.rs`, `src/rpc.rs`, `src/socket.rs`
- Create: `README.md` (stub pointing to docs)

**Interfaces:**
- Consumes: (none)
- Produces: compilable empty modules; package name `herdr-http-plugin`

- [ ] **Step 1: Create `Cargo.toml`**

```toml
[package]
name = "herdr-http-plugin"
version = "0.1.0"
edition = "2021"
description = "Herdr plugin that forwards model.* RPCs to an HTTP AI endpoint"
license = "MIT"

[dependencies]
anyhow = "1"
futures-util = "0.3"
reqwest = { version = "0.12", default-features = false, features = ["json", "stream", "rustls-tls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "net", "io-util", "sync", "time"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

[dev-dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "macros", "net", "io-util", "sync", "time"] }
wiremock = "0.6"
```

- [ ] **Step 2: Create stub modules**

Each of `ndjson.rs`, `validation.rs`, `http_client.rs`, `model.rs`, `streaming.rs`, `rpc.rs`, `socket.rs` starts as:

```rust
//! Module docs filled in later tasks.
```

`src/main.rs`:

```rust
mod http_client;
mod model;
mod ndjson;
mod rpc;
mod socket;
mod streaming;
mod validation;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    tracing::info!("herdr-http-plugin scaffold; wire-up in later tasks");
    Ok(())
}
```

- [ ] **Step 3: Create stub `README.md`**

```markdown
# herdr-http-plugin

Herdr plugin: Unix-socket NDJSON `model.*` RPCs → HTTP `/v1` model server.

See [docs/README.md](docs/README.md) for install, config, streaming, and auto-disable.

```bash
export MODEL_BASE_URL="http://localhost:8000/v1"
herdr --plugin ./target/debug/herdr-http-plugin
```
```

- [ ] **Step 4: Verify build**

Run: `cargo build`
Expected: success (warnings OK)

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml src README.md
git commit -m "chore: scaffold herdr-http-plugin crate"
```

---

### Task 2: NDJSON framing

**Files:**
- Modify: `src/ndjson.rs`
- Test: unit tests inside `src/ndjson.rs` (`#[cfg(test)]`)

**Interfaces:**
- Consumes: `tokio::io::{AsyncBufReadExt, AsyncWriteExt}`
- Produces:
  - `pub async fn read_json_line<R: AsyncBufReadExt + Unpin>(reader: &mut R) -> anyhow::Result<Option<serde_json::Value>>`
  - `pub async fn write_json_line<W: AsyncWriteExt + Unpin>(writer: &mut W, value: &serde_json::Value) -> anyhow::Result<()>`
  - `pub async fn write_json_line_flush<W: AsyncWriteExt + Unpin>(writer: &mut W, value: &serde_json::Value) -> anyhow::Result<()>`

- [ ] **Step 1: Write failing tests in `src/ndjson.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;

    #[tokio::test]
    async fn read_json_line_parses_object() {
        let mut reader = BufReader::new(b"{\"a\":1}\n".as_slice());
        let v = read_json_line(&mut reader).await.unwrap().unwrap();
        assert_eq!(v["a"], 1);
    }

    #[tokio::test]
    async fn write_json_line_appends_newline() {
        let mut buf = Vec::new();
        write_json_line(&mut buf, &serde_json::json!({"ok": true}))
            .await
            .unwrap();
        assert_eq!(std::str::from_utf8(&buf).unwrap(), "{\"ok\":true}\n");
    }

    #[tokio::test]
    async fn read_json_line_eof_returns_none() {
        let mut reader = BufReader::new(b"".as_slice());
        assert!(read_json_line(&mut reader).await.unwrap().is_none());
    }
}
```

- [ ] **Step 2: Run tests — expect FAIL**

Run: `cargo test ndjson -- --nocapture`
Expected: FAIL (functions not found / not defined)

- [ ] **Step 3: Implement `src/ndjson.rs`**

```rust
//! NDJSON framing: one JSON object per line.

use anyhow::{Context, Result};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

pub async fn read_json_line<R>(reader: &mut R) -> Result<Option<Value>>
where
    R: AsyncBufReadExt + Unpin,
{
    let mut line = String::new();
    let n = reader
        .read_line(&mut line)
        .await
        .context("reading NDJSON line")?;
    if n == 0 {
        return Ok(None);
    }
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let value = serde_json::from_str(trimmed).context("parsing NDJSON line")?;
    Ok(Some(value))
}

pub async fn write_json_line<W>(writer: &mut W, value: &Value) -> Result<()>
where
    W: AsyncWriteExt + Unpin,
{
    let mut bytes = serde_json::to_vec(value).context("serializing NDJSON")?;
    bytes.push(b'\n');
    writer.write_all(&bytes).await.context("writing NDJSON")?;
    Ok(())
}

pub async fn write_json_line_flush<W>(writer: &mut W, value: &Value) -> Result<()>
where
    W: AsyncWriteExt + Unpin,
{
    write_json_line(writer, value).await?;
    writer.flush().await.context("flushing NDJSON")?;
    Ok(())
}
```

Keep the `#[cfg(test)]` module from Step 1 in the same file.

- [ ] **Step 4: Run tests — expect PASS**

Run: `cargo test ndjson -- --nocapture`
Expected: all three tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/ndjson.rs
git commit -m "feat: add NDJSON line framing helpers"
```

---

### Task 3: Validation types, URL join, and probe logic

**Files:**
- Modify: `src/validation.rs`
- Test: `src/validation.rs` unit tests + wiremock probe tests

**Interfaces:**
- Consumes: `reqwest::Client`
- Produces:
  - `pub enum EndpointKind { Generate, ChatCompletions }`
  - `pub enum BackendState { Disabled { reason: String }, Active { endpoint_url: String, kind: EndpointKind } }`
  - `pub fn trim_base_url(raw: &str) -> String`
  - `pub fn join_endpoint(base: &str, path: &str) -> String`
  - `pub async fn validate_backend(client: &reqwest::Client, base_url: &str) -> BackendState`

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn trim_base_url_strips_whitespace_and_trailing_slash() {
        assert_eq!(trim_base_url("  http://x/v1/  "), "http://x/v1");
    }

    #[test]
    fn join_endpoint_avoids_double_slash() {
        assert_eq!(
            join_endpoint("http://x/v1", "/generate"),
            "http://x/v1/generate"
        );
        assert_eq!(
            join_endpoint("http://x/v1/", "chat/completions"),
            "http://x/v1/chat/completions"
        );
    }

    #[tokio::test]
    async fn empty_base_disables() {
        let client = reqwest::Client::new();
        match validate_backend(&client, "   ").await {
            BackendState::Disabled { reason } => assert!(!reason.is_empty()),
            _ => panic!("expected Disabled"),
        }
    }

    #[tokio::test]
    async fn prefers_generate_when_present() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/generate"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        match validate_backend(&client, &server.uri()).await {
            BackendState::Active {
                endpoint_url,
                kind: EndpointKind::Generate,
            } => assert_eq!(endpoint_url, format!("{}/generate", server.uri())),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn falls_back_to_chat_completions_on_generate_404() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/generate"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(405))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        match validate_backend(&client, &server.uri()).await {
            BackendState::Active {
                kind: EndpointKind::ChatCompletions,
                ..
            } => {}
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn both_404_disables() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/generate"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        assert!(matches!(
            validate_backend(&client, &server.uri()).await,
            BackendState::Disabled { .. }
        ));
    }
}
```

- [ ] **Step 2: Run tests — expect FAIL**

Run: `cargo test validation -- --nocapture`
Expected: FAIL (missing types/fns)

- [ ] **Step 3: Implement `src/validation.rs`**

```rust
//! Startup endpoint validation and BackendState.

use reqwest::Client;
use tracing::warn;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndpointKind {
    Generate,
    ChatCompletions,
}

#[derive(Debug, Clone)]
pub enum BackendState {
    Disabled { reason: String },
    Active {
        endpoint_url: String,
        kind: EndpointKind,
    },
}

pub fn trim_base_url(raw: &str) -> String {
    raw.trim().trim_end_matches('/').to_string()
}

pub fn join_endpoint(base: &str, path: &str) -> String {
    let base = base.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    format!("{base}/{path}")
}

async fn probe(client: &Client, url: &str) -> Result<(), ProbeFail> {
    match client.get(url).send().await {
        Ok(resp) if resp.status().as_u16() == 404 => Err(ProbeFail::NotFound),
        Ok(_) => Ok(()),
        Err(_) => Err(ProbeFail::Unreachable),
    }
}

#[derive(Debug)]
enum ProbeFail {
    NotFound,
    Unreachable,
}

pub async fn validate_backend(client: &Client, base_url: &str) -> BackendState {
    let base = trim_base_url(base_url);
    if base.is_empty() {
        warn!("MODEL_BASE_URL empty; model tools disabled");
        return BackendState::Disabled {
            reason: "MODEL_BASE_URL is empty".into(),
        };
    }

    let generate_url = join_endpoint(&base, "generate");
    match probe(client, &generate_url).await {
        Ok(()) => {
            return BackendState::Active {
                endpoint_url: generate_url,
                kind: EndpointKind::Generate,
            };
        }
        Err(ProbeFail::NotFound) | Err(ProbeFail::Unreachable) => {}
    }

    let chat_url = join_endpoint(&base, "chat/completions");
    match probe(client, &chat_url).await {
        Ok(()) => BackendState::Active {
            endpoint_url: chat_url,
            kind: EndpointKind::ChatCompletions,
        },
        Err(_) => {
            warn!("no valid model endpoint under {base}; model tools disabled");
            BackendState::Disabled {
                reason: format!("no valid endpoint under {base}"),
            }
        }
    }
}
```

Note: when `/generate` is **Unreachable**, still try `/chat/completions` (both must fail to Disable). When `/generate` is NotFound, try chat. Implementation above tries chat after either generate failure — matches spec “try chat if generate fails.”

- [ ] **Step 4: Run tests — expect PASS**

Run: `cargo test validation -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/validation.rs Cargo.toml
git commit -m "feat: validate MODEL_BASE_URL and auto-disable backend"
```

---

### Task 4: HTTP client helper

**Files:**
- Modify: `src/http_client.rs`

**Interfaces:**
- Consumes: `MODEL_API_KEY` env (read by caller or helper)
- Produces:
  - `pub fn build_client() -> anyhow::Result<reqwest::Client>`
  - `pub fn apply_auth(req: reqwest::RequestBuilder, api_key: Option<&str>) -> reqwest::RequestBuilder`

- [ ] **Step 1: Write failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_auth_sets_bearer_when_key_present() {
        let client = reqwest::Client::new();
        let req = apply_auth(client.get("http://example.com"), Some("secret"))
            .build()
            .unwrap();
        let auth = req.headers().get(reqwest::header::AUTHORIZATION).unwrap();
        assert_eq!(auth.to_str().unwrap(), "Bearer secret");
    }

    #[test]
    fn apply_auth_skips_when_absent() {
        let client = reqwest::Client::new();
        let req = apply_auth(client.get("http://example.com"), None)
            .build()
            .unwrap();
        assert!(req.headers().get(reqwest::header::AUTHORIZATION).is_none());
    }
}
```

- [ ] **Step 2: Run — expect FAIL**

Run: `cargo test http_client -- --nocapture`
Expected: FAIL

- [ ] **Step 3: Implement**

```rust
//! Shared reqwest client and auth helpers.

use anyhow::{Context, Result};
use reqwest::{Client, RequestBuilder};

pub fn build_client() -> Result<Client> {
    Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .context("building reqwest client")
}

pub fn apply_auth(req: RequestBuilder, api_key: Option<&str>) -> RequestBuilder {
    match api_key {
        Some(key) if !key.is_empty() => req.bearer_auth(key),
        _ => req,
    }
}

pub fn read_api_key() -> Option<String> {
    std::env::var("MODEL_API_KEY")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
```

- [ ] **Step 4: Run — expect PASS**

Run: `cargo test http_client -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/http_client.rs
git commit -m "feat: add HTTP client and optional Bearer auth"
```

---

### Task 5: Model request/response mapping (non-stream)

**Files:**
- Modify: `src/model.rs`
- Test: `src/model.rs`

**Interfaces:**
- Consumes: `EndpointKind`, `http_client::apply_auth`
- Produces:
  - `pub struct GenerateParams { pub model: Option<String>, pub prompt: String, pub temperature: Option<f64>, pub max_tokens: Option<u32> }`
  - `pub fn params_from_value(params: &serde_json::Value) -> anyhow::Result<GenerateParams>`
  - `pub fn build_request_body(kind: &EndpointKind, params: &GenerateParams, stream: bool) -> serde_json::Value`
  - `pub fn extract_completion(body: &serde_json::Value) -> anyhow::Result<String>`
  - `pub async fn generate_completion(client, endpoint_url, kind, params, api_key) -> anyhow::Result<String>`

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::validation::EndpointKind;

    #[test]
    fn build_generate_body_uses_prompt() {
        let params = GenerateParams {
            model: Some("m".into()),
            prompt: "hi".into(),
            temperature: Some(0.2),
            max_tokens: Some(16),
        };
        let body = build_request_body(&EndpointKind::Generate, &params, false);
        assert_eq!(body["prompt"], "hi");
        assert_eq!(body["stream"], false);
        assert!(body.get("messages").is_none());
    }

    #[test]
    fn build_chat_body_uses_messages() {
        let params = GenerateParams {
            model: None,
            prompt: "hi".into(),
            temperature: None,
            max_tokens: None,
        };
        let body = build_request_body(&EndpointKind::ChatCompletions, &params, true);
        assert_eq!(body["messages"][0]["content"], "hi");
        assert_eq!(body["stream"], true);
        assert!(body.get("prompt").is_none());
        assert!(body.get("temperature").is_none());
    }

    #[test]
    fn extract_chat_completion() {
        let v = serde_json::json!({
            "choices": [{"message": {"content": "hello"}}]
        });
        assert_eq!(extract_completion(&v).unwrap(), "hello");
    }

    #[test]
    fn extract_generate_text_field() {
        let v = serde_json::json!({"text": "yo"});
        assert_eq!(extract_completion(&v).unwrap(), "yo");
    }
}
```

- [ ] **Step 2: Run — expect FAIL**

Run: `cargo test model -- --nocapture`
Expected: FAIL

- [ ] **Step 3: Implement mapping + `generate_completion`**

```rust
//! Map Herdr params to HTTP bodies and parse completions.

use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde_json::{json, Value};

use crate::http_client::apply_auth;
use crate::validation::EndpointKind;

#[derive(Debug, Clone)]
pub struct GenerateParams {
    pub model: Option<String>,
    pub prompt: String,
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
}

pub fn params_from_value(params: &Value) -> Result<GenerateParams> {
    let prompt = params
        .get("prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if prompt.is_empty() {
        bail!("params.prompt is required");
    }
    Ok(GenerateParams {
        model: params
            .get("model")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        prompt,
        temperature: params.get("temperature").and_then(|v| v.as_f64()),
        max_tokens: params
            .get("max_tokens")
            .and_then(|v| v.as_u64())
            .map(|n| n as u32),
    })
}

pub fn build_request_body(kind: &EndpointKind, params: &GenerateParams, stream: bool) -> Value {
    let mut body = match kind {
        EndpointKind::Generate => json!({
            "prompt": params.prompt,
            "stream": stream,
        }),
        EndpointKind::ChatCompletions => json!({
            "messages": [{"role": "user", "content": params.prompt}],
            "stream": stream,
        }),
    };
    if let Some(model) = &params.model {
        body["model"] = json!(model);
    }
    if let Some(t) = params.temperature {
        body["temperature"] = json!(t);
    }
    if let Some(m) = params.max_tokens {
        body["max_tokens"] = json!(m);
    }
    body
}

pub fn extract_completion(body: &Value) -> Result<String> {
    if let Some(s) = body
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
    {
        return Ok(s.to_string());
    }
    for key in ["text", "completion", "output"] {
        if let Some(s) = body.get(key).and_then(|v| v.as_str()) {
            return Ok(s.to_string());
        }
    }
    if let Some(s) = body
        .pointer("/choices/0/text")
        .and_then(|v| v.as_str())
    {
        return Ok(s.to_string());
    }
    bail!("could not extract completion from response");
}

pub async fn generate_completion(
    client: &Client,
    endpoint_url: &str,
    kind: &EndpointKind,
    params: &GenerateParams,
    api_key: Option<&str>,
) -> Result<String> {
    let body = build_request_body(kind, params, false);
    let req = apply_auth(client.post(endpoint_url).json(&body), api_key);
    let resp = req.send().await.context("POST generate")?;
    let status = resp.status();
    let value: Value = resp.json().await.context("decode generate JSON")?;
    if !status.is_success() {
        bail!("generate HTTP {status}: {value}");
    }
    extract_completion(&value)
}
```

- [ ] **Step 4: Run — expect PASS**

Run: `cargo test model -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/model.rs
git commit -m "feat: map Herdr params to HTTP generate/chat bodies"
```

---

### Task 6: Streaming parse + stream_generate with fallback

**Files:**
- Modify: `src/streaming.rs`
- Test: `src/streaming.rs`

**Interfaces:**
- Consumes: `model::{build_request_body, extract_completion, generate_completion, GenerateParams}`, `http_client::apply_auth`, `ndjson::write_json_line_flush`, `EndpointKind`
- Produces:
  - `pub fn extract_sse_delta(data_line: &str) -> Option<String>`
  - `pub fn push_stream_buffer(buffer: &mut String, chunk: &str) -> Vec<String>` — returns deltas drained from complete SSE `data:` lines
  - `pub async fn stream_generate<W>(..., writer: &mut W, id: &Value) -> Result<()>` where stream events + final result are written; on stream failure calls non-stream fallback and writes only final result

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_sse_delta_from_chat_chunk() {
        let data = r#"{"choices":[{"delta":{"content":"Hel"}}]}"#;
        assert_eq!(extract_sse_delta(data).as_deref(), Some("Hel"));
    }

    #[test]
    fn extract_sse_delta_skips_done() {
        assert!(extract_sse_delta("[DONE]").is_none());
    }

    #[test]
    fn push_stream_buffer_splits_sse_lines() {
        let mut buf = String::new();
        let d1 = push_stream_buffer(&mut buf, "data: {\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\n");
        assert_eq!(d1, vec!["a".to_string()]);
        let d2 = push_stream_buffer(
            &mut buf,
            "data: {\"choices\":[{\"delta\":{\"content\":\"b\"}}]}\n\ndata: [DONE]\n",
        );
        assert_eq!(d2, vec!["b".to_string()]);
    }
}
```

- [ ] **Step 2: Run — expect FAIL**

Run: `cargo test streaming -- --nocapture`
Expected: FAIL

- [ ] **Step 3: Implement SSE helpers + `stream_generate`**

```rust
//! Streaming HTTP consumption, SSE parsing, and non-stream fallback.

use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tracing::warn;

use crate::http_client::apply_auth;
use crate::model::{
    build_request_body, extract_completion, generate_completion, GenerateParams,
};
use crate::ndjson::write_json_line_flush;
use crate::validation::EndpointKind;

pub fn extract_sse_delta(data: &str) -> Option<String> {
    let data = data.trim();
    if data.is_empty() || data == "[DONE]" {
        return None;
    }
    let v: Value = serde_json::from_str(data).ok()?;
    if let Some(s) = v
        .pointer("/choices/0/delta/content")
        .and_then(|x| x.as_str())
    {
        if !s.is_empty() {
            return Some(s.to_string());
        }
    }
    if let Some(s) = v.get("text").and_then(|x| x.as_str()) {
        if !s.is_empty() {
            return Some(s.to_string());
        }
    }
    None
}

pub fn push_stream_buffer(buffer: &mut String, chunk: &str) -> Vec<String> {
    buffer.push_str(chunk);
    let mut deltas = Vec::new();
    while let Some(pos) = buffer.find('\n') {
        let line = buffer[..pos].trim_end_matches('\r').to_string();
        *buffer = buffer[pos + 1..].to_string();
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("data:") {
            if let Some(d) = extract_sse_delta(rest.trim()) {
                deltas.push(d);
            }
        } else if !line.is_empty() {
            // NDJSON / raw fallback line
            if let Ok(v) = serde_json::from_str::<Value>(line) {
                if let Some(d) = v
                    .pointer("/choices/0/delta/content")
                    .and_then(|x| x.as_str())
                {
                    if !d.is_empty() {
                        deltas.push(d.to_string());
                    }
                }
            } else {
                deltas.push(line.to_string());
            }
        }
    }
    deltas
}

fn should_fallback_status(status: reqwest::StatusCode) -> bool {
    status.as_u16() == 400 || status.as_u16() == 405 || status.as_u16() == 415
}

pub async fn stream_generate<W>(
    client: &Client,
    endpoint_url: &str,
    kind: &EndpointKind,
    params: &GenerateParams,
    api_key: Option<&str>,
    id: &Value,
    writer: &mut W,
) -> Result<()>
where
    W: AsyncWriteExt + Unpin,
{
    let body = build_request_body(kind, params, true);
    let req = apply_auth(client.post(endpoint_url).json(&body), api_key);
    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            warn!("stream request failed ({e}); falling back to non-stream");
            let text = generate_completion(client, endpoint_url, kind, params, api_key).await?;
            write_json_line_flush(writer, &json!({"id": id, "result": {"completion": text}}))
                .await?;
            return Ok(());
        }
    };

    if should_fallback_status(resp.status()) || !resp.status().is_success() {
        warn!(
            "streaming unsupported or failed (HTTP {}); falling back",
            resp.status()
        );
        let text = generate_completion(client, endpoint_url, kind, params, api_key).await?;
        write_json_line_flush(writer, &json!({"id": id, "result": {"completion": text}}))
            .await?;
        return Ok(());
    }

    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    let mut full = String::new();
    let mut saw_delta = false;

    while let Some(item) = stream.next().await {
        let bytes = item.context("reading stream chunk")?;
        let chunk = String::from_utf8_lossy(&bytes);
        for delta in push_stream_buffer(&mut buf, &chunk) {
            saw_delta = true;
            full.push_str(&delta);
            write_json_line_flush(
                writer,
                &json!({"event": "model.stream", "delta": delta}),
            )
            .await?;
        }
    }

    if !saw_delta {
        warn!("empty stream; falling back to non-stream");
        let text = generate_completion(client, endpoint_url, kind, params, api_key).await?;
        write_json_line_flush(writer, &json!({"id": id, "result": {"completion": text}}))
            .await?;
        return Ok(());
    }

    write_json_line_flush(
        writer,
        &json!({"id": id, "result": {"completion": full}}),
    )
    .await?;
    Ok(())
}
```

Remove unused `extract_completion` / `bail` imports if the compiler warns — keep only what is used.

- [ ] **Step 4: Run — expect PASS**

Run: `cargo test streaming -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/streaming.rs
git commit -m "feat: stream SSE deltas with non-stream fallback"
```

---

### Task 7: RPC dispatch (including disabled tools)

**Files:**
- Modify: `src/rpc.rs`
- Test: `src/rpc.rs`

**Interfaces:**
- Consumes: `BackendState`, `model::*`, `streaming::stream_generate`, `ndjson::*`
- Produces:
  - `pub async fn handle_rpc<W>(state, client, api_key, request: Value, writer: &mut W) -> Result<()>`
  - `pub fn model_info_result(state: &BackendState) -> Value`

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::validation::BackendState;

    #[test]
    fn model_info_disabled_has_no_tools() {
        let state = BackendState::Disabled {
            reason: "empty".into(),
        };
        let info = model_info_result(&state);
        assert_eq!(info["tools"], json!([]));
        assert_eq!(info["generation_enabled"], false);
    }

    #[test]
    fn model_info_active_lists_tools() {
        let state = BackendState::Active {
            endpoint_url: "http://x/v1/generate".into(),
            kind: crate::validation::EndpointKind::Generate,
        };
        let info = model_info_result(&state);
        assert_eq!(info["generation_enabled"], true);
        assert!(info["tools"].as_array().unwrap().len() >= 2);
        assert_eq!(info["streaming"], true);
    }
}
```

- [ ] **Step 2: Run — expect FAIL**

Run: `cargo test rpc -- --nocapture`
Expected: FAIL

- [ ] **Step 3: Implement `src/rpc.rs`**

```rust
//! Herdr NDJSON RPC dispatch for model.* methods.

use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tracing::warn;

use crate::model::{generate_completion, params_from_value};
use crate::ndjson::write_json_line_flush;
use crate::streaming::stream_generate;
use crate::validation::{BackendState, EndpointKind};

pub fn model_info_result(state: &BackendState) -> Value {
    match state {
        BackendState::Disabled { reason } => json!({
            "name": "herdr-http-plugin",
            "generation_enabled": false,
            "streaming": false,
            "tools": [],
            "disabled_reason": reason,
        }),
        BackendState::Active {
            endpoint_url,
            kind,
        } => {
            let kind_str = match kind {
                EndpointKind::Generate => "generate",
                EndpointKind::ChatCompletions => "chat_completions",
            };
            json!({
                "name": "herdr-http-plugin",
                "generation_enabled": true,
                "streaming": true,
                "endpoint": endpoint_url,
                "endpoint_kind": kind_str,
                "tools": ["model.generate", "model.stream_generate", "model.info"],
            })
        }
    }
}

pub async fn handle_rpc<W>(
    state: &BackendState,
    client: &Client,
    api_key: Option<&str>,
    request: Value,
    writer: &mut W,
) -> Result<()>
where
    W: AsyncWriteExt + Unpin,
{
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request
        .get("method")
        .and_then(|m| m.as_str())
        .unwrap_or("");
    let params = request.get("params").cloned().unwrap_or_else(|| json!({}));

    match method {
        "model.info" => {
            write_json_line_flush(writer, &json!({"id": id, "result": model_info_result(state)}))
                .await?;
        }
        "model.generate" => match state {
            BackendState::Disabled { reason } => {
                write_json_line_flush(
                    writer,
                    &json!({"id": id, "error": {"message": format!("model tools disabled: {reason}")}}),
                )
                .await?;
            }
            BackendState::Active {
                endpoint_url,
                kind,
            } => match params_from_value(&params) {
                Ok(p) => match generate_completion(client, endpoint_url, kind, &p, api_key).await
                {
                    Ok(text) => {
                        write_json_line_flush(
                            writer,
                            &json!({"id": id, "result": {"completion": text}}),
                        )
                        .await?;
                    }
                    Err(e) => {
                        write_json_line_flush(
                            writer,
                            &json!({"id": id, "error": {"message": e.to_string()}}),
                        )
                        .await?;
                    }
                },
                Err(e) => {
                    write_json_line_flush(
                        writer,
                        &json!({"id": id, "error": {"message": e.to_string()}}),
                    )
                    .await?;
                }
            },
        },
        "model.stream_generate" => match state {
            BackendState::Disabled { reason } => {
                write_json_line_flush(
                    writer,
                    &json!({"id": id, "error": {"message": format!("model tools disabled: {reason}")}}),
                )
                .await?;
            }
            BackendState::Active {
                endpoint_url,
                kind,
            } => match params_from_value(&params) {
                Ok(p) => {
                    if let Err(e) = stream_generate(
                        client,
                        endpoint_url,
                        kind,
                        &p,
                        api_key,
                        &id,
                        writer,
                    )
                    .await
                    {
                        warn!("stream_generate failed: {e}");
                        write_json_line_flush(
                            writer,
                            &json!({"id": id, "error": {"message": e.to_string()}}),
                        )
                        .await?;
                    }
                }
                Err(e) => {
                    write_json_line_flush(
                        writer,
                        &json!({"id": id, "error": {"message": e.to_string()}}),
                    )
                    .await?;
                }
            },
        },
        other => {
            write_json_line_flush(
                writer,
                &json!({"id": id, "error": {"message": format!("unknown method: {other}")}}),
            )
            .await?;
        }
    }
    Ok(())
}
```

Fix imports: add `use serde_json::json` in tests; drop unused `Context` if unused.

- [ ] **Step 4: Run — expect PASS**

Run: `cargo test rpc -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/rpc.rs
git commit -m "feat: dispatch model.info/generate/stream_generate RPCs"
```

---

### Task 8: Socket listener + main wiring

**Files:**
- Modify: `src/socket.rs`
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: all modules
- Produces:
  - `pub async fn run_listener(path: &str, state: BackendState, client: Client, api_key: Option<String>) -> Result<()>`
  - `main` reads env, validates, runs listener; exits on missing socket path or bind failure

- [ ] **Step 1: Implement `src/socket.rs`**

```rust
//! Unix domain socket accept loop for Herdr plugin connections.

use std::sync::Arc;

use anyhow::{Context, Result};
use reqwest::Client;
use tokio::io::BufReader;
use tokio::net::UnixListener;
use tracing::{error, info};

use crate::ndjson::read_json_line;
use crate::rpc::handle_rpc;
use crate::validation::BackendState;

pub async fn run_listener(
    path: &str,
    state: BackendState,
    client: Client,
    api_key: Option<String>,
) -> Result<()> {
    // Remove stale socket file if present (Unix); ignore errors on Windows if unsupported.
    let _ = std::fs::remove_file(path);

    let listener = UnixListener::bind(path)
        .with_context(|| format!("binding HERDR_PLUGIN_SOCKET at {path}"))?;
    info!("listening on {path}");

    let state = Arc::new(state);
    let api_key = Arc::new(api_key);

    loop {
        let (stream, _) = listener.accept().await.context("accept")?;
        let state = Arc::clone(&state);
        let client = client.clone();
        let api_key = Arc::clone(&api_key);
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, &state, &client, api_key.as_deref()).await {
                error!("connection error: {e:#}");
            }
        });
    }
}

async fn handle_connection(
    stream: tokio::net::UnixStream,
    state: &BackendState,
    client: &Client,
    api_key: Option<&str>,
) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let request = match read_json_line(&mut reader).await? {
        Some(v) => v,
        None => return Ok(()),
    };
    handle_rpc(state, client, api_key, request, &mut writer).await?;
    // Socket closed when writer/reader dropped after RPC.
    Ok(())
}
```

- [ ] **Step 2: Wire `src/main.rs`**

```rust
mod http_client;
mod model;
mod ndjson;
mod rpc;
mod socket;
mod streaming;
mod validation;

use anyhow::{bail, Context, Result};
use tracing::info;

use crate::http_client::{build_client, read_api_key};
use crate::socket::run_listener;
use crate::validation::validate_backend;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let socket_path = match std::env::var("HERDR_PLUGIN_SOCKET") {
        Ok(p) if !p.trim().is_empty() => p,
        _ => bail!("HERDR_PLUGIN_SOCKET is required"),
    };

    let client = build_client()?;
    let api_key = read_api_key();
    let base = std::env::var("MODEL_BASE_URL").unwrap_or_default();
    let state = validate_backend(&client, &base).await;
    match &state {
        validation::BackendState::Active {
            endpoint_url,
            kind,
        } => info!(%endpoint_url, ?kind, "backend active"),
        validation::BackendState::Disabled { reason } => {
            info!(%reason, "backend disabled; serving socket without model tools")
        }
    }

    run_listener(&socket_path, state, client, api_key)
        .await
        .context("plugin listener")?;
    Ok(())
}
```

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: success. On Windows, if `UnixListener` fails to compile, bump tokio to latest 1.x and ensure Windows 10+ AF_UNIX; do **not** add TCP fallback.

- [ ] **Step 4: Manual socket smoke (optional if UDS available)**

```bash
# terminal A
export HERDR_PLUGIN_SOCKET=/tmp/herdr-http-plugin.sock
export MODEL_BASE_URL=   # empty → disabled
cargo run

# terminal B (Linux/macOS/WSL)
printf '%s\n' '{"id":1,"method":"model.info","params":{}}' | nc -U /tmp/herdr-http-plugin.sock
```

Expected: NDJSON with `generation_enabled: false`.

- [ ] **Step 5: Commit**

```bash
git add src/socket.rs src/main.rs
git commit -m "feat: bind HERDR_PLUGIN_SOCKET and serve model RPCs"
```

---

### Task 9: Documentation suite

**Files:**
- Create: `docs/README.md`
- Create: `docs/ARCHITECTURE.md`
- Create: `docs/TUI_USAGE.md`
- Create: `docs/FEATURES.md`
- Create: `docs/DIAGRAMS.md`
- Modify: `README.md` (ensure run commands match)

**Interfaces:**
- Consumes: finished behavior from Tasks 1–8
- Produces: complete docs per spec §11

- [ ] **Step 1: Write `docs/README.md`** covering overview, install (`cargo build`), `MODEL_BASE_URL` / `MODEL_API_KEY`, Herdr discovery via `--plugin` + `HERDR_PLUGIN_SOCKET`, auto-disable, streaming, fallback. Include:

```bash
export MODEL_BASE_URL="http://localhost:8000/v1"
herdr --plugin ./target/debug/herdr-http-plugin
```

- [ ] **Step 2: Write `docs/ARCHITECTURE.md`** with component table, ASCII data-flow, and at least one Mermaid sequence (Herdr → plugin → HTTP → deltas).

- [ ] **Step 3: Write `docs/TUI_USAGE.md`** describing streaming tokens appearing live, errors as plugin error messages, auto-disable as model provider not selected / tools missing; include ASCII TUI interaction diagram.

- [ ] **Step 4: Write `docs/FEATURES.md`** with sections: Auto-disable, Streaming, Fallback, Endpoint validation, Tool registration, Socket lifecycle, NDJSON framing.

- [ ] **Step 5: Write `docs/DIAGRAMS.md`** including Mermaid diagrams for: architecture, streaming flow, non-streaming flow, auto-disable decision tree, TUI rendering flow; plus ASCII plugin architecture, socket lifecycle, request/response flow, and one picture-style ASCII art diagram.

- [ ] **Step 6: Commit**

```bash
git add docs/README.md docs/ARCHITECTURE.md docs/TUI_USAGE.md docs/FEATURES.md docs/DIAGRAMS.md README.md
git commit -m "docs: add herdr-http-plugin documentation suite"
```

---

### Task 10: Full test pass and polish

**Files:**
- Modify: any file with clippy/test failures
- Optionally add wiremock test for `generate_completion` in `model.rs` if time permits

**Interfaces:**
- Consumes: full crate
- Produces: green `cargo test` and `cargo clippy` (warnings addressed for new code)

- [ ] **Step 1: Run full test suite**

Run: `cargo test`
Expected: all PASS

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: clean (fix any issues inline)

- [ ] **Step 3: Confirm binary name**

Run: `cargo build` then verify `target/debug/herdr-http-plugin` (or `.exe` on Windows) exists.

- [ ] **Step 4: Final commit if fixes landed**

```bash
git add -u
git commit -m "fix: address clippy and test polish for herdr-http-plugin"
```

(Skip empty commit if nothing changed.)

---

## Spec coverage checklist (self-review)

| Spec requirement | Task |
|------------------|------|
| Rust + listed deps | 1 |
| Module layout | 1 |
| NDJSON framing | 2 |
| UDS + `HERDR_PLUGIN_SOCKET` | 8 |
| `model.generate` / `stream_generate` / `info` | 7 |
| Close after RPC | 8 |
| Auto-disable / probe order | 3 |
| Optional `MODEL_API_KEY` | 4 |
| Body by endpoint kind | 5 |
| Streaming deltas + final | 6 |
| Non-stream fallback | 6 |
| Docs suite + diagrams | 9 |
| Cross-platform UDS, no TCP fallback | 8 + Global Constraints |
| Unit tests listed in spec | 2–7, 10 |

## Placeholder / consistency notes

- Types `BackendState`, `EndpointKind`, `GenerateParams` names are stable across tasks.
- `stream_generate` writes NDJSON directly to the socket writer; `handle_rpc` does not double-wrap the final result.
- Probe tries chat after any generate probe failure (404 or unreachable), matching “both must succeed-path fail → Disabled.”
