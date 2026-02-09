# k-Terminus

**Terminal multiplexer for managing sessions across multiple machines via Tailscale.**

k-Terminus enables you to access and manage terminal sessions on any machine in your Tailnet from a single interface. It eliminates SSH key management, port forwarding, and firewall configuration by leveraging Tailscale's secure mesh network.

## Features

- **Multi-machine access** - Connect to any machine on your Tailnet
- **Session persistence** - Sessions stay alive when you disconnect
- **Zero configuration** - Works out of the box with Tailscale
- **Desktop and CLI** - GUI application or command-line interface
- **Secure by default** - WireGuard encryption, Tailscale authentication
- **Free and open source** - MIT licensed

## How It Works

```
┌─────────────────────────────────────────────────────────────────┐
│                     TAILSCALE NETWORK                           │
│                   (Encrypted WireGuard)                         │
│                                                                 │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐      │
│  │ Your Laptop  │    │ Home Server  │    │ Cloud VM     │      │
│  │ (Orchestrator)│◄──│   (Agent)    │    │   (Agent)    │      │
│  │              │◄───┼──────────────┼────┤              │      │
│  │    :2222     │    │              │    │              │      │
│  └──────────────┘    └──────────────┘    └──────────────┘      │
│                                                                 │
│  k-terminus serve    k-terminus join    k-terminus join        │
│                      my-laptop          my-laptop              │
└─────────────────────────────────────────────────────────────────┘
```

## Quick Start

### Prerequisites

1. **Install Tailscale** on all machines: https://tailscale.com/download
2. **Log in** with the same account on all machines: `sudo tailscale up`

**System Requirements:**
- macOS 10.15+, Linux (kernel 3.10+), or Windows 10+
- Tailscale 1.32+ recommended

### Installation

```bash
git clone https://github.com/Adiaslow/kTerminus
cd kTerminus
cargo install --path crates/kt-cli
```

The `k-terminus` binary will be in `~/.cargo/bin/` (add to PATH if needed).

**Desktop App:**
```bash
just app  # Build and open desktop app
```

For development builds, see [CONTRIBUTING.md](CONTRIBUTING.md).

### Usage

**1. Start the orchestrator on your main machine:**
```bash
$ k-terminus serve

  k-Terminus Orchestrator

  Listening on: macbook-pro.tailb54f12.ts.net:2222

  Pairing Code: ABC123XY

  To connect agents, run on remote machines:
    k-terminus join ABC123XY      # using pairing code
    k-terminus join macbook-pro   # using hostname
```

**2. Connect agents from remote machines:**
```bash
$ k-terminus join macbook-pro -a home-server
ℹ Connecting to macbook-pro.tailb54f12.ts.net:2222 via Tailscale...
✓ Connected as 'home-server'
```

**3. List connected machines:**
```bash
$ k-terminus list
Connected Machines:
╭──────────────┬─────────────┬──────────────────┬───────┬───────────┬──────────╮
│ ID           │ ALIAS       │ HOSTNAME         │ OS    │ STATUS    │ SESSIONS │
├──────────────┼─────────────┼──────────────────┼───────┼───────────┼──────────┤
│ local-dev1   │ home-server │ ubuntu-server    │ linux │ connected │ 0        │
│ local-dev2   │ cloud-vm    │ debian-vm        │ linux │ connected │ 1        │
╰──────────────┴─────────────┴──────────────────┴───────┴───────────┴──────────╯

Active Sessions:
╭───────────┬──────────────┬─────────┬───────┬─────────────╮
│ SESSION   │ MACHINE      │ SHELL   │ PID   │ CREATED     │
├───────────┼──────────────┼─────────┼───────┼─────────────┤
│ session-1 │ local-dev2   │ default │ 42315 │ 1770259899Z │
╰───────────┴──────────────┴─────────┴───────┴─────────────╯
```

**4. Connect to a machine:**
```bash
$ k-terminus connect home-server
ℹ Creating session on 'home-server'...
✓ Session created: session-2 (PID: pending)
ℹ Attaching to session... (Press Ctrl+] to detach)

[home-server] $ whoami
adam
[home-server] $
```

No SSH keys to manage, no port forwarding to configure.

## Commands

| Command | Description |
|---------|-------------|
| `k-terminus serve` | Start orchestrator (alias: `start`) |
| `k-terminus stop` | Stop orchestrator |
| `k-terminus join <host>` | Connect to orchestrator as agent (alias: `agent`) |
| `k-terminus list` | List connected machines and active sessions |
| `k-terminus connect <machine>` | Create new session and attach to machine |
| `k-terminus attach <session>` | Attach to existing session |
| `k-terminus status` | Show orchestrator status and health |
| `k-terminus kill <session>` | Terminate a session |
| `k-terminus config` | Manage configuration (show, edit, get, set) |

**Options:**
- `-a, --alias` - Set machine alias when joining
- `-f, --foreground` - Run in foreground (serve/join)
- `-v, --verbose` - Increase logging verbosity
- `-c, --config` - Use custom config file

See `k-terminus <command> --help` for detailed options.

## Desktop App

k-Terminus includes a native desktop application built with Tauri 2.0:

- **Sidebar** showing all connected machines with clickable tag filtering
- **Terminal pane splitting** - split horizontally or vertically for side-by-side terminals
- **Drag-and-drop** - drag tabs to pane edges to create splits
- **Tabbed interface** with multiple sessions per machine
- **Real-time updates** via IPC connection to orchestrator
- **Cross-platform:** macOS, Linux, Windows

### Keyboard Shortcuts

| Action | macOS | Windows/Linux |
|--------|-------|---------------|
| Split Right | `Cmd+D` | `Ctrl+Shift+D` |
| Split Down | `Cmd+Shift+D` | `Ctrl+Shift+Alt+D` |
| Close Pane | `Cmd+W` | `Ctrl+Shift+W` |
| Focus Next Pane | `Cmd+]` | `Ctrl+Shift+]` |
| Focus Prev Pane | `Cmd+[` | `Ctrl+Shift+[` |

Standard terminal shortcuts (`Ctrl+C`, `Ctrl+D`, etc.) work as expected.

Run with `just app` from the project root.

## Configuration

Configuration is optional. k-Terminus works out of the box with sensible defaults.

**Config location:**
- macOS: `~/Library/Application Support/k-terminus/config.toml`
- Linux: `~/.config/k-terminus/config.toml`
- Windows: `%APPDATA%\k-terminus\config.toml`

**Example config:**
```toml
[orchestrator]
# Default is 127.0.0.1:2222 (localhost only)
# Use 0.0.0.0:2222 for network access
bind_address = "0.0.0.0:2222"
heartbeat_interval = 30
heartbeat_timeout = 90

[orchestrator.backoff]
initial = 1
max = 60
multiplier = 2.0
```

See [docs/CONFIGURATION.md](docs/CONFIGURATION.md) for full reference.

## Documentation

- **[FAQ](docs/FAQ.md)** - Frequently asked questions
- **[Configuration Guide](docs/CONFIGURATION.md)** - Complete configuration reference
- **[CLI Reference](docs/CLI.md)** - All commands and options
- **[Security Model](SECURITY.md)** - Threat model and security practices
- **[Architecture](docs/ARCHITECTURE.md)** - Technical architecture and design
- **[Deployment Guide](DEPLOYMENT.md)** - Production deployment and service setup

## Security

k-Terminus uses multiple layers of security:

| Layer | Mechanism |
|-------|-----------|
| **Network** | Tailscale WireGuard encryption |
| **Identity** | Tailscale device identity (same tailnet = trusted) |
| **Transport** | SSH protocol |
| **Keys** | Auto-generated Ed25519 keys |
| **Input Validation** | Size limits on session input (64KB) and protocol frames (16MB) |
| **Session Isolation** | Sessions bound to owning machine, cleaned up on disconnect |
| **Resource Limits** | Configurable max connections and sessions per machine |

**Trust model:** Being on the same Tailscale network is the trust boundary. If a device is in your tailnet, it's trusted. This means:

- No manual SSH key distribution needed
- No OAuth flows or verification codes
- Removing a device from Tailscale revokes access

**Hardening options:**

```toml
[orchestrator]
max_connections = 100           # Limit concurrent agent connections
max_sessions_per_machine = 10   # Limit sessions per machine
```

For details, see [SECURITY.md](SECURITY.md).

## Troubleshooting

### "Tailscale is not installed"
Install Tailscale from https://tailscale.com/download and run `sudo tailscale up`.

### "Tailscale is not logged in"
Run `sudo tailscale up` and complete authentication.

### "Connection refused"
1. Ensure the orchestrator is running: `k-terminus status`
2. Ensure both machines are on the same Tailscale network: `tailscale status`
3. Check the orchestrator is listening: `k-terminus serve --foreground -v`

### "Not in Tailscale network"
The connecting machine must be on the same Tailscale network as the orchestrator. Check `tailscale status` on both machines.

## Contributing

```bash
cargo build --workspace  # Build everything
cargo test --workspace   # Run tests
just dev                 # Desktop app with hot reload
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for details.

## Responsible Use

k-Terminus is designed for **legitimate system administration** of machines you own or are authorized to manage.

**This software must NOT be used for:**
- Unauthorized access to any computer system
- Deploying on machines without explicit owner consent
- Any illegal activity

See [NOTICE](NOTICE) for full ethical use guidelines.

## License

MIT License - See [LICENSE](LICENSE)
