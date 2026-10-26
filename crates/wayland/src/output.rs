//! Wayland output discovery, selection, and hotplug fallback.

use smithay_client_toolkit::output::OutputState;
use wayland_client::protocol::wl_output::WlOutput;

#[derive(Debug, Clone)]
pub struct OutputDescriptor {
    pub wl_output: WlOutput,
    pub name: String,
    pub logical_size: (u32, u32),
    pub scale_factor: i32,
}

#[derive(Debug, Default)]
pub struct OutputTracker {
    outputs: Vec<OutputDescriptor>,
}

impl OutputTracker {
    pub fn sync_from_state(&mut self, state: &OutputState) {
        let mut refreshed = Vec::new();
        for wl_output in state.outputs() {
            let info = state.info(&wl_output);
            let name = info
                .as_ref()
                .and_then(|i| i.name.clone())
                .unwrap_or_else(|| format!("output-{}", refreshed.len()));
            let logical_size = info
                .as_ref()
                .and_then(|i| i.logical_size)
                .map(|(w, h)| (w.max(1) as u32, h.max(1) as u32))
                .unwrap_or((1920, 1080));
            let scale_factor = info.as_ref().map_or(1, |i| i.scale_factor.max(1));
            refreshed.push(OutputDescriptor {
                wl_output,
                name,
                logical_size,
                scale_factor,
            });
        }
        self.outputs = refreshed;
    }

    pub fn outputs(&self) -> &[OutputDescriptor] {
        &self.outputs
    }

    /// Select output matching `preferred` (`None` or `"auto"` picks the first output).
    pub fn select(&self, preferred: Option<&str>) -> Option<&OutputDescriptor> {
        if let Some(target) = preferred.filter(|p| !p.is_empty() && !p.eq_ignore_ascii_case("auto"))
            && let Some(found) = self.outputs.iter().find(|o| o.name == target)
        {
            return Some(found);
        }
        self.outputs.first()
    }

    pub fn contains(&self, wl_output: &WlOutput) -> bool {
        self.outputs.iter().any(|o| &o.wl_output == wl_output)
    }

    pub fn find_by_wl(&self, wl_output: &WlOutput) -> Option<&OutputDescriptor> {
        self.outputs.iter().find(|o| &o.wl_output == wl_output)
    }
}
