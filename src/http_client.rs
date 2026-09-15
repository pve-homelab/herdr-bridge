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
