# VPetRust IPC & CLI Reference

`vpet-rs` exposes a local Unix domain socket at `$XDG_RUNTIME_DIR/vpet-rs.sock` (override via `VPET_SOCKET`) for keybinds, scripts, and status bars.

## Security Model

- **Socket mode:** `0600` (owner read/write only), bound strictly after acquiring `$XDG_RUNTIME_DIR/vpet-rs.lock` (`flock`).
- **Peer credential verification:** Every connection is checked via `getsockopt(SO_PEERCRED)` so only processes matching `geteuid()` are accepted.
- **Input bounds:** Newline-delimited UTF-8 JSON capped at `4096` bytes per request; no arbitrary filesystem paths are accepted over IPC.

## CLI Subcommands

```bash
vpet-rs                                # Start layer-shell daemon
vpet-rs init-config                    # Write default ~/.config/vpet-rs/config.toml
vpet-rs ctl status                     # Print pet stats, mode, output, layer, and position
vpet-rs ctl toggle-clickthrough        # Toggle empty wl_region click-through
vpet-rs ctl show                       # Show pet surface
vpet-rs ctl hide                       # Hide pet surface and clear input region
vpet-rs ctl layer overlay              # Switch between `top` and `overlay`
vpet-rs ctl feed "Fruit Juice"         # Feed specific item (or `snack` for default)
vpet-rs ctl sleep                      # Put pet to sleep
vpet-rs ctl wake                       # Wake pet up
vpet-rs ctl say "Hello niri!"          # Show a speech bubble above the pet
vpet-rs ctl reload                     # Reload config.toml without restarting
vpet-rs ctl quit                       # Save state and stop daemon
```

## Wire Protocol (Newline-Delimited JSON)

Requests:

```json
{"cmd":"toggle-clickthrough"}
{"cmd":"show"}
{"cmd":"hide"}
{"cmd":"layer","mode":"overlay"}
{"cmd":"feed","item":"Fruit Juice"}
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
