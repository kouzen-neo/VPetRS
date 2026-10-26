//! Wayland `wp_fractional_scale_v1` + `wp_viewporter` protocol glue (`scale_120`, where `120 == 1.0x`).

use std::sync::Arc;

use smithay_client_toolkit::dispatch2::Dispatch2;
use wayland_client::{
    Connection, Proxy, QueueHandle, backend::ObjectData, globals::GlobalList,
    protocol::wl_surface::WlSurface,
};
use wayland_protocols::wp::{
    fractional_scale::v1::client::{
        wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
        wp_fractional_scale_v1::{self, WpFractionalScaleV1},
    },
    viewporter::client::{wp_viewport::WpViewport, wp_viewporter::WpViewporter},
};

use crate::surface::WaylandShell;

#[derive(Debug, Default)]
pub struct FractionalScaleGlobals {
    viewporter: Option<WpViewporter>,
    manager: Option<WpFractionalScaleManagerV1>,
}

#[derive(Debug)]
pub struct SurfaceViewport {
    pub viewport: Option<WpViewport>,
    pub fractional: Option<WpFractionalScaleV1>,
}

impl Drop for SurfaceViewport {
    fn drop(&mut self) {
        if let Some(f) = self.fractional.take() {
            f.destroy();
        }
        if let Some(vp) = self.viewport.take() {
            vp.destroy();
        }
    }
}

#[derive(Debug, Default)]
pub struct NoopDispatchData;

#[derive(Debug)]
pub struct FractionalSurfaceData {
    pub surface: WlSurface,
}

impl FractionalScaleGlobals {
    #[must_use]
    pub fn bind(globals: &GlobalList, qh: &QueueHandle<WaylandShell>) -> Self {
        let viewporter = globals
            .bind::<WpViewporter, _, _>(qh, 1..=1, NoopDispatchData)
            .ok();
        let manager = globals
            .bind::<WpFractionalScaleManagerV1, _, _>(qh, 1..=1, NoopDispatchData)
            .ok();
        Self {
            viewporter,
            manager,
        }
    }

    #[must_use]
    pub fn attach(&self, surface: &WlSurface, qh: &QueueHandle<WaylandShell>) -> SurfaceViewport {
        let viewport = self
            .viewporter
            .as_ref()
            .map(|vp| vp.get_viewport(surface, qh, NoopDispatchData));
        let fractional = self.manager.as_ref().map(|mgr| {
            mgr.get_fractional_scale(
                surface,
                qh,
                FractionalSurfaceData {
                    surface: surface.clone(),
                },
            )
        });
        SurfaceViewport {
            viewport,
            fractional,
        }
    }
}

/// Convert logical pixel length `logical` to physical buffer pixels using `scale_120` (`120 == 100%`, `180 == 150%`).
#[must_use]
pub const fn logical_to_buffer_dim(logical: u32, scale_120: u32) -> u32 {
    let s = if scale_120 == 0 { 120 } else { scale_120 };
    (((logical as u64) * (s as u64) + 60) / 120) as u32
}

/// Convert signed logical coordinate `coord` to physical buffer pixel coordinate using `scale_120`.
#[must_use]
pub fn logical_to_buffer_coord(coord: i32, scale_120: u32) -> i32 {
    let s = i64::from(scale_120.max(1));
    let num = i64::from(coord) * s;
    let rounded = if num >= 0 {
        (num + 60) / 120
    } else {
        (num - 60) / 120
    };
    rounded.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

impl<P: Proxy> Dispatch2<P, WaylandShell> for NoopDispatchData {
    fn event(
        &self,
        _state: &mut WaylandShell,
        _proxy: &P,
        _event: P::Event,
        _conn: &Connection,
        _qh: &QueueHandle<WaylandShell>,
    ) {
    }

    fn event_created_child(_opcode: u16, _qh: &QueueHandle<WaylandShell>) -> Arc<dyn ObjectData> {
        unreachable!("no server-created child objects on viewporter/fractional manager")
    }
}

impl Dispatch2<WpFractionalScaleV1, WaylandShell> for FractionalSurfaceData {
    fn event(
        &self,
        state: &mut WaylandShell,
        _proxy: &WpFractionalScaleV1,
        event: wp_fractional_scale_v1::Event,
        _conn: &Connection,
        qh: &QueueHandle<WaylandShell>,
    ) {
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event {
            state.on_preferred_fractional_scale(&self.surface, scale, qh);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractional_scale_150_percent_math_is_exact() {
        // 1920x1080 logical @ 150% (scale_120 = 180) -> 2880x1620 physical buffer!
        assert_eq!(logical_to_buffer_dim(1920, 180), 2880);
        assert_eq!(logical_to_buffer_dim(1080, 180), 1620);
        assert_eq!(logical_to_buffer_coord(80, 180), 120);
        assert_eq!(logical_to_buffer_coord(-20, 180), -30);
    }
}
