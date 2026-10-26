# VPetRust (`vpet-rs`)

Niri-first Wayland desktop pet written in **100% Rust** (`wlr-layer-shell` + `tiny-skia` + `cosmic-text`), reusing VPet content data (`.lps` files and sprite folders) with **zero `.NET` / CoreCLR / Mono / Avalonia dependencies**.

---

## Highlights

- **Native `wlr-layer-shell` (`zwlr_layer_surface_v1`):** Owns its coordinates, alpha-silhouette `wl_region` (`< 512` spans), and explicit empty-region click-through — zero window rules needed on `niri`, zero keyboard focus theft (`keyboard_interactivity = none`), persistent across workspaces, and runtime switchable between `top` and `overlay` layers.
- **Ultra-lightweight pure native ELF (`8.2 MiB` stripped):** Links solely against `libwayland-client`, `libc`, `libm`, and `libffi`. Measured on `1920x1080` `niri`: **~0.67% single-core CPU** and **~60.4 MiB RSS** with 621 indexed animations and preloaded sprite clips.
- **VPet Content Compatibility:** Parses `LinePutScript` (`.lps`) files (`pet/vup.lps`, `food/*.lps`, `text/*.lps`, `theme/*.lps`, `lang/*/*.lps`), indexes `PetLoader`/`GraphInfo` animation trees (`A_Start -> B_Loop -> C_End`, `Single`, `FoodAnimation`, APNG, GIF), and enforces `0000_core`-first mod override priority while rejecting legacy `.NET` `.dll` plugins.
- **Crash-Safe Simulation & Control:** Deterministic `10 Hz` `vpet-core` simulation, `rmp-serde` MessagePack `v1` atomic saves with 5-slot backup recovery, `ksni` StatusNotifierItem tray for `waybar`, TOML hot-reload, and `0600` `SO_PEERCRED` Unix socket IPC (`vpet-rs ctl`).

---

## Build & Verify

```bash
cargo build --release --locked -p vpet-app
./scripts/run-headless-test.sh
```

Binary output: `target/release/vpet-rs`

---

## Usage

```bash
vpet-rs init-config                    # Generate ~/.config/vpet-rs/config.toml
vpet-rs                                # Start layer-shell pet daemon
vpet-rs ctl status                     # Inspect stats, mode, layer, output, position
vpet-rs ctl toggle-clickthrough        # Toggle mouse click-through
vpet-rs ctl layer overlay              # Switch between `top` and `overlay`
vpet-rs ctl feed "Fruit Juice"         # Feed an item from 0000_core/food
vpet-rs ctl say "Hello niri!"          # Render a speech bubble
vpet-rs ctl sleep                      # Put pet to sleep (or `wake`)
vpet-rs ctl reload                     # Hot-reload config.toml
vpet-rs ctl quit                       # Save world and exit cleanly
```

---

## Documentation

- Architecture & ADR specification: [`IMPLEMENTATION.md`](IMPLEMENTATION.md)
- Phase checklist & commit map: [`TODO.md`](TODO.md)
- Changelog: [`CHANGELOG.md`](CHANGELOG.md)
- Gate 0 live niri report: [`docs/gate0-report.md`](docs/gate0-report.md)
- Asset & LPS format reference: [`docs/assets-format.md`](docs/assets-format.md)
- Gameplay formulas: [`docs/gameplay-formulas.md`](docs/gameplay-formulas.md)
- Save format & backup recovery: [`docs/save-format.md`](docs/save-format.md)
- IPC & CLI reference: [`docs/ipc.md`](docs/ipc.md)
- Niri integration & snippets: [`docs/niri.md`](docs/niri.md) & [`packaging/niri-snippet.kdl`](packaging/niri-snippet.kdl)
- Troubleshooting: [`docs/troubleshooting.md`](docs/troubleshooting.md)

---

## License & Asset Terms

- **VPetRust source code:** Dual-licensed under [`MIT`](LICENSE-MIT) OR [`Apache-2.0`](LICENSE-APACHE).
- **VPet assets (`0000_core`):** User-supplied, **not** bundled in this repository or AUR package. Art, animations, and content from [LorisYounger/VPet](https://github.com/LorisYounger/VPet) remain governed by upstream VUP-Simulator terms (attribution required, may not be sold, commercial use requires permission). VPetRust is an independent pure-Rust reimplementation, not an official VPet product.
