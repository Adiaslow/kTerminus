#!/bin/bash
# k-Terminus installer
# Usage: curl -sSL https://raw.githubusercontent.com/Adiaslow/kTerminus/main/install.sh | bash

set -e

REPO="Adiaslow/kTerminus"
BINARY_NAME="k-terminus"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

info() { echo -e "${BLUE}==>${NC} $1"; }
success() { echo -e "${GREEN}==>${NC} $1"; }
warn() { echo -e "${YELLOW}==>${NC} $1"; }
error() { echo -e "${RED}==>${NC} $1"; exit 1; }

# Detect OS and architecture
detect_platform() {
    local os arch

    os=$(uname -s | tr '[:upper:]' '[:lower:]')
    arch=$(uname -m)

    case "$os" in
        linux)
            os="unknown-linux-gnu"
            ;;
        darwin)
            os="apple-darwin"
            ;;
        msys*|mingw*|cygwin*)
            os="pc-windows-msvc"
            ;;
        *)
            error "Unsupported OS: $os"
            ;;
    esac

    case "$arch" in
        x86_64|amd64)
            arch="x86_64"
            ;;
        arm64|aarch64)
            arch="aarch64"
            ;;
        *)
            error "Unsupported architecture: $arch"
            ;;
    esac

    echo "${arch}-${os}"
}

# Get latest release version
get_latest_version() {
    curl -sSL "https://api.github.com/repos/${REPO}/releases/latest" | \
        grep '"tag_name"' | \
        sed -E 's/.*"([^"]+)".*/\1/'
}

# Download and install
install() {
    local platform version url tmpdir

    platform=$(detect_platform)
    info "Detected platform: $platform"

    info "Fetching latest version..."
    version=$(get_latest_version)
    if [ -z "$version" ]; then
        error "Could not determine latest version. Check your internet connection."
    fi
    info "Latest version: $version"

    url="https://github.com/${REPO}/releases/download/${version}/${BINARY_NAME}-${platform}.tar.gz"
    info "Downloading from: $url"

    tmpdir=$(mktemp -d)
    trap "rm -rf $tmpdir" EXIT

    if ! curl -sSL "$url" -o "$tmpdir/k-terminus.tar.gz"; then
        error "Failed to download. The release may not exist for your platform."
    fi

    info "Extracting..."
    tar -xzf "$tmpdir/k-terminus.tar.gz" -C "$tmpdir"

    info "Installing to $INSTALL_DIR..."
    if [ -w "$INSTALL_DIR" ]; then
        mv "$tmpdir/$BINARY_NAME" "$INSTALL_DIR/"
    else
        warn "Need sudo to install to $INSTALL_DIR"
        sudo mv "$tmpdir/$BINARY_NAME" "$INSTALL_DIR/"
    fi

    chmod +x "$INSTALL_DIR/$BINARY_NAME"

    success "k-terminus $version installed successfully!"
    echo ""
    echo "  To get started:"
    echo "    k-terminus --help"
    echo ""
    echo "  Quick start:"
    echo "    k-terminus serve     # On your main machine"
    echo "    k-terminus join <host>  # On remote machines"
    echo ""

    # Check for Tailscale
    if ! command -v tailscale &> /dev/null; then
        warn "Tailscale is not installed. k-Terminus requires Tailscale."
        echo "  Install from: https://tailscale.com/download"
    fi
}

# Run
install
