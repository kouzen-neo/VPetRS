#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

FORBIDDEN_TREE='dotnet|mono|avalonia|coreclr'
FORBIDDEN_LDD='coreclr|mono|dotnet'

if cargo tree --workspace 2>/dev/null | grep -Eiq "$FORBIDDEN_TREE"; then
    echo "FAIL: forbidden .NET/Avalonia crate in cargo tree" >&2
    cargo tree --workspace | grep -Ei "$FORBIDDEN_TREE" >&2 || true
    exit 1
fi

BIN=""
for candidate in "$ROOT/target/release/vpet-rs" "$ROOT/target/debug/vpet-rs"; do
    if [[ -x "$candidate" ]]; then
        BIN="$candidate"
        break
    fi
done

if [[ -z "$BIN" ]]; then
    cargo build --quiet -p vpet-app
    BIN="$ROOT/target/debug/vpet-rs"
fi

if ! file "$BIN" | grep -q 'ELF'; then
    echo "FAIL: $BIN is not a native ELF binary" >&2
    file "$BIN" >&2
    exit 1
fi

if ldd "$BIN" | grep -Eiq "$FORBIDDEN_LDD"; then
    echo "FAIL: $BIN links against forbidden .NET runtime" >&2
    ldd "$BIN" | grep -Ei "$FORBIDDEN_LDD" >&2 || true
    exit 1
fi

echo "OK: pure native ELF binary with zero .NET dependencies ($BIN)"
