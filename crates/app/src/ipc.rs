//! Unix domain socket IPC server (`0600` + `SO_PEERCRED` peer UID verification) and client.

use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::PermissionsExt,
            net::{UnixListener, UnixStream},
        },
    },
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use tracing::warn;

const MAX_IPC_LINE_BYTES: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "kebab-case")]
pub enum IpcRequest {
    Show,
    Hide,
    ToggleClickthrough,
    Layer { mode: String },
    Feed { item: String },
    Sleep,
    Wake,
    Say { text: String },
    Chat { prompt: String },
    PromptInput,
    Work { name: Option<String> },
    CancelWork,
    Reload,
    Status,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatusData {
    pub name: String,
    pub level: u32,
    pub exp: f64,
    pub money: f64,
    pub strength: f64,
    pub food: f64,
    pub drink: f64,
    pub feeling: f64,
    pub health: f64,
    pub likability: f64,
    pub mode: String,
    pub sleeping: bool,
    pub active_work: Option<String>,
    pub ai_enabled: bool,
    pub ai_model: String,
    pub visible: bool,
    pub click_through: bool,
    pub layer: String,
    pub output: String,
    pub position: [i32; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IpcResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl IpcResponse {
    #[must_use]
    pub fn success(data: impl Serialize) -> Self {
        Self {
            ok: true,
            data: serde_json::to_value(data).ok(),
            error: None,
        }
    }

    #[must_use]
    pub fn fail(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(msg.into()),
        }
    }
}

pub struct IpcServer {
    listener: UnixListener,
    path: PathBuf,
    expected_uid: u32,
}

impl IpcServer {
    /// Bind non-blocking Unix socket with strict `0600` permissions.
    /// Must only be called AFTER acquiring `InstanceLock`.
    pub fn bind(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        if path.exists() {
            let _ = fs::remove_file(path);
        }

        let listener = UnixListener::bind(path)
            .with_context(|| format!("failed to bind IPC socket `{}`", path.display()))?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .with_context(|| format!("failed to chmod 0600 `{}`", path.display()))?;
        listener.set_nonblocking(true)?;

        let expected_uid = unsafe { libc::geteuid() };
        Ok(Self {
            listener,
            path: path.to_path_buf(),
            expected_uid,
        })
    }

    #[must_use]
    pub fn raw_fd(&self) -> std::os::fd::BorrowedFd<'_> {
        use std::os::fd::AsFd;
        self.listener.as_fd()
    }

    /// Accept all pending connections without blocking and invoke `handler` for verified peers.
    pub fn accept_pending<F>(&self, mut handler: F)
    where
        F: FnMut(IpcRequest) -> IpcResponse,
    {
        loop {
            match self.listener.accept() {
                Ok((stream, _addr)) => {
                    if let Err(err) = self.handle_client(stream, &mut handler) {
                        warn!(%err, "IPC client handling failed");
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(err) => {
                    warn!(%err, "IPC accept error");
                    break;
                }
            }
        }
    }

    fn handle_client<F>(&self, stream: UnixStream, handler: &mut F) -> Result<()>
    where
        F: FnMut(IpcRequest) -> IpcResponse,
    {
        let peer_uid = peer_cred_uid(&stream)?;
        if peer_uid != self.expected_uid {
            bail!(
                "rejected IPC peer with uid {peer_uid} (expected {})",
                self.expected_uid
            );
        }

        stream.set_nonblocking(false)?;
        stream.set_read_timeout(Some(Duration::from_millis(500)))?;
        stream.set_write_timeout(Some(Duration::from_millis(500)))?;

        let mut reader = BufReader::new(&stream);
        let mut line = String::new();
        let bytes = read_bounded_line(&mut reader, &mut line, MAX_IPC_LINE_BYTES)?;
        if bytes == 0 {
            return Ok(());
        }

        let resp = match serde_json::from_str::<IpcRequest>(line.trim()) {
            Ok(req) => validate_request(req).map_or_else(IpcResponse::fail, handler),
            Err(err) => IpcResponse::fail(format!("invalid request JSON: {err}")),
        };

        let mut out = serde_json::to_vec(&resp)?;
        out.push(b'\n');
        (&stream).write_all(&out)?;
        (&stream).flush()?;
        Ok(())
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub fn send_ipc_request(socket_path: &Path, req: &IpcRequest) -> Result<IpcResponse> {
    let stream = UnixStream::connect(socket_path).with_context(|| {
        format!(
            "cannot connect to `{}` (is vpet-rs running?)",
            socket_path.display()
        )
    })?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;

    let mut payload = serde_json::to_vec(req)?;
    payload.push(b'\n');
    (&stream).write_all(&payload)?;
    (&stream).flush()?;

    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let resp: IpcResponse =
        serde_json::from_str(line.trim()).context("malformed JSON response from vpet-rs")?;
    Ok(resp)
}

fn validate_request(req: IpcRequest) -> std::result::Result<IpcRequest, String> {
    match &req {
        IpcRequest::Feed { item }
            if item.trim().is_empty() || item.contains('/') || item.contains('\0') =>
        {
            return Err("invalid item identifier".to_owned());
        }
        IpcRequest::Say { text } if text.trim().is_empty() || text.len() > 512 => {
            return Err("speech bubble text must be 1..=512 bytes".to_owned());
        }
        IpcRequest::Chat { prompt } if prompt.trim().is_empty() || prompt.len() > 2048 => {
            return Err("chat prompt must be 1..=2048 bytes".to_owned());
        }
        IpcRequest::Layer { mode }
            if !mode.eq_ignore_ascii_case("top") && !mode.eq_ignore_ascii_case("overlay") =>
        {
            return Err("layer mode must be `top` or `overlay`".to_owned());
        }
        IpcRequest::Work { name: Some(name) }
            if name.trim().is_empty() || name.contains('/') || name.contains('\0') =>
        {
            return Err("invalid work task name".to_owned());
        }
        _ => {}
    }
    Ok(req)
}

fn read_bounded_line<R: BufRead>(
    reader: &mut R,
    buf: &mut String,
    max_bytes: usize,
) -> Result<usize> {
    let mut raw = Vec::new();
    let mut total = 0_usize;
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            break;
        }
        if let Some(nl_pos) = available.iter().position(|&b| b == b'\n') {
            let take = nl_pos + 1;
            if total + take > max_bytes {
                bail!("IPC line exceeded {max_bytes} byte limit");
            }
            raw.extend_from_slice(&available[..take]);
            reader.consume(take);
            total += take;
            break;
        }
        let len = available.len();
        if total + len > max_bytes {
            bail!("IPC line exceeded {max_bytes} byte limit");
        }
        raw.extend_from_slice(available);
        reader.consume(len);
        total += len;
    }
    *buf = String::from_utf8(raw).context("IPC request is not valid UTF-8")?;
    Ok(total)
}

fn peer_cred_uid(stream: &UnixStream) -> Result<u32> {
    let mut cred = libc::ucred {
        pid: 0,
        uid: u32::MAX,
        gid: u32::MAX,
    };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let rc = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&raw mut cred).cast::<libc::c_void>(),
            &raw mut len,
        )
    };
    if rc != 0 {
        bail!(
            "getsockopt(SO_PEERCRED) failed: {}",
            std::io::Error::last_os_error()
        );
    }
    Ok(cred.uid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipc_roundtrips_json_over_0600_socket_with_peercred() {
        let sock = std::env::temp_dir().join(format!("vpet_rs_ipc_{}.sock", std::process::id()));
        let server = IpcServer::bind(&sock).unwrap();

        let mode = fs::metadata(&sock).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);

        let sock_client = sock.clone();
        let client_thread = std::thread::spawn(move || {
            send_ipc_request(&sock_client, &IpcRequest::ToggleClickthrough).unwrap()
        });

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        let mut handled = false;
        while !handled && std::time::Instant::now() < deadline {
            server.accept_pending(|req| {
                handled = true;
                assert_eq!(req, IpcRequest::ToggleClickthrough);
                IpcResponse::success(serde_json::json!({ "click_through": true }))
            });
            std::thread::sleep(Duration::from_millis(5));
        }

        let resp = client_thread.join().unwrap();
        assert!(resp.ok);
        assert_eq!(resp.data.unwrap()["click_through"], true);
    }

    #[test]
    fn rejects_path_traversal_in_feed_item() {
        assert!(
            validate_request(IpcRequest::Feed {
                item: "../etc/passwd".to_owned()
            })
            .is_err()
        );
    }
}
