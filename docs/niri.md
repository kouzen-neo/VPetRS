# Running VPetRust on Niri

`vpet-rs` renders on a `zwlr_layer_surface_v1` surface (namespace `"vpet-rs"`), so **no `window-rule` is required** in niri.

---

## 1. Layer Behavior on Niri

| Setting (`display.layer`) | Workspace switch | Fullscreen window | Keyboard focus (`focus-follows-mouse`) |
|---|---|---|---|
| `"top"` (default) | Stays visible on the output | Automatically hidden behind fullscreen windows | Never stolen (`keyboard_interactivity = none`) |
| `"overlay"` | Stays visible on the output | Remains visible above fullscreen windows | Never stolen (`keyboard_interactivity = none`) |

Switch live at any time:

```bash
vpet-rs ctl layer overlay
vpet-rs ctl layer top
```

---

## 2. Click-Through & Input Region

- Normal mode: `wl_surface.set_input_region` contains `< 512` horizontal alpha spans matching the pet's visible pixels. Clicking transparent space passes straight through to underlying tiled windows.
- Click-through mode: `wl_surface.set_input_region` is set to an explicit **empty** `wl_region`, making the entire pet transparent to pointer input.

Toggle click-through:

```bash
vpet-rs ctl toggle-clickthrough
```

---

## 3. Optional `config.kdl` Snippets

See [`packaging/niri-snippet.kdl`](../packaging/niri-snippet.kdl) for optional `spawn-at-startup`, `layer-rule` (`block-out-from "screencast"`), and keybind examples.

### Overview Handling

In niri, `top` and `overlay` layer surfaces remain visible during Overview. If you prefer hiding the pet while Overview is open, call `vpet-rs ctl hide` / `vpet-rs ctl show` from a keybind or an external `niri msg --json event-stream` listener.
