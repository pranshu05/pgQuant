#!/bin/bash
set -e

# pgQuant installation script

REPO="pranshu05/pgQuant"
GH_API="https://api.github.com/repos/$REPO/releases/latest"

echo "Installing pgQuant..."

# Check pg_config
if ! command -v pg_config >/dev/null 2>&1; then
    echo "Error: pg_config not found. Please ensure PostgreSQL development packages are installed and pg_config is in your PATH."
    exit 1
fi

PG_VERSION=$(pg_config --version | awk '{print $2}' | cut -d. -f1)
echo "Detected PostgreSQL version: $PG_VERSION"

if [[ ! "$PG_VERSION" =~ ^(14|15|16|17)$ ]]; then
    echo "Error: pgQuant only supports PostgreSQL 14, 15, 16, and 17. Detected: $PG_VERSION"
    exit 1
fi

# Detect OS and Arch
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

if [ "$OS" != "linux" ]; then
    echo "Error: Pre-compiled binaries are currently only available for Linux."
    echo "Please build from source. See https://github.com/$REPO for instructions."
    exit 1
fi

if [[ "$ARCH" == "x86_64" || "$ARCH" == "amd64" ]]; then
    ARCH="amd64"
else
    echo "Error: Pre-compiled binaries are currently only available for amd64 architectures."
    echo "Please build from source. See https://github.com/$REPO for instructions."
    exit 1
fi

# Fetch latest version
echo "Fetching latest release information..."
LATEST_RELEASE=$(curl -sSL "$GH_API")
VERSION=$(echo "$LATEST_RELEASE" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$VERSION" ]; then
    echo "Error: Could not determine latest version from GitHub."
    exit 1
fi

echo "Latest version is $VERSION"

ASSET_NAME="pgquant-${VERSION}-pg${PG_VERSION}-${OS}-${ARCH}.zip"
# Fallback to check if the URL exists in the assets
DOWNLOAD_URL=$(echo "$LATEST_RELEASE" | grep -o "\"browser_download_url\": \"[^\"]*${ASSET_NAME}\"" | cut -d'"' -f4)

if [ -z "$DOWNLOAD_URL" ]; then
    echo "Error: Could not find release asset $ASSET_NAME for version $VERSION."
    exit 1
fi

echo "Downloading $ASSET_NAME..."
TMP_DIR=$(mktemp -d)
curl -sSL "$DOWNLOAD_URL" -o "$TMP_DIR/$ASSET_NAME"

echo "Unzipping..."
unzip -q "$TMP_DIR/$ASSET_NAME" -d "$TMP_DIR/extracted"

PG_LIB=$(pg_config --pkglibdir)
PG_EXT=$(pg_config --sharedir)/extension

echo "Installing files to PostgreSQL directories (may prompt for sudo)..."
sudo cp "$TMP_DIR/extracted"/pgquant.so "$PG_LIB/"
sudo cp "$TMP_DIR/extracted"/pgquant.control "$PG_EXT/"
sudo cp "$TMP_DIR/extracted"/pgquant--*.sql "$PG_EXT/"

# Cleanup
rm -rf "$TMP_DIR"

echo ""
echo "============================================================"
echo "pgQuant $VERSION successfully installed!"
echo "To enable it, connect to your database and run:"
echo ""
echo "    CREATE EXTENSION pgquant;"
echo "============================================================"
