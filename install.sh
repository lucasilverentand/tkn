#!/bin/sh
set -e

REPO="lucasilverentand/tkn"
INSTALL_DIR="/usr/local/bin"

# Detect platform
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Darwin) os="apple-darwin" ;;
  Linux)  os="unknown-linux-gnu" ;;
  *)      echo "Unsupported OS: $OS" >&2; exit 1 ;;
esac

case "$ARCH" in
  x86_64|amd64)  arch="x86_64" ;;
  arm64|aarch64) arch="aarch64" ;;
  *)             echo "Unsupported architecture: $ARCH" >&2; exit 1 ;;
esac

TARGET="${arch}-${os}"

# Get latest release tag
LATEST="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | cut -d'"' -f4)"

if [ -z "$LATEST" ]; then
  echo "Failed to fetch latest release" >&2
  exit 1
fi

URL="https://github.com/${REPO}/releases/download/${LATEST}/tkn-${TARGET}.tar.gz"

echo "Installing tkn ${LATEST} (${TARGET})..."

TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

curl -fsSL "$URL" | tar xz -C "$TMPDIR"

if [ -w "$INSTALL_DIR" ]; then
  mv "$TMPDIR/tkn" "$INSTALL_DIR/tkn"
else
  sudo mv "$TMPDIR/tkn" "$INSTALL_DIR/tkn"
fi

echo "Installed tkn to ${INSTALL_DIR}/tkn"
echo
echo "Next steps:"
echo "  tkn doctor"
echo "  tkn setup claude"
echo "  tkn setup codex --repo /path/to/repo"
