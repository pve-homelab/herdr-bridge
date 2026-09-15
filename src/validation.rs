//! Startup endpoint validation and BackendState.

use reqwest::Client;
use tracing::warn;

use crate::http_client::apply_auth;

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

async fn probe(client: &Client, url: &str, api_key: Option<&str>) -> Result<(), ProbeFail> {
    let req = apply_auth(client.get(url), api_key);
    match req.send().await {
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

pub async fn validate_backend(
    client: &Client,
    base_url: &str,
    api_key: Option<&str>,
) -> BackendState {
    let base = trim_base_url(base_url);
    if base.is_empty() {
        warn!("MODEL_BASE_URL empty; model tools disabled");
        return BackendState::Disabled {
            reason: "MODEL_BASE_URL is empty".into(),
        };
    }

    let generate_url = join_endpoint(&base, "generate");
    match probe(client, &generate_url, api_key).await {
        Ok(()) => {
            return BackendState::Active {
                endpoint_url: generate_url,
                kind: EndpointKind::Generate,
            };
        }
        Err(ProbeFail::NotFound) | Err(ProbeFail::Unreachable) => {}
    }

    let chat_url = join_endpoint(&base, "chat/completions");
    match probe(client, &chat_url, api_key).await {
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
        match validate_backend(&client, "   ", None).await {
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
        match validate_backend(&client, &server.uri(), None).await {
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
        match validate_backend(&client, &server.uri(), None).await {
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
            validate_backend(&client, &server.uri(), None).await,
            BackendState::Disabled { .. }
        ));
    }
}
