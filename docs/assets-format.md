# VPet Asset & LPS Format Reference

This document specifies how `vpet-content` and `vpet-mods` read upstream VPet content packs (such as `mod/0000_core`) without executing any `.NET` code.

---

## 1. LinePutScript (`.lps`) Grammar

Each non-empty line not prefixed by `///` consists of `:|`-delimited segments:

```text
head_name#head_info:|sub1#val1:|sub2#val2:|///optional trailing comment
```

- **UTF-8 BOM & line endings:** Leading `\u{FEFF}` BOM and `\r\n` / `\n` line endings are stripped transparently.
- **Escapes:**
  - `/|` -> literal `|`
  - `/#` -> literal `#`
  - `/:` -> literal `:`
  - `\n`, `\r` -> newline
  - `\t` -> tab, `\\` -> `\`
- **Order & duplicates:** `LpsLine.subs` preserves exact insertion order and duplicate sub keys.

---

## 2. Coordinate Space (`pet/vup.lps`)

Raw VPet sprite PNGs in `0000_core/pet/vup` are `1000x1000` RGBA, while hit rectangles and anchor points in `vup.lps` are authored in **`500x500` logical pet space**:

- `touchhead`: `px=159, py=16, sw=189, sh=178`
- `touchbody`: `px=166, py=206, sw=163, sh=136`
- `pinch`: `px=149, py=128, sw=56, sh=59`
- `raisepoint`: `(290, 128)` for `happy`/`nomal`/`poorcondition`, `(225, 115)` for `ill`

When rendering the pet at display size `S` logical pixels (for example `S = 250`), hit coordinates scale by `S / 500.0`.

---

## 3. Animation Folder Resolution (`AnimationIndex`)

`AnimationIndex::scan_pet_dir` implements `PetLoader.LoadGraph` + `GraphInfo`:

1. **`info.lps` directory:** Directives (`pnganimation`, `apnganimation`, `picture`, `foodanimation`) are resolved relative to the directory (normalizing `\` to `/`), and recursion stops for that subtree.
2. **Leaf directory (no subdirectories):**
   - Multiple `.png` files -> `AnimationSource::PngSequence` sorted by natural order (`_000_250.png` before `_010_125.png`), with frame duration parsed from trailing `_<ms>.png` (fallback `125 ms`, minimum `16 ms`).
   - Single `.png` -> `AnimationSource::SingleImage`.
   - `.apng` / `.gif` -> `AnimationSource::AnimatedImage`.
3. **Metadata extraction (`GraphInfo` parity):**
   - Relative path tokens split on `/`, `\`, and `_` (lowercased).
   - `ModeType`: `info["mode"]` or token `happy` | `nomal` | `poorcondition` | `ill` (default `Nomal`).
   - `GraphType`: `info["graph"]` or first matching token subsequence across the 23 `GraphType` variants (`Raised_Dynamic` -> `["raised", "dynamic"]`, `Touch_Head` -> `["touch", "head"]`, `Switch_Up` -> `["switch", "up"]`, etc.).
   - `AnimatType`: `info["animat"]` or token `a`/`start` (`AStart`), `b`/`loop` (`BLoop`), `c`/`end` (`CEnd`), `single` (`Single`).
   - `Name`: `info.info`, else last remaining non-numeric / non-`~` token, else `graph_type.canonical_name()`.

---

## 4. Mod Loading & Override Priority

1. `0000_core` is always loaded first.
2. Additional mod directories are loaded in natural lexicographical order.
3. **Overrides:**
   - `food/*.lps`: later mod with identical food `name` (case-insensitive) overrides earlier item.
   - `theme/*.lps`: later mod with identical theme `id` overrides earlier theme.
   - `lang/<locale>/*.lps`: later translation key overrides earlier key.
   - `text/*.lps` and `pet/<id>/`: appended to the candidate pool.
   - Legacy `.dll` plugins are never loaded and emit a warning in `ContentDatabase.warnings`.
