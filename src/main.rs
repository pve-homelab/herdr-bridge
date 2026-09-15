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
