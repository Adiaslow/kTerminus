# Frequently Asked Questions

## General

### What is k-Terminus?

k-Terminus is a terminal multiplexer that lets you manage sessions across multiple machines from one interface. It works exclusively over Tailscale for secure, zero-configuration connectivity.

### How is this different from tmux/screen?

tmux and screen multiplex sessions on a *single* machine. k-Terminus multiplexes sessions across *multiple* machines. Think of it as tmux for your entire network.

You can use k-Terminus *with* tmux - start a tmux session on a remote machine through k-Terminus.

### How is this different from SSH?

k-Terminus uses Tailscale for network connectivity, eliminating SSH key management, port forwarding, and firewall configuration. All machines on your Tailnet can connect automatically.

### Do I need to open any ports?

No. Tailscale handles all networking through WireGuard tunnels. No ports need to be opened on any machine.

### Can I use this without Tailscale?

No. k-Terminus is built specifically for Tailscale networks and relies on Tailscale's authentication and encrypted connectivity.

## Setup & Installation

### How do I install k-Terminus?

See the [Installation section](../README.md#installation) in the README. Quick install:

```bash
curl -fsSL https://raw.githubusercontent.com/Adiaslow/kTerminus/main/install.sh | sh
```

Or use Homebrew, cargo, or download from releases.

### Which machine should run the orchestrator?

Run the orchestrator on the machine you use most (typically your laptop or desktop). This is the central hub that agents connect to.

You can run only one orchestrator at a time.

### Can I run multiple orchestrators?

No. Only one orchestrator can run at a time. If you try to start a second orchestrator, it will detect the existing one and refuse to start.

If you need to switch machines, stop the orchestrator on the first machine before starting it on another.

### Do I need to install anything on remote machines?

Just the k-terminus CLI binary. Run `k-terminus join <orchestrator>` to connect the machine as an agent.

## Usage

### How do I connect a new machine?

On the orchestrator:
```bash
k-terminus serve
```

On the remote machine:
```bash
k-terminus join my-laptop
# Or use the pairing code shown when orchestrator starts:
k-terminus join ABC123
```

The machine will stay connected and auto-reconnect if the network drops.

### What if my machine reboots?

Agents don't auto-start after reboot. You have two options:

1. **Run manually** after each reboot: `k-terminus join <orchestrator> --foreground`
2. **Set up as a service** (recommended): See [DEPLOYMENT.md](DEPLOYMENT.md) for systemd (Linux) or launchd (macOS) templates

### How do I stop an agent?

Press `Ctrl+C` if running in foreground, or:

```bash
k-terminus stop
```

This gracefully closes all sessions and disconnects.

### Can I have multiple sessions on one machine?

Yes! Each machine can have multiple terminal sessions. Create new sessions from the desktop app or CLI:

```bash
k-terminus connect machine-name
```

### How do I list all connected machines?

```bash
k-terminus list
```

### How do I see all active sessions?

```bash
k-terminus list
```

This shows both machines and their active sessions.

### What happens if I lose network connection?

**Agents**: Automatically reconnect with exponential backoff (1s, 2s, 4s, up to 60s).

**Sessions**: Remain alive on the remote machine. When the connection restores, you can reattach to them.

### Can I attach to someone else's session?

No. Sessions have ownership. Only the client that created a session can access it. This prevents accidental interference when multiple people are connected to the same orchestrator.

## Desktop App

### Do I need the desktop app?

No. The CLI (`k-terminus`) works standalone. The desktop app provides a GUI with:
- Visual topology of your machines
- Pane splitting (side-by-side terminals)
- Drag-and-drop tab management
- Real-time connection status

### Can I use the desktop app and CLI at the same time?

Yes! They both connect to the same orchestrator via IPC. Sessions created in one are visible in the other.

### Why does the desktop app fail to connect?

The desktop app connects to the orchestrator via IPC (localhost:22230). Make sure:
1. The orchestrator is running: `k-terminus status`
2. You're on the same machine as the orchestrator
3. No firewall is blocking localhost:22230

## Configuration

### Where is the config file?

- macOS: `~/Library/Application Support/k-terminus/config.toml`
- Linux: `~/.config/k-terminus/config.toml`
- Windows: `%APPDATA%\k-terminus\config.toml`

### Do I need to configure anything?

No. k-Terminus works out of the box. Configuration is optional for tuning heartbeat intervals, backoff timers, etc.

### Can I change the orchestrator port?

Yes. Edit `config.toml`:

```toml
[orchestrator]
bind_address = "0.0.0.0:2222"  # Default
```

Change `:2222` to any port. Note that this is *only* accessible within your Tailnet.

### Can I set a custom shell?

Yes. Per-machine in `config.toml`:

```toml
[[machines]]
id = "machine-name"
default_shell = "/bin/zsh"
```

Or set `SHELL` environment variable before starting the agent.

## Security

### Is my terminal traffic encrypted?

Yes. All traffic goes through Tailscale's WireGuard tunnels (ChaCha20-Poly1305 encryption).

### How does authentication work?

k-Terminus doesn't have its own authentication. It uses Tailscale's identity - if you're on the Tailnet, you can connect. Think of it like SSH but with Tailscale SSO instead of keys.

### Can someone on my Tailnet access my sessions?

Only if they have access to the orchestrator machine (via IPC). Remote agents cannot access each other's sessions.

### What if my Tailscale credentials are compromised?

Follow Tailscale's security recommendations. k-Terminus inherits Tailscale's security model - securing your Tailnet secures k-Terminus.

### Does k-Terminus log my terminal input?

No. Terminal input/output is not logged. Only connection events (machine connected, session created, etc.) are logged.

## Troubleshooting

### "Tailscale is not installed"

Install Tailscale: https://tailscale.com/download

### "Tailscale is not logged in"

Log in to Tailscale:
```bash
sudo tailscale up
```

### "Connection refused" when running join

The orchestrator isn't running or isn't reachable. Check:
1. Is the orchestrator running? `k-terminus status` on that machine
2. Are both machines on the same Tailnet? `tailscale status`
3. Can you ping the orchestrator? `ping orchestrator-hostname`

### "IPC connection dropped"

The orchestrator stopped or restarted. Restart the desktop app or CLI connection.

### Agent keeps disconnecting

Check network stability. Agents use exponential backoff and will eventually reconnect. If you see frequent disconnects:
1. Check Tailscale connection: `tailscale status`
2. Check logs on the agent: `k-terminus join ... --foreground -vv`

### Desktop app shows no machines

Make sure:
1. Orchestrator is running: `k-terminus status`
2. Agents are connected: `k-terminus list` (from CLI)
3. Desktop app is connecting to the right orchestrator (IPC should auto-detect)

## Performance

### How many machines can I connect?

Tested with dozens of agents. The practical limit depends on your orchestrator machine's resources and network bandwidth.

Each agent uses:
- ~5MB RAM when idle
- Minimal CPU (<1%) when idle
- Network bandwidth proportional to terminal output

### How many sessions can I have?

Each machine supports 64 concurrent sessions by default. This is configurable.

### Does k-Terminus work over slow connections?

Yes. Tailscale works well over cellular, satellite, etc. Terminal output is text-based and low bandwidth.

### Does it work offline?

No. Both orchestrator and agents need active Tailnet connectivity.

## Use Cases

### Can I use this for production monitoring?

k-Terminus is designed for interactive terminal access, not production monitoring. Consider purpose-built monitoring tools (Prometheus, Datadog, etc.) for production.

k-Terminus is excellent for:
- Development across multiple machines
- Research computing access
- Managing personal servers
- Accessing lab equipment

### Can I use this with CI/CD?

Not recommended. k-Terminus is for interactive sessions. For automation, use standard SSH or Tailscale SSH.

### Can I access machines outside my Tailnet?

No. All machines must be on the same Tailnet.

### Can I share access with teammates?

Anyone on your Tailnet can connect to the orchestrator. However, session ownership means they'll create their own sessions - they can't see yours.

For collaborative access, consider tmux's attach feature through k-Terminus.

## Development

### How do I build from source?

```bash
git clone https://github.com/Adiaslow/kTerminus.git
cd kTerminus
cargo build --release -p k-terminus
```

Binary will be in `target/release/k-terminus`.

### How do I contribute?

See [CONTRIBUTING.md](../CONTRIBUTING.md).

### How do I report bugs?

Open an issue on GitHub: https://github.com/Adiaslow/kTerminus/issues

For security vulnerabilities, see [SECURITY.md](../SECURITY.md) for private disclosure.

## Misc

### Why "k-Terminus"?

The "k" is for kappa (κ), a common variable in research. Terminus is the end of a journey - in this case, the terminal endpoint. It's also a nod to the research background of the project.

### Is there a roadmap?

Check the [GitHub issues](https://github.com/Adiaslow/kTerminus/issues) for planned features. Major items:
- Session persistence across orchestrator restarts
- Better multi-user collaboration
- Plugin system for custom integrations

### Can I use this commercially?

Yes! k-Terminus is MIT licensed. Use it freely for personal or commercial purposes.

### How can I support the project?

- Star the repo on GitHub
- Report bugs and suggest features
- Contribute code or documentation
- Share it with others who might find it useful

No donations accepted - keep your money for your research!
