# VPetRust Save Format (`v1`)

## Location & Backup Layout

- Primary save: `$XDG_DATA_HOME/vpet-rs/saves/<profile>.mpak` (default `~/.local/share/vpet-rs/saves/default.mpak`)
- Rolling backups: `<profile>.bak.1` (newest) through `<profile>.bak.5` (oldest)

## Wire Encoding

Named MessagePack map (`rmp_serde::to_vec_named`) serialized from `vpet_core::SaveFile`:

| Field | Type | Description |
|---|---|---|
| `version` | `u16` | Schema version (`1`) |
| `saved_at_unix` | `i64` | Save timestamp (UTC seconds) |
| `profile` | `String` | Save slot name |
| `pet` | `PetState` | Level, exp, money, stats, buffered nutrients, `ModeType`, sleep flag |
| `inventory` | `Vec<InventoryItem>` | Owned item IDs and stack counts |
| `buffs` | `Vec<BuffState>` | Active timed buffs (`ends_at_unix`, multipliers) |
| `schedule` | `Vec<ScheduledTask>` | Queued task definitions |
| `statistics` | `Statistics` | Lifetime simulation counters |
| `rng_state` | `u64` | Deterministic `SplitMix64` PRNG state |
| `output_positions` | `BTreeMap<String, (i32, i32)>` | Saved pet `(x, y)` coordinates per Wayland output name |

## Crash-Safe Atomic Persistence

1. Encode `SaveFile` in memory (`vpet_core::encode_save`).
2. Write to hidden sibling temporary file `.<profile>.mpak.tmp.<pid>` and call `fsync`.
3. Rotate `<profile>.bak.4 -> .bak.5` ... `<profile>.mpak -> .bak.1`.
4. Atomically `rename` temp file over `<profile>.mpak` and `fsync` parent directory.
5. On startup, `SaveStore::load_or_recover` tries `<profile>.mpak`, then `.bak.1`..`.bak.5` on corruption.
