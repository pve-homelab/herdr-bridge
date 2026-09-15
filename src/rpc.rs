//! Herdr NDJSON RPC dispatch for model.* methods.

use anyhow::Result;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validation::BackendState;
    use serde_json::json;

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
