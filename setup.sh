#!/usr/bin/env bash
set -euo pipefail

# Cooper — One-command team setup
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/Eldridge-Morgan/cooper/main/setup.sh | sh
#
# What it does:
#   1. Checks prerequisites (Node.js, git, gh)
#   2. Ensures GitHub CLI has packages scope
#   3. Installs the Cooper CLI binary
#   4. Configures npm for @eldridge-morgan packages (global .npmrc)
#   5. Optionally scaffolds a new project

REPO="Eldridge-Morgan/cooper"
INSTALL_DIR="${HOME}/.cooper/bin"

info()  { printf "\033[1;34m→\033[0m %s\n" "$1"; }
ok()    { printf "\033[1;32m✓\033[0m %s\n" "$1"; }
err()   { printf "\033[1;31m✗\033[0m %s\n" "$1" >&2; exit 1; }
warn()  { printf "\033[1;33m!\033[0m %s\n" "$1"; }

# ── Step 1: Prerequisites ──────────────────────────────────────────

info "Checking prerequisites..."

command -v git >/dev/null 2>&1 || err "git is not installed"
command -v node >/dev/null 2>&1 || err "Node.js is not installed (need 22+)"
command -v npm >/dev/null 2>&1 || err "npm is not installed"

NODE_MAJOR=$(node -v | cut -d. -f1 | tr -d 'v')
if [ "$NODE_MAJOR" -lt 20 ]; then
  warn "Node.js $(node -v) detected — recommend 22+"
fi

# Check for GitHub CLI
if ! command -v gh >/dev/null 2>&1; then
  info "Installing GitHub CLI..."
  case "$(uname -s)" in
    Darwin*)
      if command -v brew >/dev/null 2>&1; then
        brew install gh
      else
        err "Install Homebrew first: https://brew.sh — then re-run this script"
      fi
      ;;
    Linux*)
      if command -v apt-get >/dev/null 2>&1; then
        # Debian/Ubuntu/Mint
        sudo mkdir -p -m 755 /etc/apt/keyrings
        curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | sudo tee /etc/apt/keyrings/githubcli-archive-keyring.gpg > /dev/null
        sudo chmod go+r /etc/apt/keyrings/githubcli-archive-keyring.gpg
        echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | sudo tee /etc/apt/sources.list.d/github-cli.list > /dev/null
        sudo apt-get update && sudo apt-get install gh -y
      elif command -v dnf >/dev/null 2>&1; then
        # Fedora/RHEL
        sudo dnf install gh -y
      else
        err "Install GitHub CLI manually: https://cli.github.com"
      fi
      ;;
    *) err "Install GitHub CLI manually: https://cli.github.com" ;;
  esac
fi

ok "Prerequisites OK"

# ── Step 2: GitHub auth ────────────────────────────────────────────

info "Checking GitHub authentication..."

# Skip auth entirely if already logged in with repo access
if gh auth status >/dev/null 2>&1; then
  ok "GitHub auth detected"

  # Only add packages scope if missing
  SCOPES=$(gh auth status 2>&1 | grep "Token scopes" || true)
  if ! echo "$SCOPES" | grep -q "read:packages"; then
    info "Adding packages scope..."
    gh auth refresh --hostname github.com --scopes read:packages,write:packages
  fi
else
  info "Logging in to GitHub (a browser window will open)..."
  gh auth login --hostname github.com --web --scopes read:packages,write:packages
fi

ok "GitHub auth ready"

# ── Step 3: Install Cooper CLI ─────────────────────────────────────

info "Installing Cooper CLI..."

# Detect platform
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
TOKEN=$(gh auth token)

# Get latest release
TAG=$(curl -fsSL -H "Authorization: token $TOKEN" \
  "https://api.github.com/repos/${REPO}/releases/latest" \
  | grep '"tag_name"' | head -1 | cut -d'"' -f4)

if [ -z "$TAG" ]; then
  err "Could not find latest release"
fi

# Get asset ID
ASSET_ID=$(curl -fsSL -H "Authorization: token $TOKEN" \
  -H "Accept: application/vnd.github+json" \
  "https://api.github.com/repos/${REPO}/releases/tags/${TAG}" \
  | grep -B3 "\"name\": \"${ASSET_NAME}\"" \
  | grep '"id"' | head -1 | grep -o '[0-9]*')

if [ -z "$ASSET_ID" ]; then
  err "Binary not found for ${OS}/${ARCH} in release ${TAG}"
fi

# Download and install
TMPDIR=$(mktemp -d)
trap 'rm -rf "${TMPDIR:-}"' EXIT

curl -fsSL -L \
  -H "Authorization: token $TOKEN" \
  -H "Accept: application/octet-stream" \
  -o "${TMPDIR}/${ASSET_NAME}" \
  "https://api.github.com/repos/${REPO}/releases/assets/${ASSET_ID}"

mkdir -p "$INSTALL_DIR"
tar xzf "${TMPDIR}/${ASSET_NAME}" -C "$INSTALL_DIR"
chmod +x "${INSTALL_DIR}/cooper"

ok "Cooper ${TAG} installed to ${INSTALL_DIR}/cooper"

# ── Step 4: Add to PATH ───────────────────────────────────────────

if ! echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
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
    fi
  fi
  export PATH="${INSTALL_DIR}:$PATH"
fi

# ── Step 5: Configure npm for GitHub Packages ─────────────────────

info "Configuring npm for @eldridge-morgan packages..."

# Global .npmrc for the scope
GLOBAL_NPMRC="${HOME}/.npmrc"

# Add scope registry if not present
if ! grep -qF "@eldridge-morgan:registry" "$GLOBAL_NPMRC" 2>/dev/null; then
  echo "@eldridge-morgan:registry=https://npm.pkg.github.com" >> "$GLOBAL_NPMRC"
fi

# Add auth token (using gh token dynamically)
# Remove old token line if present, then add fresh one
if [ -f "$GLOBAL_NPMRC" ]; then
  grep -v 'npm\.pkg\.github\.com/:_authToken' "$GLOBAL_NPMRC" > "${GLOBAL_NPMRC}.tmp" 2>/dev/null || true
  mv "${GLOBAL_NPMRC}.tmp" "$GLOBAL_NPMRC"
fi
echo "//npm.pkg.github.com/:_authToken=$(gh auth token)" >> "$GLOBAL_NPMRC"

ok "npm configured — @eldridge-morgan packages will resolve automatically"

# ── Step 6: Verify ────────────────────────────────────────────────

echo ""
ok "Setup complete!"
echo ""
echo "  Cooper ${TAG} is ready to use."
echo ""
echo "  Create a new project:"
echo "    cooper new my-app"
echo "    cd my-app"
echo "    npm install"
echo "    cooper run"
echo ""
echo "  Your dev server will start at http://localhost:4000"
echo "  Dashboard at http://localhost:9500"
echo ""

# Open a new shell if PATH was modified
if ! command -v cooper >/dev/null 2>&1; then
  warn "Open a new terminal for PATH changes to take effect"
fi
