# Diagrams

Visual reference for `herdr-http-plugin`. Mermaid renders in GitHub and many Markdown viewers.

## Mermaid: architecture

```mermaid
flowchart LR
    H[Herdr] -->|UDS NDJSON| SK[socket]
    SK --> RPC[rpc]
    RPC --> MI[model.info]
    RPC --> MG[model.generate]
    RPC --> MSG[model.stream_generate]
    MG --> M[model]
    MSG --> ST[streaming]
    M --> HC[http_client]
    ST --> HC
    HC -->|HTTP| BE[(MODEL_BASE_URL)]
    VAL[validation] -->|startup probe| BE
    VAL -->|BackendState| RPC
    MAIN[main] --> VAL
    MAIN --> SK
```

## Mermaid: streaming flow

```mermaid
flowchart TD
    A[model.stream_generate] --> B[POST stream=true]
    B --> C{HTTP OK?}
    C -->|no| F[fallback: POST stream=false]
    C -->|yes| D[bytes_stream loop]
    D --> E[parse SSE / NDJSON]
    E --> G{non-empty delta?}
    G -->|yes| H[write model.stream delta + flush]
    H --> D
    G -->|no| D
    D --> I{stream ended}
    I -->|saw deltas| J[write final completion]
    I -->|no deltas| F
    F --> J
```

## Mermaid: non-streaming flow

```mermaid
sequenceDiagram
    participant H as Herdr
    participant R as rpc/model
    participant B as Backend

    H->>R: model.generate
    R->>B: POST stream=false
    B-->>R: JSON body
    R->>R: extract_completion
    R->>H: {"id","result":{"completion":"…"}}
    Note over H,R: socket closed
```

## Mermaid: auto-disable decision tree

```mermaid
flowchart TD
    START([Startup]) --> SOCKET{HERDR_PLUGIN_SOCKET set?}
    SOCKET -->|no| EXIT([Exit error])
    SOCKET -->|yes| TRIM[Trim MODEL_BASE_URL]
    TRIM --> EMPTY{empty?}
    EMPTY -->|yes| DIS([Disabled: empty URL])
    EMPTY -->|no| G[GET /generate]
    G --> GOK{404 or unreachable?}
    GOK -->|no| ACTG([Active: Generate])
    GOK -->|yes| C[GET /chat/completions]
    C --> COK{404 or unreachable?}
    COK -->|no| ACTC([Active: ChatCompletions])
    COK -->|yes| DIS2([Disabled: no valid endpoint])
    DIS --> BIND[Bind UDS, serve model.info]
    DIS2 --> BIND
    ACTG --> BIND
    ACTC --> BIND
```

## Mermaid: TUI rendering flow

```mermaid
flowchart LR
    U[User action in TUI] --> H[Herdr]
    H --> P[Plugin socket RPC]
    P --> D{stream_generate?}
    D -->|yes| DELTA[delta events]
    DELTA --> TUI[Append tokens in UI]
    DELTA --> FIN[final completion]
    D -->|no| RES[single result or error]
    FIN --> TUI
    RES --> TUI
    P --> ERR[error.message]
    ERR --> TUI
```

---

## ASCII: plugin architecture

```text
                    ┌─────────────────────────────────────┐
                    │           herdr-http-plugin           │
                    │  ┌─────────┐      ┌──────────────┐  │
                    │  │  main   │─────►│  validation  │  │
                    │  └────┬────┘      └──────────────┘  │
                    │       │                             │
                    │  ┌────▼────┐      ┌──────────────┐  │
                    │  │ socket  │─────►│     rpc      │  │
                    │  └─────────┘      └───┬──────┬───┘  │
                    │                       │      │      │
                    │              ┌────────▼──┐ ┌─▼──────┐ │
                    │              │  model  │ │streaming│ │
                    │              └────┬────┘ └────┬────┘ │
                    │                   └──────┬─────┘      │
                    │                    ┌─────▼─────┐     │
                    │                    │http_client│     │
                    │                    └─────┬─────┘     │
                    └──────────────────────────┼───────────┘
                                               │ HTTP
                                               ▼
                                         MODEL_BASE_URL
```

## ASCII: socket lifecycle

```text
  [Herdr spawns plugin]
         │
         ▼
  remove stale socket file (best effort)
         │
         ▼
  UnixListener::bind(HERDR_PLUGIN_SOCKET) ──fail──► exit (no TCP fallback)
         │
         ▼
  ┌────────────────── accept loop ──────────────────┐
  │  accept() ──► spawn task                        │
  │       read_line (one NDJSON request)              │
  │       handle_rpc ──► 1..N write lines           │
  │       drop stream (connection closed)             │
  └─────────────────────────────────────────────────┘
```

## ASCII: request/response flow

```text
REQUEST (one line):
  {"id":42,"method":"model.stream_generate","params":{"prompt":"hi"}}

RESPONSE (many lines possible):
  {"event":"model.stream","delta":"Hel"}
  {"event":"model.stream","delta":"lo"}
  {"id":42,"result":{"completion":"Hello"}}

ERROR (one line):
  {"id":42,"error":{"message":"model tools disabled: …"}}
```

## ASCII: picture-style — Herdr ↔ plugin ↔ cloud

```text
      .---.
     /     \          ╭──────────────────╮
    |  o o  |  Herdr  │  herdr-http-     │     ╭─────────╮
    |   ▽   │ ◄─UDS──►│  plugin          │─HTTP►│  LLM    │
     \     /          │  (NDJSON bridge) │     │  server │
      `---'           ╰──────────────────╯     ╰─────────╯
   "user at TUI"         "Rust Tokio bin"        "localhost:8000/v1"
```

## Related docs

- [ARCHITECTURE.md](ARCHITECTURE.md) — narrative architecture
- [FEATURES.md](FEATURES.md) — feature descriptions
- [TUI_USAGE.md](TUI_USAGE.md) — TUI behavior
