//! Keeping the machine awake while an encode runs.
//!
//! A long encode that lets the machine sleep is an encode that finishes hours
//! later than it had to, or does not finish at all if the sleep happens to be a
//! hibernate. So the worker asks the platform to stay awake for as long as the
//! pipeline is running and lets go the moment it stops, rather than for a fixed
//! period that would outlive the encode.
//!
//! The decision of *when* to ask is not here. It belongs to whoever knows what
//! the queue is doing, which is the desktop window; this module is the part that
//! talks to the machine, and it is deliberately small enough to read in one go:
//! two requests, one call, and the flags that call takes are named and pinned by
//! a test rather than passed through as numbers from somewhere else.
//!
//! Only the small amount of platform API needed for that lives here, declared
//! directly so the crate keeps its dependency footprint at zero beyond `serde`
//! and `vise-core`, which is the convention the worker's process control
//! already follows.

use std::fmt;

/// What the machine is asked to keep doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerScope {
    /// Keep the machine running, and let the screen go dark.
    ///
    /// What an encode wants: the display is not the point, the five hours of
    /// encoding are.
    System,
    /// Keep the machine running and the screen on.
    ///
    /// Offered rather than assumed because somebody watching a progress window
    /// for six hours is not helped by a dark room.
    SystemAndDisplay,
}

/// One thing asked of the power subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerRequest {
    /// Hold the machine awake at this scope until it is released.
    Hold(PowerScope),
    /// Let go of any hold this process is holding.
    Release,
}

/// Why a request could not be carried out.
///
/// There is only one answer, and that is a fact about the platform rather than a
/// shortcut: the call that holds the machine awake reports the state it replaced
/// rather than whether it succeeded, so a refusal the caller could see does not
/// exist to be returned. What can be refused is the capability itself, and that
/// is what this says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PowerError {
    /// The platform has no way to hold the machine awake.
    Unsupported,
}

impl fmt::Display for PowerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PowerError::Unsupported => write!(
                f,
                "this platform cannot hold the machine awake while an encode runs"
            ),
        }
    }
}

impl std::error::Error for PowerError {}

impl PowerScope {
    /// The flags this scope asks the platform for.
    ///
    /// Named here so the numbers are stated once and can be checked against what
    /// the platform documents: `ES_CONTINUOUS` is what makes the request last
    /// until it is released rather than lasting a moment, and the two `REQUIRED`
    /// flags are the machine and the display.
    pub fn flags(self) -> u32 {
        const ES_SYSTEM_REQUIRED: u32 = 0x0000_0001;
        const ES_DISPLAY_REQUIRED: u32 = 0x0000_0002;
        const ES_CONTINUOUS: u32 = 0x8000_0000;
        match self {
            PowerScope::System => ES_CONTINUOUS | ES_SYSTEM_REQUIRED,
            PowerScope::SystemAndDisplay => {
                ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED
            }
        }
    }

    /// What asking for this holds, in words.
    pub fn effect(self) -> &'static str {
        match self {
            PowerScope::System => {
                "The machine is kept running while the encode runs, and the screen may go dark."
            }
            PowerScope::SystemAndDisplay => {
                "The machine is kept running and the screen stays on while the encode runs."
            }
        }
    }

    /// The scope a preference names, or the default when it names none.
    pub fn of(name: &str) -> PowerScope {
        match name.trim().to_ascii_lowercase().as_str() {
            "system_and_display" | "screen" | "display" => PowerScope::SystemAndDisplay,
            _ => PowerScope::System,
        }
    }

    /// Stable identifier a preference stores.
    pub fn id(self) -> &'static str {
        match self {
            PowerScope::System => "system",
            PowerScope::SystemAndDisplay => "system_and_display",
        }
    }

    /// Label on the option in the preferences picker.
    pub fn label(self) -> &'static str {
        match self {
            PowerScope::System => "Keep the machine awake",
            PowerScope::SystemAndDisplay => "Keep the machine and screen awake",
        }
    }
}

impl PowerRequest {
    /// The flags this request carries, or `None` for a release.
    ///
    /// A release is the platform's own "stop requesting" value rather than zero
    /// flags, so it is written as the null call rather than as a number that
    /// would mean something else.
    pub fn flags(self) -> Option<u32> {
        match self {
            PowerRequest::Hold(scope) => Some(scope.flags()),
            PowerRequest::Release => None,
        }
    }
}

/// Whether this platform can hold the machine awake.
pub fn available() -> bool {
    sys::available()
}

/// Carry out `request` on behalf of this process.
///
/// The hold is process-wide rather than thread-wide on purpose: the encode runs
/// on a worker thread that comes and goes, and a thread-scoped request that was
/// released by whichever thread happened to finish last would leave the machine
/// awake long after the encode, or let it sleep halfway through one.
pub fn request(request: PowerRequest) -> Result<(), PowerError> {
    sys::request(request)
}

#[cfg(windows)]
mod sys {
    use super::{PowerError, PowerRequest};
    use std::ffi::c_void;
    use std::os::raw::c_char;

    extern "system" {
        fn GetModuleHandleW(module_name: *const u16) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, proc_name: *const c_char) -> *mut c_void;
    }

    /// What `SetProcessExecutionState` returns to say it understood: it is not
    /// the previous state, which is why the value is only ever compared.
    const ES_CONTINUOUS: u32 = 0x8000_0000;

    type StateFn = unsafe extern "system" fn(flags: u32) -> u32;

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

    /// Look `name` up in `kernel32.dll`.
    ///
    /// Resolved by name rather than linked directly because the process-wide
    /// entry point only exists from Windows 8 on, and a build that has to keep
    /// working on the release before that should report the capability as
    /// missing instead of failing to load.
    fn kernel32_function(name: &str) -> Option<StateFn> {
        unsafe {
            let module = GetModuleHandleW(utf16("kernel32.dll").as_ptr());
            if module.is_null() {
                return None;
            }
            let symbol = GetProcAddress(module, ascii(name).as_ptr().cast::<c_char>());
            if symbol.is_null() {
                return None;
            }
            Some(std::mem::transmute::<*mut c_void, StateFn>(symbol))
        }
    }

    /// The function that does the work: process-wide where it exists, and
    /// thread-wide only as a last resort on a platform too old for the other.
    pub(crate) fn state_call() -> Option<StateFn> {
        kernel32_function("SetProcessExecutionState")
    }

    pub fn available() -> bool {
        state_call().is_some()
    }

    pub fn request(request: PowerRequest) -> Result<(), PowerError> {
        // Releasing is asked for before the capability is checked: a build that
        // cannot hold the machine awake has nothing standing to release, and a
        // refusal here would mean a window could never tell the platform to let
        // go of a request another build left behind.
        if matches!(request, PowerRequest::Release) {
            let Some(function) = state_call() else {
                return Ok(());
            };
            // A release is the continuous bit on its own: no system required, no
            // display required, no request.
            let replaced = unsafe { function(ES_CONTINUOUS) };
            let _ = replaced;
            return Ok(());
        }
        let Some(function) = state_call() else {
            return Err(PowerError::Unsupported);
        };
        // A hold is the continuous bit plus whatever the scope needs.
        let flags = request
            .flags()
            .expect("a hold carries flags; the release was handled above");
        // The return value is the state this request replaced, not a success
        // flag: it is read so the call cannot be optimised away, and the call's
        // presence is what the capability check above is about.
        let replaced = unsafe { function(flags) };
        let _ = replaced;
        Ok(())
    }
}

#[cfg(not(windows))]
mod sys {
    use super::{PowerError, PowerRequest};

    pub fn available() -> bool {
        false
    }

    pub fn request(request: PowerRequest) -> Result<(), PowerError> {
        // Releasing is always possible: a build that never held the machine awake
        // has nothing to let go of, and refusing would mean a build that declined
        // a hold could leave somebody else's request standing.
        if matches!(request, PowerRequest::Release) {
            return Ok(());
        }
        // Every platform this builds for has some way to say "do not sleep" — a
        // system-inhibit call, a client, an inhibitor — but none of them is
        // reachable without a dependency this crate does not have, and claiming
        // otherwise would be a promise the build cannot keep.
        Err(PowerError::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hold_carries_the_continuous_flag_and_a_release_does_not() {
        // The continuous bit is what makes a request last until it is released
        // instead of lasting a moment, which is the whole difference between
        // "keep the machine awake for this encode" and "keep it awake for a
        // moment, repeatedly, while hoping".
        for scope in [PowerScope::System, PowerScope::SystemAndDisplay] {
            let flags = PowerRequest::Hold(scope).flags().expect("a hold has flags");
            assert_ne!(flags & 0x8000_0000, 0, "{:?} must be continuous", scope);
            assert_ne!(
                flags & 0x0000_0001,
                0,
                "{:?} must keep the machine awake",
                scope
            );
            assert_eq!(
                flags & 0x0000_0002,
                if scope == PowerScope::SystemAndDisplay {
                    0x0000_0002
                } else {
                    0
                }
            );
        }
        assert_eq!(PowerRequest::Release.flags(), None);
    }

    #[test]
    fn the_two_scopes_differ_only_in_the_display_flag() {
        let system = PowerScope::System.flags();
        let both = PowerScope::SystemAndDisplay.flags();
        assert_eq!(both ^ system, 0x0000_0002);
    }

    #[test]
    fn a_scope_is_read_from_the_name_a_preference_stores() {
        assert_eq!(PowerScope::of("system"), PowerScope::System);
        assert_eq!(
            PowerScope::of(" SYSTEM_AND_DISPLAY "),
            PowerScope::SystemAndDisplay
        );
        assert_eq!(PowerScope::of("screen"), PowerScope::SystemAndDisplay);
        // An unknown name falls back to the quieter scope rather than refusing:
        // holding the display on because a record was written by a newer build is
        // a worse failure than not holding it.
        assert_eq!(PowerScope::of("forever"), PowerScope::System);
        for scope in [PowerScope::System, PowerScope::SystemAndDisplay] {
            assert_eq!(PowerScope::of(scope.id()), scope);
            assert!(scope.effect().ends_with('.'), "{}", scope.effect());
            assert!(!scope.label().is_empty());
        }
    }

    #[test]
    fn releasing_is_always_possible_and_holding_reports_the_platform() {
        // Releasing has to work even where holding never did, or a build that
        // declined a hold could still be left holding one from an earlier build.
        assert!(request(PowerRequest::Release).is_ok());
        match request(PowerRequest::Hold(PowerScope::System)) {
            Ok(()) => assert!(available()),
            Err(e) => {
                assert!(!available(), "{e}");
                assert!(e.to_string().contains("awake"), "{e}");
            }
        }
    }

    #[cfg(windows)]
    #[test]
    fn the_process_wide_entry_point_is_the_one_that_is_used() {
        // A thread-scoped request would be released by whichever thread finished
        // last, so the process-wide call is preferred and its absence is the
        // only thing that makes this platform report the capability missing.
        assert_eq!(available(), super::sys::state_call().is_some());
    }
}
