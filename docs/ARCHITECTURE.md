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

**Request format:** JSON with `type` field
```json
{"type": "ping"}
{"type": "list_machines"}
{"type": "create_session", "machine_id": "home-server", "shell": null}
```

**Response format:** JSON with `type` field
```json
{"type": "pong"}
{"type": "machines", "machines": [...]}
{"type": "session_created", "id": "...", "machine_id": "..."}
{"type": "error", "message": "..."}
```

## Concurrency Model

- **Tokio** async runtime for all I/O
- **DashMap** for concurrent connection/session storage
- **CancellationToken** for graceful shutdown
- **mpsc channels** for event broadcasting

## Error Handling

- **anyhow** for error propagation with context
- **thiserror** for typed errors in libraries
- Clear error messages when Tailscale is unavailable

## Testing Strategy

| Level | Location | Description |
|-------|----------|-------------|
| Unit | `src/*.rs` | Inline module tests |
| Integration | `tests/*.rs` | Component interaction |
| E2E | `kt-cli/tests/e2e_test.rs` | Full process spawning |

Run with `cargo test --workspace -- --test-threads=1` for reliability.
