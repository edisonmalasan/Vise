//! Preset and configuration persistence for Vise Transcoder.
//!
//! This module provides built-in and user presets, application of presets to
//! jobs, storage of user presets in a JSON file, import/export helpers, and
//! application settings persistence. It has no UI dependency.

use crate::{
    AudioCodec, ContainerFormat, CoreError, EncoderId, EncodingMode, JobSpec, SubtitleMode,
    VideoCodec, VideoEncodingOptions,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PresetSpec {
    #[serde(default)]
    pub container: Option<ContainerFormat>,
    #[serde(default)]
    pub format: Option<ContainerFormat>,

    #[serde(default)]
    pub video_codec: Option<VideoCodec>,
    #[serde(default)]
    pub video: Option<VideoCodec>,

    #[serde(default)]
    pub audio_codec: Option<AudioCodec>,
    #[serde(default)]
    pub audio: Option<AudioCodec>,

    /// The encoder the preset is written for.
    ///
    /// A codec says what is in the file; this says which encoder writes it, and
    /// it is what makes a hardware preset a hardware preset. Availability is
    /// resolved against the machine at selection time by
    /// [`crate::catalog::resolve_availability`], never guessed from the codec.
    #[serde(default)]
    pub encoder: Option<EncoderId>,

    /// Encoder-side coding choices: speed preset, profile, tune, level, range.
    ///
    /// The quality families in the general catalog are mostly this member, so it
    /// travels with the preset rather than being retyped on the video tab.
    #[serde(default)]
    pub video_encoding: Option<VideoEncodingOptions>,

    /// How the encoder is asked to spend its bits.
    #[serde(default)]
    pub encoding_mode: Option<EncodingMode>,

    /// The size the encoder should aim for, in bytes, when the mode is
    /// [`EncodingMode::TargetSize`].
    #[serde(default)]
    pub target_size: Option<u64>,

    /// The longest title this preset is meant for, in seconds.
    ///
    /// A size-targeted preset is only honest for the length it was designed
    /// around, so the cap travels with it. It describes the preset rather than
    /// the job, so [`apply_preset`] carries it as far as the preset and leaves
    /// the range to whoever applies it.
    #[serde(default)]
    pub duration_cap_seconds: Option<u64>,

    #[serde(default)]
    pub subtitle_mode: Option<SubtitleMode>,
    #[serde(default)]
    pub subtitles: Option<SubtitleMode>,

    #[serde(default)]
    pub title_index: Option<u32>,
    #[serde(default)]
    pub title: Option<u32>,

    #[serde(default)]
    pub crop: Option<(u32, u32, u32, u32)>,
    #[serde(default)]
    pub scale_to: Option<(u32, u32)>,
    #[serde(default)]
    pub quality: Option<u32>,

    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

impl PresetSpec {
    pub fn apply_to(&self, job: &JobSpec) -> JobSpec {
        apply_preset(self, job)
    }

    /// The container the preset asks for, whichever member carries it.
    pub fn container_of(&self) -> Option<ContainerFormat> {
        self.container.or(self.format)
    }

    /// The video codec the preset asks for, whichever member carries it.
    pub fn video_codec_of(&self) -> Option<VideoCodec> {
        self.video_codec.or(self.video)
    }

    /// The audio codec the preset asks for, whichever member carries it.
    pub fn audio_codec_of(&self) -> Option<AudioCodec> {
        self.audio_codec.or(self.audio)
    }

    /// The subtitle handling the preset asks for, whichever member carries it.
    pub fn subtitle_mode_of(&self) -> Option<SubtitleMode> {
        self.subtitle_mode.or(self.subtitles)
    }

    /// The title the preset names, whichever member carries it.
    pub fn title_index_of(&self) -> Option<u32> {
        self.title_index.or(self.title)
    }

    /// This preset in its canonical form.
    ///
    /// Cleaning is what makes two presets that mean the same thing compare equal
    /// and serialize the same way. Every duplicated member is folded into the one
    /// this build reads, blank text is dropped, and a geometry with nothing in it
    /// is dropped rather than kept as a value no encoder can act on. It changes
    /// nothing a preset means, and running it twice changes nothing the second
    /// time.
    pub fn cleaned(&self) -> Self {
        let mut out = self.clone();
        if out.container.is_none() {
            out.container = out.format;
        }
        out.format = None;
        if out.video_codec.is_none() {
            out.video_codec = out.video;
        }
        out.video = None;
        if out.audio_codec.is_none() {
            out.audio_codec = out.audio;
        }
        out.audio = None;
        if out.subtitle_mode.is_none() {
            out.subtitle_mode = out.subtitles;
        }
        out.subtitles = None;
        if out.title_index.is_none() {
            out.title_index = out.title;
        }
        out.title = None;
        out.name = trimmed(out.name.take());
        out.description = trimmed(out.description.take());
        out.scale_to = out
            .scale_to
            .filter(|(width, height)| *width > 0 && *height > 0);
        out.crop = out
            .crop
            .filter(|(_, _, width, height)| *width > 0 && *height > 0);
        out
    }

    /// This preset brought in line with the catalog template.
    ///
    /// Normalizing is filling and then pruning. Every member the preset leaves
    /// out is taken from the template, so a preset always has a container, a
    /// video codec, and an audio codec to answer with even when its file named
    /// none; the tuning members that still say exactly what the template says
    /// are then dropped again, so a preset that overrides nothing comes back
    /// holding nothing rather than holding a copy of the defaults. Doing it
    /// twice gives the same answer as doing it once.
    pub fn normalized(&self, template: &PresetSpec) -> Self {
        let template = template.cleaned();
        let mut out = self.cleaned();
        if out.container.is_none() {
            out.container = template.container;
        }
        if out.video_codec.is_none() {
            out.video_codec = template.video_codec;
        }
        if out.audio_codec.is_none() {
            out.audio_codec = template.audio_codec;
        }
        if out.subtitle_mode.is_none() {
            out.subtitle_mode = template.subtitle_mode;
        }
        if out.encoder.is_none() {
            out.encoder = template.encoder;
        }
        if out.encoding_mode.is_none() {
            out.encoding_mode = template.encoding_mode;
        }
        out.without_template_defaults(&template)
    }

    /// The preset with every tuning member the template already answers removed.
    ///
    /// This is the last half of [`PresetSpec::normalized`] and the question an
    /// imported file asks on its own: which of these values are really this
    /// preset's own rather than the defaults wearing this preset's name?
    ///
    /// Only tuning is pruned. The container and the codecs are what the preset
    /// *is* rather than how finely it is tuned, and a preset that says MP4 and
    /// H.264 must keep saying so even when the template happens to agree.
    fn without_template_defaults(mut self, template: &PresetSpec) -> Self {
        if self.subtitle_mode == template.subtitle_mode {
            self.subtitle_mode = None;
        }
        if self.encoder == template.encoder {
            self.encoder = None;
        }
        if self.encoding_mode == template.encoding_mode {
            self.encoding_mode = None;
        }
        if self.quality == template.quality {
            self.quality = None;
        }
        self
    }
}

/// A string that has something in it, or nothing rather than an empty string.
fn trimmed(value: Option<String>) -> Option<String> {
    value.and_then(|text| {
        let text = text.trim().to_string();
        (!text.is_empty()).then_some(text)
    })
}

pub type Preset = PresetSpec;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuiltInPreset {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub preset: PresetSpec,
}

pub type BuiltInPresetCatalog = Vec<BuiltInPreset>;

/// The presets that ship with the build, in catalog order.
///
/// The list is read from the embedded catalog rather than written here, so the
/// presets are data this build carries rather than code this build contains, and
/// the catalog is the one place a preset is described, versioned, and checked.
/// Each spec is normalized against the catalog template on the way out, so a
/// caller never has to know which members the template already answers.
pub fn builtin_presets() -> Vec<BuiltInPreset> {
    crate::catalog::builtin_entries()
        .iter()
        .map(|entry| entry.to_builtin())
        .collect()
}

/// The preset with this identifier.
///
/// An identifier an earlier build stored still resolves, through the catalog's
/// alias table, so a settings file naming one of them keeps naming the preset it
/// meant after the entries were put on the catalog's own scheme.
pub fn builtin_preset(id: &str) -> Option<BuiltInPreset> {
    let wanted = crate::catalog::resolve_builtin_id(id)?;
    builtin_presets().into_iter().find(|p| p.id == wanted)
}

impl BuiltInPreset {
    pub fn all() -> Vec<Self> {
        builtin_presets()
    }
    pub fn by_id(id: &str) -> Option<Self> {
        builtin_preset(id)
    }
}

pub fn apply_preset(preset: &PresetSpec, job: &JobSpec) -> JobSpec {
    let mut updated = job.clone();
    if let Some(v) = preset.container_of() {
        updated.container = v;
    }
    if let Some(v) = preset.video_codec_of() {
        updated.video_codec = v;
    }
    if let Some(v) = preset.audio_codec_of() {
        updated.audio_codec = v;
    }
    if let Some(v) = preset.subtitle_mode_of() {
        updated.subtitle_mode = v;
    }
    if let Some(v) = preset.title_index_of() {
        updated.title_index = Some(v);
    }
    if let Some(v) = preset.crop {
        updated.crop = Some(v);
    }
    if let Some(v) = preset.scale_to {
        updated.scale_to = Some(v);
    }
    if let Some(v) = preset.quality {
        updated.quality = Some(v);
    }
    apply_rate_control(preset, &mut updated);
    apply_encoder_settings(preset, &mut updated);
    updated
}

/// Write the rate control a preset asks for onto a job.
///
/// The mode is written to the named member and to the flag beside it, because a
/// job can be read by either: a caller that only sets `target_size_mode` and a
/// caller that only sets `encoding_mode` mean the same thing, and which of them
/// a given reader looks at is not something a preset should depend on.
fn apply_rate_control(preset: &PresetSpec, job: &mut JobSpec) {
    let Some(mode) = preset.encoding_mode else {
        return;
    };
    job.encoding_mode = Some(mode);
    job.constant_quality = mode.is_constant_quality();
    job.average_bitrate_mode = mode.is_average_bitrate();
    job.target_size_mode = mode.is_target_size();
    job.two_pass = mode.is_two_pass();
    job.two_pass_encoding = job.two_pass;
    job.lossless = mode.is_lossless();
    job.lossless_encoding = job.lossless;
    if let Some(size) = preset.target_size {
        job.target_size = Some(size);
        job.target_file_size = Some(size);
        job.target_size_bytes = Some(size);
    }
}

/// Write the encoder a preset is written for onto a job.
///
/// The encoder name goes where the pipeline reads it, which is the job's
/// hardware-encode options, so a hardware preset reaches the encoder it was
/// written for instead of being re-decided from the codec at encode time.
fn apply_encoder_settings(preset: &PresetSpec, job: &mut JobSpec) {
    if let Some(encoding) = preset.video_encoding.clone() {
        let mut merged = job.video_encoding.clone();
        merged.preset = encoding.preset.clone().or(merged.preset);
        merged.profile = encoding.profile.clone().or(merged.profile);
        merged.tune = encoding.tune.clone().or(merged.tune);
        merged.level = encoding.level.clone().or(merged.level);
        merged.colour_range = encoding.colour_range.or(merged.colour_range);
        job.video_encoding = merged;
    }
    let Some(encoder) = preset.encoder else {
        return;
    };
    job.hardware_acceleration.encode.encoder = Some(encoder.identifier().to_string());
    if !encoder.is_hardware() {
        return;
    }
    let api = encoder.hardware_api();
    job.hardware_acceleration.encode.enabled = true;
    job.hardware_acceleration.encode.api = api;
    job.hardware_acceleration.api = api;
    if !job.hardware_acceleration.enabled {
        job.hardware_acceleration.enabled = true;
        job.hardware_acceleration.mode = crate::HardwareMode::Auto;
    }
}

pub fn apply(preset: &PresetSpec, job: &JobSpec) -> JobSpec {
    apply_preset(preset, job)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserPreset {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub preset: PresetSpec,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
struct PresetFile {
    #[serde(default = "default_version")]
    version: u32,
    #[serde(default, alias = "items", alias = "presets")]
    presets: Vec<UserPreset>,
}

fn default_version() -> u32 {
    1
}

pub fn load_user_presets(path: &Path) -> Result<Vec<UserPreset>, CoreError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(path).map_err(|e| CoreError::Io(e.to_string()))?;
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    // Try array of UserPreset first.
    if let Ok(list) = serde_json::from_str::<Vec<UserPreset>>(&raw) {
        return Ok(list);
    }
    // Try file wrapper.
    if let Ok(file) = serde_json::from_str::<PresetFile>(&raw) {
        return Ok(file.presets);
    }
    // Try array of PresetSpec (generate ids/names).
    if let Ok(list) = serde_json::from_str::<Vec<PresetSpec>>(&raw) {
        return Ok(list
            .into_iter()
            .enumerate()
            .map(|(i, p)| UserPreset {
                id: format!("user-{}", i + 1),
                name: p
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("Preset {}", i + 1)),
                description: p.description.clone(),
                preset: p,
            })
            .collect());
    }
    Err(CoreError::Parse("invalid preset file".to_string()))
}

pub fn save_user_presets(path: &Path, presets: &[UserPreset]) -> Result<(), CoreError> {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let file = PresetFile {
        version: 1,
        presets: presets.to_vec(),
    };
    let raw = serde_json::to_string_pretty(&file).map_err(|e| CoreError::Parse(e.to_string()))?;
    fs::write(path, raw).map_err(|e| CoreError::Io(e.to_string()))
}

pub fn load_presets(path: &Path) -> Result<Vec<UserPreset>, CoreError> {
    load_user_presets(path)
}
pub fn read_user_presets(path: &Path) -> Result<Vec<UserPreset>, CoreError> {
    load_user_presets(path)
}
pub fn list_user_presets(path: &Path) -> Result<Vec<UserPreset>, CoreError> {
    load_user_presets(path)
}

fn now_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{prefix}-{nanos}")
}

pub fn create_user_preset(
    path: &Path,
    name: &str,
    preset: PresetSpec,
) -> Result<UserPreset, CoreError> {
    let mut presets = load_user_presets(path)?;
    let up = UserPreset {
        id: now_id("user"),
        name: name.to_string(),
        description: preset.description.clone(),
        preset,
    };
    presets.push(up.clone());
    save_user_presets(path, &presets)?;
    Ok(up)
}

pub fn create_preset(path: &Path, name: &str, preset: PresetSpec) -> Result<UserPreset, CoreError> {
    create_user_preset(path, name, preset)
}

/// Store a copy of a preset under a new name.
///
/// The copy is deep: the settings are read out of the original and stored in a
/// new record, so changing the copy cannot reach back into what it was copied
/// from. A copy is the only way to change a preset that ships with the build, so
/// a shared one would let an edit of the copy rewrite the shipped preset for
/// everybody who opens the list afterwards.
pub fn clone_user_preset(
    path: &Path,
    id: &str,
    name: Option<&str>,
) -> Result<UserPreset, CoreError> {
    let mut presets = load_user_presets(path)?;
    let Some(source) = presets.iter().find(|preset| preset.id == id) else {
        return Err(CoreError::Invalid(format!("preset not found: {id}")));
    };
    let mut copy = source.clone();
    copy.id = now_id("user");
    copy.name = match name.map(str::trim).filter(|name| !name.is_empty()) {
        Some(wanted) => wanted.to_string(),
        None => crate::catalog::copy_suffix(&source.name),
    };
    copy.preset.name = Some(copy.name.clone());
    presets.push(copy.clone());
    save_user_presets(path, &presets)?;
    Ok(copy)
}

/// Move a stored preset `delta` places through the library.
///
/// The library is left exactly as it was when the move would take the preset off
/// either end, so a page that asks for "one further up" at the top gets nothing
/// rather than a wrap-around it did not ask for.
pub fn move_user_preset(path: &Path, id: &str, delta: i32) -> Result<bool, CoreError> {
    let mut presets = load_user_presets(path)?;
    let moved = crate::catalog::move_entry_by(&mut presets, id, delta, |preset| preset.id.as_str());
    if moved {
        save_user_presets(path, &presets)?;
    }
    Ok(moved)
}

/// Put a stored preset at an absolute place in the library.
pub fn place_user_preset(path: &Path, id: &str, target: usize) -> Result<bool, CoreError> {
    let mut presets = load_user_presets(path)?;
    let moved =
        crate::catalog::move_entry_to(&mut presets, id, target, |preset| preset.id.as_str());
    if moved {
        save_user_presets(path, &presets)?;
    }
    Ok(moved)
}

/// The identifiers a stored preset list is in, in order.
pub fn user_preset_order(presets: &[UserPreset]) -> Vec<String> {
    presets.iter().map(|preset| preset.id.clone()).collect()
}

pub fn update_user_preset(
    path: &Path,
    id: &str,
    name: Option<&str>,
    preset: Option<PresetSpec>,
) -> Result<UserPreset, CoreError> {
    let mut presets = load_user_presets(path)?;
    let idx = presets
        .iter()
        .position(|p| p.id == id)
        .ok_or_else(|| CoreError::Invalid(format!("preset not found: {id}")))?;
    if let Some(n) = name {
        presets[idx].name = n.to_string();
    }
    if let Some(p) = preset {
        presets[idx].description = p.description.clone();
        presets[idx].preset = p;
    }
    let updated = presets[idx].clone();
    save_user_presets(path, &presets)?;
    Ok(updated)
}

pub fn edit_user_preset(
    path: &Path,
    id: &str,
    name: Option<&str>,
    preset: Option<PresetSpec>,
) -> Result<UserPreset, CoreError> {
    update_user_preset(path, id, name, preset)
}
pub fn update_preset(
    path: &Path,
    id: &str,
    name: Option<&str>,
    preset: Option<PresetSpec>,
) -> Result<UserPreset, CoreError> {
    update_user_preset(path, id, name, preset)
}
pub fn edit_preset(
    path: &Path,
    id: &str,
    name: Option<&str>,
    preset: Option<PresetSpec>,
) -> Result<UserPreset, CoreError> {
    update_user_preset(path, id, name, preset)
}

pub fn delete_user_preset(path: &Path, id: &str) -> Result<(), CoreError> {
    let mut presets = load_user_presets(path)?;
    presets.retain(|p| p.id != id);
    save_user_presets(path, &presets)
}
pub fn delete_preset(path: &Path, id: &str) -> Result<(), CoreError> {
    delete_user_preset(path, id)
}
pub fn remove_user_preset(path: &Path, id: &str) -> Result<(), CoreError> {
    delete_user_preset(path, id)
}
pub fn remove_preset(path: &Path, id: &str) -> Result<(), CoreError> {
    delete_user_preset(path, id)
}

pub struct UserPresetStore {
    path: PathBuf,
}

pub type PresetStore = UserPresetStore;
pub type UserPresetLibrary = UserPresetStore;

impl UserPresetStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn load(&self) -> Result<Vec<UserPreset>, CoreError> {
        load_user_presets(&self.path)
    }
    pub fn list(&self) -> Result<Vec<UserPreset>, CoreError> {
        self.load()
    }
    pub fn get(&self, id: &str) -> Result<Option<UserPreset>, CoreError> {
        Ok(self.load()?.into_iter().find(|p| p.id == id))
    }
    pub fn save(&self, presets: &[UserPreset]) -> Result<(), CoreError> {
        save_user_presets(&self.path, presets)
    }
    pub fn create(&self, name: &str, preset: PresetSpec) -> Result<UserPreset, CoreError> {
        create_user_preset(&self.path, name, preset)
    }
    pub fn create_preset(&self, name: &str, preset: PresetSpec) -> Result<UserPreset, CoreError> {
        self.create(name, preset)
    }
    pub fn update(
        &self,
        id: &str,
        name: Option<&str>,
        preset: Option<PresetSpec>,
    ) -> Result<UserPreset, CoreError> {
        update_user_preset(&self.path, id, name, preset)
    }
    pub fn edit(
        &self,
        id: &str,
        name: Option<&str>,
        preset: Option<PresetSpec>,
    ) -> Result<UserPreset, CoreError> {
        self.update(id, name, preset)
    }
    pub fn update_preset(
        &self,
        id: &str,
        name: Option<&str>,
        preset: Option<PresetSpec>,
    ) -> Result<UserPreset, CoreError> {
        self.update(id, name, preset)
    }
    pub fn edit_preset(
        &self,
        id: &str,
        name: Option<&str>,
        preset: Option<PresetSpec>,
    ) -> Result<UserPreset, CoreError> {
        self.update(id, name, preset)
    }
    pub fn delete(&self, id: &str) -> Result<(), CoreError> {
        delete_user_preset(&self.path, id)
    }
    pub fn remove(&self, id: &str) -> Result<(), CoreError> {
        self.delete(id)
    }
    pub fn delete_preset(&self, id: &str) -> Result<(), CoreError> {
        self.delete(id)
    }
    /// Store a copy of a preset under a new name.
    pub fn clone(&self, id: &str, name: Option<&str>) -> Result<UserPreset, CoreError> {
        clone_user_preset(&self.path, id, name)
    }
    /// Move a preset through the library.
    pub fn move_by(&self, id: &str, delta: i32) -> Result<bool, CoreError> {
        move_user_preset(&self.path, id, delta)
    }
    /// Put a preset at a named place in the library.
    pub fn place(&self, id: &str, target: usize) -> Result<bool, CoreError> {
        place_user_preset(&self.path, id, target)
    }
}

pub fn import_preset(path: &Path) -> Result<PresetSpec, CoreError> {
    let raw = fs::read_to_string(path).map_err(|e| CoreError::Io(e.to_string()))?;
    serde_json::from_str(&raw).map_err(|e| CoreError::Parse(e.to_string()))
}
pub fn import_preset_file(path: &Path) -> Result<PresetSpec, CoreError> {
    import_preset(path)
}

/// Read a stored preset out of a file.
///
/// Two shapes are read, in this order: the versioned document this build writes,
/// and the bare record an earlier build wrote. The versioned one is tried first
/// because it is the one that says which schema it is in; a file that declares a
/// version this build cannot read is refused rather than falling through to the
/// second shape, which would read a newer file's settings as an older record and
/// lose whatever the newer build had put in it.
pub fn import_user_preset(path: &Path) -> Result<UserPreset, CoreError> {
    let raw = fs::read_to_string(path).map_err(|e| CoreError::Io(e.to_string()))?;
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
        if value.get("version").is_some() || value.get("entry").is_some() {
            return crate::catalog::read_preset_document(&value).map(|document| {
                let entry = document.entry;
                UserPreset {
                    id: entry.id,
                    name: entry.name,
                    description: Some(entry.description),
                    preset: entry.spec,
                }
            });
        }
    }
    serde_json::from_str(&raw).map_err(|e| CoreError::Parse(e.to_string()))
}
pub fn export_preset(path: &Path, preset: &PresetSpec) -> Result<(), CoreError> {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let raw = serde_json::to_string_pretty(preset).map_err(|e| CoreError::Parse(e.to_string()))?;
    fs::write(path, raw).map_err(|e| CoreError::Io(e.to_string()))
}
pub fn export_preset_file(path: &Path, preset: &PresetSpec) -> Result<(), CoreError> {
    export_preset(path, preset)
}

/// Write a stored preset to a file as a versioned document.
///
/// The version travels with the preset, so a file this build writes says which
/// schema it is in rather than leaving a reader to work it out from the shape.
pub fn export_user_preset(path: &Path, preset: &UserPreset) -> Result<(), CoreError> {
    let document = crate::catalog::PresetDocument::new(user_preset_to_entry(
        preset,
        crate::catalog::PresetCategory::General,
    ));
    crate::catalog::export_preset_document(path, &document)
}

/// A stored preset written as a catalog entry.
pub fn user_preset_to_entry(
    preset: &UserPreset,
    category: crate::catalog::PresetCategory,
) -> crate::catalog::PresetEntry {
    crate::catalog::PresetEntry {
        id: preset.id.clone(),
        name: preset.name.clone(),
        description: preset.description.clone().unwrap_or_default(),
        category,
        family: None,
        encoder: preset.preset.encoder,
        compatibility: None,
        spec: preset.preset.clone(),
    }
}

/// A catalog entry read as a stored preset.
pub fn entry_to_user_preset(entry: &crate::catalog::PresetEntry) -> UserPreset {
    UserPreset {
        id: entry.id.clone(),
        name: entry.name.clone(),
        description: Some(entry.description.clone()),
        preset: entry.spec.clone(),
    }
}

// Settings persistence

/// Version stamped into every settings file this build writes.
///
/// The number is the whole point of the member: a settings file that does not
/// say which build wrote it can only be read by guessing, and a guess is either
/// too careful to ever change anything or too eager to drop something a user
/// set. With the number in the file, a build can say which of the two it is
/// doing before it reads anything else.
pub const SETTINGS_VERSION: u32 = 2;

/// Version a settings file is read as when it declares none.
///
/// Files written before the version existed are read as this version and are
/// migrated forward on the next save, which is why no data is moved here: the
/// members themselves are unchanged, and only the declaration is added.
pub const SETTINGS_UNVERSIONED: u32 = 1;

/// Members of a settings document that are named rather than bagged.
///
/// Everything else a file carries lands in [`AppSettings::extra`] untouched,
/// which is what lets a file written by a newer build be read without losing
/// the members this build has never heard of.
pub const NAMED_SETTINGS: [&str; 5] = [
    "version",
    "default_preset",
    "output_directory",
    "recent_input",
    "user_presets_path",
];

/// Application settings as they are stored.
///
/// `version` is written by [`save_settings`] and read back by
/// [`load_settings`]. It is the number the file declared before this build
/// touched it, which is how the migration questions below are answered. It is
/// never written back smaller than the number that was found, so a file a newer
/// build wrote stays readable by that build after this one saves it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// Version this build writes, and the version it found in the file.
    #[serde(default = "current_settings_version")]
    pub version: u32,
    #[serde(default)]
    pub default_preset: Option<String>,
    #[serde(default)]
    pub output_directory: Option<String>,
    #[serde(default)]
    pub recent_input: Option<String>,
    #[serde(default)]
    pub user_presets_path: Option<String>,
    #[serde(default, flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
    /// Version the file on disk declared, or zero when it declared none.
    ///
    /// Bookkeeping rather than content: it is never written back, and it is not
    /// part of equality, so a settings document read from a file still equals
    /// the document that was saved to it.
    #[serde(skip)]
    pub written_version: u32,
}

/// The version a settings file this build writes carries.
fn current_settings_version() -> u32 {
    SETTINGS_VERSION
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            default_preset: None,
            output_directory: None,
            recent_input: None,
            user_presets_path: None,
            extra: std::collections::BTreeMap::new(),
            written_version: 0,
        }
    }
}

impl AppSettings {
    /// Whether the file this was read from predates this build.
    ///
    /// An unversioned file counts: it is read as [`SETTINGS_UNVERSIONED`] and
    /// brought forward on the next save.
    pub fn needs_migration(&self) -> bool {
        self.written_version > 0 && self.written_version < SETTINGS_VERSION
    }

    /// Whether the file this was read from was written by a newer build.
    ///
    /// True means this build cannot be sure what the file means, so anything it
    /// does not recognise has to be left exactly where it found it.
    pub fn from_the_future(&self) -> bool {
        self.written_version > SETTINGS_VERSION
    }

    /// The version the file on disk declared, in words, for a status line.
    pub fn written_version_label(&self) -> String {
        if self.written_version == 0 {
            "no version".to_string()
        } else {
            format!("version {}", self.written_version)
        }
    }
}

// Equality is about the document, not about how it was read: the two
// bookkeeping members are skipped so that a settings document read back from
// disk equals the one that was written to it.
impl PartialEq for AppSettings {
    fn eq(&self, other: &Self) -> bool {
        self.version == other.version
            && self.default_preset == other.default_preset
            && self.output_directory == other.output_directory
            && self.recent_input == other.recent_input
            && self.user_presets_path == other.user_presets_path
            && self.extra == other.extra
    }
}

pub type Settings = AppSettings;
pub type AppConfig = AppSettings;
pub type Config = AppSettings;
pub type ViseSettings = AppSettings;

/// Read one settings document, whatever version wrote it.
///
/// The document is read member by member rather than decoded in one go, and
/// that is the difference between losing one wrong value and losing the file.
/// A `default_preset` that is not a string costs that one member; every other
/// member, and every member of the bag, survives. Anything this build does not
/// recognise is kept verbatim, so a file a newer build wrote is read as far as
/// it can be and written back without being flattened by an older reader.
///
/// A document that is not an object at all is refused: there is nothing in it
/// to read, and guessing would mean writing a file over somebody else's.
pub fn read_settings(document: &serde_json::Value) -> Result<AppSettings, CoreError> {
    let Some(members) = document.as_object() else {
        return Err(CoreError::Parse(
            "a settings file has to be a JSON object".to_string(),
        ));
    };
    // A version that is not a whole number cannot be read as one, so the file is
    // treated as unversioned rather than refused: it is migrated forward and
    // the wrong member is dropped, which loses nothing that could be recovered.
    let written_version = members
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .map(|version| version.min(u64::from(u32::MAX)) as u32)
        .unwrap_or(SETTINGS_UNVERSIONED);
    let mut settings = AppSettings {
        written_version,
        default_preset: named_string(members, "default_preset"),
        output_directory: named_string(members, "output_directory"),
        recent_input: named_string(members, "recent_input"),
        user_presets_path: named_string(members, "user_presets_path"),
        ..AppSettings::default()
    };
    for (name, member) in members {
        if NAMED_SETTINGS.contains(&name.as_str()) {
            continue;
        }
        settings.extra.insert(name.clone(), member.clone());
    }
    // Never write a version below the one that was found. A file a newer build
    // wrote keeps its number, so that build can still read what this one saved.
    settings.version = settings.written_version.max(SETTINGS_VERSION);
    Ok(settings)
}

/// A named string member, or nothing when it is absent or the wrong shape.
fn named_string(
    members: &serde_json::Map<String, serde_json::Value>,
    name: &str,
) -> Option<String> {
    members
        .get(name)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

/// Read the settings file at `path`.
///
/// A file that is missing, empty, or not valid JSON is an error rather than an
/// empty document: callers decide what an unreadable settings file means for
/// them, and a window that has been told a file is broken can say so.
pub fn load_settings(path: &Path) -> Result<AppSettings, CoreError> {
    if !path.exists() {
        return Ok(AppSettings::default());
    }
    let raw = fs::read_to_string(path).map_err(|e| CoreError::Io(e.to_string()))?;
    if raw.trim().is_empty() {
        return Ok(AppSettings::default());
    }
    let document: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| CoreError::Parse(format!("the settings file is not valid JSON: {e}")))?;
    read_settings(&document)
}

pub fn save_settings(path: &Path, settings: &AppSettings) -> Result<(), CoreError> {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let raw =
        serde_json::to_string_pretty(settings).map_err(|e| CoreError::Parse(e.to_string()))?;
    fs::write(path, raw).map_err(|e| CoreError::Io(e.to_string()))
}
pub fn load_app_settings(path: &Path) -> Result<AppSettings, CoreError> {
    load_settings(path)
}
pub fn save_app_settings(path: &Path, settings: &AppSettings) -> Result<(), CoreError> {
    save_settings(path, settings)
}
pub fn load_config(path: &Path) -> Result<AppSettings, CoreError> {
    load_settings(path)
}
pub fn save_config(path: &Path, settings: &AppSettings) -> Result<(), CoreError> {
    save_settings(path, settings)
}

pub struct SettingsStore {
    path: PathBuf,
}
pub type AppSettingsStore = SettingsStore;
impl SettingsStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn load(&self) -> Result<AppSettings, CoreError> {
        load_settings(&self.path)
    }
    pub fn save(&self, settings: &AppSettings) -> Result<(), CoreError> {
        save_settings(&self.path, settings)
    }
}
