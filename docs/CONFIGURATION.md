# k-Terminus Configuration Reference

k-Terminus works out of the box with sensible defaults. Configuration is optional.

## Config File Location

| Platform | Path |
|----------|------|
| macOS | `~/Library/Application Support/k-terminus/config.toml` |
| Linux | `~/.config/k-terminus/config.toml` |
| Windows | `%APPDATA%\k-terminus\config.toml` |

Use `k-terminus config path` to see the config directory on your system.

## Configuration Commands

```bash
# Show current configuration
k-terminus config show

# Show config directory path
k-terminus config path

# Edit config in default editor
k-terminus config edit

# Get a specific value
k-terminus config get orchestrator.bind_address

# Set a value
k-terminus config set orchestrator.heartbeat_interval 60
```

## Orchestrator Configuration

```toml
[orchestrator]
# Address and port to listen for agent connections
# Default: "127.0.0.1:2222" (localhost only for security)
# Use "0.0.0.0:2222" to accept connections from the network
bind_address = "127.0.0.1:2222"

# Port for IPC (CLI/desktop communication) - localhost only
# Default: 22230
ipc_port = 22230

# Path to SSH host key (auto-generated if missing)
# Default: <config_dir>/host_key
host_key_path = "~/.config/k-terminus/host_key"

# Paths to authorized public keys (deprecated - Tailscale is required)
# Note: This setting is no longer used. All authentication is via Tailscale.
# Default: []
auth_keys = []

# Seconds between heartbeat pings to agents
# Default: 30
heartbeat_interval = 30

# Seconds to wait before considering a connection dead
# Default: 90
heartbeat_timeout = 90

# Maximum concurrent agent connections (optional)
# Limits the number of remote machines that can connect simultaneously.
# When the limit is reached, new connections are rejected with an error.
# Set to limit resource usage or for security hardening.
# Default: unlimited (no limit)
max_connections = 100

# Maximum sessions per machine (optional)
# Limits the number of terminal sessions that can be created on a single machine.
# Prevents resource exhaustion from too many PTY processes.
# When exceeded, new session requests return SessionLimitExceeded error.
# Default: unlimited (no limit)
max_sessions_per_machine = 10

# Tailscale hostname (auto-detected, rarely needs manual setting)
# tailscale_hostname = "my-laptop.tailnet-abc.ts.net"
```

## Backoff Configuration

Controls reconnection behavior for agents.

```toml
[orchestrator.backoff]
# Initial delay before first retry (seconds)
# Default: 1
initial = 1

# Maximum delay between retries (seconds)
# Default: 60
max = 60

# Multiplier for each retry (exponential backoff)
# Default: 2.0
multiplier = 2.0

# Random jitter factor (0.0 to 1.0)
# Default: 0.25
jitter = 0.25
```

## Machine Profiles

Define default settings for specific machines.

```toml
[machines.gpu-server]
# Human-readable alias
alias = "lab-gpu-01"

# Tags for filtering
tags = ["gpu", "compute", "lab"]

# Default shell to spawn
default_shell = "/bin/bash"

# Environment variables for sessions
[machines.gpu-server.env]
CUDA_VISIBLE_DEVICES = "0,1"
```

## Agent Configuration

Agent configuration is typically passed via CLI flags, but can also be set in config.

```toml
[agent]
# Orchestrator to connect to
# Usually passed via: k-terminus join <orchestrator>
orchestrator_address = "my-laptop.tailnet-abc.ts.net:2222"

# Path to private key (auto-generated if missing)
# Default: <config_dir>/agent_key
private_key_path = "~/.config/k-terminus/agent_key"

# Machine alias (defaults to hostname)
# alias = "my-machine"

# Default shell for sessions
# default_shell = "/bin/zsh"

# Connection timeout in seconds
# Default: 30
connect_timeout = 30

# Maximum concurrent sessions
# max_sessions = 10
```

## Full Example

```toml
# k-Terminus Configuration
# Location: ~/.config/k-terminus/config.toml

[orchestrator]
# Use 0.0.0.0:2222 to accept network connections
bind_address = "127.0.0.1:2222"
ipc_port = 22230
heartbeat_interval = 30
heartbeat_timeout = 90

[orchestrator.backoff]
initial = 1
max = 60
multiplier = 2.0
jitter = 0.25

# GPU Server Profile
[machines.gpu-server]
alias = "lab-gpu"
tags = ["gpu", "compute"]
default_shell = "/bin/bash"

[machines.gpu-server.env]
CUDA_VISIBLE_DEVICES = "0,1,2,3"

# Build Server Profile
[machines.build-server]
alias = "ci-builder"
tags = ["ci", "build"]

[machines.build-server.env]
CI = "true"
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `K_TERMINUS_CONFIG` | Override config file path |
| `RUST_LOG` | Set log level (e.g., `debug`, `trace`) |

## CLI Flag Overrides

Most config options can be overridden via CLI flags:

```bash
# Override bind address
k-terminus serve --bind 0.0.0.0:3333

# Override config file
k-terminus --config /path/to/config.toml serve

# Verbose logging
k-terminus serve -vv
```

## Data Files

In addition to the config file, k-Terminus stores:

| File | Description |
|------|-------------|
| `host_key` | SSH host key (Ed25519) |
| `agent_key` | Agent's SSH private key |
| `agent_key.pub` | Agent's SSH public key |
| `ipc_auth_token` | IPC authentication token (mode 600) |
| `orchestrator.pid` | PID file when running as daemon |

These are auto-generated on first run. The `ipc_auth_token` is regenerated each time the orchestrator starts.
