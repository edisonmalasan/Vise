//! The filters tab: every filter this build can apply, and what it will do.
//!
//! A filter row is the same shape throughout — a switch, a named strength where
//! one exists, and custom value fields for when the named ones are not enough.
//! A row whose filter the worker contract has no field for is shown greyed out
//! with the reason, so nothing can be switched on that would then be ignored.

use serde::{Deserialize, Serialize};
use vise_core::VideoProcessingOptions;

use crate::tabs::common::{
    ChoiceGroup, ChoiceGroupViewModel, ChoiceOption, NumField, NumberViewModel,
};
use crate::tabs::dimensions::{self, DimensionsSettings, Flip};
use crate::tabs::source::TitleFacts;

/// Which filters are on, and with what strength.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FilterSettings {
    /// Remove interlace weaving.
    pub deinterlace: bool,
    /// Look for combed frames before removing the weave.
    pub comb_detect: bool,
    /// Reduce noise.
    pub denoise: bool,
    /// Named denoise strength.
    pub denoise_preset: String,
    /// Denoise strength the user typed.
    pub denoise_strength: NumField,
    /// Sharpen the picture.
    pub sharpen: bool,
    /// Named sharpen strength.
    pub sharpen_preset: String,
    /// Sharpen amount the user typed.
    pub sharpen_amount: NumField,
    /// Rebuild frames that were telecined for film.
    pub telecine: bool,
    /// Smooth block edges.
    pub deblock: bool,
    /// Widen the frame with a border.
    pub pad: bool,
    /// Frame width to pad to.
    pub pad_width: NumField,
    /// Frame height to pad to.
    pub pad_height: NumField,
    /// Smooth colour edges.
    pub chroma_smooth: bool,
    /// Throw the colour away.
    pub grayscale: bool,
    /// Convert the picture to another colour space.
    pub colour_space: String,
}

impl Default for FilterSettings {
    fn default() -> Self {
        Self {
            deinterlace: false,
            comb_detect: false,
            denoise: false,
            denoise_preset: "moderate".to_string(),
            denoise_strength: NumField::blank(),
            sharpen: false,
            sharpen_preset: "moderate".to_string(),
            sharpen_amount: NumField::blank(),
            telecine: false,
            deblock: false,
            pad: false,
            pad_width: NumField::blank(),
            pad_height: NumField::blank(),
            chroma_smooth: false,
            grayscale: false,
            colour_space: "keep".to_string(),
        }
    }
}

/// Named denoise strengths, weakest first.
const DENOISE_PRESETS: [(&str, &str, u32); 4] = [
    ("light", "Light", 3),
    ("moderate", "Moderate", 6),
    ("strong", "Strong", 10),
    ("heaviest", "Heaviest", 16),
];

/// Named sharpen amounts, weakest first.
const SHARPEN_PRESETS: [(&str, &str, f32); 4] = [
    ("light", "Light", 0.3),
    ("moderate", "Moderate", 0.7),
    ("strong", "Strong", 1.2),
    ("heaviest", "Heaviest", 1.8),
];

impl FilterSettings {
    /// The denoise strength, whether it came from a name or from the user.
    pub fn denoise_value(&self) -> Option<u32> {
        if !self.denoise_strength.is_blank() {
            return self
                .denoise_strength
                .whole("Denoise strength")
                .ok()
                .flatten();
        }
        named_strength(&self.denoise_preset, DENOISE_PRESETS)
    }

    /// The sharpen amount, whether it came from a name or from the user.
    pub fn sharpen_value(&self) -> Option<f32> {
        if !self.sharpen_amount.is_blank() {
            return self
                .sharpen_amount
                .number("Sharpen amount")
                .ok()
                .flatten()
                .map(|amount| amount as f32);
        }
        named_strength(&self.sharpen_preset, SHARPEN_PRESETS)
    }
}

fn named_strength<T: Copy>(name: &str, presets: [(&str, &str, T); 4]) -> Option<T> {
    presets
        .iter()
        .find(|(id, _, _)| *id == name.trim().to_ascii_lowercase())
        .map(|(_, _, value)| *value)
}

/// Filter choices as the interface layer sends them.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FilterInput {
    /// Which filter row changed.
    pub filter: Option<String>,
    /// Turn the filter on or off.
    pub enabled: Option<bool>,
    /// Named strength, or the choice for a row that is a choice itself.
    pub choice: Option<String>,
    /// First custom value, such as a strength or a width.
    pub value: Option<String>,
    /// Second custom value, for a row with two of them.
    pub value_two: Option<String>,
}

impl FilterInput {
    /// Apply the change to the row named by `filter`.
    ///
    /// An unknown row is refused with its name, and so is a named strength the
    /// row does not have.
    pub fn apply(self, settings: &mut FilterSettings) -> Result<(), String> {
        let filter = self
            .filter
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .ok_or_else(|| "Name the filter you want to change.".to_string())?;
        match filter {
            "deinterlace" => settings.deinterlace = self.enabled.unwrap_or(settings.deinterlace),
            "comb_detect" => settings.comb_detect = self.enabled.unwrap_or(settings.comb_detect),
            "telecine" => settings.telecine = self.enabled.unwrap_or(settings.telecine),
            "chroma_smooth" => {
                settings.chroma_smooth = self.enabled.unwrap_or(settings.chroma_smooth);
            }
            "grayscale" => settings.grayscale = self.enabled.unwrap_or(settings.grayscale),
            "deblock" => settings.deblock = self.enabled.unwrap_or(settings.deblock),
            "pad" => {
                settings.pad = self.enabled.unwrap_or(settings.pad);
                if let Some(value) = self.value {
                    settings.pad_width = NumField { text: value };
                }
                if let Some(value) = self.value_two {
                    settings.pad_height = NumField { text: value };
                }
            }
            "denoise" => {
                settings.denoise = self.enabled.unwrap_or(settings.denoise);
                apply_named(
                    &mut settings.denoise_preset,
                    self.choice,
                    DENOISE_PRESETS,
                    filter,
                )?;
                if let Some(value) = self.value {
                    settings.denoise_strength = NumField { text: value };
                }
            }
            "sharpen" => {
                settings.sharpen = self.enabled.unwrap_or(settings.sharpen);
                apply_named(
                    &mut settings.sharpen_preset,
                    self.choice,
                    SHARPEN_PRESETS,
                    filter,
                )?;
                if let Some(value) = self.value {
                    settings.sharpen_amount = NumField { text: value };
                }
            }
            "colour_space" => {
                let value = self
                    .choice
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or("keep")
                    .to_ascii_lowercase();
                if !colour_space_options()
                    .iter()
                    .any(|option| option.id == value)
                {
                    return Err(format!(
                        "'{value}' is not a colour space this build offers."
                    ));
                }
                settings.colour_space = value;
            }
            _ => return Err(format!("'{filter}' is not a filter this build offers.")),
        }
        Ok(())
    }
}

fn apply_named<T>(
    current: &mut String,
    value: Option<String>,
    presets: [(&str, &str, T); 4],
    filter: &str,
) -> Result<(), String> {
    let Some(value) = value.map(|value| value.trim().to_ascii_lowercase()) else {
        return Ok(());
    };
    if value == "custom" || value.is_empty() {
        return Ok(());
    }
    if presets.iter().any(|(id, _, _)| *id == value) {
        *current = value;
        return Ok(());
    }
    Err(format!(
        "'{value}' is not a named strength the {filter} filter offers."
    ))
}

/// Colour spaces the filters tab offers.
pub fn colour_space_options() -> Vec<ChoiceOption> {
    vec![
        ChoiceOption::new("keep", "Keep the source colour space"),
        ChoiceOption::new("bt709", "BT.709 (HD)"),
        ChoiceOption::new("bt470bg", "BT.470BG (PAL)"),
        ChoiceOption::new("smpte170m", "SMPTE 170M (NTSC)"),
        ChoiceOption::new("bt2020nc", "BT.2020 non-constant luminance"),
        ChoiceOption::new("fcc", "FCC (wide gamut)"),
    ]
}

/// Why the deblock row is not on offer.
pub const DEBLOCK_REASON: &str =
    "The worker contract has no deblock filter yet, so switching this on would do nothing.";

/// One filter row as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterRowViewModel {
    /// Stable identifier the interface layer sends back.
    pub id: String,
    /// Row heading.
    pub label: String,
    /// One line under the heading.
    pub hint: String,
    /// Whether the filter is on.
    pub enabled: bool,
    /// Whether this build can apply the filter at all.
    pub available: bool,
    /// Why it cannot, when it cannot.
    pub reason: String,
    /// Pickers inside the row; a strength picker, or the row's own choice.
    pub picks: Vec<ChoiceGroupViewModel>,
    /// Custom value fields inside the row.
    pub fields: Vec<NumberViewModel>,
    /// What the filter will do when it is on.
    pub summary: String,
}

/// The filters tab as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiltersTabViewModel {
    /// Every filter row.
    pub rows: Vec<FilterRowViewModel>,
    /// One line naming the filters that are on.
    pub summary: String,
    /// What has to be fixed before the job can be queued.
    pub problems: Vec<String>,
    /// Whether the tab's choices can be used.
    pub ready: bool,
}

/// Build the filters tab.
///
/// Orientation lives on the dimensions tab, so its row appears here as the same
/// controls reading the same values, which keeps the filter order in one place.
pub fn tab_view(
    settings: &FilterSettings,
    orientation: &DimensionsSettings,
    _facts: &TitleFacts,
) -> FiltersTabViewModel {
    let mut problems: Vec<String> = Vec::new();
    let mut rows: Vec<FilterRowViewModel> = Vec::new();

    // Rows that are off all read the same way, so the phrase lives in one place.
    fn off() -> String {
        "Off.".to_string()
    }
    let on = |text: &str| text.to_string();

    rows.push(FilterRowViewModel {
        id: "deinterlace".into(),
        label: "Deinterlace".into(),
        hint: "Rebuilds whole frames from interlaced video using Yadif.".into(),
        enabled: settings.deinterlace,
        available: true,
        reason: String::new(),
        picks: Vec::new(),
        fields: Vec::new(),
        summary: if settings.deinterlace {
            on("Interlaced frames are woven back into whole frames.")
        } else {
            off()
        },
    });

    let comb_reason = if settings.deinterlace {
        String::new()
    } else {
        "Comb detection only does something when the weave is being removed.".to_string()
    };
    rows.push(FilterRowViewModel {
        id: "comb_detect".into(),
        label: "Comb detection".into(),
        hint: "Checks each frame for combing before rebuilding it.".into(),
        enabled: settings.comb_detect,
        available: comb_reason.is_empty(),
        reason: comb_reason,
        picks: Vec::new(),
        fields: Vec::new(),
        summary: if settings.comb_detect && settings.deinterlace {
            on("Frames are checked for combing first.")
        } else {
            off()
        },
    });

    // Denoise: a named strength, or a value of the user's own.
    let mut denoise_strength = NumberViewModel::new(
        "denoiseStrength",
        "Custom strength",
        settings.denoise_strength.text.trim(),
        "",
    )
    .ranged(1.0, 32.0);
    let denoise_custom = settings.denoise && settings.denoise_preset == "custom";
    if !settings.denoise {
        denoise_strength = denoise_strength.blocked("Turn the filter on to choose a strength.");
    } else if !denoise_custom {
        denoise_strength = denoise_strength
            .blocked("Choose the custom strength name to type a value of your own.");
    }
    check_value(
        &settings.denoise_strength,
        "Denoise strength",
        true,
        1.0,
        32.0,
        denoise_custom && settings.denoise,
        &mut problems,
        &mut denoise_strength,
    );
    rows.push(FilterRowViewModel {
        id: "denoise".into(),
        label: "Denoise".into(),
        hint: "Softens grain and sensor noise.".into(),
        enabled: settings.denoise,
        available: true,
        reason: String::new(),
        picks: vec![ChoiceGroupViewModel::build(
            "denoiseStrengthName",
            "Strength",
            "How hard the noise is reduced.",
            &settings.denoise_preset,
            &named_choices(&DENOISE_PRESETS),
            settings.denoise,
            if settings.denoise {
                ""
            } else {
                "Turn the filter on to choose a strength."
            },
        )],
        fields: vec![denoise_strength],
        summary: if settings.denoise {
            match settings.denoise_value() {
                Some(value) => format!("Noise reduction at strength {value}."),
                None => "Noise reduction with the strength still to choose.".to_string(),
            }
        } else {
            off()
        },
    });

    let mut sharpen_amount = NumberViewModel::new(
        "sharpenAmount",
        "Custom amount",
        settings.sharpen_amount.text.trim(),
        "",
    )
    .ranged(0.0, 3.0);
    let sharpen_custom = settings.sharpen && settings.sharpen_preset == "custom";
    if !settings.sharpen {
        sharpen_amount = sharpen_amount.blocked("Turn the filter on to choose a strength.");
    } else if !sharpen_custom {
        sharpen_amount =
            sharpen_amount.blocked("Choose the custom amount name to type a value of your own.");
    }
    check_value(
        &settings.sharpen_amount,
        "Sharpen amount",
        false,
        0.0,
        3.0,
        sharpen_custom && settings.sharpen,
        &mut problems,
        &mut sharpen_amount,
    );
    rows.push(FilterRowViewModel {
        id: "sharpen".into(),
        label: "Sharpen".into(),
        hint: "Brings back detail lost to scaling or compression.".into(),
        enabled: settings.sharpen,
        available: true,
        reason: String::new(),
        picks: vec![ChoiceGroupViewModel::build(
            "sharpenAmountName",
            "Strength",
            "How much edge detail is added.",
            &settings.sharpen_preset,
            &named_choices(&SHARPEN_PRESETS),
            settings.sharpen,
            if settings.sharpen {
                ""
            } else {
                "Turn the filter on to choose a strength."
            },
        )],
        fields: vec![sharpen_amount],
        summary: if settings.sharpen {
            match settings.sharpen_value() {
                Some(value) => format!("Sharpening at amount {value:.2}."),
                None => "Sharpening with the amount still to choose.".to_string(),
            }
        } else {
            off()
        },
    });

    rows.push(FilterRowViewModel {
        id: "telecine".into(),
        label: "Telecine".into(),
        hint: "Rebuilds the frames that were telecined from film.".into(),
        enabled: settings.telecine,
        available: true,
        reason: String::new(),
        picks: Vec::new(),
        fields: Vec::new(),
        summary: if settings.telecine {
            on("Film cadence is rebuilt before the picture is scaled.")
        } else {
            off()
        },
    });

    rows.push(FilterRowViewModel {
        id: "deblock".into(),
        label: "Deblock".into(),
        hint: "Smooths the edges between blocks in a compressed picture.".into(),
        enabled: settings.deblock,
        available: false,
        reason: DEBLOCK_REASON.to_string(),
        picks: Vec::new(),
        fields: Vec::new(),
        summary: if settings.deblock {
            on("Asked for, but this build cannot apply it.")
        } else {
            "Off, and not on offer in this build.".to_string()
        },
    });

    // Padding: one row, two sizes.
    let mut pad_width =
        NumberViewModel::new("padWidth", "Width", settings.pad_width.text.trim(), "px")
            .ranged(2.0, 100_000.0);
    let mut pad_height =
        NumberViewModel::new("padHeight", "Height", settings.pad_height.text.trim(), "px")
            .ranged(2.0, 100_000.0);
    if !settings.pad {
        for field in [&mut pad_width, &mut pad_height] {
            *field = field
                .clone()
                .blocked("Turn padding on to choose the frame size.");
        }
    }
    if settings.pad {
        check_value(
            &settings.pad_width,
            "Pad to width",
            true,
            2.0,
            100_000.0,
            true,
            &mut problems,
            &mut pad_width,
        );
        check_value(
            &settings.pad_height,
            "Pad to height",
            true,
            2.0,
            100_000.0,
            true,
            &mut problems,
            &mut pad_height,
        );
        // Padding a frame to an odd size would leave the encoder with a frame it
        // cannot code, so the rule is the same as for any other size.
        for (field, value, label) in [
            (&mut pad_width, &settings.pad_width, "Pad to width"),
            (&mut pad_height, &settings.pad_height, "Pad to height"),
        ] {
            let odd = value
                .whole(label)
                .ok()
                .flatten()
                .is_some_and(|size| size % 2 != 0);
            if odd {
                let problem = format!("{label} cannot be odd; encoders need even dimensions.");
                *field = field.clone().invalid(problem.clone());
                problems.push(problem);
            }
        }
        if settings.pad_width.is_blank() || settings.pad_height.is_blank() {
            problems.push("A padded frame needs both a width and a height.".to_string());
        }
    }
    rows.push(FilterRowViewModel {
        id: "pad".into(),
        label: "Pad".into(),
        hint: "Adds a border so the frame reaches a chosen size.".into(),
        enabled: settings.pad,
        available: true,
        reason: String::new(),
        picks: Vec::new(),
        fields: vec![pad_width, pad_height],
        summary: if !settings.pad {
            off()
        } else if settings.pad_width.is_blank() || settings.pad_height.is_blank() {
            on("Padding needs a width and a height.")
        } else {
            format!(
                "Padded to {}x{}.",
                settings.pad_width.text.trim(),
                settings.pad_height.text.trim()
            )
        },
    });

    // Orientation: the same values the dimensions tab holds.
    rows.push(FilterRowViewModel {
        id: "rotate_flip".into(),
        label: "Turn and mirror".into(),
        hint: "Set on the dimensions tab; shown here so the filter order reads in one place."
            .into(),
        enabled: orientation.rotation() != 0 || orientation.mirror() != Flip::None,
        available: true,
        reason: String::new(),
        picks: vec![
            ChoiceGroupViewModel::build(
                "rotate",
                "Turn",
                "Which way the picture is turned.",
                &orientation.rotate,
                &ChoiceGroup::of([
                    ("0", "No turn"),
                    ("90", "Quarter turn clockwise"),
                    ("180", "Half turn"),
                    ("270", "Quarter turn anticlockwise"),
                ])
                .options,
                true,
                "",
            ),
            ChoiceGroupViewModel::build(
                "flip",
                "Mirror",
                "Which way the picture is mirrored.",
                orientation.flip.as_str(),
                &ChoiceGroup::of([
                    ("none", "No mirroring"),
                    ("horizontal", "Left to right"),
                    ("vertical", "Top to bottom"),
                    ("both", "Both ways"),
                ])
                .options,
                true,
                "",
            ),
        ],
        fields: Vec::new(),
        summary: orientation_summary(orientation),
    });

    let colour_active = settings.colour_space != "keep";
    rows.push(FilterRowViewModel {
        id: "colour_space".into(),
        label: "Colour space".into(),
        hint: "Converts the picture to a named colour space.".into(),
        enabled: colour_active,
        available: true,
        reason: String::new(),
        picks: vec![ChoiceGroupViewModel::build(
            "colourSpace",
            "Colour space",
            "Converts the picture to a named colour space.",
            &settings.colour_space,
            &colour_space_options(),
            true,
            "",
        )],
        fields: Vec::new(),
        summary: if colour_active {
            format!("Converted to {}.", settings.colour_space)
        } else {
            off()
        },
    });

    rows.push(FilterRowViewModel {
        id: "chroma_smooth".into(),
        label: "Chroma smoothing".into(),
        hint: "Evens out the edges between colour areas.".into(),
        enabled: settings.chroma_smooth,
        available: true,
        reason: String::new(),
        picks: Vec::new(),
        fields: Vec::new(),
        summary: if settings.chroma_smooth {
            on("Colour edges are smoothed.")
        } else {
            off()
        },
    });

    rows.push(FilterRowViewModel {
        id: "grayscale".into(),
        label: "Greyscale".into(),
        hint: "Throws the colour away.".into(),
        enabled: settings.grayscale,
        available: true,
        reason: String::new(),
        picks: Vec::new(),
        fields: Vec::new(),
        summary: if settings.grayscale {
            on("The output has no colour, so the colour rows above no longer apply.")
        } else {
            off()
        },
    });

    if settings.grayscale && settings.chroma_smooth {
        problems.push(
            "Greyscale already removes the colour that chroma smoothing works on.".to_string(),
        );
    }
    if settings.grayscale && colour_active {
        problems
            .push("A greyscale picture cannot also be converted to a colour space.".to_string());
    }

    let active: Vec<String> = rows
        .iter()
        .filter(|row| row.enabled)
        .map(|row| row.label.clone())
        .collect();
    let summary = if active.is_empty() {
        "No filters are on; the picture is passed through as it was decoded.".to_string()
    } else {
        format!("On: {}.", active.join(", "))
    };

    FiltersTabViewModel {
        rows,
        summary,
        ready: problems.is_empty(),
        problems,
    }
}

fn orientation_summary(orientation: &DimensionsSettings) -> String {
    let turn = match orientation.rotation() {
        0 => "no turn".to_string(),
        90 => "a quarter turn clockwise".to_string(),
        180 => "a half turn".to_string(),
        _ => "a quarter turn anticlockwise".to_string(),
    };
    let mirror = match orientation.mirror() {
        Flip::None => String::new(),
        Flip::Horizontal => " and mirrored left to right".to_string(),
        Flip::Vertical => " and mirrored top to bottom".to_string(),
        Flip::Both => " and mirrored both ways".to_string(),
    };
    format!("{turn}{mirror}.")
}

/// Named strengths as choice options, with a custom name at the end.
fn named_choices<T>(presets: &[(&str, &str, T)]) -> Vec<ChoiceOption> {
    let mut options: Vec<ChoiceOption> = presets
        .iter()
        .map(|(id, label, _)| ChoiceOption::new(id, label))
        .collect();
    options.push(ChoiceOption::new("custom", "Custom value"));
    options
}

/// Check a custom value, marking the field and reporting anything wrong.
#[allow(clippy::too_many_arguments)]
fn check_value(
    field: &NumField,
    label: &str,
    whole: bool,
    min: f64,
    max: f64,
    active: bool,
    problems: &mut Vec<String>,
    view: &mut NumberViewModel,
) {
    if !active {
        return;
    }
    let read = if whole {
        field
            .whole(label)
            .map(|value| value.map(|value| value as f64))
    } else {
        field.number(label)
    };
    match read {
        Err(problem) => {
            *view = view.clone().invalid(problem.clone());
            problems.push(problem);
        }
        Ok(None) => {
            let problem = format!("Choose a custom value for {label}, or pick a named strength.");
            *view = view.clone().invalid(problem.clone());
            problems.push(problem);
        }
        Ok(Some(value)) => {
            if value < min || value > max {
                let problem = format!(
                    "{label} must be between {} and {}.",
                    trim_number(min),
                    trim_number(max)
                );
                *view = view.clone().invalid(problem.clone());
                problems.push(problem);
            }
        }
    }
}

fn trim_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

/// The filters as the job model wants them.
pub fn to_job_options(
    settings: &FilterSettings,
    orientation: &DimensionsSettings,
) -> VideoProcessingOptions {
    let mut options = VideoProcessingOptions {
        deinterlace: settings.deinterlace,
        deinterlacing: settings.deinterlace,
        comb_detect: settings.comb_detect,
        comb_detection: settings.comb_detect,
        detelecine: settings.telecine,
        denoise: settings.denoise,
        denoise_strength: settings.denoise_value(),
        sharpen: settings.sharpen,
        sharpen_amount: settings.sharpen_value(),
        chroma_smooth: settings.chroma_smooth,
        chroma_smoothing: settings.chroma_smooth,
        grayscale: settings.grayscale,
        color_space: if settings.colour_space == "keep" {
            None
        } else {
            Some(settings.colour_space.clone())
        },
        ..Default::default()
    };
    if settings.pad && !settings.pad_width.is_blank() && !settings.pad_height.is_blank() {
        if let (Ok(Some(width)), Ok(Some(height))) = (
            settings.pad_width.whole("Pad to width"),
            settings.pad_height.whole("Pad to height"),
        ) {
            // An odd padded frame would be refused by the encoder, so it is not
            // carried into the job.
            if width % 2 == 0 && height % 2 == 0 {
                options.pad = Some((width, height));
                options.pad_to = Some((width, height));
            }
        }
    }
    let (rotation, flip) = dimensions::to_job_geometry_orientation(orientation);
    options.rotate = rotation;
    options.flip = Some(flip);
    options
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> TitleFacts {
        TitleFacts {
            number: 1,
            width: 1920,
            height: 1080,
            ..Default::default()
        }
    }

    fn filter_row<'a>(tab: &'a FiltersTabViewModel, id: &str) -> &'a FilterRowViewModel {
        tab.rows
            .iter()
            .find(|row| row.id == id)
            .unwrap_or_else(|| panic!("{id} row"))
    }

    #[test]
    fn the_defaults_leave_every_filter_off() {
        let view = tab_view(
            &FilterSettings::default(),
            &DimensionsSettings::default(),
            &facts(),
        );
        assert!(view.ready, "{:?}", view.problems);
        assert!(view.rows.iter().all(|row| !row.enabled));
        assert!(view.summary.contains("No filters are on"));
        assert_eq!(view.rows.len(), 11, "every row is present, on or off");
    }

    #[test]
    fn deblock_is_shown_but_cannot_be_switched_on() {
        let idle = tab_view(
            &FilterSettings::default(),
            &DimensionsSettings::default(),
            &facts(),
        );
        let deblock = filter_row(&idle, "deblock");
        assert!(!deblock.available);
        assert!(
            deblock.reason.contains("no deblock filter"),
            "{}",
            deblock.reason
        );

        let asked_for = FilterSettings {
            deblock: true,
            ..Default::default()
        };
        let asked = tab_view(&asked_for, &DimensionsSettings::default(), &facts());
        let deblock = filter_row(&asked, "deblock");
        assert!(deblock.enabled, "the request is never hidden");
        assert!(
            deblock.summary.contains("cannot apply"),
            "{}",
            deblock.summary
        );
        // The job model has nowhere to put it, so nothing is invented for it.
        let chain = to_job_options(&asked_for, &DimensionsSettings::default()).build_filter_chain();
        assert!(!chain.contains("deblock"), "{chain}");
    }

    #[test]
    fn comb_detection_only_matters_with_deinterlacing() {
        let alone = tab_view(
            &FilterSettings {
                comb_detect: true,
                ..Default::default()
            },
            &DimensionsSettings::default(),
            &facts(),
        );
        let row = filter_row(&alone, "comb_detect");
        assert!(!row.available);
        assert!(row.reason.contains("weave"), "{}", row.reason);

        let together = tab_view(
            &FilterSettings {
                deinterlace: true,
                comb_detect: true,
                ..Default::default()
            },
            &DimensionsSettings::default(),
            &facts(),
        );
        let row = filter_row(&together, "comb_detect");
        assert!(row.available);
        assert!(row.summary.contains("checked for combing"));
        let options = to_job_options(
            &FilterSettings {
                deinterlace: true,
                comb_detect: true,
                ..Default::default()
            },
            &DimensionsSettings::default(),
        );
        assert!(options.deinterlace && options.comb_detect);
        assert!(options.build_filter_chain().contains("combdet"));
    }

    #[test]
    fn a_named_strength_becomes_a_number_and_a_custom_value_overrides_it() {
        let settings = FilterSettings {
            denoise: true,
            denoise_preset: "strong".into(),
            sharpen: true,
            sharpen_preset: "light".into(),
            ..Default::default()
        };
        assert_eq!(settings.denoise_value(), Some(10));
        assert_eq!(settings.sharpen_value(), Some(0.3));
        let options = to_job_options(&settings, &DimensionsSettings::default());
        assert!(options.denoise);
        assert_eq!(options.denoise_strength, Some(10));
        assert_eq!(options.sharpen_amount, Some(0.3));
        assert!(options.build_filter_chain().contains("hqdn3d=10:10:15:15"));

        let custom = FilterSettings {
            denoise: true,
            denoise_preset: "custom".into(),
            denoise_strength: NumField::of("9"),
            ..Default::default()
        };
        assert_eq!(custom.denoise_value(), Some(9));
    }

    #[test]
    fn a_custom_value_outside_its_range_is_reported_and_not_applied() {
        let settings = FilterSettings {
            denoise: true,
            denoise_preset: "custom".into(),
            denoise_strength: NumField::of("99"),
            ..Default::default()
        };
        let view = tab_view(&settings, &DimensionsSettings::default(), &facts());
        assert!(!view.ready);
        assert!(
            view.problems.iter().any(|p| p.contains("between 1 and 32")),
            "{:?}",
            view.problems
        );
        let field = &filter_row(&view, "denoise").fields[0];
        assert!(!field.valid);
        assert!(field.problem.contains("between 1 and 32"));

        let blank = tab_view(
            &FilterSettings {
                denoise: true,
                denoise_preset: "custom".into(),
                ..Default::default()
            },
            &DimensionsSettings::default(),
            &facts(),
        );
        assert!(blank
            .problems
            .iter()
            .any(|p| p.contains("Choose a custom value")));
    }

    #[test]
    fn the_custom_field_is_closed_until_the_custom_name_is_chosen() {
        let named = tab_view(
            &FilterSettings {
                denoise: true,
                ..Default::default()
            },
            &DimensionsSettings::default(),
            &facts(),
        );
        let field = &filter_row(&named, "denoise").fields[0];
        assert!(!field.available);
        assert!(field.reason.contains("custom strength"), "{}", field.reason);
        assert!(filter_row(&named, "denoise").picks[0].available);

        let off = tab_view(
            &FilterSettings::default(),
            &DimensionsSettings::default(),
            &facts(),
        );
        let field = &filter_row(&off, "denoise").fields[0];
        assert!(
            field.reason.contains("Turn the filter on"),
            "{}",
            field.reason
        );
        assert!(!filter_row(&off, "denoise").picks[0].available);
        assert!(filter_row(&off, "denoise").picks[0]
            .reason
            .contains("Turn the filter on"));
    }

    #[test]
    fn padding_needs_both_sizes_and_even_ones() {
        let missing = FilterSettings {
            pad: true,
            pad_width: NumField::of("1920"),
            ..Default::default()
        };
        let view = tab_view(&missing, &DimensionsSettings::default(), &facts());
        assert!(view
            .problems
            .iter()
            .any(|p| p.contains("both a width and a height")));
        assert!(filter_row(&view, "pad")
            .summary
            .contains("needs a width and a height"));
        assert_eq!(
            to_job_options(&missing, &DimensionsSettings::default()).pad,
            None
        );

        let odd = FilterSettings {
            pad: true,
            pad_width: NumField::of("1921"),
            pad_height: NumField::of("1080"),
            ..Default::default()
        };
        let view = tab_view(&odd, &DimensionsSettings::default(), &facts());
        assert!(
            view.problems.iter().any(|p| p.contains("even dimensions")),
            "{:?}",
            view.problems
        );
        assert_eq!(
            to_job_options(&odd, &DimensionsSettings::default()).pad,
            None
        );

        let good = FilterSettings {
            pad: true,
            pad_width: NumField::of("1920"),
            pad_height: NumField::of("1080"),
            ..Default::default()
        };
        assert_eq!(
            to_job_options(&good, &DimensionsSettings::default()).pad,
            Some((1920, 1080))
        );
    }

    #[test]
    fn a_colour_space_is_chosen_or_the_source_one_is_kept() {
        let kept = to_job_options(&FilterSettings::default(), &DimensionsSettings::default());
        assert_eq!(kept.color_space, None);
        let chosen = to_job_options(
            &FilterSettings {
                colour_space: "bt709".into(),
                ..Default::default()
            },
            &DimensionsSettings::default(),
        );
        assert_eq!(chosen.color_space.as_deref(), Some("bt709"));
        assert!(chosen.build_filter_chain().contains("colorspace=bt709"));
    }

    #[test]
    fn greyscale_conflicts_with_the_colour_work() {
        let view = tab_view(
            &FilterSettings {
                grayscale: true,
                chroma_smooth: true,
                colour_space: "bt709".into(),
                ..Default::default()
            },
            &DimensionsSettings::default(),
            &facts(),
        );
        assert!(!view.ready);
        assert!(view.problems.iter().any(|p| p.contains("chroma smoothing")));
        assert!(view.problems.iter().any(|p| p.contains("colour space")));
        assert!(filter_row(&view, "grayscale")
            .summary
            .contains("no longer apply"));
    }

    #[test]
    fn the_turn_and_mirror_row_reads_the_dimensions_tab() {
        let turned = DimensionsSettings {
            rotate: "90".into(),
            flip: "horizontal".into(),
            ..Default::default()
        };
        let view = tab_view(&FilterSettings::default(), &turned, &facts());
        let row = filter_row(&view, "rotate_flip");
        assert!(row.enabled);
        assert_eq!(row.picks[0].selected, "90");
        assert_eq!(row.picks[1].selected, "horizontal");
        assert!(
            row.summary.contains("quarter turn clockwise"),
            "{}",
            row.summary
        );

        let options = to_job_options(&FilterSettings::default(), &turned);
        assert_eq!(options.rotate, Some(90));
        assert_eq!(options.flip, Some(vise_core::FlipMode::Horizontal));
        assert!(options.build_filter_chain().contains("hflip"));
    }

    #[test]
    fn a_filter_that_does_not_exist_is_refused_with_its_name() {
        let mut settings = FilterSettings::default();
        let error = FilterInput {
            filter: Some("sharpen".into()),
            choice: Some("gentle".into()),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect_err("unknown strength");
        assert!(error.contains("gentle"), "{error}");

        let unknown = FilterInput {
            filter: Some("magic".into()),
            enabled: Some(true),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect_err("unknown filter");
        assert!(unknown.contains("magic"), "{unknown}");

        let missing = FilterInput {
            enabled: Some(true),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect_err("no filter named");
        assert!(missing.contains("Name the filter"), "{missing}");

        let colour = FilterInput {
            filter: Some("colour_space".into()),
            choice: Some("rec2020".into()),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect_err("unknown colour space");
        assert!(colour.contains("rec2020"), "{colour}");
    }

    #[test]
    fn a_known_filter_change_is_applied() {
        let mut settings = FilterSettings::default();
        FilterInput {
            filter: Some("deinterlace".into()),
            enabled: Some(true),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect("deinterlace");
        FilterInput {
            filter: Some("sharpen".into()),
            enabled: Some(true),
            choice: Some("strong".into()),
            value: Some("0.9".into()),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect("sharpen");
        FilterInput {
            filter: Some("pad".into()),
            enabled: Some(true),
            value: Some("1920".into()),
            value_two: Some("1080".into()),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect("pad");
        assert!(settings.deinterlace);
        assert!(settings.sharpen);
        assert_eq!(settings.sharpen_preset, "strong");
        assert_eq!(settings.sharpen_value(), Some(0.9));
        assert!(settings.pad);
        assert_eq!(settings.pad_width, NumField::of("1920"));
        assert_eq!(settings.pad_height, NumField::of("1080"));
    }
}
