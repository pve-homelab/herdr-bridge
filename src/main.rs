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
