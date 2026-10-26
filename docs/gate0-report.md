# Gate 0 Verification Report — Niri `wlr-layer-shell` Spike

**Date:** 2026-10-27
**Tag:** `v0.1.0`
**Target environment:** Arch Linux, `niri` Wayland session, output `eDP-1` (`1920x1080 @ 60.003 Hz`, scale `1`)

---

## Summary

**PASS.** All core Gate 0 invariants succeeded on live `niri` without window rules, compositor patches, or `.NET` dependencies.

| Check | Result | Measured / Observed |
|---|---|---|
| Pure native ELF, zero `.NET` | PASS | `scripts/check-no-dotnet.sh` green; 93-crate lockfile |
| `niri msg layers` registration | PASS | Namespace `"vpet-rs"`, `Keyboard interactivity: none` |
| Full-output geometry & non-exclusive zone | PASS | `configure` size `(1920, 1080)`, `exclusive_zone = -1` |
| Real VPet PNG sprite decode & BGRA swizzle | PASS | `_000_250.png` (`1000x1000`) scaled to `250x250` premultiplied BGRA |
| Alpha-silhouette `wl_region` (`< 512` rects) | PASS | `77` rectangles covering non-transparent pixels (`alpha >= 16`) |
| Click-through toggle (`SIGUSR1` / empty region) | PASS | Switches `input_rects: 77 -> 0` via `Some(&empty_wl_region)` |
| Live layer switch `Top` <-> `Overlay` (`SIGUSR2`) | PASS | Verified in `niri msg layers` (`Top layer` -> `Overlay layer`) |
| Zero keyboard focus theft (`focus-follows-mouse`) | PASS | Focused window (`Alacritty`) unchanged before, during, and after |
| 30 FPS walk + frame-callback throttling | PASS | `64` frames committed in `2.2 s`, partial damage rect clear/blit |
| Zero wasted commits when static (`VPET_WALK=0`) | PASS | `1` initial commit over `1.2 s` (`0.0%` steady-state CPU) |
| Memory footprint (`< 150 MB` budget) | PASS | `25,168 KB` (~`24.6 MB` RSS, including two `1920x1080x4` shm buffers) |
| Output persistence across workspaces | PASS | `wlr-layer-shell` surface is bound to `eDP-1` output, independent of workspace |

## Architectural Notes Confirmed

1. **Wayland display auto-discovery:** When launched from non-graphical subshells where `WAYLAND_DISPLAY` is unset, `vpet-wayland` falls back to scanning `$XDG_RUNTIME_DIR/wayland-*` sockets (`wayland-1` on this session).
2. **Per-slot damage clearing:** Each `ShmSwapchain` slot records its own previous drawn rectangle (`prev_rect`) while reporting `last_committed_rect.union(drawn_rect)` to `wl_surface.damage_buffer`, preventing two-frame-old ghosting under double buffering without clearing the full `1920x1080` canvas.
3. **Overview behavior:** On `niri`, `Top` and `Overlay` layer surfaces remain visible above workspace switching; optional hide-on-overview remains available via IPC (`vpet-rs ctl hide`) in Phase 6.
