//! Data mod loader and future scripting sandbox.
//!
//! Legacy `.NET` `MainPlugin` DLLs are intentionally unsupported by design (ADR-001).

use std::{fs, path::Path, path::PathBuf};

use tracing::warn;
use vpet_content::ContentDatabase;
pub use vpet_content::ModMetadata as LoadedMod;

#[derive(Debug, Default)]
pub struct ModLoader {
    roots: Vec<PathBuf>,
}

impl ModLoader {
    #[must_use]
    pub fn new(primary_root: impl Into<PathBuf>) -> Self {
        Self {
            roots: vec![primary_root.into()],
        }
    }

    pub fn add_mod_root(&mut self, extra_root: impl Into<PathBuf>) {
        let p = extra_root.into();
        if !self.roots.contains(&p) {
            self.roots.push(p);
        }
    }

    /// Load all configured roots into a unified [`ContentDatabase`], warning if any mod
    /// contains unsupported legacy `.dll` plugins while still loading its data assets.
    #[must_use]
    pub fn load_all(&self) -> ContentDatabase {
        let mut db = ContentDatabase::default();
        for root in &self.roots {
            if root.exists() {
                db.merge_root(root);
            }
        }
        for m in &db.mods {
            for dll in find_legacy_dlls(&m.path) {
                let msg = format!(
                    "ignoring legacy .NET plugin `{}` in mod `{}` (pure Rust runtime)",
                    dll.display(),
                    m.name
                );
                warn!("{msg}");
                db.warnings.push(msg);
            }
        }
        db
    }

    #[must_use]
    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }
}

fn find_legacy_dlls(mod_path: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let plugin_dir = mod_path.join("plugin");
    for dir in [mod_path, plugin_dir.as_path()] {
        if let Ok(entries) = fs::read_dir(dir) {
            for p in entries.filter_map(Result::ok).map(|e| e.path()) {
                if p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("dll"))
                {
                    found.push(p);
                }
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warns_and_skips_legacy_dotnet_plugin_dlls() {
        let tmp = std::env::temp_dir().join(format!("vpet_rs_dll_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let mod_dir = tmp.join("0020_legacy");
        fs::create_dir_all(mod_dir.join("plugin")).unwrap();
        fs::write(
            mod_dir.join("info.lps"),
            "vupmod#LegacyMod:|author#Test:|\n",
        )
        .unwrap();
        fs::write(mod_dir.join("plugin").join("MainPlugin.dll"), b"MZ-fake").unwrap();

        let loader = ModLoader::new(&tmp);
        let db = loader.load_all();
        assert_eq!(db.mods.len(), 1);
        assert!(db.warnings.iter().any(|w| w.contains("MainPlugin.dll")));

        let _ = fs::remove_dir_all(&tmp);
    }
}
