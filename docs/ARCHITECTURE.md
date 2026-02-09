# k-Terminus Architecture

Technical architecture documentation for developers.

## Overview

k-Terminus uses a hub-and-spoke architecture where remote machines (agents) establish reverse SSH tunnels to a local orchestrator. This inverts the traditional SSH model for firewall-friendly connectivity.

```
                         ┌─────────────────────┐
                         │    Orchestrator     │
                         │   (Your Laptop)     │
                         │                     │
              ┌──────────┤  SSH Server :2222   │
              │          │  IPC Server :22230  │
              │          └─────────────────────┘
              │                    ▲
              │                    │ IPC
              │                    │
    ┌─────────┴─────────┐  ┌──────┴───────┐
    │                   │  │              │
    ▼                   ▼  │   CLI/GUI    │
┌───────────┐     ┌───────────┐           │
│  Agent 1  │     │  Agent 2  │           │
│ (Server)  │     │ (Cloud)   │           │
└───────────┘     └───────────┘           │
    PTY               PTY                 │
                                          │
                              Terminal I/O
```

## Components

### Orchestrator (`kt-orchestrator`)

The orchestrator runs on your local machine and:

1. **SSH Server** - Accepts reverse tunnel connections from agents
2. **Connection Pool** - Tracks connected machines with health monitoring
3. **Session Manager** - Manages terminal sessions across machines
4. **IPC Server** - Provides API for CLI and desktop app
5. **Tailscale Verifier** - Authenticates peers via tailnet membership

**Key files:**
- `src/server/listener.rs` - SSH server accepting connections
- `src/server/handler.rs` - SSH authentication and message handling
- `src/connection/pool.rs` - Connection tracking with DashMap
- `src/session/manager.rs` - Session lifecycle management
- `src/ipc/server.rs` - JSON-based IPC over TCP
- `src/auth/tailscale.rs` - Tailscale peer verification

### Agent (`kt-agent`)

The agent runs on remote machines and:

1. **Tunnel Connector** - Establishes SSH connection to orchestrator
2. **PTY Manager** - Spawns and manages pseudo-terminals
3. **Reconnection** - Automatic reconnect with exponential backoff
4. **Event Loop** - Handles orchestrator commands

**Key files:**
- `src/tunnel/connector.rs` - SSH client connection
- `src/tunnel/active.rs` - Active tunnel message handling
- `src/pty/manager.rs` - PTY spawning and I/O
- `src/tunnel/reconnect.rs` - Backoff strategy

### Protocol (`kt-protocol`)

Wire protocol for multiplexing sessions over SSH:

```
┌──────────────┬─────────────┬────────────────┬─────────────┐
│ Session ID   │ Message Type│ Payload Length │ Payload     │
│ (4 bytes)    │ (1 byte)    │ (3 bytes)      │ (variable)  │
└──────────────┴─────────────┴────────────────┴─────────────┘
```

**Message Types:**
| Type | Code | Direction | Description |
|------|------|-----------|-------------|
| Register | 0x08 | Agent → Orch | Machine registration |
| RegisterAck | 0x09 | Orch → Agent | Registration confirmation |
| SessionCreate | 0x01 | Orch → Agent | Create new PTY |
| SessionReady | 0x02 | Agent → Orch | PTY created with PID |
| Data | 0x03 | Both | Terminal I/O |
| Resize | 0x04 | Orch → Agent | Window size change |
| SessionClose | 0x05 | Both | Session termination |
| Heartbeat | 0x06 | Orch → Agent | Keep-alive ping |
| HeartbeatAck | 0x07 | Agent → Orch | Keep-alive pong |

**Key files:**
- `src/frame.rs` - Frame encoding/decoding
- `src/message.rs` - Message type definitions
- `src/codec.rs` - Tokio codec implementation

### Core (`kt-core`)

Shared functionality:

- **Configuration** - TOML config parsing with serde
- **Tailscale Integration** - Status queries, peer lookup
- **IPC Types** - Request/response definitions
- **Setup** - First-run initialization

**Key files:**
- `src/config/` - Configuration structs
- `src/tailscale.rs` - Tailscale CLI wrapper
- `src/ipc.rs` - IPC message types
- `src/setup.rs` - Auto-setup logic

### CLI (`kt-cli`)

Single binary providing all commands:

- Orchestrator mode (`serve`)
- Agent mode (`join`)
- Management commands (`list`, `connect`, etc.)

**Key files:**
- `src/main.rs` - Command routing and implementations
- `src/commands/` - Individual command implementations
- `src/ipc/client.rs` - IPC client for management commands

### Desktop App (`kt-desktop`)

Tauri 2.0 application with React frontend and embedded orchestrator:

**Rust Backend (`src-tauri/`):**
- `src/orchestrator.rs` - Embedded orchestrator lifecycle management
- `src/ipc_client.rs` - IPC client for orchestrator communication
- `src/commands.rs` - Tauri command handlers
- `src/state.rs` - Application state management

**React Frontend (`src/`):**
- `stores/terminals.ts` - Terminal tabs and sessions (Zustand)
- `stores/layout.ts` - Pane layout tree (Zustand with persistence)
- `stores/machines.ts` - Connected machines state
- `components/terminal/` - Terminal rendering and pane management
- `components/sidebar/` - Machine list with virtual scrolling

**Pane Layout System:**

The terminal view uses a recursive tree-based layout:

```typescript
type LayoutNode =
  | { type: "pane"; id: string; tabId: string }
  | { type: "split"; id: string; direction: "horizontal" | "vertical";
      children: LayoutNode[]; sizes: number[] };
```

- Tree is stored in Zustand with localStorage persistence
- `react-resizable-panels` handles resize interactions
- Drag-and-drop uses HTML5 API with drop zones on pane edges

## Data Flow

### Connection Establishment

```
Agent                                    Orchestrator
  │                                           │
  ├── TCP connect (Tailscale IP:2222) ───────►│
  │                                           │
  │                              Check: loopback or tailnet?
  │                                           │
  │◄─────────── SSH handshake ────────────────┤
  │                                           │
  ├── SSH auth (public key) ─────────────────►│
  │                                           │
  │                              Loopback? Accept
  │                              Tailscale peer? Accept
  │                              Otherwise? Reject
  │                                           │
  │◄─────────── Auth accept ──────────────────┤
  │                                           │
  ├── Open channel ──────────────────────────►│
  │                                           │
  ├── Register{machine_id, hostname, os} ────►│
  │                                           │
  │◄── RegisterAck{accepted: true} ───────────┤
  │                                           │
  │           Connection established          │
  │◄─────────── Heartbeat ────────────────────┤
  ├── HeartbeatAck ──────────────────────────►│
```

### Session Creation

```
CLI                      Orchestrator                    Agent
 │                            │                            │
 ├── IPC: CreateSession ─────►│                            │
 │      {machine_id}          │                            │
 │                            ├── SessionCreate ──────────►│
 │                            │    {session_id, shell}     │
 │                            │                            │
 │                            │                       spawn PTY
 │                            │                            │
 │                            │◄── SessionReady ───────────┤
 │                            │    {session_id, pid}       │
 │                            │                            │
 │◄── IPC: SessionCreated ────┤                            │
 │    {session_id}            │                            │
 │                            │                            │
 │    [User types]            │                            │
 │                            │                            │
 ├── IPC: TerminalInput ─────►│                            │
 │    {session_id, data}      ├── Data ───────────────────►│
 │                            │                       PTY stdin
 │                            │                            │
 │                            │                       PTY stdout
 │                            │◄── Data ───────────────────┤
 │◄── IPC: TerminalOutput ────┤                            │
 │    {session_id, data}      │                            │
```

## Authentication Flow

k-Terminus uses Tailscale as its sole authentication mechanism:

```rust
// In handler.rs auth_publickey()

// 1. Loopback connections are always trusted (same machine)
if peer_ip.is_loopback() {
    self.machine_id = Some(MachineId::new(&format!("local-{}", &fingerprint[..8])));
    return Ok(Auth::Accept);
}

// 2. Tailscale verification
if let Some(peer_info) = self.state.tailscale.verify_peer(peer_ip) {
    // Peer is in our tailnet - trusted
    self.machine_id = Some(MachineId::new(&peer_info.device_name));
    return Ok(Auth::Accept);
}

// 3. Reject - not loopback and not in tailnet
Ok(Auth::Reject { proceed_with_methods: None })
```

The Tailscale verifier caches peer list for 30 seconds:

```rust
// In auth/tailscale.rs
pub fn verify_peer(&self, ip: IpAddr) -> Option<TailscalePeer> {
    // Check cache
    if cache.last_refresh.elapsed() < CACHE_DURATION {
        return cache.peers.iter().find(|p| p.ips.contains(&ip_str));
    }

    // Refresh from `tailscale status --json`
    self.refresh_cache();
    // ...
}
```

## IPC Protocol

CLI and desktop app communicate with orchestrator via TCP (localhost:22230).

### Authentication

IPC connections require token-based authentication:

1. Orchestrator generates random 64-character token on startup
2. Token written to `~/.k-terminus/ipc_auth_token` with mode 600
3. Clients read token from file and send `Authenticate` request
4. All requests except `Ping` and `VerifyPairingCode` require authentication

```
Client                              Orchestrator
  │                                      │
  ├── {"type": "authenticate",           │
  │    "token": "abc123..."} ───────────►│
  │                                      │
  │◄── {"type": "authenticated"} ────────┤
  │                                      │
  ├── {"type": "list_machines"} ────────►│
  │                                      │
  │◄── {"type": "machines", ...} ────────┤
```

### Request/Response Format

**Request format:** JSON with `type` field
```json
{"type": "ping"}
{"type": "authenticate", "token": "..."}
{"type": "list_machines"}
{"type": "create_session", "machine_id": "home-server", "shell": null}
{"type": "verify_pairing_code", "code": "ABC123"}
```

**Response format:** JSON with `type` field
```json
{"type": "pong"}
{"type": "authenticated"}
{"type": "authentication_required"}
{"type": "machines", "machines": [...]}
{"type": "session_created", "id": "...", "machine_id": "..."}
{"type": "pairing_code_valid", "valid": true}
{"type": "error", "message": "..."}
```

### Rate Limiting

IPC server enforces rate limits to prevent abuse:
- 1000 requests per second per client
- Maximum 100 concurrent connections

## Concurrency Model

- **Tokio** async runtime for all I/O
- **DashMap** for concurrent connection/session storage
- **CancellationToken** for graceful shutdown
- **mpsc channels** for event broadcasting

## Error Handling

- **anyhow** for error propagation with context
- **thiserror** for typed errors in libraries
- Clear error messages when Tailscale is unavailable

## Security Model

k-Terminus implements multiple security measures to protect against common attack vectors.

### Input Validation

All input from external sources is validated before processing:

- **Session input**: Terminal data is limited to 64KB per message to prevent memory exhaustion attacks
- **Protocol messages**: Frame payloads are limited to 16MB (enforced by 24-bit length field in frame header)
- **IPC requests**: JSON requests are validated with size limits before parsing

### Session Ownership Tracking

Sessions are bound to specific machines to prevent unauthorized access:

- Each session stores the `machine_id` of the machine that owns it
- Session operations (input, resize, close) verify the session belongs to the requesting machine
- Sessions can only be created on connected machines

### Session Cleanup on Disconnect

When an agent disconnects (intentionally or due to network failure):

1. All sessions belonging to that machine are identified via `remove_by_machine()`
2. Sessions are cleanly terminated and removed from the session manager
3. Resources (PTY handles, buffers) are released
4. IPC clients are notified of session termination

This prevents orphaned sessions and ensures clean state recovery.

## Scalability

k-Terminus supports configurable limits for resource management.

### Connection Limits

The orchestrator can enforce maximum concurrent agent connections:

```toml
[orchestrator]
max_connections = 100  # Optional, default unlimited
```

When the limit is reached:
- New connection attempts receive a clear error message
- Existing connections are not affected
- Administrators can monitor connection count via `k-terminus status`

### Session Limits

Per-machine session limits prevent resource exhaustion:

```toml
[orchestrator]
max_sessions_per_machine = 10  # Optional, default unlimited
```

When creating a session would exceed the limit:
- The session creation request is rejected with `SessionLimitExceeded` error
- Existing sessions continue to function
- Users can close sessions to free capacity

## Protocol Versioning

The k-Terminus protocol includes version negotiation for forward compatibility.

### Version Field in Register Message

When agents connect, they send a `Register` message that may include:

```rust
Register {
    machine_id: String,
    hostname: String,
    os: String,
    arch: String,
    version: Option<String>,  // Protocol version (e.g., "1.0")
}
```

The orchestrator can use this to:
- Reject connections from incompatible protocol versions
- Enable or disable features based on agent capabilities
- Log version distribution for compatibility planning

### Version Compatibility

| Protocol Version | Features |
|------------------|----------|
| 1.0 | Core functionality (sessions, heartbeat) |

Future versions will maintain backward compatibility where possible.

## Testing Strategy

| Level | Location | Tests | Description |
|-------|----------|-------|-------------|
| Unit | `src/*.rs` | ~85 | Inline module tests |
| CLI Integration | `kt-cli/tests/cli_integration.rs` | 14 | CLI argument parsing and output |
| IPC Integration | `kt-orchestrator/tests/ipc_integration.rs` | 14 | IPC server communication |
| E2E | `kt-cli/tests/e2e_test.rs` | 7 | Full process spawning |

**Total: 120+ tests, all passing.**

Run with `cargo test --workspace -- --test-threads=1` for reliability.

Note: E2E tests spawn actual orchestrator and agent processes but use loopback connections, so they work without Tailscale installed (useful for CI environments).
