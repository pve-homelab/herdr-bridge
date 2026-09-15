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
