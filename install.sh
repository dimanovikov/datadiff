#!/bin/sh
# datadiff installer for macOS and Linux: downloads the latest release
# binary for the current platform — no compiler needed.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/cloudroad-io/datadiff/main/install.sh | sh
#
# Optional: DATADIFF_INSTALL_DIR=/custom/dir (default: ~/.local/bin)
set -eu

REPO="cloudroad-io/datadiff"
INSTALL_DIR="${DATADIFF_INSTALL_DIR:-$HOME/.local/bin}"

# --- detect platform -> release target triple ---
os=$(uname -s)
arch=$(uname -m)
case "$os" in
    Linux)  os_part="unknown-linux-gnu" ;;
    Darwin) os_part="apple-darwin" ;;
    *) echo "error: unsupported OS '$os' (on Windows use install.ps1)" >&2; exit 1 ;;
esac
case "$arch" in
    x86_64|amd64)  arch_part="x86_64" ;;
    arm64|aarch64) arch_part="aarch64" ;;
    *) echo "error: unsupported architecture '$arch'" >&2; exit 1 ;;
esac
target="$arch_part-$os_part"

# --- resolve the latest release tag ---
tag=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
    | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p')
if [ -z "$tag" ]; then
    echo "error: cannot determine the latest release of $REPO" >&2
    exit 1
fi
url="https://github.com/$REPO/releases/download/$tag/datadiff-$tag-$target.tar.gz"

# --- download and install ---
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
echo "Downloading $url"
curl -fsSL "$url" -o "$tmp/datadiff.tar.gz"
tar -xzf "$tmp/datadiff.tar.gz" -C "$tmp"
mkdir -p "$INSTALL_DIR"
mv "$tmp/datadiff" "$INSTALL_DIR/datadiff"
chmod +x "$INSTALL_DIR/datadiff"

echo "Installed datadiff $tag to $INSTALL_DIR/datadiff"
case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *) echo "note: $INSTALL_DIR is not in PATH — add it, e.g.: export PATH=\"$INSTALL_DIR:\$PATH\"" ;;
esac
