//! The Vise background worker.
//!
//! The desktop application never runs the transcode pipeline in-process. It
//! starts this crate's binary, which exposes a loopback-only,
//! token-authenticated JSON API, and it watches the process so an encoder crash
//! costs the job and nothing else.
//!
//! Module map:
//!
//! - [`runtime`] — shared state and startup settings.
//! - [`server`] — the loopback listener.
//! - [`router`] — request dispatch.
//! - [`scan`] — the scan state machine.
//! - [`encode`] — the encode state machine and process control.
//! - [`log`] — the bounded log buffer.
//! - [`platform`] — process suspension for pause and resume.

pub mod encode;
pub mod log;
pub mod platform;
pub mod router;
pub mod runtime;
pub mod scan;
pub mod server;

pub use runtime::{StartupError, WorkerRuntime, WorkerSettings};
pub use server::{ServerError, ServerHandle, WorkerServer};

use std::io::Read;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use vise_ipc::supervisor::parent_is_alive;

/// Start the worker: bind, publish the handshake, watch the parent, and serve.
///
/// Returns once the listener has stopped.
pub fn run(settings: WorkerSettings) -> Result<(), Box<dyn std::error::Error>> {
    let monitor_parent = settings.monitor_parent;
    let parent_pid = settings.parent_pid;

    let server = WorkerServer::bind(WorkerRuntime::new(settings))?;
    let port = server.local_addr().port();
    let runtime = server.runtime();
    runtime
        .logs()
        .info(format!("listening on 127.0.0.1:{port}"));

    if monitor_parent {
        spawn_parent_monitor(runtime.clone(), parent_pid.unwrap_or(0), port);
    }

    server.publish_handshake()?;
    server.serve();
    runtime.logs().info("worker stopped");
    Ok(())
}

/// Wake the accept loop by dialling the worker's own port.
///
/// The loop parks in `accept`, so setting the shutdown flag is not enough on its
/// own; a loopback connection lets it notice.
pub fn wake_listener(port: u16) {
    if port == 0 {
        return;
    }
    let _ = std::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, port));
}

/// Whether the worker was started with a pipe on its standard input.
///
/// The supervisor always hands over a pipe so the worker sees end-of-file when
/// the desktop application goes away. A worker started by hand from a terminal
/// must not treat that terminal as a parent that can disappear.
pub fn stdin_is_pipe() -> bool {
    #[cfg(windows)]
    {
        use std::ffi::c_void;
        extern "system" {
            fn GetStdHandle(kind: u32) -> *mut c_void;
            fn GetFileType(handle: *mut c_void) -> u32;
        }
        const STD_INPUT_HANDLE: u32 = -10i32 as u32;
        const FILE_TYPE_PIPE: u32 = 0x0003;
        unsafe {
            let handle = GetStdHandle(STD_INPUT_HANDLE);
            if handle.is_null() {
                return false;
            }
            GetFileType(handle) == FILE_TYPE_PIPE
        }
    }
    #[cfg(not(windows))]
    {
        // There is no portable test for this, so assume the pipe is present and
        // let the process-id watcher carry the monitoring.
        true
    }
}

/// Watch the parent process and stop the worker when it disappears.
///
/// Two mechanisms run together. The parent holds the write end of a pipe that
/// is connected to the worker's standard input, so the worker sees end-of-file
/// the moment the parent goes away. A process-id probe covers the case where
/// the worker was started without that pipe.
pub fn spawn_parent_monitor(runtime: Arc<WorkerRuntime>, parent_pid: u32, port: u16) {
    if parent_pid == 0 {
        return;
    }
    let flag = runtime.shutdown_flag();
    let logs = runtime.logs().clone();

    // Watcher one: the parent's pipe. Only meaningful when the parent actually
    // gave the worker a pipe; on an inherited console or a closed handle,
    // end-of-file arrives immediately and would look like an orphaned worker.
    if stdin_is_pipe() {
        let flag = flag.clone();
        let logs = logs.clone();
        let _ = std::thread::Builder::new()
            .name("vise-parent-pipe".into())
            .spawn(move || {
                let mut stdin = std::io::stdin();
                loop {
                    if flag.load(Ordering::SeqCst) {
                        return;
                    }
                    let mut buffer = [0u8; 64];
                    match stdin.read(&mut buffer) {
                        Ok(0) => {
                            logs.warn("parent process pipe closed; terminating the worker process");
                            flag.store(true, Ordering::SeqCst);
                            wake_listener(port);
                            return;
                        }
                        Ok(_) => continue,
                        Err(_) => return,
                    }
                }
            });
    }

    // Watcher two: the parent process id.
    let _ = std::thread::Builder::new()
        .name("vise-parent-monitor".into())
        .spawn(move || {
            loop {
                if flag.load(Ordering::SeqCst) {
                    return;
                }
                if !parent_is_alive(parent_pid) {
                    logs.warn(format!(
                        "parent process {parent_pid} is no longer running; terminating the worker process"
                    ));
                    flag.store(true, Ordering::SeqCst);
                    wake_listener(port);
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parent_monitor_ignores_pid_zero() {
        let runtime = Arc::new(WorkerRuntime::new(WorkerSettings::for_tests(0, "t")));
        spawn_parent_monitor(runtime.clone(), 0, 0);
        assert!(!runtime.shutdown_requested());
    }

    #[test]
    fn parent_monitor_terminates_when_the_parent_is_gone() {
        let runtime = Arc::new(WorkerRuntime::new(WorkerSettings::for_tests(0, "t")));
        // A process id that cannot be running; the probe must notice.
        spawn_parent_monitor(runtime.clone(), 0xFFFF_FF00, 0);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !runtime.shutdown_requested() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(
            runtime.shutdown_requested(),
            "the monitor must stop the worker when its parent is gone"
        );
    }

    #[test]
    fn monitor_leaves_a_live_parent_alone() {
        let runtime = Arc::new(WorkerRuntime::new(WorkerSettings::for_tests(0, "t")));
        spawn_parent_monitor(runtime.clone(), std::process::id(), 0);
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(!runtime.shutdown_requested());
    }
}
