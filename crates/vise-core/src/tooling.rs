//! Where the media tools come from.
//!
//! A shipped build carries its own ffmpeg and ffprobe, because a user who
//! installs the application should not also have to install a build of the
//! encoder, and because the version matters: the encoders and filters this
//! application names are only present in some builds. So the bundled copy is
//! asked for first, and the machine's `PATH` is only consulted when the
//! application has none of its own.
//!
//! The order is deliberately fixed rather than configurable: a user who needs a
//! particular build can put it on the `PATH` of a portable install, but nobody
//! should be able to end up with an encoder that quietly lacks a named filter.
//! The discovered paths are reported, so a build that fell back to the machine
//! can say so rather than silently behaving differently from a shipped one.

use std::path::{Path, PathBuf};

/// The platform's executable suffix.
#[cfg(windows)]
const EXE_SUFFIX: &str = ".exe";
#[cfg(not(windows))]
const EXE_SUFFIX: &str = "";

/// Where a media tool was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolSource {
    /// Shipped alongside the application.
    Bundled(PathBuf),
    /// Found on the machine's search path.
    Path(PathBuf),
    /// Found by a platform-specific fallback (a per-user package directory).
    PlatformFallback(PathBuf),
    /// Not found anywhere.
    Missing,
}

impl ToolSource {
    /// The resolved path, when there is one.
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Bundled(path) | Self::Path(path) | Self::PlatformFallback(path) => Some(path),
            Self::Missing => None,
        }
    }

    /// Whether this is the application's own copy.
    pub fn is_bundled(&self) -> bool {
        matches!(self, Self::Bundled(_))
    }

    /// One line saying where the tool came from.
    pub fn describe(&self, name: &str) -> String {
        match self {
            Self::Bundled(path) => format!("{name} is bundled at {}", path.display()),
            Self::Path(path) => format!(
                "{name} is not bundled with this build, so the one on the search path is used: {}",
                path.display()
            ),
            Self::PlatformFallback(path) => format!(
                "{name} is not bundled with this build, so one installed per user is used: {}",
                path.display()
            ),
            Self::Missing => {
                format!("{name} could not be found: this build carries none and none is installed")
            }
        }
    }
}

/// The directories a bundled build looks in, relative to the application root.
///
/// The layouts differ because the platforms package differently: a Windows
/// installer unpacks beside the executable, a macOS bundle puts tools under
/// `Resources`, and a Linux package puts them beside the binary or under a
/// `libexec`-style directory. All are tried, so the same code serves all three.
pub const BUNDLED_SUBDIRECTORIES: &[&str] = &[
    "",
    "media",
    "bin",
    "Resources",
    "Resources/media",
    "libexec",
];

/// Names a bundled tool may be published under, in the order they are tried.
///
/// Some packagers append a version suffix to keep two builds apart on the same
/// machine, so a plain name is not the only one worth looking for.
pub fn bundled_names(name: &str) -> Vec<String> {
    let mut names = vec![format!("{name}{EXE_SUFFIX}")];
    // On a platform with no suffix the two candidates would be the same string,
    // so the bare name is only offered when it differs.
    let bare = name.to_string();
    if bare != names[0] {
        names.push(bare);
    }
    names
}

/// Find a media tool, preferring the application's own copy.
///
/// `root` is the application directory; the search covers every layout in
/// [`BUNDLED_SUBDIRECTORIES`] beneath it, and only then falls back to the
/// machine. A root that does not exist is not an error: a development build
/// simply has nothing bundled, and the machine's copy is the right answer there.
pub fn resolve(name: &str, root: Option<&Path>) -> ToolSource {
    if let Some(root) = root {
        if let Some(found) = find_bundled(name, root) {
            return ToolSource::Bundled(found);
        }
    }
    find_on_machine(name)
}

/// Find a media tool inside an application directory.
pub fn find_bundled(name: &str, root: &Path) -> Option<PathBuf> {
    for sub in BUNDLED_SUBDIRECTORIES {
        let dir = if sub.is_empty() {
            root.to_path_buf()
        } else {
            root.join(sub)
        };
        for candidate in bundled_names(name) {
            let path = dir.join(&candidate);
            if is_executable(&path) {
                return Some(path);
            }
        }
    }
    None
}

/// Whether a path is a file this build can run.
///
/// Existence is not enough: a directory called `ffmpeg.exe` or a file with no
/// read permission would be found by a naive check and then fail at the first
/// spawn with a far less useful message.
pub fn is_executable(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Find a media tool on the machine, after the bundled copy was not found.
pub fn find_on_machine(name: &str) -> ToolSource {
    for dir in machine_search_path() {
        for candidate in bundled_names(name) {
            let path = dir.join(&candidate);
            if is_executable(&path) {
                return ToolSource::Path(path);
            }
        }
    }
    platform_fallback(name)
        .map(ToolSource::PlatformFallback)
        .unwrap_or(ToolSource::Missing)
}

/// The directories searched on the machine, read from the environment.
fn machine_search_path() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(path_env) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_env) {
            if !dir.as_os_str().is_empty() && !dirs.contains(&dir) {
                dirs.push(dir);
            }
        }
    }
    dirs
}

/// Per-user install locations that are not on the search path by default.
#[cfg(windows)]
fn platform_fallback(name: &str) -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from)?;
    let base = local.join("Microsoft").join("WinGet").join("Packages");
    for entry in std::fs::read_dir(base).ok()?.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        for candidate in bundled_names(name) {
            let tool = path.join(candidate);
            if is_executable(&tool) {
                return Some(tool);
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn platform_fallback(name: &str) -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let tool = home.join("bin").join(format!("{name}{EXE_SUFFIX}"));
    is_executable(&tool).then_some(tool)
}

#[cfg(not(any(windows, target_os = "macos")))]
fn platform_fallback(_name: &str) -> Option<PathBuf> {
    None
}

/// What a build is using for its media tools.
///
/// Both tools are reported together because a mismatch is the interesting case:
/// an ffmpeg from this build beside an ffprobe from another is how a scan and an
/// encode end up disagreeing about a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaTooling {
    /// How ffmpeg was found.
    pub ffmpeg: ToolSource,
    /// How ffprobe was found.
    pub ffprobe: ToolSource,
    /// Whether the application's own copy is being used for both.
    pub fully_bundled: bool,
    /// The search root that was consulted, when one was.
    pub root: Option<PathBuf>,
}

impl MediaTooling {
    /// Resolve both tools for an application directory.
    pub fn resolve(root: Option<&Path>) -> Self {
        let ffmpeg = resolve("ffmpeg", root);
        let ffprobe = resolve("ffprobe", root);
        Self {
            fully_bundled: ffmpeg.is_bundled() && ffprobe.is_bundled(),
            ffmpeg,
            ffprobe,
            root: root.map(Path::to_path_buf),
        }
    }

    /// One line naming the pair, for a diagnostics report or the activity log.
    pub fn describe(&self) -> String {
        let mut lines = vec![
            self.ffmpeg.describe("ffmpeg"),
            self.ffprobe.describe("ffprobe"),
        ];
        if !self.fully_bundled {
            lines.push(
                "This is not a fully bundled build, so results may differ from a shipped one."
                    .to_string(),
            );
        }
        lines.join(" ")
    }

    /// Whether both tools were found.
    pub fn is_complete(&self) -> bool {
        self.ffmpeg.path().is_some() && self.ffprobe.path().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vise-tooling-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    #[cfg(unix)]
    fn make_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(path, b"#!/bin/sh\nexit 0\n").expect("write");
        let mut perms = std::fs::metadata(path).expect("meta").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms).expect("chmod");
    }

    #[cfg(not(unix))]
    fn make_executable(path: &Path) {
        std::fs::write(path, b"MZ").expect("write");
    }

    #[test]
    fn a_tool_beside_the_executable_wins_over_the_search_path() {
        let root = scratch("beside");
        let bundled = root.join(format!("ffmpeg{EXE_SUFFIX}"));
        make_executable(&bundled);
        let found = resolve("ffmpeg", Some(&root));
        assert_eq!(found, ToolSource::Bundled(bundled));
        assert!(found.is_bundled());
        assert!(found.describe("ffmpeg").contains("bundled"));
    }

    #[test]
    fn every_packaged_layout_is_searched() {
        for sub in ["media", "bin", "Resources", "Resources/media", "libexec"] {
            let root = scratch(&format!("layout-{}", sub.replace('/', "-")));
            let dir = root.join(sub);
            std::fs::create_dir_all(&dir).expect("dir");
            let bundled = dir.join(format!("ffmpeg{EXE_SUFFIX}"));
            make_executable(&bundled);
            let found = resolve("ffmpeg", Some(&root));
            assert_eq!(
                found,
                ToolSource::Bundled(bundled),
                "the {sub} layout was not searched"
            );
        }
    }

    #[test]
    fn the_bundled_copy_is_found_without_an_executable_suffix_too() {
        let root = scratch("no-suffix");
        let bundled = root.join("ffmpeg");
        make_executable(&bundled);
        assert_eq!(resolve("ffmpeg", Some(&root)), ToolSource::Bundled(bundled));
    }

    #[test]
    fn a_directory_named_like_a_tool_is_not_taken_for_one() {
        let root = scratch("decoy-dir");
        std::fs::create_dir_all(root.join(format!("ffmpeg{EXE_SUFFIX}"))).expect("dir");
        // Nothing bundled, so the machine is consulted instead, and a directory
        // that happens to share the name must not be mistaken for a binary.
        let found = resolve("ffmpeg", Some(&root));
        assert_ne!(
            found,
            ToolSource::Bundled(root.join(format!("ffmpeg{EXE_SUFFIX}"))),
            "a directory was accepted as the tool"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_bundled_file_that_cannot_be_run_is_not_taken_for_one() {
        let root = scratch("not-executable");
        let bundled = root.join(format!("ffmpeg{EXE_SUFFIX}"));
        std::fs::write(&bundled, b"not a program").expect("write");
        // No execute bit, so it must not be treated as a runnable tool.
        assert!(
            !is_executable(&bundled),
            "a file with no execute bit is not runnable"
        );
    }

    #[test]
    fn a_build_with_nothing_bundled_says_so_rather_than_pretending() {
        let tooling = MediaTooling::resolve(Some(Path::new("/nonexistent-application-root")));
        assert!(
            !tooling.fully_bundled,
            "nothing was bundled, so this is not a fully bundled build"
        );
        let text = tooling.describe();
        assert!(
            text.contains("not a fully bundled build"),
            "the difference has to be visible: {text}"
        );
    }

    #[test]
    fn both_tools_are_reported_together() {
        let root = scratch("pair");
        let ffmpeg = root.join(format!("ffmpeg{EXE_SUFFIX}"));
        make_executable(&ffmpeg);
        // ffprobe is deliberately absent, which is the case worth naming: a
        // half-bundled build is not the same as a fully bundled one.
        let tooling = MediaTooling::resolve(Some(&root));
        assert!(tooling.ffmpeg.is_bundled());
        assert!(!tooling.ffprobe.is_bundled());
        assert!(!tooling.fully_bundled);
        assert!(tooling.describe().contains("not a fully bundled build"));
    }

    #[test]
    fn a_missing_tool_says_it_is_missing() {
        let source = ToolSource::Missing;
        assert_eq!(source.path(), None);
        assert!(source.describe("ffmpeg").contains("could not be found"));
    }
}
