//! Platform process control for the worker's pause, resume, and stop routes.
//!
//! Pause is implemented by suspending the encoder process rather than by
//! signalling it: an encoder does not understand a pause request, and a
//! suspended process keeps its buffered output intact, so resuming continues
//! from exactly where it stopped.
//!
//! Only the small amount of platform API needed for that lives here, declared
//! directly so the crate keeps its dependency footprint at zero beyond
//! `serde` and `vise-core`.

/// Outcome of a process-control request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlError {
    /// The process could not be opened; it probably already exited.
    NotFound(u32),
    /// The platform does not offer the requested operation.
    Unsupported(&'static str),
    /// The platform call failed.
    Failed(String),
}

impl std::fmt::Display for ControlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ControlError::NotFound(pid) => write!(f, "process {pid} is no longer running"),
            ControlError::Unsupported(what) => {
                write!(f, "{what} is not supported on this platform")
            }
            ControlError::Failed(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for ControlError {}

/// Suspend a process.
pub fn suspend(pid: u32) -> Result<(), ControlError> {
    sys::suspend(pid)
}

/// Resume a suspended process.
pub fn resume(pid: u32) -> Result<(), ControlError> {
    sys::resume(pid)
}

/// Whether the platform supports suspending processes.
pub fn suspend_supported() -> bool {
    sys::suspend_supported()
}

#[cfg(windows)]
mod sys {
    use super::ControlError;
    use std::ffi::c_void;
    use std::os::raw::c_char;

    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
        fn CloseHandle(handle: *mut c_void) -> i32;
        fn GetModuleHandleW(module_name: *const u16) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, proc_name: *const c_char) -> *mut c_void;
    }

    const PROCESS_SUSPEND_RESUME: u32 = 0x0800;

    type NtProcessFn = unsafe extern "system" fn(handle: isize) -> i32;

    fn utf16(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn ascii(s: &str) -> Vec<u8> {
        s.as_bytes()
            .iter()
            .copied()
            .chain(std::iter::once(0))
            .collect()
    }

    fn ntdll_function(name: &str) -> Option<NtProcessFn> {
        unsafe {
            let module = GetModuleHandleW(utf16("ntdll.dll").as_ptr());
            if module.is_null() {
                return None;
            }
            let symbol = GetProcAddress(module, ascii(name).as_ptr().cast::<c_char>());
            if symbol.is_null() {
                return None;
            }
            Some(std::mem::transmute::<*mut c_void, NtProcessFn>(symbol))
        }
    }

    pub fn suspend_supported() -> bool {
        ntdll_function("NtSuspendProcess").is_some()
    }

    pub fn suspend(pid: u32) -> Result<(), ControlError> {
        let function = ntdll_function("NtSuspendProcess")
            .ok_or(ControlError::Unsupported("process suspension"))?;
        unsafe {
            let handle = OpenProcess(PROCESS_SUSPEND_RESUME, 0, pid);
            if handle.is_null() {
                return Err(ControlError::NotFound(pid));
            }
            let status = function(handle as isize);
            CloseHandle(handle);
            if status < 0 {
                return Err(ControlError::Failed(format!(
                    "NtSuspendProcess failed with status {status:#x}"
                )));
            }
        }
        Ok(())
    }

    pub fn resume(pid: u32) -> Result<(), ControlError> {
        let function =
            ntdll_function("NtResumeProcess").ok_or(ControlError::Unsupported("process resume"))?;
        unsafe {
            let handle = OpenProcess(PROCESS_SUSPEND_RESUME, 0, pid);
            if handle.is_null() {
                return Err(ControlError::NotFound(pid));
            }
            let status = function(handle as isize);
            CloseHandle(handle);
            if status < 0 {
                return Err(ControlError::Failed(format!(
                    "NtResumeProcess failed with status {status:#x}"
                )));
            }
        }
        Ok(())
    }
}

#[cfg(unix)]
mod sys {
    use super::ControlError;

    extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }

    const SIGSTOP: i32 = 19;
    const SIGCONT: i32 = 18;

    fn send(pid: u32, signal: i32, what: &str) -> Result<(), ControlError> {
        let result = unsafe { kill(pid as i32, signal) };
        if result == 0 {
            Ok(())
        } else {
            Err(ControlError::Failed(format!(
                "{what} failed for process {pid}"
            )))
        }
    }

    pub fn suspend_supported() -> bool {
        true
    }

    pub fn suspend(pid: u32) -> Result<(), ControlError> {
        send(pid, SIGSTOP, "SIGSTOP")
    }

    pub fn resume(pid: u32) -> Result<(), ControlError> {
        send(pid, SIGCONT, "SIGCONT")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suspension_is_available_on_supported_targets() {
        assert!(
            suspend_supported(),
            "every platform Vise targets exposes process suspension"
        );
    }

    #[test]
    fn control_errors_render_usefully() {
        assert!(ControlError::NotFound(42).to_string().contains("42"));
        assert!(ControlError::Unsupported("process suspension")
            .to_string()
            .contains("not supported"));
        assert!(ControlError::Failed("boom".into())
            .to_string()
            .contains("boom"));
    }

    #[cfg(windows)]
    #[test]
    fn controlling_a_missing_process_is_reported_not_panicked() {
        // Process id 0 never identifies a user process, so the platform call
        // must fail cleanly instead of aborting.
        assert_eq!(suspend(0), Err(ControlError::NotFound(0)));
        assert_eq!(resume(0), Err(ControlError::NotFound(0)));
    }
}
