# k-Terminus development commands
#
# Quick start:
#   just              - Run desktop app
#   just serve        - Run orchestrator (CLI)
#   just join <host>  - Connect as agent
#
# Examples:
#   just join my-laptop
#   just list
#   just connect my-server

# Default: run the desktop app
default: run

# Run the desktop GUI (orchestrator starts automatically)
run:
    cd apps/kt-desktop && npm run tauri:dev

# Run orchestrator in foreground
serve *ARGS:
    cargo run -p k-terminus -- serve --foreground {{ARGS}}

# Connect as agent to an orchestrator
join HOST *ARGS:
    cargo run -p k-terminus -- join {{HOST}} --foreground {{ARGS}}

# List connected machines
list:
    cargo run -p k-terminus -- list

# Show orchestrator status
status:
    cargo run -p k-terminus -- status

# Connect to a machine
connect MACHINE *ARGS:
    cargo run -p k-terminus -- connect {{MACHINE}} {{ARGS}}

# Run any CLI command
cli *ARGS:
    cargo run -p k-terminus -- {{ARGS}}

# === Build & Test ===

build:
    cargo build --release --workspace

build-desktop:
    cd apps/kt-desktop && npm run tauri:build

check:
    cargo check --workspace

test:
    cargo test --workspace

fmt:
    cargo fmt --all

lint:
    cargo clippy --workspace -- -D warnings

clean:
    cargo clean

# Install frontend dependencies
install-deps:
    cd apps/kt-desktop && npm install
