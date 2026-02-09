# Contributing to k-Terminus

Thank you for your interest in contributing to k-Terminus!

## Development Setup

### Prerequisites

- **Rust** (stable, 1.75+): https://rustup.rs
- **Node.js** (18+): For desktop app development
- **Tailscale**: For testing (https://tailscale.com/download)

### Clone and Build

```bash
git clone https://github.com/Adiaslow/kTerminus
cd kTerminus

# Build all Rust crates
cargo build --workspace

# Install the CLI locally
cargo install --path crates/kt-cli
```

### Running Tests

```bash
# Run all tests (use single thread to avoid port conflicts)
cargo test --workspace -- --test-threads=1

# Run specific test suite
cargo test --test e2e_test
cargo test --test cli_integration
cargo test --test ipc_integration

# Run with verbose output
cargo test --workspace -- --test-threads=1 --nocapture
```

### Development Workflow

**Run orchestrator in foreground:**
```bash
k-terminus serve --foreground -vv
```

**Run agent in foreground (separate terminal):**
```bash
k-terminus join <orchestrator-name> --foreground
```

**Test IPC manually:**
```bash
# Send ping to orchestrator
echo '{"type":"ping"}' | nc localhost 22230
```

## Project Structure

```
kTerminus/
├── Cargo.toml              # Workspace configuration
├── crates/
│   ├── kt-protocol/        # Wire protocol (frames, messages, codecs)
│   ├── kt-core/            # Shared types, config, Tailscale integration
│   ├── kt-orchestrator/    # SSH server, connection pool, IPC server
│   ├── kt-agent/           # SSH client, PTY management, reconnection
│   └── kt-cli/             # CLI binary and commands
└── apps/
    └── kt-desktop/         # Tauri desktop app (React + Rust)
```

### Crate Dependencies

```
kt-cli ──────────────┐
                     ▼
kt-orchestrator ──► kt-core ──► kt-protocol
                     ▲
kt-agent ────────────┘
```

## Code Style

- Follow standard Rust formatting: `cargo fmt`
- Run clippy: `cargo clippy --workspace`
- Keep functions small and focused
- Document public APIs with doc comments
- Write tests for new functionality

## Testing Guidelines

### Unit Tests
- Place in the same file as the code being tested
- Use `#[cfg(test)]` module
- Focus on isolated function behavior

### Integration Tests
- Place in `crates/<crate>/tests/`
- Test component interactions
- Use real IPC/networking where possible

### E2E Tests
- Located in `crates/kt-cli/tests/e2e_test.rs`
- Spawn actual orchestrator and agent processes
- Require Tailscale for agent connection tests

## Architecture Overview

### Connection Flow

1. **Agent connects** to orchestrator via SSH (port 2222)
2. **Loopback check**: Localhost connections (127.0.0.1) are always accepted
3. **Tailscale verification**: Orchestrator checks if peer IP is in tailnet
4. **Reject**: Non-tailnet, non-loopback connections are rejected
5. **Registration**: Agent sends machine info, orchestrator acknowledges
6. **Session multiplexing**: Multiple PTY sessions over single SSH channel

### Key Components

| Component | Location | Responsibility |
|-----------|----------|----------------|
| `SshServer` | kt-orchestrator/src/server/ | Accept SSH connections |
| `ClientHandler` | kt-orchestrator/src/server/handler.rs | Handle SSH auth, frames |
| `TailscaleVerifier` | kt-orchestrator/src/auth/tailscale.rs | Verify tailnet membership |
| `IpcServer` | kt-orchestrator/src/ipc/ | CLI/GUI communication |
| `TunnelConnector` | kt-agent/src/tunnel/ | Establish SSH tunnel |
| `PtyManager` | kt-agent/src/pty/ | Spawn and manage PTYs |

### Protocol

Messages are framed with:
- 4-byte session ID
- 1-byte message type
- 3-byte payload length
- Variable payload

See `kt-protocol/src/` for message types.

## Making Changes

1. **Create a branch**: `git checkout -b feature/my-feature`
2. **Make changes** with tests
3. **Run tests**: `cargo test --workspace -- --test-threads=1`
4. **Format**: `cargo fmt`
5. **Lint**: `cargo clippy --workspace`
6. **Commit**: Use conventional commit messages
7. **Push and PR**: Open a pull request

### Commit Messages

Use conventional commits:
- `feat: add session recording`
- `fix: handle reconnection timeout`
- `docs: update CLI reference`
- `test: add IPC integration tests`
- `refactor: simplify auth flow`

## Desktop App Development

```bash
cd apps/kt-desktop

# Install dependencies
npm install

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```

The desktop app uses:
- **React 18** + TypeScript for UI
- **Tauri 2.0** for native shell
- **xterm.js** for terminal emulation
- **Tailwind CSS** for styling

## Documentation

| Document | Description |
|----------|-------------|
| [README.md](README.md) | Project overview and quick start |
| [SECURITY.md](SECURITY.md) | Security model and policies |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Technical architecture details |
| [docs/CLI.md](docs/CLI.md) | CLI command reference |
| [docs/CONFIGURATION.md](docs/CONFIGURATION.md) | Configuration options |
| [docs/TECHNICAL_DEBT.md](docs/TECHNICAL_DEBT.md) | Known issues and improvement plans |
| [docs/specs/tech_spec.md](docs/specs/tech_spec.md) | Full technical specification |

## Questions?

Open an issue for questions, feature requests, or bug reports.
