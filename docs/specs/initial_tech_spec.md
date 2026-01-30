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
8. [Future Enhancements](#7-future-enhancements)
9. [Conclusion](#8-conclusion)
10. [Appendices](#appendices)

---

## Executive Summary

k-Terminus is a distributed terminal session manager designed to orchestrate command-line environments across heterogeneous infrastructure through reverse SSH tunnels. Built in Rust with Tauri for cross-platform desktop deployment, k-Terminus enables unified management of multiple remote machines from a single local orchestrator.

The primary use case is managing distributed Claude Code sessions across lab servers, development machines, and research infrastructure. However, k-Terminus is architected as a general-purpose tool suitable for any workflow requiring coordinated terminal access across multiple systems.

### Key Features

- Reverse tunnel architecture (client-initiated, firewall-friendly)
- Session multiplexing over single tunnel per machine
- Persistent connections with automatic reconnection
- Unified CLI for all connected machines
- Cross-platform desktop GUI via Tauri
- Zero-dependency single binary distribution

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
2. Orchestrator validates client public key against authorized_keys
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
2. **Authentication**: Public key authentication against orchestrator's authorized_keys
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

- **SessionCreate**: Request new PTY allocation
- **SessionReady**: Acknowledge session creation with PTY details
- **Data**: Stdin/stdout/stderr streams
- **Resize**: Window dimension changes (rows, cols)
- **SessionClose**: Terminate session
- **Heartbeat**: Keep-alive ping/pong

### 4.3 Authentication & Security

#### Authentication Mechanism

- Public key authentication only (no password support)
- Client machines configured with SSH key pair
- Orchestrator maintains authorized_keys file
- Each machine identified by public key fingerprint

#### Security Considerations

- SSH provides transport encryption (all data encrypted in transit)
- Host key verification prevents MITM attacks
- Orchestrator only accepts connections from known machines
- No credential storage on remote machines (key-based auth only)
- Session isolation: Each session runs in separate PTY with distinct process context

---

## 5. Configuration Management

### 5.1 Configuration File Structure

Configuration stored in TOML format at `~/.config/k-terminus/config.toml` with hierarchical organization.

#### Orchestrator Configuration

- **bind_address**: Listen address for reverse tunnel connections (default: `0.0.0.0:2222`)
- **auth_keys**: List of authorized public key paths
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
bind_address = "0.0.0.0:2222"
auth_keys = [
  "~/.ssh/k-terminus_ed25519.pub"
]
heartbeat_interval = 30

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
| `k-terminus start` | Start orchestrator daemon and begin accepting connections |
| `k-terminus list` | Display all connected machines and active sessions with status |
| `k-terminus connect <machine>` | Create new session on specified machine and attach terminal |
| `k-terminus attach <session>` | Attach terminal to existing session by session_id |
| `k-terminus exec <machine> <cmd>` | Execute one-off command on machine and return output |
| `k-terminus status` | Show orchestrator status and connection health metrics |
| `k-terminus kill <session>` | Terminate specified session |
| `k-terminus logs` | Display orchestrator logs and connection events |

### 6.2 Command Examples

#### Basic Workflow

```bash
# Start orchestrator
$ k-terminus start
Orchestrator started on 0.0.0.0:2222

# List connected machines
$ k-terminus list
MACHINE          STATUS     SESSIONS  UPTIME
macbook          connected  2         5h 23m
lab-gpu-01       connected  1         2d 7h
ci-builder       connected  0         12h 45m

# Connect to new session
$ k-terminus connect lab-gpu-01
[lab-gpu-01:session-3] $

# Execute one-off command
$ k-terminus exec ci-builder 'git status'
On branch main
Your branch is up to date with 'origin/main'.
```

#### Advanced Usage

```bash
# List sessions with filtering
$ k-terminus list --machine lab-gpu-01
$ k-terminus list --tag gpu

# Attach to specific session
$ k-terminus attach lab-gpu-01:session-1

# View connection health
$ k-terminus status
Orchestrator: running (pid 12345)
Connections: 3 active
Sessions: 3 active
Uptime: 2d 5h 12m
```

---

## 7. Future Enhancements

### 7.1 Phase 1 (Post-MVP)

- Session persistence: Reconnect to sessions after orchestrator restart
- Session recording: Capture terminal output for replay/audit
- Bandwidth throttling: Rate limiting for constrained networks
- Advanced filtering: Query machines by capabilities, load, tags
- Health checks: Automated testing of remote machine availability

### 7.2 Phase 2 (Advanced Features)

- Session sharing: Multiple users attach to same session (tmux-style)
- File transfer: SCP-like functionality over tunnels
- Port forwarding: Expose remote services through tunnel
- Load balancing: Distribute sessions across multiple machines with same tag
- Metrics collection: Resource usage tracking (CPU, memory, network)

### 7.3 Phase 3 (Ecosystem Integration)

- Claude Code integration: First-class support as Claude Code transport
- VS Code extension: Terminal provider for remote development
- API server: REST/gRPC API for programmatic access
- Plugin system: Extensibility for custom transport protocols
- Kubernetes operator: Manage distributed sessions in containerized environments

---

## 8. Conclusion

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
