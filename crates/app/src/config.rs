//! TOML configuration loader and mtime-based live hot-reload tracker.

use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use vpet_core::ModeType;
use vpet_wayland::LayerMode;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub app: AppSection,
    pub display: DisplaySection,
    pub pet: PetSection,
    pub simulation: SimulationSection,
    pub audio: AudioSection,
    pub ai: AiSection,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSection {
    pub content_root: String,
    pub log_level: String,
    pub profile: String,
    pub cache_mb: usize,
}

impl Default for AppSection {
    fn default() -> Self {
        Self {
            content_root: "~/.local/share/vpet-rs/content/0000_core".to_owned(),
            log_level: "info".to_owned(),
            profile: "default".to_owned(),
            cache_mb: 256,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplaySection {
    pub output: String,
    pub layer: String,
    pub scale: f32,
    pub click_through: bool,
    pub position: [i32; 2],
    pub auto_walk: bool,
}

impl Default for DisplaySection {
    fn default() -> Self {
        Self {
            output: "auto".to_owned(),
            layer: "top".to_owned(),
            scale: 1.0,
            click_through: false,
            position: [80, 760],
            auto_walk: true,
        }
    }
}

impl DisplaySection {
    #[must_use]
    pub fn layer_mode(&self) -> LayerMode {
        LayerMode::parse(&self.layer).unwrap_or(LayerMode::Top)
    }

    #[must_use]
    pub fn sprite_px(&self) -> u32 {
        ((250.0 * self.scale.clamp(0.25, 4.0)).round() as u32).clamp(64, 1000)
    }

    #[must_use]
    pub fn preferred_output(&self) -> Option<String> {
        let trimmed = self.output.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("auto") {
            None
        } else {
            Some(trimmed.to_owned())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PetSection {
    pub name: String,
    pub pet_id: String,
    pub start_mode: String,
    pub locale: String,
}

impl Default for PetSection {
    fn default() -> Self {
        Self {
            name: "Loris".to_owned(),
            pet_id: "vup".to_owned(),
            start_mode: "nomal".to_owned(),
            locale: "en".to_owned(),
        }
    }
}

impl PetSection {
    #[must_use]
    pub fn parsed_start_mode(&self) -> ModeType {
        ModeType::parse_loose(&self.start_mode).unwrap_or(ModeType::Nomal)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SimulationSection {
    pub tick_hz: u32,
    pub autosave_secs: u64,
    pub self_care: bool,
    pub auto_work: bool,
}

impl Default for SimulationSection {
    fn default() -> Self {
        Self {
            tick_hz: 10,
            autosave_secs: 300,
            self_care: true,
            auto_work: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioSection {
    pub enabled: bool,
    pub volume: f32,
}

impl Default for AudioSection {
    fn default() -> Self {
        Self {
            enabled: true,
            volume: 0.8,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AiSection {
    pub enabled: bool,
    pub api_base: String,
    pub api_key: String,
    pub model: String,
    pub system_prompt: String,
    pub timeout_secs: u64,
}

impl Default for AiSection {
    fn default() -> Self {
        Self {
            enabled: true,
            api_base: "http://localhost:20128/v1".to_owned(),
            api_key: String::new(),
            model: "9routerall".to_owned(),
            system_prompt: "You are Loris, a lively and affectionate desktop pet. Keep your replies concise (1-2 short sentences) so they fit nicely in a speech bubble.".to_owned(),
            timeout_secs: 25,
        }
    }
}

impl AppConfig {
    pub fn load_file(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read config `{}`", path.display()))?;
        toml::from_str(&raw).with_context(|| format!("invalid TOML in `{}`", path.display()))
    }

    pub fn load_or_default(path: &Path) -> Self {
        if path.is_file() {
            match Self::load_file(path) {
                Ok(cfg) => return cfg,
                Err(err) => {
                    tracing::warn!(%err, "failed to parse config.toml; falling back to defaults");
                }
            }
        }
        Self::default()
    }

    pub fn to_pretty_toml(&self) -> Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }
}

#[derive(Debug)]
pub struct ConfigWatcher {
    path: PathBuf,
    last_mtime: Option<SystemTime>,
}

impl ConfigWatcher {
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        let last_mtime = file_mtime(&path);
        Self { path, last_mtime }
    }

    /// Return newly parsed config if `config.toml` mtime changed since last check.
    pub fn check_modified(&mut self) -> Option<AppConfig> {
        let cur = file_mtime(&self.path);
        if cur != self.last_mtime {
            self.last_mtime = cur;
            if self.path.is_file() {
                return AppConfig::load_file(&self.path).ok();
            }
        }
        None
    }
}

fn file_mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).ok()?.modified().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_default_toml_and_detects_hot_reload() {
        let cfg = AppConfig::default();
        let text = cfg.to_pretty_toml().unwrap();
        let parsed: AppConfig = toml::from_str(&text).unwrap();
        assert_eq!(cfg, parsed);
        assert_eq!(cfg.display.sprite_px(), 250);

        let tmp = std::env::temp_dir().join(format!("vpet_rs_cfg_{}.toml", std::process::id()));
        let _ = fs::remove_file(&tmp);
        let mut watcher = ConfigWatcher::new(tmp.clone());
        assert!(watcher.check_modified().is_none());

        fs::write(&tmp, "[display]\nlayer = \"overlay\"\nscale = 1.2\n").unwrap();
        let reloaded = watcher
            .check_modified()
            .expect("detected newly created config");
        assert_eq!(reloaded.display.layer_mode(), LayerMode::Overlay);
        assert_eq!(reloaded.display.sprite_px(), 300);

        let _ = fs::remove_file(&tmp);
    }
}
