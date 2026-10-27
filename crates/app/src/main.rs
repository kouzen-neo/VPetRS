//! VPetRust CLI and Wayland layer-shell desktop pet entrypoint.

pub mod ai;
pub mod anim;
pub mod app_loop;
pub mod config;
pub mod import;
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
    /// Link existing VPet assets or download 0000_core via git sparse-checkout
    SetupAssets {
        /// Source path to an existing 0000_core folder (e.g. from Steam or local clone)
        #[arg(long)]
        from: Option<PathBuf>,

        /// Destination path (default: ~/.local/share/vpet-rs/content/0000_core)
        #[arg(long)]
        dest: Option<PathBuf>,
    },
    /// Import an existing legacy VPet Save.lps into VPetRS
    ImportSave {
        /// Path to the legacy Save.lps file
        path: PathBuf,

        /// Target profile name
        #[arg(long, default_value = "default")]
        profile: String,
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
    /// Toggle automatic feeding and hydration when low
    ToggleSelfcare,
    /// Toggle automatic work selection when pet is well-fed
    ToggleAutowork,
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
    /// Chat with the pet via AI LLM (Ollama, LM Studio, DeepSeek, OpenAI)
    Chat { prompt: String },
    /// Open a desktop input prompter (fuzzel, zenity, or kdialog) to chat with the pet
    PromptChat,
    /// List available work, study, and play tasks
    Works,
    /// Start a work, study, or play task by name (or first available if omitted)
    Work {
        #[arg(default_value = "")]
        name: String,
    },
    /// Cancel the active work session
    CancelWork,
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
        Some(Commands::SetupAssets { from, dest }) => {
            let target = dest.unwrap_or_else(|| paths.data_dir.join("content").join("0000_core"));
            setup_assets_command(from.as_deref(), &target)
        }
        Some(Commands::ImportSave { path, profile }) => {
            let world = crate::import::import_legacy_vpet_save(&path, &profile)?;
            let store = crate::persistence::SaveStore::new(&paths.saves_dir);
            let saved = store.save_world(&world)?;
            println!(
                "Successfully imported `{}` into profile `{}` -> `{}`",
                path.display(),
                profile,
                saved.display()
            );
            Ok(())
        }
        Some(Commands::Ctl { action }) => {
            let req = match action {
                CtlAction::Show => IpcRequest::Show,
                CtlAction::Hide => IpcRequest::Hide,
                CtlAction::ToggleClickthrough => IpcRequest::ToggleClickthrough,
                CtlAction::ToggleSelfcare => IpcRequest::ToggleSelfCare,
                CtlAction::ToggleAutowork => IpcRequest::ToggleAutoWork,
                CtlAction::Layer { mode } => IpcRequest::Layer { mode },
                CtlAction::Feed { item } => IpcRequest::Feed { item },
                CtlAction::Sleep => IpcRequest::Sleep,
                CtlAction::Wake => IpcRequest::Wake,
                CtlAction::Say { text } => IpcRequest::Say { text },
                CtlAction::Chat { prompt } => IpcRequest::Chat { prompt },
                CtlAction::PromptChat => IpcRequest::PromptInput,
                CtlAction::Works => IpcRequest::ListWorks,
                CtlAction::Work { name } => {
                    let opt = if name.trim().is_empty() {
                        None
                    } else {
                        Some(name)
                    };
                    IpcRequest::Work { name: opt }
                }
                CtlAction::CancelWork => IpcRequest::CancelWork,
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

fn setup_assets_command(from: Option<&std::path::Path>, dest: &std::path::Path) -> Result<()> {
    if dest.join("info.lps").is_file() {
        println!("VPet assets already present at `{}`", dest.display());
        return Ok(());
    }

    if let Some(src) = from {
        if !src.join("info.lps").is_file() {
            anyhow::bail!(
                "source directory `{}` does not contain VPet `info.lps`",
                src.display()
            );
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        std::os::unix::fs::symlink(src, dest)
            .or_else(|_| copy_dir_all(src, dest))
            .with_context(|| {
                format!(
                    "failed to link/copy `{}` to `{}`",
                    src.display(),
                    dest.display()
                )
            })?;
        println!(
            "Linked VPet assets from `{}` -> `{}`",
            src.display(),
            dest.display()
        );
        return Ok(());
    }

    // Auto-detect local Steam installations
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    for steam_dir in [
        home.join(".local/share/Steam/steamapps/common/VPet/mod/0000_core"),
        home.join(".steam/steam/steamapps/common/VPet/mod/0000_core"),
        home.join(".steam/root/steamapps/common/VPet/mod/0000_core"),
    ] {
        if steam_dir.join("info.lps").is_file() {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            std::os::unix::fs::symlink(&steam_dir, dest).with_context(|| {
                format!(
                    "failed to symlink Steam assets from `{}`",
                    steam_dir.display()
                )
            })?;
            println!(
                "Found VPet on Steam! Linked `{}` -> `{}`",
                steam_dir.display(),
                dest.display()
            );
            return Ok(());
        }
    }

    // If not found locally, fetch 0000_core via git sparse-checkout
    println!(
        "No existing VPet installation found. Downloading 0000_core via git sparse-checkout..."
    );
    let tmp = tempfile::Builder::new().prefix("vpet-assets-").tempdir()?;
    let status = std::process::Command::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            "--filter=blob:none",
            "--sparse",
            "https://github.com/LorisYounger/VPet.git",
        ])
        .arg(tmp.path())
        .status()
        .context("failed to execute git clone")?;
    if !status.success() {
        anyhow::bail!("git clone failed with status {status}");
    }

    let status = std::process::Command::new("git")
        .current_dir(tmp.path())
        .args([
            "sparse-checkout",
            "set",
            "VPet-Simulator.Windows/mod/0000_core",
        ])
        .status()
        .context("failed to execute git sparse-checkout")?;
    if !status.success() {
        anyhow::bail!("git sparse-checkout failed with status {status}");
    }

    let downloaded = tmp.path().join("VPet-Simulator.Windows/mod/0000_core");
    if !downloaded.join("info.lps").is_file() {
        anyhow::bail!("downloaded sparse tree missing 0000_core/info.lps");
    }

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(&downloaded, dest).or_else(|_| copy_dir_all(&downloaded, dest))?;
    println!("Successfully installed VPet assets to `{}`", dest.display());
    Ok(())
}

fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}
