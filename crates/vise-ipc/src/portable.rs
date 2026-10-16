//! Portable-mode settings, including the process-isolation switch.
//!
//! A portable install keeps all of its state next to the executable. It can
//! also disable process isolation, which is the one case where the desktop
//! application must not spawn a worker at all: some constrained environments
//! (locked-down media servers, packaging sandboxes) forbid the extra process
//! or the loopback listener it needs.

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
}
