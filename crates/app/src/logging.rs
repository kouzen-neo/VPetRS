//! Structured `tracing` setup with stderr + size-capped rolling log file (`<= 2 MiB`).

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::Path,
    sync::Mutex,
};

use tracing_subscriber::{EnvFilter, fmt::MakeWriter};

const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;

struct TeeWriter {
    file: Option<Mutex<File>>,
}

struct TeeGuard<'a> {
    file: Option<&'a Mutex<File>>,
}

impl Write for TeeGuard<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let _ = io::stderr().write_all(buf);
        if let Some(m) = self.file
            && let Ok(mut f) = m.lock()
        {
            let _ = f.write_all(buf);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let _ = io::stderr().flush();
        if let Some(m) = self.file
            && let Ok(mut f) = m.lock()
        {
            let _ = f.flush();
        }
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for TeeWriter {
    type Writer = TeeGuard<'a>;

    fn make_writer(&'a self) -> Self::Writer {
        TeeGuard {
            file: self.file.as_ref(),
        }
    }
}

pub fn init_logging(default_level: &str, log_file: Option<&Path>) {
    let file_mutex = log_file.and_then(open_capped_log).map(Mutex::new);
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(TeeWriter { file: file_mutex })
        .try_init();
}

fn open_capped_log(path: &Path) -> Option<File> {
    let parent = path.parent()?;
    fs::create_dir_all(parent).ok()?;
    if let Ok(meta) = fs::metadata(path)
        && meta.len() > MAX_LOG_BYTES
    {
        let rotated = path.with_extension("log.1");
        let _ = fs::rename(path, rotated);
    }
    OpenOptions::new().create(true).append(true).open(path).ok()
}
