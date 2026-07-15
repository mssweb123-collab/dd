#!/bin/bash
# Script to easily start the MSS School Billing App

# Add Cargo to PATH
export PATH="$HOME/.cargo/bin:$PATH"

# Get script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Change directory to the app folder
cd "$SCRIPT_DIR"

echo "==========================================="
echo " Starting MSS School Billing Application..."
echo "==========================================="

# Start the Tauri dev environment
npm run tauri dev
