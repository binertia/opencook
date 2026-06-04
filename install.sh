#!/usr/bin/env bash
set -e

# OpenCook — One-liner installer
# Usage: curl -fsSL https://raw.githubusercontent.com/ai-gateway/ai-gateway/main/install.sh | bash

REPO="ai-gateway/ai-gateway"
BINARY="opencook"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

info() { printf "${BLUE}ℹ${NC}  %s\n" "$1"; }
ok()   { printf "${GREEN}✓${NC}  %s\n" "$1"; }
warn() { printf "${YELLOW}⚠${NC}  %s\n" "$1"; }
err()  { printf "${RED}✗${NC}  %s\n" "$1" >&2; }

# Detect OS and architecture
detect_target() {
    local os arch
    os=$(uname -s | tr '[:upper:]' '[:lower:]')
    arch=$(uname -m)

    case "$os" in
        linux)
            case "$arch" in
                x86_64)  echo "x86_64-unknown-linux-gnu" ;;
                aarch64) echo "aarch64-unknown-linux-gnu" ;;
                arm64)   echo "aarch64-unknown-linux-gnu" ;;
                *)       echo "" ;;
            esac
            ;;
        darwin)
            case "$arch" in
                x86_64)  echo "x86_64-apple-darwin" ;;
                aarch64) echo "aarch64-apple-darwin" ;;
                arm64)   echo "aarch64-apple-darwin" ;;
                *)       echo "" ;;
            esac
            ;;
        *)
            echo ""
            ;;
    esac
}

# Try to install pre-built binary
install_prebuilt() {
    local target="$1"
    local version="${VERSION:-latest}"
    local url

    if [ "$version" = "latest" ]; then
        url="https://github.com/${REPO}/releases/latest/download/${BINARY}-${target}.tar.gz"
    else
        url="https://github.com/${REPO}/releases/download/${version}/${BINARY}-${target}.tar.gz"
    fi

    info "Downloading ${BINARY} ${version} for ${target}..."

    local tmpdir
    tmpdir=$(mktemp -d)
    trap 'rm -rf "$tmpdir"' EXIT

    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$url" -o "${tmpdir}/${BINARY}.tar.gz" 2>/dev/null || return 1
    elif command -v wget >/dev/null 2>&1; then
        wget -q "$url" -O "${tmpdir}/${BINARY}.tar.gz" 2>/dev/null || return 1
    else
        err "Neither curl nor wget found. Please install one of them."
        return 1
    fi

    tar -xzf "${tmpdir}/${BINARY}.tar.gz" -C "$tmpdir"
    mv "${tmpdir}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
    chmod +x "${INSTALL_DIR}/${BINARY}"

    ok "Installed ${BINARY} to ${INSTALL_DIR}"
    return 0
}

# Install via cargo
install_cargo() {
    if ! command -v cargo >/dev/null 2>&1; then
        err "Rust/Cargo not found. Install from https://rustup.rs"
        return 1
    fi

    info "Installing ${BINARY} via cargo..."
    cargo install --locked --git https://github.com/${REPO} --bin opencook
    ok "Installed ${BINARY} via cargo"
    return 0
}

# Build from source
install_source() {
    if ! command -v cargo >/dev/null 2>&1; then
        err "Rust/Cargo not found. Install from https://rustup.rs"
        exit 1
    fi

    info "Building ${BINARY} from source..."
    local tmpdir
    tmpdir=$(mktemp -d)
    trap 'rm -rf "$tmpdir"' EXIT

    git clone --depth 1 https://github.com/${REPO}.git "$tmpdir/repo"
    cd "$tmpdir/repo"
    cargo build --release --bin opencook
    cp "target/release/opencook" "${INSTALL_DIR}/${BINARY}"
    chmod +x "${INSTALL_DIR}/${BINARY}"

    ok "Built and installed ${BINARY} to ${INSTALL_DIR}"
}

main() {
    echo ""
    info "OpenCook Installer"
    echo ""

    # Ensure install directory exists
    mkdir -p "$INSTALL_DIR"

    # Check if directory is in PATH
    case ":$PATH:" in
        *":$INSTALL_DIR:"*) ;;
        *)
            warn "$INSTALL_DIR is not in your PATH"
            info "Add this to your shell profile:"
            echo "    export PATH=\"\$PATH:$INSTALL_DIR\""
            ;;
    esac

    local target
    target=$(detect_target)

    # Try pre-built binary first
    if [ -n "$target" ]; then
        if install_prebuilt "$target"; then
            : # success
        else
            warn "Pre-built binary not available for ${target}"
            if command -v cargo >/dev/null 2>&1; then
                install_cargo
            else
                err "Cannot install automatically."
                err "Options:"
                err "  1. Install Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
                err "  2. Build from source: git clone https://github.com/${REPO}.git && cd ai-gateway && cargo build --release"
                exit 1
            fi
        fi
    else
        warn "Unknown platform: $(uname -s) $(uname -m)"
        install_cargo
    fi

    # Verify installation
    if command -v "${INSTALL_DIR}/${BINARY}" >/dev/null 2>&1 || command -v "$BINARY" >/dev/null 2>&1; then
        echo ""
        ok "Installation complete!"
        echo ""
        echo "  Start the gateway:  ${BINARY} serve"
        echo "  Configure profile:  ${BINARY} config"
        echo "  View profile:       ${BINARY} profile"
        echo ""
        info "Getting started:"
        echo "    export OPENAI_API_KEY=sk-..."
        echo "    ${BINARY} serve"
        echo ""
    else
        err "Installation may have failed. Check ${INSTALL_DIR}"
        exit 1
    fi
}

main "$@"
