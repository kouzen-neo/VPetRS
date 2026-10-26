# VPetRust Gameplay Formulas

Reference formulas implemented in `vpet-core` (derived from `GameSave_VPet.cs`):

## 1. Leveling & Stat Caps

- **Experience to next level:** `LevelUpNeed = 200 * Level - 100` (`100` at Level 1).
- **Likability cap:** starts at `100.0`, increases by `+10.0` per level-up; overflow above `LikabilityMax` converts `1:1` into `Health`.
- **Stamina / Hunger / Thirst cap (`StrengthMax`):**
  `max(1, (100 + floor((Level * (1 + LevelMax))^0.75) * 4) * (1 + attr_limit_strength))`
- **Mood cap (`FeelingMax`):**
  `max(1, (100 + floor((Level * (1 + LevelMax))^0.75) * 2) * (1 + attr_limit_feeling))`
- **Health:** clamped to `[0.0, 100.0]`. Negative `StrengthFood` or `StrengthDrink` spills directly into `Health`; negative `Feeling` splits `50% / 50%` into `Health` and `Likability`.

## 2. Feeding & Buffered Digestion (`StoreTake`)

- Consuming a `FoodEffect` deducts `1` item from inventory if owned, otherwise spends `price` from `money`.
- `Exp`, `Feeling`, `Health`, and `Likability` apply immediately (scaled by active `exp_multiplier` buffs).
- `Strength`, `StrengthFood`, and `StrengthDrink` split `50%` immediate + `50%` into `store_*` pools.
- Every `50` simulation ticks (`5.0 s` at `10 Hz`), `store_take()` transfers `10%` of each `store_*` pool into the active stat (snapping to `0` once `|store| < 1.0`).

## 3. Condition Mode (`CalMode`)

1. `realhel = 60 - (feeling/feeling_max >= 0.80 ? 12 : 0) - (likability >= 80 ? 12 : likability >= 40 ? 6 : 0)`
2. If `health <= realhel / 2` -> `ModeType::Ill`; else if `health <= realhel` -> `ModeType::PoorCondition`.
3. `realfel = 0.90 - (likability >= 80 ? 0.20 : likability >= 40 ? 0.10 : 0.0)`
4. If `feeling / feeling_max >= realfel` -> `ModeType::Happy`; else if `<= realfel / 2` -> `ModeType::PoorCondition`; else `ModeType::Nomal`.
