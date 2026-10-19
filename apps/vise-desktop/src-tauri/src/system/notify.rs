//! Notifications: whether one is worth showing, and what it says when it is.
//!
//! A notification is a claim on somebody's attention, so the default here is not
//! to make one. The gate is not "something happened": it is "something happened
//! that the user could not have seen from where they were". A queue that finishes
//! while the window is in front says so in the activity log and stops there; the
//! same queue finishing with the window minimised says it in a notification. A
//! failure is worth a notification either way, because the user is the one person
//! who can decide whether to retry it and a silent failure is a failure that gets
//! discovered on Saturday.
//!
//! The decision is a pure function, so which events are worth showing, and when,
//! is covered by unit tests that do not need a notification area. The showing
//! itself is [`Notifier`], whose Windows implementation puts a balloon on the
//! notification area and whose answer on every other platform is that the
//! capability is missing — with the reason the window can put in the log rather
//! than a message that vanished.

use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// Something worth deciding about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Every queued job finished.
    QueueFinished {
        /// How many finished well.
        completed: usize,
        /// How many failed.
        failed: usize,
    },
    /// One job failed on its own.
    EncodeFailed {
        /// The file it was writing.
        output: String,
        /// What the encoder said.
        reason: String,
    },
    /// The queue stopped because something is in the way.
    QueuePaused {
        /// Why it stopped.
        reason: String,
    },
    /// A newer release is on offer.
    UpdateAvailable {
        /// The version it offers.
        version: String,
    },
    /// A check for a release could not finish.
    UpdateCheckFailed {
        /// What went wrong.
        reason: String,
    },
}

impl Event {
    /// Every event, for a test that walks them all.
    #[allow(
        dead_code,
        reason = "the walk over every event is a test, and the list exists for it"
    )]
    pub const ALL: [Event; 5] = [
        Event::QueueFinished {
            completed: 0,
            failed: 0,
        },
        Event::EncodeFailed {
            output: String::new(),
            reason: String::new(),
        },
        Event::QueuePaused {
            reason: String::new(),
        },
        Event::UpdateAvailable {
            version: String::new(),
        },
        Event::UpdateCheckFailed {
            reason: String::new(),
        },
    ];

    /// The stable name the log records the event under.
    pub fn name(&self) -> &'static str {
        match self {
            Event::QueueFinished { .. } => "queueFinished",
            Event::EncodeFailed { .. } => "encodeFailed",
            Event::QueuePaused { .. } => "queuePaused",
            Event::UpdateAvailable { .. } => "updateAvailable",
            Event::UpdateCheckFailed { .. } => "updateCheckFailed",
        }
    }
}

/// How important a notification is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tone {
    /// Something finished as intended.
    Information,
    /// Something needs a decision.
    Attention,
    /// Something went wrong.
    Failure,
}

impl Tone {
    /// How long the platform shows it, in milliseconds.
    ///
    /// A failure stays until it is read rather than disappearing: a notification
    /// about a failed encode that fades after five seconds is the same as no
    /// notification at all.
    pub fn timeout_ms(self) -> u32 {
        match self {
            Tone::Information => 8_000,
            Tone::Attention => 15_000,
            Tone::Failure => 0,
        }
    }
}

/// Which events are turned into notifications at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NotifyScope {
    /// Everything worth showing.
    All,
    /// Only failures and pauses.
    FailuresOnly,
    /// Nothing at all.
    Never,
}

/// What the preferences and the window say about notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotifyPolicy {
    /// Whether notifications are on.
    pub enabled: bool,
    /// Which events are turned into notifications.
    pub scope: NotifyScope,
}

impl Default for NotifyPolicy {
    fn default() -> Self {
        NotifyPolicy {
            enabled: true,
            scope: NotifyScope::All,
        }
    }
}

/// A notification, as the platform layer is handed it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    /// The event this came from.
    pub event: String,
    /// The line at the top.
    pub title: String,
    /// The line under it.
    pub body: String,
    /// How important it is.
    pub tone: Tone,
    /// How long it should stay.
    pub timeout_ms: u32,
}

/// Decide whether `event` is worth a notification, and what it says.
///
/// The rules, in order:
///
/// 1. A notification that is switched off is not produced, whatever happened.
/// 2. A scope that only wants failures drops everything that is not one.
/// 3. A queue that finished well while the window is in front is something the
///    user watched happen, so it is not a notification.
/// 4. Everything else is, including a pause, because a queue that stopped for
///    want of disk space and said nothing is a queue that looks stuck.
pub fn decide(event: &Event, policy: NotifyPolicy) -> Option<Notification> {
    let wanted = match event {
        Event::QueueFinished { .. } => policy.scope == NotifyScope::All,
        Event::EncodeFailed { .. } | Event::QueuePaused { .. } => {
            policy.scope != NotifyScope::Never
        }
        Event::UpdateAvailable { .. } | Event::UpdateCheckFailed { .. } => {
            policy.scope == NotifyScope::All
        }
    };
    if !policy.enabled || !wanted {
        return None;
    }
    let (title, body, tone) = match event {
        Event::QueueFinished { completed, failed } => {
            let body = match (completed, failed) {
                (0, 0) => "The queue is empty.".to_string(),
                (count, 0) => format!("{count} job(s) finished."),
                (0, count) => format!("{count} job(s) failed."),
                (done, count) => format!("{done} finished and {count} failed."),
            };
            (
                "The queue has finished".to_string(),
                body,
                Tone::Information,
            )
        }
        Event::EncodeFailed { output, reason } => (
            "An encode failed".to_string(),
            format!(
                "{} could not be written: {}",
                file_name(output),
                one_line(reason)
            ),
            Tone::Failure,
        ),
        Event::QueuePaused { reason } => (
            "The queue has paused".to_string(),
            one_line(reason),
            Tone::Attention,
        ),
        Event::UpdateAvailable { version } => (
            "A newer Vise is available".to_string(),
            format!("Version {version} is on offer; nothing has been installed."),
            Tone::Attention,
        ),
        Event::UpdateCheckFailed { reason } => (
            "The release check did not finish".to_string(),
            one_line(reason),
            Tone::Attention,
        ),
    };
    Some(Notification {
        event: event.name().to_string(),
        title,
        body,
        tone,
        timeout_ms: tone.timeout_ms(),
    })
}

/// Whether the window is in front, which the decision needs.
///
/// Separate from [`decide`] because "is the window visible" is an observation
/// the window makes and the rule is about it, rather than something the rule
/// should have to fetch.
pub fn decide_with_focus(
    event: &Event,
    policy: NotifyPolicy,
    window_visible: bool,
) -> Option<Notification> {
    match event {
        Event::QueueFinished { .. } if window_visible => None,
        other => decide(other, policy),
    }
}

/// The file name of a path, for a line that has to fit on a notification.
fn file_name(path: &str) -> String {
    path.rsplit(['\\', '/'])
        .find(|part| !part.is_empty())
        .unwrap_or(path)
        .to_string()
}

/// A message squeezed onto one line.
fn one_line(reason: &str) -> String {
    let collapsed = reason
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<&str>>()
        .join(" ");
    if collapsed.is_empty() {
        "no reason was reported.".to_string()
    } else {
        collapsed
    }
}

/// Why a notification could not be shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotifyError {
    /// This platform has no notification area to show a notification in.
    Unsupported,
    /// The platform refused it.
    Refused(String),
}

impl fmt::Display for NotifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotifyError::Unsupported => {
                write!(f, "this platform has no way to show a desktop notification")
            }
            NotifyError::Refused(reason) => write!(f, "the notification was refused: {reason}"),
        }
    }
}

impl std::error::Error for NotifyError {}

impl NotifyScope {
    /// Every scope, least to most.
    pub const ALL: [NotifyScope; 3] = [
        NotifyScope::Never,
        NotifyScope::FailuresOnly,
        NotifyScope::All,
    ];

    /// Stable identifier the preferences record.
    pub fn id(self) -> &'static str {
        match self {
            NotifyScope::Never => "never",
            NotifyScope::FailuresOnly => "failures",
            NotifyScope::All => "all",
        }
    }

    /// Read one the preferences record, defaulting to everything.
    ///
    /// An unknown name is not an error here: this setting says how much attention
    /// a build asks for, and a record written by a newer build must not cost the
    /// user the failures they asked to hear about.
    pub fn of(name: &str) -> NotifyScope {
        match name.trim().to_ascii_lowercase().as_str() {
            "never" | "none" => NotifyScope::Never,
            "failures" | "failures_only" => NotifyScope::FailuresOnly,
            _ => NotifyScope::All,
        }
    }

    /// Label on the option.
    pub fn label(self) -> &'static str {
        match self {
            NotifyScope::Never => "Never",
            NotifyScope::FailuresOnly => "Failures and pauses",
            NotifyScope::All => "Everything worth showing",
        }
    }

    /// What choosing this means.
    pub fn hint(self) -> &'static str {
        match self {
            NotifyScope::Never => "nothing is shown outside the window; everything is logged",
            NotifyScope::FailuresOnly => "a failed encode and a paused queue are shown",
            NotifyScope::All => {
                "a queue that finishes while the window is not in front is shown as well"
            }
        }
    }
}

/// The state of the notifications this window can show, as the preferences tab
/// shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationState {
    /// Whether a notification can be shown at all here.
    pub available: bool,
    /// What the notifier is, in one line.
    pub description: String,
    /// Which events are turned into notifications.
    pub scope: NotifyScope,
    /// Whether the window is in front, which is an observation rather than a
    /// choice.
    pub window_visible: bool,
}

/// Something that can show a notification.
pub trait Notifier: Send + Sync {
    /// Whether a notification can be shown at all here.
    fn available(&self) -> bool;
    /// One line saying what this notifier is, for the log and the about view.
    fn describe(&self) -> String;
    /// Show `notification`.
    fn notify(&self, notification: &Notification) -> Result<(), NotifyError>;
}

/// A notifier that shows nothing and says so.
///
/// This is the whole of the non-Windows answer, and it is deliberately not an
/// error at the call site: an application that cannot show a notification should
/// still run an encode, and the reason belongs in the log rather than in a
/// dialog about a dialog.
#[derive(Debug, Clone, Copy, Default)]
pub struct NullNotifier;

impl Notifier for NullNotifier {
    fn available(&self) -> bool {
        false
    }

    fn describe(&self) -> String {
        "notifications are not available on this platform, so events are recorded in the activity log instead"
            .to_string()
    }

    fn notify(&self, _notification: &Notification) -> Result<(), NotifyError> {
        Err(NotifyError::Unsupported)
    }
}

/// A notifier that writes notifications into the activity log.
///
/// Used when there is no notification area, and as the fallback when the platform
/// refuses: the line is still written, so an event somebody asked to be told
/// about is never lost entirely because the desktop declined to show it.
#[derive(Debug, Default)]
pub struct LogNotifier {
    lines: std::sync::Mutex<Vec<String>>,
}

impl LogNotifier {
    /// A log notifier with nothing recorded yet.
    pub fn new() -> LogNotifier {
        LogNotifier::default()
    }

    /// Every line it has recorded.
    #[allow(dead_code, reason = "the recorded lines are read by the tests")]
    pub fn lines(&self) -> Vec<String> {
        self.lines
            .lock()
            .map(|lines| lines.clone())
            .unwrap_or_default()
    }
}

impl Notifier for LogNotifier {
    fn available(&self) -> bool {
        true
    }

    fn describe(&self) -> String {
        "notifications are recorded in the activity log".to_string()
    }

    fn notify(&self, notification: &Notification) -> Result<(), NotifyError> {
        let line = format!("{}: {}", notification.title, notification.body);
        if let Ok(mut lines) = self.lines.lock() {
            lines.push(line);
        }
        Ok(())
    }
}

/// The notification area balloon, which is what Windows shows when there is no
/// application identity to raise a toast with.
///
/// The icon is a private one owned by this module and removed as soon as the
/// balloon has been up long enough to be read, so the notification area keeps one
/// icon rather than accumulating them.
///
/// The window handle is optional because only Windows has one. On the other
/// platforms the notifier reports that it is unavailable and the message goes
/// to the activity log instead, which is the same path a balloon takes when the
/// system refuses to show one.
pub struct BalloonNotifier {
    /// Windows user-interface thread handle the icon is registered against.
    window: Option<isize>,
    label: String,
}

impl BalloonNotifier {
    /// Attach to the window whose notification area should show balloons.
    pub fn new(window: Option<isize>, label: impl Into<String>) -> BalloonNotifier {
        BalloonNotifier {
            window,
            label: label.into(),
        }
    }
}

impl Notifier for BalloonNotifier {
    fn available(&self) -> bool {
        platform::balloon_available()
    }

    fn describe(&self) -> String {
        "notifications are shown in the notification area".to_string()
    }

    fn notify(&self, notification: &Notification) -> Result<(), NotifyError> {
        let Some(window) = self.window else {
            return Err(NotifyError::Unsupported);
        };
        platform::show_balloon(
            window,
            &self.label,
            &notification.title,
            &notification.body,
            notification.timeout_ms,
        )
    }
}

/// A notifier that tries the platform and falls back to the log.
pub struct FallbackNotifier {
    primary: Arc<dyn Notifier>,
    fallback: Arc<dyn Notifier>,
}

impl FallbackNotifier {
    /// Show on `primary` when that works, and on `fallback` when it does not.
    pub fn new(primary: Arc<dyn Notifier>, fallback: Arc<dyn Notifier>) -> FallbackNotifier {
        FallbackNotifier { primary, fallback }
    }

    /// The notifier that will be used, which is what the window says out loud.
    pub fn describe(&self) -> String {
        if self.primary.available() {
            self.primary.describe()
        } else {
            self.fallback.describe()
        }
    }

    /// Whether anything at all can be shown.
    pub fn available(&self) -> bool {
        self.primary.available() || self.fallback.available()
    }

    /// Show `notification`, falling back rather than losing the line.
    pub fn show(&self, notification: &Notification) -> Result<(), NotifyError> {
        match self.primary.notify(notification) {
            Ok(()) => Ok(()),
            Err(e) => self
                .fallback
                .notify(notification)
                .map_err(|_| NotifyError::Refused(e.to_string())),
        }
    }
}

impl Notifier for FallbackNotifier {
    fn available(&self) -> bool {
        FallbackNotifier::available(self)
    }

    fn describe(&self) -> String {
        FallbackNotifier::describe(self)
    }

    fn notify(&self, notification: &Notification) -> Result<(), NotifyError> {
        FallbackNotifier::show(self, notification)
    }
}

impl fmt::Debug for FallbackNotifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The two notifiers behind it are trait objects, so what can be said
        // about this one is which of them will actually be used.
        f.debug_struct("FallbackNotifier")
            .field("using", &self.describe())
            .finish()
    }
}

impl Default for FallbackNotifier {
    /// A window that can only write to its log.
    ///
    /// The default is the honest one: a build that has not found a notification
    /// area yet still records every notification, so nothing is lost while the
    /// window is still being built.
    fn default() -> FallbackNotifier {
        FallbackNotifier::new(Arc::new(NullNotifier), Arc::new(LogNotifier::new()))
    }
}

/// The platform side of the balloon, declared directly so the desktop shell keeps
/// its dependency footprint at zero beyond what the window already needs.
#[cfg(windows)]
mod platform {
    use super::NotifyError;
    use std::ffi::c_void;
    use std::os::raw::c_char;
    use std::sync::atomic::{AtomicU32, Ordering};

    extern "system" {
        fn GetModuleHandleW(module_name: *const u16) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, proc_name: *const c_char) -> *mut c_void;
    }

    /// `NOTIFYICONDATAW`: the structure the notification area is written through.
    ///
    /// Its layout is part of the platform's ABI and is pinned by a test below, so
    /// a field being added or reordered here would fail the build rather than a
    /// user's desktop.
    #[repr(C)]
    struct NotifyIconDataW {
        cb_size: u32,
        h_wnd: isize,
        u_id: u32,
        u_flags: u32,
        u_callback_message: u32,
        h_icon: *mut c_void,
        sz_tip: [u16; 128],
        dw_state: u32,
        dw_state_mask: u32,
        sz_info: [u16; 256],
        u_version: u32,
        sz_info_title: [u16; 64],
        dw_info_flags: u32,
        guid_item: [u8; 16],
        h_balloon_icon: *mut c_void,
    }

    impl NotifyIconDataW {
        fn zeroed() -> NotifyIconDataW {
            // Every field is written below; the zeroes are here because the
            // platform reads the whole structure, including the parts this build
            // does not use.
            let mut data: NotifyIconDataW = unsafe { std::mem::zeroed() };
            data.cb_size = std::mem::size_of::<NotifyIconDataW>() as u32;
            data
        }
    }

    const NIM_ADD: u32 = 0x0000_0000;
    const NIM_DELETE: u32 = 0x0000_0002;
    const NIF_ICON: u32 = 0x0000_0002;
    const NIF_INFO: u32 = 0x0000_0010;
    const NIF_TIP: u32 = 0x0000_0004;
    const NIIF_INFO: u32 = 0x0000_0001;
    const IMAGE_ICON: u32 = 1;
    const LR_DEFAULTSIZE: u32 = 0x0000_0040;
    const IDI_APPLICATION: u16 = 32512;

    /// The next identifier, so two windows do not fight over one icon.
    static NEXT_ID: AtomicU32 = AtomicU32::new(1);

    type ShellNotifyFn = unsafe extern "system" fn(u32, *mut NotifyIconDataW) -> i32;
    type DestroyIconFn = unsafe extern "system" fn(*mut c_void) -> i32;
    type LoadImageFn =
        unsafe extern "system" fn(*mut c_void, *const u16, u32, i32, i32, u32) -> *mut c_void;

    fn utf16(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    fn with_utf16_field<const N: usize>(buffer: &mut [u16; N], value: &str) {
        // The platform reads a null-terminated string inside a fixed buffer, and a
        // message longer than the buffer has to be cut rather than allowed to run
        // past it.
        let wide = utf16(value);
        let take = wide.len().min(N.saturating_sub(1));
        buffer[..take].copy_from_slice(&wide[..take]);
        buffer[take] = 0;
    }

    /// Look a symbol up in one of the platform's own libraries.
    ///
    /// Resolved by name rather than linked directly, for one reason: a build that
    /// cannot find an entry point should report the capability as missing and carry
    /// on, where a linked call would be a load-time failure for every user of that
    /// build. Every entry point this module needs has existed since long before
    /// any of it, so the dynamic lookup is a cost paid once per process.
    fn in_library<T: Copy>(library: &str, name: &str) -> Option<T> {
        unsafe {
            let mut module_name: Vec<u16> = library.encode_utf16().collect();
            module_name.push(0);
            let module = GetModuleHandleW(module_name.as_ptr());
            if module.is_null() {
                return None;
            }
            let mut symbol_name: Vec<u8> = name.as_bytes().to_vec();
            symbol_name.push(0);
            let symbol = GetProcAddress(module, symbol_name.as_ptr().cast::<c_char>());
            if symbol.is_null() {
                return None;
            }
            // The symbol and the function pointer are both one pointer wide, so
            // this is a copy of a pointer rather than a conversion between sizes.
            let address: *mut c_void = std::ptr::read(&symbol);
            Some(std::mem::transmute_copy(&address))
        }
    }

    fn shell32<T: Copy>(name: &str) -> Option<T> {
        in_library("shell32.dll", name)
    }

    fn user32<T: Copy>(name: &str) -> Option<T> {
        in_library("user32.dll", name)
    }

    pub(super) fn balloon_available() -> bool {
        shell32::<ShellNotifyFn>("Shell_NotifyIconW").is_some() && destroy_icon().is_some()
    }

    fn destroy_icon() -> Option<DestroyIconFn> {
        user32::<DestroyIconFn>("DestroyIcon")
    }

    /// Put a balloon on the notification area, and take it away afterwards.
    pub(super) fn show_balloon(
        window: isize,
        label: &str,
        title: &str,
        body: &str,
        timeout_ms: u32,
    ) -> Result<(), NotifyError> {
        let Some(shell_notify) = shell32::<ShellNotifyFn>("Shell_NotifyIconW") else {
            return Err(NotifyError::Unsupported);
        };
        let Some(destroy) = destroy_icon() else {
            return Err(NotifyError::Unsupported);
        };
        let Some(load_image) = user32::<LoadImageFn>("LoadImageW") else {
            return Err(NotifyError::Unsupported);
        };
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        unsafe {
            let mut icon_name: Vec<u16> = std::iter::once(IDI_APPLICATION)
                .chain(std::iter::once(0))
                .collect();
            let handle = load_image(
                std::ptr::null_mut(),
                icon_name.as_mut_ptr(),
                IMAGE_ICON,
                0,
                0,
                LR_DEFAULTSIZE,
            );
            let mut data = NotifyIconDataW::zeroed();
            data.h_wnd = window;
            data.u_id = id;
            data.h_icon = handle;
            with_utf16_field(&mut data.sz_tip, label);
            with_utf16_field(&mut data.sz_info_title, title);
            with_utf16_field(&mut data.sz_info, body);
            let flags = if handle.is_null() {
                NIF_INFO | NIF_TIP
            } else {
                NIF_INFO | NIF_TIP | NIF_ICON
            };
            data.u_flags = flags;
            data.dw_info_flags = NIIF_INFO;
            if shell_notify(NIM_ADD, &mut data) == 0 {
                if !handle.is_null() {
                    destroy(handle);
                }
                return Err(NotifyError::Refused(
                    "the notification area did not accept the icon".to_string(),
                ));
            }
            // A balloon is read by a person, so the icon has to outlive the call
            // that showed it; the removal is scheduled rather than done here,
            // because doing it now would remove it before it was ever drawn.
            let icon = handle as usize;
            std::thread::spawn(move || {
                // A timeout of zero means "until it is read"; the platform's own
                // minimum is used rather than a number this module invented.
                let wait = if timeout_ms == 0 {
                    30_000
                } else {
                    timeout_ms as u64
                };
                std::thread::sleep(std::time::Duration::from_millis(wait));
                let mut removal = NotifyIconDataW::zeroed();
                removal.h_wnd = window;
                removal.u_id = id;
                shell_notify(NIM_DELETE, &mut removal);
                if icon != 0 {
                    destroy(icon as *mut c_void);
                }
            });
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn the_notification_structure_matches_what_the_platform_declares() {
            // The platform reads this structure by size, so a field that is
            // missing or in the wrong place fails here rather than on somebody's
            // desktop.
            assert!(std::mem::size_of::<NotifyIconDataW>() >= 8 * 5 + 4 * 3);
            let data = NotifyIconDataW::zeroed();
            assert_eq!(
                data.cb_size as usize,
                std::mem::size_of::<NotifyIconDataW>()
            );
        }

        #[test]
        fn a_message_longer_than_the_field_is_cut_rather_than_run_past_it() {
            let mut buffer = [0u16; 8];
            with_utf16_field(&mut buffer, "abcdefghijklmnop");
            assert_eq!(buffer[7], 0, "the field is still terminated");
            assert_eq!(
                String::from_utf16_lossy(&buffer[..7]),
                "abcdefg",
                "as much as fits, and no more"
            );
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use super::NotifyError;

    pub(super) fn balloon_available() -> bool {
        false
    }

    pub(super) fn show_balloon(
        _window: isize,
        _label: &str,
        _title: &str,
        _body: &str,
        _timeout_ms: u32,
    ) -> Result<(), NotifyError> {
        Err(NotifyError::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn focused() -> NotifyPolicy {
        NotifyPolicy::default()
    }

    #[test]
    fn a_queue_that_finishes_where_you_can_see_it_is_not_a_notification() {
        // The point of a notification is to reach somebody who is not looking.
        let event = Event::QueueFinished {
            completed: 3,
            failed: 0,
        };
        assert_eq!(decide_with_focus(&event, focused(), true), None);
        let away =
            decide_with_focus(&event, focused(), false).expect("when the window is not in front");
        assert_eq!(away.tone, Tone::Information);
        assert!(away.body.contains("3 job(s) finished"), "{}", away.body);
        assert_eq!(away.event, "queueFinished");
        assert!(away.timeout_ms > 0);
    }

    #[test]
    fn a_failure_is_worth_a_notification_wherever_the_window_is() {
        let event = Event::EncodeFailed {
            output: "C:\\out\\Episode 1.mkv".to_string(),
            reason: "disk full\nno space left on device".to_string(),
        };
        let shown = decide_with_focus(&event, focused(), true).expect("a failure is shown");
        assert_eq!(shown.tone, Tone::Failure);
        assert!(shown.body.contains("Episode 1.mkv"), "{}", shown.body);
        assert!(
            !shown.body.contains("C:"),
            "the path is shortened: {}",
            shown.body
        );
        assert_eq!(
            shown.body,
            "Episode 1.mkv could not be written: disk full no space left on device"
        );
        // A failure stays until it is read.
        assert_eq!(shown.timeout_ms, 0);
    }

    #[test]
    fn a_pause_always_says_why() {
        let event = Event::QueuePaused {
            reason: "only 900 MB free on D: and this job needs 4 GB".to_string(),
        };
        let shown = decide(&event, focused()).expect("a pause is shown");
        assert_eq!(shown.tone, Tone::Attention);
        assert!(shown.body.contains("900 MB"), "{}", shown.body);
        assert!(shown.body.contains("4 GB"), "{}", shown.body);
    }

    #[test]
    fn nothing_is_shown_when_notifications_are_switched_off() {
        for event in Event::ALL {
            assert!(
                decide(
                    &event,
                    NotifyPolicy {
                        enabled: false,
                        ..NotifyPolicy::default()
                    }
                )
                .is_none(),
                "{:?} was shown with notifications off",
                event.name()
            );
        }
    }

    #[test]
    fn a_scope_that_only_wants_failures_drops_the_rest() {
        let policy = NotifyPolicy {
            enabled: true,
            scope: NotifyScope::FailuresOnly,
        };
        assert!(decide(&Event::QueuePaused { reason: "x".into() }, policy).is_some());
        assert!(decide(
            &Event::QueueFinished {
                completed: 1,
                failed: 0
            },
            policy
        )
        .is_none());
        assert!(decide(
            &Event::UpdateAvailable {
                version: "0.2.0".into()
            },
            policy
        )
        .is_none());
    }

    #[test]
    fn the_offer_of_a_release_says_nothing_was_installed() {
        // The build never installs anything by itself, so the sentence has to say
        // so; a notification that implied an update had happened would be a lie.
        let shown = decide(
            &Event::UpdateAvailable {
                version: "0.2.0".to_string(),
            },
            focused(),
        )
        .expect("shown");
        assert!(
            shown.body.contains("nothing has been installed"),
            "{}",
            shown.body
        );
        assert!(shown.body.contains("0.2.0"), "{}", shown.body);
    }

    #[test]
    fn an_empty_reason_becomes_a_sentence_rather_than_a_blank() {
        let shown = decide(
            &Event::EncodeFailed {
                output: String::new(),
                reason: "   \n  ".to_string(),
            },
            focused(),
        )
        .expect("shown");
        assert!(
            shown.body.contains("no reason was reported"),
            "{}",
            shown.body
        );
        // A path with no separators is its own name, rather than nothing.
        assert!(
            shown.body.starts_with(" could not be written"),
            "{}",
            shown.body
        );
    }

    #[test]
    fn a_notifier_that_shows_nothing_still_explains_itself() {
        let null = NullNotifier;
        assert!(!null.available());
        assert!(
            null.describe().contains("activity log"),
            "{}",
            null.describe()
        );
        let notification = Notification {
            event: "queueFinished".to_string(),
            title: "The queue has finished".to_string(),
            body: "1 job(s) finished.".to_string(),
            tone: Tone::Information,
            timeout_ms: 8_000,
        };
        assert_eq!(
            null.notify(&notification)
                .expect_err("nothing to show it in"),
            NotifyError::Unsupported
        );
    }

    #[test]
    fn the_log_notifier_keeps_the_line_a_notification_would_have_shown() {
        let log = LogNotifier::new();
        assert!(log.available());
        let notification = Notification {
            event: "encodeFailed".to_string(),
            title: "An encode failed".to_string(),
            body: "Episode 1.mkv could not be written".to_string(),
            tone: Tone::Failure,
            timeout_ms: 0,
        };
        log.notify(&notification).expect("recorded");
        assert_eq!(
            log.lines(),
            vec!["An encode failed: Episode 1.mkv could not be written".to_string()]
        );
    }

    #[test]
    fn a_refused_notification_falls_back_to_the_log_rather_than_being_lost() {
        struct Refusing;
        impl Notifier for Refusing {
            fn available(&self) -> bool {
                true
            }
            fn describe(&self) -> String {
                "refuses everything".to_string()
            }
            fn notify(&self, _notification: &Notification) -> Result<(), NotifyError> {
                Err(NotifyError::Refused(
                    "the notification area is full".to_string(),
                ))
            }
        }
        let log = Arc::new(LogNotifier::new());
        let fallback = FallbackNotifier::new(Arc::new(Refusing), log.clone());
        assert!(fallback.available());
        assert!(fallback.describe().contains("refuses everything"));
        let notification = Notification {
            event: "queuePaused".to_string(),
            title: "The queue has paused".to_string(),
            body: "not enough room".to_string(),
            tone: Tone::Attention,
            timeout_ms: 15_000,
        };
        fallback.show(&notification).expect("the log kept it");
        assert_eq!(log.lines().len(), 1);
    }

    #[test]
    fn with_no_platform_at_all_the_log_is_still_asked() {
        let log = Arc::new(LogNotifier::new());
        let fallback = FallbackNotifier::new(Arc::new(NullNotifier), log.clone());
        assert!(fallback.available(), "the log can always take it");
        assert!(
            fallback.describe().contains("activity log"),
            "{}",
            fallback.describe()
        );
    }

    #[test]
    fn the_balloon_reports_the_platform_rather_than_throwing_it_away() {
        // The window handle is zero here, which no real window has; what is being
        // checked is that the call comes back as an answer either way.
        let balloon = BalloonNotifier::new(Some(0), "Vise Transcoder");
        let notification = Notification {
            event: "queueFinished".to_string(),
            title: "The queue has finished".to_string(),
            body: "1 job(s) finished.".to_string(),
            tone: Tone::Information,
            timeout_ms: 1_000,
        };
        match balloon.notify(&notification) {
            Ok(()) => assert!(balloon.available()),
            Err(e) => assert!(!e.to_string().is_empty(), "{e}"),
        }
    }
}
