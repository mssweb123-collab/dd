#!/bin/bash
# Build script for MSS School Billing App
# This script builds the Tauri application for production

set -e

echo "==========================================="
echo " Building MSS School Billing Application..."
echo "==========================================="

# Ensure cargo is in PATH
export PATH="$HOME/.cargo/bin:$PATH"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Install npm dependencies
echo "[1/3] Installing npm dependencies..."
npm install

# Build the Tauri application
echo "[2/3] Building Tauri application..."
npm run tauri build

echo "==========================================="
echo " Build complete!"
echo "==========================================="
echo "You can find the built application in:"
echo "  src-tauri/target/release/bundle/"
echo "==========================================="
