#!/usr/bin/env bash
set -euo pipefail

# Cooper CLI installer
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/Eldridge-Morgan/cooper/main/install.sh | sh
#
# For private repos:
#   curl -fsSL https://raw.githubusercontent.com/Eldridge-Morgan/cooper/main/install.sh | GITHUB_TOKEN=ghp_xxx sh
#
# Options (env vars):
#   COOPER_VERSION   - specific version (default: latest)
#   COOPER_DIR       - install directory (default: ~/.cooper/bin)
#   GITHUB_TOKEN     - GitHub token for private repo access

REPO="Eldridge-Morgan/cooper"
INSTALL_DIR="${COOPER_DIR:-$HOME/.cooper/bin}"

info()  { printf "\033[1;34m→\033[0m %s\n" "$1"; }
ok()    { printf "\033[1;32m✓\033[0m %s\n" "$1"; }
err()   { printf "\033[1;31m✗\033[0m %s\n" "$1" >&2; exit 1; }

# --- Detect platform ---
detect_platform() {
  local os arch

  case "$(uname -s)" in
    Linux*)  os="linux" ;;
    Darwin*) os="darwin" ;;
    MINGW*|MSYS*|CYGWIN*) os="windows" ;;
    *) err "Unsupported OS: $(uname -s)" ;;
  esac

  case "$(uname -m)" in
    x86_64|amd64) arch="x86_64" ;;
    arm64|aarch64) arch="arm64" ;;
    *) err "Unsupported architecture: $(uname -m)" ;;
  esac

  echo "${os}|${arch}"
}

# --- Resolve version ---
resolve_version() {
  if [ -n "${COOPER_VERSION:-}" ]; then
    echo "$COOPER_VERSION"
    return
  fi

  local auth_header=""
  if [ -n "${GITHUB_TOKEN:-}" ]; then
    auth_header="Authorization: token $GITHUB_TOKEN"
  fi

  local latest
  latest=$(curl -fsSL ${auth_header:+-H "$auth_header"} \
    "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' | head -1 | cut -d'"' -f4)

  if [ -z "$latest" ]; then
    err "Could not determine latest version. Set COOPER_VERSION or check your GITHUB_TOKEN."
  fi

  echo "$latest"
}

# --- Download and install ---
install() {
  local platform version os arch asset_name url auth_args

  platform=$(detect_platform)
  os="${platform%%|*}"
  arch="${platform##*|}"
  version=$(resolve_version)

  info "Installing Cooper ${version} for ${os}/${arch}"

  if [ "$os" = "windows" ]; then
    asset_name="cooper-${os}-${arch}.zip"
  else
    asset_name="cooper-${os}-${arch}.tar.gz"
  fi

  local auth_header=""
  if [ -n "${GITHUB_TOKEN:-}" ]; then
    auth_header="Authorization: token $GITHUB_TOKEN"
  fi

  # Get asset ID from release metadata (JSON)
  local release_url="https://api.github.com/repos/${REPO}/releases/tags/${version}"
  local release_json

  release_json=$(curl -fsSL \
    ${auth_header:+-H "$auth_header"} \
    -H "Accept: application/vnd.github+json" \
    "$release_url")

  local asset_id
  asset_id=$(echo "$release_json" \
    | grep -B3 "\"name\": \"${asset_name}\"" \
    | grep '"id"' | head -1 | grep -o '[0-9]*')

  if [ -z "$asset_id" ]; then
    err "Asset ${asset_name} not found in release ${version}"
  fi

  local tmpdir
  tmpdir=$(mktemp -d)
  trap 'rm -rf "${tmpdir:-}"' EXIT

  info "Downloading ${asset_name}..."
  curl -fsSL -L \
    ${auth_header:+-H "$auth_header"} \
    -H "Accept: application/octet-stream" \
    -o "${tmpdir}/${asset_name}" \
    "https://api.github.com/repos/${REPO}/releases/assets/${asset_id}"

  info "Extracting..."
  mkdir -p "$INSTALL_DIR"

  if [ "$os" = "windows" ]; then
    unzip -o "${tmpdir}/${asset_name}" -d "$INSTALL_DIR"
  else
    tar xzf "${tmpdir}/${asset_name}" -C "$INSTALL_DIR"
    chmod +x "${INSTALL_DIR}/cooper"
  fi

  ok "Installed to ${INSTALL_DIR}/cooper"

  # --- Add to PATH ---
  if ! echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
    local shell_rc=""
    case "${SHELL:-/bin/sh}" in
      */zsh)  shell_rc="$HOME/.zshrc" ;;
      */bash) shell_rc="$HOME/.bashrc" ;;
      */fish) shell_rc="$HOME/.config/fish/config.fish" ;;
    esac

    if [ -n "$shell_rc" ]; then
      local path_line="export PATH=\"${INSTALL_DIR}:\$PATH\""
      if [ "${SHELL:-}" = "*/fish" ]; then
        path_line="set -gx PATH ${INSTALL_DIR} \$PATH"
      fi

      if ! grep -qF "$INSTALL_DIR" "$shell_rc" 2>/dev/null; then
        echo "" >> "$shell_rc"
        echo "# Cooper CLI" >> "$shell_rc"
        echo "$path_line" >> "$shell_rc"
        info "Added ${INSTALL_DIR} to PATH in ${shell_rc}"
        info "Run: source ${shell_rc}  (or open a new terminal)"
      fi
    else
      info "Add ${INSTALL_DIR} to your PATH manually"
    fi
  fi

  ok "Cooper ${version} is ready!"
  echo ""
  echo "  Run:  cooper --help"
  echo ""
}

install
