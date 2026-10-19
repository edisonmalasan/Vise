//! Source picking: input validation, drag-and-drop acceptance, and the recent
//! source list.
//!
//! The window never guesses what a user meant. A path arriving from the file
//! picker, a drag-and-drop payload, or a recent-source entry all pass through
//! [`resolve_source`] first, so the same rules apply to every entry point and
//! the user gets one specific reason when a source is refused.
//!
//! Recent sources are application settings, not a separate store: the list is
//! written into the settings file through the core settings API that presets
//! already use, which keeps one authoritative configuration file.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use vise_core::{load_settings, save_settings};

/// How many entries the recent source list keeps.
pub const RECENT_SOURCE_LIMIT: usize = 10;

/// Longest path the picker accepts, matching the usual platform limit.
pub const MAX_PATH_LENGTH: usize = 4096;

/// Separator the current platform uses.
#[cfg(windows)]
const MAIN_SEPARATOR: char = '\\';

/// Separator the current platform uses.
#[cfg(not(windows))]
const MAIN_SEPARATOR: char = '/';

/// Separator the other platforms use, accepted as input and normalised away.
#[cfg(windows)]
const ALTERNATE_SEPARATOR: &str = "/";

/// Separator the other platforms use, accepted as input and normalised away.
#[cfg(not(windows))]
const ALTERNATE_SEPARATOR: &str = "\\";

/// Settings member that holds the recent source list.
pub const RECENT_SOURCES_KEY: &str = "recent_sources";

/// What kind of thing a source entry points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    /// A single media file.
    #[default]
    File,
    /// A directory whose media files are scanned.
    Folder,
}

impl SourceKind {
    /// Human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            SourceKind::File => "File",
            SourceKind::Folder => "Folder",
        }
    }

    /// Classify an existing path.
    pub fn of(path: &Path) -> Self {
        if path.is_dir() {
            SourceKind::Folder
        } else {
            SourceKind::File
        }
    }
}

/// Why a source path was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceError {
    /// Nothing was typed, or only whitespace.
    Empty,
    /// The path is longer than any platform accepts.
    TooLong,
    /// The path contains a byte that cannot appear in a file name.
    InvalidCharacter,
    /// The path refers to the current or parent directory rather than a source.
    RelativeMarker,
    /// Nothing exists at the path.
    NotFound,
    /// The path exists but is neither a file nor a directory.
    NotASource,
}

impl SourceError {
    /// One line the interface can show next to the picker.
    pub fn message(self) -> &'static str {
        match self {
            SourceError::Empty => "Choose a file or folder to scan.",
            SourceError::TooLong => "That path is too long to scan.",
            SourceError::InvalidCharacter => {
                "That path contains characters a file name cannot hold."
            }
            SourceError::RelativeMarker => "Enter the full path of the source.",
            SourceError::NotFound => "That source no longer exists.",
            SourceError::NotASource => "That path is neither a file nor a folder.",
        }
    }
}

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

/// A source the window can scan, ready for the interface layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceViewModel {
    /// Absolute path handed to the worker.
    pub path: String,
    /// File or folder name.
    pub name: String,
    /// Containing directory, empty for a filesystem root.
    pub location: String,
    /// What kind of source this is.
    pub kind: SourceKind,
    /// Name and kind, as one label.
    pub label: String,
}

/// A dropped or pasted path that was refused, with the reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RejectedSource {
    /// The text that arrived from the interface layer.
    pub raw: String,
    /// Why it was refused.
    pub reason: String,
}

/// Result of offering several paths to the picker at once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceDropReport {
    /// Sources accepted, in the order they were offered.
    pub accepted: Vec<SourceViewModel>,
    /// Paths refused, with a reason each.
    pub rejected: Vec<RejectedSource>,
}

/// A source the user opened recently.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentSource {
    /// Absolute path of the source.
    pub path: String,
    /// Kind recorded when the source was opened.
    pub kind: SourceKind,
    /// Monotonic stamp; larger means more recent.
    #[serde(default)]
    pub stamp: u64,
}

/// Recent source list, newest first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RecentSources {
    entries: Vec<RecentSource>,
}

impl RecentSources {
    /// An empty list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of entries held.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Record a source as the most recently opened one.
    ///
    /// Re-opening a source moves it to the front instead of duplicating it,
    /// and the list never grows past [`RECENT_SOURCE_LIMIT`].
    pub fn record(&mut self, source: &SourceViewModel, stamp: u64) {
        self.entries
            .retain(|entry| !same_path(&entry.path, &source.path));
        self.entries.insert(
            0,
            RecentSource {
                path: source.path.clone(),
                kind: source.kind,
                stamp,
            },
        );
        self.entries.truncate(RECENT_SOURCE_LIMIT);
    }

    /// Drop one entry, reporting whether it was there.
    pub fn forget(&mut self, path: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|entry| !same_path(&entry.path, path));
        self.entries.len() != before
    }

    /// Remove entries whose source no longer exists, reporting how many went.
    pub fn prune_missing(&mut self) -> usize {
        let before = self.entries.len();
        self.entries.retain(|entry| Path::new(&entry.path).exists());
        before - self.entries.len()
    }

    /// Forget everything.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Entries as view models, dropping any whose source has disappeared.
    pub fn view_models(&mut self) -> Vec<SourceViewModel> {
        self.prune_missing();
        self.entries
            .iter()
            .map(|entry| SourceViewModel {
                path: entry.path.clone(),
                name: file_name_of(&entry.path),
                location: parent_of(&entry.path),
                kind: entry.kind,
                label: format!("{} · {}", file_name_of(&entry.path), entry.kind.label()),
            })
            .collect()
    }

    /// Read the list out of the settings file, treating an unreadable or
    /// malformed entry as an empty list rather than an application failure.
    pub fn load(settings_path: &Path) -> Self {
        let Ok(settings) = load_settings(settings_path) else {
            return Self::new();
        };
        settings
            .extra
            .get(RECENT_SOURCES_KEY)
            .cloned()
            .and_then(|value| serde_json::from_value::<Vec<RecentSource>>(value).ok())
            .map(|entries| Self { entries })
            .unwrap_or_default()
    }

    /// Write the list back into the settings file.
    pub fn save(&self, settings_path: &Path) -> Result<(), String> {
        let mut settings = load_settings(settings_path).unwrap_or_default();
        let value = serde_json::to_value(&self.entries)
            .map_err(|e| format!("recent sources could not be encoded: {e}"))?;
        settings.extra.insert(RECENT_SOURCES_KEY.to_string(), value);
        save_settings(settings_path, &settings)
            .map_err(|e| format!("settings could not be saved: {e}"))
    }
}

/// Compare two paths the way the current platform does.
fn same_path(left: &str, right: &str) -> bool {
    if cfg!(windows) {
        left.trim().eq_ignore_ascii_case(right.trim())
    } else {
        left.trim() == right.trim()
    }
}

fn file_name_of(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_string()
}

fn parent_of(path: &str) -> String {
    Path::new(path)
        .parent()
        .and_then(|parent| parent.to_str())
        .unwrap_or("")
        .to_string()
}

/// Normalise the text a user typed or dropped.
///
/// This is pure text handling: it trims, rejects unusable input, and tidies
/// separators. Whether the path exists is a separate question answered by
/// [`resolve_source`], so both halves can be tested on their own.
pub fn normalize_path(raw: &str) -> Result<String, SourceError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(SourceError::Empty);
    }
    if trimmed.chars().count() > MAX_PATH_LENGTH {
        return Err(SourceError::TooLong);
    }
    if trimmed.contains('\0') {
        return Err(SourceError::InvalidCharacter);
    }
    let body = trimmed
        .replace(ALTERNATE_SEPARATOR, &MAIN_SEPARATOR.to_string())
        .trim_end_matches(MAIN_SEPARATOR)
        .to_string();
    let body = if body.is_empty() {
        // The input was a bare separator, which is a filesystem root.
        MAIN_SEPARATOR.to_string()
    } else {
        body
    };
    if matches!(body.as_str(), "." | "..") {
        return Err(SourceError::RelativeMarker);
    }
    Ok(body)
}

/// Validate and classify a source path.
///
/// This is the single gate every source passes through, whether it came from
/// the file picker, a drag-and-drop payload, or the recent list.
pub fn resolve_source(raw: &str) -> Result<SourceViewModel, SourceError> {
    let normalized = normalize_path(raw)?;
    let path = PathBuf::from(&normalized);
    if !path.exists() {
        return Err(SourceError::NotFound);
    }
    if !path.is_file() && !path.is_dir() {
        return Err(SourceError::NotASource);
    }
    let kind = SourceKind::of(&path);
    let name = file_name_of(&normalized);
    Ok(SourceViewModel {
        label: format!("{name} · {}", kind.label()),
        name,
        location: parent_of(&normalized),
        kind,
        path: normalized,
    })
}

/// Offer several paths to the picker at once, keeping every reason for a
/// refusal so the interface can explain a partial drop instead of failing
/// silently.
pub fn inspect_paths(raw_paths: &[String]) -> SourceDropReport {
    let mut accepted: Vec<SourceViewModel> = Vec::new();
    let mut rejected: Vec<RejectedSource> = Vec::new();
    for raw in raw_paths {
        match resolve_source(raw) {
            Ok(source) => {
                if !accepted
                    .iter()
                    .any(|existing| same_path(&existing.path, &source.path))
                {
                    accepted.push(source);
                }
            }
            Err(e) => rejected.push(RejectedSource {
                raw: raw.trim().to_string(),
                reason: e.message().to_string(),
            }),
        }
    }
    SourceDropReport { accepted, rejected }
}

/// Settings file location for the desktop application.
///
/// Settings follow the platform convention rather than a single hard-coded
/// folder, and the join itself is separated out so it can be checked without
/// touching the environment.
pub fn settings_path(config_root: &Path) -> PathBuf {
    config_root.join("Vise Transcoder").join("settings.json")
}

/// Platform configuration root, if the environment names one.
pub fn config_root() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(PathBuf::from).or_else(|| {
            std::env::var_os("USERPROFILE")
                .map(|p| PathBuf::from(p).join("AppData").join("Roaming"))
        })
    }
    #[cfg(not(windows))]
    {
        if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME") {
            let path = PathBuf::from(dir);
            if path.is_absolute() {
                return Some(path);
            }
        }
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config"))
    }
}

/// Settings file for this machine, falling back to the working directory when
/// the platform names no configuration root.
pub fn default_settings_path() -> PathBuf {
    settings_path(&config_root().unwrap_or_else(|| PathBuf::from(".")))
}

/// Monotonic stamp for recent source ordering.
pub fn next_stamp(previous: u64) -> u64 {
    previous.saturating_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use vise_core::{load_settings, save_settings, AppSettings};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vise-desktop-sources-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn rejects_input_that_cannot_name_a_source() {
        assert_eq!(normalize_path("   "), Err(SourceError::Empty));
        assert_eq!(normalize_path("\u{0}"), Err(SourceError::InvalidCharacter));
        assert_eq!(normalize_path("."), Err(SourceError::RelativeMarker));
        assert_eq!(normalize_path(".."), Err(SourceError::RelativeMarker));
        assert_eq!(
            normalize_path(&"x".repeat(MAX_PATH_LENGTH + 1)),
            Err(SourceError::TooLong)
        );
    }

    // This test is about Windows path shapes: a drive letter and a backslash
    // name one file on Windows and one odd filename on Linux, so the
    // behaviour it checks only has one answer on one platform.
    #[cfg(windows)]
    #[test]
    fn normalises_separators_and_trailing_slashes() {
        let native = MAIN_SEPARATOR;
        assert_eq!(
            normalize_path(&format!("  C{native}media{native}movie.mkv  ")).unwrap(),
            format!("C{native}media{native}movie.mkv")
        );
        assert_eq!(
            normalize_path(&format!("C{native}media{native}clips{native}")).unwrap(),
            format!("C{native}media{native}clips")
        );
        assert_eq!(
            normalize_path(native.to_string().as_str()).unwrap(),
            native.to_string()
        );
        // The foreign separator is normalised to the native one.
        let mixed = normalize_path(&format!("C{native}media/other")).unwrap();
        assert!(!mixed.contains('/'), "unexpected separator in {mixed:?}");
    }

    #[test]
    fn classifies_files_and_folders() {
        let dir = temp_dir("classify");
        let file = dir.join("movie.mkv");
        fs::write(&file, b"x").expect("file");

        let file_view = resolve_source(file.to_str().unwrap()).expect("file source");
        assert_eq!(file_view.kind, SourceKind::File);
        assert_eq!(file_view.name, "movie.mkv");
        assert_eq!(file_view.label, "movie.mkv · File");
        let folder_name = dir.file_name().expect("name").to_string_lossy().to_string();
        assert!(file_view.location.ends_with(&folder_name));

        let folder_view = resolve_source(dir.to_str().unwrap()).expect("folder source");
        assert_eq!(folder_view.kind, SourceKind::Folder);
        assert_eq!(folder_view.label, format!("{folder_name} · Folder"));
    }

    #[test]
    fn reports_a_missing_source_distinctly_from_an_empty_one() {
        let dir = temp_dir("missing");
        let missing = dir.join("gone.mkv");
        let error = resolve_source(missing.to_str().unwrap()).unwrap_err();
        assert_eq!(error, SourceError::NotFound);
        assert!(error.message().contains("no longer exists"));
    }

    #[test]
    fn drop_report_keeps_accepted_and_refused_paths() {
        let dir = temp_dir("drop");
        let file = dir.join("movie.mkv");
        fs::write(&file, b"x").expect("file");
        let paths = vec![
            "".to_string(),
            file.to_string_lossy().to_string(),
            file.to_string_lossy().to_string(),
            dir.join("nope.mkv").to_string_lossy().to_string(),
        ];
        let report = inspect_paths(&paths);
        assert_eq!(report.accepted.len(), 1, "the duplicate is collapsed");
        assert_eq!(report.rejected.len(), 2);
        assert!(!report.accepted.is_empty());
        assert!(report.rejected[0]
            .reason
            .contains("Choose a file or folder"));
    }

    #[test]
    fn recent_sources_move_to_the_front_and_stay_bounded() {
        let dir = temp_dir("recents");
        let mut sources = RecentSources::new();
        for index in 0..RECENT_SOURCE_LIMIT + 4 {
            let file = dir.join(format!("movie-{index}.mkv"));
            fs::write(&file, b"x").expect("file");
            let view = resolve_source(file.to_str().unwrap()).expect("source");
            sources.record(&view, next_stamp(index as u64));
        }
        assert_eq!(sources.len(), RECENT_SOURCE_LIMIT);
        assert!(sources
            .view_models()
            .first()
            .expect("first")
            .path
            .ends_with(&format!("movie-{}.mkv", RECENT_SOURCE_LIMIT + 3)));

        // Re-opening the oldest entry promotes it instead of duplicating.
        let oldest = sources.view_models().last().expect("oldest").clone();
        let view = resolve_source(&oldest.path).expect("source");
        sources.record(&view, 100);
        assert_eq!(sources.len(), RECENT_SOURCE_LIMIT);
        let models = sources.view_models();
        assert_eq!(models[0].path, oldest.path);
        assert_eq!(
            models
                .iter()
                .filter(|model| same_path(&model.path, &oldest.path))
                .count(),
            1
        );
    }

    #[test]
    fn recent_sources_survive_a_settings_round_trip() {
        let dir = temp_dir("persist");
        let settings = settings_path(&dir);
        let file = dir.join("movie.mkv");
        fs::write(&file, b"x").expect("file");
        let view = resolve_source(file.to_str().unwrap()).expect("source");

        let mut sources = RecentSources::new();
        sources.record(&view, 7);
        sources.save(&settings).expect("save");

        let loaded = RecentSources::load(&settings);
        assert_eq!(loaded, sources);
        assert_eq!(loaded.len(), 1);

        let mut settings_doc: AppSettings = load_settings(&settings).expect("settings read back");
        assert!(settings_doc.extra.contains_key(RECENT_SOURCES_KEY));
        settings_doc.default_preset = Some("high-quality".to_string());
        save_settings(&settings, &settings_doc).expect("settings saved");

        let again = RecentSources::load(&settings);
        assert_eq!(again.len(), 1, "the list survived another writer");
    }

    #[test]
    fn a_missing_settings_file_reads_as_an_empty_list() {
        let dir = temp_dir("missing-settings");
        let settings = settings_path(&dir);
        assert_eq!(RecentSources::load(&settings).len(), 0);
        // A settings file that is not valid JSON is ignored rather than fatal.
        fs::create_dir_all(settings.parent().unwrap()).expect("dir");
        fs::write(&settings, "{ not json").expect("write");
        assert_eq!(RecentSources::load(&settings).len(), 0);
    }

    #[test]
    fn prunes_entries_whose_source_is_gone() {
        let dir = temp_dir("prune");
        let file = dir.join("movie.mkv");
        fs::write(&file, b"x").expect("file");
        let mut sources = RecentSources::new();
        sources.record(&resolve_source(file.to_str().unwrap()).expect("source"), 1);
        fs::remove_file(&file).expect("remove");
        assert_eq!(sources.prune_missing(), 1);
        assert!(sources.view_models().is_empty());
        assert_eq!(sources.len(), 0);
    }

    #[test]
    fn forget_and_clear_remove_entries() {
        let dir = temp_dir("forget");
        let file = dir.join("movie.mkv");
        fs::write(&file, b"x").expect("file");
        let view = resolve_source(file.to_str().unwrap()).expect("source");
        let mut sources = RecentSources::new();
        sources.record(&view, 1);
        assert!(sources.forget(&view.path));
        assert!(!sources.forget(&view.path));
        sources.record(&view, 2);
        assert_eq!(sources.view_models().len(), 1);
        sources.clear();
        assert_eq!(sources.len(), 0);
    }
}
