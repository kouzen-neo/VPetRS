//! XDG path resolution, `~` expansion, default `0000_core` discovery, and single-instance `flock`.

use std::{
    env,
    fs::{self, File, OpenOptions},
    os::fd::AsRawFd,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

const LOCAL_DEV_CORE_PATH: &str =
    "/home/kouzen/Documents/VPet/VPet-Simulator.Windows/mod/0000_core";

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub config_file: PathBuf,
    pub data_dir: PathBuf,
    pub saves_dir: PathBuf,
    pub mods_dir: PathBuf,
    pub log_file: PathBuf,
    pub socket_path: PathBuf,
    pub lock_path: PathBuf,
}

impl AppPaths {
    #[must_use]
    pub fn resolve(config_override: Option<&Path>) -> Self {
        let home = env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"));

        let config_base = env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));
        let config_file = config_override
            .map(Path::to_path_buf)
            .or_else(|| env::var_os("VPET_CONFIG").map(PathBuf::from))
            .unwrap_or_else(|| config_base.join("vpet-rs").join("config.toml"));

        let data_dir = env::var_os("VPET_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                env::var_os("XDG_DATA_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| home.join(".local").join("share"))
                    .join("vpet-rs")
            });

        let runtime_dir = env::var_os("VPET_RUNTIME_DIR")
            .map(PathBuf::from)
            .or_else(|| env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from))
            .unwrap_or_else(|| {
                let uid = unsafe { libc::geteuid() };
                PathBuf::from(format!("/tmp/vpet-rs-{uid}"))
            });

        let socket_path = env::var_os("VPET_SOCKET")
            .map(PathBuf::from)
            .unwrap_or_else(|| runtime_dir.join("vpet-rs.sock"));
        let lock_path = env::var_os("VPET_LOCK")
            .map(PathBuf::from)
            .unwrap_or_else(|| runtime_dir.join("vpet-rs.lock"));

        Self {
            config_file,
            saves_dir: data_dir.join("saves"),
            mods_dir: data_dir.join("mods"),
            log_file: data_dir.join("logs").join("vpet-rs.log"),
            data_dir,
            socket_path,
            lock_path,
        }
    }

    #[must_use]
    pub fn resolve_content_root(&self, configured: &str) -> PathBuf {
        let expanded = expand_tilde(configured);
        if expanded.exists() {
            return expanded;
        }
        for candidate in [
            self.data_dir.join("content").join("0000_core"),
            PathBuf::from("/usr/share/vpet-rs/content/0000_core"),
            PathBuf::from("/usr/local/share/vpet-rs/content/0000_core"),
            PathBuf::from(LOCAL_DEV_CORE_PATH),
        ] {
            if candidate.exists() {
                return candidate;
            }
        }
        expanded
    }
}

#[must_use]
pub fn expand_tilde(raw: &str) -> PathBuf {
    if let Some(rest) = raw.strip_prefix("~/")
        && let Some(home) = env::var_os("HOME")
    {
        return PathBuf::from(home).join(rest);
    }
    if raw == "~"
        && let Some(home) = env::var_os("HOME")
    {
        return PathBuf::from(home);
    }
    PathBuf::from(raw)
}

/// Exclusive non-blocking `flock` guard preventing multiple concurrent `vpet-rs` daemons.
pub struct InstanceLock {
    _file: File,
    path: PathBuf,
}

impl InstanceLock {
    pub fn acquire(lock_path: &Path) -> Result<Self> {
        if let Some(parent) = lock_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create runtime directory `{}`", parent.display())
            })?;
        }
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(lock_path)
            .with_context(|| format!("failed to open lock file `{}`", lock_path.display()))?;

        let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if rc != 0 {
            bail!(
                "another vpet-rs instance is already running (lock held at `{}`)",
                lock_path.display()
            );
        }

        Ok(Self {
            _file: file,
            path: lock_path.to_path_buf(),
        })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enforces_single_instance_flock() {
        let lock = std::env::temp_dir().join(format!("vpet_rs_lock_{}.lock", std::process::id()));
        let guard = InstanceLock::acquire(&lock).expect("first lock succeeds");
        assert!(
            InstanceLock::acquire(&lock).is_err(),
            "second lock must fail"
        );
        drop(guard);
        assert!(
            InstanceLock::acquire(&lock).is_ok(),
            "re-acquire after drop succeeds"
        );
        let _ = fs::remove_file(&lock);
    }
}
