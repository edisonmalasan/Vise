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

/// How much scheduling priority a process asks for.
///
/// Zero leaves the process where the platform put it, which is the answer that
/// costs nothing to give and the one a machine should get by default. The
/// numbers are a scale rather than names because every platform this builds for
/// expresses priority as a number with zero in the middle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    /// Much below normal: yield to everything else on the machine.
    Low,
    /// Below normal.
    BelowNormal,
    /// Whatever the platform gives a process by default.
    Normal,
    /// Above normal: take a larger share of the machine.
    AboveNormal,
    /// Much above normal.
    High,
}

impl Priority {
    /// Read a priority from the signed scale the contract carries.
    pub fn of(level: i32) -> Self {
        match level {
            i32::MIN..=-2 => Priority::Low,
            -1 => Priority::BelowNormal,
            0 => Priority::Normal,
            1 => Priority::AboveNormal,
            _ => Priority::High,
        }
    }

    /// The level this priority is, on the scale the contract carries.
    pub fn level(self) -> i32 {
        match self {
            Priority::Low => -2,
            Priority::BelowNormal => -1,
            Priority::Normal => 0,
            Priority::AboveNormal => 1,
            Priority::High => 2,
        }
    }

    /// The name the interface shows for it.
    pub fn label(self) -> &'static str {
        match self {
            Priority::Low => "Idle",
            Priority::BelowNormal => "Below normal",
            Priority::Normal => "Normal",
            Priority::AboveNormal => "Above normal",
            Priority::High => "Realtime",
        }
    }

    /// What asking for it means, for the preference that chooses it.
    pub fn effect(self) -> &'static str {
        match self {
            Priority::Low => "The worker steps aside for everything else on the machine.",
            Priority::BelowNormal => "The worker gives way to other programs.",
            Priority::Normal => "The worker takes a share of the machine like any other program.",
            Priority::AboveNormal => "The worker asks for more of the machine than other programs get.",
            Priority::High => "The worker asks for nearly all of the machine and can starve other programs.",
        }
    }

    /// Whether a process on this platform can be moved to this priority after
    /// it has started.
    pub fn is_supported(self) -> bool {
        sys::priority_supported()
    }
}

/// Move `pid` to `priority`.
///
/// The worker asks for this on itself rather than being started at a priority,
/// because a process cannot be created above the priority of whoever created
/// it: the parent cannot hand over a priority it does not itself hold. Asking
/// from inside is therefore the only route that reaches the levels above
/// normal, and it is also the only route that works once a portable settings
/// file has already decided the worker should not be isolated at all.
pub fn set_priority(pid: u32, priority: Priority) -> Result<(), ControlError> {
    if !priority.is_supported() {
        return Err(ControlError::Unsupported("process priority"));
    }
    sys::set_priority(pid, priority)
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

    // Priorities are the same numbers on every Windows version this builds for,
    // so they are named here once rather than looked up.
    const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
    const IDLE_PRIORITY_CLASS: u32 = 0x0000_0040;
    const NORMAL_PRIORITY_CLASS: u32 = 0x0000_0020;
    const ABOVE_NORMAL_PRIORITY_CLASS: u32 = 0x0000_8000;
    const HIGH_PRIORITY_CLASS: u32 = 0x0000_0080;
    const REALTIME_PRIORITY_CLASS: u32 = 0x0000_0100;
    const PROCESS_SET_INFORMATION: u32 = 0x0200;
    const PROCESS_QUERY_INFORMATION: u32 = 0x0400;

    extern "system" {
        fn SetPriorityClass(process: *mut c_void, class: u32) -> i32;
    }

    fn priority_class(priority: super::Priority) -> u32 {
        match priority {
            super::Priority::Low => IDLE_PRIORITY_CLASS,
            super::Priority::BelowNormal => BELOW_NORMAL_PRIORITY_CLASS,
            super::Priority::Normal => NORMAL_PRIORITY_CLASS,
            super::Priority::AboveNormal => ABOVE_NORMAL_PRIORITY_CLASS,
            super::Priority::High => REALTIME_PRIORITY_CLASS,
        }
    }

    pub fn priority_supported() -> bool {
        true
    }

    pub fn set_priority(pid: u32, priority: super::Priority) -> Result<(), super::ControlError> {
        let handle = unsafe {
            OpenProcess(
                PROCESS_SET_INFORMATION | PROCESS_QUERY_INFORMATION,
                0,
                pid,
            )
        };
        if handle.is_null() {
            return Err(super::ControlError::NotFound(pid));
        }
        let result = unsafe { SetPriorityClass(handle, priority_class(priority)) };
        unsafe {
            CloseHandle(handle);
        }
        if result == 0 {
            // Moving yourself to a realtime class needs a privilege most
            // sessions do not hold, and asking anyway has to come back as a
            // reason rather than as a silent no-op.
            return Err(super::ControlError::Failed(format!(
                "the process priority could not be changed to {} (error {})",
                priority.label(),
                std::io::Error::last_os_error()
            )));
        }
        Ok(())
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

    // POSIX has no portable way to ask for a scheduling priority, and the nice
    // value is a permission rather than a right: only a privileged process may
    // lower another process's priority. Saying so is better than a call that
    // appears to succeed and does nothing.
    pub fn priority_supported() -> bool {
        false
    }

    pub fn set_priority(
        _pid: u32,
        _priority: super::Priority,
    ) -> Result<(), super::ControlError> {
        Err(super::ControlError::Unsupported("process priority"))
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

    #[test]
    fn the_priority_scale_round_trips_every_level() {
        for level in -2..=2 {
            let priority = Priority::of(level);
            assert_eq!(priority.level(), level, "level {level} round-trips");
        }
        // Anything outside the scale clamps to the nearest end rather than
        // inventing a level the platform has no name for.
        assert_eq!(Priority::of(-99), Priority::Low);
        assert_eq!(Priority::of(99), Priority::High);
    }

    #[test]
    fn every_priority_names_itself_and_says_what_it_does() {
        let all = [
            Priority::Low,
            Priority::BelowNormal,
            Priority::Normal,
            Priority::AboveNormal,
            Priority::High,
        ];
        let labels: Vec<&str> = all.iter().map(|p| p.label()).collect();
        assert_eq!(labels.len(), 5, "{labels:?}");
        for priority in all {
            assert!(!priority.label().is_empty());
            assert!(priority.effect().ends_with('.'), "{}", priority.effect());
        }
        // The middle of the scale has to read as the unremarkable answer, or a
        // user cannot tell which one is the one to leave alone.
        assert_eq!(Priority::Normal.level(), 0);
    }

    #[test]
    fn a_platform_that_cannot_move_a_priority_says_so_rather_than_pretending() {
        // The application asks for a priority whether or not this platform can
        // grant it, so the refusal is the behaviour under test, not a fallback.
        match set_priority(std::process::id(), Priority::AboveNormal) {
            Ok(()) => assert!(Priority::AboveNormal.is_supported()),
            Err(e) => {
                assert!(!Priority::AboveNormal.is_supported(), "{e}");
                assert!(e.to_string().contains("priority"), "{e}");
            }
        }
    }
}
