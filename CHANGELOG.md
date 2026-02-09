# Changelog

All notable changes to k-Terminus will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

#### Desktop App
- **Terminal pane splitting**: Split terminals horizontally (Cmd+D) or vertically (Cmd+Shift+D)
- **Drag-and-drop splitting**: Drag tabs to pane edges to create splits
- **Cross-platform shortcuts**: Mac uses Cmd, Windows/Linux uses Ctrl+Shift to avoid terminal conflicts
- **Clickable tag filtering**: Click machine tags to filter the machine list
- **Layout persistence**: Pane layouts saved and restored across sessions
- **Focus cycling**: Navigate between panes with Cmd+]/Cmd+[

#### Security
- **IPC Authentication**: Token-based authentication for IPC connections prevents unauthorized local processes from controlling the orchestrator
  - Random 64-character token generated on orchestrator startup
  - Token written to `~/.k-terminus/ipc_auth_token` with restricted permissions (600)
  - All IPC requests (except Ping) require authentication
  - Constant-time token comparison to prevent timing attacks

- **Content Security Policy (CSP)**: Enabled strict CSP for the Tauri desktop app to prevent XSS attacks

- **SSH Bind Address**: Default SSH bind address changed from `0.0.0.0:2222` to `127.0.0.1:2222` for security
  - Prevents unintended network exposure
  - Use `0.0.0.0:2222` explicitly for network access

- **Shell Path Validation**: Agent validates shell paths against a whitelist and `/etc/shells`
  - Prevents command injection via malicious shell paths

- **IPC Rate Limiting**: Per-client rate limiting (1000 req/s) and connection limits (100 max connections)
  - Prevents DoS from malicious or buggy clients

- **Pairing Code Security**: Pairing code logging reduced from INFO to DEBUG level

#### Features
- **Orchestrator lifecycle management**: Smart startup/shutdown for the desktop app
  - Automatically connects to existing orchestrator if running
  - Starts embedded orchestrator if none running
  - PID file management for daemon mode

- **Connection health monitoring**: Heartbeat-based health checks with automatic disconnection of unresponsive agents

- **Session ownership tracking**: Sessions track their creating IPC client for access control

- **Virtual scrolling**: MachineList component uses `@tanstack/react-virtual` for efficient rendering of large machine lists

- **Centralized icons**: All SVG icons consolidated into `Icons.tsx` component

#### Code Quality
- **Time utilities**: Extracted common time calculations to `kt_core::time` module
- **Error handling**: Replaced `.expect()` panics with Option-based error handling in SSH handler
- **Test improvements**: Replaced `.unwrap()` with descriptive `.expect()` in tests
- **Accessibility**: Added keyboard support and ARIA attributes to resize handle
- **Toast notifications**: Added user feedback for session creation, kill, and subscription errors
- **Array keys**: Fixed React array keys in LogsView to use unique IDs

#### Testing
- **Full test coverage**: 120+ tests across unit, integration, and E2E levels
- **All tests passing**: All E2E tests enabled and passing (no Tailscale required for CI)
- **Persistent IPC connections**: E2E tests properly maintain session ownership across requests

#### Documentation
- **Technical debt tracking**: Added `docs/TECHNICAL_DEBT.md` documenting 35 known issues from architectural audit - all now resolved
- **Updated all docs**: README, CHANGELOG, SECURITY, ARCHITECTURE, tech_spec, CONFIGURATION now reflect current implementation

### Changed
- IpcServer::new now returns `Result<Self>` instead of `Self` to handle token file errors
- IpcResponse enum extended with `Authenticated` and `AuthenticationRequired` variants
- IpcRequest enum extended with `Authenticate { token: String }` variant

### Fixed
- IPC pairing code verification now works without authentication (required for agent discovery)
- CLI now properly authenticates with IPC server (was missing authentication step)
- E2E tests now use persistent IPC connections to maintain session ownership
- All E2E tests enabled (previously `test_e2e_agent_connects_to_orchestrator` and `test_e2e_full_session_flow` were skipped)

### Security
- Fixed potential information disclosure via pairing code in logs
- Fixed missing input validation in IPC server (now has size limits and rate limiting)
- Fixed missing authentication in IPC protocol

## [0.1.0] - Initial Release

### Added
- **Orchestrator**: Central daemon managing agent connections via SSH
  - Listens for agent connections on configurable port (default 2222)
  - Multiplexes multiple terminal sessions per agent
  - Tailscale-based peer verification for zero-config authentication

- **Agent**: Lightweight daemon running on remote machines
  - Connects to orchestrator via SSH reverse tunnel
  - Manages PTY sessions for terminal access
  - Automatic reconnection with exponential backoff

- **Desktop App**: Tauri-based cross-platform GUI
  - Machine list with connection status
  - Multi-tab terminal interface using xterm.js
  - Topology view showing network connections
  - Session management (create, attach, kill)

- **CLI**: Command-line interface for headless operation
  - `k-terminus serve` - Start orchestrator daemon
  - `k-terminus join <host>` - Connect as agent
  - `k-terminus list` - List connected machines
  - `k-terminus connect <machine>` - Open terminal session
  - `k-terminus status` - Show orchestrator status

- **Protocol**: Custom binary protocol over SSH
  - Frame-based multiplexing with session IDs
  - Message types: Register, SessionCreate, Data, Resize, Close, Heartbeat
  - Version negotiation for forward compatibility

- **Configuration**
  - TOML-based configuration files
  - Auto-detection of Tailscale hostname
  - Configurable timeouts, ports, and paths

### Architecture
- Workspace with multiple crates:
  - `kt-core`: Shared types, configuration, utilities
  - `kt-protocol`: Wire protocol definitions
  - `kt-orchestrator`: Server-side daemon
  - `kt-agent`: Client-side daemon
  - `kt-cli`: Command-line interface
  - `kt-desktop`: Tauri desktop application

[Unreleased]: https://github.com/Adiaslow/kTerminus/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Adiaslow/kTerminus/releases/tag/v0.1.0
