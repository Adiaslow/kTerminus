# k-Terminus

**Seamless multi-machine terminal access via Tailscale.**

Manage terminal sessions across all your machines from one place - no manual SSH key copying, no port forwarding, no complex configuration.

## Why k-Terminus?

I'm a computational researcher working across three labs, two universities, and a biotech company. Nine machines. Different operating systems. Different networks. Different buildings. One is underground in a vivarium.

Every day I commute 1-2 hours each way with a 50 lb backpack—two laptops, because company policy requires work to run on their hardware.

One machine lives past an elevator, four badge doors, an air shower, and three airlocks. When the IP changes, I make that entire journey just to read it off the screen. For four numbers.

I travel constantly—campus, coffee shops, hotels, home—each network with its own firewall blocking what I need. I work with unpublished research and biotech data under NDA, so "just open the port" isn't an option. And enterprise tools with per-seat pricing? Not in my budget.

**So I built k-Terminus.**

Tailscale gives me one secure network across all my machines—stable hostnames, no IP hunting, connections that work through any firewall. k-Terminus gives me one terminal to manage them all.

Now the vivarium machine is just `vivarium-server`. The work laptop stays home while I access it from anywhere. My backpack is lighter.

It's **free**, because researchers shouldn't have to choose between tools and groceries.

It's **secure**—WireGuard encryption, SSO authentication, zero open ports to the internet. Tailscale solved the hard problems; k-Terminus just requires you to be on the same tailnet. No passwords. No certificates. No exposed attack surface.

It's **open source**, because I'm a scientist and I believe knowledge should be shared.

**Tailscale (free) + k-Terminus (free) = secure access to all your machines, from anywhere, for $0.**

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

### Installation

**Quick install (macOS/Linux):**
```bash
curl -sSL https://raw.githubusercontent.com/Adiaslow/kTerminus/main/install.sh | bash
```

**Homebrew (macOS):**
```bash
brew install Adiaslow/tap/k-terminus
```

**Download binaries:**

Visit the [Releases](https://github.com/Adiaslow/kTerminus/releases) page and download:
- **CLI:** `k-terminus-<platform>.tar.gz` (or `.zip` for Windows)
- **Desktop App:** `.dmg` (macOS), `.deb`/`.AppImage` (Linux), `.msi` (Windows)

**Cargo (Rust):**
```bash
cargo install k-terminus
```

**From source:**
```bash
git clone https://github.com/Adiaslow/kTerminus
cd kTerminus
cargo install --path crates/kt-cli
```

### Usage

**On your main machine (orchestrator):**
```bash
$ k-terminus serve

  k-Terminus Orchestrator

  Listening on: my-laptop.tailb54f12.ts.net:2222

  To connect agents, run on remote machines:
    k-terminus join my-laptop
```

**On remote machines (agents):**
```bash
$ k-terminus join my-laptop
Connecting to my-laptop.tailb54f12.ts.net:2222 via Tailscale...
Connected as 'home-server'
```

**Back on your main machine:**
```bash
# List connected machines
$ k-terminus list
MACHINES
┌────────────────┬───────────┬──────────┬─────────┐
│ ID             │ Hostname  │ Status   │ Sessions│
├────────────────┼───────────┼──────────┼─────────┤
│ home-server    │ home-srv  │ connected│ 0       │
│ cloud-vm       │ ubuntu-vm │ connected│ 1       │
└────────────────┴───────────┴──────────┴─────────┘

# Connect to a machine
$ k-terminus connect home-server
[home-server] $ whoami
adam
[home-server] $
```

That's it. No SSH keys to copy, no config files to edit.

## Commands

| Command | Description |
|---------|-------------|
| `k-terminus serve` | Start orchestrator (accepts agent connections) |
| `k-terminus stop` | Stop orchestrator |
| `k-terminus join <host>` | Connect to orchestrator as agent |
| `k-terminus list` | List connected machines and sessions |
| `k-terminus connect <machine>` | Open terminal to machine |
| `k-terminus attach <session>` | Attach to existing session |
| `k-terminus status` | Show orchestrator status |
| `k-terminus kill <session>` | Terminate a session |
| `k-terminus config show` | Show current configuration |

Run `k-terminus --help` or `k-terminus <command> --help` for details.

## Desktop App

k-Terminus includes a native desktop application built with Tauri 2.0:

- **Sidebar** showing all connected machines
- **Tabbed terminal panes** for multiple sessions
- **Real-time updates** via IPC connection to orchestrator
- **Cross-platform:** macOS, Linux, Windows

Download from the [Releases](https://github.com/Adiaslow/kTerminus/releases) page.

## Configuration

Configuration is optional. k-Terminus works out of the box with sensible defaults.

**Config location:**
- macOS: `~/Library/Application Support/k-terminus/config.toml`
- Linux: `~/.config/k-terminus/config.toml`
- Windows: `%APPDATA%\k-terminus\config.toml`

**Example config:**
```toml
[orchestrator]
bind_address = "0.0.0.0:2222"
heartbeat_interval = 30
heartbeat_timeout = 90

[orchestrator.backoff]
initial = 1
max = 60
multiplier = 2.0
```

See [docs/CONFIGURATION.md](docs/CONFIGURATION.md) for full reference.

## Security

k-Terminus uses multiple layers of security:

| Layer | Mechanism |
|-------|-----------|
| **Network** | Tailscale WireGuard encryption |
| **Identity** | Tailscale device identity (same tailnet = trusted) |
| **Transport** | SSH protocol |
| **Keys** | Auto-generated Ed25519 keys |

**Trust model:** Being on the same Tailscale network is the trust boundary. If a device is in your tailnet, it's trusted. This means:

- No manual SSH key distribution needed
- No OAuth flows or verification codes
- Removing a device from Tailscale revokes access

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

## Project Structure

```
kTerminus/
├── crates/
│   ├── kt-protocol/     # Wire protocol for session multiplexing
│   ├── kt-core/         # Shared types, config, Tailscale integration
│   ├── kt-orchestrator/ # Daemon accepting agent connections
│   ├── kt-agent/        # Remote agent with PTY management
│   └── kt-cli/          # Command-line interface (k-terminus binary)
└── apps/
    └── kt-desktop/      # Tauri 2.0 desktop application
```

## Development

```bash
# Build all crates
cargo build --workspace

# Run tests
cargo test --workspace -- --test-threads=1

# Run orchestrator in foreground with debug logging
k-terminus serve --foreground -vv

# Run agent in foreground
k-terminus join <orchestrator> --foreground
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for development guide.

## Responsible Use

k-Terminus is designed for **legitimate system administration** of machines you own or are authorized to manage.

**This software must NOT be used for:**
- Unauthorized access to any computer system
- Deploying on machines without explicit owner consent
- Any illegal activity

See [NOTICE](NOTICE) for full ethical use guidelines.

## License

MIT License - See [LICENSE](LICENSE)
