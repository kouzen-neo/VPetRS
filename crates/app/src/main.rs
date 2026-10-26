//! VPetRust CLI and Wayland layer-shell desktop pet entrypoint.

pub mod anim;
pub mod app_loop;
pub mod config;
pub mod ipc;
pub mod logging;
pub mod paths;
pub mod persistence;
pub mod tray;

use std::{fs, path::PathBuf, process::ExitCode, time::Duration};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use vpet_wayland::LayerMode;

use crate::{
    app_loop::{RunOverrides, run_daemon},
    config::AppConfig,
    ipc::{IpcRequest, send_ipc_request},
    logging::init_logging,
    paths::AppPaths,
};

#[derive(Debug, Parser)]
#[command(
    name = "vpet-rs",
    version,
    about = "Niri-first Wayland desktop pet written in 100% Rust"
)]
struct Cli {
    /// Custom path to `config.toml`
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    /// Override layer (`top` or `overlay`)
    #[arg(long)]
    layer: Option<String>,

    /// Start in click-through mode
    #[arg(long)]
    click_through: bool,

    /// Preferred Wayland output name (`auto`, `eDP-1`, ...)
    #[arg(long)]
    output: Option<String>,

    /// Disable StatusNotifierItem tray registration
    #[arg(long)]
    no_tray: bool,

    /// Automatically exit after N milliseconds (integration testing)
    #[arg(long, hide = true)]
    exit_after_ms: Option<u64>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Send a control command to the running `vpet-rs` daemon over Unix socket IPC
    Ctl {
        #[command(subcommand)]
        action: CtlAction,
    },
    /// Write default `config.toml` if not already present
    InitConfig {
        /// Overwrite existing config file
        #[arg(long)]
        force: bool,
    },
}

#[derive(Debug, Subcommand)]
enum CtlAction {
    /// Show the pet surface
    Show,
    /// Hide the pet surface and clear input region
    Hide,
    /// Toggle mouse click-through (empty `wl_region`)
    ToggleClickthrough,
    /// Switch Wayland layer (`top` or `overlay`)
    Layer { mode: String },
    /// Feed an item by name (or `snack` for an affordable default snack)
    Feed {
        #[arg(default_value = "snack")]
        item: String,
    },
    /// Put the pet to sleep
    Sleep,
    /// Wake the pet up
    Wake,
    /// Display a speech bubble above the pet
    Say { text: String },
    /// Reload `config.toml` without restarting
    Reload,
    /// Print daemon & pet status JSON
    Status,
    /// Save state and stop the running daemon
    Quit,
}

fn main() -> ExitCode {
    match try_main() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn try_main() -> Result<()> {
    let cli = Cli::parse();
    let paths = AppPaths::resolve(cli.config.as_deref());

    match cli.command {
        Some(Commands::InitConfig { force }) => {
            if paths.config_file.exists() && !force {
                anyhow::bail!(
                    "config already exists at `{}` (pass --force to overwrite)",
                    paths.config_file.display()
                );
            }
            if let Some(parent) = paths.config_file.parent() {
                fs::create_dir_all(parent)?;
            }
            let text = AppConfig::default().to_pretty_toml()?;
            fs::write(&paths.config_file, text)
                .with_context(|| format!("failed to write `{}`", paths.config_file.display()))?;
            println!("{}", paths.config_file.display());
            Ok(())
        }
        Some(Commands::Ctl { action }) => {
            let req = match action {
                CtlAction::Show => IpcRequest::Show,
                CtlAction::Hide => IpcRequest::Hide,
                CtlAction::ToggleClickthrough => IpcRequest::ToggleClickthrough,
                CtlAction::Layer { mode } => IpcRequest::Layer { mode },
                CtlAction::Feed { item } => IpcRequest::Feed { item },
                CtlAction::Sleep => IpcRequest::Sleep,
                CtlAction::Wake => IpcRequest::Wake,
                CtlAction::Say { text } => IpcRequest::Say { text },
                CtlAction::Reload => IpcRequest::Reload,
                CtlAction::Status => IpcRequest::Status,
                CtlAction::Quit => IpcRequest::Quit,
            };
            let resp = send_ipc_request(&paths.socket_path, &req)?;
            println!("{}", serde_json::to_string_pretty(&resp)?);
            if !resp.ok {
                anyhow::bail!(
                    "{}",
                    resp.error
                        .unwrap_or_else(|| "IPC command failed".to_owned())
                );
            }
            Ok(())
        }
        None => {
            let cfg = AppConfig::load_or_default(&paths.config_file);
            init_logging(&cfg.app.log_level, Some(&paths.log_file));

            let layer = match cli.layer {
                Some(raw) => Some(
                    LayerMode::parse(&raw)
                        .ok_or_else(|| anyhow::anyhow!("--layer must be `top` or `overlay`"))?,
                ),
                None => None,
            };
            let overrides = RunOverrides {
                layer,
                click_through: cli.click_through.then_some(true),
                output: cli.output,
                exit_after: cli.exit_after_ms.map(Duration::from_millis),
                disable_tray: cli.no_tray,
            };
            run_daemon(paths, cfg, overrides)
        }
    }
}
