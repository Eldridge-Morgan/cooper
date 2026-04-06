#!/usr/bin/env bash
set -euo pipefail

# Cooper — One-command setup
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/Eldridge-Morgan/cooper/main/setup.sh | sh

REPO="Eldridge-Morgan/cooper"
INSTALL_DIR="${HOME}/.cooper/bin"

info()  { printf "\033[1;34m→\033[0m %s\n" "$1"; }
ok()    { printf "\033[1;32m✓\033[0m %s\n" "$1"; }
err()   { printf "\033[1;31m✗\033[0m %s\n" "$1" >&2; exit 1; }
warn()  { printf "\033[1;33m!\033[0m %s\n" "$1"; }

# ── Step 1: Prerequisites ──────────────────────────────────────────

info "Checking prerequisites..."

command -v curl >/dev/null 2>&1 || err "curl is not installed"
command -v node >/dev/null 2>&1 || err "Node.js is not installed (need 20+)"
command -v npm >/dev/null 2>&1 || err "npm is not installed"

NODE_MAJOR=$(node -v | cut -d. -f1 | tr -d 'v')
if [ "$NODE_MAJOR" -lt 20 ]; then
  warn "Node.js $(node -v) detected — recommend 22+"
fi

ok "Prerequisites OK"

# ── Step 2: Detect platform ───────────────────────────────────────

case "$(uname -s)" in
  Linux*)  OS="linux" ;;
  Darwin*) OS="darwin" ;;
  *) err "Unsupported OS: $(uname -s)" ;;
esac

case "$(uname -m)" in
  x86_64|amd64) ARCH="x86_64" ;;
  arm64|aarch64) ARCH="arm64" ;;
  *) err "Unsupported architecture: $(uname -m)" ;;
esac

ASSET_NAME="cooper-${OS}-${ARCH}.tar.gz"
info "Platform: ${OS}/${ARCH}"

# ── Step 3: Download and install Cooper CLI ───────────────────────

info "Fetching latest release..."

TAG=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
  | grep '"tag_name"' | head -1 | cut -d'"' -f4)

if [ -z "$TAG" ]; then
  err "Could not find latest release"
fi

# Get the direct download URL from release assets
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${TAG}/${ASSET_NAME}"

TMPDIR=$(mktemp -d)
trap 'rm -rf "${TMPDIR:-}"' EXIT

info "Downloading Cooper ${TAG}..."
curl -fsSL -o "${TMPDIR}/${ASSET_NAME}" "$DOWNLOAD_URL"

mkdir -p "$INSTALL_DIR"
tar xzf "${TMPDIR}/${ASSET_NAME}" -C "$INSTALL_DIR"
chmod +x "${INSTALL_DIR}/cooper"

ok "Cooper ${TAG} installed"

# ── Step 4: Make `cooper` available immediately ───────────────────

# Try to symlink into a directory already on PATH
LINKED=false
for BIN_DIR in /usr/local/bin /usr/bin; do
  if [ -d "$BIN_DIR" ] && [ -w "$BIN_DIR" ]; then
    ln -sf "${INSTALL_DIR}/cooper" "${BIN_DIR}/cooper"
    LINKED=true
    break
  fi
done

# If no writable system bin dir, try with sudo
if [ "$LINKED" = false ]; then
  if [ -d /usr/local/bin ]; then
    sudo ln -sf "${INSTALL_DIR}/cooper" /usr/local/bin/cooper 2>/dev/null && LINKED=true
  fi
fi

# Fallback: add ~/.cooper/bin to PATH in shell rc
if [ "$LINKED" = false ]; then
  SHELL_RC=""
  case "${SHELL:-/bin/sh}" in
    */zsh)  SHELL_RC="$HOME/.zshrc" ;;
    */bash) SHELL_RC="$HOME/.bashrc" ;;
    */fish) SHELL_RC="$HOME/.config/fish/config.fish" ;;
  esac

  if [ -n "$SHELL_RC" ]; then
    if ! grep -qF "$INSTALL_DIR" "$SHELL_RC" 2>/dev/null; then
      echo "" >> "$SHELL_RC"
      echo "# Cooper CLI" >> "$SHELL_RC"
      echo "export PATH=\"${INSTALL_DIR}:\$PATH\"" >> "$SHELL_RC"
      info "Added to PATH in ${SHELL_RC}"
    fi
  fi
fi
export PATH="${INSTALL_DIR}:$PATH"

# ── Done ──────────────────────────────────────────────────────────

echo ""
ok "Setup complete!"
echo ""
echo "  Cooper ${TAG} is ready to use."
echo ""
echo "  Get started:"
echo "    cooper new my-app"
echo "    cd my-app"
echo "    npm install"
echo "    cooper run"
echo ""
echo "  Dev server: http://localhost:4000"
echo "  Dashboard:  http://localhost:9500"
echo ""

if ! command -v cooper >/dev/null 2>&1; then
  warn "Open a new terminal for PATH changes to take effect"
fi
