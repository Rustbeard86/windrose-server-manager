//! Low-level process management primitives.
//!
//! # Windows stdin limitations
//!
//! Windrose (and most Windows console game servers) read from the Windows
//! console input buffer rather than the standard stdin pipe.  When the process
//! is spawned with `stdin(Stdio::piped())` the server's own console subsystem
//! does not see the bytes written to the pipe; only servers that explicitly
//! call `ReadFile` on `STDIN_HANDLE` will receive them.
//!
//! The implementation below pipes stdin and attempts to write commands through
//! it.  If the server ignores the pipe the `send_command` call will succeed
//! silently (the bytes are buffered in the OS pipe).  Callers should treat
//! command delivery as "best effort" and document this limitation to users.
//!
//! A future enhancement could use `WriteConsoleInput` to inject key events
//! directly into the server's console input buffer (via `AttachConsole` /
//! `FreeConsole`), but that requires the process to have a console window and
//! carries significant complexity.

use std::io;
use std::path::Path;

use tokio::io::AsyncWriteExt;
use tokio::process::{Child, ChildStdin, Command};
use tracing::info;

// ---------------------------------------------------------------------------
// ManagedProcess
// ---------------------------------------------------------------------------

/// Wraps a spawned child process, holding the handle and optional stdin pipe.
///
/// Obtaining the `pid` requires the process to be alive; the value is captured
/// at spawn time and cached so it remains available after the process exits.
pub struct ManagedProcess {
    /// Process ID captured at spawn time.
    pub pid: u32,
    pub(crate) child: Child,
    stdin: Option<ChildStdin>,
}

impl std::fmt::Debug for ManagedProcess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManagedProcess")
            .field("pid", &self.pid)
            .finish_non_exhaustive()
    }
}

impl ManagedProcess {
    pub(crate) fn new(mut child: Child) -> Self {
        // `child.id()` returns `None` only if the process has already exited,
        // which should not happen immediately after a successful spawn.
        // We fall back to 0 as a sentinel; callers should treat 0 as "unknown".
        let pid = child.id().unwrap_or(0);
        let stdin = child.stdin.take();
        Self { pid, child, stdin }
    }

    /// Write a command line to the server's stdin pipe.
    ///
    /// Returns `Ok(())` if the bytes were accepted by the OS pipe buffer.
    /// Whether the server process actually reads and acts on the command
    /// depends on the server implementation — see module-level docs.
    pub async fn send_command(&mut self, cmd: &str) -> io::Result<()> {
        match self.stdin.as_mut() {
            Some(stdin) => {
                let line = format!("{}\n", cmd);
                stdin.write_all(line.as_bytes()).await?;
                stdin.flush().await
            }
            None => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "stdin pipe not available — process was spawned without piped stdin",
            )),
        }
    }

    /// Attempt a graceful shutdown by writing "stop" to stdin.
    ///
    /// Returns `true` if the command was delivered to the pipe buffer.
    pub async fn graceful_stop(&mut self) -> bool {
        self.send_command("stop").await.is_ok()
    }

    /// Forcefully terminate the process via `SIGKILL` (Unix) or
    /// `TerminateProcess` (Windows).
    pub async fn kill(&mut self) -> io::Result<()> {
        self.child.kill().await
    }

    /// Non-blocking poll: returns `Some(status)` if the process has already
    /// exited, `None` if it is still running.
    #[allow(dead_code)]
    pub fn try_wait(&mut self) -> io::Result<Option<std::process::ExitStatus>> {
        self.child.try_wait()
    }

    /// Async wait for the process to exit.
    pub async fn wait(&mut self) -> io::Result<std::process::ExitStatus> {
        self.child.wait().await
    }
}

// ---------------------------------------------------------------------------
// spawn
// ---------------------------------------------------------------------------

/// Spawn the server executable and return a [`ManagedProcess`] handle.
///
/// `stdin` is piped so that commands can be forwarded.  `stdout` and `stderr`
/// are inherited (the server is expected to write to its own log file).
pub fn spawn(
    exe_path: &Path,
    args: &[String],
    working_dir: Option<&Path>,
) -> io::Result<ManagedProcess> {
    let mut cmd = Command::new(exe_path);
    cmd.args(args);

    if let Some(dir) = working_dir {
        cmd.current_dir(dir);
    }

    // Pipe stdin so we can attempt command delivery.
    cmd.stdin(std::process::Stdio::piped());
    // Inherit stdout/stderr — the server writes its own log file.
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());

    let child = cmd.spawn()?;
    let managed = ManagedProcess::new(child);
    info!(
        pid = managed.pid,
        path = %exe_path.display(),
        "Server process spawned"
    );
    Ok(managed)
}
