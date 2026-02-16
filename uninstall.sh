#!/bin/bash

set -e

echo "🗑️  Uninstalling tedtui..."

# Check possible install locations
LOCATIONS=(
    "/usr/local/bin/tedtui"
    "$HOME/.local/bin/tedtui"
)

FOUND=false
for location in "${LOCATIONS[@]}"; do
    if [ -f "$location" ]; then
        echo "Removing: $location"
        rm "$location"
        FOUND=true
    fi
done

if [ "$FOUND" = true ]; then
    echo "✅ tedtui has been uninstalled"
else
    echo "⚠️  tedtui was not found in any standard location"
fi
