# VPetRust

Niri-first Wayland desktop pet written in 100% Rust (`wlr-layer-shell` + `tiny-skia`), reusing VPet content data (`.lps` and sprite folders) with zero .NET dependencies.

## Status

Phase 0 bootstrap complete (`v0.0.1`). Next milestone: **Phase 1 / Gate 0** (`wlr-layer-shell` spike on niri).

- Architecture & specification: [`IMPLEMENTATION.md`](IMPLEMENTATION.md)
- Phase board & commit map: [`TODO.md`](TODO.md)
- Release history: [`CHANGELOG.md`](CHANGELOG.md)
- Contributor / agent rules: [`AGENTS.md`](AGENTS.md)

## Quick checks

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace
./scripts/check-no-dotnet.sh
```

## License

- Code: `MIT OR Apache-2.0`.
- VPet assets (`0000_core`) are user-supplied and remain subject to upstream VUP-Simulator terms (attribution required, non-commercial without permission).
