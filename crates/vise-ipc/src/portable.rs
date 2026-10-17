//! Portable-mode settings, including the process-isolation switch and the
//! places state is written to.
//!
//! A portable install keeps all of its state next to the executable. It can
//! also disable process isolation, which is the one case where the desktop
//! application must not spawn a worker at all: some constrained environments
//! (locked-down media servers, packaging sandboxes) forbid the extra process
//! or the loopback listener it needs.
//!
//! # Where state is written
//!
//! The same file decides where the settings, the queue, and the presets live:
//! an installed build writes them under the per-user configuration directory,
//! and a portable build writes them beside the executable. The resolution is
//! [`resolve_storage`], and it is deliberately a pure function of the entries
//! and the two roots rather than something that touches the filesystem, so the
//! rules — including what a `..` in the file may and may not do — can be stated
//! and tested without a portable drive attached.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

/// Key controlling whether the desktop application spawns a worker process.
pub const PROCESS_ISOLATION_KEY: &str = "worker.process_isolation.enabled";

/// Environment variable that overrides portable-mode file discovery.
pub const PORTABLE_INI_ENV: &str = "VISE_PORTABLE_INI";

/// Conventional file name for portable-mode settings.
pub const PORTABLE_INI_FILE: &str = "portable.ini";

/// Why portable settings could not be read.
#[derive(Debug)]
pub enum PortableError {
    /// The file could not be read.
    Io { path: PathBuf, message: String },
    /// The file contained a line that is neither a comment nor `key=value`.
    Malformed {
        path: PathBuf,
        line_number: usize,
        line: String,
    },
}

impl fmt::Display for PortableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PortableError::Io { path, message } => write!(
                f,
                "portable settings could not be read from {}: {message}",
                path.display()
            ),
            PortableError::Malformed {
                path,
                line_number,
                line,
            } => write!(
                f,
                "portable settings at {} line {line_number} are malformed: {line}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for PortableError {}

/// Whether a textual boolean is meaningful.
pub fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" | "enabled" => Some(true),
        "0" | "false" | "no" | "off" | "disabled" => Some(false),
        _ => None,
    }
}

/// Parse `key=value` settings, ignoring comments and blank lines.
///
/// Keys are lowercased and separators (`-` and `.`) are treated alike so
/// `worker.process_isolation.enabled` and `worker-process-isolation-enabled`
/// name the same setting.
pub fn parse_ini(text: &str) -> Result<BTreeMap<String, String>, PortableError> {
    let mut map = BTreeMap::new();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty()
            || line.starts_with('#')
            || line.starts_with(';')
            || line.starts_with('[')
        {
            continue;
        }
        match line.split_once('=') {
            Some((key, value)) => {
                let key = normalize_key(key);
                if key.is_empty() {
                    return Err(PortableError::Malformed {
                        path: PathBuf::from(PORTABLE_INI_FILE),
                        line_number: index + 1,
                        line: line.to_string(),
                    });
                }
                map.insert(key, value.trim().to_string());
            }
            None => {
                return Err(PortableError::Malformed {
                    path: PathBuf::from(PORTABLE_INI_FILE),
                    line_number: index + 1,
                    line: line.to_string(),
                })
            }
        }
    }
    Ok(map)
}

fn normalize_key(key: &str) -> String {
    key.trim().to_ascii_lowercase().replace(['-', ' '], ".")
}

/// Effective portable-mode settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortableSettings {
    /// Whether the desktop application may spawn a worker process.
    pub process_isolation_enabled: bool,
    /// Whether a `portable.ini` was found at all.
    pub portable_mode_active: bool,
    /// Directory used for portable state, when one was declared.
    pub storage_directory: Option<String>,
    /// Every key that was present, for diagnostics.
    pub entries: BTreeMap<String, String>,
}

impl Default for PortableSettings {
    fn default() -> Self {
        PortableSettings {
            process_isolation_enabled: true,
            portable_mode_active: false,
            storage_directory: None,
            entries: BTreeMap::new(),
        }
    }
}

impl PortableSettings {
    /// Whether a worker process should be started.
    pub fn should_isolate(&self) -> bool {
        self.process_isolation_enabled
    }

    /// Message shown when isolation has been switched off.
    pub fn isolation_disabled_reason(&self) -> String {
        "process isolation is disabled in the portable settings file, so the worker process was not started"
            .to_string()
    }
}

/// Settings used when no portable-mode file exists.
pub fn default_settings() -> PortableSettings {
    PortableSettings::default()
}

/// Locate the portable-mode file for an executable directory.
///
/// The environment override wins, then the conventional file name next to the
/// executable.
pub fn locate(executable_dir: &Path) -> Option<PathBuf> {
    if let Some(value) = std::env::var_os(PORTABLE_INI_ENV) {
        let path = PathBuf::from(value);
        return Some(path);
    }
    let candidate = executable_dir.join(PORTABLE_INI_FILE);
    if candidate.is_file() {
        Some(candidate)
    } else {
        None
    }
}

/// Read portable settings from `path`.
pub fn read_settings(path: &Path) -> Result<PortableSettings, PortableError> {
    let text = std::fs::read_to_string(path).map_err(|e| PortableError::Io {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    let entries = parse_ini(&text).map_err(|e| match e {
        PortableError::Malformed {
            line_number, line, ..
        } => PortableError::Malformed {
            path: path.to_path_buf(),
            line_number,
            line,
        },
        other => other,
    })?;
    Ok(settings_from_entries(entries, true))
}

/// Build settings from already-parsed entries.
pub fn settings_from_entries(
    entries: BTreeMap<String, String>,
    portable_mode_active: bool,
) -> PortableSettings {
    let process_isolation_enabled = entries
        .get(&normalize_key(PROCESS_ISOLATION_KEY))
        .and_then(|v| parse_bool(v))
        // An unparsable value must not silently enable a process that the
        // operator tried to turn off, so anything unrecognised disables it.
        .or_else(|| {
            entries
                .contains_key(&normalize_key(PROCESS_ISOLATION_KEY))
                .then_some(false)
        })
        .unwrap_or(true);

    let storage_directory = entries
        .get("storage.directory")
        .filter(|v| !v.trim().is_empty())
        .cloned();

    PortableSettings {
        process_isolation_enabled,
        portable_mode_active,
        storage_directory,
        entries,
    }
}

/// Conventional file name of the settings document.
pub const SETTINGS_FILE: &str = "settings.json";
/// Conventional file name of the queue snapshot.
pub const QUEUE_FILE: &str = "queue.json";
/// Conventional file name of the preset library.
pub const PRESETS_FILE: &str = "presets.json";

/// What one kind of state is stored as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageKind {
    /// The settings document: preferences, theme, direction, recent sources.
    Settings,
    /// The queue snapshot written when the window closes and after every change.
    Queue,
    /// The user's own presets.
    Presets,
}

impl StorageKind {
    /// The key a directory for this kind of state is named by in the file.
    ///
    /// Kept next to the enum so a kind cannot be added without deciding how an
    /// operator points it somewhere else.
    pub const fn directory_key(self) -> &'static str {
        match self {
            StorageKind::Settings => "storage.settings.directory",
            StorageKind::Queue => "storage.queue.directory",
            StorageKind::Presets => "storage.presets.directory",
        }
    }

    /// The file name this kind of state is stored under.
    pub const fn file_name(self) -> &'static str {
        match self {
            StorageKind::Settings => SETTINGS_FILE,
            StorageKind::Queue => QUEUE_FILE,
            StorageKind::Presets => PRESETS_FILE,
        }
    }

    /// Every kind, so the rules can be checked for all of them at once.
    pub const ALL: [StorageKind; 3] = [
        StorageKind::Settings,
        StorageKind::Queue,
        StorageKind::Presets,
    ];
}

/// One place state is written to, and why it was chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageLocation {
    /// The directory this kind of state is written to.
    pub directory: PathBuf,
    /// The file inside it.
    pub file: PathBuf,
    /// The key in the portable file that pointed here, when one did.
    pub from_key: Option<String>,
    /// What was refused while resolving, said as one line.
    ///
    /// A file that names a directory this build will not follow does not move
    /// state there, and does not lose the rest of the arrangement either: the next
    /// key is tried and this one is reported, so the operator finds out from the
    /// window rather than from state that vanished somewhere else.
    pub refusal: Option<String>,
}

/// Every place state is written to, resolved once at start-up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageRoots {
    /// Whether a portable settings file was found at all.
    pub portable: bool,
    /// The directory everything defaults into.
    pub root: PathBuf,
    /// Where the settings document lives.
    pub settings: StorageLocation,
    /// Where the queue snapshot lives.
    pub queue: StorageLocation,
    /// Where the preset library lives.
    pub presets: StorageLocation,
    /// Anything that was refused, in the order it was found.
    pub refusals: Vec<String>,
}

impl Default for StorageRoots {
    /// Every kind of state in the working directory, with nothing portable about
    /// it.
    ///
    /// The default is only what a caller that never resolved anything gets; the
    /// window resolves through [`resolve_storage`] before it reads or writes.
    fn default() -> StorageRoots {
        let base = PathBuf::new();
        let place = |kind: StorageKind| StorageLocation {
            directory: base.clone(),
            file: base.join(kind.file_name()),
            from_key: None,
            refusal: None,
        };
        StorageRoots {
            portable: false,
            root: base.clone(),
            settings: place(StorageKind::Settings),
            queue: place(StorageKind::Queue),
            presets: place(StorageKind::Presets),
            refusals: Vec::new(),
        }
    }
}

impl StorageRoots {
    /// The location for `kind`.
    pub fn of(&self, kind: StorageKind) -> &StorageLocation {
        match kind {
            StorageKind::Settings => &self.settings,
            StorageKind::Queue => &self.queue,
            StorageKind::Presets => &self.presets,
        }
    }

    /// One line naming where every kind of state is written.
    pub fn summary(&self) -> String {
        StorageKind::ALL
            .iter()
            .map(|kind| {
                let location = self.of(*kind);
                format!("{} at {}", kind.file_name(), location.file.display())
            })
            .collect::<Vec<String>>()
            .join("; ")
    }
}

/// Resolve where settings, the queue, and the presets are written.
///
/// `installed_root` is the per-user configuration directory an installed build
/// uses, and `portable_root` is the directory the portable file was found in.
/// Which one wins is decided by whether a portable file exists at all: an
/// operator who put the file there asked for the state to travel with it, and a
/// build that quietly kept writing to the profile would defeat the whole
/// arrangement.
///
/// Three keys can move a kind of state, most specific first:
/// `storage.settings.directory`, `storage.queue.directory`, and
/// `storage.presets.directory`, with `storage.directory` as the fallback for
/// all three. A relative value is read against the portable root; an absolute
/// one is taken as written, because pointing the state at a network share is a
/// deliberate act; and a relative value that climbs out of the portable root
/// with `..` is refused, because a marker file in a folder should not be able to
/// reach anywhere else on the machine by accident.
pub fn resolve_storage(
    settings: &PortableSettings,
    portable_root: &Path,
    installed_root: &Path,
) -> StorageRoots {
    let portable = settings.portable_mode_active;
    let base = if portable {
        portable_root
    } else {
        installed_root
    };
    let mut refusals = Vec::new();
    let mut resolve = |kind: StorageKind| -> StorageLocation {
        // Most specific key first, and the general one as the fallback, so an
        // operator who names one kind gets that kind moved without having to name
        // it again for the other two.
        let mut refused: Option<String> = None;
        for key in [kind.directory_key(), GENERAL_DIRECTORY_KEY] {
            let Some(value) = settings
                .entries
                .get(&normalize_key(key))
                .filter(|value| !value.trim().is_empty())
            else {
                continue;
            };
            let key = key.to_string();
            let value = value.trim().to_string();
            let candidate = PathBuf::from(&value);
            // A directory is only honoured when the portable file is what declared
            // it: without that file there is no operator instruction behind these
            // entries, and a build that could be talked into moving its own profile
            // by a value it invented would be a build nobody could reason about. An
            // absolute value is then taken as written, because pointing the state
            // at a network share is a deliberate act; a relative one is read
            // against the portable root, and one that climbs out of it is refused,
            // because a marker file in a folder should not be able to reach
            // anywhere else on the machine by accident.
            let refusal = if !portable {
                Some(format!(
                    "{key} is only honoured when a portable settings file declares it, so '{value}' was not used"
                ))
            } else if !candidate.is_absolute() && escapes_root(&candidate) {
                Some(format!(
                    "'{value}' in {key} points outside the folder the portable settings file is in, so it was not used"
                ))
            } else {
                None
            };
            if let Some(reason) = refusal {
                refusals.push(reason.clone());
                refused.get_or_insert(reason);
                // A refused value falls through to the next key rather than to the
                // root, so one bad line does not undo the rest of the arrangement.
                continue;
            }
            let directory = if candidate.is_absolute() {
                candidate
            } else {
                portable_root.join(candidate)
            };
            return StorageLocation {
                file: directory.join(kind.file_name()),
                directory,
                from_key: Some(key),
                refusal: refused,
            };
        }
        StorageLocation {
            directory: base.to_path_buf(),
            file: base.join(kind.file_name()),
            from_key: None,
            refusal: refused,
        }
    };
    let settings_location = resolve(StorageKind::Settings);
    let queue = resolve(StorageKind::Queue);
    let presets = resolve(StorageKind::Presets);
    StorageRoots {
        portable,
        root: base.to_path_buf(),
        settings: settings_location,
        queue,
        presets,
        refusals,
    }
}

/// The key that moves every kind of state at once.
pub const GENERAL_DIRECTORY_KEY: &str = "storage.directory";

/// Whether a relative path climbs above the root it is read against.
///
/// Judged on the components rather than by canonicalising, because the point is
/// to refuse a `..` whether or not the target happens to exist yet, and whether
/// or not a link on this machine would take it somewhere surprising.
fn escapes_root(candidate: &Path) -> bool {
    let mut depth: i64 = 0;
    for component in candidate.components() {
        match component {
            std::path::Component::ParentDir => {
                depth -= 1;
                if depth < 0 {
                    return true;
                }
            }
            std::path::Component::Normal(_) => depth += 1,
            _ => {}
        }
    }
    false
}

/// Read the portable settings that apply to a directory, if any.
///
/// Returns the defaults (isolation enabled) when no portable-mode file exists.
pub fn load_for(executable_dir: &Path) -> Result<PortableSettings, PortableError> {
    match locate(executable_dir) {
        Some(path) => read_settings(&path),
        None => Ok(PortableSettings {
            portable_mode_active: false,
            ..PortableSettings::default()
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vise-ipc-portable-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn parses_bool_spellings() {
        for value in ["1", "true", "TRUE", "yes", "on", "enabled"] {
            assert_eq!(parse_bool(value), Some(true), "{value}");
        }
        for value in ["0", "false", "No", "off", "disabled"] {
            assert_eq!(parse_bool(value), Some(false), "{value}");
        }
        assert_eq!(parse_bool("maybe"), None);
    }

    #[test]
    fn parses_ini_and_ignores_comments() {
        let map = parse_ini(
            "# a comment\n; another\n\nstorage.directory = /data\nworker.process_isolation.enabled=false\n",
        )
        .expect("parse");
        assert_eq!(map.get("storage.directory").unwrap(), "/data");
        assert_eq!(
            map.get(&normalize_key(PROCESS_ISOLATION_KEY)).unwrap(),
            "false"
        );
    }

    #[test]
    fn rejects_malformed_lines() {
        let err = parse_ini("this is not a pair").unwrap_err();
        assert!(err.to_string().contains("line 1"));
    }

    #[test]
    fn isolation_defaults_to_enabled() {
        let settings = default_settings();
        assert!(settings.should_isolate());
        assert!(!settings.portable_mode_active);
        assert!(settings.isolation_disabled_reason().contains("portable"));
    }

    #[test]
    fn portable_file_can_disable_isolation() {
        let dir = temp_dir("disable");
        let path = dir.join(PORTABLE_INI_FILE);
        std::fs::write(
            &path,
            format!("{PROCESS_ISOLATION_KEY} = false\nstorage.directory = data\n"),
        )
        .expect("write");

        let settings = read_settings(&path).expect("read");
        assert!(!settings.should_isolate());
        assert!(settings.portable_mode_active);
        assert_eq!(settings.storage_directory.as_deref(), Some("data"));
        assert!(settings
            .entries
            .contains_key(&normalize_key(PROCESS_ISOLATION_KEY)));
    }

    #[test]
    fn portable_file_can_keep_isolation_on() {
        let dir = temp_dir("enable");
        let path = dir.join(PORTABLE_INI_FILE);
        std::fs::write(&path, format!("{PROCESS_ISOLATION_KEY}=true\n")).expect("write");
        let settings = read_settings(&path).expect("read");
        assert!(settings.should_isolate());
    }

    #[test]
    fn unparsable_value_disables_isolation_rather_than_guessing() {
        let entries = parse_ini(&format!("{PROCESS_ISOLATION_KEY}=perhaps\n")).expect("parse");
        let settings = settings_from_entries(entries, true);
        assert!(!settings.should_isolate());
    }

    #[test]
    fn discovers_the_file_next_to_the_executable() {
        let dir = temp_dir("discover");
        assert!(locate(&dir).is_none());
        std::fs::write(dir.join(PORTABLE_INI_FILE), "storage.directory = data\n").expect("write");
        assert!(locate(&dir).is_some());

        let settings = load_for(&dir).expect("load");
        assert!(settings.portable_mode_active);
        assert!(
            settings.should_isolate(),
            "isolation is on unless switched off"
        );
    }

    #[test]
    fn reports_unreadable_files() {
        let dir = temp_dir("missing");
        let err = read_settings(&dir.join("absent.ini")).unwrap_err();
        assert!(err.to_string().contains("could not be read"));
    }

    fn settings_from(text: &str, portable: bool) -> PortableSettings {
        settings_from_entries(parse_ini(text).expect("parse"), portable)
    }

    #[test]
    fn an_installed_build_writes_its_state_in_the_configuration_directory() {
        let roots = resolve_storage(
            &PortableSettings::default(),
            Path::new("/opt/vise"),
            Path::new("/home/user/.config/vise"),
        );
        assert!(!roots.portable);
        assert_eq!(roots.root, PathBuf::from("/home/user/.config/vise"));
        for kind in StorageKind::ALL {
            let location = roots.of(kind);
            assert_eq!(
                location.file,
                Path::new("/home/user/.config/vise").join(kind.file_name())
            );
            assert!(location.from_key.is_none());
            assert!(location.refusal.is_none());
        }
        assert!(roots.refusals.is_empty());
        assert!(roots.summary().contains("queue.json"));
    }

    #[test]
    fn a_portable_build_writes_its_state_beside_the_executable() {
        let roots = resolve_storage(
            &settings_from("", true),
            Path::new("/media/stick/vise"),
            Path::new("/home/user/.config/vise"),
        );
        assert!(roots.portable);
        assert_eq!(roots.root, PathBuf::from("/media/stick/vise"));
        assert_eq!(
            roots.of(StorageKind::Queue).file,
            PathBuf::from("/media/stick/vise/queue.json")
        );
        assert_eq!(
            roots.of(StorageKind::Presets).file,
            PathBuf::from("/media/stick/vise/presets.json")
        );
    }

    #[test]
    fn one_directory_key_moves_every_kind_of_state() {
        let roots = resolve_storage(
            &settings_from("storage.directory = data\n", true),
            Path::new("/media/stick/vise"),
            Path::new("/home/user/.config/vise"),
        );
        for kind in StorageKind::ALL {
            let location = roots.of(kind);
            assert_eq!(location.directory, PathBuf::from("/media/stick/vise/data"));
            assert_eq!(location.from_key.as_deref(), Some("storage.directory"));
        }
    }

    #[test]
    fn a_kind_can_be_pointed_somewhere_else_on_its_own() {
        // The most specific key wins, and the other two kinds keep the general
        // one: an operator who wants the queue on a fast drive is asking for that
        // one thing, not for the presets to follow.
        let roots = resolve_storage(
            &settings_from(
                "storage.directory = data\nstorage.queue.directory = scratch\n",
                true,
            ),
            Path::new("/media/stick/vise"),
            Path::new("/home/user/.config/vise"),
        );
        assert_eq!(
            roots.queue.directory,
            PathBuf::from("/media/stick/vise/scratch"),
            "the queue follows its own key"
        );
        assert_eq!(
            roots.queue.from_key.as_deref(),
            Some("storage.queue.directory")
        );
        assert_eq!(
            roots.settings.directory,
            PathBuf::from("/media/stick/vise/data")
        );
        assert_eq!(
            roots.presets.directory,
            PathBuf::from("/media/stick/vise/data")
        );
        assert_eq!(
            roots.queue.file,
            PathBuf::from("/media/stick/vise/scratch/queue.json")
        );
    }

    #[test]
    fn a_relative_directory_that_climbs_out_is_refused_and_says_which_line() {
        let roots = resolve_storage(
            &settings_from(
                "storage.queue.directory = ../../windows/system32\nstorage.directory = data\n",
                true,
            ),
            Path::new("/media/stick/vise"),
            Path::new("/home/user/.config/vise"),
        );
        // The refused key falls back to the general one rather than to nothing,
        // because the operator still asked for their state to travel with the
        // folder.
        assert_eq!(
            roots.queue.directory,
            PathBuf::from("/media/stick/vise/data")
        );
        assert!(
            roots.queue.refusal.is_some(),
            "the queue is told it was not obeyed"
        );
        assert_eq!(roots.refusals.len(), 1);
        assert!(roots.refusals[0].contains("storage.queue.directory"));
        assert!(roots.settings.refusal.is_none());
    }

    #[test]
    fn a_relative_directory_that_stays_inside_is_allowed_however_deep() {
        let roots = resolve_storage(
            &settings_from("storage.directory = ./state/../state/vise\n", true),
            Path::new("/media/stick/vise"),
            Path::new("/home/user/.config/vise"),
        );
        assert_eq!(
            roots.root.join("state/vise"),
            PathBuf::from("/media/stick/vise/state/vise")
        );
        assert!(roots.settings.refusal.is_none());
        assert!(roots.refusals.is_empty());
    }

    #[test]
    fn an_absolute_directory_is_taken_as_written() {
        // A network share is a deliberate choice, so it is honoured — but only in
        // a portable install, where the file naming it is the operator's own.
        let roots = resolve_storage(
            &settings_from("storage.presets.directory = \\\\nas\\vise\\presets\n", true),
            Path::new("/media/stick/vise"),
            Path::new("/home/user/.config/vise"),
        );
        assert_eq!(
            roots.presets.directory,
            PathBuf::from("\\\\nas\\vise\\presets"),
            "an absolute value is used as written"
        );
        assert!(roots.presets.refusal.is_none());
    }

    #[test]
    fn an_installed_build_ignores_a_directory_it_cannot_honoured() {
        // Without a portable file there is no operator instruction to follow, so
        // an absolute value must not be able to move the profile somewhere else.
        let mut settings = PortableSettings::default();
        settings.entries.insert(
            "storage.queue.directory".to_string(),
            "/etc/vise".to_string(),
        );
        let roots = resolve_storage(&settings, Path::new("/opt/vise"), Path::new("/cfg/vise"));
        assert_eq!(roots.queue.directory, Path::new("/cfg/vise"));
        assert!(roots.queue.refusal.is_some(), "and it says so");
        assert_eq!(roots.refusals.len(), 1);
        assert!(roots.refusals[0].contains("only honoured"));
    }
}
