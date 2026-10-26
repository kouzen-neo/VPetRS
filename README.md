# VPetRS

**A Rust rewrite of [VPet-Simulator](https://github.com/LorisYounger/VPet), built Wayland-first for the [niri](https://github.com/YaLTeR/niri) compositor.**

VPetRS is a from-scratch reimplementation of [LorisYounger/VPet](https://github.com/LorisYounger/VPet) (the C#/WPF virtual desktop pet) written in **100% Rust**. It runs as a native `wlr-layer-shell` surface and has **zero .NET / CoreCLR / Mono / Avalonia dependencies**. It reads the original VPet content packs directly (`.lps` LinePutScript data and PNG animation folders), so you can use the existing `0000_core` assets and data mods without running any C# code.

> **Note:** VPetRS is an independent community project. It is **not** an official VPet product and is not affiliated with or endorsed by the VPet authors. VPet artwork is **not** included in this repository (see [License & Asset Terms](#license--asset-terms)).

---

## Why a rewrite?

On tiling Wayland compositors such as niri, the original VPet runs as a normal `xdg_toplevel` window, and that causes problems:

- the compositor ignores positions requested by the app, so auto-walk, drag, and position restore don't work;
- transparent pixels still capture clicks, so true click-through is impossible;
- with `focus-follows-mouse`, hovering the pet steals keyboard focus;
- the pet does not follow you across workspaces.

VPetRS renders on `zwlr_layer_surface_v1` instead, which fixes all four:

| Behavior | VPetRS on niri |
|---|---|
| Positioning | Full-output layer surface; the pet moves freely using internal coordinates |
| Click-through | Alpha-silhouette `wl_region` (fewer than 512 spans); an explicit empty region gives full click-through |
| Keyboard focus | `keyboard_interactivity = none`, so focus is never stolen |
| Workspaces | Layer surfaces belong to the output, so the pet stays visible on every workspace |
| Fullscreen | `top` layer hides behind fullscreen apps; `overlay` stays on top (switchable at runtime) |

---

## Features

- **Native layer-shell rendering:** `smithay-client-toolkit` + `calloop`, `tiny-skia` CPU compositing into `wl_shm`, damage tracking, frame-callback throttling, and fractional scaling (`wp_fractional_scale_v1` + `wp_viewporter`).
- **VPet content compatibility:** LinePutScript parser and a `PetLoader` / `GraphInfo`-compatible animation indexer. Supports `A_Start -> B_Loop -> C_End` chains, `Single`, `FoodAnimation`, APNG, and GIF. Content is localized to English through `lang/en/*.lps`.
- **Interactions:** head/body touch hit zones from `vup.lps`, drag-to-raise, sleep/wake, auto-walk with screen-edge bounce, and speech bubbles shaped with `cosmic-text`.
- **Simulation:** a deterministic 10 Hz core that follows the `GameSave_VPet` formulas (stat caps, leveling, digestion, condition modes), plus feeding, inventory, and timed buffs.
- **AI Chat (LLM):** background AI worker compatible with OpenAI, Ollama, LM Studio, and DeepSeek endpoints (defaults to `http://localhost:20128/v1` with model `9routerall`), supporting both standard JSON and streaming SSE chunks with speech bubble replies.
- **Crash-safe saves:** MessagePack save files written atomically (`fsync` + rename) with 5 rotating backups and automatic recovery from a corrupt save.
- **Desktop integration:** StatusNotifierItem tray icon (shows in `waybar`), live TOML config reload, and a Unix-socket IPC CLI (`vpet-rs ctl`, socket mode `0600`, `SO_PEERCRED` check).
- **Lightweight:** an 8.2 MiB stripped native ELF that links only `libwayland-client`, `libc`, `libm`, and `libffi`. Measured at about **0.7% of one CPU core** and **about 60 MiB RSS** on a 1920x1080 niri output.

---

## Requirements

- A Wayland compositor that supports `wlr-layer-shell`: niri, Hyprland, sway, river, labwc, Wayfire, and others. GNOME/Mutter is **not** supported.
- Rust **1.89+** (stable) and `libwayland-client`.
- The VPet `0000_core` content pack, which you provide yourself (from your own VPet installation or the [upstream repository](https://github.com/LorisYounger/VPet)).

---

## Build

```bash
git clone https://github.com/kouzen-neo/VPetRS.git
cd VPetRS
cargo build --release --locked -p vpet-app
# Binary: target/release/vpet-rs
```

Run the full quality gate (fmt, clippy, tests, and the no-.NET check):

```bash
./scripts/run-headless-test.sh
```

Arch Linux users can build the AUR-style package from [`packaging/PKGBUILD`](packaging/PKGBUILD).

---

## Setup & Usage

### 1. Initialize Configuration

```bash
vpet-rs init-config                     # writes default ~/.config/vpet-rs/config.toml
```

### 2. Install VPet Core Assets

VPetRS requires the VPet `0000_core` content directory (containing `info.lps` and animation/food data). Choose any of the following options:

- **Automatic (Recommended):**
  ```bash
  vpet-rs setup-assets
  ```
  This automatically checks local Steam VPet installations (`~/.local/share/Steam/...`), links them if found, or downloads `0000_core` via git sparse-checkout (~1.1 GB, fetching only the assets rather than the full git history) into `~/.local/share/vpet-rs/content/0000_core`.

- **From an existing local folder / clone:**
  ```bash
  vpet-rs setup-assets --from /path/to/VPet/VPet-Simulator.Windows/mod/0000_core
  ```

- **Via helper script:**
  ```bash
  ./scripts/download-vpet-assets.sh
  ```

### 3. Run & Control

Start the pet daemon and control it from the command line:

```bash
vpet-rs                                 # start the layer-shell daemon
vpet-rs ctl status                      # stats, mode, layer, output, position (JSON)
vpet-rs ctl toggle-clickthrough         # toggle click-through
vpet-rs ctl layer overlay               # switch between `top` and `overlay`
vpet-rs ctl feed "Fruit Juice"          # feed an item from the food database
vpet-rs ctl chat "Hello Loris! How are you?" # chat with pet via AI LLM
vpet-rs ctl prompt-chat                 # open desktop prompt dialog to chat
vpet-rs ctl works                       # list all work/study/play tasks
vpet-rs ctl work "Copywriting"          # start working
vpet-rs ctl cancel-work                 # cancel active work
vpet-rs ctl say "Hello niri!"           # show a speech bubble
vpet-rs ctl sleep                       # put the pet to sleep (or `wake`)
vpet-rs ctl reload                      # reload config.toml
vpet-rs ctl quit                        # save and exit
vpet-rs import-save /path/to/Save.lps   # import legacy VPet-Simulator save
```

Optional niri autostart, `layer-rule`, and keybind snippets are in [`packaging/niri-snippet.kdl`](packaging/niri-snippet.kdl) and [`docs/niri.md`](docs/niri.md). Nothing is added to your niri config automatically.

---

## Project Layout

| Crate | Responsibility |
|---|---|
| `vpet-core` | Pure deterministic simulation: stats, buffs, inventory, save schema (no I/O) |
| `vpet-content` | LPS parser, animation indexer, food/text/theme/lang databases, mod scanner |
| `vpet-render` | `tiny-skia` compositing, frame decoders, LRU cache, chat bubbles |
| `vpet-wayland` | Layer-shell surface, input region, shm swapchain, pointer, fractional scale |
| `vpet-mods` | Data-mod loader (legacy .NET `.dll` plugins are rejected) |
| `vpet-app` | `vpet-rs` binary: event loop, config, IPC, tray, persistence |

---

## Documentation

- [`CHANGELOG.md`](CHANGELOG.md): release history
- [`docs/niri.md`](docs/niri.md): niri integration
- [`docs/ipc.md`](docs/ipc.md): IPC and CLI reference
- [`docs/assets-format.md`](docs/assets-format.md): LPS and animation folder format
- [`docs/gameplay-formulas.md`](docs/gameplay-formulas.md): simulation formulas
- [`docs/save-format.md`](docs/save-format.md): save format and backup recovery
- [`docs/troubleshooting.md`](docs/troubleshooting.md): troubleshooting

---

## Credits

- **[VPet-Simulator](https://github.com/LorisYounger/VPet)** by [LorisYounger](https://github.com/LorisYounger) and contributors: the original project, its game design, data formats, and artwork. VPetRS is a Rust rewrite inspired by and compatible with it.
- Built with [smithay-client-toolkit](https://github.com/Smithay/client-toolkit), [tiny-skia](https://github.com/linebender/tiny-skia), [cosmic-text](https://github.com/pop-os/cosmic-text), [calloop](https://github.com/Smithay/calloop), and [ksni](https://github.com/iovxw/ksni).

---

## License & Asset Terms

- **VPetRS source code** is dual-licensed under [MIT](LICENSE-MIT) **or** [Apache-2.0](LICENSE-APACHE), at your option.
- **VPet assets** (`0000_core` artwork, animations, and text) are **not** included in this repository. They belong to the upstream [VPet](https://github.com/LorisYounger/VPet) project and are governed by its terms: attribution is required, they may not be sold, and commercial use requires permission from the original authors.
