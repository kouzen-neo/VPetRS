//! One-way import from legacy VPet-Simulator (`Save.lps` / `GameSave_v2`) into VPetRS.

use std::path::Path;

use anyhow::{Context, Result, bail};
use vpet_content::LpsDocument;
use vpet_core::{DeterministicRng, ModeType, PetState, Statistics, World};

pub fn import_legacy_vpet_save(lps_path: &Path, profile: &str) -> Result<World> {
    let doc = LpsDocument::parse_file(lps_path)
        .with_context(|| format!("failed to read legacy VPet save `{}`", lps_path.display()))?;

    let Some(vpet_line) = doc.find_line("vpet").or_else(|| doc.lines.first()) else {
        bail!(
            "legacy save `{}` has no valid `vpet` line",
            lps_path.display()
        );
    };

    let name = if vpet_line.info.trim().is_empty() {
        "Loris".to_owned()
    } else {
        vpet_line.info.trim().to_owned()
    };

    let mut pet = PetState::new(name);
    if let Some(m) = vpet_line.get_f64("money") {
        pet.money = m.max(0.0);
    }
    if let Some(lv) = vpet_line.get_u32("level") {
        pet.level = lv.max(1);
    }
    if let Some(lvm) = vpet_line.get_u32("levelmax") {
        pet.level_max = lvm;
    }
    if let Some(exp) = vpet_line.get_f64("exp") {
        pet.exp = exp.max(0.0);
    }
    if let Some(s) = vpet_line.get_f64("strength") {
        pet.strength = s.max(0.0);
    }
    if let Some(ss) = vpet_line.get_f64("storestrength") {
        pet.store_strength = ss.max(0.0);
    }
    if let Some(sf) = vpet_line.get_f64("strengthfood") {
        pet.strength_food = sf.max(0.0);
    }
    if let Some(ssf) = vpet_line.get_f64("storestrengthfood") {
        pet.store_strength_food = ssf.max(0.0);
    }
    if let Some(sd) = vpet_line.get_f64("strengthdrink") {
        pet.strength_drink = sd.max(0.0);
    }
    if let Some(ssd) = vpet_line.get_f64("storestrengthdrink") {
        pet.store_strength_drink = ssd.max(0.0);
    }
    if let Some(f) = vpet_line.get_f64("feeling") {
        pet.feeling = f.max(0.0);
    }
    if let Some(h) = vpet_line.get_f64("health") {
        pet.set_health(h);
    }
    if let Some(l) = vpet_line.get_f64("likability") {
        pet.likability = l.max(0.0);
    }
    if let Some(lm) = vpet_line.get_f64("likabilitymax") {
        pet.likability_max = lm.max(100.0);
    }
    if let Some(m) = vpet_line.get("mode").and_then(ModeType::parse_loose) {
        pet.mode = m;
    } else {
        pet.mode = pet.cal_mode();
    }

    let mut statistics = Statistics::default();
    if let Some(stat_line) = doc.find_line("statistics") {
        statistics.times_fed = stat_line.get_u32("Feed").unwrap_or(0) as u64;
        statistics.times_petted = stat_line.get_u32("Touch").unwrap_or(0) as u64;
    }

    let seed = 0x5650_4554_5341_5645;
    let mut world = World::new(profile, pet.name.clone(), seed);
    world.pet = pet;
    world.statistics = statistics;
    world.rng = DeterministicRng::from_seed(seed);

    Ok(world)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_legacy_vpet_save_lps() {
        let raw = "vpet#LorisTest:|money#450.5:|exp#120:|level#5:|strength#95:|strengthfood#80:|strengthdrink#85:|feeling#75:|health#98:|likability#45:|mode#Happy:|\n\
            statistics:|Feed#12:|Touch#34:|\n";
        let tmp = std::env::temp_dir().join(format!("vpet_legacy_{}.lps", std::process::id()));
        std::fs::write(&tmp, raw).unwrap();

        let world = import_legacy_vpet_save(&tmp, "imported").unwrap();
        assert_eq!(world.profile, "imported");
        assert_eq!(world.pet.name, "LorisTest");
        assert_eq!(world.pet.level, 5);
        assert!((world.pet.money - 450.5).abs() < 1e-6);
        assert_eq!(world.pet.mode, ModeType::Happy);
        assert_eq!(world.statistics.times_fed, 12);
        assert_eq!(world.statistics.times_petted, 34);

        let _ = std::fs::remove_file(&tmp);
    }
}
