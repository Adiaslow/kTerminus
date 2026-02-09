# Deployment Guide

This guide covers building, packaging, and distributing k-Terminus.

## Prerequisites

### Build Dependencies

- **Rust** 1.75+ with `cargo`
- **Node.js** 18+ with `pnpm`
- **Tauri CLI** 2.0+ (`cargo install tauri-cli`)

### Platform-Specific

**macOS:**
- Xcode Command Line Tools
- Code signing certificate (for distribution)

**Linux:**
- `libwebkit2gtk-4.1-dev`
- `libappindicator3-dev`
- `librsvg2-dev`

**Windows:**
- Visual Studio Build Tools 2019+
- WebView2 Runtime

## Building

### Development Build

```bash
# Build all crates
cargo build --workspace

# Build desktop app (dev mode)
cd apps/kt-desktop
pnpm install
pnpm tauri:dev
```

### Release Build

```bash
# Build optimized binaries
cargo build --workspace --release

# Build desktop app for distribution
cd apps/kt-desktop
pnpm tauri build
```

## Binaries

After a release build, binaries are located at:

| Binary | Path | Description |
|--------|------|-------------|
| `kt-orchestrator` | `target/release/kt-orchestrator` | Orchestrator daemon |
| `kt-agent` | `target/release/kt-agent` | Agent daemon |
| `k-terminus` | `target/release/k-terminus` | CLI tool |
| Desktop App | `apps/kt-desktop/src-tauri/target/release/bundle/` | Platform bundle |

## Configuration Files

Default locations:

| Platform | Config Directory |
|----------|-----------------|
| macOS | `~/.config/k-terminus/` |
| Linux | `~/.config/k-terminus/` |
| Windows | `%APPDATA%\k-terminus\` |

Key files:
- `config.toml` - Main configuration
- `ipc_auth_token` - IPC authentication token (auto-generated)
- `orchestrator.pid` - PID file for daemon mode
- `host_key` - SSH host key (auto-generated)

## Daemon Mode

### Running the Orchestrator

```bash
# Foreground (for debugging)
k-terminus serve

# Background daemon
k-terminus serve --daemon

# Check status
k-terminus status

# Stop daemon
k-terminus stop
```

### Systemd Service (Linux)

Create `/etc/systemd/system/k-terminus.service`:

```ini
[Unit]
Description=k-Terminus Orchestrator
After=network.target tailscaled.service

[Service]
Type=simple
User=your-username
ExecStart=/usr/local/bin/kt-orchestrator
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

Enable and start:

```bash
sudo systemctl daemon-reload
sudo systemctl enable k-terminus
sudo systemctl start k-terminus
```

### launchd Service (macOS)

Create `~/Library/LaunchAgents/com.k-terminus.orchestrator.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.k-terminus.orchestrator</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/kt-orchestrator</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/k-terminus.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/k-terminus.err</string>
</dict>
</plist>
```

Load the service:

```bash
launchctl load ~/Library/LaunchAgents/com.k-terminus.orchestrator.plist
```

## Agent Deployment

### Manual Installation

1. Copy `kt-agent` binary to remote machine
2. Run: `kt-agent --orchestrator <hostname>`

### Automated via SSH

```bash
# Copy agent binary
scp target/release/kt-agent user@remote:/usr/local/bin/

# Start agent
ssh user@remote "kt-agent --orchestrator my-laptop"
```

### Agent as Service

Similar to orchestrator, create a systemd/launchd service for the agent on remote machines.

## Desktop App Distribution

### macOS

Build creates `.app` bundle and `.dmg` installer:

```bash
cd apps/kt-desktop
pnpm tauri build
# Output: src-tauri/target/release/bundle/macos/k-Terminus.app
# Output: src-tauri/target/release/bundle/dmg/k-Terminus_*.dmg
```

For distribution:
1. Sign with Developer ID
2. Notarize with Apple
3. Staple notarization ticket

### Linux

Build creates `.deb`, `.rpm`, and `.AppImage`:

```bash
pnpm tauri build
# Output: src-tauri/target/release/bundle/deb/*.deb
# Output: src-tauri/target/release/bundle/rpm/*.rpm
# Output: src-tauri/target/release/bundle/appimage/*.AppImage
```

### Windows

Build creates `.exe` installer and `.msi`:

```bash
pnpm tauri build
# Output: src-tauri/target/release/bundle/msi/*.msi
# Output: src-tauri/target/release/bundle/nsis/*.exe
```

## Security Considerations

### IPC Authentication

The orchestrator generates a random authentication token on startup:
- Stored at `~/.config/k-terminus/ipc_auth_token`
- File permissions set to 600 (owner read/write only)
- Required for all IPC requests except health checks

### SSH Host Key

The orchestrator generates an SSH host key on first run:
- Stored at `~/.config/k-terminus/host_key`
- Used for agent authentication
- Protect this file carefully

### Tailscale Requirement

k-Terminus relies on Tailscale for:
- Network identity (no separate auth needed)
- Encrypted transport (WireGuard)
- NAT traversal

Ensure Tailscale is installed and authenticated on all machines.

## Troubleshooting

### Common Issues

**Port already in use:**
```bash
# Check what's using the port
lsof -i :2222
# Kill stale process or change port in config
```

**IPC connection refused:**
```bash
# Check if orchestrator is running
k-terminus status
# Check IPC port
netstat -an | grep 22230
```

**Agent can't connect:**
```bash
# Verify Tailscale status
tailscale status
# Check orchestrator logs
journalctl -u k-terminus -f
```

### Logs

| Component | Log Location |
|-----------|--------------|
| Orchestrator | stderr / journald |
| Agent | stderr / journald |
| Desktop App | `~/.k-terminus/logs/` |

Enable debug logging:
```bash
RUST_LOG=debug kt-orchestrator
```

## Updates

### Manual Update

1. Stop running services
2. Replace binaries
3. Restart services

### Rolling Updates

For agents, you can update them one at a time:
1. Stop agent on remote machine
2. Replace binary
3. Start agent (auto-reconnects)

The orchestrator maintains connections while agents reconnect.
