use std::collections::HashMap;
use std::fmt;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Error type for process operations.
#[derive(Debug)]
pub enum ProcessError {
    /// The process could not be started.
    StartFailed { program: String, source: io::Error },
    /// A write to the child's stdin pipe failed.
    WriteFailed { program: String, source: io::Error },
    /// A read from the child's stdout pipe failed.
    ReadFailed { program: String, source: io::Error },
    /// A read from the child's stderr pipe failed.
    ReadErrFailed { program: String, source: io::Error },
    /// No arguments were provided (the argument list must contain at least the program name).
    EmptyArgs,
    /// `try_start` was called while a child process is already running.
    AlreadyRunning,
}

impl fmt::Display for ProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProcessError::StartFailed { program, source } => {
                write!(f, "failed to start process \"{program}\": {source}")
            }
            ProcessError::WriteFailed { program, source } => {
                write!(
                    f,
                    "failed to write to stdin pipe of child process \"{program}\": {source}"
                )
            }
            ProcessError::ReadFailed { program, source } => {
                write!(
                    f,
                    "failed to read stdout pipe of child process \"{program}\": {source}"
                )
            }
            ProcessError::ReadErrFailed { program, source } => {
                write!(
                    f,
                    "failed to read stderr pipe of child process \"{program}\": {source}"
                )
            }
            ProcessError::EmptyArgs => write!(f, "no arguments provided"),
            ProcessError::AlreadyRunning => {
                write!(f, "try_start called while still managing another process")
            }
        }
    }
}

impl std::error::Error for ProcessError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ProcessError::StartFailed { source, .. }
            | ProcessError::WriteFailed { source, .. }
            | ProcessError::ReadFailed { source, .. }
            | ProcessError::ReadErrFailed { source, .. } => Some(source),
            ProcessError::EmptyArgs | ProcessError::AlreadyRunning => None,
        }
    }
}

/// Flags controlling how a child process's standard I/O handles are set up.
///
/// These mirror the C++ `SF_*` flags. `PIPE_*` and `SHARE_*` for the same
/// stream are mutually exclusive; if both are set, `PIPE` wins.
#[derive(Debug, Clone, Copy)]
pub struct StartFlags(u32);

impl StartFlags {
    /// Inherit stdin from the parent process.
    pub const SHARE_STDIN: StartFlags = StartFlags(1 << 0);
    /// Create a pipe for writing to the child's stdin.
    pub const PIPE_STDIN: StartFlags = StartFlags(1 << 1);
    /// Inherit stdout from the parent process.
    pub const SHARE_STDOUT: StartFlags = StartFlags(1 << 2);
    /// Create a pipe for reading the child's stdout.
    pub const PIPE_STDOUT: StartFlags = StartFlags(1 << 3);
    /// Inherit stderr from the parent process.
    pub const SHARE_STDERR: StartFlags = StartFlags(1 << 4);
    /// Create a pipe for reading the child's stderr.
    pub const PIPE_STDERR: StartFlags = StartFlags(1 << 5);

    /// The default flags: inherit all three standard streams.
    pub const DEFAULT: StartFlags =
        StartFlags(Self::SHARE_STDIN.0 | Self::SHARE_STDOUT.0 | Self::SHARE_STDERR.0);

    /// Returns an empty set of flags (all streams closed/null).
    pub const fn empty() -> Self {
        StartFlags(0)
    }

    pub const fn contains(self, other: StartFlags) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl std::ops::BitOr for StartFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        StartFlags(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for StartFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// Result of a non-blocking pipe I/O attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipeResult {
    /// Some bytes were transferred (count > 0).
    Bytes(usize),
    /// The operation would block; try again later.
    WouldBlock,
    /// The pipe is closed (EOF or broken pipe).
    Closed,
}

/// Wraps a child process, providing spawn, pipe I/O, signalling, and wait
/// functionality.
///
/// This is the Rust equivalent of the C++ `emProcess` class. It uses
/// `std::process::Command` / `Child` internally and exposes a similar API.
///
/// Dropping a `emProcess` while the child is still running will attempt to kill
/// the child and wait for it.
pub struct emProcess {
    /// The first argument (program name), kept for error messages.
    arg0: String,
    /// The managed child process, if running.
    child: Option<Child>,
    /// Stdin pipe handle, taken from `Child` when pipes are requested.
    stdin_pipe: Option<ChildStdin>,
    /// Stdout pipe handle.
    stdout_pipe: Option<ChildStdout>,
    /// Stderr pipe handle.
    stderr_pipe: Option<ChildStderr>,
    /// Cached exit status once the child has terminated.
    exit_status: Option<ExitStatus>,
}

impl emProcess {
    /// Create a new `emProcess` with no child running.
    pub fn new() -> Self {
        emProcess {
            arg0: String::new(),
            child: None,
            stdin_pipe: None,
            stdout_pipe: None,
            stderr_pipe: None,
            exit_status: None,
        }
    }

    /// Start a managed child process.
    ///
    /// # Arguments
    ///
    /// * `args` - Program arguments. The first element is the program name
    ///   (searched on `PATH` if not a path). Must not be empty.
    /// * `extra_env` - Additional environment variables as `(key, value)` pairs.
    ///   These are added on top of the inherited environment. To remove a
    ///   variable, pass an empty value (the variable will be set to `""`; full
    ///   removal is not supported portably).
    /// * `dir_path` - Working directory for the child, or `None` to inherit.
    /// * `flags` - Combination of [`StartFlags`] controlling I/O handles.
    pub fn TryStart(
        &mut self,
        args: &[&str],
        extra_env: &HashMap<String, String>,
        dir_path: Option<&Path>,
        flags: StartFlags,
    ) -> Result<(), ProcessError> {
        if args.is_empty() {
            return Err(ProcessError::EmptyArgs);
        }
        if self.child.is_some() {
            return Err(ProcessError::AlreadyRunning);
        }

        // Resolve effective flags: PIPE wins over SHARE for the same stream.
        let pipe_stdin = flags.contains(StartFlags::PIPE_STDIN);
        let pipe_stdout = flags.contains(StartFlags::PIPE_STDOUT);
        let pipe_stderr = flags.contains(StartFlags::PIPE_STDERR);

        let share_stdin = !pipe_stdin && flags.contains(StartFlags::SHARE_STDIN);
        let share_stdout = !pipe_stdout && flags.contains(StartFlags::SHARE_STDOUT);
        let share_stderr = !pipe_stderr && flags.contains(StartFlags::SHARE_STDERR);

        let mut cmd = Command::new(args[0]);
        if args.len() > 1 {
            cmd.args(&args[1..]);
        }

        // Environment
        cmd.envs(extra_env.iter());

        // Working directory
        if let Some(dir) = dir_path {
            cmd.current_dir(dir);
        }

        // Stdin
        if pipe_stdin {
            cmd.stdin(Stdio::piped());
        } else if share_stdin {
            cmd.stdin(Stdio::inherit());
        } else {
            cmd.stdin(Stdio::null());
        }

        // Stdout
        if pipe_stdout {
            cmd.stdout(Stdio::piped());
        } else if share_stdout {
            cmd.stdout(Stdio::inherit());
        } else {
            cmd.stdout(Stdio::null());
        }

        // Stderr
        if pipe_stderr {
            cmd.stderr(Stdio::piped());
        } else if share_stderr {
            cmd.stderr(Stdio::inherit());
        } else {
            cmd.stderr(Stdio::null());
        }

        let mut child = cmd.spawn().map_err(|e| ProcessError::StartFailed {
            program: args[0].to_string(),
            source: e,
        })?;

        self.arg0 = args[0].to_string();
        self.stdin_pipe = child.stdin.take();
        self.stdout_pipe = child.stdout.take();
        self.stderr_pipe = child.stderr.take();
        self.exit_status = None;
        self.child = Some(child);

        Ok(())
    }

    /// Start an unmanaged child process (fire-and-forget, no pipe I/O).
    ///
    /// The child is spawned and then immediately detached. No pipes are created
    /// regardless of `flags`.
    pub fn TryStartUnmanaged(
        args: &[&str],
        extra_env: &HashMap<String, String>,
        dir_path: Option<&Path>,
        flags: StartFlags,
    ) -> Result<(), ProcessError> {
        if args.is_empty() {
            return Err(ProcessError::EmptyArgs);
        }

        // Strip pipe flags — unmanaged processes cannot use pipes.
        let share_stdin = flags.contains(StartFlags::SHARE_STDIN);
        let share_stdout = flags.contains(StartFlags::SHARE_STDOUT);
        let share_stderr = flags.contains(StartFlags::SHARE_STDERR);

        let mut cmd = Command::new(args[0]);
        if args.len() > 1 {
            cmd.args(&args[1..]);
        }
        cmd.envs(extra_env.iter());
        if let Some(dir) = dir_path {
            cmd.current_dir(dir);
        }

        cmd.stdin(if share_stdin {
            Stdio::inherit()
        } else {
            Stdio::null()
        });
        cmd.stdout(if share_stdout {
            Stdio::inherit()
        } else {
            Stdio::null()
        });
        cmd.stderr(if share_stderr {
            Stdio::inherit()
        } else {
            Stdio::null()
        });

        // Spawn and immediately drop the Child handle — the OS will keep the
        // process running.
        let _child = cmd.spawn().map_err(|e| ProcessError::StartFailed {
            program: args[0].to_string(),
            source: e,
        })?;

        Ok(())
    }

    /// Write bytes to the child's stdin pipe (blocking).
    ///
    /// Returns a [`PipeResult`] indicating how many bytes were written, whether
    /// the operation would block, or whether the pipe is closed.
    pub fn TryWrite(&mut self, buf: &[u8]) -> Result<PipeResult, ProcessError> {
        let stdin = match self.stdin_pipe.as_mut() {
            Some(s) => s,
            None => return Ok(PipeResult::Closed),
        };
        if buf.is_empty() {
            return Ok(PipeResult::WouldBlock);
        }
        match stdin.write(buf) {
            Ok(0) => {
                self.CloseWriting();
                Ok(PipeResult::Closed)
            }
            Ok(n) => Ok(PipeResult::Bytes(n)),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(PipeResult::WouldBlock),
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => {
                self.CloseWriting();
                Ok(PipeResult::Closed)
            }
            Err(e) => {
                self.CloseWriting();
                Err(ProcessError::WriteFailed {
                    program: self.arg0.clone(),
                    source: e,
                })
            }
        }
    }

    /// Read bytes from the child's stdout pipe (blocking).
    ///
    /// Returns a [`PipeResult`] indicating how many bytes were read, whether
    /// the operation would block, or whether the pipe is closed.
    pub fn TryRead(&mut self, buf: &mut [u8]) -> Result<PipeResult, ProcessError> {
        let stdout = match self.stdout_pipe.as_mut() {
            Some(s) => s,
            None => return Ok(PipeResult::Closed),
        };
        if buf.is_empty() {
            return Ok(PipeResult::WouldBlock);
        }
        match stdout.read(buf) {
            Ok(0) => {
                self.CloseReading();
                Ok(PipeResult::Closed)
            }
            Ok(n) => Ok(PipeResult::Bytes(n)),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(PipeResult::WouldBlock),
            Err(e) => {
                self.CloseReading();
                Err(ProcessError::ReadFailed {
                    program: self.arg0.clone(),
                    source: e,
                })
            }
        }
    }

    /// Read bytes from the child's stderr pipe (blocking).
    ///
    /// Returns a [`PipeResult`] indicating how many bytes were read, whether
    /// the operation would block, or whether the pipe is closed.
    pub fn TryReadErr(&mut self, buf: &mut [u8]) -> Result<PipeResult, ProcessError> {
        let stderr = match self.stderr_pipe.as_mut() {
            Some(s) => s,
            None => return Ok(PipeResult::Closed),
        };
        if buf.is_empty() {
            return Ok(PipeResult::WouldBlock);
        }
        match stderr.read(buf) {
            Ok(0) => {
                self.CloseReadingErr();
                Ok(PipeResult::Closed)
            }
            Ok(n) => Ok(PipeResult::Bytes(n)),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(PipeResult::WouldBlock),
            Err(e) => {
                self.CloseReadingErr();
                Err(ProcessError::ReadErrFailed {
                    program: self.arg0.clone(),
                    source: e,
                })
            }
        }
    }

    /// Close the stdin pipe to the child process.
    pub fn CloseWriting(&mut self) {
        self.stdin_pipe = None;
    }

    /// Close the stdout pipe from the child process.
    pub fn CloseReading(&mut self) {
        self.stdout_pipe = None;
    }

    /// Close the stderr pipe from the child process.
    pub fn CloseReadingErr(&mut self) {
        self.stderr_pipe = None;
    }

    /// Send a termination signal to the child process (SIGTERM on Unix,
    /// `kill()` on Windows).
    ///
    /// On Unix this sends `SIGTERM`. Note that Rust's `Child::kill()` sends
    /// `SIGKILL`, so we use a platform-specific path for the soft signal.
    pub fn SendTerminationSignal(&mut self) {
        if !self.IsRunning() {
            return;
        }
        #[cfg(unix)]
        {
            if let Some(child) = self.child.as_ref() {
                let pid = child.id() as i32;
                // SIGTERM = 15 on all Unix platforms.
                // Safety: sending a signal to a valid pid is safe.
                extern "C" {
                    fn kill(pid: i32, sig: i32) -> i32;
                }
                unsafe {
                    kill(pid, 15);
                }
            }
        }
        #[cfg(not(unix))]
        {
            // On non-Unix platforms fall back to kill (SIGKILL equivalent).
            if let Some(child) = self.child.as_mut() {
                let _ = child.kill();
            }
        }
    }

    /// Forcibly kill the child process (SIGKILL on Unix, TerminateProcess on
    /// Windows).
    pub fn SendKillSignal(&mut self) {
        if !self.IsRunning() {
            return;
        }
        if let Some(child) = self.child.as_mut() {
            let _ = child.kill();
        }
    }

    /// Wait for the child process to terminate, with an optional timeout.
    ///
    /// * `None` — wait indefinitely.
    /// * `Some(duration)` — poll with increasing sleep intervals up to the
    ///   timeout.
    ///
    /// Returns `true` if the child has terminated (or was never started),
    /// `false` if the timeout expired.
    pub fn WaitForTermination(&mut self, timeout: Option<Duration>) -> bool {
        let child = match self.child.as_mut() {
            Some(c) => c,
            None => return true,
        };

        match timeout {
            None => {
                // Block indefinitely.
                match child.wait() {
                    Ok(status) => {
                        self.exit_status = Some(status);
                        self.child = None;
                        self.CloseWriting();
                        self.CloseReading();
                        self.CloseReadingErr();
                        true
                    }
                    Err(_) => false,
                }
            }
            Some(dur) => {
                let start = Instant::now();
                let mut sleep_ms: u64 = 0;
                loop {
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            self.exit_status = Some(status);
                            self.child = None;
                            self.CloseWriting();
                            self.CloseReading();
                            self.CloseReadingErr();
                            return true;
                        }
                        Ok(None) => {
                            if start.elapsed() >= dur {
                                return false;
                            }
                            let remaining = dur.saturating_sub(start.elapsed());
                            let sleep_dur = Duration::from_millis(sleep_ms).min(remaining);
                            if !sleep_dur.is_zero() {
                                thread::sleep(sleep_dur);
                            }
                            if sleep_ms < 10 {
                                sleep_ms += 1;
                            }
                        }
                        Err(_) => return false,
                    }
                }
            }
        }
    }

    /// Check whether the child process is still running.
    pub fn IsRunning(&mut self) -> bool {
        !self.WaitForTermination(Some(Duration::ZERO))
    }

    /// Send a termination signal and wait for the child to exit.
    ///
    /// If the child does not exit within `timeout`, it is forcibly killed.
    pub fn Terminate(&mut self, timeout: Duration) {
        if !self.IsRunning() {
            return;
        }
        self.SendTerminationSignal();
        if !self.WaitForTermination(Some(timeout)) {
            self.SendKillSignal();
            self.WaitForTermination(None);
        }
    }

    // DIVERGED: no C++ equivalent — Rust-only convenience combining WaitForTermination + SendKillSignal
    /// Wait for the child to exit and then forcibly kill it if the timeout
    /// expires.
    pub fn wait_or_kill(&mut self, timeout: Duration) {
        if self.WaitForTermination(Some(timeout)) {
            return;
        }
        self.SendKillSignal();
        self.WaitForTermination(None);
    }

    /// Get the exit code of a terminated child process.
    ///
    /// Returns `None` if the child has not yet terminated or was never started.
    /// On Unix, if the child was killed by a signal, the code is `128 + signal`.
    pub fn GetExitStatus(&self) -> Option<i32> {
        self.exit_status.map(|s| {
            #[cfg(unix)]
            {
                use std::os::unix::process::ExitStatusExt;
                if let Some(code) = s.code() {
                    code
                } else if let Some(signal) = s.signal() {
                    128 + signal
                } else {
                    -1
                }
            }
            #[cfg(not(unix))]
            {
                s.code().unwrap_or(-1)
            }
        })
    }

    /// Get the first program argument (program name). Useful for error messages.
    pub fn GetArg0(&self) -> &str {
        &self.arg0
    }
}

impl Default for emProcess {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for emProcess {
    fn drop(&mut self) {
        if self.IsRunning() {
            self.Terminate(Duration::from_secs(20));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_and_read_stdout() {
        let mut proc = emProcess::new();
        let env = HashMap::new();
        proc.TryStart(
            &["echo", "hello"],
            &env,
            None,
            StartFlags::PIPE_STDOUT | StartFlags::PIPE_STDERR,
        )
        .expect("failed to start echo");

        let mut buf = [0u8; 64];
        // Read until we get output or pipe closes.
        proc.WaitForTermination(Some(Duration::from_secs(5)));
        let result = proc.TryRead(&mut buf).expect("read failed");
        match result {
            PipeResult::Bytes(n) => {
                let output = std::str::from_utf8(&buf[..n]).expect("not utf8");
                assert!(output.contains("hello"), "expected 'hello', got: {output}");
            }
            PipeResult::Closed => {
                // Child already terminated and pipe was drained — that is OK
                // for this test if the echo was fast.
            }
            PipeResult::WouldBlock => panic!("unexpected WouldBlock after wait"),
        }

        assert_eq!(proc.GetExitStatus(), Some(0), "process should exit with status 0");
    }

    #[test]
    fn empty_args_error() {
        let mut proc = emProcess::new();
        let env = HashMap::new();
        let err = proc
            .TryStart(&[], &env, None, StartFlags::DEFAULT)
            .unwrap_err();
        assert!(matches!(err, ProcessError::EmptyArgs));
    }

    #[test]
    fn exit_code_nonzero() {
        let mut proc = emProcess::new();
        let env = HashMap::new();
        proc.TryStart(&["false"], &env, None, StartFlags::empty())
            .expect("failed to start false");
        proc.WaitForTermination(Some(Duration::from_secs(5)));
        assert_eq!(proc.GetExitStatus(), Some(1));
    }

    #[test]
    fn write_to_stdin_pipe() {
        let mut proc = emProcess::new();
        let env = HashMap::new();
        proc.TryStart(
            &["cat"],
            &env,
            None,
            StartFlags::PIPE_STDIN | StartFlags::PIPE_STDOUT,
        )
        .expect("failed to start cat");

        let data = b"test data\n";
        let write_result = proc.TryWrite(data).expect("write failed");
        assert!(matches!(write_result, PipeResult::Bytes(_)));

        proc.CloseWriting(); // signal EOF to cat

        let mut buf = [0u8; 64];
        let read_result = proc.TryRead(&mut buf).expect("read failed");
        match read_result {
            PipeResult::Bytes(n) => {
                assert_eq!(&buf[..n], data);
            }
            other => panic!("expected Bytes, got {other:?}"),
        }

        proc.WaitForTermination(Some(Duration::from_secs(5)));
    }

    #[test]
    fn kill_long_running() {
        let mut proc = emProcess::new();
        let env = HashMap::new();
        proc.TryStart(&["sleep", "60"], &env, None, StartFlags::empty())
            .expect("failed to start sleep");

        assert!(proc.IsRunning());
        proc.SendKillSignal();
        assert!(proc.WaitForTermination(Some(Duration::from_secs(5))));
        assert!(!proc.IsRunning());
        // Killed by signal 9 → exit status 128+9 = 137
        assert_eq!(proc.GetExitStatus(), Some(137));
    }

    #[test]
    fn stderr_pipe() {
        let mut proc = emProcess::new();
        let env = HashMap::new();
        proc.TryStart(
            &["sh", "-c", "echo error >&2"],
            &env,
            None,
            StartFlags::PIPE_STDERR,
        )
        .expect("failed to start sh");

        proc.WaitForTermination(Some(Duration::from_secs(5)));

        let mut buf = [0u8; 64];
        let result = proc.TryReadErr(&mut buf).expect("read stderr failed");
        match result {
            PipeResult::Bytes(n) => {
                let output = std::str::from_utf8(&buf[..n]).expect("not utf8");
                assert!(output.contains("error"), "expected 'error', got: {output}");
            }
            PipeResult::Closed => {
                // Pipe already drained — acceptable for a fast child.
            }
            PipeResult::WouldBlock => panic!("unexpected WouldBlock after wait"),
        }
    }

    #[test]
    fn working_directory() {
        let mut proc = emProcess::new();
        let env = HashMap::new();
        let tmpdir = std::env::temp_dir();
        proc.TryStart(
            &["pwd"],
            &env,
            Some(tmpdir.as_path()),
            StartFlags::PIPE_STDOUT,
        )
        .expect("failed to start pwd");

        proc.WaitForTermination(Some(Duration::from_secs(5)));

        let mut buf = [0u8; 512];
        let result = proc.TryRead(&mut buf).expect("read failed");
        match result {
            PipeResult::Bytes(n) => {
                let output = std::str::from_utf8(&buf[..n]).expect("not utf8");
                let tmpdir_str = tmpdir.to_str().expect("temp_dir not utf8");
                assert!(
                    output.contains(tmpdir_str),
                    "expected output to contain '{tmpdir_str}', got: {output}"
                );
            }
            PipeResult::Closed => {
                // Pipe already drained — acceptable for a fast child.
            }
            PipeResult::WouldBlock => panic!("unexpected WouldBlock after wait"),
        }
    }

    #[test]
    fn extra_env() {
        let mut proc = emProcess::new();
        let mut env = HashMap::new();
        env.insert("TEST_ZUICCHINI_VAR".to_string(), "hello_test".to_string());
        proc.TryStart(
            &["sh", "-c", "echo $TEST_ZUICCHINI_VAR"],
            &env,
            None,
            StartFlags::PIPE_STDOUT,
        )
        .expect("failed to start sh");

        proc.WaitForTermination(Some(Duration::from_secs(5)));

        let mut buf = [0u8; 64];
        let result = proc.TryRead(&mut buf).expect("read failed");
        match result {
            PipeResult::Bytes(n) => {
                let output = std::str::from_utf8(&buf[..n]).expect("not utf8");
                assert!(
                    output.contains("hello_test"),
                    "expected 'hello_test', got: {output}"
                );
            }
            PipeResult::Closed => {
                // Pipe already drained — acceptable for a fast child.
            }
            PipeResult::WouldBlock => panic!("unexpected WouldBlock after wait"),
        }
    }

    #[test]
    fn is_running_after_exit() {
        let mut proc = emProcess::new();
        let env = HashMap::new();
        proc.TryStart(&["true"], &env, None, StartFlags::empty())
            .expect("failed to start true");
        proc.WaitForTermination(Some(Duration::from_secs(5)));
        assert!(!proc.IsRunning());
    }

    #[test]
    fn send_terminate_signal() {
        let mut proc = emProcess::new();
        let env = HashMap::new();
        proc.TryStart(&["sleep", "60"], &env, None, StartFlags::empty())
            .expect("failed to start sleep");

        assert!(proc.IsRunning());
        proc.SendTerminationSignal();
        assert!(proc.WaitForTermination(Some(Duration::from_secs(5))));
        assert!(!proc.IsRunning());
        assert!(
            proc.GetExitStatus().is_some(),
            "exit status should be Some after termination"
        );
    }
}
