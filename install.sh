#!/bin/bash

set -e

echo "🔨 Building in release mode..."
cargo build --release

# Determine install location
if [ -w "/usr/local/bin" ]; then
    INSTALL_DIR="/usr/local/bin"
elif [ -d "$HOME/.local/bin" ]; then
    INSTALL_DIR="$HOME/.local/bin"
else
    mkdir -p "$HOME/.local/bin"
    INSTALL_DIR="$HOME/.local/bin"
fi

echo "📦 Installing tedtui to $INSTALL_DIR..."
cp target/release/tedtui "$INSTALL_DIR/tedtui"
chmod +x "$INSTALL_DIR/tedtui"

echo "📦 Installing tedlist to $INSTALL_DIR..."
cp target/release/tedlist "$INSTALL_DIR/tedlist"
chmod +x "$INSTALL_DIR/tedlist"

echo "✅ Installation complete!"
echo ""
echo "  tedtui installed to: $INSTALL_DIR/tedtui"
echo "  tedlist installed to: $INSTALL_DIR/tedlist"

# Check if the install directory is in PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    echo ""
    echo "⚠️  Warning: $INSTALL_DIR is not in your PATH"
    echo "   Add this line to your ~/.bashrc or ~/.zshrc:"
    echo "   export PATH=\"\$PATH:$INSTALL_DIR\""
else
    echo ""
    echo "You can now run 'tedtui' and 'tedlist' from anywhere!"
fi
