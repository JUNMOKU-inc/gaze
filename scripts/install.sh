#!/bin/sh
# install.sh — Gaze CLI installer
# Works with private GitHub repos via `gh` CLI authentication.
#
# Usage:
#   gh release download --repo RQ-Akiyoshi/gaze --pattern 'gaze-*' -D /tmp/gaze-dl && sh /tmp/gaze-dl/install.sh
#   or:
#   curl -fsSL <url>/install.sh | sh   (public repos only)
set -e

REPO="RQ-Akiyoshi/gaze"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

# Detect platform
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Darwin) ;;
  *) echo "Error: Gaze CLI currently supports macOS only."; exit 1 ;;
esac

case "$ARCH" in
  arm64|aarch64) TARGET="aarch64-apple-darwin" ;;
  x86_64)        TARGET="x86_64-apple-darwin" ;;
  *)             echo "Error: Unsupported architecture: $ARCH"; exit 1 ;;
esac

# Require gh CLI for private repo access
if ! command -v gh >/dev/null 2>&1; then
  echo "Error: GitHub CLI (gh) is required for installation."
  echo ""
  echo "Install it with:"
  echo "  brew install gh"
  echo ""
  echo "Then authenticate:"
  echo "  gh auth login"
  exit 1
fi

# Check gh auth status
if ! gh auth status >/dev/null 2>&1; then
  echo "Error: Not authenticated with GitHub CLI."
  echo "Run: gh auth login"
  exit 1
fi

# Fetch latest version
LATEST="$(gh release view --repo "$REPO" --json tagName --jq '.tagName' 2>/dev/null)" || true

if [ -z "$LATEST" ]; then
  echo "Error: Could not determine latest version. Check repo access."
  exit 1
fi

VERSION="${LATEST#v}"
ASSET="gaze-v${VERSION}-${TARGET}.tar.gz"

echo "Installing gaze v${VERSION} for ${TARGET}..."
echo "  To: $INSTALL_DIR/gaze"

# Download and extract
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

gh release download "$LATEST" \
  --repo "$REPO" \
  --pattern "$ASSET" \
  --dir "$TMP"

tar xzf "$TMP/$ASSET" -C "$TMP"

# Install
if ! mkdir -p "$INSTALL_DIR" 2>/dev/null || ! [ -w "$INSTALL_DIR" ]; then
  echo ""
  echo "Permission denied for $INSTALL_DIR. Try one of:"
  echo "  sudo sh scripts/install.sh"
  echo "  INSTALL_DIR=~/.local/bin sh scripts/install.sh"
  exit 1
fi
mv "$TMP/gaze" "$INSTALL_DIR/gaze"
chmod +x "$INSTALL_DIR/gaze"

echo ""
echo "Installed gaze v${VERSION} to $INSTALL_DIR/gaze"
"$INSTALL_DIR/gaze" --version
