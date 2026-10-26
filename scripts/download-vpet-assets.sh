#!/usr/bin/env bash
set -euo pipefail

DEST="${1:-${XDG_DATA_HOME:-$HOME/.local/share}/vpet-rs/content/0000_core}"

if [[ -d "$DEST" && -f "$DEST/info.lps" ]]; then
    echo "VPet core assets already installed at: $DEST"
    exit 0
fi

# 1. Check local Steam installation
for candidate in \
    "$HOME/.local/share/Steam/steamapps/common/VPet/mod/0000_core" \
    "$HOME/.steam/steam/steamapps/common/VPet/mod/0000_core" \
    "$HOME/.steam/root/steamapps/common/VPet/mod/0000_core"; do
    if [[ -d "$candidate" && -f "$candidate/info.lps" ]]; then
        echo "Found existing VPet installation on Steam: $candidate"
        mkdir -p "$(dirname "$DEST")"
        ln -sf "$candidate" "$DEST"
        echo "Linked to: $DEST"
        exit 0
    fi
done

# 2. Check local dev directory if present
if [[ -d "/home/kouzen/Documents/VPet/VPet-Simulator.Windows/mod/0000_core" ]]; then
    candidate="/home/kouzen/Documents/VPet/VPet-Simulator.Windows/mod/0000_core"
    mkdir -p "$(dirname "$DEST")"
    ln -sf "$candidate" "$DEST"
    echo "Linked local dev assets to: $DEST"
    exit 0
fi

# 3. Download via git sparse-checkout (only 0000_core, not the whole repository)
echo "Downloading VPet 0000_core assets via git sparse-checkout..."
TMP=$(mktemp -d -t vpet-assets-XXXXXX)
trap 'rm -rf "$TMP"' EXIT

git clone --depth 1 --filter=blob:none --sparse https://github.com/LorisYounger/VPet.git "$TMP"
git -C "$TMP" sparse-checkout set VPet-Simulator.Windows/mod/0000_core

mkdir -p "$(dirname "$DEST")"
mv "$TMP/VPet-Simulator.Windows/mod/0000_core" "$DEST"

echo "Successfully installed VPet assets to: $DEST"
