# k-Terminus

## Design & Technical Specification

**Distributed Terminal Session Manager**

Version 1.0 Design Document  
January 2026

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Project Overview](#1-project-overview)
3. [System Architecture](#2-system-architecture)
4. [Technical Stack](#3-technical-stack)
5. [Protocol Design](#4-protocol-design)
6. [Configuration Management](#5-configuration-management)
7. [Command-Line Interface](#6-command-line-interface)
8. [Desktop Application](#7-desktop-application-tauri)
9. [Crate Structure](#8-crate-structure)
10. [Future Enhancements](#9-future-enhancements)
11. [Conclusion](#10-conclusion)
12. [Appendices](#appendices)

---

## Executive Summary

k-Terminus is a distributed terminal session manager designed to orchestrate command-line environments across heterogeneous infrastructure through reverse SSH tunnels. Built in Rust with Tauri for cross-platform desktop deployment, k-Terminus enables unified management of multiple remote machines from a single local orchestrator.

### Implementation Status (February 2026)

| Component | Status | Notes |
|-----------|--------|-------|
| **Core Protocol** | ✅ Complete | Frame codec, message types, session multiplexing |
| **Orchestrator** | ✅ Complete | SSH server, connection pool, session manager, IPC with authentication |
| **Agent** | ✅ Complete | SSH tunnel, PTY management, reconnection, pairing code discovery |
| **CLI** | ✅ Complete | All commands implemented |
| **Tailscale Auth** | ✅ Complete | Peer verification (Tailscale-only, no fallback) |
| **Test Suite** | ✅ Complete | 120+ tests (unit, integration, e2e), all passing |
| **Desktop App** | ✅ Complete | Full UI with pane splitting, embedded orchestrator, real IPC integration |
| **Terminal I/O** | ✅ Complete | Bidirectional PTY streaming via xterm.js |
| **Security Hardening** | ✅ Complete | Input validation, session ownership, resource limits, IPC authentication |
| **Protocol Versioning** | ✅ Complete | Version field in Register message for compatibility |

The primary use case is managing distributed Claude Code sessions across lab servers, development machines, and research infrastructure. However, k-Terminus is architected as a general-purpose tool suitable for any workflow requiring coordinated terminal access across multiple systems.

### Key Features

- **Tailscale networking**: Zero-config mesh VPN with stable device hostnames
- **Simple setup**: One command to serve, one command to join (no manual key copying)
- **Reverse tunnel architecture**: Client-initiated, firewall-friendly connections
- **Session multiplexing**: Multiple sessions over single tunnel per machine
- **Persistent connections**: Automatic reconnection with exponential backoff
- **Unified CLI**: Single binary for orchestrator, agent, and management
- **Cross-platform desktop GUI**: Tauri app with embedded terminal emulator
- **Layered security**: Tailscale WireGuard encryption + SSH transport

---

## 1. Project Overview

### 1.1 Name Etymology

The name k-Terminus combines mathematical notation with classical terminology:

- **k-mer**: Computational biology term for sequence k-tuples, referencing the project's origin in research computing
- **k-nearest neighbors**: Machine learning algorithm representing graph-based computation, metaphorically analogous to distributed node management
- **Terminus**: Latin for "boundary" or "endpoint," the etymological root of "terminal." Also the Roman god of boundaries and landmarks

The hyphenated k- prefix evokes mathematical notation (k-space, k-means, k-fold) while grounding the tool in terminal management through the terminus component. This creates a professional, technical aesthetic appropriate for infrastructure tooling.

### 1.2 Problem Statement

#### Current Pain Points

- Claude Code sessions are isolated per-machine with no orchestration layer
- Multi-machine workflows require manual SSH connections and frequent context switching
- Firewalls and NAT complicate direct inbound connections to lab/research infrastructure
- No unified view of distributed terminal sessions across heterogeneous environment
- Nested SSH sessions create latency and complexity (SSH over SSH)

#### Target Environment

- Local development machine (MacBook, Linux workstation)
- University lab servers (behind institutional firewall)
- GPU compute nodes (restricted access, specialized hardware)
- CI/CD build machines (ephemeral, automated deployment)
- Research infrastructure (specialized software stacks, shared resources)

### 1.3 Solution Approach

k-Terminus implements a hub-and-spoke architecture where remote machines initiate reverse SSH tunnels to a local orchestrator. This inverts the traditional SSH model:

- **Firewall-friendly**: Remote machines connect outbound (typically allowed by default)
- **Persistent connections**: Tunnels remain open, enabling immediate session creation
- **Session multiplexing**: Multiple logical sessions over single tunnel (avoiding nested SSH)
- **Local orchestration**: All control logic runs on trusted local machine
- **Zero remote configuration**: Lightweight client agent is only remote component

---

## 2. System Architecture

### 2.1 High-Level Design

The system consists of three primary components:

1. **Orchestrator** (local machine): Accepts reverse tunnels, manages connection pool, multiplexes sessions
2. **Client Agent** (remote machines): Lightweight daemon that establishes tunnel and manages local PTY sessions
3. **CLI/GUI Interface**: User-facing tools for session management and orchestration

### 2.2 Component Architecture

| Component | Responsibilities |
|-----------|-----------------|
| **Orchestrator** | • Accept and authenticate reverse SSH tunnels<br>• Maintain connection pool with health monitoring<br>• Multiplex terminal sessions over tunnels<br>• Handle automatic reconnection with backoff<br>• Provide CLI/GUI interface to connected machines<br>• Persist connection state and configuration |
| **Client Agent** | • Establish outbound reverse SSH tunnel<br>• Authenticate using configured credentials<br>• Create and manage local PTY sessions<br>• Stream stdin/stdout/stderr bidirectionally<br>• Report system metrics (CPU, memory, disk)<br>• Handle window resize events |
| **CLI Interface** | • List connected machines and active sessions<br>• Create new sessions on specified machines<br>• Attach to existing sessions<br>• Execute one-off commands<br>• Query connection status and health<br>• Manage configuration |
| **GUI Interface** | • Visual topology of connected machines<br>• Interactive session management<br>• Real-time health and metrics dashboard<br>• Configuration editor<br>• Embedded terminal emulator<br>• Connection logs and diagnostics |

### 2.3 Data Flow

#### Connection Establishment

1. Remote client agent initiates outbound SSH connection to orchestrator
2. Orchestrator validates peer via Tailscale network membership (or accepts loopback)
3. Tunnel registered in connection pool with unique machine_id
4. Client sends initial capability/status message
5. Orchestrator confirms registration and begins heartbeat monitoring

#### Session Creation

1. User issues connect command via CLI
2. Orchestrator looks up machine_id in connection pool
3. Creates session request with unique session_id
4. Sends request over tunnel to client agent
5. Client allocates PTY and spawns shell process
6. Bidirectional stream established (stdin → PTY → stdout)
7. User terminal attached to remote session

#### Session Interaction

- Local keystrokes → Orchestrator → Tunnel → Client Agent → PTY stdin
- PTY stdout → Client Agent → Tunnel → Orchestrator → Local terminal
- Window resize → Orchestrator → Tunnel → Client Agent → PTY ioctl

---

## 3. Technical Stack

### 3.1 Core Technologies

| Technology | Purpose | Rationale |
|------------|---------|-----------|
| **Rust** | Core language for all components | Memory safety, zero-cost abstractions, excellent async support, cross-platform single binary |
| **Tokio** | Async runtime | Production-grade async I/O for handling concurrent connections efficiently |
| **Tauri** | Desktop application framework | Lightweight Rust-based alternative to Electron with native OS integration |
| **russh** | SSH protocol implementation | Pure Rust SSH library with async support for tunnel management |
| **portable-pty** | Cross-platform PTY | Unified PTY abstraction for Unix and Windows terminal emulation |
| **clap** | CLI argument parsing | Robust command-line interface with subcommands and validation |
| **serde + TOML** | Configuration serialization | Human-readable configuration format with type-safe deserialization |
| **Tailscale** | Mesh VPN networking | Zero-config networking across NAT/firewalls, stable device identities, WireGuard encryption, implicit trust boundary |

### 3.2 Design Rationale: Why Rust?

- **Memory Safety**: Eliminates entire classes of bugs (use-after-free, buffer overflows) critical for network-facing code handling untrusted connections
- **Zero-Cost Abstractions**: High-level ergonomics without runtime overhead, essential for performance-sensitive networking code
- **Async Ecosystem**: Tokio provides production-grade async runtime for handling hundreds of concurrent connections with minimal resource usage
- **Cross-Platform**: Single codebase compiles to native binaries for Linux, macOS, Windows without runtime dependencies
- **Strong Typing**: Protocol correctness enforced at compile-time through type system, preventing subtle bugs
- **Embedded Ecosystem**: Tauri and portable-pty provide first-class Rust support for desktop GUI and terminal emulation

---

## 4. Protocol Design

### 4.1 Reverse Tunnel Protocol

#### Connection Lifecycle

1. **Client Initiation**: Remote agent initiates outbound SSH connection to orchestrator's listening port
2. **Authentication**: Tailscale peer verification (or loopback acceptance for local connections)
3. **Registration**: Client sends capability message containing machine_id, hostname, system info
4. **Confirmation**: Orchestrator acknowledges registration and begins heartbeat protocol
5. **Steady State**: Tunnel remains open with periodic heartbeats (30s interval)
6. **Failure Handling**: Connection loss triggers automatic reconnection with exponential backoff (1s, 2s, 4s, 8s, max 60s)

### 4.2 Session Multiplexing Protocol

Multiple terminal sessions are multiplexed over a single SSH tunnel using a lightweight framing protocol.

#### Message Format

- **Header**: 8 bytes (session_id: 4 bytes, message_type: 1 byte, payload_length: 3 bytes)
- **Payload**: Variable-length binary data

#### Message Types

| Type | Code | Description |
|------|------|-------------|
| **SessionCreate** | 0x01 | Request new PTY allocation |
| **SessionReady** | 0x02 | Acknowledge session creation with PTY details |
| **Data** | 0x03 | Stdin/stdout/stderr streams |
| **Resize** | 0x04 | Window dimension changes (rows, cols) |
| **SessionClose** | 0x05 | Terminate session |
| **Heartbeat** | 0x06 | Keep-alive ping |
| **HeartbeatAck** | 0x07 | Keep-alive pong |
| **Register** | 0x08 | Agent registration with machine info and protocol version |
| **RegisterAck** | 0x09 | Registration acknowledgment |
| **Error** | 0xFF | Error response |

### Protocol Version

The Register message includes an optional `version` field for protocol compatibility:

```rust
Register {
    machine_id: String,
    hostname: String,
    os: String,
    arch: String,
    version: Option<String>,  // e.g., "1.0"
}
```

This enables:
- Graceful version negotiation between agent and orchestrator
- Feature detection based on agent capabilities
- Logging of protocol version distribution

### 4.3 Authentication & Security

#### Authentication Mechanism

- Tailscale network membership is the primary authentication
- Loopback connections (127.0.0.1) are always accepted for local development
- Public key authentication over SSH (no passwords)
- Each machine identified by Tailscale device name

#### Security Considerations

- SSH provides transport encryption (all data encrypted in transit)
- WireGuard (Tailscale) provides additional encryption layer
- Host key verification prevents MITM attacks
- Orchestrator only accepts connections from same Tailscale network
- No credential storage on remote machines (key-based auth only)
- Session isolation: Each session runs in separate PTY with distinct process context
- Input validation: Session input limited to 64KB, frame payloads limited to 16MB
- Session ownership: Sessions are bound to their creating machine, preventing cross-machine access
- Resource limits: Configurable max_connections and max_sessions_per_machine

#### Session Lifecycle

Sessions follow a defined lifecycle with automatic cleanup:

1. **Creation**: Session created via IPC, assigned to machine, PTY spawned on agent
2. **Active**: Terminal I/O flows bidirectionally, resize events propagated
3. **Termination**: Session closed explicitly or when:
   - User closes session via CLI/GUI
   - Agent disconnects (all sessions for that machine are cleaned up)
   - Orchestrator shuts down (all sessions terminated)

The `remove_by_machine()` function ensures clean session removal when an agent disconnects, preventing orphaned sessions and resource leaks.

### 4.4 Network Layer (Tailscale Integration)

k-Terminus uses Tailscale as its network layer to provide seamless connectivity across any network topology.

#### Why Tailscale?

- **NAT Traversal**: Works through firewalls, corporate NAT, hotel WiFi, mobile hotspots
- **Stable Identities**: Each device gets a persistent hostname (e.g., `laptop.tailnet.ts.net`) that never changes
- **Zero Configuration**: No port forwarding, no dynamic DNS, no manual IP management
- **WireGuard Security**: Military-grade encryption with minimal overhead
- **Free Tier**: 100 devices, sufficient for personal/small team use

#### Network Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    TAILSCALE MESH NETWORK                   │
│                   (WireGuard encrypted)                     │
│                                                             │
│   ┌─────────────┐  ┌─────────────┐  ┌─────────────┐       │
│   │   Laptop    │  │ Home Server │  │  Cloud VM   │       │
│   │ 100.64.1.1  │  │ 100.64.1.2  │  │ 100.64.1.3  │       │
│   │ Orchestrator│◄─│   Agent     │  │   Agent     │       │
│   │ :2222 (SSH) │◄─┼─────────────┼──┤             │       │
│   └─────────────┘  └─────────────┘  └─────────────┘       │
└─────────────────────────────────────────────────────────────┘
```

#### Device Discovery

Machines are addressed by Tailscale hostname rather than IP:
- Orchestrator: `my-laptop.tailnet-abc.ts.net:2222`
- Connect: `k-terminus join my-laptop` (resolves automatically)

#### Dependency

Tailscale must be installed and authenticated on all machines. The `k-terminus setup` command handles detection, installation, and configuration automatically.

### 4.5 Tailscale-Based Authentication

k-Terminus uses Tailscale as its authentication layer. Being on the same Tailscale network is the trust boundary - no additional OAuth or manual key exchange required.

#### Why Tailscale for Auth?

- **No Manual Key Exchange**: Users don't need to copy SSH keys between machines
- **Already Authenticated**: You logged into Tailscale via SSO (Google, GitHub, Microsoft, etc.)
- **Implicit Trust**: Same tailnet = same user/organization = trusted
- **Simple Revocation**: Remove device from Tailscale to revoke access

#### Connection Flow

```
Agent (Remote)                              Orchestrator (Local)
     │                                              │
     ├─── TCP connect (100.x.x.y:2222) ────────────▶│
     │                                              │
     │                         Query: tailscale status --json
     │                         "Is 100.x.x.y in my tailnet?"
     │                                              │
     │                         Yes: "lab-server.tailnet.ts.net"
     │                                              │
     ├─── SSH handshake ───────────────────────────▶│
     │    (auto-generated key)                      │
     │                                              │
     │◀────────────── Accept ───────────────────────┤
     │                                              │
     └─── Register: {device: "lab-server"} ────────▶│
```

#### Security Model

| Layer | Mechanism |
|-------|-----------|
| Identity | Tailscale device identity (verified via SSO) |
| Authorization | Same tailnet = trusted |
| Network | Tailscale WireGuard (encrypted) |
| Transport | SSH (encrypted tunnel) |

All traffic is encrypted twice (WireGuard + SSH). Tailscale handles identity verification.

---

## 5. Configuration Management

### 5.1 Configuration File Structure

Configuration stored in TOML format with hierarchical organization.

**Config Directory Locations:**
| Platform | Path |
|----------|------|
| **macOS** | `~/Library/Application Support/k-terminus/` |
| **Linux** | `~/.config/k-terminus/` |
| **Windows** | `%APPDATA%\k-terminus\` |

#### Orchestrator Configuration

- **bind_address**: Listen address for reverse tunnel connections (default: `127.0.0.1:2222`)
- **host_key_path**: Path to SSH host key file
- **tailscale_hostname**: Tailscale device hostname (auto-populated by setup)
- **heartbeat_interval**: Seconds between heartbeats (default: 30)
- **reconnect_backoff**: Exponential backoff parameters

#### Machine Profiles

- **alias**: Human-readable name for machine
- **host_key**: SSH host key fingerprint for verification
- **tags**: Organizational labels (e.g., `gpu`, `compute`, `dev`)
- **default_shell**: Shell to spawn in new sessions (default: user's login shell)
- **env**: Environment variables to set in sessions

### 5.2 Example Configuration

```toml
[orchestrator]
# Default is 127.0.0.1:2222; use 0.0.0.0:2222 for network access
bind_address = "0.0.0.0:2222"
host_key_path = "~/.config/k-terminus/host_key"

# Tailscale integration (auto-populated by k-terminus setup)
tailscale_hostname = "my-laptop.tailnet-abc.ts.net"

heartbeat_interval = 30

[orchestrator.backoff]
initial = 1
max = 60
multiplier = 2.0
jitter = 0.25

[machines.laptop]
alias = "macbook"
host_key = "ssh-ed25519 AAAAC3..."
tags = ["dev", "local"]

[machines.gpu-server]
alias = "lab-gpu-01"
host_key = "ssh-ed25519 AAAAC3..."
tags = ["gpu", "compute", "lab"]
default_shell = "/bin/bash"
env = { CUDA_VISIBLE_DEVICES = "0,1" }

[machines.build-farm]
alias = "ci-builder"
host_key = "ssh-ed25519 AAAAC3..."
tags = ["ci", "build"]
```

---

## 6. Command-Line Interface

### 6.1 Core Commands

| Command | Description |
|---------|-------------|
| `k-terminus serve` | Start orchestrator and begin accepting connections |
| `k-terminus join <host>` | Connect to orchestrator as an agent (host = Tailscale hostname) |
| `k-terminus list` | Display all connected machines and active sessions with status |
| `k-terminus connect <machine>` | Create new session on specified machine and attach terminal |
| `k-terminus status` | Show orchestrator status and connection health metrics |
| `k-terminus stop` | Stop orchestrator daemon |
| `k-terminus kill <session>` | Terminate specified session |
| `k-terminus config show` | Display current configuration |
| `k-terminus config edit` | Open config file in editor |
| `k-terminus config path` | Print config directory path |

### 6.2 Command Examples

#### Basic Workflow

```bash
# On your main machine - start the orchestrator
$ k-terminus serve
Starting k-Terminus orchestrator...
Listening on my-laptop.tailnet-abc.ts.net:2222

To connect agents, run on remote machines:
  k-terminus join my-laptop

# On a remote machine - join the orchestrator
$ k-terminus join my-laptop
Connecting to my-laptop.tailnet-abc.ts.net...
Connected! This machine is now available as "lab-server"

# Back on your main machine - list connected machines
$ k-terminus list
MACHINE          STATUS     SESSIONS  UPTIME
lab-gpu-01       connected  1         2d 7h
home-server      connected  0         12h 45m

# Connect to remote session
$ k-terminus connect lab-gpu-01
[lab-gpu-01:session-3] $
```

Both machines must be on the same Tailscale network. That's it - no OAuth setup, no manual key copying.

#### Advanced Usage

```bash
# List sessions with filtering
$ k-terminus list --machine lab-gpu-01
$ k-terminus list --tag gpu

# View connection health
$ k-terminus status
Orchestrator: running (pid 12345)
Connections: 3 active
Sessions: 3 active
Uptime: 2d 5h 12m

# Kill a session
$ k-terminus kill session-a1b2c3
Session terminated
```

---

## 7. Desktop Application (Tauri)

### 7.1 Architecture

The desktop GUI is built with Tauri 2.0, providing a native application with web-based UI. The app includes an embedded orchestrator that starts automatically when needed.

```
┌─────────────────────────────────────────────────────────────┐
│                    Tauri Application                        │
│  ┌────────────────────┐    ┌─────────────────────────────┐ │
│  │   Rust Backend     │    │      Web Frontend           │ │
│  │                    │    │                             │ │
│  │  • Embedded        │◄──►│  • React 18 + TypeScript    │ │
│  │    Orchestrator    │    │  • xterm.js terminals       │ │
│  │  • IPC client      │    │  • Tailwind CSS styling     │ │
│  │  • Event streaming │    │  • Zustand state mgmt       │ │
│  │  • Session relay   │    │  • react-resizable-panels   │ │
│  │                    │    │                             │ │
│  └────────────────────┘    └─────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

**Orchestrator Lifecycle:**
- On startup, checks if an orchestrator is already running (via IPC ping)
- If running, connects as a client
- If not running, starts an embedded orchestrator
- PID file management for daemon mode
- Graceful shutdown on app exit

### 7.2 Frontend Stack

| Technology | Purpose |
|------------|---------|
| **React 18** | Component framework |
| **TypeScript** | Type-safe JavaScript |
| **xterm.js** | Terminal emulation (GPU-accelerated via WebGL) |
| **Tailwind CSS** | Utility-first styling |
| **Zustand** | Lightweight state management with persist middleware |
| **React Flow** | Network topology visualization |
| **react-resizable-panels** | Pane splitting and resizing |
| **@tanstack/react-virtual** | Virtual scrolling for large lists |

### 7.3 UI Components

| View | Description |
|------|-------------|
| **Terminals** | Pane-based terminal interface with xterm.js, splittable horizontally/vertically |
| **Topology** | Visual graph showing orchestrator and connected machines |
| **Health** | Real-time metrics dashboard (uptime, connections, sessions) |
| **Logs** | Searchable log viewer with filtering |
| **Sidebar** | Machine list with virtual scrolling, clickable tag filtering |

### 7.3.1 Pane Layout System

The terminal view uses a recursive tree-based layout system for flexible pane management:

```
Layout Tree Example:
      split-h
      /     \
   pane1   split-v
           /     \
        pane2   pane3

Renders as:
┌─────────┬─────────┐
│         │  pane2  │
│  pane1  ├─────────┤
│         │  pane3  │
└─────────┴─────────┘
```

**Layout Features:**
- **Split panes**: Cmd+D (horizontal), Cmd+Shift+D (vertical)
- **Drag-and-drop**: Drag tabs to pane edges to create splits
- **Close panes**: Cmd+W closes the focused pane
- **Focus cycling**: Cmd+] / Cmd+[ to navigate between panes
- **Persistence**: Layout saved to localStorage and restored across sessions

**Cross-Platform Shortcuts:**
| Action | macOS | Windows/Linux |
|--------|-------|---------------|
| Split Right | `Cmd+D` | `Ctrl+Shift+D` |
| Split Down | `Cmd+Shift+D` | `Ctrl+Shift+Alt+D` |
| Close Pane | `Cmd+W` | `Ctrl+Shift+W` |
| Focus Next | `Cmd+]` | `Ctrl+Shift+]` |
| Focus Prev | `Cmd+[` | `Ctrl+Shift+[` |

Windows/Linux uses `Ctrl+Shift+` prefix to avoid conflicts with terminal shortcuts like `Ctrl+D` (EOF) and `Ctrl+W` (delete word).

### 7.4 Terminal Features

- Full ANSI color support (256 colors)
- GPU-accelerated rendering (WebGL addon)
- Clickable URLs (web-links addon)
- Auto-fit to container (fit addon)
- Copy/paste support
- Window resize handling
- Custom theme (Tokyo Night Dark)

### 7.5 Tauri Commands

| Command | Description |
|---------|-------------|
| `get_status` | Returns orchestrator status (uptime, connections, sessions) |
| `start_orchestrator` | Starts embedded orchestrator or connects to existing |
| `stop_orchestrator` | Stops orchestrator and disconnects all agents |
| `list_machines` | Returns connected machines with status |
| `create_session` | Creates PTY session on specified machine |
| `kill_session` | Terminates session and closes PTY |
| `terminal_write` | Sends input to session PTY |
| `terminal_resize` | Updates PTY window dimensions |
| `subscribe_to_session` | Starts streaming output for a session |
| `disconnect_machine` | Disconnects a specific agent |

### 7.6 Event System

Real-time updates via Tauri events:

| Event | Payload | Description |
|-------|---------|-------------|
| `machine-connected` | Machine info | Agent connected to orchestrator |
| `machine-disconnected` | Machine ID | Agent disconnected |
| `terminal-output` | `{session_id, data}` | PTY output bytes (Base64) |
| `session-closed` | Session ID | Session terminated |
| `ipc-error` | Error message | IPC communication failure |

---

## 8. Crate Structure

### 8.1 Workspace Organization

```
k-Terminus/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── kt-protocol/        # Wire protocol, message types
│   ├── kt-core/            # Shared types, config, Tailscale integration
│   ├── kt-orchestrator/    # Server: SSH, connection pool, state
│   ├── kt-agent/           # Client: tunnel, PTY management
│   └── kt-cli/             # CLI binary (k-terminus command)
└── apps/
    └── kt-desktop/         # Tauri desktop application
        ├── src/            # React frontend
        └── src-tauri/      # Rust backend
```

### 8.2 Crate Responsibilities

| Crate | Purpose |
|-------|---------|
| **kt-protocol** | Message types, frame codec, session IDs |
| **kt-core** | Config loading, Tailscale integration, setup utilities |
| **kt-orchestrator** | SSH server, connection pool, Tailscale peer verification |
| **kt-agent** | SSH client, PTY spawning, reconnection logic |
| **kt-cli** | CLI argument parsing, command implementations |
| **kt-desktop** | Tauri app, React UI, terminal rendering |

### 8.3 Dependency Graph

```
kt-cli ──────────────┐
                     ▼
kt-orchestrator ──► kt-core ──► kt-protocol
                     ▲
kt-agent ────────────┘

kt-desktop (Tauri)
  └── kt-core, kt-protocol
```

---

## 9. Future Enhancements

### 9.1 Version 1.0 (In Progress)

**High Impact:**
- Search in terminal: Cmd+F to search scrollback buffer
- Tab switching: Cmd+1-9 to jump to tab by number
- Font zoom: Cmd+/Cmd- to adjust terminal font size
- Tab renaming: Double-click tab to rename
- Settings panel: Light/dark/system theme toggle, font size

**Medium Impact:**
- Tab reordering: Drag tabs in tab bar to reorder
- Machine aliases: Rename machines from the UI (persisted to orchestrator)
- Activity indicator: Show output activity for background tabs
- Connection status: Show reconnecting progress
- Bell notification: System notification on terminal bell (optional)

**Polish:**
- Keyboard shortcuts help: Cmd+? shows shortcut cheatsheet
- About/version: Version info, update status
- Welcome/onboarding: First-run guidance for pairing

**Advanced:**
- Drag-to-upload: Drop file on pane to upload to remote cwd
- Session persistence: Sessions survive orchestrator restart

### 9.2 Post-1.0 Features

- Session sharing: Multiple users attach to same session (opt-in, read-only default)
- File download UI: Browse remote filesystem, download locally
- Bandwidth throttling: Rate limiting for constrained networks
- Health checks: Automated testing of remote machine availability
- Load balancing: Distribute sessions across machines with same tag

### 9.3 Ecosystem Integration

- Claude Code integration: First-class support as Claude Code transport
- VS Code extension: Terminal provider for remote development
- API server: REST/gRPC API for programmatic access
- Kubernetes operator: Manage distributed sessions in containerized environments

### 9.4 Features Not Implementing

| Feature | Reason |
|---------|--------|
| **Session recording** | Security risk: creates persistent artifacts containing passwords, secrets, and sensitive data |
| **Port forwarding** | Unnecessary: Tailscale provides `tailscale serve` and `tailscale funnel` for this purpose |
| **Full file transfer UI** | Standard tools (scp, rsync) work over Tailscale; minimal drag-to-upload is sufficient |

See [SECURITY.md](../../SECURITY.md) for detailed rationale.

---

## 10. Conclusion

k-Terminus addresses a critical gap in distributed development workflows by providing unified orchestration of terminal sessions across heterogeneous infrastructure. The reverse tunnel architecture solves real-world networking constraints while the multiplexing protocol ensures efficient resource utilization.

Built in Rust with Tauri, k-Terminus delivers native performance with memory safety guarantees. The modular architecture enables future extensibility while maintaining a clean separation of concerns between orchestration, transport, and presentation layers.

While primarily designed for managing Claude Code sessions across research infrastructure, k-Terminus is architected as a general-purpose tool suitable for any distributed terminal workflow. The combination of firewall-friendly reverse tunnels, persistent connections, and unified management makes it valuable for remote development, CI/CD orchestration, and multi-datacenter operations.

The name k-Terminus embodies the project's mathematical rigor (k-space, k-nearest neighbors) and classical foundations (Latin terminus), positioning it as professional infrastructure tooling for the modern era.

---

## Appendices

### Appendix A: Package Availability

Comprehensive verification confirms k-Terminus name availability across all major package managers:

- **crates.io (Rust)**: Available
- **npm**: Available
- **Homebrew**: Available
- **GitHub organization**: Available

Recommended distribution strategy:

- Primary: `cargo install k-terminus`
- macOS: `brew install k-terminus`
- Linux: Download binary from GitHub releases
- Desktop: Tauri app bundle (.dmg, .deb, .exe)

#### Complete Installation Flow

```bash
# On your main machine (orchestrator)
cargo install k-terminus
k-terminus serve          # Start accepting connections

# On remote machines (agents)
k-terminus join <your-device-name>   # Connect to orchestrator
```

Both machines must be on the same Tailscale network.

**Prerequisites:**
- Rust toolchain (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- Tailscale account (free at tailscale.com) - same account on all machines

### Appendix B: Alternative Names Considered

During naming research, numerous alternatives were evaluated and rejected due to namespace collisions or semantic mismatch. Notable rejections include:

- **Swarm**: Saturated (Docker Swarm, Zellij workspace, multiple agent frameworks)
- **Fleet**: Saturated (Kolide Fleet, serverless platforms)
- **HyperTerm**: Major collision with Vercel's popular terminal emulator
- **Terminus**: Saturated (Pantheon CLI, multiple emulators, Warp docs)
- **Span**: Taken (SPAN.io smart panel CLI)
- **Atlas**: Heavily saturated (MongoDB, O'Reilly, Intel, note-taking)
- **Fiber**: Taken (Go framework, Uber distributed computing)

The k- prefix successfully differentiates from these existing tools while maintaining the mathematical and terminal management semantics core to the project identity.
