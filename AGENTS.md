# VPetRust — Agent & Maintainer Rules

Read [`IMPLEMENTATION.md`](IMPLEMENTATION.md) and [`TODO.md`](TODO.md) before changing code.

## Non-negotiable rules

1. **100% Rust, zero .NET.** Never introduce CoreCLR, Mono, Avalonia, C# interop, or `.dll` plugin loading. `./scripts/check-no-dotnet.sh` must stay green.
2. **Wayland layer-shell only for the pet surface.** Do not pull in `winit`, `iced`, `egui`, `slint`, or `tao` for the pet window.
3. **Strict crate boundaries:**
   - `vpet-core` has zero workspace dependencies and no direct filesystem, Wayland, or wall-clock access.
   - `vpet-wayland` knows nothing about gameplay.
   - `vpet-app` is the sole composition root.
4. **Input region discipline:** `wl_surface.set_input_region(None)` means the **entire** surface receives input. Click-through MUST set `Some(&empty_wl_region)`.
5. **Commit & changelog discipline (KZKT style):**
   - Conventional commits (`feat(scope): ...`, `fix(scope): ...`, `docs: ...`, `chore: ...`), imperative subject ≤ 72 chars, concise bullet body.
   - **Never** add `Co-Authored-By` or any generator/attribution trailer.
   - `CHANGELOG.md` follows Keep a Changelog + SemVer with bold-lead bullets (`- **Feature Name**: ...`). Document user-visible changes under `## [Unreleased]` and promote to `## [vX.Y.Z] - YYYY-MM-DD` before tagging a phase.
6. **Quality gate before every commit:**
   ```bash
   cargo fmt --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   cargo build --workspace
   ./scripts/check-no-dotnet.sh
   ```
