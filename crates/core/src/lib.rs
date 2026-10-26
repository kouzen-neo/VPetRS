//! Pure deterministic pet simulation core.

use serde::{Deserialize, Serialize};

/// Pet condition / mood tier matching upstream VPet `IGameSave.ModeType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ModeType {
    Happy,
    #[default]
    Nomal,
    PoorCondition,
    Ill,
}

impl ModeType {
    pub const ALL: [Self; 4] = [Self::Happy, Self::Nomal, Self::PoorCondition, Self::Ill];

    /// Upstream VPet `[Flags]` bit value (`Happy=1, Nomal=2, PoorCondition=4, Ill=8`).
    #[must_use]
    pub const fn bit(self) -> u8 {
        match self {
            Self::Happy => 1,
            Self::Nomal => 2,
            Self::PoorCondition => 4,
            Self::Ill => 8,
        }
    }

    #[must_use]
    pub const fn matches_mask(self, mask: u8) -> bool {
        mask == 0 || (mask & self.bit()) != 0
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Happy => "happy",
            Self::Nomal => "nomal",
            Self::PoorCondition => "poorcondition",
            Self::Ill => "ill",
        }
    }

    #[must_use]
    pub fn parse_loose(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "happy" => Some(Self::Happy),
            "nomal" | "normal" => Some(Self::Nomal),
            "poorcondition" | "poor_condition" | "poor" => Some(Self::PoorCondition),
            "ill" => Some(Self::Ill),
            _ => None,
        }
    }
}
