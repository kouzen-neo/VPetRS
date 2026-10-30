# VPetRust IPC & CLI Reference

`vpet-rs` exposes a local Unix domain socket at `$XDG_RUNTIME_DIR/vpet-rs.sock` (override via `VPET_SOCKET`) for keybinds, scripts, and status bars.

## Security Model

- **Socket mode:** `0600` (owner read/write only), bound strictly after acquiring `$XDG_RUNTIME_DIR/vpet-rs.lock` (`flock`).
- **Peer credential verification:** Every connection is checked via `getsockopt(SO_PEERCRED)` so only processes matching `geteuid()` are accepted.
- **Input bounds:** Newline-delimited UTF-8 JSON capped at `4096` bytes per request; no arbitrary filesystem paths are accepted over IPC.

## CLI Subcommands

```bash
vpet-rs                                 # Start layer-shell daemon
vpet-rs init-config [--force]           # Write default ~/.config/vpet-rs/config.toml
vpet-rs setup-assets [--from PATH]      # Link or download the 0000_core content pack
vpet-rs import-save PATH [--profile P]  # Import a legacy VPet Save.lps
```

## CLI Control Commands

```bash
vpet-rs ctl status                     # Print pet stats, mode, output, layer, and position
vpet-rs ctl toggle-clickthrough        # Toggle empty wl_region click-through
vpet-rs ctl toggle-selfcare            # Toggle self-care auto-feeding
vpet-rs ctl toggle-autowork            # Toggle automatic work selection
vpet-rs ctl show                       # Show pet surface
vpet-rs ctl hide                       # Hide pet surface and clear input region
vpet-rs ctl layer overlay              # Switch between `top` and `overlay`
vpet-rs ctl feed "Fruit Juice"         # Feed specific item (or `snack` for default)
vpet-rs ctl foods [category]           # List all 151 foods, optionally filtered
vpet-rs ctl buy "Hamburger" 3          # Buy items into the backpack (count 1..=99)
vpet-rs ctl bag                        # List backpack contents (alias: inventory)
vpet-rs ctl money 1000000              # Set wallet balance (cheat / testing, alias: set-money)
vpet-rs ctl dance [loops]              # Dance to music (alias: music, default 3 loops)
vpet-rs ctl side-hide                  # Hide behind the nearest screen edge
vpet-rs ctl side-hide --left           # Force the left edge
vpet-rs ctl side-hide --right          # Force the right edge
vpet-rs ctl climb [left|right|top]     # Climb a wall or the ceiling (default: auto)
vpet-rs ctl bday                       # Play the Happy Birthday animation
vpet-rs ctl works                      # List all work, study, and play tasks
vpet-rs ctl work "Copywriting"         # Start task (or `vpet-rs ctl work` for first available)
vpet-rs ctl cancel-work                # Cancel active task
vpet-rs ctl sleep                      # Put pet to sleep
vpet-rs ctl wake                       # Wake pet up
vpet-rs ctl say "Hello niri!"          # Show a speech bubble above the pet
vpet-rs ctl chat "Hello Loris!"        # Chat with the pet through the configured LLM
vpet-rs ctl prompt-chat                # Open a desktop input prompt (fuzzel/zenity/kdialog)
vpet-rs ctl reload                     # Reload config.toml without restarting
vpet-rs ctl quit                       # Save state and stop daemon
```

## Wire Protocol (Newline-Delimited JSON)

Requests:

```json
{"cmd":"toggle-clickthrough"}
{"cmd":"toggle-self-care"}
{"cmd":"toggle-auto-work"}
{"cmd":"show"}
{"cmd":"hide"}
{"cmd":"layer","mode":"overlay"}
{"cmd":"feed","item":"Fruit Juice"}
{"cmd":"list-foods","category":"Drink"}
{"cmd":"buy","item":"Hamburger","count":3}
{"cmd":"set-money","amount":1000000}
{"cmd":"list-inventory"}
{"cmd":"dance","loops":3}
{"cmd":"side-hide","left":true}
{"cmd":"climb","side":"top"}
{"cmd":"bday"}
{"cmd":"list-works"}
{"cmd":"work","name":"Copywriting"}
{"cmd":"cancel-work"}
{"cmd":"chat","prompt":"Hello Loris!"}
{"cmd":"prompt-input"}
{"cmd":"sleep"}
{"cmd":"wake"}
{"cmd":"say","text":"Hello!"}
{"cmd":"reload"}
{"cmd":"status"}
{"cmd":"quit"}
```

Responses:

```json
{"ok":true,"data":{"click_through":true}}
{"ok":false,"error":"unknown food item `xyz`"}
```
