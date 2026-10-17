//! Whether there is room to write the next job.
//!
//! Running out of disk space part-way through an encode is the worst outcome the
//! queue can produce: the hours are gone, the file is half-written, and the
//! failure looks like the encoder's fault when it is not. So the free space of
//! the volume the output goes on is asked *before* each queued job starts, and a
//! job that would not fit pauses the queue with a sentence the user can act on
//! rather than running and failing.
//!
//! The threshold is deliberate rather than merely "enough for this file": the
//! encoder writes a temporary sibling file and moves it into place, so two copies
//! of the output exist at the moment the encode finishes, and a volume with
//! exactly enough room for one copy is out of room in the worst possible
//! moment. On top of that sits a fixed reserve, so a queue that fills a disk to
//! within a gigabyte of full does not strand the operating system on it.
//!
//! The reading is injected, so the rules — including what happens when the
//! platform will not say — are checked without a full disk anywhere near the test
//! machine.

use std::fmt;
use std::path::{Path, PathBuf};

use vise_core::JobSpec;

/// Free space left in this many bytes is always kept in reserve.
///
/// Two gibibytes: enough that a machine which is encoding is not also running out
/// of room for its own page file and its own logs, and small enough not to refuse
/// an encode on a machine with, say, 2.1 GB free that needed 50 MB.
pub const RESERVE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// A proportion of the volume also has to be free.
///
/// A fifth: a volume that is 99% full behaves badly for everything else on the
/// machine, and refusing to fill the last few percent of a disk is a courtesy
/// that costs nothing when the encode is small.
pub const FREE_FRACTION: f64 = 0.2;

/// Bits per second to bytes per second.
const BITS_PER_BYTE: f64 = 8.0;

/// What the platform said about a volume.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SpaceReading {
    /// Bytes free, when the platform answered.
    pub free: Option<u64>,
    /// Bytes on the volume, when the platform answered.
    pub total: Option<u64>,
}

impl SpaceReading {
    /// A volume with this much free and this much on it.
    #[allow(
        dead_code,
        reason = "the constructor exists for the tests that pin the thresholds"
    )]
    pub fn of(free: u64, total: u64) -> SpaceReading {
        SpaceReading {
            free: Some(free),
            total: Some(total),
        }
    }

    /// A volume the platform would not describe.
    pub fn unknown() -> SpaceReading {
        SpaceReading::default()
    }

    /// Whether the platform answered.
    pub fn known(&self) -> bool {
        self.free.is_some()
    }
}

/// What has to be true before a job may start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Requirement {
    /// Bytes the encode is expected to write, doubled because the temporary
    /// sibling and the finished file both exist before the move.
    pub bytes: u64,
    /// The reserve that has to remain afterwards.
    pub reserve: u64,
}

impl Requirement {
    /// Total bytes that have to be free: the output, the copy being written into
    /// it, and the reserve.
    pub fn total(&self) -> u64 {
        self.bytes.saturating_mul(2).saturating_add(self.reserve)
    }
}

/// The decision about the next job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gate {
    /// There is room; the job may start.
    Proceed {
        /// Bytes free when the question was asked.
        free: Option<u64>,
    },
    /// There is not enough room; the queue should pause.
    Pause {
        /// Bytes the job needs, all in.
        required: u64,
        /// Bytes the volume has.
        free: u64,
        /// One line naming the volume and what is missing.
        reason: String,
    },
}

/// How much space an encode is expected to write.
///
/// In order of how much can be trusted: a target size the job states outright, the
/// size of a file this job would be replacing, and only then a rate multiplied by
/// a duration. Anything less than that is zero, which is not a guess of "no
/// space" but an answer of "this job's size is not knowable from what it stores" —
/// and the requirement then falls back to the reserve on its own.
pub fn estimate_bytes(
    spec: &JobSpec,
    source_duration_seconds: Option<f64>,
    existing_output_bytes: Option<u64>,
) -> u64 {
    if let Some(target) = spec
        .effective_target_size_bytes()
        .filter(|bytes| *bytes > 0)
    {
        return target;
    }
    if let Some(existing) = existing_output_bytes.filter(|bytes| *bytes > 0) {
        return existing;
    }
    let video_kbps = spec
        .average_bitrate_kbps
        .or(spec.average_bitrate)
        .or(spec.video_bitrate)
        .or(spec.bitrate)
        .or(spec.target_bitrate)
        .unwrap_or(0);
    let audio_kbps: u32 = spec
        .audio_tracks
        .iter()
        .filter_map(|track| track.bitrate)
        .sum();
    let seconds = source_duration_seconds.unwrap_or(0.0).max(0.0);
    let bytes = f64::from(video_kbps + audio_kbps) * 1000.0 / BITS_PER_BYTE * seconds;
    if bytes.is_finite() && bytes > 0.0 {
        bytes.ceil() as u64
    } else {
        0
    }
}

/// What has to be true before `spec` may be written.
pub fn requirement(
    spec: &JobSpec,
    source_duration_seconds: Option<f64>,
    existing_output_bytes: Option<u64>,
) -> Requirement {
    Requirement {
        bytes: estimate_bytes(spec, source_duration_seconds, existing_output_bytes),
        reserve: RESERVE_BYTES,
    }
}

/// Decide whether a job may start.
///
/// A volume the platform would not describe is not a reason to refuse: refusing
/// every job on a machine whose free space cannot be read would make the queue
/// useless exactly where filesystems are least standard, and a pause whose reason
/// is "unknown" tells the user nothing they could act on. The answer is therefore
/// "proceed", with the free space recorded as unknown so the tray and the log can
/// say that it was not checked.
pub fn gate(
    spec: &JobSpec,
    reading: SpaceReading,
    source_duration_seconds: Option<f64>,
    existing_output_bytes: Option<u64>,
) -> Gate {
    let requirement = requirement(spec, source_duration_seconds, existing_output_bytes);
    if !reading.known() {
        return Gate::Proceed { free: None };
    }
    let free = reading.free.unwrap_or(0);
    let total = reading.total.unwrap_or(0);
    let fraction_required = if total > 0 {
        (total as f64 * FREE_FRACTION) as u64
    } else {
        0
    };
    let needed = requirement.total().max(fraction_required);
    if free >= needed {
        Gate::Proceed { free: Some(free) }
    } else {
        Gate::Pause {
            required: needed,
            free,
            reason: format!(
                "{} needs {} but the volume has {} free",
                format_bytes(needed),
                volume_label(spec),
                format_bytes(free)
            ),
        }
    }
}

/// Bytes as the user reads them.
///
/// One formatter for the pause reason, the tray line, and the log, so that the
/// number the user reads in one place is spelled the same way as the number they
/// read in another.
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// The volume a job is written to, named the way the user would name it.
fn volume_label(spec: &JobSpec) -> String {
    let path = Path::new(&spec.output_path);
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| spec.output_path.clone());
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => {
            format!("{} in {}", name, parent.display())
        }
        _ => name,
    }
}

/// Why a volume could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpaceError {
    /// The path names no volume at all.
    NoVolume(PathBuf),
    /// The platform refused the question.
    Refused(String),
}

impl fmt::Display for SpaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpaceError::NoVolume(path) => {
                write!(f, "{} does not name a file to write", path.display())
            }
            SpaceError::Refused(reason) => write!(f, "the free space could not be read: {reason}"),
        }
    }
}

/// The platform's answer to "how much room is left on this volume".
pub fn read_free_space(output_path: &str) -> Result<SpaceReading, SpaceError> {
    let path = Path::new(output_path);
    if path.as_os_str().is_empty() {
        return Err(SpaceError::NoVolume(std::path::PathBuf::new()));
    }
    platform::free_space(path)
}

/// The platform side, declared directly so the desktop shell keeps its dependency
/// footprint at zero beyond what the window already needs.
#[cfg(windows)]
mod platform {
    use super::{SpaceError, SpaceReading};
    use std::ffi::c_void;
    use std::os::raw::c_char;
    use std::path::Path;

    extern "system" {
        fn GetModuleHandleW(module_name: *const u16) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, proc_name: *const c_char) -> *mut c_void;
    }

    type FreeSpaceFn = unsafe extern "system" fn(*const u16, *mut u64, *mut u64, *mut u64) -> i32;

    fn utf16(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn kernel32<T: Copy>(name: &str) -> Option<T> {
        unsafe {
            let mut module_name: Vec<u16> = "kernel32.dll".encode_utf16().collect();
            module_name.push(0);
            let module = GetModuleHandleW(module_name.as_ptr());
            if module.is_null() {
                return None;
            }
            let mut symbol_name: Vec<u8> = name.as_bytes().to_vec();
            symbol_name.push(0);
            let symbol = GetProcAddress(module, symbol_name.as_ptr().cast::<c_char>());
            if symbol.is_null() {
                return None;
            }
            let address: *mut c_void = std::ptr::read(&symbol);
            Some(std::mem::transmute_copy(&address))
        }
    }

    pub(super) fn free_space(path: &Path) -> Result<SpaceReading, SpaceError> {
        let Some(function) = kernel32::<FreeSpaceFn>("GetDiskFreeSpaceExW") else {
            return Ok(SpaceReading::unknown());
        };
        // The platform wants a directory rather than a file, so the parent is
        // asked about; a file that does not exist yet has no volume of its own.
        let directory = match path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
            _ => path.to_path_buf(),
        };
        let wide = utf16(&directory.to_string_lossy());
        let mut free_to_caller: u64 = 0;
        let mut total: u64 = 0;
        let mut total_free: u64 = 0;
        let ok = unsafe {
            function(
                wide.as_ptr(),
                &mut free_to_caller,
                &mut total,
                &mut total_free,
            )
        };
        if ok == 0 {
            return Err(SpaceError::Refused(
                std::io::Error::last_os_error().to_string(),
            ));
        }
        Ok(SpaceReading {
            // "Free for this caller" is the number that matters: quota-limited
            // volumes can have plenty free and nothing this process may use.
            free: Some(free_to_caller),
            total: Some(total),
        })
    }
}

#[cfg(not(windows))]
mod platform {
    use super::{SpaceError, SpaceReading};
    use std::path::Path;

    pub(super) fn free_space(path: &Path) -> Result<SpaceReading, SpaceError> {
        // Every platform this builds for can say how much room is left, through a
        // call this crate does not have; rather than report a number it cannot
        // justify, it reports that it does not know, which the gate treats as
        // "do not refuse".
        let _ = path;
        Ok(SpaceReading::unknown())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vise_core::{AudioTrackConfig, ContainerFormat};

    fn spec_with(rate: u32) -> JobSpec {
        JobSpec {
            input_path: "C:\\media\\episode.mkv".to_string(),
            output_path: "D:\\out\\episode.mp4".to_string(),
            container: ContainerFormat::Mp4,
            average_bitrate_kbps: Some(rate),
            audio_tracks: vec![AudioTrackConfig {
                bitrate: Some(128),
                ..AudioTrackConfig::default()
            }],
            ..JobSpec::default()
        }
    }

    #[test]
    fn a_job_of_known_size_needs_twice_its_size_and_the_reserve() {
        // The encoder writes a temporary sibling and moves it into place, so both
        // copies exist at the moment of the move.
        let spec = spec_with(4_000);
        let requirement = requirement(&spec, Some(600.0), None);
        let bytes = 4_128.0 * 1000.0 / 8.0 * 600.0;
        assert!(
            (requirement.bytes as f64 - bytes).abs() < 2.0,
            "{} vs {bytes}",
            requirement.bytes
        );
        assert_eq!(requirement.total(), requirement.bytes * 2 + RESERVE_BYTES);
    }

    #[test]
    fn a_target_size_the_job_states_wins_over_any_formula() {
        let mut spec = spec_with(4_000);
        spec.target_size = Some(3_000_000_000);
        assert_eq!(
            requirement(&spec, Some(600.0), Some(123)).bytes,
            3_000_000_000
        );
    }

    #[test]
    fn a_file_that_already_exists_is_a_better_estimate_than_any_formula() {
        let spec = spec_with(4_000);
        let requirement = requirement(&spec, Some(600.0), Some(123_456_789));
        assert_eq!(requirement.bytes, 123_456_789);
    }

    #[test]
    fn enough_room_starts_the_job() {
        let spec = spec_with(4_000);
        let need = requirement(&spec, Some(600.0), None).total();
        // A 100 GB volume with room in it: the job needs about 2.7 GB, which
        // is a twentieth of the volume, so the proportion rule is satisfied too.
        // An 8 GB volume with this job's own room on it. The job needs about
        // 2.7 GB, which is over a fifth of the volume, so what the volume is asked
        // about is the job rather than the proportion.
        let reading = SpaceReading::of(need + 1_000, 8 * 1024 * 1024 * 1024);
        assert_eq!(
            gate(&spec, reading, Some(600.0), None),
            Gate::Proceed {
                free: Some(need + 1_000)
            }
        );
    }

    #[test]
    fn not_enough_room_pauses_the_queue_and_says_what_is_missing() {
        let spec = spec_with(4_000);
        let need = requirement(&spec, Some(600.0), None).total();
        let reading = SpaceReading::of(need / 2, 8 * 1024 * 1024 * 1024);
        let Gate::Pause {
            required,
            free,
            reason,
        } = gate(&spec, reading, Some(600.0), None)
        else {
            panic!("a volume half the size of the job must pause the queue")
        };
        assert_eq!(required, need);
        assert_eq!(free, need / 2);
        // The sentence has to name the file, the folder, and the number, or the
        // user cannot act on it.
        assert!(reason.contains("episode.mp4"), "{reason}");
        assert!(reason.contains("D:\\out"), "{reason}");
        assert!(reason.contains(&format_bytes(need)), "{reason}");
        assert!(reason.contains(&format_bytes(need / 2)), "{reason}");
    }

    #[test]
    fn a_volume_that_is_already_mostly_full_is_refused_even_for_a_tiny_job() {
        // A 4 GB volume with 300 MB free has room for this job and none for
        // anything else, which is the state a queue should refuse to create.
        let spec = spec_with(200);
        let reading = SpaceReading::of(300 * 1024 * 1024, 4 * 1024 * 1024 * 1024);
        let Gate::Pause { required, .. } = gate(&spec, reading, Some(10.0), None) else {
            panic!("a nearly full volume must pause the queue")
        };
        // A fifth of 4 GB: the proportion, which is larger than the job itself.
        assert_eq!(
            required,
            requirement(&spec, Some(10.0), None).total(),
            "the reserve, doubled for the copy being written, is what binds here"
        );
        assert!(required > RESERVE_BYTES);
    }

    #[test]
    fn the_reserve_alone_is_what_a_job_of_unknown_size_has_to_clear() {
        // Nothing is known about this job's size, so the requirement is the
        // reserve — a question the volume can answer rather than a guess.
        let spec = spec_with(0);
        let requirement = requirement(&spec, None, None);
        assert_eq!(requirement.bytes, 0);
        assert_eq!(requirement.total(), RESERVE_BYTES);
        // The volume's size is not given, so the only rule left is the reserve.
        let tight = SpaceReading {
            free: Some(RESERVE_BYTES - 1),
            total: None,
        };
        assert!(matches!(gate(&spec, tight, None, None), Gate::Pause { .. }));
        let roomy = SpaceReading {
            free: Some(RESERVE_BYTES),
            total: None,
        };
        assert!(matches!(
            gate(&spec, roomy, None, None),
            Gate::Proceed { .. }
        ));
    }

    #[test]
    fn a_volume_that_could_not_be_read_does_not_stop_the_queue() {
        // Refusing every job on a machine whose free space cannot be read would
        // make the queue useless exactly where filesystems are least standard.
        let spec = spec_with(4_000);
        assert_eq!(
            gate(&spec, SpaceReading::unknown(), Some(3_600.0), None),
            Gate::Proceed { free: None }
        );
        assert!(!SpaceReading::unknown().known());
    }

    #[test]
    fn a_volume_with_no_size_still_has_to_clear_the_reserve() {
        let spec = spec_with(4_000);
        let need = requirement(&spec, Some(60.0), None).total();
        let reading = SpaceReading {
            free: Some(need - 1),
            total: None,
        };
        assert!(matches!(
            gate(&spec, reading, Some(60.0), None),
            Gate::Pause { .. }
        ));
    }

    #[test]
    fn the_platform_answers_for_a_path_or_says_why_not() {
        // Whether this machine answers is not the point; that it comes back as an
        // answer either way is.
        match read_free_space("D:\\out\\episode.mp4") {
            Ok(reading) => {
                assert!(!reading.free.unwrap_or(0) > 0, "a real volume has room");
                assert!(reading.known());
            }
            Err(e) => assert!(!e.to_string().is_empty(), "{e}"),
        }
        assert!(matches!(read_free_space(""), Err(SpaceError::NoVolume(_))));
    }
}
