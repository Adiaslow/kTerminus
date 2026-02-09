# k-Terminus
#
# Usage:
#   just app          - Run desktop app
#   just serve        - Start orchestrator (CLI)
#   just join ABC123  - Connect as agent
#
# Examples:
#   just app
#   just serve
#   just join ABC123
#   just list

# Path to release binary
cli := "./target/release/k-terminus"
app_path := "target/release/bundle/macos/k-Terminus.app"

# Default: show help
default:
    @just --list

# === Desktop App ===

# Run desktop app
app: _build-app
    @open "{{app_path}}" || (echo "App not found, building..." && just _build-app && open "{{app_path}}")

# === CLI Commands ===

# Start orchestrator
serve *ARGS: _build
    {{cli}} serve --foreground {{ARGS}}

# Connect as agent (pairing code or hostname)
join *ARGS: _build
    {{cli}} join {{ARGS}} --foreground

# List connected machines
list: _build
    {{cli}} list

# Show orchestrator status
status: _build
    {{cli}} status

# Connect to a machine
connect MACHINE *ARGS: _build
    {{cli}} connect {{MACHINE}} {{ARGS}}

# Stop orchestrator
stop: _build
    {{cli}} stop

# Run any CLI command
cli *ARGS: _build
    {{cli}} {{ARGS}}

# Install CLI globally (~/.cargo/bin)
install:
    cargo install --path crates/kt-cli

# === Development ===

# Run desktop app (dev mode with hot reload)
dev: _kill-dev
    cd apps/kt-desktop && pnpm run tauri:dev

# Kill orphaned dev processes (Vite on port 1420)
_kill-dev:
    @lsof -ti:1420 | xargs kill -9 2>/dev/null || true

# Clean up stale orchestrator files (PID, token)
clean-state:
    @rm -f ~/Library/Application\ Support/k-terminus/orchestrator.pid 2>/dev/null || true
    @rm -f ~/Library/Application\ Support/k-terminus/ipc_auth_token 2>/dev/null || true
    @rm -f ~/.config/k-terminus/orchestrator.pid 2>/dev/null || true
    @rm -f ~/.config/k-terminus/ipc_auth_token 2>/dev/null || true
    @echo "Cleaned up stale state files"

# Run orchestrator (dev)
dev-serve *ARGS:
    cargo run -p k-terminus -- serve --foreground {{ARGS}}

# Connect as agent (dev)
dev-join *ARGS:
    cargo run -p k-terminus -- join {{ARGS}} --foreground

# === Internal ===

# Build CLI (release)
_build:
    @cargo build --release -p k-terminus 2>/dev/null || cargo build --release -p k-terminus

# Build desktop app (release)
_build-app:
    cd apps/kt-desktop && pnpm run tauri:build

# === Build & Test ===

# Build everything (release)
build: _build _build-app

# Type check
check:
    cargo check --workspace

# Run tests
test:
    cargo test --workspace

# Format code
fmt:
    cargo fmt --all

# Lint code
lint:
    cargo clippy --workspace -- -D warnings

# Clean build artifacts
clean:
    cargo clean
    rm -rf apps/kt-desktop/src-tauri/target

# Install dependencies
setup:
    cd apps/kt-desktop && pnpm install

# === Aliases ===

alias a := app
alias s := serve
alias j := join
alias c := connect
alias l := list
