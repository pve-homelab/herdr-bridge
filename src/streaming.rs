//! Streaming HTTP consumption, SSE parsing, and non-stream fallback.

use anyhow::{Context, Result};
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt;
use tracing::warn;

use crate::http_client::apply_auth;
use crate::model::{build_request_body, generate_completion, GenerateParams};
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
