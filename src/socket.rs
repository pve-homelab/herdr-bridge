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
