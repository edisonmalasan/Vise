//! The static preview window.
//!
//! A scan can be asked for a number of still frames per title; this window is
//! what those frames are for. It holds the still list, moves along it, asks the
//! worker for one frame at a time, and offers a crop rectangle drawn over the
//! frame that is wired straight into the dimensions tab.
//!
//! The crop overlay is the interesting part. The page draws a rectangle in
//! whatever units the displayed image happens to be; the window converts it to
//! source pixels against the reported frame size, checks that it sits inside
//! the frame and has even sides, and then writes it into the dimensions
//! settings. So the rectangle that gets encoded is the rectangle that was
//! checked, not a second guess made downstream.

use serde::{Deserialize, Serialize};

use vise_ipc::contract::PreviewCommand;

/// Identifier of the preview surface.
pub const SURFACE: &str = "preview";

/// The most stills a scan will extract for one title.
pub const MAX_STILLS: u32 = 32;

/// Where in the title a still frame sits, as the page reports a drag or a
/// slider move. Values are fractions of the duration so the control works
/// before the duration is known.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StillPosition {
    /// Fraction of the title, 0.0 to 1.0.
    pub fraction: f64,
}

impl StillPosition {
    /// A position at `fraction` of the title, clamped to the title.
    pub fn of(fraction: f64) -> Self {
        Self {
            fraction: fraction.clamp(0.0, 1.0),
        }
    }
}

/// The crop rectangle as the page reports it, in fractions of the frame.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CropOverlay {
    /// Left edge, as a fraction of the frame width.
    pub left: f64,
    /// Top edge, as a fraction of the frame height.
    pub top: f64,
    /// Width, as a fraction of the frame width.
    pub width: f64,
    /// Height, as a fraction of the frame height.
    pub height: f64,
}

impl CropOverlay {
    /// A rectangle covering the whole frame.
    pub fn whole() -> Self {
        Self {
            left: 0.0,
            top: 0.0,
            width: 1.0,
            height: 1.0,
        }
    }

    /// Whether the rectangle is usable as drawn, before it is scaled.
    ///
    /// The fractions have to be finite, inside the frame, and describe a region
    /// with some area; anything else would produce a crop the dimensions tab
    /// would refuse, so it is caught here where the user can see why.
    pub fn usable(&self) -> bool {
        let parts = [self.left, self.top, self.width, self.height];
        if parts.iter().any(|part| !part.is_finite()) {
            return false;
        }
        self.left >= 0.0
            && self.top >= 0.0
            && self.width > 0.0
            && self.height > 0.0
            && self.left + self.width <= 1.0 + f64::EPSILON
            && self.top + self.height <= 1.0 + f64::EPSILON
    }

    /// The same rectangle in source pixels inside a `width` by `height` frame.
    ///
    /// Sides are rounded down to even numbers because the encoders this build
    /// drives refuse an odd chroma plane.
    pub fn in_pixels(self, width: u32, height: u32) -> Result<(u32, u32, u32, u32), String> {
        if width == 0 || height == 0 {
            return Err(
                "This title reports no frame size, so there is nothing to crop.".to_string(),
            );
        }
        if !self.usable() {
            return Err("The crop rectangle has to sit inside the frame.".to_string());
        }
        let even = |value: f64| -> u32 {
            let whole = (value.round() as i64) & !1;
            whole.max(2) as u32
        };
        let left = ((self.left * f64::from(width)).round() as i64).max(0) as u32;
        let top = ((self.top * f64::from(height)).round() as i64).max(0) as u32;
        let crop_width = even(self.width * f64::from(width));
        let crop_height = even(self.height * f64::from(height));
        if left + crop_width > width || top + crop_height > height {
            return Err("The crop rectangle reaches past the edge of the frame.".to_string());
        }
        Ok((left, top, crop_width, crop_height))
    }
}

/// One stop on the still slider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StillViewModel {
    /// Position of this stop among the stills, counting from zero.
    pub number: u32,
    /// Where in the title it sits, in seconds.
    pub seconds: f64,
    /// Where in the title it sits, as a fraction of the whole.
    pub fraction: f64,
    /// Where in the title it sits, as `h:mm:ss`.
    pub label: String,
    /// Whether this is the still being shown.
    pub current: bool,
}

/// The crop overlay, as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CropOverlayViewModel {
    /// Reported frame width.
    pub frame_width: u32,
    /// Reported frame height.
    pub frame_height: u32,
    /// Left edge in source pixels.
    pub left: u32,
    /// Top edge in source pixels.
    pub top: u32,
    /// Width in source pixels.
    pub width: u32,
    /// Height in source pixels.
    pub height: u32,
    /// Whether the rectangle is one this build can hand to the encoder.
    pub valid: bool,
    /// Why it is not, when it is not.
    pub problem: String,
    /// Whether applying it would change the dimensions tab.
    pub applies: bool,
    /// One line for the hint under the overlay.
    pub summary: String,
}

impl CropOverlayViewModel {
    /// The overlay, blocked with a reason the user can act on.
    fn blocked(frame: (u32, u32), problem: impl Into<String>) -> Self {
        let problem = problem.into();
        Self {
            frame_width: frame.0,
            frame_height: frame.1,
            left: 0,
            top: 0,
            width: 0,
            height: 0,
            valid: false,
            applies: false,
            summary: problem.clone(),
            problem,
        }
    }
}

/// The still frame on screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StillImageViewModel {
    /// Which still this is.
    pub number: u32,
    /// Where it sits in the title.
    pub label: String,
    /// Size of the encoded image, for the caption.
    pub byte_length: u64,
    /// The media type of the image bytes.
    ///
    /// The worker writes its stills as JPEG files, and the page cannot tell
    /// that from the bytes, so the type travels with the frame.
    pub media_type: String,
    /// The image itself, base64, ready for an `img` element.
    pub data_base64: String,
}

/// The media type of a still the worker produced.
pub const STILL_MEDIA_TYPE: &str = "image/jpeg";

/// One control in the preview window, with its availability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewControlViewModel {
    /// Element identifier.
    pub id: String,
    /// Label on the control.
    pub label: String,
    /// Whether it can be used right now.
    pub available: bool,
    /// Why it cannot, when it cannot.
    pub reason: String,
}

impl PreviewControlViewModel {
    /// A control that is on offer.
    pub fn open(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            available: true,
            reason: String::new(),
        }
    }

    /// The control, unavailable with a reason the user can act on.
    pub fn blocked(mut self, reason: impl Into<String>) -> Self {
        self.available = false;
        self.reason = reason.into();
        self
    }

    /// The control, closed with a reason, or on offer when the reason is empty.
    fn closed(mut self, reason: &str) -> Self {
        if !reason.trim().is_empty() {
            self.available = false;
            self.reason = reason.to_string();
        }
        self
    }
}

/// The preview window, as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewViewModel {
    /// Whether a source and title are loaded at all.
    pub has_source: bool,
    /// The source the stills come from.
    pub source_label: String,
    /// The title the stills come from.
    pub title_label: String,
    /// Duration of the title, in seconds.
    pub duration_seconds: f64,
    /// How many stills the scan was asked for.
    pub still_count: u32,
    /// Where in the still list the window is.
    pub still_number: u32,
    /// The frame on screen.
    pub image: Option<StillImageViewModel>,
    /// Every stop on the slider.
    pub stills: Vec<StillViewModel>,
    /// Where the window is along the slider.
    pub position: StillPosition,
    /// The rectangle drawn over the frame, in fractions.
    pub overlay: CropOverlay,
    /// Whether the slider can be moved at all.
    pub scrub_available: bool,
    /// Why the slider is closed, when it is.
    pub scrub_reason: String,
    /// The crop overlay.
    pub crop: CropOverlayViewModel,
    /// The still-count control.
    pub still_count_control: PreviewControlViewModel,
    /// The previous-still control.
    pub previous: PreviewControlViewModel,
    /// The next-still control.
    pub next: PreviewControlViewModel,
    /// The apply-crop control.
    pub apply_crop: PreviewControlViewModel,
    /// The rescan control.
    pub rescan: PreviewControlViewModel,
    /// One line summarising the window.
    pub summary: String,
    /// Whether the frame area should show its empty message.
    pub empty: bool,
    /// The empty message.
    pub empty_label: String,
}

/// What the window is looking at.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PreviewState {
    /// How many stills the next scan should extract.
    still_count: u32,
    /// Which still is on screen.
    still_number: u32,
    /// Duration of the chosen title.
    duration_seconds: f64,
    /// Reported frame size of the chosen title.
    frame: (u32, u32),
    /// The rectangle drawn over the frame.
    overlay: CropOverlay,
    /// The last frame the worker returned.
    image: Option<StillImage>,
    /// Whether a still has been asked for since the title changed.
    loaded: bool,
    /// Why the last fetch produced nothing.
    failure: Option<String>,
}

/// One still, as the worker returned it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StillImage {
    /// Which still this is.
    pub number: u32,
    /// Size of the encoded image.
    pub byte_length: u64,
    /// The image bytes, base64.
    pub data_base64: String,
}

impl Default for PreviewState {
    fn default() -> Self {
        Self {
            still_count: 10,
            still_number: 0,
            duration_seconds: 0.0,
            frame: (0, 0),
            overlay: CropOverlay::whole(),
            image: None,
            loaded: false,
            failure: None,
        }
    }
}

impl PreviewState {
    /// A preview window that extracts `still_count` stills on the next scan.
    ///
    /// Taken from the General preferences at start-up, so the count a user chose
    /// once is the count the first preview they open uses, rather than the count
    /// this struct happens to default to.
    pub fn with_still_count(still_count: u32) -> Self {
        Self {
            still_count,
            ..Self::default()
        }
    }

    /// How many stills the next scan should extract.
    pub fn still_count(&self) -> u32 {
        self.still_count
    }

    /// Which still is on screen.
    pub fn still_number(&self) -> u32 {
        self.still_number
    }

    /// The title the window is looking at, as a duration and a frame size.
    pub fn describes(&self) -> (f64, (u32, u32)) {
        (self.duration_seconds, self.frame)
    }

    /// Point the window at a title, forgetting the previous frame.
    pub fn use_title(&mut self, duration_seconds: f64, frame: (u32, u32)) {
        self.duration_seconds = duration_seconds;
        self.frame = frame;
        self.still_number = 0;
        self.overlay = CropOverlay::whole();
        self.image = None;
        self.loaded = false;
        self.failure = None;
    }

    /// Ask for a different number of stills on the next scan.
    ///
    /// Zero is allowed and means "no previews"; anything above the ceiling is
    /// refused rather than clamped, because a silent clamp would leave the user
    /// waiting for frames that will never arrive.
    pub fn set_still_count(&mut self, count: u32) -> Result<(), String> {
        if count > MAX_STILLS {
            return Err(format!(
                "Ask for at most {MAX_STILLS} stills per title, not {count}."
            ));
        }
        self.still_count = count;
        Ok(())
    }

    /// The seconds a still number sits at, spread evenly across the title.
    pub fn seconds_for(&self, number: u32) -> f64 {
        if self.still_count <= 1 || self.duration_seconds <= 0.0 {
            return 0.0;
        }
        // Stills are spread so the first and last frames are both included,
        // which is what makes the last one useful for spotting a truncated end.
        let steps = f64::from(self.still_count - 1);
        let step = number as f64 / steps;
        (step * self.duration_seconds).clamp(0.0, self.duration_seconds)
    }

    /// The fraction a still number sits at.
    pub fn fraction_for(&self, number: u32) -> f64 {
        if self.still_count <= 1 {
            return 0.0;
        }
        let steps = f64::from(self.still_count - 1);
        f64::from(number) / steps
    }

    /// Every stop on the slider.
    pub fn stills(&self) -> Vec<StillViewModel> {
        (0..self.still_count)
            .map(|number| {
                let seconds = self.seconds_for(number);
                StillViewModel {
                    number,
                    seconds,
                    fraction: self.fraction_for(number),
                    label: format!("{} · {}", number + 1, seconds_label(seconds)),
                    current: number == self.still_number,
                }
            })
            .collect()
    }

    /// Move to still `number`, refusing a stop that is not on the slider.
    pub fn select(&mut self, number: u32) -> Result<u32, String> {
        if number >= self.still_count {
            return Err(format!(
                "There is no still {} in a list of {}.",
                number + 1,
                self.still_count
            ));
        }
        self.still_number = number;
        self.loaded = false;
        self.failure = None;
        Ok(number)
    }

    /// Move by `delta` stops, staying inside the slider.
    pub fn step(&mut self, delta: i32) -> u32 {
        let last = self.still_count.saturating_sub(1) as i64;
        let wanted = (self.still_number as i64 + i64::from(delta)).clamp(0, last);
        self.still_number = wanted.max(0) as u32;
        self.loaded = false;
        self.failure = None;
        self.still_number
    }

    /// Move to the still at `position`.
    pub fn scrub(&mut self, position: StillPosition) -> Result<u32, String> {
        if self.still_count == 0 {
            return Err("This scan was asked for no stills.".to_string());
        }
        let last = f64::from(self.still_count - 1);
        let number = (position.fraction.clamp(0.0, 1.0) * last).round() as u32;
        self.select(number.min(self.still_count - 1))
    }

    /// Whether a still is on screen for the chosen title.
    pub fn has_image(&self) -> bool {
        self.loaded && self.image.is_some()
    }

    /// Record the frame the worker returned for the current still.
    pub fn accept_image(&mut self, image: StillImage) {
        self.image = Some(image);
        self.loaded = true;
        self.failure = None;
    }

    /// Record that the worker could not produce the frame.
    ///
    /// The reason is kept so the window can say why the frame area is empty
    /// rather than leaving the user with a blank rectangle.
    pub fn fail_image(&mut self, message: impl Into<String>) {
        self.image = None;
        self.loaded = false;
        self.failure = Some(message.into());
    }

    /// Move the rectangle, refusing one that cannot be used.
    pub fn set_overlay(&mut self, overlay: CropOverlay) -> Result<(), String> {
        if !overlay.usable() {
            return Err("The crop rectangle has to sit inside the frame.".to_string());
        }
        overlay.in_pixels(self.frame.0, self.frame.1)?;
        self.overlay = overlay;
        Ok(())
    }

    /// Reset the rectangle to the whole frame.
    pub fn reset_overlay(&mut self) {
        self.overlay = CropOverlay::whole();
    }

    /// The crop this rectangle describes, in the form the dimensions tab
    /// writes.
    pub fn crop_rect(&self) -> Result<Option<(u32, u32, u32, u32)>, String> {
        let pixels = self.overlay.in_pixels(self.frame.0, self.frame.1)?;
        let whole = self.frame.0 > 0
            && self.frame.1 > 0
            && pixels == (0, 0, self.frame.0 & !1, self.frame.1 & !1);
        Ok(if whole { None } else { Some(pixels) })
    }

    /// The command that fetches the current still from the worker.
    pub fn command(&self, path: &str, title_index: u32) -> PreviewCommand {
        PreviewCommand {
            path: path.to_string(),
            title_index,
            time_seconds: self.seconds_for(self.still_number),
            output_file: String::new(),
        }
    }

    /// Everything the preview window shows.
    pub fn view(
        &self,
        has_source: bool,
        source_label: &str,
        title_label: &str,
    ) -> PreviewViewModel {
        let crop = match self.overlay.in_pixels(self.frame.0, self.frame.1) {
            Ok((left, top, width, height)) => CropOverlayViewModel {
                frame_width: self.frame.0,
                frame_height: self.frame.1,
                left,
                top,
                width,
                height,
                valid: true,
                problem: String::new(),
                applies: self.crop_rect().ok().flatten().is_some(),
                summary: format!(
                    "Crop {left}, {top} · {width} by {height} of {}x{}.",
                    self.frame.0, self.frame.1
                ),
            },
            Err(problem) => CropOverlayViewModel::blocked(self.frame, problem),
        };
        let has_stills = self.still_count > 0;
        let scrub_reason = if !has_source {
            "Scan a source first.".to_string()
        } else if !has_stills {
            "This scan was asked for no stills; ask for a few and scan again.".to_string()
        } else {
            String::new()
        };
        let stills = self.stills();
        let image = self.image.as_ref().map(|image| StillImageViewModel {
            number: image.number,
            label: stills
                .iter()
                .find(|still| still.number == image.number)
                .map(|still| still.label.clone())
                .unwrap_or_else(|| format!("Still {}", image.number + 1)),
            byte_length: image.byte_length,
            media_type: STILL_MEDIA_TYPE.to_string(),
            data_base64: image.data_base64.clone(),
        });
        let back_reason = stop_reason(&stills, -1);
        let forward_reason = stop_reason(&stills, 1);
        let previous =
            PreviewControlViewModel::open("previewPrevious", "Previous still").closed(&back_reason);
        let next =
            PreviewControlViewModel::open("previewNext", "Next still").closed(&forward_reason);
        PreviewViewModel {
            has_source,
            source_label: source_label.to_string(),
            title_label: title_label.to_string(),
            duration_seconds: self.duration_seconds,
            still_count: self.still_count,
            still_number: self.still_number,
            position: StillPosition::of(self.fraction_for(self.still_number)),
            overlay: self.overlay,
            image,
            stills,
            scrub_available: scrub_reason.is_empty(),
            scrub_reason,
            still_count_control: PreviewControlViewModel::open(
                "previewStillCount",
                "Stills per title",
            ),
            previous,
            next,
            apply_crop: if crop.valid && crop.applies {
                PreviewControlViewModel::open("previewApplyCrop", "Use this crop")
            } else {
                PreviewControlViewModel::open("previewApplyCrop", "Use this crop")
                    .blocked(crop_apply_reason(crop.valid, crop.applies))
            },
            rescan: PreviewControlViewModel::open("previewRescan", "Scan again with these stills")
                .closed(if has_source {
                    ""
                } else {
                    "Scan a source first."
                }),
            summary: if !has_source {
                "Scan a source to see its frames.".to_string()
            } else if !has_stills {
                "This source was scanned without preview stills.".to_string()
            } else {
                format!(
                    "Still {} of {} · {}",
                    self.still_number + 1,
                    self.still_count,
                    seconds_label(self.seconds_for(self.still_number))
                )
            },
            empty: !self.has_image(),
            empty_label: match self.failure.as_deref() {
                Some(reason) if !reason.trim().is_empty() => reason.to_string(),
                _ if !has_source => "No source is loaded.".to_string(),
                _ if !has_stills => "Ask for a few stills, then scan again.".to_string(),
                _ => "This still has not been fetched yet.".to_string(),
            },
            crop,
        }
    }
}

/// Why the previous or next control is closed at this stop.
fn stop_reason(stills: &[StillViewModel], delta: i32) -> String {
    if stills.is_empty() {
        return "This scan was asked for no stills.".to_string();
    }
    let current = match stills.iter().find(|still| still.current) {
        Some(current) => current.number as i64,
        None => return "Fetch a still first.".to_string(),
    };
    let last = stills.len() as i64 - 1;
    let wanted = current + i64::from(delta);
    if wanted < 0 {
        return "This is the first still.".to_string();
    }
    if wanted > last {
        return "This is the last still.".to_string();
    }
    String::new()
}

/// Why applying the crop is or is not on offer.
fn crop_apply_reason(valid: bool, applies: bool) -> &'static str {
    if !valid {
        return "The crop rectangle has to sit inside the frame.";
    }
    if applies {
        return "";
    }
    "The rectangle covers the whole frame, so there is nothing to crop."
}

/// A duration as `h:mm:ss`, using the same formatting the title list uses.
pub fn seconds_label(seconds: f64) -> String {
    crate::viewmodel::format_duration(seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(duration: f64, frame: (u32, u32), stills: u32) -> PreviewState {
        let mut state = PreviewState::default();
        state.use_title(duration, frame);
        state.set_still_count(stills).expect("stills");
        state
    }

    #[test]
    fn stills_are_spread_across_the_title_including_both_ends() {
        let state = state(3600.0, (1920, 1080), 5);
        assert_eq!(state.seconds_for(0), 0.0);
        assert_eq!(state.seconds_for(4), 3600.0);
        assert_eq!(state.seconds_for(1), 900.0);
        let stills = state.stills();
        assert_eq!(stills.len(), 5);
        assert_eq!(stills[0].label, "1 · 0:00");
        assert_eq!(stills[4].label, "5 · 1:00:00");
        assert!(stills[0].current);
        assert_eq!(state.fraction_for(4), 1.0);
    }

    #[test]
    fn a_single_still_sits_at_the_start_and_a_title_of_no_length_falls_back_to_zero() {
        let one = state(3600.0, (640, 360), 1);
        assert_eq!(one.seconds_for(0), 0.0);
        assert_eq!(one.fraction_for(0), 0.0);

        let silent = state(0.0, (640, 360), 3);
        assert_eq!(silent.seconds_for(2), 0.0);
        assert_eq!(seconds_label(f64::NAN), "0:00");
        assert_eq!(seconds_label(-4.0), "0:00");
    }

    #[test]
    fn a_still_count_this_build_will_not_extract_is_refused_rather_than_clamped() {
        let mut state = PreviewState::default();
        let error = state.set_still_count(MAX_STILLS + 1).expect_err("too many");
        assert!(error.contains("at most 32"), "{error}");
        assert_eq!(state.still_count(), 10, "the old count stands");
        state.set_still_count(0).expect("no stills");
        assert_eq!(state.still_count(), 0);
        assert!(state
            .scrub(StillPosition::of(0.5))
            .expect_err("no stills")
            .contains("no stills"));
    }

    #[test]
    fn the_slider_moves_and_refuses_a_stop_that_is_not_on_it() {
        let mut state = state(600.0, (1280, 720), 4);
        assert_eq!(state.step(1), 1);
        assert_eq!(state.step(1), 2);
        assert_eq!(state.step(-5), 0, "it stops at the first still");
        assert_eq!(state.step(99), 3, "and at the last");
        assert!(state
            .select(4)
            .expect_err("past the end")
            .contains("no still 5"));
        assert_eq!(state.still_number, 3, "the last stop stands");

        assert_eq!(state.scrub(StillPosition::of(0.5)).expect("middle"), 2);
        assert_eq!(state.scrub(StillPosition::of(9.0)).expect("past"), 3);
        assert_eq!(state.scrub(StillPosition::of(-1.0)).expect("before"), 0);
    }

    #[test]
    fn changing_the_title_forgets_the_previous_frame() {
        let mut state = state(600.0, (1280, 720), 4);
        state.select(2).expect("still");
        state.accept_image(StillImage {
            number: 2,
            byte_length: 1024,
            data_base64: "AAA".into(),
        });
        assert!(state.has_image());

        state.use_title(1200.0, (1920, 1080));
        assert!(!state.has_image());
        assert_eq!(state.still_number, 0);
        assert_eq!(state.frame, (1920, 1080));
    }

    #[test]
    fn the_overlay_is_scaled_to_source_pixels_and_even_sides() {
        let state = state(600.0, (1920, 1080), 4);
        let mut state = state;
        state
            .set_overlay(CropOverlay {
                left: 0.1,
                top: 0.1,
                width: 0.5,
                height: 0.5,
            })
            .expect("overlay");
        let overlay = state.view(true, "", "").overlay;
        assert!(overlay.usable());
        assert_eq!(
            overlay.in_pixels(1920, 1080).expect("pixels"),
            (192, 108, 960, 540)
        );
        assert_eq!(state.crop_rect().expect("crop"), Some((192, 108, 960, 540)));

        // An odd side is rounded down, because the encoders refuse one.
        state
            .set_overlay(CropOverlay {
                left: 0.0,
                top: 0.0,
                width: 0.3333,
                height: 0.5,
            })
            .expect("overlay");
        let (left, top, width, height) = state
            .view(true, "", "")
            .overlay
            .in_pixels(1921, 1080)
            .expect("pixels");
        assert_eq!(width % 2, 0, "even width");
        assert_eq!(height % 2, 0, "even height");
        assert_eq!(left, 0);
        assert_eq!(top, 0);
    }

    #[test]
    fn an_overlay_that_leaves_the_frame_is_refused_with_a_reason() {
        let mut state = state(600.0, (1920, 1080), 4);
        let outside = CropOverlay {
            left: 0.8,
            top: 0.0,
            width: 0.4,
            height: 0.5,
        };
        assert!(!outside.usable());
        assert!(state
            .set_overlay(outside)
            .expect_err("outside the frame")
            .contains("inside the frame"));
        assert_eq!(
            state.view(true, "", "").overlay,
            CropOverlay::whole(),
            "the old rectangle stands"
        );

        let backwards = CropOverlay {
            left: 0.5,
            top: 0.5,
            width: 0.6,
            height: 0.1,
        };
        assert!(!backwards.usable(), "the rectangle reaches past the edge");
        assert!(state.set_overlay(backwards).is_err());

        let nan = CropOverlay {
            left: f64::NAN,
            top: 0.0,
            width: 0.5,
            height: 0.5,
        };
        assert!(!nan.usable());
        assert!(state.set_overlay(nan).is_err());
    }

    #[test]
    fn a_frame_with_no_reported_size_cannot_be_cropped() {
        let mut state = PreviewState::default();
        state.use_title(600.0, (0, 0));
        let error = state
            .set_overlay(CropOverlay::whole())
            .expect_err("no frame size");
        assert!(error.contains("frame size"), "{error}");
        let view = state.view(true, "movie.mkv", "Title 1");
        assert!(!view.crop.valid);
        assert!(view.crop.summary.contains("frame size"));
    }

    #[test]
    fn covering_the_whole_frame_is_reported_as_nothing_to_crop() {
        let mut state = state(600.0, (1920, 1080), 4);
        assert_eq!(state.crop_rect().expect("crop"), None);
        let view = state.view(true, "movie.mkv", "Title 1");
        assert!(view.crop.valid);
        assert!(!view.crop.applies);
        assert!(!view.apply_crop.available);
        assert!(view.apply_crop.reason.contains("whole frame"));

        state
            .set_overlay(CropOverlay {
                left: 0.05,
                top: 0.05,
                width: 0.9,
                height: 0.9,
            })
            .expect("overlay");
        let view = state.view(true, "movie.mkv", "Title 1");
        assert!(view.crop.applies);
        assert!(view.apply_crop.available);
        assert!(view.apply_crop.reason.is_empty());
    }

    #[test]
    fn the_worker_command_carries_the_source_title_and_still_position() {
        let state = state(3600.0, (1920, 1080), 5);
        let mut state = state;
        state.select(2).expect("still");
        let command = state.command("C:\\media\\movie.mkv", 3);
        assert_eq!(command.path, "C:\\media\\movie.mkv");
        assert_eq!(command.title_index, 3);
        assert_eq!(command.time_seconds, 1800.0);
        assert!(
            command.output_file.is_empty(),
            "the image comes back inline"
        );
    }

    #[test]
    fn the_view_reports_which_controls_are_on_offer() {
        let mut state = state(3600.0, (1920, 1080), 3);
        let view = state.view(true, "movie.mkv", "Title 1");
        assert!(view.has_source);
        assert!(view.empty);
        assert_eq!(view.empty_label, "This still has not been fetched yet.");
        assert!(!view.previous.available);
        assert!(view.previous.reason.contains("first still"));
        assert!(view.next.available);
        assert!(view.scrub_available);
        assert!(view.summary.contains("Still 1 of 3"));

        state.step(1);
        assert!(state.view(true, "movie.mkv", "Title 1").previous.available);

        state.select(2).expect("last");
        let view = state.view(true, "movie.mkv", "Title 1");
        assert!(view.next.reason.contains("last still"));
        assert!(view.rescan.available);
        assert!(view.still_count_control.available);

        state.accept_image(StillImage {
            number: 2,
            byte_length: 2048,
            data_base64: "Zm9v".into(),
        });
        let view = state.view(true, "movie.mkv", "Title 1");
        assert!(!view.empty);
        let image = view.image.expect("image");
        assert_eq!(image.number, 2);
        assert_eq!(image.data_base64, "Zm9v");
        assert_eq!(image.label, "3 · 1:00:00");
    }

    #[test]
    fn without_a_source_the_window_says_what_it_needs() {
        let state = PreviewState::default();
        let view = state.view(false, "", "");
        assert!(!view.has_source);
        assert!(!view.scrub_available);
        assert!(view.scrub_reason.contains("Scan a source"));
        assert!(view.summary.contains("Scan a source"));
        assert_eq!(view.empty_label, "No source is loaded.");
        assert!(
            view.still_count_control.available,
            "the count is kept for the next scan"
        );
        assert!(!view.rescan.available);
    }

    #[test]
    fn a_scan_without_stills_is_reported_rather_than_shown_as_an_empty_frame() {
        let mut state = state(3600.0, (1920, 1080), 0);
        let view = state.view(true, "movie.mkv", "Title 1");
        assert!(!view.scrub_available);
        assert!(view.summary.contains("without preview stills"));
        assert_eq!(view.empty_label, "Ask for a few stills, then scan again.");
        state.reset_overlay();
        assert_eq!(state.view(true, "", "").overlay, CropOverlay::whole());
    }

    #[test]
    fn a_failed_fetch_clears_the_frame_rather_than_leaving_a_stale_one() {
        let mut state = state(3600.0, (1920, 1080), 3);
        state.accept_image(StillImage {
            number: 0,
            byte_length: 10,
            data_base64: "AAA".into(),
        });
        state.fail_image("no decoders found");
        assert!(!state.has_image());
        assert!(state.view(true, "movie.mkv", "Title 1").empty);
    }
}
