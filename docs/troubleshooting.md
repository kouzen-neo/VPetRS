# Troubleshooting

## 1. `cannot connect to .../vpet-rs.sock (is vpet-rs running?)`

- Verify `vpet-rs` daemon is running (`pgrep -a vpet-rs`).
- Ensure both the daemon and `vpet-rs ctl` share the same `$XDG_RUNTIME_DIR` (typically `/run/user/1000`).

## 2. `another vpet-rs instance is already running`

- Only one `vpet-rs` daemon may run per `$XDG_RUNTIME_DIR/vpet-rs.lock` (`flock`).
- Stop the existing instance cleanly with `vpet-rs ctl quit` before starting a new one.

## 3. Pet uses procedural mascot instead of VPet sprites

- Point `[app].content_root` in `~/.config/vpet-rs/config.toml` to your VPet `0000_core` directory (containing `info.lps` and `pet/vup/`), then run `vpet-rs ctl reload`.

## 4. Corrupt save file after power loss

- `vpet-rs` writes saves atomically (`temp -> fsync -> rename`) and rotates `.bak.1`..`.bak.5`.
- On startup, any truncated or corrupted `.mpak` file automatically falls back to the newest valid backup and logs a `WARN` entry in `~/.local/share/vpet-rs/logs/vpet-rs.log`.

## 5. Pet hidden behind fullscreen windows

- By default `display.layer = "top"` yields to fullscreen apps.
- To keep the pet above fullscreen windows, run `vpet-rs ctl layer overlay` or set `layer = "overlay"` in `config.toml`.
