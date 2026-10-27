# Changelog

All notable changes to VPetRust are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Desktop Right-Click Context Menu (`vpet-render`, `vpet-wayland`, `vpet-app`)**:
  - Right-clicking directly on the pet character on the desktop immediately opens a styled context menu right beside the pet.
  - Features full navigation mirroring the Waybar tray menu: Status Header, Visibility/Click-Through/Overlay toggles, Sleeping, Self-Care, Auto-Work, Quick Snack/Water, Food Store submenu with pricing, Backpack inventory submenu with stack counts, Dance to Music, Work/Study/Play activities with level locks, Cancel Work, Chat with Pet prompt, Reload Config, and Quit.
  - Automatically captures clicks outside to dismiss the menu and restores the transparent alpha input region instantly.
  - Pauses pet auto-walk while the menu is open to prevent the pet from walking away from an open menu.
  - Built with 100% pure Rust `tiny-skia` + `cosmic-text` without any GTK, Qt, or external UI toolkits.

## [v1.6.0] - 2026-10-28

### Added

- **Backpack Inventory System (`vpet-app`, `vpet-core`)**:
  - Added ability to stockpile food items in the pet's backpack via `vpet-rs ctl buy <item> [count]`.
  - Added backpack inspection command `vpet-rs ctl bag` (alias: `inventory`).
  - Prioritized consuming items from the backpack for $0 before spending funds when feeding.
  - Added interactive `Backpack` submenu to the Waybar StatusNotifierItem tray menu showing owned item stacks (e.g. `Hamburg (x3)`) with one-click consumption.
- **Music Catch & Dancing Reactions (`vpet-app`, `vpet-content`)**:
  - Connected canonical `Music` animation sets (`Music/A` AStart -> `Music/B` BLoop -> `Music/C` CEnd) across all pet modes.
  - Added background MPRIS player audio detection (`crates/app/src/music.rs`): automatically triggers dancing when music or video plays while the pet is idle.
  - Added CLI command `vpet-rs ctl dance` (alias: `music`) with configurable loop count.
  - Added `Dance to Music` button to the Waybar tray menu.
  - Preloaded essential `music` clips for 0ms start.

## [v1.5.1] - 2026-10-28

### Added

- **Full Waybar Tray Food Store & Categorized Buying (`vpet-app`)**:
  - Added categorized `Food Store` submenu to the StatusNotifierItem (Waybar tray) menu with `Meals`, `Drinks`, `Snacks`, `Medicine`, and `Gifts & Fun`.
  - Listed every store item with price (e.g. `Hamburg ($28.00)`, `Cola ($10.00)`), automatically enabling affordable items and graying out items exceeding current funds.
  - Allowed direct buying and feeding from the tray menu, displaying the food name in the speech bubble and rendering the specific food item icon in the pet's hands.
  - Added CLI command `vpet-rs ctl foods` (alias: `foodstore`) with optional category filter to inspect prices, nutrient restoration, and affordability.
  - Retained quick one-click `Quick Snack` and `Quick Water` buttons in the tray menu.

## [v1.5.0] - 2026-10-27

### Added

- **Screen Bezel Wall Alignment & Edge Drop Auto-Climbing (`SideHide`)**:
  - Aligned `SideHide` animations with the physical screen bezel by applying VPet's canonical wall offsets (`219` left, `281` right in 500px canvas space, scaled proportionally) into negative surface X on the left and overscan X on the right.
  - Allowed `WaylandShell` positioning to render partially off-screen while cleanly clipping `DamageRect` and clamping `wl_region` input rects within surface bounds.
  - Automatically triggered wall climbing when releasing a dragged pet within 60 px of either screen edge.
  - Increased border collision threshold to 25 px and added directed wall-seeking behavior during idle walks.
  - Added CLI command `vpet-rs ctl side-hide` (alias: `sidehide`) with `--left` and `--right` flags to manually trigger wall climbing on demand.

### Fixed

- **Food Item Icon Lookup & Keyframe Opacity (`vpet-content`, `vpet-app`)**:
  - Parsed `image#` attribute from LPS food definitions (e.g. `黄金月饼.png` for `` `金`秋月饼 ``), resolving missing food icon paths.
  - Fixed keyframe opacity parsing so fractional opacity values (such as `0.375`) are not erroneously divided by 100 into near-zero transparency.
  - Prioritized items categorized under `Snack` with guaranteed icons when feeding snacks, and added fallback sandwich/bread icons.

## [v1.4.1] - 2026-10-27

### Fixed

- **Feeding Frame Composition & Async Playback Timing**:
  - Sliced 3-layer `FoodAnimation` clips at union timestamps of background, foreground, and keyframes so no animation frames freeze during long eating intervals.
  - Prioritized mode-specific subdirectories when resolving `back_lay` and `front_lay` PNG folders (fixing `Drink` in `Nomal` and `Ill` modes).
  - Ensured single transient animation playback pauses until asynchronous background decoding completes rather than prematurely cutting off in 125ms.
  - Rendered food item icon in the pet's hand for `eat`, `drink`, and `gift` actions.
  - Prioritized pure water bottles for `water` queries and tray menu actions.

## [v1.4.0] - 2026-10-27

### Added

- **Self-Care (Auto-Feed) & Auto-Work System (`vpet-app`)**:
  - Added `self_care` toggle (default: `true`) under `[simulation]` in `config.toml`: automatically feeds and hydrates the pet with snacks/water when hunger or thirst drops below 30% and stored nutrients are low.
  - Added `auto_work` toggle (default: `false`) under `[simulation]` in `config.toml`: automatically starts the highest unlocked `Work` job when the pet is well-fed, idle, healthy, and has sufficient stamina.
  - Added CLI commands `vpet-rs ctl toggle-selfcare` and `vpet-rs ctl toggle-autowork`.
  - Added interactive checkmarks for `Self-Care (Auto-Feed)` and `Auto-Work` to the StatusNotifierItem waybar tray menu.
  - Resumed previous `Working` action state seamlessly when transient actions (such as auto-feeding or being petted) complete.

## [v1.3.1] - 2026-10-27

### Fixed

- **Starvation & Study/Play Balance Invariants**:
  - Prevented continuous health drain when hunger/thirst are already at 0.0 (penalty only applies once during depletion transition; gentle starvation damage replaces runaway drain).
  - Fixed sleep metabolism so stamina and health regenerate without draining hunger or thirst, matching VPet C# AFK design.
  - Clamped money to non-negative values and automatically canceled study/play tasks when funds are insufficient to pay tuition.
  - Sanitized existing negative save balance with automatic $10 recovery allowance on boot.
- **Surface & Speech Bubble Alignment**:
  - Removed top clamping in speech bubble composition so the pet's coordinates remain perfectly anchored when speech bubbles appear and disappear.
- **Command & Tray Usability**:
  - Added dedicated `Give Water` action in StatusNotifierItem tray menu.
  - Separated `IpcRequest::ListWorks` (`vpet-rs ctl works`) from `IpcRequest::Work` (`vpet-rs ctl work [name]`).
  - Added fallback to free food/water in `feed` command when money is completely depleted.

## [v1.3.0] - 2026-10-27

### Added

- **100% Complete VPet Animation Parity Across All 23 Categories**:
  - Connected `StartUp` (played on daemon boot) and `Shutdown` (played before exit).
  - Connected `Pinch` (cheeks pinch animation chaining `A_Start -> B_Loop -> C_End` when clicking pinch hit box).
  - Connected `Eating` & `Drink` 3-layer `FoodAnimation` compositing `back_lay` + `front_lay` PNG sequences and item positions.
  - Connected `LevelUp` celebratory animation when pet gains enough experience.
  - Connected `SideHide` border interactions (`SideHide_Left_Main` / `SideHide_Left_Rise` and `SideHide_Right_Main` / `SideHide_Right_Rise`) climbing and hiding at screen edges during auto-walk.
  - Connected `Idel` fidget animations (Squat, Yawning, Meow, Bubbles, Boring, Tennis, amusement, etc.) randomly during idle periods.
  - Connected `StateOne` alternative idle state.
  - Connected `Switch` state transition animations (`Switch_Up`, `Switch_Down`, `Switch_Hunger`, `Switch_Thirsty`).
  - Connected `Think` animation while AI LLM queries are being processed.

## [v1.2.0] - 2026-10-27

### Added

- **OpenAI-Compatible AI Chat Integration (`vpet-app::ai`)**:
  - Implemented background `AiWorker` thread running HTTP chat completions without blocking Wayland presentation or input handling.
  - Defaults to `http://localhost:20128/v1` with model `9routerall` (also compatible with Ollama, LM Studio, DeepSeek, and OpenAI).
  - Handles both standard JSON and Server-Sent Events (SSE `text/event-stream`) streaming chunk aggregation.
  - Added CLI `vpet-rs ctl chat "<prompt>"` and `vpet-rs ctl prompt-chat`.
  - Added StatusNotifierItem tray menu item `"Chat with Pet..."` with desktop prompt fallback (`fuzzel`, `zenity`, or `kdialog`).
  - Integrated speech bubble replies with `Say` animation transitions and likability, feeling, and experience rewards.

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
