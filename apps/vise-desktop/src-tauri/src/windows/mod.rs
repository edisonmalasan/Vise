//! The secondary windows and dialogs of the desktop shell.
//!
//! Each surface — the queue, the queue selection sheet, the activity log, the
//! static preview, the error view, the countdown alert, the about view, and
//! the preset manager — is described here as one value. The page reads that
//! description to decide what to show, which element takes focus when the
//! surface opens, whether Escape closes it, and which role and modal flag it
//! carries. The page therefore holds no window logic of its own: it cannot
//! open a surface the window does not describe, and it cannot present one with
//! the wrong accessibility semantics.
//!
//! Nothing here talks to the media tools. The queue window reads the session,
//! the log window reads the worker contract, and the preview window asks the
//! worker for stills; all of that is arranged in `main.rs`, and every decision
//! those surfaces make is a plain function or method covered by unit tests.

pub mod dialogs;
pub mod log;
pub mod presets;
pub mod preview;
pub mod queue;

use serde::{Deserialize, Serialize};

/// How a secondary surface is presented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SurfaceKind {
    /// A working surface with its own scrollable body.
    Window,
    /// A short modal that closes on Escape or its own close control.
    Dialog,
}

impl SurfaceKind {
    /// The ARIA role the page puts on the surface.
    ///
    /// Every secondary surface is a dialog as far as assistive technology is
    /// concerned: it is an overlay inside the application window, and the user
    /// is expected to finish with it before returning to the main surface.
    pub fn role(self) -> &'static str {
        match self {
            SurfaceKind::Window => "dialog",
            SurfaceKind::Dialog => "dialog",
        }
    }
}

/// What the page needs to know to present one surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceSpec {
    /// Stable identifier the page uses to pick the body to render.
    pub id: String,
    /// Title on the surface header.
    pub label: String,
    /// One line describing what the surface is for.
    pub hint: String,
    /// Whether it behaves like a window or a short modal.
    pub kind: SurfaceKind,
    /// The ARIA role the page puts on the surface.
    pub role: String,
    /// Whether the surface is a modal overlay for assistive technology.
    pub modal: bool,
    /// Whether Escape closes it.
    pub escape_closes: bool,
    /// Element identifier that takes focus when the surface opens.
    pub initial_focus: String,
    /// Element identifier of the toolbar control that opens the surface.
    ///
    /// The page builds the toolbar from this rather than from a list of its own,
    /// so a surface the window offers always has a control that opens it.
    pub button_id: String,
    /// Whether the toolbar offers it.
    ///
    /// The countdown alert is not: it is opened by the window when a destructive
    /// action asks for confirmation, so a toolbar control for it would let
    /// somebody ask a question about nothing.
    pub openable: bool,
}

impl SurfaceSpec {
    /// A surface of `kind` that Escape closes.
    fn of(kind: SurfaceKind, id: &str, label: &str, hint: &str, initial_focus: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            hint: hint.to_string(),
            kind,
            role: kind.role().to_string(),
            modal: true,
            escape_closes: true,
            initial_focus: initial_focus.to_string(),
            button_id: button_id(id),
            openable: true,
        }
    }

    /// The same surface, with Escape closing it.
    fn holding(id: &str, label: &str, hint: &str, initial_focus: &str) -> Self {
        let mut spec = Self::of(SurfaceKind::Dialog, id, label, hint, initial_focus);
        // Escape must not dismiss this one: a person who pressed Escape by
        // reflex has not read the question, and a refusal they did not mean to
        // give is as bad as a destructive action they did not mean to take.
        spec.escape_closes = false;
        // Nor is it on the toolbar: it exists only while a destructive action
        // is waiting to be confirmed.
        spec.openable = false;
        spec
    }
}

/// The toolbar control that opens the surface named `id`.
///
/// `queue` becomes `openQueueBtn` and `queue-selection` becomes
/// `openQueueSelectionBtn`, so the identifiers are made of the same words the
/// surface is called by.
pub fn button_id(id: &str) -> String {
    let mut name = String::from("open");
    let mut capitalise = true;
    for character in id.chars() {
        if character == '-' {
            capitalise = true;
            continue;
        }
        if capitalise {
            name.extend(character.to_uppercase());
            capitalise = false;
        } else {
            name.push(character);
        }
    }
    name.push_str("Btn");
    name
}

/// Every secondary surface, in the order the toolbar offers them.
pub fn surfaces() -> Vec<SurfaceSpec> {
    vec![
        SurfaceSpec::of(
            SurfaceKind::Window,
            queue::SURFACE,
            "Queue",
            "Every queued job, in the order it runs, with its progress.",
            &format!("{}close", queue::SURFACE),
        ),
        SurfaceSpec::of(
            SurfaceKind::Dialog,
            queue::SELECTION_SURFACE,
            "Queue selection",
            "Choose which titles are queued, and where each one is written.",
            &format!("{}close", queue::SELECTION_SURFACE),
        ),
        SurfaceSpec::of(
            SurfaceKind::Window,
            log::SURFACE,
            "Activity",
            "Every line the window and the worker have written, newest last.",
            &format!("{}close", log::SURFACE),
        ),
        SurfaceSpec::of(
            SurfaceKind::Window,
            preview::SURFACE,
            "Preview",
            "Still frames from the chosen title, with a crop overlay.",
            &format!("{}close", preview::SURFACE),
        ),
        SurfaceSpec::of(
            SurfaceKind::Dialog,
            dialogs::ERROR_SURFACE,
            "Problems",
            "Everything that went wrong, with a copyable report.",
            &format!("{}close", dialogs::ERROR_SURFACE),
        ),
        SurfaceSpec::holding(
            dialogs::ALERT_SURFACE,
            "Are you sure?",
            "A countdown before an action that cannot be undone.",
            // Focus starts on the way out rather than on the confirm control:
            // the confirm control is closed until the countdown reaches zero, and
            // focus that lands on a closed control goes nowhere.
            &format!("{}cancel", dialogs::ALERT_SURFACE),
        ),
        SurfaceSpec::of(
            SurfaceKind::Dialog,
            dialogs::ABOUT_SURFACE,
            "About Vise Transcoder",
            "Version, build, and what this build is made of.",
            &format!("{}close", dialogs::ABOUT_SURFACE),
        ),
        SurfaceSpec::of(
            SurfaceKind::Window,
            presets::SURFACE,
            "Presets",
            "Add, edit, copy, import, export, and retire presets.",
            &format!("{}close", presets::SURFACE),
        ),
    ]
}

/// Every surface the build offers, as the page renders its toolbar from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceCatalogViewModel {
    /// One entry per surface, in the order the toolbar lists them.
    pub surfaces: Vec<SurfaceSpec>,
}

/// Which surfaces are open right now.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfaceStateViewModel {
    /// The open surfaces, in the order they were opened.
    pub open: Vec<SurfaceSpec>,
    /// Where focus goes back to when the front surface closes.
    pub restore_focus: String,
    /// The surface the user last had in front, when one is.
    pub front: String,
}

/// The description of `id`, when it is a surface this build offers.
pub fn spec_of(id: &str) -> Option<SurfaceSpec> {
    surfaces().into_iter().find(|spec| spec.id == id)
}

/// Which surfaces are open, and which one was open before them.
///
/// The remembered surface is what lets the page put focus back where the user
/// left it rather than dropping the caret at the top of the window.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SurfaceState {
    open: Vec<String>,
    previous: String,
}

impl SurfaceState {
    /// The catalog the page renders its toolbar and overlays from.
    pub fn catalog(&self) -> SurfaceCatalogViewModel {
        SurfaceCatalogViewModel {
            surfaces: surfaces(),
        }
    }

    /// What is open right now.
    pub fn view(&self) -> SurfaceStateViewModel {
        SurfaceStateViewModel {
            open: self.open_surfaces(),
            restore_focus: self.restore_focus(),
            front: self.front().unwrap_or_default().to_string(),
        }
    }

    /// Whether `id` names a surface this build offers.
    pub fn is_surface(id: &str) -> bool {
        surfaces().iter().any(|spec| spec.id == id)
    }

    /// Whether `id` is open.
    pub fn is_open(&self, id: &str) -> bool {
        self.open.iter().any(|open| open == id)
    }

    /// Open `id`, remembering what was in front before.
    ///
    /// A name that is not a surface is refused rather than opened as an empty
    /// overlay, so a stale page cannot leave an unusable window open.
    pub fn open(&mut self, id: &str) -> Result<SurfaceSpec, String> {
        let spec = spec_of(id)
            .ok_or_else(|| format!("'{}' is not a window this build offers.", id.trim()))?;
        if self.is_open(id) {
            return Ok(spec);
        }
        if let Some(current) = self.front() {
            self.previous = current.to_string();
        }
        self.open.push(spec.id.clone());
        Ok(spec)
    }

    /// Close `id`, returning whether it had been open.
    pub fn close(&mut self, id: &str) -> bool {
        let before = self.open.len();
        self.open.retain(|open| open != id);
        self.open.len() != before
    }

    /// Close every surface.
    pub fn close_all(&mut self) {
        self.open.clear();
        self.previous.clear();
    }

    /// The surface the user last had in front, when one is still open.
    pub fn front(&self) -> Option<&str> {
        self.open.last().map(String::as_str)
    }

    /// Where focus goes back to when the front surface closes.
    pub fn restore_focus(&self) -> String {
        if self.previous.is_empty() || !self.is_open(&self.previous) {
            return "openQueueBtn".to_string();
        }
        format!("{}close", self.previous)
    }

    /// The open surfaces, in the order they were opened.
    pub fn open_surfaces(&self) -> Vec<SurfaceSpec> {
        surfaces()
            .into_iter()
            .filter(|spec| self.is_open(&spec.id))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_surface_carries_the_focus_and_escape_rules_the_page_needs() {
        let all = surfaces();
        assert_eq!(all.len(), 8);
        for spec in &all {
            assert!(!spec.id.is_empty());
            assert!(!spec.label.is_empty());
            assert!(
                spec.initial_focus.starts_with(&spec.id),
                "{} focuses {}",
                spec.id,
                spec.initial_focus
            );
            assert_eq!(spec.kind.role(), "dialog");
        }
        // The countdown alert holds its own confirm control until the countdown
        // reaches zero, so Escape must not dismiss it as a refusal.
        let alert = spec_of(dialogs::ALERT_SURFACE).expect("alert");
        assert!(!alert.escape_closes);
        for spec in &all {
            if spec.id != dialogs::ALERT_SURFACE {
                assert!(spec.escape_closes, "{}", spec.id);
            }
        }
    }

    #[test]
    fn every_openable_surface_has_one_toolbar_control_and_the_alert_has_none() {
        let all = surfaces();
        let mut buttons: Vec<&str> = Vec::new();
        for spec in &all {
            if !spec.openable {
                assert_eq!(spec.id, dialogs::ALERT_SURFACE, "{}", spec.id);
                continue;
            }
            assert!(spec.button_id.starts_with("open"), "{}", spec.button_id);
            assert!(spec.button_id.ends_with("Btn"), "{}", spec.button_id);
            assert!(
                !buttons.contains(&spec.button_id.as_str()),
                "two surfaces share the control {}",
                spec.button_id
            );
            buttons.push(&spec.button_id);
        }
        assert_eq!(button_id("queue"), "openQueueBtn");
        assert_eq!(button_id("queue-selection"), "openQueueSelectionBtn");
        assert!(all.iter().any(|spec| spec.button_id == "openQueueBtn"));
    }

    #[test]
    fn surfaces_open_and_close_in_the_order_they_were_opened() {
        let mut state = SurfaceState::default();
        assert!(!state.is_open(queue::SURFACE));
        state.open(queue::SURFACE).expect("queue");
        state.open(log::SURFACE).expect("log");
        assert_eq!(state.open_surfaces().len(), 2);
        assert_eq!(state.front(), Some(log::SURFACE));

        assert!(state.close(log::SURFACE));
        assert!(!state.close(log::SURFACE), "closing twice is a no-op");
        assert_eq!(state.front(), Some(queue::SURFACE));
    }

    #[test]
    fn a_surface_this_build_does_not_offer_is_refused_by_name() {
        let mut state = SurfaceState::default();
        let error = state.open("registry editor").expect_err("unknown surface");
        assert!(error.contains("registry editor"), "{error}");
        assert!(state.open_surfaces().is_empty());
        assert!(!SurfaceState::is_surface("registry editor"));
        assert!(SurfaceState::is_surface("about"));
    }

    #[test]
    fn opening_a_surface_that_is_already_open_keeps_the_front_one_in_front() {
        let mut state = SurfaceState::default();
        state.open(queue::SURFACE).expect("queue");
        state.open(log::SURFACE).expect("log");
        state.open(queue::SURFACE).expect("queue again");
        assert_eq!(state.front(), Some(log::SURFACE));
        assert_eq!(state.open_surfaces().len(), 2);
    }

    #[test]
    fn focus_returns_to_the_surface_that_was_in_front_before() {
        let mut state = SurfaceState::default();
        state.open(queue::SURFACE).expect("queue");
        state.open(dialogs::ABOUT_SURFACE).expect("about");
        assert_eq!(state.restore_focus(), format!("{}close", queue::SURFACE));

        state.close(dialogs::ABOUT_SURFACE);
        state.close(queue::SURFACE);
        // Nothing is left to return to, so focus goes to the toolbar control
        // that opens the queue rather than nowhere.
        assert_eq!(state.restore_focus(), "openQueueBtn");

        state.close_all();
        assert!(state.open_surfaces().is_empty());
    }
}
