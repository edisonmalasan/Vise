//! Where an encode writes while it is running, and what is left behind.
//!
//! The encoder writes to a partial file beside the destination and the finished
//! file is only moved into place once the run succeeds, so a cancelled or failed
//! encode never leaves a half-written destination behind. The naming lives here
//! rather than in the worker because the *window* is the side that has to find
//! those files: when the worker dies mid-encode it cannot tidy up after itself,
//! and a partial file nobody recognises is a file the user finds in their output
//! folder with no explanation.
//!
//! One naming rule, one place: the worker creates partials from this module and
//! the window sweeps them from the same module, so the sweep cannot drift away
//! from what the writer writes.

use std::path::{Path, PathBuf};

/// The marker that separates a partial file from a finished one.
const MARKER: &str = ".vise-part-";

/// Temporary output path used while an encode runs.
pub fn partial_output_path(output: &Path) -> PathBuf {
    partial_output_path_for(output, std::process::id())
}

/// Temporary output path used while an encode runs in the process `pid`.
pub fn partial_output_path_for(output: &Path, pid: u32) -> PathBuf {
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    let stem = output
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let extension = output.extension().and_then(|e| e.to_str()).unwrap_or("mkv");
    parent.join(format!("{stem}{MARKER}{pid}.{extension}"))
}

/// The partial files that were left beside `output`, whatever process wrote them.
///
/// The pattern is the destination's own stem and extension with any process id
/// in the middle, so this answers for one destination only: a sweep can never
/// reach into another job's output.
pub fn partial_outputs_for(output: &Path) -> Vec<PathBuf> {
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    let stem = output
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let extension = output.extension().and_then(|e| e.to_str());
    let prefix = format!("{stem}{MARKER}");
    let Ok(entries) = std::fs::read_dir(parent) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .filter_map(|entry| entry.path().into_string().ok().map(PathBuf::from))
        .filter(|path| {
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                return false;
            };
            let Some(rest) = name.strip_prefix(prefix.as_str()) else {
                return false;
            };
            match extension {
                Some(extension) => rest.ends_with(&format!(".{extension}")),
                None => !rest.contains('.'),
            }
        })
        .collect();
    found.sort();
    found
}

/// What a sweep managed to remove, and what it could not.
///
/// A partial that could not be deleted is named rather than forgotten: the user
/// has to remove it themselves, and the only honest thing to tell them is which
/// file it is.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PartialCleanup {
    /// Files that are gone.
    pub removed: Vec<PathBuf>,
    /// Files that are still there, with why they stayed.
    pub failed: Vec<(PathBuf, String)>,
}

impl PartialCleanup {
    /// Whether nothing is left behind.
    pub fn is_clean(&self) -> bool {
        self.failed.is_empty()
    }

    /// One line describing what the sweep did.
    pub fn summary(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if !self.removed.is_empty() {
            parts.push(format!(
                "{} partial file(s) removed: {}",
                self.removed.len(),
                self.removed
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !self.failed.is_empty() {
            parts.push(format!(
                "{} partial file(s) could not be removed and are still there: {}",
                self.failed.len(),
                self.failed
                    .iter()
                    .map(|(path, _)| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        parts.join("; ")
    }
}

/// Remove the partial files beside `output`.
///
/// Only for a caller whose worker is gone: while a worker is alive the partial
/// beside a destination is a file somebody is writing.
pub fn remove_partial_outputs(output: &Path) -> PartialCleanup {
    let mut cleanup = PartialCleanup::default();
    for path in partial_outputs_for(output) {
        match std::fs::remove_file(&path) {
            Ok(()) => cleanup.removed.push(path),
            Err(e) => cleanup.failed.push((path, e.to_string())),
        }
    }
    cleanup
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vise-ipc-partial-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn a_partial_sits_next_to_the_destination_and_names_its_writer() {
        let partial = partial_output_path_for(Path::new("/tmp/out/movie.mkv"), 4321);
        assert_eq!(partial.parent().unwrap(), Path::new("/tmp/out"));
        let name = partial.file_name().unwrap().to_string_lossy().to_string();
        assert_eq!(name, "movie.vise-part-4321.mkv");
    }

    #[test]
    fn the_sweep_finds_what_the_writer_wrote_and_leaves_everything_else() {
        let dir = temp_dir("sweep");
        let destination = dir.join("movie.mkv");
        std::fs::write(dir.join("movie.vise-part-111.mkv"), b"x").expect("write");
        std::fs::write(dir.join("movie.vise-part-222.mkv"), b"x").expect("write");
        // Not partials of this destination: another job's file, a finished file,
        // and a partial belonging to a different container.
        std::fs::write(dir.join("other.mkv"), b"x").expect("write");
        std::fs::write(dir.join("other.vise-part-111.mkv"), b"x").expect("write");
        std::fs::write(dir.join("movie.mp4"), b"x").expect("write");
        std::fs::write(dir.join("movie.vise-part-111.mp4"), b"x").expect("write");

        let found = partial_outputs_for(&destination);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found.iter().all(|p| p
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("movie.vise-part-")));

        let cleanup = remove_partial_outputs(&destination);
        assert_eq!(cleanup.removed.len(), 2);
        assert!(cleanup.is_clean(), "{:?}", cleanup.failed);
        assert!(cleanup.summary().contains("2 partial file(s) removed"));
        assert!(destination.with_file_name("other.mkv").exists());
        assert!(dir.join("other.vise-part-111.mkv").exists());
        assert!(dir.join("movie.mp4").exists());
        assert!(dir.join("movie.vise-part-111.mp4").exists());
        assert!(!dir.join("movie.vise-part-111.mkv").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_destination_with_no_partials_sweeps_clean() {
        let dir = temp_dir("empty");
        let destination = dir.join("movie.mkv");
        std::fs::write(&destination, b"finished").expect("write");
        let cleanup = remove_partial_outputs(&destination);
        assert!(cleanup.removed.is_empty());
        assert!(cleanup.is_clean());
        assert_eq!(cleanup.summary(), "");
        assert!(destination.exists(), "the finished file is not touched");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_folder_that_cannot_be_listed_leaves_nothing_behind() {
        let cleanup = remove_partial_outputs(Path::new("/no-such-folder/movie.mkv"));
        assert!(cleanup.removed.is_empty());
        assert!(cleanup.is_clean());
    }
}
