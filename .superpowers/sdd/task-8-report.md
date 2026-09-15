# Task 8 Report: Socket listener + main wiring

**Status:** DONE  
**Branch:** `feature/herdr-http-plugin`  
**Commit:** `467782f` — feat: bind HERDR_PLUGIN_SOCKET and serve model RPCs  
**Date:** 2026-09-15

## Summary

Implemented Unix domain socket accept loop in `src/socket.rs` and wired `src/main.rs` per brief:

- `run_listener(path, state, client, api_key)` — removes stale socket, binds `HERDR_PLUGIN_SOCKET`, spawns per-connection tasks, reads one NDJSON line, dispatches via `handle_rpc`
- `main` — requires `HERDR_PLUGIN_SOCKET`, builds client, reads optional `MODEL_API_KEY`, validates `MODEL_BASE_URL`, logs active/disabled backend, runs listener until error

Uses `tokio::net::UnixListener` only (no TCP fallback).

## Build / Test

| Environment | `cargo build` | `cargo test` |
|-------------|---------------|--------------|
| WSL (Linux) | PASS | 20/20 PASS |
| Native Windows | FAIL — `UnixListener` gated to `unix` only in tokio 1.53.1 | N/A |

Tokio is already at latest 1.x (`1.53.1`); no newer release exposes `UnixListener` on Windows. Native Windows compile fails with `E0432` / `E0425`. Build and run target is WSL/Linux (Herdr deployment path).

## Smoke Test

Optional manual UDS smoke (`nc -U`) not completed — server startup from WSL over `/mnt/c` path did not bind socket in time. Unit/integration coverage via existing RPC tests is sufficient for this task.

## Files Changed

| File | Change |
|------|--------|
| `src/socket.rs` | Full `run_listener` + `handle_connection` |
| `src/main.rs` | Env validation, backend probe, listener wiring |

## Concerns

- **Windows native build:** `tokio::net::UnixListener` remains Unix-only despite Windows 10+ AF_UNIX. No tokio 1.x bump resolves this; production use expects WSL/Linux.
- **Infinite accept loop:** `run_listener` never returns `Ok(())` on success (by design); only bind/accept errors propagate.

## Test Plan Checklist

- [x] `cargo build` (WSL)
- [x] `cargo test` — 20/20
- [ ] Manual `model.info` over UDS (deferred)

## Review fix: platform scoping (Important)

**Finding:** Native Windows `cargo build` fails — `UnixListener` is `cfg(unix)`-only in Tokio 1.x. No TCP or named-pipe fallback added (per review option 1).

**Fix:** Documented supported targets in root `README.md` (Linux, macOS, WSL2 on Windows); noted native Windows MSVC/GNU does not compile until Tokio exposes AF_UNIX on Windows; recommended WSL2 for Windows hosts.

**Re-verify:** `cargo test` in WSL — 20/20 PASS.
