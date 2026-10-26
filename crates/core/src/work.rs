//! Work / study / play session state machine (`Idle -> Working -> Finish/Cancel`).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkSession {
    pub name: String,
    pub graph: String,
    pub remaining_secs: f64,
    pub total_secs: f64,
    pub money_per_min: f64,
    pub exp_per_min: f64,
    pub food_cost_per_min: f64,
    pub drink_cost_per_min: f64,
    pub feeling_cost_per_min: f64,
    pub finish_bonus: f64,
}

impl WorkSession {
    #[must_use]
    pub const fn is_finished(&self) -> bool {
        self.remaining_secs <= 0.0
    }
}
