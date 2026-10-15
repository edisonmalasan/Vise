//! Preset and configuration persistence for Vise Transcoder.
//!
//! This module provides built-in and user presets, application of presets to
//! jobs, storage of user presets in a JSON file, import/export helpers, and
//! application settings persistence. It has no UI dependency.

use crate::{AudioCodec, ContainerFormat, CoreError, JobSpec, SubtitleMode, VideoCodec};
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

pub fn builtin_presets() -> Vec<BuiltInPreset> {
    vec![
        BuiltInPreset {
            id: "fast-1080p".to_string(),
            name: "Fast 1080p".to_string(),
            description: Some("Fast H.264 encode at 1080p".to_string()),
            preset: PresetSpec {
                container: Some(ContainerFormat::Mp4),
                format: Some(ContainerFormat::Mp4),
                video_codec: Some(VideoCodec::H264),
                video: Some(VideoCodec::H264),
                audio_codec: Some(AudioCodec::Aac),
                audio: Some(AudioCodec::Aac),
                quality: Some(22),
                ..Default::default()
            },
        },
        BuiltInPreset {
            id: "high-quality".to_string(),
            name: "High Quality".to_string(),
            description: Some("Higher quality H.265 encode".to_string()),
            preset: PresetSpec {
                container: Some(ContainerFormat::Mkv),
                format: Some(ContainerFormat::Mkv),
                video_codec: Some(VideoCodec::H265),
                video: Some(VideoCodec::H265),
                audio_codec: Some(AudioCodec::Flac),
                audio: Some(AudioCodec::Flac),
                quality: Some(18),
                ..Default::default()
            },
        },
        BuiltInPreset {
            id: "web-optimized".to_string(),
            name: "Web Optimized".to_string(),
            description: Some("WebM/VP9 for web delivery".to_string()),
            preset: PresetSpec {
                container: Some(ContainerFormat::Webm),
                format: Some(ContainerFormat::Webm),
                video_codec: Some(VideoCodec::Vp9),
                video: Some(VideoCodec::Vp9),
                audio_codec: Some(AudioCodec::Opus),
                audio: Some(AudioCodec::Opus),
                quality: Some(28),
                ..Default::default()
            },
        },
    ]
}

pub fn builtin_preset(id: &str) -> Option<BuiltInPreset> {
    builtin_presets().into_iter().find(|p| p.id == id)
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
    if let Some(v) = preset.container.or(preset.format) {
        updated.container = v;
    }
    if let Some(v) = preset.video_codec.or(preset.video) {
        updated.video_codec = v;
    }
    if let Some(v) = preset.audio_codec.or(preset.audio) {
        updated.audio_codec = v;
    }
    if let Some(v) = preset.subtitle_mode.or(preset.subtitles) {
        updated.subtitle_mode = v;
    }
    if let Some(v) = preset.title_index.or(preset.title) {
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
    updated
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
}

pub fn import_preset(path: &Path) -> Result<PresetSpec, CoreError> {
    let raw = fs::read_to_string(path).map_err(|e| CoreError::Io(e.to_string()))?;
    serde_json::from_str(&raw).map_err(|e| CoreError::Parse(e.to_string()))
}
pub fn import_preset_file(path: &Path) -> Result<PresetSpec, CoreError> {
    import_preset(path)
}
pub fn import_user_preset(path: &Path) -> Result<UserPreset, CoreError> {
    let raw = fs::read_to_string(path).map_err(|e| CoreError::Io(e.to_string()))?;
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
pub fn export_user_preset(path: &Path, preset: &UserPreset) -> Result<(), CoreError> {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let raw = serde_json::to_string_pretty(preset).map_err(|e| CoreError::Parse(e.to_string()))?;
    fs::write(path, raw).map_err(|e| CoreError::Io(e.to_string()))
}

// Settings persistence
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AppSettings {
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
}

pub type Settings = AppSettings;
pub type AppConfig = AppSettings;
pub type Config = AppSettings;
pub type ViseSettings = AppSettings;

pub fn load_settings(path: &Path) -> Result<AppSettings, CoreError> {
    if !path.exists() {
        return Ok(AppSettings::default());
    }
    let raw = fs::read_to_string(path).map_err(|e| CoreError::Io(e.to_string()))?;
    if raw.trim().is_empty() {
        return Ok(AppSettings::default());
    }
    serde_json::from_str(&raw).map_err(|e| CoreError::Parse(e.to_string()))
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
