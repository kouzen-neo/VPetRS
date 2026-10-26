//! VPetRust entrypoint — Phase 1 (Gate 0) `wlr-layer-shell` spike on niri.

pub mod anim;
pub mod persistence;

use std::{
    env,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use calloop::{
    EventLoop,
    signals::{Signal, Signals},
    timer::{TimeoutAction, Timer},
};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;
use vpet_render::{Pixmap, load_sprite_scaled, procedural_mascot_pixmap};
use vpet_wayland::{LayerMode, SurfaceConfig, SurfaceEvent, WaylandShell};

const DEFAULT_VPET_FRAME: &str = "/home/kouzen/Documents/VPet/VPet-Simulator.Windows/mod/0000_core/pet/vup/Default/Nomal/1/_000_250.png";

struct AppState {
    shell: WaylandShell,
    walk_enabled: bool,
    walk_vx: i32,
    running: bool,
    started_at: Instant,
    exit_after: Option<Duration>,
}

fn env_bool(key: &str, default: bool) -> bool {
    match env::var(key) {
        Ok(v) => matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => default,
    }
}

fn load_initial_sprite(size: u32) -> Result<Pixmap> {
    let candidate = env::var("VPET_SPRITE").ok().map(PathBuf::from).or_else(|| {
        let p = Path::new(DEFAULT_VPET_FRAME);
        p.exists().then(|| p.to_path_buf())
    });

    if let Some(path) = candidate {
        match load_sprite_scaled(&path, size, size) {
            Ok(pm) => {
                info!(sprite = %path.display(), size, "loaded sprite PNG");
                return Ok(pm);
            }
            Err(err) => {
                warn!(%err, "failed to load sprite PNG; falling back to procedural mascot");
            }
        }
    }

    procedural_mascot_pixmap(size).context("failed to build procedural mascot sprite")
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    if env::args().any(|a| a == "--version" || a == "-V") {
        println!("vpet-rs {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let size = env::var("VPET_SIZE")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(250)
        .clamp(32, 1000);

    let layer = env::var("VPET_LAYER")
        .ok()
        .and_then(|v| LayerMode::parse(&v))
        .unwrap_or(LayerMode::Top);

    let preferred_output = env::var("VPET_OUTPUT").ok().filter(|s| !s.is_empty());
    let click_through = env_bool("VPET_CLICKTHROUGH", false);
    let walk_enabled = env_bool("VPET_WALK", true);
    let exit_after = env::var("VPET_EXIT_AFTER_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map(Duration::from_millis);

    let sprite = load_initial_sprite(size)?;

    let mut event_loop: EventLoop<'static, AppState> =
        EventLoop::try_new().context("failed to initialize calloop event loop")?;
    let loop_handle = event_loop.handle();

    let surface_cfg = SurfaceConfig {
        namespace: "vpet-rs".to_owned(),
        preferred_output,
        layer,
        click_through,
        visible: true,
        initial_position: (80, 760),
    };

    let shell =
        WaylandShell::connect_and_insert(surface_cfg, sprite, &loop_handle, |s| &mut s.shell)
            .context("failed to initialize Wayland layer-shell")?;

    info!(
        output = %shell.active_output_name(),
        output_size = ?shell.output_logical_size(),
        scale = shell.scale_factor(),
        layer = %shell.layer_mode().as_str(),
        click_through = shell.click_through(),
        input_rects = shell.last_input_region_rects(),
        walk_enabled,
        "Gate 0 layer-shell ready"
    );

    let signals = Signals::new(&[
        Signal::SIGUSR1,
        Signal::SIGUSR2,
        Signal::SIGINT,
        Signal::SIGTERM,
    ])
    .context("failed to register UNIX signal source")?;
    loop_handle
        .insert_source(signals, |event, (), state| match event.signal() {
            Signal::SIGUSR1 => {
                let enabled = state.shell.toggle_click_through();
                state.shell.present();
                info!(
                    click_through = enabled,
                    input_rects = state.shell.last_input_region_rects(),
                    "SIGUSR1 toggled click-through"
                );
            }
            Signal::SIGUSR2 => {
                let next = match state.shell.layer_mode() {
                    LayerMode::Top => LayerMode::Overlay,
                    LayerMode::Overlay => LayerMode::Top,
                };
                state.shell.set_layer_mode(next);
                state.shell.present();
                info!(layer = %next.as_str(), "SIGUSR2 switched layer");
            }
            Signal::SIGINT | Signal::SIGTERM => {
                info!("received termination signal; shutting down");
                state.running = false;
            }
            _ => {}
        })
        .map_err(|e| anyhow::anyhow!("failed to insert signal source: {e}"))?;

    let tick_interval = Duration::from_millis(33); // ~30 FPS
    loop_handle
        .insert_source(Timer::from_duration(tick_interval), move |_, (), state| {
            if let Some(limit) = state.exit_after
                && state.started_at.elapsed() >= limit
            {
                info!(
                    frames_committed = state.shell.frames_committed(),
                    input_rects = state.shell.last_input_region_rects(),
                    pos = ?state.shell.position(),
                    "exit timer elapsed"
                );
                state.running = false;
                return TimeoutAction::Drop;
            }

            for ev in state.shell.drain_events() {
                if let SurfaceEvent::Clicked { x, y, button } = ev {
                    info!(x, y, button, "pointer click on pet");
                }
            }

            if state.walk_enabled && !state.shell.is_dragging() {
                let (mut x, y) = state.shell.position();
                let (out_w, _) = state.shell.output_logical_size();
                let (sw, _) = state.shell.sprite_logical_size();
                let max_x = (out_w as i32 - sw as i32).max(0);
                x += state.walk_vx;
                if x <= 0 {
                    x = 0;
                    state.walk_vx = state.walk_vx.abs();
                } else if x >= max_x {
                    x = max_x;
                    state.walk_vx = -state.walk_vx.abs();
                }
                state.shell.set_position(x, y);
                state.shell.present();
            }

            TimeoutAction::ToDuration(tick_interval)
        })
        .map_err(|e| anyhow::anyhow!("failed to insert 30fps timer: {e}"))?;

    let mut state = AppState {
        shell,
        walk_enabled,
        walk_vx: 3,
        running: true,
        started_at: Instant::now(),
        exit_after,
    };

    while state.running && !state.shell.is_closed() {
        event_loop
            .dispatch(None, &mut state)
            .context("event loop dispatch error")?;
    }

    Ok(())
}
