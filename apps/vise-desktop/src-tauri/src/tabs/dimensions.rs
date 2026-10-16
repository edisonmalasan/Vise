//! The dimensions tab: crop, scale, aspect ratio, size limits, rotation.
//!
//! The rules here are geometry rules. A crop has to sit inside the frame, a
//! scale target has to be even for the encoder to accept it, and a quarter turn
//! swaps the axes so a size means something different after it. The tab
//! resolves all of that here and hands the job a rectangle it can act on.

use serde::{Deserialize, Serialize};

use crate::tabs::common::{ChoiceGroup, ChoiceGroupViewModel, NumField, NumberViewModel};
use crate::tabs::source::TitleFacts;

/// How much of the frame is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CropMode {
    /// The whole frame.
    None,
    /// A rectangle the user types.
    Manual,
    /// The largest centred rectangle of a chosen shape.
    AspectRatio,
}

impl CropMode {
    /// Stable identifier the interface layer sends back.
    pub fn as_str(self) -> &'static str {
        match self {
            CropMode::None => "none",
            CropMode::Manual => "manual",
            CropMode::AspectRatio => "aspect",
        }
    }

    /// The mode named by `text`.
    pub fn parse(text: &str) -> Option<Self> {
        match text
            .trim()
            .to_ascii_lowercase()
            .replace(['-', ' '], "_")
            .as_str()
        {
            "none" | "off" | "keep" => Some(CropMode::None),
            "manual" | "custom" | "region" => Some(CropMode::Manual),
            "aspect" | "aspect_ratio" | "ratio" | "auto" => Some(CropMode::AspectRatio),
            _ => None,
        }
    }
}

/// How the picture is resized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleMode {
    /// No resize.
    Keep,
    /// Exactly the size the user types.
    Exact,
    /// As large as fits inside a box, keeping the shape.
    Fit,
    /// A chosen shape, keeping the picture inside it.
    Aspect,
}

impl ScaleMode {
    /// Stable identifier the interface layer sends back.
    pub fn as_str(self) -> &'static str {
        match self {
            ScaleMode::Keep => "keep",
            ScaleMode::Exact => "exact",
            ScaleMode::Fit => "fit",
            ScaleMode::Aspect => "aspect",
        }
    }

    /// The mode named by `text`.
    pub fn parse(text: &str) -> Option<Self> {
        match text
            .trim()
            .to_ascii_lowercase()
            .replace(['-', ' '], "_")
            .as_str()
        {
            "keep" | "none" | "source" => Some(ScaleMode::Keep),
            "exact" | "size" | "dimensions" => Some(ScaleMode::Exact),
            "fit" | "limit" | "box" => Some(ScaleMode::Fit),
            "aspect" | "aspect_ratio" | "ratio" => Some(ScaleMode::Aspect),
            _ => None,
        }
    }
}

/// Mirror direction as the window holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flip {
    /// No mirroring.
    None,
    /// Left to right.
    Horizontal,
    /// Top to bottom.
    Vertical,
    /// Both ways.
    Both,
}

impl Flip {
    /// Stable identifier the interface layer sends back.
    pub fn as_str(self) -> &'static str {
        match self {
            Flip::None => "none",
            Flip::Horizontal => "horizontal",
            Flip::Vertical => "vertical",
            Flip::Both => "both",
        }
    }

    /// The direction named by `text`.
    pub fn parse(text: &str) -> Option<Self> {
        match text
            .trim()
            .to_ascii_lowercase()
            .replace(['-', ' '], "_")
            .as_str()
        {
            "none" | "off" => Some(Flip::None),
            "horizontal" | "hflip" => Some(Flip::Horizontal),
            "vertical" | "vflip" => Some(Flip::Vertical),
            "both" | "both_ways" | "hv" => Some(Flip::Both),
            _ => None,
        }
    }

    /// How the core model spells this direction.
    pub fn to_core(self) -> vise_core::FlipMode {
        match self {
            Flip::None => vise_core::FlipMode::None,
            Flip::Horizontal => vise_core::FlipMode::Horizontal,
            Flip::Vertical => vise_core::FlipMode::Vertical,
            Flip::Both => vise_core::FlipMode::Both,
        }
    }
}

/// The dimension choices as the window holds them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DimensionsSettings {
    /// Crop mode.
    pub crop_mode: String,
    /// Left edge of a manual crop.
    pub crop_x: NumField,
    /// Top edge of a manual crop.
    pub crop_y: NumField,
    /// Width of a manual crop.
    pub crop_width: NumField,
    /// Height of a manual crop.
    pub crop_height: NumField,
    /// Scale mode.
    pub scale_mode: String,
    /// Width of an exact scale.
    pub scale_width: NumField,
    /// Height of an exact scale.
    pub scale_height: NumField,
    /// Shape to crop or scale to, such as `16:9`.
    pub aspect_ratio: String,
    /// Largest width the output may have.
    pub max_width: NumField,
    /// Largest height the output may have.
    pub max_height: NumField,
    /// Turn, in degrees.
    pub rotate: String,
    /// Mirror direction.
    pub flip: String,
}

impl Default for DimensionsSettings {
    fn default() -> Self {
        Self {
            crop_mode: CropMode::None.as_str().to_string(),
            crop_x: NumField::blank(),
            crop_y: NumField::blank(),
            crop_width: NumField::blank(),
            crop_height: NumField::blank(),
            scale_mode: ScaleMode::Keep.as_str().to_string(),
            scale_width: NumField::blank(),
            scale_height: NumField::blank(),
            aspect_ratio: String::new(),
            max_width: NumField::blank(),
            max_height: NumField::blank(),
            rotate: "0".to_string(),
            flip: Flip::None.as_str().to_string(),
        }
    }
}

impl DimensionsSettings {
    /// The crop mode as an enum.
    pub fn crop(&self) -> CropMode {
        CropMode::parse(&self.crop_mode).unwrap_or(CropMode::None)
    }

    /// The scale mode as an enum.
    pub fn scale(&self) -> ScaleMode {
        ScaleMode::parse(&self.scale_mode).unwrap_or(ScaleMode::Keep)
    }

    /// The turn in degrees, as a right angle.
    pub fn rotation(&self) -> u32 {
        match self.rotate.trim() {
            "90" | "270" => self.rotate.trim().parse().unwrap_or(90),
            _ => 0,
        }
    }

    /// The mirror direction as an enum.
    pub fn mirror(&self) -> Flip {
        Flip::parse(&self.flip).unwrap_or(Flip::None)
    }
}

/// Dimension choices as the interface layer sends them.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DimensionsInput {
    /// Crop mode.
    pub crop_mode: Option<String>,
    /// Left edge of a manual crop.
    pub crop_x: Option<String>,
    /// Top edge of a manual crop.
    pub crop_y: Option<String>,
    /// Width of a manual crop.
    pub crop_width: Option<String>,
    /// Height of a manual crop.
    pub crop_height: Option<String>,
    /// Scale mode.
    pub scale_mode: Option<String>,
    /// Width of an exact scale.
    pub scale_width: Option<String>,
    /// Height of an exact scale.
    pub scale_height: Option<String>,
    /// Shape to crop or scale to, such as `16:9`.
    pub aspect_ratio: Option<String>,
    /// Largest width the output may have.
    pub max_width: Option<String>,
    /// Largest height the output may have.
    pub max_height: Option<String>,
    /// Turn, in degrees.
    pub rotate: Option<String>,
    /// Mirror direction.
    pub flip: Option<String>,
}

impl DimensionsInput {
    /// Apply the change, refusing names this build does not offer.
    pub fn apply(self, settings: &mut DimensionsSettings) -> Result<(), String> {
        if let Some(mode) = self.crop_mode {
            settings.crop_mode = CropMode::parse(&mode)
                .ok_or_else(|| format!("'{}' is not a crop mode this build offers.", mode.trim()))?
                .as_str()
                .to_string();
        }
        if let Some(mode) = self.scale_mode {
            settings.scale_mode = ScaleMode::parse(&mode)
                .ok_or_else(|| {
                    format!("'{}' is not a scaling mode this build offers.", mode.trim())
                })?
                .as_str()
                .to_string();
        }
        for (name, value) in [
            ("crop_x", self.crop_x),
            ("crop_y", self.crop_y),
            ("crop_width", self.crop_width),
            ("crop_height", self.crop_height),
            ("scale_width", self.scale_width),
            ("scale_height", self.scale_height),
            ("max_width", self.max_width),
            ("max_height", self.max_height),
        ] {
            if let Some(text) = value {
                match name {
                    "crop_x" => settings.crop_x = NumField { text },
                    "crop_y" => settings.crop_y = NumField { text },
                    "crop_width" => settings.crop_width = NumField { text },
                    "crop_height" => settings.crop_height = NumField { text },
                    "scale_width" => settings.scale_width = NumField { text },
                    "scale_height" => settings.scale_height = NumField { text },
                    "max_width" => settings.max_width = NumField { text },
                    _ => settings.max_height = NumField { text },
                }
            }
        }
        if let Some(value) = self.aspect_ratio {
            settings.aspect_ratio = value;
        }
        if let Some(value) = self.rotate {
            let value = value.trim().to_string();
            let degrees = value
                .parse::<u32>()
                .map_err(|_| format!("Rotation must be 0, 90, 180, or 270, not '{value}'."))?;
            if ![0, 90, 180, 270].contains(&degrees) {
                return Err(format!(
                    "Rotation must be 0, 90, 180, or 270, not '{value}'."
                ));
            }
            settings.rotate = degrees.to_string();
        }
        if let Some(value) = self.flip {
            settings.flip = Flip::parse(&value)
                .ok_or_else(|| {
                    format!(
                        "'{}' is not a mirror direction this build offers.",
                        value.trim()
                    )
                })?
                .as_str()
                .to_string();
        }
        Ok(())
    }
}

/// Read a shape such as `16:9` or `1.78` as a width-to-height ratio.
pub fn aspect_ratio_value(text: &str) -> Result<f64, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("Choose a shape such as 16:9.".to_string());
    }
    let value = if let Some((left, right)) = text.split_once(':') {
        let left = left
            .trim()
            .parse::<f64>()
            .map_err(|_| format!("'{text}' is not a shape; write it as 16:9."))?;
        let right = right
            .trim()
            .parse::<f64>()
            .map_err(|_| format!("'{text}' is not a shape; write it as 16:9."))?;
        if left <= 0.0 || right <= 0.0 {
            return Err(format!(
                "'{text}' is not a shape; both sides must be above zero."
            ));
        }
        left / right
    } else {
        text.parse::<f64>()
            .map_err(|_| format!("'{text}' is not a shape; write it as 16:9 or 1.78."))?
    };
    if !value.is_finite() || value <= 0.0 {
        return Err(format!("'{text}' is not a usable shape."));
    }
    Ok(value)
}

/// Round a size down to a whole number of pixels.
fn even(value: f64) -> u32 {
    let floored = value.floor();
    if (floored as i64) % 2 == 0 {
        floored as u32
    } else {
        floored as u32 - 1
    }
}

/// The crop rectangle for the chosen mode, or `None` when nothing is cropped.
pub fn crop_rect(
    settings: &DimensionsSettings,
    facts: &TitleFacts,
) -> Result<Option<(u32, u32, u32, u32)>, String> {
    match settings.crop() {
        CropMode::None => Ok(None),
        CropMode::Manual => {
            if !facts.has_frame_size() {
                return Err(
                    "A manual crop needs a frame size, which this title does not report."
                        .to_string(),
                );
            }
            let x = settings.crop_x.whole("Crop left edge")?.unwrap_or(0);
            let y = settings.crop_y.whole("Crop top edge")?.unwrap_or(0);
            let width = settings.crop_width.whole("Crop width")?.ok_or(
                "Choose a crop width, or set the crop mode back to keeping the whole frame."
                    .to_string(),
            )?;
            let height = settings.crop_height.whole("Crop height")?.ok_or(
                "Choose a crop height, or set the crop mode back to keeping the whole frame."
                    .to_string(),
            )?;
            if width < 2 || height < 2 {
                return Err("A crop has to be at least 2 by 2 pixels.".to_string());
            }
            if width % 2 != 0 || height % 2 != 0 {
                return Err(
                    "Encoders need even dimensions, so a crop cannot have an odd width or height."
                        .to_string(),
                );
            }
            // The crop runs before the turn, so it is checked against the frame
            // as the source has it.
            if x + width > facts.width || y + height > facts.height {
                return Err(format!(
                    "A {width} by {height} crop at {x},{y} does not fit inside the {} by {} frame.",
                    facts.width, facts.height
                ));
            }
            Ok(Some((x, y, width, height)))
        }
        CropMode::AspectRatio => {
            if !facts.has_frame_size() {
                return Err(
                    "A shape crop needs a frame size, which this title does not report."
                        .to_string(),
                );
            }
            let ratio = aspect_ratio_value(&settings.aspect_ratio)?;
            let source = facts.width as f64 / facts.height as f64;
            let (width, height) = if source > ratio {
                (even(ratio * facts.height as f64), facts.height)
            } else {
                (facts.width, even(facts.width as f64 / ratio))
            };
            let width = even(width as f64).max(2);
            let height = even(height as f64).max(2);
            Ok(Some((
                (facts.width.saturating_sub(width)) / 2,
                (facts.height.saturating_sub(height)) / 2,
                width,
                height,
            )))
        }
    }
}

/// The scale target for the chosen mode, or `None` when nothing is resized.
pub fn scale_target(
    settings: &DimensionsSettings,
    facts: &TitleFacts,
) -> Result<Option<(u32, u32)>, String> {
    let (frame_w, frame_h) = facts.turned_size(settings.rotation());
    match settings.scale() {
        ScaleMode::Keep => Ok(None),
        ScaleMode::Exact => {
            let width = settings.scale_width.whole("Scale width")?.ok_or(
                "Choose a width, or set the scaling mode back to keeping the source size."
                    .to_string(),
            )?;
            let height = settings.scale_height.whole("Scale height")?.ok_or(
                "Choose a height, or set the scaling mode back to keeping the source size."
                    .to_string(),
            )?;
            if width < 2 || height < 2 {
                return Err("A frame has to be at least 2 by 2 pixels.".to_string());
            }
            if width % 2 != 0 || height % 2 != 0 {
                return Err(
                    "Encoders need even dimensions, so a frame cannot have an odd width or height."
                        .to_string(),
                );
            }
            Ok(Some((width, height)))
        }
        ScaleMode::Fit => {
            let max_w = settings.max_width.whole("Largest width")?;
            let max_h = settings.max_height.whole("Largest height")?;
            if max_w.is_none() && max_h.is_none() {
                return Err(
                    "Set at least one size limit so the picture has something to fit inside."
                        .to_string(),
                );
            }
            if frame_w == 0 || frame_h == 0 {
                return Err(
                    "A size limit needs a frame size, which this title does not report."
                        .to_string(),
                );
            }
            for value in [max_w, max_h].into_iter().flatten() {
                if value < 2 {
                    return Err("A size limit has to be at least 2 pixels.".to_string());
                }
                if value % 2 != 0 {
                    return Err(
                        "Encoders need even dimensions, so a size limit cannot be odd.".to_string(),
                    );
                }
            }
            // A limit that was not typed is not a limit at all, so it must not hold the
            // picture down; only a limit smaller than the frame scales anything.
            let scale_w = max_w.map_or(f64::INFINITY, |max| max as f64 / frame_w as f64);
            let scale_h = max_h.map_or(f64::INFINITY, |max| max as f64 / frame_h as f64);
            // Fitting never enlarges: a limit larger than the frame is simply
            // not a limit.
            let factor = scale_w.min(scale_h).min(1.0);
            let width = even(frame_w as f64 * factor).max(2);
            let height = even(frame_h as f64 * factor).max(2);
            Ok(Some((width, height)))
        }
        ScaleMode::Aspect => {
            if frame_w == 0 || frame_h == 0 {
                return Err(
                    "A shape needs a frame size, which this title does not report.".to_string(),
                );
            }
            let ratio = aspect_ratio_value(&settings.aspect_ratio)?;
            let source = frame_w as f64 / frame_h as f64;
            let (width, height) = if source > ratio {
                (even(ratio * frame_h as f64), frame_h)
            } else {
                (frame_w, even(frame_w as f64 / ratio))
            };
            Ok(Some((
                even(width as f64).max(2),
                even(height as f64).max(2),
            )))
        }
    }
}

/// The frame the encoder will see, as `1920x1080`.
pub fn output_size(settings: &DimensionsSettings, facts: &TitleFacts) -> String {
    let cropped = crop_rect(settings, facts).unwrap_or(None);
    let base = match cropped {
        Some((_, _, width, height)) => (width as f64, height as f64),
        None => (
            if facts.width == 0 {
                0.0
            } else {
                facts.width as f64
            },
            if facts.height == 0 {
                0.0
            } else {
                facts.height as f64
            },
        ),
    };
    let (mut width, mut height) = if settings.rotation() % 180 == 90 {
        (base.1, base.0)
    } else {
        base
    };
    if let Ok(Some((target_w, target_h))) = scale_target(settings, facts) {
        width = target_w as f64;
        height = target_h as f64;
    }
    if width == 0.0 || height == 0.0 {
        return "unknown size".to_string();
    }
    format!("{width:.0}x{height:.0}")
}

/// The dimensions tab as the page renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DimensionsTabViewModel {
    /// Crop mode picker.
    pub crop_mode: ChoiceGroupViewModel,
    /// Left edge of a manual crop.
    pub crop_x: NumberViewModel,
    /// Top edge of a manual crop.
    pub crop_y: NumberViewModel,
    /// Width of a manual crop.
    pub crop_width: NumberViewModel,
    /// Height of a manual crop.
    pub crop_height: NumberViewModel,
    /// Scaling mode picker.
    pub scale_mode: ChoiceGroupViewModel,
    /// Width of an exact scale.
    pub scale_width: NumberViewModel,
    /// Height of an exact scale.
    pub scale_height: NumberViewModel,
    /// Shape field.
    pub aspect_ratio: crate::tabs::common::TextViewModel,
    /// Largest width the output may have.
    pub max_width: NumberViewModel,
    /// Largest height the output may have.
    pub max_height: NumberViewModel,
    /// Turn picker.
    pub rotate: ChoiceGroupViewModel,
    /// Mirror picker.
    pub flip: ChoiceGroupViewModel,
    /// The frame the encoder will see.
    pub output_label: String,
    /// One line describing what the tab will do.
    pub summary: String,
    /// What has to be fixed before the job can be queued.
    pub problems: Vec<String>,
    /// Whether the tab's choices can be used.
    pub ready: bool,
}

/// Build the dimensions tab.
pub fn tab_view(settings: &DimensionsSettings, facts: &TitleFacts) -> DimensionsTabViewModel {
    let mut problems: Vec<String> = Vec::new();
    let frame_reason = if facts.has_frame_size() {
        String::new()
    } else {
        "The scan reported no frame size for this title.".to_string()
    };

    let crop_options = ChoiceGroup::of([
        ("none", "Keep the whole frame"),
        ("manual", "A rectangle I choose"),
        ("aspect", "A shape I choose"),
    ]);
    let crop = settings.crop();
    let crop_mode = ChoiceGroupViewModel::build(
        "cropMode",
        "Crop",
        "How much of the frame is kept.",
        crop.as_str(),
        &crop_options.options,
        true,
        "",
    );

    let scale_options = ChoiceGroup::of([
        ("keep", "Keep the source size"),
        ("exact", "Exactly this size"),
        ("fit", "Fit inside a size limit"),
        ("aspect", "This shape"),
    ]);
    let scale = settings.scale();
    let scale_mode = ChoiceGroupViewModel::build(
        "scaleMode",
        "Scaling",
        "How the picture is resized.",
        scale.as_str(),
        &scale_options.options,
        true,
        "",
    );

    // Every field that the current modes do not read says so and why.
    let crop_needs_frame = !frame_reason.is_empty();
    let manual = |field: NumberViewModel, what: &str| -> NumberViewModel {
        if crop == CropMode::Manual {
            field
        } else if crop == CropMode::AspectRatio {
            field.blocked(format!(
                "Cropping to a shape computes the rectangle itself, so {what} is not used."
            ))
        } else {
            field.blocked("The whole frame is kept, so there is nothing to crop.".to_string())
        }
    };
    let mut crop_x = manual(
        NumberViewModel::new("cropX", "Left", settings.crop_x.text.trim(), "px")
            .ranged(0.0, 100_000.0),
        "the left edge",
    );
    let mut crop_y = manual(
        NumberViewModel::new("cropY", "Top", settings.crop_y.text.trim(), "px")
            .ranged(0.0, 100_000.0),
        "the top edge",
    );
    let mut crop_width = manual(
        NumberViewModel::new("cropWidth", "Width", settings.crop_width.text.trim(), "px")
            .ranged(2.0, 100_000.0),
        "the width",
    );
    let mut crop_height = manual(
        NumberViewModel::new(
            "cropHeight",
            "Height",
            settings.crop_height.text.trim(),
            "px",
        )
        .ranged(2.0, 100_000.0),
        "the height",
    );
    if crop_needs_frame {
        for field in [&mut crop_x, &mut crop_y, &mut crop_width, &mut crop_height] {
            *field = field.clone().blocked(frame_reason.clone());
        }
    }

    let mut scale_width = NumberViewModel::new(
        "scaleWidth",
        "Width",
        settings.scale_width.text.trim(),
        "px",
    )
    .ranged(2.0, 100_000.0);
    let mut scale_height = NumberViewModel::new(
        "scaleHeight",
        "Height",
        settings.scale_height.text.trim(),
        "px",
    )
    .ranged(2.0, 100_000.0);
    let mut max_width = NumberViewModel::new(
        "maxWidth",
        "Largest width",
        settings.max_width.text.trim(),
        "px",
    )
    .ranged(2.0, 100_000.0);
    let mut max_height = NumberViewModel::new(
        "maxHeight",
        "Largest height",
        settings.max_height.text.trim(),
        "px",
    )
    .ranged(2.0, 100_000.0);

    match scale {
        ScaleMode::Keep => {
            for (field, what) in [
                (&mut scale_width, "a width"),
                (&mut scale_height, "a height"),
                (&mut max_width, "a width limit"),
                (&mut max_height, "a height limit"),
            ] {
                *field = field
                    .clone()
                    .blocked(format!("The source size is kept, so {what} is not used."));
            }
        }
        ScaleMode::Exact => {
            for (field, what) in [
                (&mut max_width, "a width limit"),
                (&mut max_height, "a height limit"),
            ] {
                *field = field
                    .clone()
                    .blocked(format!("An exact size is given, so {what} is not used."));
            }
        }
        ScaleMode::Fit => {
            for (field, what) in [
                (&mut scale_width, "a width"),
                (&mut scale_height, "a height"),
            ] {
                *field = field.clone().blocked(format!(
                    "A size limit decides the size, so {what} is not used."
                ));
            }
        }
        ScaleMode::Aspect => {
            for (field, what) in [
                (&mut scale_width, "a width"),
                (&mut scale_height, "a height"),
                (&mut max_width, "a width limit"),
                (&mut max_height, "a height limit"),
            ] {
                *field = field
                    .clone()
                    .blocked(format!("A shape decides the size, so {what} is not used."));
            }
        }
    }
    if scale != ScaleMode::Keep && !frame_reason.is_empty() {
        for field in [
            &mut scale_width,
            &mut scale_height,
            &mut max_width,
            &mut max_height,
        ] {
            *field = field.clone().blocked(frame_reason.clone());
        }
    }

    let aspect_needs = crop == CropMode::AspectRatio || scale == ScaleMode::Aspect;
    let mut aspect_ratio = crate::tabs::common::TextViewModel::new(
        "aspectRatio",
        "Shape",
        settings.aspect_ratio.trim(),
        "16:9",
    );
    if !aspect_needs {
        aspect_ratio = aspect_ratio
            .blocked("Choose a shape crop or a shape scale to use this field.".to_string());
    } else {
        if aspect_ratio_value(&settings.aspect_ratio).is_err() {
            let problem = aspect_ratio_value(&settings.aspect_ratio).unwrap_err();
            aspect_ratio = aspect_ratio.invalid(problem.clone());
            problems.push(problem);
        }
        if !frame_reason.is_empty() {
            aspect_ratio = aspect_ratio.blocked(frame_reason.clone());
        }
    }

    let rotate_options = ChoiceGroup::of([
        ("0", "No turn"),
        ("90", "Quarter turn clockwise"),
        ("180", "Half turn"),
        ("270", "Quarter turn anticlockwise"),
    ]);
    let rotate = ChoiceGroupViewModel::build(
        "rotate",
        "Turn",
        "Which way the finished picture is turned.",
        &settings.rotate,
        &rotate_options.options,
        true,
        "",
    );
    let flip_options = ChoiceGroup::of([
        ("none", "No mirroring"),
        ("horizontal", "Left to right"),
        ("vertical", "Top to bottom"),
        ("both", "Both ways"),
    ]);
    let flip = ChoiceGroupViewModel::build(
        "flip",
        "Mirror",
        "Which way the finished picture is mirrored.",
        settings.flip.as_str(),
        &flip_options.options,
        true,
        "",
    );

    // The two resolvers are the rules; whatever they say is what the tab shows.
    if let Err(problem) = crop_rect(settings, facts) {
        if crop == CropMode::Manual && problem.starts_with("Choose a crop") {
            crop_width = crop_width.invalid(problem.clone());
            crop_height = crop_height.invalid(problem.clone());
        }
        problems.push(problem);
    }
    if let Err(problem) = scale_target(settings, facts) {
        problems.push(problem);
    }

    let output_label = output_size(settings, facts);
    let summary = match (&crop_rect(settings, facts), &scale_target(settings, facts)) {
        (Ok(Some((x, y, w, h))), Ok(Some((sw, sh)))) => {
            format!("Crop to {w}x{h} at {x},{y}, then output {sw}x{sh}.")
        }
        (Ok(Some((x, y, w, h))), Ok(None)) => {
            format!("Crop to {w}x{h} at {x},{y}, keeping the rest of the size.")
        }
        (Ok(None), Ok(Some((sw, sh)))) => format!("Output {sw}x{sh}."),
        (Ok(None), Ok(None)) if settings.mirror() == Flip::None => {
            "The picture is passed through at its source size.".to_string()
        }
        _ => "The picture changes size or shape as the fields describe.".to_string(),
    };

    DimensionsTabViewModel {
        crop_mode,
        crop_x,
        crop_y,
        crop_width,
        crop_height,
        scale_mode,
        scale_width,
        scale_height,
        aspect_ratio,
        max_width,
        max_height,
        rotate,
        flip,
        output_label,
        summary,
        ready: problems.is_empty(),
        problems,
    }
}

/// The crop rectangle and the scale target, either of which may be absent.
pub type JobGeometry = (Option<(u32, u32, u32, u32)>, Option<(u32, u32)>);

/// The geometry as the job model wants it.
pub fn to_job_geometry(
    settings: &DimensionsSettings,
    facts: &TitleFacts,
) -> Result<JobGeometry, String> {
    Ok((crop_rect(settings, facts)?, scale_target(settings, facts)?))
}

/// The turn and mirror as the job model wants them.
pub fn to_job_geometry_orientation(
    settings: &DimensionsSettings,
) -> (Option<i32>, vise_core::FlipMode) {
    let rotation = match settings.rotation() {
        90 => Some(90),
        180 => Some(180),
        270 => Some(270),
        _ => None,
    };
    (rotation, settings.mirror().to_core())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> TitleFacts {
        TitleFacts {
            number: 1,
            width: 1920,
            height: 1080,
            frame_rate: 25.0,
            duration_seconds: 600.0,
            ..Default::default()
        }
    }

    #[test]
    fn the_defaults_keep_the_frame_as_it_is() {
        let settings = DimensionsSettings::default();
        let view = tab_view(&settings, &facts());
        assert!(view.ready, "{:?}", view.problems);
        assert_eq!(view.output_label, "1920x1080");
        assert_eq!(view.crop_mode.selected, "none");
        assert!(!view.crop_x.available, "a field nothing reads says so");
        assert!(view.crop_x.reason.contains("whole frame"));
        assert!(!view.scale_width.available);
        assert!(!view.aspect_ratio.available);
        assert!(view.summary.contains("source size"));
    }

    #[test]
    fn a_manual_crop_is_checked_against_the_frame() {
        let settings = DimensionsSettings {
            crop_mode: "manual".into(),
            crop_width: NumField::of(1280),
            crop_height: NumField::of(720),
            crop_x: NumField::of(320),
            crop_y: NumField::of(180),
            ..Default::default()
        };
        assert_eq!(
            crop_rect(&settings, &facts()).expect("rect"),
            Some((320, 180, 1280, 720))
        );
        let view = tab_view(&settings, &facts());
        assert!(view.ready, "{:?}", view.problems);
        assert!(view.summary.contains("1280x720"));

        let outside = DimensionsSettings {
            crop_mode: "manual".into(),
            crop_width: NumField::of(1920),
            crop_height: NumField::of(1080),
            crop_x: NumField::of(16),
            ..Default::default()
        };
        let problem = crop_rect(&outside, &facts()).expect_err("does not fit");
        assert!(problem.contains("does not fit"), "{problem}");
        assert!(!tab_view(&outside, &facts()).ready);
    }

    #[test]
    fn a_crop_must_be_even_or_the_encoder_refuses_it() {
        let settings = DimensionsSettings {
            crop_mode: "manual".into(),
            crop_width: NumField::of(1281),
            crop_height: NumField::of(720),
            ..Default::default()
        };
        let problem = crop_rect(&settings, &facts()).expect_err("odd width");
        assert!(problem.contains("even dimensions"), "{problem}");
    }

    #[test]
    fn a_shape_crop_centres_the_largest_rectangle_it_can() {
        let settings = DimensionsSettings {
            crop_mode: "aspect".into(),
            aspect_ratio: "4:3".into(),
            ..Default::default()
        };
        // 1920x1080 is wider than 4:3, so the height stays and the width drops.
        assert_eq!(
            crop_rect(&settings, &facts()).expect("rect"),
            Some((240, 0, 1440, 1080))
        );

        let tall = DimensionsSettings {
            crop_mode: "aspect".into(),
            aspect_ratio: "9:16".into(),
            ..Default::default()
        };
        assert_eq!(
            crop_rect(&tall, &facts()).expect("rect"),
            Some((657, 0, 606, 1080))
        );
        let view = tab_view(&tall, &facts());
        assert!(view.ready, "{:?}", view.problems);
        assert!(
            !view.crop_width.available,
            "the shape computes the rectangle"
        );
        assert!(view.crop_width.reason.contains("computes"));
    }

    #[test]
    fn a_shape_is_read_either_as_a_ratio_or_a_number() {
        assert!((aspect_ratio_value("16:9").expect("ratio") - 16.0 / 9.0).abs() < 0.0001);
        assert!((aspect_ratio_value("1.78").expect("decimal") - 1.78).abs() < 0.0001);
        assert!(aspect_ratio_value("wide")
            .expect_err("text")
            .contains("16:9"));
        assert!(aspect_ratio_value("").expect_err("blank").contains("16:9"));
        assert!(aspect_ratio_value("0:9")
            .expect_err("zero")
            .contains("above zero"));
    }

    #[test]
    fn a_shape_crop_needs_a_frame_size() {
        let settings = DimensionsSettings {
            crop_mode: "aspect".into(),
            aspect_ratio: "16:9".into(),
            ..Default::default()
        };
        let silent = TitleFacts::default();
        assert!(crop_rect(&settings, &silent)
            .expect_err("no frame")
            .contains("frame size"));
        let view = tab_view(&settings, &silent);
        assert!(!view.ready);
        assert!(!view.aspect_ratio.available);
        assert!(view.aspect_ratio.reason.contains("frame size"));
    }

    #[test]
    fn an_exact_scale_is_even_and_used_as_typed() {
        let settings = DimensionsSettings {
            scale_mode: "exact".into(),
            scale_width: NumField::of(1280),
            scale_height: NumField::of(720),
            ..Default::default()
        };
        assert_eq!(
            scale_target(&settings, &facts()).expect("target"),
            Some((1280, 720))
        );
        assert_eq!(output_size(&settings, &facts()), "1280x720");

        let odd = DimensionsSettings {
            scale_mode: "exact".into(),
            scale_width: NumField::of(1281),
            scale_height: NumField::of(720),
            ..Default::default()
        };
        assert!(scale_target(&odd, &facts())
            .expect_err("odd")
            .contains("even dimensions"));

        let missing = DimensionsSettings {
            scale_mode: "exact".into(),
            scale_width: NumField::of(1280),
            ..Default::default()
        };
        assert!(scale_target(&missing, &facts())
            .expect_err("no height")
            .contains("height"));
    }

    #[test]
    fn a_size_limit_fits_inside_the_frame_without_enlarging_it() {
        let settings = DimensionsSettings {
            scale_mode: "fit".into(),
            max_width: NumField::of(1280),
            ..Default::default()
        };
        // 1920x1080 fitted to 1280 wide is 1280x720.
        assert_eq!(
            scale_target(&settings, &facts()).expect("fit"),
            Some((1280, 720))
        );

        let generous = DimensionsSettings {
            scale_mode: "fit".into(),
            max_width: NumField::of(4000),
            ..Default::default()
        };
        assert_eq!(
            scale_target(&generous, &facts()).expect("no enlargement"),
            Some((1920, 1080)),
            "a limit above the source size leaves the source size alone"
        );

        let none = DimensionsSettings {
            scale_mode: "fit".into(),
            ..Default::default()
        };
        assert!(scale_target(&none, &facts())
            .expect_err("no limit")
            .contains("at least one size limit"));

        let odd = DimensionsSettings {
            scale_mode: "fit".into(),
            max_height: NumField::of(721),
            ..Default::default()
        };
        assert!(scale_target(&odd, &facts())
            .expect_err("odd limit")
            .contains("even"));
    }

    #[test]
    fn a_quarter_turn_swaps_what_the_size_limit_means() {
        // After the turn the frame is 1080x1920, so a 1280 height limit is a
        // real limit and the result is 720x1280.
        let tall_limit = DimensionsSettings {
            scale_mode: "fit".into(),
            max_height: NumField::of(1280),
            rotate: "90".into(),
            ..Default::default()
        };
        assert_eq!(
            scale_target(&tall_limit, &facts()).expect("turned"),
            Some((720, 1280))
        );
        assert_eq!(output_size(&tall_limit, &facts()), "720x1280");

        // A 1280 width limit is wider than the turned frame already is, so it
        // leaves the size alone rather than enlarging the picture.
        let wide_limit = DimensionsSettings {
            scale_mode: "fit".into(),
            max_width: NumField::of(1280),
            rotate: "90".into(),
            ..Default::default()
        };
        assert_eq!(
            scale_target(&wide_limit, &facts()).expect("no change"),
            Some((1080, 1920))
        );
    }

    #[test]
    fn a_crop_is_measured_against_the_frame_before_the_turn() {
        let settings = DimensionsSettings {
            crop_mode: "manual".into(),
            crop_width: NumField::of(1920),
            crop_height: NumField::of(1080),
            rotate: "90".into(),
            ..Default::default()
        };
        assert_eq!(
            crop_rect(&settings, &facts()).expect("rect"),
            Some((0, 0, 1920, 1080)),
            "the crop runs before the turn"
        );
        assert_eq!(output_size(&settings, &facts()), "1080x1920");
    }

    #[test]
    fn unknown_names_and_bad_turns_are_refused() {
        let mut settings = DimensionsSettings::default();
        let crop = DimensionsInput {
            crop_mode: Some("sideways".into()),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect_err("unknown crop mode");
        assert!(crop.contains("sideways"), "{crop}");

        let scale = DimensionsInput {
            scale_mode: Some("stretch".into()),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect_err("unknown scale mode");
        assert!(scale.contains("stretch"), "{scale}");

        let flip = DimensionsInput {
            flip: Some("diagonal".into()),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect_err("unknown flip");
        assert!(flip.contains("diagonal"), "{flip}");

        let rotate = DimensionsInput {
            rotate: Some("45".into()),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect_err("odd turn");
        assert!(rotate.contains("0, 90, 180, or 270"), "{rotate}");

        DimensionsInput {
            rotate: Some("270".into()),
            flip: Some("both".into()),
            crop_mode: Some("aspect".into()),
            scale_mode: Some("fit".into()),
            aspect_ratio: Some("21:9".into()),
            ..Default::default()
        }
        .apply(&mut settings)
        .expect("valid change");
        assert_eq!(settings.rotate, "270");
        assert_eq!(settings.flip, "both");
        assert_eq!(settings.crop(), CropMode::AspectRatio);
        assert_eq!(settings.scale(), ScaleMode::Fit);
        assert_eq!(settings.rotation(), 270);
        assert_eq!(settings.mirror(), Flip::Both);
        assert_eq!(
            to_job_geometry_orientation(&settings),
            (Some(270), vise_core::FlipMode::Both)
        );
    }

    #[test]
    fn the_geometry_hands_the_job_a_rectangle_and_a_target() {
        let settings = DimensionsSettings {
            crop_mode: "manual".into(),
            crop_width: NumField::of(1900),
            crop_height: NumField::of(1000),
            scale_mode: "exact".into(),
            scale_width: NumField::of(950),
            scale_height: NumField::of(500),
            ..Default::default()
        };
        let (crop, scale) = to_job_geometry(&settings, &facts()).expect("geometry");
        assert_eq!(crop, Some((0, 0, 1900, 1000)));
        assert_eq!(scale, Some((950, 500)));
    }
}
