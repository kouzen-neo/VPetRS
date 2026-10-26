# VPetRust — TODO & Rencana Kerja

Dokumen ini adalah **papan kerja** proyek. `IMPLEMENTATION.md` menjelaskan *apa* dan
*mengapa*; dokumen ini menjelaskan *urutan*, *kriteria selesai*, dan *commit* untuk
setiap langkah.

**Status:** Fase 7 selesai (`v0.7.0`). Fase 8 (MVP hardening `v1.0.0`) siap dimulai.
**Terakhir diperbarui:** 2026-10-27

---

## 1. Aturan commit

Repo ini memakai **satu commit per fase** (boleh dipecah menjadi sub-commit yang sudah
didaftarkan di fase tersebut). Setiap commit harus:

- mengikuti [Conventional Commits](https://www.conventionalcommits.org/):
  `type(scope): subject`
  - `type`: `feat`, `fix`, `docs`, `refactor`, `test`, `perf`, `chore`, `build`, `ci`
  - `scope`: `core`, `content`, `render`, `wayland`, `app`, `mods`, `packaging`, `docs`
- subject ≤ 72 karakter, imperative, tanpa titik di akhir;
- body berupa bullet list singkat tentang apa yang berubah;
- **tanpa** `Co-Authored-By` atau trailer atribusi apa pun;
- lolos gate sebelum dibuat:
  ```bash
  cargo fmt --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  cargo build --workspace
  ./scripts/check-no-dotnet.sh
  ```
- menyertakan pembaruan `CHANGELOG.md` di bawah `## [Unreleased]` bila ada perubahan
  yang terlihat pengguna;
- mencentang kotak yang relevan di dokumen ini.

Tag anotasi dibuat di akhir fase sesuai tabel commit map (§3). Pesan tag:
`vX.Y.Z — <ringkasan fase>`.

---

## 2. Papan progres

| Fase | Nama | Status | Tag |
|---|---|---|---|
| 0 | Bootstrap repo & workspace | `[x]` selesai | `v0.0.1` |
| 1 | Gate 0 — spike layer-shell niri | `[x]` selesai | `v0.1.0` |
| 2 | Content layer (LPS, index animasi, mod) | `[x]` selesai | `v0.2.0` |
| 3 | Renderer & animation player | `[x]` selesai | `v0.3.0` |
| 4 | Simulasi core & save | `[x]` selesai | `v0.4.0` |
| 5 | Interaksi & state machine | `[x]` selesai | `v0.5.0` |
| 6 | Integrasi app (config, IPC, tray) | `[x]` selesai | `v0.6.0` |
| 7 | Packaging & dokumentasi | `[x]` selesai | `v0.7.0` |
| 8 | MVP hardening | `[ ]` belum | `v1.0.0` |
| 9 | Post-MVP (gameplay, audio, mods) | `[ ]` belum | — |

Legenda status: `[ ]` belum, `[~]` dikerjakan, `[x]` selesai, `[!]` terblokir.

---

## 3. Commit map

| Fase | Commit | Tag |
|---|---|---|
| 0 | `chore: bootstrap cargo workspace and project docs` | `v0.0.1` |
| 1 | `feat(wayland): bind wayland globals and create layer surface`<br>`feat(render): draw sprite to wl_shm buffer with tiny-skia`<br>`feat(wayland): derive input region from alpha mask`<br>`feat(wayland): add click-through toggle via empty input region`<br>`feat(wayland): implement pointer drag with implicit grab`<br>`feat(wayland): render on frame callbacks with damage tracking`<br>`docs(wayland): record gate 0 verification on niri` | `v0.1.0` |
| 2 | `feat(content): implement lps parser with golden tests`<br>`feat(content): index vpet animation folders`<br>`feat(content): scan mods with override priority`<br>`feat(content): load food, text, theme, and lang databases`<br>`docs(content): document lps and animation layout` | `v0.2.0` |
| 3 | `feat(render): decode png sequences, apng, and gif`<br>`feat(render): add byte-bounded lru frame cache`<br>`feat(render): decode frames off-thread with preloading`<br>`feat(render): track damage rectangles per frame`<br>`feat(render): draw chat bubbles with cosmic-text`<br>`test(render): add frame snapshot tests` | `v0.3.0` |
| 4 | `feat(core): model pet stats, modes, and leveling`<br>`feat(core): add item and food consumption`<br>`feat(core): implement deterministic simulation tick`<br>`feat(core): persist saves with atomic writes and backups`<br>`docs(core): document gameplay formulas and save format` | `v0.4.0` |
| 5 | `feat(app): wire hit regions for head, body, and raise`<br>`feat(app): drive animation state machine with a/b/c segments`<br>`feat(app): implement drag, raise, sleep, and auto-walk` | `v0.5.0` |
| 6 | `feat(app): load and hot-reload toml config`<br>`feat(app): expose unix socket ipc and vpet-rs ctl cli`<br>`feat(app): add status notifier tray menu`<br>`feat(app): autosave and enforce single instance`<br>`feat(app): select output and support multi-output surfaces` | `v0.6.0` |
| 7 | `build(packaging): add aur pkgbuild and desktop entry`<br>`docs: add niri, ipc, save, and asset documentation`<br>`ci: add check workflow for fmt, clippy, test, and no-dotnet` | `v0.7.0` |
| 8 | `perf: meet idle cpu and memory budget on 1080p`<br>`fix(app): recover from corrupt saves and lost outputs`<br>`docs: finalize readme and gate reports` | `v1.0.0` |
| 9 | dipecah saat dimulai; lihat §12 | — |

---

## 4. Fase 0 — Bootstrap repo & workspace

**Tujuan:** repo bisa di-clone, di-build, dan diuji; belum ada perilaku Wayland.

### Tugas

- [x] `git init` dengan branch `main`, set `user.name` / `user.email` lokal repo.
- [x] `.gitignore` (Rust, editor, OS, artefak build).
- [x] `.editorconfig` (UTF-8, LF, 4 spasi, final newline).
- [x] `rust-toolchain.toml` (channel stable + komponen `rustfmt`, `clippy`).
- [x] `Cargo.toml` workspace:
  - edition 2024, `resolver = "3"`;
  - members: `crates/core`, `crates/content`, `crates/render`, `crates/wayland`,
    `crates/app`, `crates/mods`, `crates/steam`;
  - `[workspace.package]` untuk versi, edition, license, rust-version;
  - `[profile.release]`: `lto = "thin"`, `codegen-units = 1`, `strip = true`,
    `panic = "abort"`.
- [x] Tujuh crate kosong dengan `lib.rs`/`main.rs` dan doc comment satu baris.
- [x] `vpet-core` **tanpa** dependency workspace lain; `vpet-app` bergantung pada
      crate lain; `vpet-steam` ditandai `publish = false` dan belum masuk build default.
- [x] `scripts/check-no-dotnet.sh`:
  - `ldd` binary tidak mengandung `coreclr|mono|dotnet`;
  - `cargo tree` tidak mengandung `dotnet|mono|avalonia|coreclr`;
  - exit non-zero bila ditemukan.
- [x] `README.md`, `CHANGELOG.md`, `TODO.md`, `AGENTS.md`.
- [x] `cargo build --workspace` dan `cargo test --workspace` hijau.

### Kriteria selesai

- `cargo metadata` menampilkan 7 crate tanpa error.
- `./scripts/check-no-dotnet.sh` lulus.
- Satu commit: `chore: bootstrap cargo workspace and project docs`, tag `v0.0.1`.

---

## 5. Fase 1 — Gate 0: spike layer-shell di niri

**Tujuan:** membuktikan layer-shell, input region, dan pergerakan bekerja di niri
**sebelum** menulis gameplay. Ini gerbang hidup-mati proyek.

### Tugas

- [x] `vpet-wayland`: koneksi `wayland-client` + `calloop` event loop.
- [x] Bind global: `wl_compositor`, `wl_shm`, `wl_output`, `wl_seat`,
      `zwlr_layer_shell_v1`.
- [x] Pilih output dari config/env (`VPET_OUTPUT`), fallback output pertama.
- [x] Buat `wl_surface` + `zwlr_layer_surface_v1`:
  - namespace `vpet-rs`;
  - layer dari env (`VPET_LAYER=top|overlay`, default `top`);
  - anchor 4 sisi, size `0×0`, `exclusive_zone = -1`;
  - `keyboard_interactivity = none`.
- [x] Terima `configure`, buat `SlotPool`, alokasi dua buffer, tangani `release`.
- [x] `vpet-render`: gambar satu PNG (dari `VPET_SPRITE`) ke pixmap `tiny-skia`,
      swizzle RGBA→BGRA, salin ke buffer.
- [x] Hitung input region dari alpha mask (per-baris, batas < 512 rect).
- [x] Toggle click-through lewat sinyal sederhana (env `VPET_CLICKTHROUGH=1` dan
      `SIGUSR1`) — IPC penuh menyusul di Fase 6.
- [x] Pointer: hover, button, motion; drag memindahkan posisi internal `(x, y)`
      dengan implicit grab sampai release.
- [x] Frame callback + damage rect area lama/baru; commit hanya saat berubah.
- [x] Timer 30 fps untuk animasi gerak sederhana (pet berjalan bolak-balik).
- [x] `docs/gate0-report.md`: hasil verifikasi + screenshot + catatan perilaku niri.

### Kriteria selesai (checklist Gate 0)

- [x] Binary start tanpa .NET.
- [x] `niri msg layers` menampilkan surface `vpet-rs`.
- [x] Surface menutupi output, tidak memesan ruang (`exclusive_zone` negatif).
- [x] Sprite tampil di posisi yang benar.
- [x] Gerakan 30 fps tidak jitter.
- [x] Drag bekerja, termasuk saat kursor keluar dari area sprite.
- [x] Klik di luar sprite menembus ke window di bawah.
- [x] Click-through toggle bekerja (region kosong).
- [x] Hover di atas pet tidak memindahkan fokus keyboard.
- [x] Ganti workspace: pet tetap terlihat.
- [x] Fullscreen: `top` hilang, `overlay` tetap terlihat.
- [x] Overview niri: catat perilaku aktual (terlihat / tidak).
- [x] Scale 1 benar; catat hasil uji fractional scale.
- [x] CPU idle < 5% satu core, memori < 150 MB.

### Gate keputusan

Jika salah satu dari empat hal inti gagal — (1) posisi benar, (2) input region
menghormati alpha, (3) click-through bekerja, (4) surface tidak merebut fokus —
**berhenti** dan laporkan. Jangan lanjut ke Fase 2.

---

## 6. Fase 2 — Content layer

- [x] Parser LPS: BOM, CRLF, `|`, `#`, `:`, escape, duplikat, urutan.
- [x] Golden test memakai file asli: `info.lps`, `pet/vup.lps`, `food/food.lps`,
      `text/ClickText.lps`, `theme/default.lps`, `lang/en/*.lps`.
- [x] Index animasi: `GraphType`, `AnimatType` (A/B/C), `ModeType`, varian, prioritas
      path; cocokkan dengan isi `mod/0000_core/pet/vup`.
- [x] Scanner mod: `0000_core` dulu, lalu folder lain urut nama; aturan override.
- [x] Database: food, text, theme, lang.
- [x] Toleransi data rusak: file gagal parse dicatat, tidak mematikan app.
- [x] `docs/assets-format.md`.
- [x] Unit test untuk tiap modul; tidak ada I/O jaringan.

---

## 7. Fase 3 — Renderer & animation player

- [x] Decoder PNG sequence dengan natural sort; APNG; GIF.
- [x] Cache LRU berbasis byte (default 256 MB), bisa dikonfigurasi.
- [x] Decode di thread terpisah (`std::thread` + channel), kirim frame siap pakai.
- [x] Preload idle/breath/sleep/touch saat startup.
- [x] Damage rect union area lama+baru; tidak clear layar penuh per frame.
- [x] Teks chat bubble dengan `cosmic-text` + rounded rect.
- [x] Snapshot test: render frame → PNG, bandingkan dengan toleransi.
- [x] Benchmark kasar: waktu decode dan waktu composite per frame.

---

## 8. Fase 4 — Simulasi core & save

- [x] `PetState`: name, level, exp, money, strength, food, drink, feeling, health,
      likability, mode.
- [x] Formula stats dan level-up; clamp ke batas level.
- [x] Mode turunan: `Nomal`, `Happy`, `PoorCondition`, `Ill`.
- [x] Item & makanan: konsumsi, efek, batas stok.
- [x] Tick deterministik 10 Hz dengan waktu disuntikkan.
- [x] RNG deterministik ber-seed dari save.
- [x] Save v1 (`rmp-serde`): tulis temp → fsync → rename → rotasi 5 backup.
- [x] Pemulihan dari save korup ke backup terbaru.
- [x] Test: roundtrip, migrasi placeholder, expiry buff.
- [x] `docs/gameplay-formulas.md`, `docs/save-format.md`.

---

## 9. Fase 5 — Interaksi & state machine

- [x] Hit area kepala/badan/angkat mengikuti konfigurasi `vup.lps`.
- [x] Klik kepala → `Touch_Head`; badan → `Touch_Body`.
- [x] Drag → `Raised_Dynamic`; drop → `Raised_Static` lalu `Common`.
- [x] Tidur, bangun, dan auto-walk dengan perilaku tepi layar.
- [x] State machine prioritas: raised > touch > work > say > move > idle > sleep.
- [x] Rantai `A_Start → B_Loop → C_End` dengan fallback ke `Single`.
- [x] Varian animasi dipilih oleh mode + seed.
- [x] Posisi disimpan per output.
- [x] Test state machine tanpa Wayland (headless).

---

## 10. Fase 6 — Integrasi app

- [x] Config `$XDG_CONFIG_HOME/vpet-rs/config.toml`, reload tanpa restart.
- [x] Satu instance: lock file di `$XDG_RUNTIME_DIR`.
- [x] IPC Unix socket `0600`, JSON per baris; `SO_PEERCRED` check.
- [x] CLI `vpet-rs ctl {show,hide,toggle-clickthrough,feed,status,reload,quit}`.
- [x] Tray `ksni` (Show/Hide, Click-through, Feed, Reload, Quit).
- [x] Autosave berkala + simpan saat keluar.
- [x] Multi-output: pilih output, tangani output hilang, pindah surface.
- [x] Logging `tracing` ke file + stderr, rotasi sederhana.
- [x] `docs/ipc.md`.

---

## 11. Fase 7 — Packaging & dokumentasi

- [x] `packaging/PKGBUILD` (`vpet-rs-git`), `.desktop`, `niri-snippet.kdl`.
- [x] `docs/niri.md`: autostart, `layer-rule`, keybind, perilaku fullscreen/overview.
- [x] `README.md` final: instalasi, konfigurasi, aset VPet, lisensi.
- [x] CI: fmt, clippy, test, build, check-no-dotnet.
- [x] Snippet `layer-rule` untuk `block-out-from "screencast"` (opsional, dikomentari).
- [x] Catatan lisensi aset VPet di README.

---

## 12. Fase 8 — MVP hardening

- [ ] Profiling idle & drag; capai budget performa.
- [ ] Uji 24 jam: tidak ada kebocoran buffer atau memori naik terus.
- [ ] Uji fractional scale (150%) dan multi-output.
- [ ] Pemulihan: save korup, output dicabut, compositor restart.
- [ ] Dokumentasi troubleshooting.
- [ ] Review checklist Definition of Done di `IMPLEMENTATION.md` §21.

---

## 13. Fase 9 — Post-MVP (belum dijadwalkan)

Urutan yang diusulkan, masing-masing jadi fase sendiri saat dimulai:

1. Work / study / play + buff + schedule.
2. Audio (`rodio`) untuk SFX dan voice.
3. Data mod lengkap + tema + lokalisasi.
4. Chat bubble lanjutan, gallery.
5. Plugin WASM (`wasmtime`) atau Rhai.
6. Steam / Workshop (`steamworks`).
7. Import save VPet.
8. UI settings (window `xdg_toplevel` terpisah, bukan layer-shell).

---

## 14. Keputusan yang masih terbuka

Perlu jawaban sebelum fase terkait dimulai:

| # | Pertanyaan | Dibutuhkan di fase | Default sementara |
|---|---|---|---|
| 1 | Aset VPet user-supplied atau dibundel? | 7 | user-supplied |
| 2 | Mod scripting: WASM, Rhai, atau tidak ada? | 9 | ditunda |
| 3 | Import save VPet masuk MVP? | 4 | tidak |
| 4 | Steam/Workshop masuk roadmap? | 9 | ditunda |
| 5 | Perlu fallback X11? | 7 | tidak |
| 6 | Nama & branding final? | 7 | `VPetRust` / binary `vpet-rs` |

---

## 15. Protokol eksekusi (untuk agent)

1. Baca `IMPLEMENTATION.md` bagian terkait + fase aktif di dokumen ini.
2. Kerjakan **satu fase penuh**, jangan mencampur fase lain.
3. Jalankan gate: `fmt`, `clippy`, `test`, `build`, `check-no-dotnet`.
4. Perbarui `CHANGELOG.md` (`## [Unreleased]`) untuk perubahan yang terlihat pengguna.
5. Centang kotak di fase itu dan ubah status di papan progres (§2).
6. Commit sesuai commit map (§3), tanpa trailer atribusi.
7. Buat tag bila fase selesai dan semua kriteria terpenuhi.
8. Lanjut ke fase berikutnya; berhenti dan tanya hanya bila:
   - Gate 0 gagal,
   - ada keputusan terbuka (§14) yang menghalangi,
   - ada perubahan di luar cakupan dokumen ini.
