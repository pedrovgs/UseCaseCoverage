#!/usr/bin/env bash

set -euo pipefail

REPO="pedrovgs/UseCaseCoverage"
GITHUB_API="https://api.github.com/repos/$REPO/releases/latest"

echo "🔍 Detecting latest release..."
RELEASE_DATA=$(curl -fsSL "$GITHUB_API")
LATEST_TAG=$(echo "$RELEASE_DATA" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

echo "📦 Latest version is $LATEST_TAG"

OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$ARCH" in
    x86_64) ARCH_PATTERN="x86_64" ;;
    aarch64|arm64) ARCH_PATTERN="aarch64" ;;
    *) ARCH_PATTERN="$ARCH" ;;
esac

if [[ "$OS" == "linux" ]]; then
    if command -v dpkg > /dev/null; then
        echo "🐧 Debian-based system detected. Searching for .deb package..."
        DEB_URL=$(echo "$RELEASE_DATA" | grep "browser_download_url" | grep ".deb" | grep "$ARCH_PATTERN" | head -n 1 | cut -d '"' -f 4 || true)
        
        if [[ -n "$DEB_URL" ]]; then
            TEMP_DEB=$(mktemp).deb
            echo "📥 Downloading $DEB_URL..."
            curl -fsSL -o "$TEMP_DEB" "$DEB_URL"
            echo "🚀 Installing package..."
            if sudo dpkg -i "$TEMP_DEB"; then
                rm "$TEMP_DEB"
                echo "✅ UseCaseCoverage installed successfully via .deb!"
                exit 0
            else
                echo "⚠️ dpkg installation failed. Falling back to binary install."
                rm "$TEMP_DEB"
            fi
        else
            echo "⚠️ Could not find a .deb package for $ARCH. Falling back to binary install."
        fi
    fi
fi

# Fallback to binary install (tar.gz)
echo "🔧 Falling back to binary installation..."
BINARY_URL=$(echo "$RELEASE_DATA" | grep "browser_download_url" | grep ".tar.gz" | grep "$OS" | grep "$ARCH_PATTERN" | head -n 1 | cut -d '"' -f 4 || true)

if [[ -z "$BINARY_URL" ]]; then
    echo "❌ Error: Could not find a suitable binary for $OS/$ARCH"
    exit 1
fi

TEMP_DIR=$(mktemp -d)
echo "📥 Downloading $BINARY_URL..."
curl -fsSL "$BINARY_URL" | tar -xz -C "$TEMP_DIR"

# Find the binary in the extracted files (it might be in a subdirectory)
UCC_BIN=$(find "$TEMP_DIR" -type f -name "ucc" | head -n 1)

if [[ -n "$UCC_BIN" ]]; then
    echo "🚀 Installing binary to /usr/local/bin..."
    sudo mv "$UCC_BIN" /usr/local/bin/ucc
    chmod +x /usr/local/bin/ucc
    rm -rf "$TEMP_DIR"
    echo "✅ UseCaseCoverage installed successfully!"
else
    echo "❌ Error: Could not find 'ucc' binary in the downloaded archive."
    rm -rf "$TEMP_DIR"
    exit 1
fi
