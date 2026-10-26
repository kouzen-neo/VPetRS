# VPetRust — Rencana Implementasi

**Status:** draft  
**Target utama:** niri (Wayland, layer-shell)  
**Prinsip:** 100% Rust. Tidak ada .NET, CoreCLR, Mono, Avalonia, WPF, atau plugin DLL .NET.  
**Tanggal:** 2026-10-26

---

## 1. Ringkasan

VPetRust adalah aplikasi desktop pet baru yang ditulis ulang sepenuhnya di Rust. VPet
(C#/WPF) hanya dipakai sebagai:

- sumber aset animasi dan data permainan,
- referensi perilaku game,
- referensi format konten (LPS).

Tidak ada kode C# yang dijalankan, dipanggil, atau di-host. Tidak ada runtime .NET di
mesin pengguna.

Berbeda dari VPet yang memakai window biasa (`xdg_toplevel`), VPetRust memakai
**`wlr-layer-shell`**. Keputusan ini menyelesaikan masalah utama desktop pet di
compositor tiling seperti niri:

- aplikasi tidak perlu memindahkan window global;
- click-through bisa benar-benar bekerja lewat input region;
- tidak merebut fokus keyboard;
- pet muncul di semua workspace pada output;
- perilaku fullscreen bisa dipilih (`top` vs `overlay`).

---

## 2. Keputusan arsitektur (ADR)

### ADR-001 — Rust murni, tanpa .NET

**Keputusan:** seluruh aplikasi, termasuk simulasi, renderer, IPC, tray, dan mod system,
ditulis di Rust.

**Konsekuensi:**

- Tidak ada kompatibilitas plugin `MainPlugin` (.NET).
- Tidak ada reuse kode `VPet-Simulator.Core` / `VPet-Simulator.Windows.Interface`.
- Aset dan data tetap bisa dipakai.
- Distribusi berupa satu binary ELF; tidak perlu runtime tambahan selain system library.

### ADR-002 — Layer-shell, bukan `xdg_toplevel`

**Keputusan:** permukaan utama pet adalah `zwlr_layer_surface_v1`.

**Alasan:** niri mengabaikan posisi window dari aplikasi. Layer-shell memberi kontrol
posisi, ukuran, input region, dan layer kepada aplikasi.

**Konsekuensi:**

- Tidak memakai `winit`, `iced`, `egui`, `slint`, atau toolkit berbasis toplevel.
- Semua window management ditangani sendiri lewat `smithay-client-toolkit`.

### ADR-003 — Surface penuh per output, posisi internal

**Keputusan:** satu layer surface seukuran output; pet digambar pada koordinat internal
`(x, y)` di dalam surface.

**Alasan:** pergerakan menjadi mulus, tidak bergantung pada update margin/posisi
compositor, dan tidak memerlukan IPC ke niri.

**Konsekuensi:**

- Perlu damage tracking agar tidak merender seluruh layar setiap frame.
- Perlu manajemen buffer `wl_shm` yang benar.
- Alternatif surface kecil + margin disimpan sebagai fallback performa.

### ADR-004 — Reuse aset dan LPS, bukan kode

**Keputusan:** parser LPS dan index animasi diimplementasikan di Rust; folder
`mod/0000_core` milik VPet bisa dibaca langsung.

**Konsekuensi:**

- Mod data (makanan, teks, tema, animasi) tetap bisa dipakai.
- Mod plugin .NET tidak bisa.
- Format save baru; import save VPet bersifat opsional.

### ADR-005 — CPU rendering dulu

**Keputusan:** `tiny-skia` + `wl_shm` untuk MVP.

**Alasan:** target awal 1920×1080 scale 1; animasi VPet berbasis frame dan tidak butuh
efek GPU kompleks.

**Konsekuensi:**

- Di 4K atau 60fps penuh, perlu optimasi damage, surface kecil, atau pindah ke GPU.
- Benchmark menjadi bagian dari Gate 0.

### ADR-006 — Wayland-only untuk MVP

**Keputusan:** MVP hanya mendukung Wayland + layer-shell.

**Alasan:** target utama niri. Menambahkan X11 sejak awal menambah kompleksitas dan
mengaburkan fokus.

**Konsekuensi:**

- Tidak bisa dipakai di sesi Xorg.
- Fallback X11 dapat dipertimbangkan setelah MVP stabil.

---

## 3. Goals dan non-goals

### Goals MVP

- Berjalan sebagai layer surface di niri tanpa window rule.
- Animasi idle, sleep, touch, raised, walk.
- Interaksi dasar: klik kepala/badan, angkat, drag, tidur.
- Stats: strength, food, drink, feeling, health, money, level, exp.
- Makan/minum dan inventory sederhana.
- Save/load dengan backup.
- Config TOML.
- Tray waybar + IPC.
- Click-through toggle.
- Paket AUR dan integrasi autostart niri.
- Tidak ada dependensi .NET sama sekali.

### Non-goals MVP

- Steam, Workshop, achievement, cloud.
- Multiplayer.
- Plugin .NET.
- Gallery, work menu lengkap, schedule kompleks.
- Tema dan lokalisasi penuh.
- Paritas seluruh 384 animasi.
- UI settings penuh.
- Dukungan GNOME/KDE Wayland.
- Dukungan X11.

---

## 4. Constraint dan asumsi

- Pengguna memakai **niri 26.04** di Arch Linux.
- Output utama: **eDP-1, 1920×1080, scale 1**.
- Compositor mendukung `wlr-layer-shell`, `wl_shm`, `xdg_wm_base >= 3`.
- Aset VPet tersedia di `VPet-Simulator.Windows/mod/0000_core` (~1,1 GB).
- Binary dibangun di mesin yang sama; tidak perlu cross-compile.
- Fokus awal satu pet, satu output; arsitektur harus siap multi-output.
- "Tanpa .NET" berarti tidak ada runtime Microsoft. System library C (Wayland,
  xkbcommon, ALSA/PipeWire, fontconfig) tetap boleh.

---

## 5. Arsitektur

### 5.1 Crate graph

```text
                    ┌──────────────┐
                    │   vpet-app   │
                    │ loop, config │
                    │ IPC, tray    │
                    └──────┬───────┘
                           │
          ┌────────────────┼────────────────┐
          │                │                │
   ┌──────▼──────┐  ┌──────▼──────┐  ┌──────▼──────┐
   │ vpet-wayland│  │ vpet-render │  │  vpet-mods  │
   │ layer-shell │  │ tiny-skia   │  │ data/WASM   │
   └──────┬──────┘  └──────┬──────┘  └──────┬──────┘
          │                │                │
          └────────────────┼────────────────┘
                           │
                  ┌────────▼────────┐
                  │   vpet-content  │
                  │ LPS, index, DB  │
                  └────────┬────────┘
                           │
                  ┌────────▼────────┐
                  │    vpet-core    │
                  │ simulasi murni  │
                  └─────────────────┘
```

### 5.2 Aturan dependensi

- `vpet-core` tidak boleh bergantung pada crate lain di workspace.
- `vpet-core` tidak boleh tahu Wayland, file, gambar, atau waktu sistem secara langsung.
- `vpet-content` hanya bergantung pada `vpet-core` dan parser LPS.
- `vpet-render` tidak boleh tahu cara membaca mod.
- `vpet-wayland` tidak boleh tahu gameplay.
- `vpet-app` adalah satu-satunya crate yang menyusun semuanya.
- Tidak ada dependensi ke .NET, Mono, atau interop C#.

### 5.3 Tanggung jawab

| Crate | Tanggung jawab |
|---|---|
| `vpet-core` | State pet, stats, buff, work, schedule, ekonomi, save model |
| `vpet-content` | Parser LPS, index animasi, database makanan/teks/tema, scanning mod |
| `vpet-render` | Scene graph sederhana, compositing, teks, damage rect |
| `vpet-wayland` | Layer surface, input region, output, buffer, pointer, frame callback |
| `vpet-app` | Event loop, config, IPC, tray, orkestrasi |
| `vpet-mods` | Mod data, nanti WASM/Rhai |
| `vpet-steam` | Opsional, terpisah dari MVP |

---

## 6. Struktur direktori

```text
VPetRust/
├── Cargo.toml                 # workspace
├── Cargo.lock
├── rust-toolchain.toml
├── IMPLEMENTATION.md
├── README.md
├── docs/
│   ├── assets-format.md
│   ├── save-format.md
│   ├── ipc.md
│   └── niri.md
├── crates/
│   ├── core/
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── state.rs
│   │   │   ├── stats.rs
│   │   │   ├── buff.rs
│   │   │   ├── item.rs
│   │   │   ├── work.rs
│   │   │   ├── schedule.rs
│   │   │   ├── save.rs
│   │   │   └── rng.rs
│   │   └── tests/
│   ├── content/
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── lps.rs
│   │   │   ├── anim.rs
│   │   │   ├── food.rs
│   │   │   ├── text.rs
│   │   │   ├── theme.rs
│   │   │   ├── lang.rs
│   │   │   └── scan.rs
│   │   └── tests/
│   ├── render/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── canvas.rs
│   │       ├── text.rs
│   │       ├── food.rs
│   │       └── damage.rs
│   ├── wayland/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── surface.rs
│   │       ├── input.rs
│   │       ├── region.rs
│   │       ├── output.rs
│   │       └── shm.rs
│   ├── app/
│   │   └── src/
│   │       ├── main.rs
│   │       ├── config.rs
│   │       ├── loop.rs
│   │       ├── anim.rs
│   │       ├── ipc.rs
│   │       ├── tray.rs
│   │       └── paths.rs
│   ├── mods/
│   └── steam/
├── assets/                    # ikon, bukan aset VPet
├── packaging/
│   ├── PKGBUILD
│   ├── vpet-rs.desktop
│   └── niri-snippet.kdl
└── scripts/
    ├── check-no-dotnet.sh
    └── run-headless-test.sh
```

---

## 7. Stack dan dependency

| Kebutuhan | Crate | Versi indikatif | Catatan |
|---|---|---|---|
| Wayland client | `wayland-client` | 0.31 | |
| Layer-shell | `wayland-protocols-wlr` | 0.3 | feature `client` |
| Toolkit | `smithay-client-toolkit` | 0.20 | layer shell, shm, output |
| Event loop | `calloop` | 0.14 | timer, channel, generic |
| Render 2D | `tiny-skia` | 0.11 | premultiplied RGBA |
| Decode | `image` | 0.25 | PNG, GIF, WebP |
| APNG | `apng` | 0.3 | jika diperlukan |
| Teks | `cosmic-text` | 0.14 | shaping + raster |
| Audio | `rodio` + `cpal` | 0.21 / 0.16 | PipeWire/ALSA |
| Serialize | `serde`, `rmp-serde`, `toml` | 1.x | |
| Tray | `ksni` | 0.2 | StatusNotifierItem |
| CLI | `clap` | 4.x | |
| Logging | `tracing`, `tracing-subscriber` | 0.1 / 0.3 | |
| Error | `thiserror`, `anyhow` | 2.x / 1.x | |
| Parallel decode | `rayon` | 1.10 | |
| Random | `rand` / `fastrand` | | deterministic seed di core |
| WASM (nanti) | `wasmtime` | | sandbox plugin |
| Scripting (alternatif) | `rhai` | | pure Rust |

Versi final dipin di `Cargo.lock`. MSRV mengikuti stable saat implementasi; target awal
Rust 1.85+ (edition 2024).

Dependensi yang **dilarang**:

- `winit`, `iced`, `egui`, `slint`, `tao`
- apa pun yang membutuhkan `dotnet`, `mono`, `CoreCLR`, atau `Avalonia`

---

## 8. Platform layer: Wayland + niri

### 8.1 Urutan startup

1. Buka koneksi Wayland.
2. Bind global: `wl_compositor`, `wl_shm`, `wl_output`, `zwlr_layer_shell_v1`,
   `wl_seat`.
3. Pilih output dari config atau output yang berisi kursor.
4. Buat `wl_surface`.
5. Buat `zwlr_layer_surface_v1`:
   - namespace: `vpet-rs`
   - layer: `top` (default) atau `overlay`
   - anchor: atas + bawah + kiri + kanan
   - size: `0 × 0` (compositor menentukan; gunakan ukuran dari `configure`)
   - exclusive zone: `-1`
   - keyboard interactivity: `none`
   - margin: `0`
6. Commit surface.
7. Terima `configure(width, height)`, buat buffer `wl_shm` sesuai ukuran.
8. Pasang input region, gambar frame pertama, commit.
9. Minta frame callback dan mulai loop simulasi.

### 8.2 Konfigurasi surface

```text
Layer            : Top (default) | Overlay (opsional)
Anchor           : TOP | BOTTOM | LEFT | RIGHT
Exclusive zone   : -1
Keyboard         : None
Size             : mengikuti configure (biasanya ukuran output)
Input region     : area pet (dinamis)
```

- `Top` = pet hilang saat window fullscreen.
- `Overlay` = pet tetap terlihat di atas fullscreen.
- `keyboard_interactivity = none` = tidak pernah merebut fokus keyboard, aman untuk
  `focus-follows-mouse`.
- `exclusive_zone = -1` = tidak memesan ruang, tidak menggeser window lain.

### 8.3 Input region

Aturan penting Wayland:

- `surface.set_input_region(None)` = **seluruh surface** menerima input.
- `surface.set_input_region(Some(region_kosong))` = **tidak ada input** (click-through).
- Region bersifat double-buffered; berlaku setelah `commit`.

Alur:

1. Hitung silhouette pet dari frame saat ini.
2. Buat `wl_region`.
3. Tambahkan rectangle per baris/segmen (mis. tiap 8 px), atau bounding box untuk MVP.
4. `surface.set_input_region(Some(&region))`.
5. Commit.

Saat mode click-through aktif:

1. Buat `wl_region` kosong.
2. Set sebagai input region.
3. Commit.

Saat mode click-through dimatikan:

1. Hitung ulang region dari frame.
2. Set dan commit.

Catatan:

- Region berada dalam koordinat surface-local, bukan buffer pixel.
- Batasi jumlah rectangle (mis. < 512) agar tidak membebani compositor.
- Pointer focus tidak berpindah selama tombol mouse ditekan; drag tetap bekerja walau
  kursor keluar dari region. Ini harus diverifikasi di Gate 0.

### 8.4 Posisi dan pergerakan

- Posisi pet adalah `(x, y)` di dalam surface, bukan posisi window.
- Auto-walk mengubah `x` per tick.
- Drag mengubah `(x, y)` berdasarkan delta pointer.
- Edge behavior:
  - berhenti di tepi,
  - memantul,
  - atau berjalan ke arah berlawanan.
- Posisi disimpan per output di config/save.
- Tidak ada IPC ke niri untuk memindahkan window.

### 8.5 Multi-output

MVP:

- Satu pet pada satu output (config `output = "eDP-1"` atau `"auto"`).
- Jika output hilang, pindah ke output pertama yang tersedia.

Arsitektur:

- `OutputManager` menyimpan daftar `wl_output`.
- Setiap output memiliki surface sendiri.
- Pet dapat dipindahkan antar-output pada iterasi berikutnya tanpa mengubah core.

### 8.6 Workspace, fullscreen, overview

- Layer surface terikat ke output, bukan workspace. Perilaku yang diharapkan: pet tetap
  terlihat saat berpindah workspace. **Verifikasi di Gate 0.**
- `Top`: di bawah window fullscreen.
- `Overlay`: di atas window fullscreen. Jadikan opsi config, default `top`.
- Overview niri: layer `top`/`overlay` tetap terlihat di atas overview. Jika mengganggu,
  subscribe `niri msg --json event-stream` dan hide surface saat overview terbuka.
- Screencast: gunakan `layer-rule` niri `block-out-from "screencast"` bila perlu.

### 8.7 Skala

- MVP: integer scale dari `wl_output.scale`; target utama scale 1.
- Struktur harus siap fractional scale:
  - bind `wp_fractional_scale_v1` dan `wp_viewporter`;
  - render pada skala fisik;
  - konversi koordinat config (logical) ke pixel.
- Input region selalu dalam koordinat surface-local.

### 8.8 Buffer dan frame

- Format: `wl_shm::Format::Argb8888` (little-endian BGRA, premultiplied).
- `tiny-skia` menghasilkan RGBA premultiplied; lakukan swizzle RGBA → BGRA saat copy
  ke buffer.
- Gunakan `SlotPool`/double buffering dari SCTK.
- Tunggu `wl_buffer.release` sebelum memakai ulang buffer.
- Gunakan `surface.damage_buffer(x, y, w, h)` untuk area lama dan baru.
- Minta `frame` callback; render hanya jika ada perubahan.
- Jika tidak ada animasi dan tidak ada input, jangan commit frame baru.

### 8.9 Kursor

- Gunakan tema kursor sistem (`wl_cursor` theme) atau sembunyikan kursor saat di atas
  pet.
- Saat drag, tampilkan kursor `grabbing` jika tema mendukung.
- Jangan melakukan keyboard grab.

### 8.10 Drag

1. `pointer.button` kiri ditekan di area pet.
2. Simpan offset pointer terhadap posisi pet.
3. `pointer.motion` → perbarui `(x, y)`.
4. `pointer.button` dilepas → selesai; simpan posisi.
5. Jika pointer keluar surface saat drag, tetap tangkap sampai release (perilaku
   implicit grab).

---

## 9. Renderer

### 9.1 Pipeline

```text
Animation frame (RGBA)
        │
        ▼
tiny-skia Pixmap (premultiplied RGBA)
        │  composite: pet + food layers + chat bubble
        ▼
Swizzle RGBA → BGRA
        │
        ▼
wl_shm buffer
        │
        ▼
damage + commit
```

### 9.2 Animasi

Model data:

```rust
struct AnimationKey {
    pet: String,
    graph: String,
    animat: AnimatType,   // Single | AStart | BLoop | CEnd
    mode: ModeType,       // Nomal | Happy | PoorCondition | Ill
    variant: u16,
}

struct Animation {
    key: AnimationKey,
    frames: Vec<Frame>,
    looped: bool,
    frame_ms: Vec<u32>,
}

struct Frame {
    image: Arc<tiny_skia::Pixmap>,
    duration_ms: u32,
    anchor: (i32, i32),
}
```

- Decode PNG folder: urutkan nama file secara natural.
- Dukung `pnganimation`, `apnganimation`, `picture`, `foodanimation` seperti VPet.
- Varian dengan nama sama dipilih acak dengan seed deterministik.
- Cache LRU berbasis byte, mis. maksimum 256 MB.
- Preload: idle/breath, sleep, touch head/body.
- Decode di thread pool (`rayon` atau thread khusus), kirim hasil lewat channel ke loop
  utama.
- Jangan decode di frame callback.

### 9.3 Teks

- `cosmic-text` untuk shaping.
- Chat bubble: rounded rect + border + teks, digambar dengan tiny-skia.
- Dukungan emoji mengikuti font sistem; sediakan fallback.
- Untuk MVP, bubble hanya di dalam surface pet (bukan window terpisah).

### 9.4 Damage tracking

- Simpan rect area pet frame sebelumnya dan frame baru.
- Damage = union kedua rect (atau dua rect terpisah).
- Untuk chat bubble, tambahkan rect bubble.
- Hindari clear seluruh layar setiap frame.
- Jika memakai surface penuh, buffer tetap di-upload penuh; optimasi berikutnya adalah
  surface kecil atau GPU.

### 9.5 Budget performa

Target MVP di 1920×1080 scale 1:

- idle 10–20 fps, animasi aktif 30 fps;
- CPU render < 10% satu core;
- memori < 300 MB (termasuk cache);
- startup < 1 detik setelah aset ter-cache;
- tidak ada frame drop saat drag.

Jika budget terlampaui:

1. kurangi ukuran cache,
2. render hanya area pet (surface kecil),
3. pindah ke GPU (`wgpu`/EGL).

---

## 10. Konten dan data

### 10.1 Parser LPS

Format garis VPet secara umum:

```text
tag:|key#value:|key#value:|
tag#value:|key#value:|
```

Persyaratan:

- dukung BOM dan CRLF;
- tangani `|`, `#`, `:` dan escape;
- pertahankan urutan dan duplikat;
- API:
  ```rust
  struct LpsDocument { lines: Vec<LpsLine> }
  struct LpsLine { name: String, info: String, subs: Vec<LpsSub> }
  struct LpsSub { name: String, info: String }
  ```
- Golden test memakai file asli: `info.lps`, `pet/vup.lps`, `food/food.lps`,
  `text/ClickText.lps`, `theme/default.lps`, `lang/en/Base2608.lps`.

### 10.2 Index animasi

Implementasi ulang logika `GraphInfo`/`PetLoader`:

- `GraphType`: `Common`, `Raised_Dynamic`, `Raised_Static`, `Move`, `Default`,
  `Touch_Head`, `Touch_Body`, `Idel`, `Sleep`, `Say`, `StateONE`, `StateTWO`,
  `StartUP`, `Shutdown`, `Work`, `Switch_Up`, `Switch_Down`, `Switch_Thirsty`,
  `Switch_Hunger`, `SideHide_*`.
- `AnimatType`: `Single`, `A_Start`, `B_Loop`, `C_End`.
- `ModeType`: `Nomal`, `Happy`, `PoorCondition`, `Ill`.
- Resolusi path dan prioritas harus diuji terhadap folder `mod/0000_core` asli.
- Dokumentasikan hasilnya di `docs/assets-format.md`.

### 10.3 Database konten

- Makanan/minuman: `food/*.lps`.
- Teks: `text/*.lps` (`ClickText`, `SelectText`, `LowText`, `Buff`, `SchedulePackage`).
- Tema: `theme/*.lps` + font.
- Lokalisasi: `lang/<locale>/*.lps`.
- Gambar item: `image/`.
- Foto/galeri: `photo/` (non-MVP).

Urutan muat:

1. `0000_core`
2. mod lain diurutkan berdasarkan nama folder
3. definisi dengan kunci sama: mod menimpa core (kecuali tipe yang bersifat append)

### 10.4 Config

Lokasi: `$XDG_CONFIG_HOME/vpet-rs/config.toml` (default `~/.config/vpet-rs/config.toml`).

```toml
[app]
content_root = "~/.local/share/vpet-rs/content/0000_core"
log_level = "info"

[display]
output = "auto"          # "auto" | "eDP-1" | ...
layer = "top"            # "top" | "overlay"
scale = 1.0
click_through = false
position = [40, 40]      # logical px, dari kiri atas output

[pet]
name = "Loris"
start_mode = "nomal"

[simulation]
tick_hz = 10
autosave_secs = 300

[audio]
enabled = true
volume = 0.8
```

### 10.5 Save

Lokasi: `$XDG_DATA_HOME/vpet-rs/saves/<profile>.mpak`  
Backup: `saves/<profile>.bak.1` … `.bak.5`

Skema:

```rust
#[derive(Serialize, Deserialize)]
struct SaveFile {
    version: u16,          // mulai dari 1
    saved_at_unix: i64,
    profile: String,
    pet: PetState,
    inventory: Vec<InventoryItem>,
    buffs: Vec<BuffState>,
    statistics: Statistics,
}
```

Aturan:

- tulis ke file temporary di direktori yang sama;
- `fsync` file dan direktori;
- rename atomik;
- rotasi backup;
- jika parse gagal, coba backup terbaru;
- migrasi versi eksplisit (`migrate_v1_to_v2`, dst.);
- import save VPet bersifat opsional dan read-only.

---

## 11. Simulasi

### 11.1 Model tick

- Tick tetap 10 Hz untuk simulasi.
- Render terpisah, mengikuti frame callback dan perubahan state.
- Waktu disuntikkan (`now: Instant`/`SystemTime`) agar bisa dites.
- Tidak ada thread untuk simulasi; semua di loop utama.
- RNG deterministik dengan seed dari save, agar perilaku bisa direproduksi.

### 11.2 State

```rust
struct PetState {
    name: String,
    level: u32,
    level_max: u32,
    exp: f64,
    money: f64,
    strength: f64,
    strength_food: f64,
    strength_drink: f64,
    feeling: f64,
    health: f64,
    likability: f64,
    mode: ModeType,
}

enum ModeType { Nomal, Happy, PoorCondition, Ill }

struct InventoryItem { id: String, count: u32 }
```

### 11.3 Stats

Referensi utama: `GameSave_VPet` di VPet.

- Batas maksimum naik mengikuti level.
- `LevelUpNeed = 200 * level - 100` (verifikasi terhadap sumber).
- Penurunan/kenaikan per detik ditentukan oleh mode dan aktivitas.
- Buff mengubah multiplier biaya/regenerasi.
- Clamp nilai ke rentang valid.
- Semua formula ditulis di `docs/gameplay-formulas.md` dan diuji.

### 11.4 Buff

```rust
struct BuffState {
    id: String,
    source_key: String,
    ends_at_unix: i64,
    activity: ActivityMask,
    effects: Vec<Effect>,
}
```

- Efek bertimer, dapat ditumpuk sesuai aturan.
- Cakupan aktivitas: Idle, Sleep, Study, Work, Play, Travel, Other, TakeItem, Schedule.
- Buff kedaluwarsa dihapus pada tick; efek langsung dihitung ulang.

### 11.5 Work/study/play

- Timer durasi, biaya food/drink/feeling, reward uang/exp.
- `LevelLimit`, `FinishBonus`.
- State machine: `Idle → Working → Finish/Cancel → Idle`.
- Schedule (fase 2): daftar tugas harian, auto-start, auto-renew.

### 11.6 State machine animasi

```text
Idle ──touch head──▶ TouchHead ──▶ Idle
Idle ──touch body──▶ TouchBody ──▶ Idle
Idle ──raise───────▶ Raised ─────▶ Idle
Idle ──sleep───────▶ Sleep ──────▶ Idle
Idle ──walk────────▶ Move ───────▶ Idle
Idle ──work────────▶ Work ───────▶ Idle
Any  ──say─────────▶ Say ────────▶ previous
```

- Prioritas: raised > touch > work > say > move > idle > sleep.
- Transisi memakai A_Start → B_Loop → C_End bila tersedia.
- Fallback ke `Single` bila animasi tidak lengkap.
- Mode (nomal/happy/poor/ill) memilih varian animasi.

### 11.7 Referensi perilaku

| C# | Rust |
|---|---|
| `GameCore` | `core::World` |
| `GameSave_VPet` | `core::PetState` + `core::save` |
| `MainLogic` | `core::sim::tick` |
| `MainDisplay` | `app::anim` + `render::scene` |
| `GraphCore` | `content::AnimationIndex` |
| `GraphInfo` | `content::anim::AnimationKey` |
| `PetLoader` | `content::scan` |
| `Buff` | `core::buff` |
| `ScheduleTask` | `core::schedule` |
| `Food`/`Item` | `core::item` |
| `CoreMOD` | `content::scan::ModSet` |
| `MWController` | `wayland::surface` |

---

## 12. UI MVP

- **Chat bubble**: digambar di layer surface.
- **Tray** (waybar): Show/Hide, Click-through, Feed, Open Config, Reload, Quit.
- **Config**: TOML, reload tanpa restart.
- **CLI/IPC**: untuk keybind niri.
- **Tidak ada** window settings penuh di MVP.
- Window settings (jika nanti) memakai `xdg_toplevel` terpisah; bukan layer-shell.

---

## 13. IPC dan CLI

Socket: `$XDG_RUNTIME_DIR/vpet-rs.sock`  
Mode: `0600`  
Protokol: newline-delimited JSON.

Request:

```json
{"cmd":"toggle-clickthrough"}
{"cmd":"show"}
{"cmd":"hide"}
{"cmd":"feed","item":"apple"}
{"cmd":"reload"}
{"cmd":"status"}
{"cmd":"quit"}
```

Response:

```json
{"ok":true,"data":{"click_through":true}}
{"ok":false,"error":"unknown item"}
```

CLI:

```bash
vpet-rs                      # start
vpet-rs ctl show
vpet-rs ctl hide
vpet-rs ctl toggle-clickthrough
vpet-rs ctl feed apple
vpet-rs ctl status
vpet-rs ctl quit
```

Keamanan:

- socket hanya untuk user;
- verifikasi `SO_PEERCRED` bila memungkinkan;
- jangan menerima path arbitrer dari IPC.

---

## 14. Integrasi niri

### 14.1 Autostart

```kdl
spawn-at-startup "vpet-rs"
```

### 14.2 Layer rule

Layer-shell bukan window, jadi **tidak perlu `window-rule`**. Gunakan `layer-rule`
bila perlu:

```kdl
layer-rule {
    match namespace="vpet-rs"
    // block-out-from "screencast"
}
```

### 14.3 Keybind

```kdl
binds {
    Mod+Shift+P { spawn "vpet-rs" "ctl" "toggle-clickthrough"; }
    Mod+Shift+O { spawn "vpet-rs" "ctl" "hide"; }
}
```

### 14.4 Catatan

- Pet muncul di semua workspace karena layer surface terikat ke output. Verifikasi di
  Gate 0.
- Fullscreen: `top` hilang, `overlay` tetap.
- Overview: hide via event stream bila perlu.
- `focus-follows-mouse` aman karena surface tidak meminta keyboard.

---

## 15. Packaging dan build

### 15.1 Cargo

```toml
[workspace]
resolver = "2"
members = [
    "crates/core",
    "crates/content",
    "crates/render",
    "crates/wayland",
    "crates/app",
    "crates/mods",
]

[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = true
panic = "abort"
```

### 15.2 Build

```bash
cargo build --release --locked
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

### 15.3 Verifikasi tanpa .NET

```bash
file target/release/vpet-rs
ldd target/release/vpet-rs | grep -Ei 'coreclr|mono|dotnet' && exit 1
cargo tree | grep -Ei 'dotnet|mono|avalonia' && exit 1
```

`scripts/check-no-dotnet.sh` menjalankan ketiga pemeriksaan.

### 15.4 AUR (draft)

```bash
pkgname=vpet-rs-git
pkgver=r1.0000000
pkgrel=1
pkgdesc="Niri-first desktop pet written in Rust"
arch=('x86_64')
url="https://example.invalid/vpet-rs"
license=('MIT OR Apache-2.0')
depends=('wayland' 'libxkbcommon' 'alsa-lib' 'fontconfig')
makedepends=('cargo' 'git')
source=("$pkgname::git+file:///home/kouzen/Documents/VPetRust")
sha256sums=('SKIP')

build() {
    cd "$srcdir/$pkgname"
    cargo build --release --locked
}

package() {
    install -Dm755 "target/release/vpet-rs" "$pkgdir/usr/bin/vpet-rs"
    install -Dm644 packaging/vpet-rs.desktop "$pkgdir/usr/share/applications/vpet-rs.desktop"
}
```

Catatan:

- Aset VPet **tidak** dibundel. Pengguna mengarahkan `content_root` ke aset yang sudah
  ada, atau memasang paket konten terpisah.
- Untuk distribusi publik, tinjau lisensi aset VPet terlebih dahulu.

---

## 16. Testing

### 16.1 Unit

- `core`: formula stats, level up, buff expiry, work reward, save roundtrip,
  determinisme RNG.
- `content`: parser LPS, index animasi, prioritas mod, lokalisasi.
- `wayland`: perhitungan region dari alpha mask, konversi koordinat, damage rect.

### 16.2 Golden

- Bandingkan index animasi dengan folder asli `mod/0000_core`.
- Snapshot frame: render ke PNG, bandingkan dengan toleransi kecil.
- Golden test untuk save v1 dan migrasi.

### 16.3 Wayland integration

- Jalankan compositor wlroots headless (mis. `sway --headless`) di CI.
- Pastikan layer surface mendapat `configure`, buffer terbentuk, input region valid.
- niri-specific (workspace persistence, overview, fullscreen) diuji manual.

### 16.4 Manual checklist niri (Gate 0)

- [ ] Binary start tanpa .NET.
- [ ] `niri msg layers` menampilkan surface `vpet-rs`.
- [ ] Surface menutupi output dan tidak memesan ruang.
- [ ] Pet tampil di posisi yang benar.
- [ ] Animasi berjalan tanpa tearing.
- [ ] Drag bekerja, termasuk saat kursor keluar area pet.
- [ ] Klik di luar pet menembus ke window di bawah.
- [ ] Toggle click-through bekerja.
- [ ] Hover pet tidak memindahkan fokus keyboard.
- [ ] Ganti workspace: pet tetap terlihat.
- [ ] Fullscreen: `top` hilang, `overlay` tetap.
- [ ] Overview: perilaku sesuai harapan (hide/visible).
- [ ] Scale 1 benar; fractional scale diuji terpisah.
- [ ] CPU/memori dalam budget.

---

## 17. Milestone

| Milestone | Isi | Kriteria selesai | Estimasi |
|---|---|---|---|
| **M0 — Spike** | Layer surface, buffer, input region, click-through, drag, satu animasi PNG | Semua checklist Gate 0 lolos | 2–3 hari |
| **M1 — Skeleton** | Workspace, config, logging, IPC, tray, CI, check-no-dotnet | `cargo test` hijau; binary start | 3–5 hari |
| **M2 — Animation** | Loader PNG/APNG, index, cache, state machine, walk | Idle/sleep/touch/raised/walk mulus | 1–2 minggu |
| **M3 — Simulation** | Stats, mode, makan, inventory, save/load | Save bertahan setelah restart | 2–3 minggu |
| **M4 — MVP** | Tray, keybind niri, packaging AUR, polish | Bisa dipakai harian | 1 minggu |
| **M5 — Gameplay** | Work/study/play, buff, schedule, chat, audio | Fitur inti VPet tersedia | 3–5 minggu |
| **M6 — Content** | Data mod, tema, lokalisasi, gallery | Mod data VPet berjalan | 4–6 minggu |
| **M7 — Optional** | Steam, workshop, WASM plugin, UI settings | — | 8+ minggu |

Estimasi untuk satu developer Rust berpengalaman, aset sudah tersedia.

---

## 18. Risiko dan mitigasi

| Risiko | Dampak | Mitigasi |
|---|---|---|
| Layer-shell/input region berperilaku beda di niri | Proyek tidak layak | Gate 0 sebelum menulis simulasi |
| `wl_shm` terlalu berat di resolusi tinggi | FPS rendah | Damage rect, surface kecil, GPU |
| Fokus/overview niri mengganggu | UX buruk | `keyboard_interactivity=none`, hide via IPC |
| Scope creep | Tidak selesai | Non-goals tegas, milestone |
| Plugin .NET hilang | Ekosistem berkurang | WASM/Rhai di fase akhir |
| Aset VPet berlisensi terpisah | Tidak bisa distribusi bebas | Atribusi, non-komersial, paket konten terpisah |
| Upstream VPet berubah | Konten tertinggal | Sinkronisasi aset berkala |
| Bug perilaku game | Rasa game berbeda | Golden test + referensi C# |
| Audio PipeWire | Suara tidak jalan | `rodio`/`cpal`, uji di sistem target |
| Steam native lib | Gagal muat | Fase terpisah, fallback tanpa Steam |

---

## 19. Lisensi

- Kode VPetRust: pilih **MIT OR Apache-2.0** (disarankan).
- Aset VPet: mengikuti ketentuan VUP-Simulator.
  - wajib atribusi;
  - tidak boleh dijual;
  - penggunaan komersial memerlukan izin;
  - cantumkan tautan ke proyek VPet.
- Jangan memberi kesan bahwa VPetRust adalah produk resmi VPet.

---

## 20. Pertanyaan terbuka

1. Satu pet atau banyak pet per output?
2. Perlu import save VPet di MVP, atau fase berikutnya?
3. Mod scripting: WASM, Rhai, atau tidak sama sekali?
4. Steam/Workshop: masuk roadmap atau tidak?
5. Perlu fallback X11?
6. Nama dan branding final?
7. Apakah aset akan didistribusikan atau selalu user-supplied?

Default yang diusulkan:

- satu pet;
- import save ditunda;
- WASM ditunda;
- Steam ditunda;
- Wayland-only;
- nama `VPetRust` sementara;
- aset user-supplied.

---

## 21. Definition of Done MVP

- Binary Rust murni, tidak ada dependensi .NET.
- Berjalan di niri sebagai layer surface.
- Layer `top` dan `overlay` bisa dipilih.
- Click-through bekerja lewat input region.
- Tidak merebut fokus keyboard.
- Pet beranimasi dan bisa digeser.
- Stats berubah, makan berfungsi, save bertahan.
- Tray waybar dan IPC berfungsi.
- Config TOML terdokumentasi.
- Paket AUR bisa dibangun.
- Checklist Gate 0 dan test workspace hijau.
- Dokumentasi `docs/` lengkap untuk format aset, save, IPC, dan niri.

---

## 22. Langkah berikutnya

1. Jalankan Gate 0 (spike layer-shell) sebelum menulis simulasi.
2. Buat workspace dan crate kosong.
3. Implementasikan `vpet-wayland` minimal: surface + shm + satu PNG.
4. Implementasikan input region dan click-through.
5. Implementasikan drag dan frame callback.
6. Baru lanjut ke animation loader dan simulasi.

Gate 0 adalah keputusan hidup-mati. Jangan menulis ekonomi, buff, atau UI sebelum
layer-shell, input region, dan perilaku niri terbukti benar.
