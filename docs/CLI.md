# k-Terminus CLI Reference

Complete reference for the `k-terminus` command-line interface.

## Synopsis

```
k-terminus [OPTIONS] <COMMAND>
```

## Global Options

| Option | Description |
|--------|-------------|
| `-c, --config <PATH>` | Path to configuration file |
| `-v, --verbose` | Increase verbosity (can repeat: `-v`, `-vv`, `-vvv`) |
| `-q, --quiet` | Suppress all output except errors |
| `-h, --help` | Print help information |
| `-V, --version` | Print version information |

## Commands

### serve

Start the orchestrator daemon. Accepts connections from agents.

```bash
k-terminus serve [OPTIONS]
```

**Options:**
| Option | Description |
|--------|-------------|
| `-f, --foreground` | Run in foreground (don't daemonize) |
| `-b, --bind <ADDRESS>` | Bind address (overrides config) |

**Examples:**
```bash
# Start as daemon
k-terminus serve

# Run in foreground with debug output
k-terminus serve --foreground -vv

# Bind to specific address
k-terminus serve --bind 0.0.0.0:3333
```

**Alias:** `start`

---

### stop

Stop the running orchestrator daemon.

```bash
k-terminus stop
```

**Examples:**
```bash
k-terminus stop
```

---

### join

Connect to an orchestrator as an agent. This machine becomes available for remote sessions.

```bash
k-terminus join <ORCHESTRATOR> [OPTIONS]
```

**Arguments:**
| Argument | Description |
|----------|-------------|
| `ORCHESTRATOR` | Orchestrator hostname (Tailscale name or full address) |

**Options:**
| Option | Description |
|--------|-------------|
| `--alias <NAME>` | Machine alias (defaults to hostname) |
| `-k, --key <PATH>` | Path to private key (auto-generated if not specified) |
| `-f, --foreground` | Run in foreground (don't daemonize) |

**Examples:**
```bash
# Connect using short Tailscale name
k-terminus join my-laptop

# Connect with full hostname
k-terminus join my-laptop.tailnet-abc.ts.net:2222

# Set custom alias
k-terminus join my-laptop --alias "home-server"

# Run in foreground
k-terminus join my-laptop --foreground
```

**Alias:** `agent`

---

### list

List connected machines and active sessions.

```bash
k-terminus list [OPTIONS]
```

**Options:**
| Option | Description |
|--------|-------------|
| `-m, --machine <NAME>` | Filter by machine name/alias |
| `-t, --tag <TAG>` | Filter by tag (can repeat) |
| `-l, --long` | Show detailed information |

**Examples:**
```bash
# List all machines
k-terminus list

# Show detailed view
k-terminus list --long

# Filter by machine
k-terminus list --machine gpu-server

# Filter by tag
k-terminus list --tag gpu --tag compute
```

---

### connect

Create a new terminal session on a machine and attach to it.

```bash
k-terminus connect <MACHINE> [OPTIONS]
```

**Arguments:**
| Argument | Description |
|----------|-------------|
| `MACHINE` | Machine identifier (name, alias, or ID) |

**Options:**
| Option | Description |
|--------|-------------|
| `-s, --shell <SHELL>` | Shell to spawn (overrides machine default) |

**Examples:**
```bash
# Connect to machine
k-terminus connect gpu-server

# Specify shell
k-terminus connect gpu-server --shell /bin/zsh
```

---

### attach

Attach to an existing session.

```bash
k-terminus attach <SESSION>
```

**Arguments:**
| Argument | Description |
|----------|-------------|
| `SESSION` | Session ID to attach to |

**Examples:**
```bash
k-terminus attach session-a1b2c3
```

---

### status

Show orchestrator status and health information.

```bash
k-terminus status [OPTIONS]
```

**Options:**
| Option | Description |
|--------|-------------|
| `-d, --detailed` | Show detailed health metrics |

**Examples:**
```bash
# Quick status
k-terminus status

# Detailed health metrics
k-terminus status --detailed
```

---

### kill

Terminate one or more sessions.

```bash
k-terminus kill <SESSION>... [OPTIONS]
```

**Arguments:**
| Argument | Description |
|----------|-------------|
| `SESSION` | Session ID(s) to kill |

**Options:**
| Option | Description |
|--------|-------------|
| `-f, --force` | Skip confirmation prompt |

**Examples:**
```bash
# Kill single session
k-terminus kill session-a1b2c3

# Kill multiple sessions
k-terminus kill session-a1b2c3 session-d4e5f6

# Force kill without confirmation
k-terminus kill session-a1b2c3 --force
```

---

### config

Manage configuration.

```bash
k-terminus config <ACTION>
```

**Subcommands:**

#### config show
Display current configuration.
```bash
k-terminus config show
```

#### config get
Get a specific config value.
```bash
k-terminus config get <KEY>

# Example
k-terminus config get orchestrator.bind_address
```

#### config set
Set a config value.
```bash
k-terminus config set <KEY> <VALUE>

# Example
k-terminus config set orchestrator.heartbeat_interval 60
```

#### config edit
Open config file in default editor.
```bash
k-terminus config edit
```

#### config path
Print config directory path.
```bash
k-terminus config path
```

---

## Exit Codes

| Code | Description |
|------|-------------|
| 0 | Success |
| 1 | General error |
| 2 | Configuration error |
| 3 | Connection error |

## Environment Variables

| Variable | Description |
|----------|-------------|
| `K_TERMINUS_CONFIG` | Override config file path |
| `RUST_LOG` | Set log level (`error`, `warn`, `info`, `debug`, `trace`) |
| `EDITOR` | Editor for `config edit` |

## Examples

### Basic Workflow

```bash
# Terminal 1: Start orchestrator
k-terminus serve

# Terminal 2 (remote machine): Join as agent
k-terminus join my-laptop

# Terminal 1: List machines
k-terminus list

# Terminal 1: Connect to remote machine
k-terminus connect home-server
```

### Debugging

```bash
# Run orchestrator with debug logging
RUST_LOG=debug k-terminus serve --foreground

# Run agent with trace logging
k-terminus join my-laptop --foreground -vvv
```

### Quick Status Check

```bash
# Just run k-terminus with no arguments
k-terminus

#   k-Terminus - Remote Terminal Access via Tailscale
#
#   Tailscale: ● my-laptop (100.64.1.1)
#   Orchestrator: ● Running
#   Machines: 2
#   Sessions: 1
```
