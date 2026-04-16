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
use tracing::{info, warn};

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
        #[cfg(windows)]
        {
            match send_console_command(self.pid, cmd) {
                Ok(true) => return Ok(()),
                Ok(false) => {
                    // Console not available for injection; fall through to stdin pipe.
                }
                Err(e) => {
                    // Some servers do not expose an attachable console. Keep going and
                    // try the stdin pipe instead of hard-failing command delivery.
                    warn!(pid = self.pid, "Console command injection failed ({e}); falling back to stdin pipe");
                }
            }
        }

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
        // Use in-band server commands for graceful stop to avoid console
        // control signals affecting the manager process on Windows.
        self.send_command("quit").await.is_ok() || self.send_command("stop").await.is_ok()
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

    #[cfg(windows)]
    {
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(CREATE_NEW_PROCESS_GROUP);
    }

    // Pipe stdin so we can attempt command delivery.
    cmd.stdin(std::process::Stdio::piped());
    // Pipe stdout/stderr so the manager can ingest live output alongside file tails.
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let child = cmd.spawn()?;
    let managed = ManagedProcess::new(child);
    info!(
        pid = managed.pid,
        path = %exe_path.display(),
        "Server process spawned"
    );
    Ok(managed)
}

pub fn pid_is_running(pid: u32) -> bool {
    let mut system = sysinfo::System::new();
    let spid = sysinfo::Pid::from_u32(pid);
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::Some(&[spid]),
        sysinfo::ProcessRefreshKind::new(),
    );
    system.process(spid).is_some()
}

#[cfg(windows)]
fn send_console_command(pid: u32, cmd: &str) -> io::Result<bool> {
    win_console::send_console_command(pid, cmd)
}

#[cfg(not(windows))]
fn send_console_command(_pid: u32, _cmd: &str) -> io::Result<bool> {
    Ok(false)
}

#[cfg(windows)]
mod win_console {
    use std::io;
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    const KEY_EVENT: u16 = 0x0001;
    const STD_INPUT_HANDLE: u32 = (-10i32) as u32;

    #[repr(C)]
    struct KeyEventRecord {
        b_key_down: i32,
        w_repeat_count: u16,
        w_virtual_key_code: u16,
        w_virtual_scan_code: u16,
        unicode_char: u16,
        dw_control_key_state: u32,
    }

    #[repr(C)]
    struct InputRecord {
        event_type: u16,
        _padding: u16,
        key_event: KeyEventRecord,
    }

    type Handle = isize;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn FreeConsole() -> i32;
        fn AttachConsole(dw_process_id: u32) -> i32;
        fn AllocConsole() -> i32;
        fn GetStdHandle(n_std_handle: u32) -> Handle;
        fn WriteConsoleInputW(
            h_console_input: Handle,
            lp_buffer: *const InputRecord,
            n_length: u32,
            lp_number_of_events_written: *mut u32,
        ) -> i32;
    }

    struct ConsoleAttachmentGuard;

    impl ConsoleAttachmentGuard {
        fn attach(pid: u32) -> io::Result<Self> {
            unsafe {
                FreeConsole();
                if AttachConsole(pid) == 0 {
                    let _ = AttachConsole(ATTACH_PARENT_PROCESS);
                    return Err(io::Error::last_os_error());
                }
            }
            Ok(Self)
        }
    }

    impl Drop for ConsoleAttachmentGuard {
        fn drop(&mut self) {
            unsafe {
                FreeConsole();
                if AttachConsole(ATTACH_PARENT_PROCESS) == 0 {
                    let _ = AllocConsole();
                }
            }
        }
    }

    pub fn send_console_command(pid: u32, cmd: &str) -> io::Result<bool> {
        let _guard = ConsoleAttachmentGuard::attach(pid)?;

        let input = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
        if input == 0 || input == -1 {
            return Err(io::Error::last_os_error());
        }

        let mut records = Vec::with_capacity((cmd.encode_utf16().count() + 1) * 2);
        for ch in cmd.encode_utf16().chain(std::iter::once('\r' as u16)) {
            let vk = if ch == '\r' as u16 { 0x0D } else { 0 };
            records.push(InputRecord {
                event_type: KEY_EVENT,
                _padding: 0,
                key_event: KeyEventRecord {
                    b_key_down: 1,
                    w_repeat_count: 1,
                    w_virtual_key_code: vk,
                    w_virtual_scan_code: 0,
                    unicode_char: ch,
                    dw_control_key_state: 0,
                },
            });
            records.push(InputRecord {
                event_type: KEY_EVENT,
                _padding: 0,
                key_event: KeyEventRecord {
                    b_key_down: 0,
                    w_repeat_count: 1,
                    w_virtual_key_code: vk,
                    w_virtual_scan_code: 0,
                    unicode_char: ch,
                    dw_control_key_state: 0,
                },
            });
        }

        let mut written = 0u32;
        let ok = unsafe {
            WriteConsoleInputW(input, records.as_ptr(), records.len() as u32, &mut written)
        } != 0;

        if !ok {
            return Err(io::Error::last_os_error());
        }

        Ok(written == records.len() as u32)
    }

}
