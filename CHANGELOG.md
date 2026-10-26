# Changelog

All notable changes to VPetRust are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [v1.1.0] - 2026-10-27

### Added

- **Work, Study, and Play System (`vpet-app`)**:
  - Wired loaded `vup.lps` work definitions (`Copywriting`, `Study`, `PlayONE`, `GrilledSausage`, `FixMenu`, `WorkClean`, `WorkTWO`, `Calligraphy`, `StudyPaint`, `RemoveObject`, `RopeSkipping`, `PlayWater`) into simulation `WorkSession` with rate scaling, finish bonuses, and completion speech bubbles.
  - Added CLI commands `vpet-rs ctl works`, `vpet-rs ctl work [name]`, and `vpet-rs ctl cancel-work`.
  - Added StatusNotifierItem tray submenu under "Work / Study / Play" with live job progress countdown (`Working: <Name> (<M>m left)`) and cancellation.
- **Legacy VPet Save Importer (`vpet-rs import-save`)**:
  - Added one-way conversion from legacy VPet-Simulator `Save.lps` / `GameSave_v2` into native atomic MessagePack saves (`~/.local/share/vpet-rs/saves/<profile>.mpak`).
- **Theme Color Application**:
  - Bound active `ThemeDef` colors (`Primary`, `PrimaryLighter`, `PrimaryText`) to `BubbleStyle` for styled chat bubbles.

## [v1.0.1] - 2026-10-27

### Fixed

- **Complete English Localization for VPet Content (`vpet-content`)**:
  - Localized all loaded `ClickText`, `LowText`, `FoodDatabase` names and descriptions, `PetConfig` metadata, `WorkDef` labels, and `ThemeDef` names through `lang/en/*.lps` during content pack loading.
  - Filtered out untranslated CJK strings and sanitized `{hostname}` / `/id` placeholders when running in English locale (`"en"`) so speech bubbles, tray labels, and CLI commands (`vpet-rs ctl feed "Fruit Juice"`) display strictly in English.

## [v1.0.0] - 2026-10-27

### Added

- **Fractional Scaling (`wp_fractional_scale_v1` + `wp_viewporter`)**:
  - Bound `wp_fractional_scale_manager_v1` and `wp_viewporter` in `vpet-wayland` (`scale_120` fixed-point math) so fractional outputs (such as `125%` and `150%`) render at native physical resolution and map surface-local `wl_region` spans accurately alongside integer scales.
- **24-Hour Soak Test & Troubleshooting Guide**:
  - Added `864,000`-tick (`24 h` at `10 Hz`) deterministic soak test verifying bounded stats, finite floating-point math, and buff expiry stability.
  - Finalized `README.md`, `IMPLEMENTATION.md`, and `docs/troubleshooting.md`.

### Changed

- **Release Profiling & Minimal Native Runtime Footprint**:
  - Profiled stripped `8.2 MiB` `--release` binary live on `niri` (`1920x1080` `eDP-1` scale `1`): measured **0.67% single-core CPU** and **60.4 MiB RSS** with `621` indexed animations and `151` foods loaded.
  - Trimmed `packaging/PKGBUILD` runtime dependencies to `wayland`, `gcc-libs`, and `glibc` (`ldd` verified pure-Rust D-Bus and fontconfig parsers).

### Fixed

- **Compositor Close & Output Hotplug Viewport Cleanup**:
  - Released `wp_viewport` and `wp_fractional_scale_v1` protocol objects cleanly before migrating or recreating layer surfaces on output disconnect.

## [v0.7.0] - 2026-10-27

### Added

- **Arch Linux AUR Packaging & Desktop Integration (`packaging/`)**:
  - Added `packaging/PKGBUILD` (`vpet-rs-git`) with locked Cargo build, test, and zero-`.NET` `check()` verification.
  - Added `packaging/vpet-rs.desktop` launcher entry and `packaging/niri-snippet.kdl` reference snippet (`layer-rule` with optional `block-out-from "screencast"`, autostart, and IPC keybind examples).
- **CI Workflow & Niri Documentation**:
  - Added `.github/workflows/ci.yml` and `scripts/run-headless-test.sh` enforcing `rustfmt`, `clippy -D warnings`, workspace tests, release build, and `scripts/check-no-dotnet.sh`.
  - Added `docs/niri.md`, `LICENSE-MIT`, and `LICENSE-APACHE`.

## [v0.6.0] - 2026-10-27

### Added

- **TOML Configuration & Live Hot-Reload (`vpet-app::config`)**:
  - Added `$XDG_CONFIG_HOME/vpet-rs/config.toml` (`[app]`, `[display]`, `[pet]`, `[simulation]`, `[audio]`) with `vpet-rs init-config` generator and automatic mtime + IPC live reload (`layer`, `scale`, `output`, `click_through`, `auto_walk`, `cache_mb`, `autosave_secs`).
- **Unix Socket IPC & `vpet-rs ctl` CLI (`vpet-app::ipc`)**:
  - Added `$XDG_RUNTIME_DIR/vpet-rs.sock` (`0600` mode, `SO_PEERCRED` UID verification, `4096`-byte line cap) and `vpet-rs ctl {show, hide, toggle-clickthrough, layer, feed, sleep, wake, say, reload, status, quit}` subcommands.
  - Added `docs/ipc.md` documenting CLI usage, security invariants, and JSON wire payloads.
- **StatusNotifierItem Tray & Full Daemon Orchestration (`vpet-app`)**:
  - Integrated `ksni` StatusNotifierItem tray for `waybar` with custom `32x32` ARGB mascot icon and live toggle checkmarks (`Show Pet`, `Click-Through`, `Overlay Above Fullscreen`, `Sleeping`, `Feed Snack`, `Reload Config`, `Quit`).
  - Added single-instance `flock` guard (`$XDG_RUNTIME_DIR/vpet-rs.lock`), size-capped rolling file+stderr `tracing` logs (`<= 2 MiB`), periodic autosave, clean shutdown save on `SIGINT`/`SIGTERM`/`quit`, and lazy `FontSystem` initialization.

## [v0.5.0] - 2026-10-27

### Added

- **Priority Animation State Machine & `A -> B -> C` Chaining (`vpet-app::anim`)**:
  - Implemented `InteractionController` enforcing state priorities (`Raised > Touch > Working > Saying > Moving > Idle > Sleeping`) with automatic `A_Start -> B_Loop -> C_End` segment chaining and `Single` animation fallback.
  - Selected animation variants deterministically using current `ModeType` (`Happy`, `Nomal`, `PoorCondition`, `Ill`) and `World` PRNG state.
- **Logical `500x500` Hit-Region Classifier & Desktop Interactions**:
  - Added `classify_hit_zone` mapping scaled pet clicks back to `vup.lps` `touchhead`, `touchbody`, and `pinch` zones to trigger `Touch_Head` / `Touch_Body` reactions, stat gains, and `ClickText` speech bubbles.
  - Added drag-raise (`Raised_Dynamic` -> `Raised_Static` drop), sleep/wake transitions, horizontal auto-walk (`walk.left` / `walk.right`) with screen-edge bounce/flip, per-output saved coordinates, and headless state-machine tests against real `0000_core/pet/vup` assets.

## [v0.4.0] - 2026-10-27

### Added

- **Pure Deterministic Simulation Core (`vpet-core`)**:
  - Implemented `PetState` with upstream `GameSave_VPet` stat caps (`StrengthMax`, `FeelingMax`), leveling (`200 * Level - 100`), starvation spillover into `Health`/`Likability`, buffered digestion (`StoreTake`), and `CalMode` (`Happy`, `Nomal`, `PoorCondition`, `Ill`).
  - Added `World` 10 Hz deterministic tick engine with injected timestamps, seeded `SplitMix64` PRNG (`DeterministicRng`), stack-bounded `Inventory`, `FoodEffect` consumption, timed `BuffState` multipliers, and `WorkSession` progression while keeping `vpet-core` 100% free of filesystem and wall-clock I/O.
- **Crash-Safe MessagePack Save System & Backup Recovery**:
  - Added `SaveFile` (`rmp-serde` named MessagePack `v1`) with explicit schema migration (`migrate_v0_to_v1`) and per-output saved coordinates.
  - Added `SaveStore` atomic writer (`temp -> fsync -> .bak.1..5 rotation -> rename -> dir fsync`) with automatic fallback to `.bak.1`..`.bak.5` when primary save data is corrupted.
  - Added `docs/gameplay-formulas.md` and `docs/save-format.md`.

## [v0.3.0] - 2026-10-27

### Added

- **Animation Frame Decoders & Food Layer Compositor (`vpet-render`)**:
  - Added `decode_png_sequence`, `decode_animated_file` (APNG + GIF frame delays), and `decode_animation_source` scaling sprites into premultiplied `Arc<Pixmap>` clips.
  - Added `compose_food_animation` compositing VPet 3-layer eating/drinking animations (`back_lay` + translated/scaled/rotated/alpha-faded item icon + `front_lay`).
- **Byte-Bounded LRU Cache & Background Worker**:
  - Added `FrameCache` with configurable byte cap (256 MiB default), MRU promotion on hit, and deterministic LRU eviction.
  - Added `AsyncClipLoader` worker thread (`vpet-decode`) with in-flight request deduplication and startup preloading so disk/PNG decoding never runs inside Wayland `frame` callbacks.
- **Pure-Rust Chat Bubble Renderer (`cosmic-text` + `tiny-skia`)**:
  - Implemented `BubbleRenderer` for shaped multi-line text inside anti-aliased rounded speech bubbles with custom `.ttf`/`.otf` font loading.
  - Integrated `set_bubble` scene composition in `vpet-wayland` while keeping `wl_region` hit-testing strictly bound to the pet silhouette so speech bubbles never block clicks.
  - Added deterministic PNG snapshot, GIF decode, LRU eviction, and async loader tests.

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
