
#!/bin/sh

# flutter-wipe installer script
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/AmanSikarwar/flutter-wipe/main/install.sh | sh
#

set -e

# --- Helper Functions ---
echo_color() {
  local color_code=$1
  shift
  echo "\033[${color_code}m$@\033[0m"
}

info() {
  echo_color "34" "$@" # Blue
}

error() {
  echo_color "31" "$@" >&2 # Red
}

# --- Main Installation Logic ---

REPO="AmanSikarwar/flutter-wipe"
INSTALL_DIR="/usr/local/bin"
EXE_NAME="flutter-wipe"

# Determine OS and Architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

case $OS in
  Linux)
    PLATFORM="linux-x86_64"
    ;;
  Darwin)
    PLATFORM="macos-universal"
    ;;
  *)
    error "Unsupported operating system: $OS"
    exit 1
    ;;
esac

# Get the latest release tag from GitHub API
LATEST_TAG=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
if [ -z "$LATEST_TAG" ]; then
  error "Could not find the latest release tag for $REPO."
  exit 1
fi

DOWNLOAD_URL="https://github.com/$REPO/releases/download/$LATEST_TAG/flutter-wipe-$PLATFORM.tar.gz"
TMP_DIR=$(mktemp -d)

info "Downloading $EXE_NAME from $DOWNLOAD_URL..."
curl -fsSL "$DOWNLOAD_URL" -o "$TMP_DIR/release.tar.gz"

info "Extracting files..."
tar -xzf "$TMP_DIR/release.tar.gz" -C "$TMP_DIR"

info "Installing $EXE_NAME and fw to $INSTALL_DIR (requires sudo)..."
if [ -w "$INSTALL_DIR" ]; then
    SUDO=""
else
    SUDO="sudo"
fi

$SUDO mv "$TMP_DIR/flutter-wipe" "$INSTALL_DIR/flutter-wipe"
$SUDO mv "$TMP_DIR/fw" "$INSTALL_DIR/fw"

# Cleanup
rm -rf "$TMP_DIR"

info "✅ Successfully installed flutter-wipe to $INSTALL_DIR"
info "You can now run 'flutter-wipe' or 'fw' from anywhere."