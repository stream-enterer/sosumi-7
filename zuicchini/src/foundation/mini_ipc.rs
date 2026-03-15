use std::fmt;

/// Errors from MiniIpc operations.
#[derive(Debug)]
pub enum MiniIpcError {
    /// An I/O error occurred.
    IoError(std::io::Error),
    /// A nix/POSIX error occurred.
    #[cfg(target_os = "linux")]
    NixError(nix::Error),
    /// No server is listening on the specified name.
    ServerNotFound,
    /// Message encoding/decoding error.
    EncodingError(String),
    /// This platform is not supported.
    UnsupportedPlatform,
}

impl fmt::Display for MiniIpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IoError(e) => write!(f, "I/O error: {e}"),
            #[cfg(target_os = "linux")]
            Self::NixError(e) => write!(f, "system error: {e}"),
            Self::ServerNotFound => write!(f, "no server found"),
            Self::EncodingError(msg) => write!(f, "encoding error: {msg}"),
            Self::UnsupportedPlatform => write!(f, "unsupported platform"),
        }
    }
}

impl std::error::Error for MiniIpcError {}

impl From<std::io::Error> for MiniIpcError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}

#[cfg(target_os = "linux")]
impl From<nix::Error> for MiniIpcError {
    fn from(e: nix::Error) -> Self {
        Self::NixError(e)
    }
}

// ── Wire format ─────────────────────────────────────────────────────

/// Encode a message as the C++ emMiniIpc wire format:
/// ASCII argc string + null byte, then null-terminated argv strings.
pub fn encode_message(args: &[&str]) -> Vec<u8> {
    let argc_str = args.len().to_string();
    let mut buf =
        Vec::with_capacity(argc_str.len() + 1 + args.iter().map(|a| a.len() + 1).sum::<usize>());
    buf.extend_from_slice(argc_str.as_bytes());
    buf.push(0);
    for arg in args {
        buf.extend_from_slice(arg.as_bytes());
        buf.push(0);
    }
    buf
}

/// Decode one complete message from the buffer. Returns `Some((args, bytes_consumed))`
/// if a complete message was found, or `None` if the buffer is incomplete.
pub fn decode_message(buf: &[u8]) -> Option<(Vec<String>, usize)> {
    let argc_end = buf.iter().position(|&b| b == 0)?;
    let argc_str = std::str::from_utf8(&buf[..argc_end]).ok()?;
    let argc: usize = argc_str.parse().ok()?;

    let mut pos = argc_end + 1;
    let mut args = Vec::with_capacity(argc);

    for _ in 0..argc {
        if pos >= buf.len() {
            return None;
        }
        let arg_end = buf[pos..].iter().position(|&b| b == 0)?;
        let arg = std::str::from_utf8(&buf[pos..pos + arg_end]).ok()?;
        args.push(arg.to_string());
        pos += arg_end + 1;
    }

    Some((args, pos))
}

type MessageCallback = Box<dyn FnMut(&[String])>;

// ── Platform-specific implementation ────────────────────────────────

#[cfg(target_os = "linux")]
mod platform {
    use std::cell::RefCell;
    use std::io::{Read, Write};
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::fs::FileTypeExt;
    use std::path::{Path, PathBuf};
    use std::rc::Rc;

    use nix::fcntl::OFlag;
    use nix::sys::stat::Mode;

    use super::{decode_message, encode_message, MiniIpcError};
    use crate::scheduler::{
        Engine, EngineCtx, EngineId, EngineScheduler, Priority, SignalId, TimerId,
    };

    use super::MessageCallback;

    // ── Path helpers ────────────────────────────────────────────────

    fn ipc_dir() -> PathBuf {
        let uid = unsafe { libc::getuid() };
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home).join(format!(".emMiniIpc-{uid}"))
    }

    fn hostname() -> String {
        std::fs::read_to_string("/etc/hostname")
            .unwrap_or_default()
            .trim()
            .to_string()
    }

    fn calc_fifo_hash(server_name: &str) -> String {
        let host = hostname();
        let username = std::env::var("USER").unwrap_or_default();

        let mut data = Vec::new();
        data.extend_from_slice(host.as_bytes());
        data.push(0);
        data.extend_from_slice(username.as_bytes());
        data.push(0);
        data.extend_from_slice(server_name.as_bytes());

        let hash = crate::foundation::calc_hash_code(&data);
        format!("{hash:08x}")
    }

    fn fifo_path(server_name: &str) -> PathBuf {
        let hash = calc_fifo_hash(server_name);
        ipc_dir().join(format!("{hash}.f.autoremoved"))
    }

    fn lock_path(server_name: &str) -> PathBuf {
        let hash = calc_fifo_hash(server_name);
        ipc_dir().join(format!("{hash}.l.autoremoved"))
    }

    fn creation_lock_path() -> PathBuf {
        ipc_dir().join("fifo-creation.lock")
    }

    // ── File locking via fcntl F_SETLKW ─────────────────────────────

    fn acquire_write_lock(path: &Path) -> Result<OwnedFd, MiniIpcError> {
        let fd = nix::fcntl::open(
            path,
            OFlag::O_WRONLY | OFlag::O_CREAT,
            Mode::S_IRUSR | Mode::S_IWUSR,
        )?;

        let flock = libc::flock {
            l_type: libc::F_WRLCK as _,
            l_whence: libc::SEEK_SET as _,
            l_start: 0,
            l_len: 0,
            l_pid: 0,
        };

        nix::fcntl::fcntl(&fd, nix::fcntl::FcntlArg::F_SETLKW(&flock))?;

        Ok(fd)
    }

    // ── Orphan cleanup ──────────────────────────────────────────────

    fn cleanup_orphans() {
        let dir = ipc_dir();
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();

            if name.ends_with(".f.autoremoved") {
                let path = entry.path();
                let meta = match std::fs::metadata(&path) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                if !meta.file_type().is_fifo() {
                    continue;
                }

                match nix::fcntl::open(&path, OFlag::O_WRONLY | OFlag::O_NONBLOCK, Mode::empty()) {
                    Ok(_fd) => {
                        // Server alive, fd drops and closes automatically
                    }
                    Err(nix::Error::ENXIO) => {
                        let _ = std::fs::remove_file(&path);
                        let lock = path
                            .to_string_lossy()
                            .replace(".f.autoremoved", ".l.autoremoved");
                        let _ = std::fs::remove_file(lock);
                    }
                    Err(_) => {}
                }
            }
        }
    }

    // ── Client ──────────────────────────────────────────────────────

    /// Client for sending one-shot messages to a MiniIpc server.
    ///
    /// Port of C++ `emMiniIpcClient`.
    pub struct MiniIpcClient;

    impl MiniIpcClient {
        /// Send a message to the named server.
        pub fn try_send(server_name: &str, args: &[&str]) -> Result<(), MiniIpcError> {
            let fifo = fifo_path(server_name);
            let lock = lock_path(server_name);

            // Open FIFO for writing (non-blocking to detect if server exists)
            let write_fd =
                match nix::fcntl::open(&fifo, OFlag::O_WRONLY | OFlag::O_NONBLOCK, Mode::empty()) {
                    Ok(fd) => fd,
                    Err(nix::Error::ENXIO | nix::Error::ENOENT) => {
                        return Err(MiniIpcError::ServerNotFound);
                    }
                    Err(e) => return Err(e.into()),
                };

            // Remove O_NONBLOCK so the write blocks if needed
            let flags = nix::fcntl::fcntl(&write_fd, nix::fcntl::FcntlArg::F_GETFL)?;
            let flags = OFlag::from_bits_truncate(flags);
            nix::fcntl::fcntl(
                &write_fd,
                nix::fcntl::FcntlArg::F_SETFL(flags & !OFlag::O_NONBLOCK),
            )?;

            // Acquire write lock to serialize with other clients
            let _lock_fd = acquire_write_lock(&lock)?;

            // Write the encoded message via std::fs::File
            let data = encode_message(args);
            let mut file = unsafe { std::fs::File::from_raw_fd(write_fd.as_raw_fd()) };
            let result = file.write_all(&data);
            // Prevent double-close (OwnedFd and File both own the fd)
            std::mem::forget(file);
            result?;

            Ok(())
        }
    }

    // ── Server inner state ──────────────────────────────────────────

    pub(crate) struct MiniIpcServerInner {
        fifo_fd: Option<OwnedFd>,
        buffer: Vec<u8>,
        callback: MessageCallback,
        server_name: String,
        serving: bool,
    }

    impl MiniIpcServerInner {
        fn poll(&mut self) {
            let Some(ref fifo_fd) = self.fifo_fd else {
                return;
            };

            let mut tmp = [0u8; 4096];
            let mut file = unsafe { std::fs::File::from_raw_fd(fifo_fd.as_raw_fd()) };
            loop {
                match file.read(&mut tmp) {
                    Ok(0) => break,
                    Ok(n) => self.buffer.extend_from_slice(&tmp[..n]),
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
            std::mem::forget(file);

            while !self.buffer.is_empty() {
                match decode_message(&self.buffer) {
                    Some((args, consumed)) => {
                        self.buffer.drain(..consumed);
                        (self.callback)(&args);
                        if !self.serving {
                            return;
                        }
                    }
                    None => break,
                }
            }
        }
    }

    // ── Server engine ───────────────────────────────────────────────

    struct MiniIpcEngine {
        inner: Rc<RefCell<MiniIpcServerInner>>,
        timer_signal: SignalId,
    }

    impl Engine for MiniIpcEngine {
        fn cycle(&mut self, ctx: &mut EngineCtx<'_>) -> bool {
            if ctx.is_signaled(self.timer_signal) {
                self.inner.borrow_mut().poll();
            }
            false
        }
    }

    // ── Server ──────────────────────────────────────────────────────

    /// FIFO-based IPC server that polls for incoming messages.
    ///
    /// Port of C++ `emMiniIpcServer`. Creates a FIFO, registers a polling
    /// engine with a 200ms timer, and invokes a callback for each received
    /// message.
    pub struct MiniIpcServer {
        inner: Rc<RefCell<MiniIpcServerInner>>,
        engine_id: EngineId,
        timer_id: TimerId,
        timer_signal: SignalId,
    }

    impl MiniIpcServer {
        /// Create a new server. Does not start serving yet.
        pub fn new(scheduler: &mut EngineScheduler, callback: MessageCallback) -> Self {
            let timer_signal = scheduler.create_signal();
            let timer_id = scheduler.create_timer(timer_signal);

            let inner = Rc::new(RefCell::new(MiniIpcServerInner {
                fifo_fd: None,
                buffer: Vec::new(),
                callback,
                server_name: String::new(),
                serving: false,
            }));

            let engine = MiniIpcEngine {
                inner: Rc::clone(&inner),
                timer_signal,
            };
            let engine_id = scheduler.register_engine(Priority::Low, Box::new(engine));
            scheduler.connect(timer_signal, engine_id);

            Self {
                inner,
                engine_id,
                timer_id,
                timer_signal,
            }
        }

        /// Start serving on the given name (or generate one).
        pub fn start_serving(
            &mut self,
            scheduler: &mut EngineScheduler,
            name: Option<&str>,
        ) -> Result<(), MiniIpcError> {
            if self.inner.borrow().serving {
                return Ok(());
            }

            let server_name = name
                .map(|n| n.to_string())
                .unwrap_or_else(|| format!("zuicchini-{}", std::process::id()));

            let dir = ipc_dir();
            std::fs::create_dir_all(&dir)?;

            cleanup_orphans();

            let fifo = fifo_path(&server_name);
            let _creation_lock = acquire_write_lock(&creation_lock_path())?;

            let fifo_fd = open_server_fifo(&fifo)?;

            let mut inner = self.inner.borrow_mut();
            inner.fifo_fd = Some(fifo_fd);
            inner.server_name = server_name;
            inner.serving = true;
            drop(inner);

            scheduler.start_timer(self.timer_id, 200, true);

            Ok(())
        }

        /// Stop serving and clean up.
        pub fn stop_serving(&mut self, scheduler: &mut EngineScheduler) {
            let mut inner = self.inner.borrow_mut();
            if !inner.serving {
                return;
            }

            scheduler.cancel_timer(self.timer_id, true);

            let server_name = inner.server_name.clone();
            inner.fifo_fd = None;
            inner.buffer.clear();
            inner.serving = false;
            drop(inner);

            let _ = std::fs::remove_file(fifo_path(&server_name));
            let _ = std::fs::remove_file(lock_path(&server_name));
        }

        pub fn is_serving(&self) -> bool {
            self.inner.borrow().serving
        }

        pub fn server_name(&self) -> String {
            self.inner.borrow().server_name.clone()
        }

        /// Remove engine/timer from scheduler. Call before dropping.
        pub fn cleanup(&mut self, scheduler: &mut EngineScheduler) {
            if self.is_serving() {
                self.stop_serving(scheduler);
            }
            scheduler.disconnect(self.timer_signal, self.engine_id);
            scheduler.remove_engine(self.engine_id);
            scheduler.remove_timer(self.timer_id);
            scheduler.remove_signal(self.timer_signal);
        }
    }

    fn open_server_fifo(fifo: &Path) -> Result<OwnedFd, MiniIpcError> {
        if fifo.exists() {
            match nix::fcntl::open(fifo, OFlag::O_WRONLY | OFlag::O_NONBLOCK, Mode::empty()) {
                Ok(_fd) => {
                    return Err(MiniIpcError::IoError(std::io::Error::new(
                        std::io::ErrorKind::AddrInUse,
                        "another server already owns this FIFO",
                    )));
                }
                Err(nix::Error::ENXIO) | Err(_) => {
                    let _ = std::fs::remove_file(fifo);
                }
            }
        }

        match nix::unistd::mkfifo(fifo, Mode::S_IRUSR | Mode::S_IWUSR) {
            Ok(()) => {}
            Err(nix::Error::EEXIST) => {
                return Err(MiniIpcError::IoError(std::io::Error::new(
                    std::io::ErrorKind::AddrInUse,
                    "another server created this FIFO",
                )));
            }
            Err(e) => return Err(e.into()),
        }

        let fd = nix::fcntl::open(fifo, OFlag::O_RDONLY | OFlag::O_NONBLOCK, Mode::empty())?;
        Ok(fd)
    }
}

#[cfg(target_os = "linux")]
pub use platform::{MiniIpcClient, MiniIpcServer};

#[cfg(not(target_os = "linux"))]
pub struct MiniIpcClient;

#[cfg(not(target_os = "linux"))]
impl MiniIpcClient {
    pub fn try_send(_server_name: &str, _args: &[&str]) -> Result<(), MiniIpcError> {
        Err(MiniIpcError::UnsupportedPlatform)
    }
}
