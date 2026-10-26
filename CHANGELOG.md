# Changelog

All notable changes to VPetRust are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [v0.2.0] - 2026-10-27

### Added

- **LinePutScript (`.lps`) Parser (`vpet-content`)**:
  - Implemented `LpsDocument`, `LpsLine`, and `LpsSub` supporting UTF-8 BOM, CRLF/LF, `///` whole-line and inline trailing comments, `:|` and `#` escapes (`/|`, `/#`, `/:`, `\n`, `\r`, `\t`, `\\`), and duplicate sub-key ordering.
  - Added golden test suite verified directly against `0000_core` (`info.lps`, `pet/vup.lps`, `food/food.lps`, `text/ClickText.lps`, `theme/default.lps`, `lang/en/Base2608.lps`).
- **VPet Animation Folder Indexer & Pet Metadata**:
  - Implemented `AnimationIndex` with full `PetLoader.LoadGraph` + `GraphInfo` parity across all 23 `GraphType` variants, `AnimatType` (`Single`, `AStart`, `BLoop`, `CEnd`), `ModeType` bitmasks (`Happy`, `Nomal`, `PoorCondition`, `Ill`), natural frame sorting, `_<ms>.png` duration extraction, and `FoodAnimation` keyframe parsing.
  - Added `PetConfig` parser for `pet/vup.lps` (`500x500` logical hit boxes `touchhead`, `touchbody`, `pinch`, `touchraised`, `raisepoint`, `work`, and `move` rules).
- **Content Databases & Mod Scanner (`vpet-content`, `vpet-mods`)**:
  - Added `FoodDatabase`, `TextDatabase` (`ClickText`, `LowText`), `ThemeDef` (ARGB hex + `.ttf`/`.otf` font discovery), and `LangTable` (`lang/<locale>/*.lps` + `info.lps` localization).
  - Implemented `ContentDatabase` and `ModLoader` enforcing `0000_core`-first load priority, deterministic mod overrides, non-fatal corrupt-file warnings, and explicit rejection/warning of legacy `.NET` `.dll` plugins.
  - Added `docs/assets-format.md` documenting LPS grammar, coordinate spaces, animation path resolution, and mod priority rules.

## [v0.1.0] - 2026-10-27

### Added

- **Wayland Layer-Shell Surface (`vpet-wayland`)**:
  - Implemented `WaylandShell` on `smithay-client-toolkit` + `calloop` binding `wl_compositor`, `wl_shm`, `wl_output`, `wl_seat`, and `zwlr_layer_shell_v1` with automatic `$XDG_RUNTIME_DIR/wayland-*` socket discovery.
  - Configured full-output `vpet-rs` layer surface (`Anchor::TOP | BOTTOM | LEFT | RIGHT`, `exclusive_zone = -1`, `KeyboardInteractivity::None`) with runtime `Top` / `Overlay` layer switching (`SIGUSR2`) and output disconnect fallback.
  - Added double/triple-buffered `ShmSwapchain` with per-slot dirty-rectangle clearing, `wl_surface.damage_buffer` union tracking, and `wl_surface.frame` callback throttling (1 commit when idle, smooth 30 FPS when walking).
- **Alpha-Silhouette Input Region & Click-Through**:
  - Added `alpha_mask_rects_scaled` to decompose non-transparent sprite pixels into `< 512` surface-local `wl_region` spans (`77` rects on `250x250` VPet idle sprite).
  - Implemented strict click-through mode (`SIGUSR1` / `VPET_CLICKTHROUGH=1`) using an explicit empty `wl_region` (`Some(&empty_region)`) rather than `None` so clicks pass cleanly to underlying windows without focus theft.
  - Added `PointerTracker` with themed cursor icons (`default` / `grabbing`) and implicit-grab pointer dragging clamped to output bounds.
- **CPU Sprite Renderer (`vpet-render`)**:
  - Added PNG/WebP/GIF first-frame loader, bilinear `tiny-skia` scaler, premultiplied RGBA -> little-endian `Argb8888` (BGRA) blitter, procedural fallback mascot, and `DamageRect` math.
- **Gate 0 Niri Verification Report**:
  - Verified live on `niri` (`eDP-1` `1920x1080` scale `1`, `24.6 MB` RSS) and recorded results in `docs/gate0-report.md`.

## [v0.0.1] - 2026-10-26

### Added

- **Cargo Workspace Bootstrap**:
  - Initialized multi-crate Cargo workspace (`vpet-core`, `vpet-content`, `vpet-render`, `vpet-wayland`, `vpet-app`, `vpet-mods`, and excluded post-MVP `vpet-steam`) targeting Rust edition 2024.
  - Enforced strict crate dependency boundaries (`vpet-core` isolated from I/O, Wayland, and rendering; `vpet-app` as sole composition root).
  - Pinned stable toolchain (`rustfmt`, `clippy`) via `rust-toolchain.toml` and shared formatting rules via `.editorconfig`.
- **Zero-.NET Verification Gate**:
  - Added `scripts/check-no-dotnet.sh` to verify `vpet-rs` builds as a native ELF binary and fails if `cargo tree` or `ldd` references CoreCLR, Mono, or Avalonia.
- **Architecture & Phase Execution Plan**:
  - Added `IMPLEMENTATION.md` specifying the niri-first `wlr-layer-shell` design, `tiny-skia` + `wl_shm` rendering pipeline, LPS content compatibility, deterministic simulation, IPC, and AUR packaging.
  - Added `TODO.md` with Phase 0–9 task checklists, Gate 0 acceptance criteria, and conventional commit/tag map.
  - Added `AGENTS.md` and `README.md` documenting non-negotiable constraints, build gates, and KZKT-style changelog/commit rules.

---

*Full history: see `git log --oneline`.*
