//! SCTK `wlr-layer-shell` full-output pet surface with alpha input region and frame throttling.

use std::num::NonZeroU32;

use calloop::LoopHandle;
use calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, FrameCallbackData, Region},
    delegate_dispatch2, delegate_registry,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        pointer::{
            CursorIcon, PointerEvent, PointerEventKind, PointerHandler, ThemeSpec, ThemedPointer,
        },
    },
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
    },
    shm::{Shm, ShmHandler},
};
use thiserror::Error;
use tracing::{debug, info, warn};
use vpet_render::{Pixmap, compose_pet_and_bubble};
use wayland_client::{
    Connection, QueueHandle,
    globals::{BindError, GlobalError, registry_queue_init},
    protocol::{wl_output, wl_pointer, wl_seat, wl_surface},
};

use crate::{
    fractional::{
        FractionalScaleGlobals, SurfaceViewport, logical_to_buffer_coord, logical_to_buffer_dim,
    },
    input::{PointerTracker, SurfaceEvent},
    output::OutputTracker,
    region::alpha_mask_rects_scale120,
    shm::{ShmPoolError, ShmSwapchain},
};

fn connect_wayland() -> Result<Connection, WaylandError> {
    if let Ok(conn) = Connection::connect_to_env() {
        return Ok(conn);
    }
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR")
        && let Ok(entries) = std::fs::read_dir(&runtime_dir)
    {
        let mut candidates: Vec<std::path::PathBuf> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.file_name().and_then(|n| n.to_str()).is_some_and(|name| {
                    name.strip_prefix("wayland-").is_some_and(|rest| {
                        !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit())
                    })
                })
            })
            .collect();
        candidates.sort();
        for sock in candidates {
            if let Ok(stream) = std::os::unix::net::UnixStream::connect(&sock)
                && let Ok(conn) = Connection::from_socket(stream)
            {
                return Ok(conn);
            }
        }
    }
    Connection::connect_to_env().map_err(WaylandError::Connect)
}
/// Maximum rectangles added to `wl_region` per commit (`< 512` per architecture spec).
pub const MAX_INPUT_REGION_RECTS: usize = 511;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerMode {
    Top,
    Overlay,
}

impl LayerMode {
    #[must_use]
    pub const fn to_wlr(self) -> Layer {
        match self {
            Self::Top => Layer::Top,
            Self::Overlay => Layer::Overlay,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Overlay => "overlay",
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        if raw.eq_ignore_ascii_case("top") {
            Some(Self::Top)
        } else if raw.eq_ignore_ascii_case("overlay") {
            Some(Self::Overlay)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct SurfaceConfig {
    pub namespace: String,
    pub preferred_output: Option<String>,
    pub layer: LayerMode,
    pub click_through: bool,
    pub visible: bool,
    pub initial_position: (i32, i32),
}

impl Default for SurfaceConfig {
    fn default() -> Self {
        Self {
            namespace: "vpet-rs".to_owned(),
            preferred_output: None,
            layer: LayerMode::Top,
            click_through: false,
            visible: true,
            initial_position: (40, 40),
        }
    }
}

#[derive(Debug, Error)]
pub enum WaylandError {
    #[error("failed to connect to Wayland display: {0}")]
    Connect(#[from] wayland_client::ConnectError),
    #[error("failed to initialize Wayland global registry: {0}")]
    Global(#[from] GlobalError),
    #[error("missing required Wayland protocol global: {0}")]
    Bind(#[from] BindError),
    #[error("Wayland dispatch failed: {0}")]
    Dispatch(#[from] wayland_client::DispatchError),
    #[error("shm pool error: {0}")]
    Shm(#[from] ShmPoolError),
    #[error("failed to insert Wayland event source into calloop: {0}")]
    InsertSource(String),
}

pub struct WaylandShell {
    conn: Connection,
    qh: QueueHandle<Self>,
    registry_state: RegistryState,
    compositor: CompositorState,
    layer_shell: LayerShell,
    shm: Shm,
    output_state: OutputState,
    seat_state: SeatState,
    fractional_globals: FractionalScaleGlobals,

    outputs: OutputTracker,
    active_output: Option<wl_output::WlOutput>,
    active_output_name: String,
    layer_surface: Option<LayerSurface>,
    surface_viewport: Option<SurfaceViewport>,
    swapchain: Option<ShmSwapchain>,
    themed_pointer: Option<ThemedPointer>,
    pointer_tracker: PointerTracker,

    config: SurfaceConfig,
    logical_size: (u32, u32),
    scale_factor: i32,
    scale_120: u32,
    initialized: bool,
    configured: bool,
    closed: bool,

    sprite: Pixmap,
    bubble: Option<Pixmap>,
    pet_pos: (i32, i32),
    needs_redraw: bool,
    needs_region_update: bool,
    frame_callback_pending: bool,
    last_rect_count: usize,
    frames_committed: u64,

    pending_events: Vec<SurfaceEvent>,
}

impl WaylandShell {
    /// Connect to Wayland, bind globals, discover outputs, create the layer surface,
    /// and register the `WaylandSource` on `loop_handle`.
    pub fn connect_and_insert<D>(
        config: SurfaceConfig,
        initial_sprite: Pixmap,
        loop_handle: &LoopHandle<'_, D>,
        map_shell: fn(&mut D) -> &mut WaylandShell,
    ) -> Result<Self, WaylandError>
    where
        D: 'static,
    {
        let conn = connect_wayland()?;
        let (globals, mut event_queue) = registry_queue_init::<Self>(&conn)?;
        let qh = event_queue.handle();

        let compositor = CompositorState::bind(&globals, &qh)?;
        let layer_shell = LayerShell::bind(&globals, &qh)?;
        let shm = Shm::bind(&globals, &qh)?;
        let output_state = OutputState::new(&globals, &qh);
        let seat_state = SeatState::new(&globals, &qh);
        let fractional_globals = FractionalScaleGlobals::bind(&globals, &qh);
        let registry_state = RegistryState::new(&globals);

        let pet_pos = config.initial_position;
        let mut shell = Self {
            conn: conn.clone(),
            qh: qh.clone(),
            registry_state,
            compositor,
            layer_shell,
            shm,
            output_state,
            seat_state,
            fractional_globals,
            outputs: OutputTracker::default(),
            active_output: None,
            active_output_name: "auto".to_owned(),
            layer_surface: None,
            surface_viewport: None,
            swapchain: None,
            themed_pointer: None,
            pointer_tracker: PointerTracker::default(),
            config,
            logical_size: (1920, 1080),
            scale_factor: 1,
            scale_120: 120,
            initialized: false,
            configured: false,
            closed: false,
            sprite: initial_sprite,
            bubble: None,
            pet_pos,
            needs_redraw: true,
            needs_region_update: true,
            frame_callback_pending: false,
            last_rect_count: 0,
            frames_committed: 0,
            pending_events: Vec::new(),
        };

        // Populate output names/scales before creating the target layer surface.
        event_queue.roundtrip(&mut shell)?;
        shell.outputs.sync_from_state(&shell.output_state);
        shell.initialized = true;
        shell.recreate_layer_surface(&qh)?;
        event_queue.roundtrip(&mut shell)?;

        loop_handle
            .insert_source(
                WaylandSource::new(conn, event_queue),
                move |(), queue, data| queue.dispatch_pending(map_shell(data)),
            )
            .map_err(|e| WaylandError::InsertSource(e.to_string()))?;

        Ok(shell)
    }

    fn recreate_layer_surface(&mut self, qh: &QueueHandle<Self>) -> Result<(), WaylandError> {
        let selected = self
            .outputs
            .select(self.config.preferred_output.as_deref())
            .cloned();

        let target_wl_output = selected.as_ref().map(|o| o.wl_output.clone());
        if let Some(desc) = &selected {
            self.active_output_name.clone_from(&desc.name);
            self.logical_size = desc.logical_size;
            self.scale_factor = desc.scale_factor.max(1);
            self.scale_120 = (self.scale_factor as u32) * 120;
        } else {
            self.active_output_name = "default".to_owned();
            self.scale_factor = 1;
            self.scale_120 = 120;
        }
        self.active_output = target_wl_output.clone();

        let wl_surface = self.compositor.create_surface(qh);
        self.surface_viewport = Some(self.fractional_globals.attach(&wl_surface, qh));
        // Install an empty input region prior to the initial map commit so transparent space
        // never intercepts pointer events.
        if let Ok(empty) = Region::new(&self.compositor) {
            wl_surface.set_input_region(Some(empty.wl_region()));
        }

        let layer = self.layer_shell.create_layer_surface(
            qh,
            wl_surface,
            self.config.layer.to_wlr(),
            Some(self.config.namespace.clone()),
            target_wl_output.as_ref(),
        );
        layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        layer.set_size(0, 0);
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer.commit();

        info!(
            output = %self.active_output_name,
            layer = %self.config.layer.as_str(),
            scale = self.scale_factor,
            "created layer-shell surface"
        );

        self.configured = false;
        self.frame_callback_pending = false;
        self.needs_redraw = true;
        self.needs_region_update = true;
        self.layer_surface = Some(layer);
        Ok(())
    }

    pub const fn is_closed(&self) -> bool {
        self.closed
    }

    pub const fn is_configured(&self) -> bool {
        self.configured
    }

    pub fn active_output_name(&self) -> &str {
        &self.active_output_name
    }

    pub const fn output_logical_size(&self) -> (u32, u32) {
        self.logical_size
    }

    pub const fn scale_factor(&self) -> i32 {
        self.scale_factor
    }

    pub const fn scale_120(&self) -> u32 {
        self.scale_120
    }

    pub(crate) fn on_preferred_fractional_scale(
        &mut self,
        surface: &wl_surface::WlSurface,
        scale_120: u32,
        qh: &QueueHandle<Self>,
    ) {
        if self
            .layer_surface
            .as_ref()
            .is_some_and(|l| l.wl_surface() == surface)
        {
            let clamped = scale_120.max(60);
            if self.scale_120 != clamped {
                self.scale_120 = clamped;
                self.needs_redraw = true;
                self.needs_region_update = true;
                self.present_if_needed(qh);
            }
        }
    }

    pub const fn position(&self) -> (i32, i32) {
        self.pet_pos
    }

    pub const fn is_dragging(&self) -> bool {
        self.pointer_tracker.is_dragging()
    }

    pub const fn click_through(&self) -> bool {
        self.config.click_through
    }

    pub const fn visible(&self) -> bool {
        self.config.visible
    }

    pub const fn layer_mode(&self) -> LayerMode {
        self.config.layer
    }

    pub const fn last_input_region_rects(&self) -> usize {
        self.last_rect_count
    }

    pub const fn frames_committed(&self) -> u64 {
        self.frames_committed
    }

    pub fn sprite_logical_size(&self) -> (u32, u32) {
        let s120 = self.scale_120.max(60);
        (
            ((u64::from(self.sprite.width()) * 120).div_ceil(u64::from(s120)) as u32).max(1),
            ((u64::from(self.sprite.height()) * 120).div_ceil(u64::from(s120)) as u32).max(1),
        )
    }

    pub fn set_sprite(&mut self, pixmap: Pixmap) {
        self.sprite = pixmap;
        self.needs_redraw = true;
        self.needs_region_update = true;
    }

    pub fn set_bubble(&mut self, bubble: Option<Pixmap>) {
        let changed = self.bubble.is_some() != bubble.is_some() || bubble.is_some();
        self.bubble = bubble;
        if changed {
            self.needs_redraw = true;
        }
    }

    pub fn set_position(&mut self, x: i32, y: i32) {
        let (sw, sh) = self.sprite_logical_size();
        let max_x = (self.logical_size.0 as i32 - sw as i32).max(0);
        let max_y = (self.logical_size.1 as i32 - sh as i32).max(0);
        let clamped = (x.clamp(0, max_x), y.clamp(0, max_y));
        if self.pet_pos != clamped {
            self.pet_pos = clamped;
            self.needs_redraw = true;
            self.needs_region_update = true;
        }
    }

    pub fn set_click_through(&mut self, enabled: bool) {
        if self.config.click_through != enabled {
            self.config.click_through = enabled;
            self.needs_region_update = true;
            info!(click_through = enabled, "updated click-through state");
        }
    }

    pub fn toggle_click_through(&mut self) -> bool {
        let next = !self.config.click_through;
        self.set_click_through(next);
        next
    }

    pub fn set_visible(&mut self, visible: bool) {
        if self.config.visible != visible {
            self.config.visible = visible;
            self.needs_redraw = true;
            self.needs_region_update = true;
            info!(visible, "updated surface visibility");
        }
    }

    pub fn set_layer_mode(&mut self, mode: LayerMode) {
        if self.config.layer != mode {
            self.config.layer = mode;
            if let Some(layer) = &self.layer_surface {
                layer.set_layer(mode.to_wlr());
                self.needs_region_update = true;
                info!(layer = %mode.as_str(), "switched layer mode");
            }
        }
    }

    pub fn select_output(&mut self, preferred: Option<String>) -> Result<(), WaylandError> {
        if self.config.preferred_output != preferred {
            self.config.preferred_output = preferred;
            let qh = self.qh.clone();
            self.recreate_layer_surface(&qh)?;
        }
        Ok(())
    }

    /// Convenience wrapper around [`Self::present_if_needed`] using the stored `QueueHandle`.
    pub fn present(&mut self) {
        let qh = self.qh.clone();
        self.present_if_needed(&qh);
    }

    pub fn drain_events(&mut self) -> Vec<SurfaceEvent> {
        std::mem::take(&mut self.pending_events)
    }

    pub fn flush(&self) {
        let _ = self.conn.flush();
    }

    fn apply_input_region(&mut self, wl_surface: &wl_surface::WlSurface) {
        let Ok(region) = Region::new(&self.compositor) else {
            return;
        };

        if self.config.click_through || !self.config.visible {
            // Empty wl_region = strict click-through (NEVER None, which means infinite/whole surface).
            wl_surface.set_input_region(Some(region.wl_region()));
            self.last_rect_count = 0;
        } else {
            let rects = alpha_mask_rects_scale120(
                self.sprite.data(),
                (self.sprite.width(), self.sprite.height()),
                self.scale_120,
                self.pet_pos,
                16,
                MAX_INPUT_REGION_RECTS,
            );
            for r in &rects {
                region.add(r.x, r.y, r.w, r.h);
            }
            wl_surface.set_input_region(Some(region.wl_region()));
            self.last_rect_count = rects.len();
        }
        self.needs_region_update = false;
    }

    /// Commit pending state or draw a new frame if dirty and no frame callback is in flight.
    pub fn present_if_needed(&mut self, qh: &QueueHandle<Self>) {
        if !self.configured {
            return;
        }
        let Some(layer) = self.layer_surface.clone() else {
            return;
        };
        let wl_surface = layer.wl_surface().clone();

        if !self.needs_redraw {
            if self.needs_region_update && !self.frame_callback_pending {
                self.apply_input_region(&wl_surface);
                layer.commit();
                self.flush();
            }
            return;
        }

        if self.frame_callback_pending {
            return;
        }

        let s120 = self.scale_120.max(60);
        let is_fractional = !s120.is_multiple_of(120)
            && self
                .surface_viewport
                .as_ref()
                .is_some_and(|sv| sv.viewport.is_some());
        let buf_w = logical_to_buffer_dim(self.logical_size.0, s120).max(1);
        let buf_h = logical_to_buffer_dim(self.logical_size.1, s120).max(1);

        if self.swapchain.is_none() {
            match ShmSwapchain::new(&self.shm, buf_w, buf_h) {
                Ok(sc) => self.swapchain = Some(sc),
                Err(err) => {
                    warn!(%err, "failed to create shm swapchain");
                    return;
                }
            }
        }
        if let Some(sc) = &mut self.swapchain
            && let Err(err) = sc.resize(&self.shm, buf_w, buf_h)
        {
            warn!(%err, "failed to resize shm swapchain");
            return;
        }

        if is_fractional {
            wl_surface.set_buffer_scale(1);
            if let Some(vp) = self
                .surface_viewport
                .as_ref()
                .and_then(|sv| sv.viewport.as_ref())
            {
                vp.set_destination(self.logical_size.0 as i32, self.logical_size.1 as i32);
            }
        } else {
            if let Some(vp) = self
                .surface_viewport
                .as_ref()
                .and_then(|sv| sv.viewport.as_ref())
            {
                vp.set_destination(-1, -1);
            }
            wl_surface.set_buffer_scale((s120 / 120).max(1) as i32);
        }
        self.apply_input_region(&wl_surface);

        let composed_scene;
        let sprite_arg = if self.config.visible {
            let px_x = logical_to_buffer_coord(self.pet_pos.0, s120);
            let px_y = logical_to_buffer_coord(self.pet_pos.1, s120);
            if self.bubble.is_some() {
                let (scene, delta) = compose_pet_and_bubble(&self.sprite, self.bubble.as_ref());
                composed_scene = scene;
                let dst_x = px_x + delta.0;
                let dst_y = (px_y + delta.1).max(0);
                Some((&composed_scene, dst_x, dst_y))
            } else {
                Some((&self.sprite, px_x, px_y))
            }
        } else {
            None
        };

        let prepared = match self
            .swapchain
            .as_mut()
            .expect("swapchain initialized")
            .draw_and_attach(&wl_surface, sprite_arg)
        {
            Ok(p) => p,
            Err(ShmPoolError::NoAvailableBuffer) => {
                debug!("shm swapchain busy; deferring redraw until buffer release");
                return;
            }
            Err(err) => {
                warn!(%err, "shm draw_and_attach failed");
                return;
            }
        };

        let d = prepared.damage;
        wl_surface.damage_buffer(d.x, d.y, d.w, d.h);
        wl_surface.frame(qh, FrameCallbackData(wl_surface.clone()));
        self.frame_callback_pending = true;
        self.needs_redraw = false;
        self.frames_committed += 1;

        layer.commit();
        self.flush();
    }

    pub fn update_cursor(&self, icon: CursorIcon) {
        if let Some(tp) = &self.themed_pointer {
            let _ = tp.set_cursor(&self.conn, icon);
        }
    }
}

impl CompositorHandler for WaylandShell {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        new_factor: i32,
    ) {
        if self
            .layer_surface
            .as_ref()
            .is_some_and(|l| l.wl_surface() == surface)
        {
            let clamped = new_factor.max(1);
            if self.scale_factor != clamped {
                self.scale_factor = clamped;
                if self.scale_120.is_multiple_of(120) {
                    self.scale_120 = (clamped as u32) * 120;
                }
                self.needs_redraw = true;
                self.needs_region_update = true;
                self.present_if_needed(qh);
            }
        }
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        if self
            .layer_surface
            .as_ref()
            .is_some_and(|l| l.wl_surface() == surface)
        {
            self.frame_callback_pending = false;
            if self.needs_redraw || self.needs_region_update {
                self.present_if_needed(qh);
            }
        }
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for WaylandShell {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
        self.outputs.sync_from_state(&self.output_state);
        if self.initialized && self.layer_surface.is_none() {
            let _ = self.recreate_layer_surface(qh);
        }
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        self.outputs.sync_from_state(&self.output_state);
        if self.active_output.as_ref() == Some(&output)
            && let Some(desc) = self.outputs.find_by_wl(&output)
            && self.scale_factor != desc.scale_factor
        {
            self.scale_factor = desc.scale_factor.max(1);
            self.needs_redraw = true;
            self.present_if_needed(qh);
        }
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        self.outputs.sync_from_state(&self.output_state);
        if self.active_output.as_ref() == Some(&output) {
            warn!("active output disconnected; migrating pet layer surface to fallback output");
            self.surface_viewport = None;
            self.layer_surface = None;
            self.swapchain = None;
            if !self.outputs.outputs().is_empty() {
                let _ = self.recreate_layer_surface(qh);
            }
        }
    }
}

impl LayerShellHandler for WaylandShell {
    fn closed(&mut self, _conn: &Connection, qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.outputs.sync_from_state(&self.output_state);
        if !self.outputs.outputs().is_empty() {
            warn!("layer surface closed by compositor; attempting recovery on available output");
            self.surface_viewport = None;
            self.layer_surface = None;
            self.swapchain = None;
            if self.recreate_layer_surface(qh).is_ok() {
                return;
            }
        }
        self.closed = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if self.layer_surface.as_ref() != Some(layer) {
            return;
        }
        let w = NonZeroU32::new(configure.new_size.0)
            .map_or(self.logical_size.0.max(256), NonZeroU32::get);
        let h = NonZeroU32::new(configure.new_size.1)
            .map_or(self.logical_size.1.max(256), NonZeroU32::get);

        self.logical_size = (w, h);
        self.configured = true;
        self.set_position(self.pet_pos.0, self.pet_pos.1);
        self.needs_redraw = true;
        self.needs_region_update = true;
        debug!(width = w, height = h, "layer surface configured");
        self.present_if_needed(qh);
    }
}

impl SeatHandler for WaylandShell {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        // Never bind wl_keyboard: keyboard_interactivity = None ensures zero focus stealing.
        if capability == Capability::Pointer && self.themed_pointer.is_none() {
            let cursor_surface = self.compositor.create_surface(qh);
            match self.seat_state.get_pointer_with_theme::<_, ()>(
                qh,
                &seat,
                self.shm.wl_shm(),
                cursor_surface,
                ThemeSpec::default(),
            ) {
                Ok(tp) => {
                    self.themed_pointer = Some(tp);
                }
                Err(err) => warn!(%err, "failed to create themed pointer"),
            }
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer {
            self.themed_pointer.take();
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl PointerHandler for WaylandShell {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        let Some(target_surface) = self.layer_surface.as_ref().map(|l| l.wl_surface().clone())
        else {
            return;
        };

        let sprite_size = self.sprite_logical_size();
        for event in events {
            if event.surface != target_surface {
                continue;
            }
            match event.kind {
                PointerEventKind::Enter { .. } => {
                    self.update_cursor(if self.pointer_tracker.is_dragging() {
                        CursorIcon::Grabbing
                    } else {
                        CursorIcon::Default
                    });
                    if let Some(ev) = self.pointer_tracker.on_enter(event.position) {
                        self.pending_events.push(ev);
                    }
                }
                PointerEventKind::Leave { .. } => {
                    if let Some(ev) = self.pointer_tracker.on_leave() {
                        self.pending_events.push(ev);
                    }
                }
                PointerEventKind::Press { button, .. } => {
                    if let Some(ev) =
                        self.pointer_tracker
                            .on_press(button, event.position, self.pet_pos)
                    {
                        self.pending_events.push(ev);
                    }
                }
                PointerEventKind::Motion { .. } => {
                    if let Some(ev) = self.pointer_tracker.on_motion(
                        event.position,
                        self.logical_size,
                        sprite_size,
                    ) {
                        match ev {
                            SurfaceEvent::DragStarted { pet_x, pet_y } => {
                                self.update_cursor(CursorIcon::Grabbing);
                                self.set_position(pet_x, pet_y);
                            }
                            SurfaceEvent::Dragged { pet_x, pet_y } => {
                                self.set_position(pet_x, pet_y);
                            }
                            _ => {}
                        }
                        self.pending_events.push(ev);
                    }
                }
                PointerEventKind::Release { button, .. } => {
                    if let Some(ev) =
                        self.pointer_tracker
                            .on_release(button, event.position, self.pet_pos)
                    {
                        if matches!(ev, SurfaceEvent::DragEnded { .. }) {
                            self.update_cursor(CursorIcon::Default);
                        }
                        self.pending_events.push(ev);
                    }
                }
                PointerEventKind::Axis { .. } => {}
            }
        }

        if self.needs_redraw || self.needs_region_update {
            self.present_if_needed(qh);
        }
    }
}

impl ShmHandler for WaylandShell {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

delegate_registry!(WaylandShell);

impl ProvidesRegistryState for WaylandShell {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

delegate_dispatch2!(WaylandShell);
