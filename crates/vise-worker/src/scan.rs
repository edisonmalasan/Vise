//! Scan state machine backing `StartScan`, `PollScanProgress`, `GetTitles`,
//! `GetMainTitle`, and `StopScan`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use vise_core::{ChapterInfo, FfprobeScanner, ScanResult, StreamInfo, TrackKind};
use vise_ipc::contract::error_codes;
use vise_ipc::contract::{
    InitCommand, ScanAudioTrack, ScanChapter, ScanCommand, ScanProgress, ScanSubtitleTrack,
    ScanTitle, TitlesResult, WorkDone, WorkState,
};

/// Everything the scan routes need to remember between calls.
#[derive(Debug)]
pub struct ScanSlot {
    state: WorkState,
    progress: ScanProgress,
    done: WorkDone,
    titles: TitlesResult,
    /// Source of the most recent successful scan, for validation and previews.
    last_source: Option<PathBuf>,
    /// Result of the most recent successful scan, cached for validation.
    last_result: Option<ScanResult>,
    cancel: Arc<AtomicBool>,
}

impl Default for ScanSlot {
    fn default() -> Self {
        Self::new()
    }
}

impl ScanSlot {
    /// A slot that has never run.
    pub fn new() -> Self {
        ScanSlot {
            state: WorkState::Unknown,
            progress: ScanProgress::default(),
            done: WorkDone::default(),
            titles: TitlesResult::default(),
            last_source: None,
            last_result: None,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    /// The cancellation flag for the current scan.
    pub fn cancel(&self) -> Arc<AtomicBool> {
        self.cancel.clone()
    }

    /// Current state.
    pub fn state(&self) -> WorkState {
        self.state
    }

    /// Copy of the current progress.
    pub fn progress(&self) -> ScanProgress {
        self.progress.clone()
    }

    /// Copy of the terminal status, when the scan finished.
    pub fn done(&self) -> WorkDone {
        self.done.clone()
    }

    /// Copy of the discovered titles.
    pub fn titles(&self) -> TitlesResult {
        self.titles.clone()
    }

    /// Index of the main feature title.
    pub fn main_title(&self) -> u32 {
        self.titles.main_feature
    }

    /// Source of the most recent scan.
    pub fn last_source(&self) -> Option<PathBuf> {
        self.last_source.clone()
    }

    /// Cached scan result for `path`, if that is the source just scanned.
    pub fn cached_result(&self, path: &str) -> Option<&ScanResult> {
        match (&self.last_source, &self.last_result) {
            (Some(source), Some(result)) if source.as_path() == Path::new(path) => Some(result),
            _ => None,
        }
    }

    /// Whether a scan is currently running.
    pub fn is_running(&self) -> bool {
        self.state == WorkState::Working
    }

    fn begin(&mut self, command: &ScanCommand) -> Arc<AtomicBool> {
        self.state = WorkState::Working;
        self.progress = ScanProgress {
            progress: 0.0,
            completed: 0,
            total: command.paths.len(),
            current_path: String::new(),
            title_count: 0,
        };
        self.done = WorkDone {
            error: error_codes::NONE,
            was_successful: true,
            message: String::new(),
        };
        self.cancel = Arc::new(AtomicBool::new(false));
        self.cancel.clone()
    }

    fn finish(&mut self, done: WorkDone) {
        self.state = WorkState::WorkDone;
        self.progress.progress = if done.was_successful {
            1.0
        } else {
            self.progress.progress
        };
        self.done = done;
    }

    /// Expand the requested paths into concrete media files.
    pub fn expand_paths(
        &self,
        paths: &[String],
        exclusions: &[String],
        excluded_extensions: &[String],
    ) -> Result<Vec<PathBuf>, String> {
        let mut files = Vec::new();
        for raw in paths {
            let path = Path::new(raw.trim());
            if path.is_dir() {
                let mut found: Vec<PathBuf> = std::fs::read_dir(path)
                    .map_err(|e| format!("could not read directory {}: {e}", path.display()))?
                    .filter_map(|entry| entry.ok())
                    .map(|entry| entry.path())
                    .filter(|candidate| {
                        candidate.is_file()
                            && !is_excluded(candidate, exclusions, excluded_extensions)
                    })
                    .collect();
                found.sort();
                files.extend(found);
            } else if path.is_file() {
                if !is_excluded(path, exclusions, excluded_extensions) {
                    files.push(path.to_path_buf());
                }
            } else {
                return Err(format!("scan source does not exist: {}", path.display()));
            }
        }
        if files.is_empty() {
            return Err("scan request resolved to no media files".to_string());
        }
        Ok(files)
    }
}

fn is_excluded(path: &Path, exclusions: &[String], excluded_extensions: &[String]) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    // An exclusion entry may be a full file name or a bare name, which is what
    // a user types when they pick a file from a source menu.
    let stem = path
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if exclusions.iter().any(|pattern| {
        let pattern = pattern.trim().to_ascii_lowercase();
        !pattern.is_empty()
            && (pattern == name || pattern == stem || name.ends_with(&format!(".{pattern}")))
    }) {
        return true;
    }
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    excluded_extensions.iter().any(|excluded| {
        excluded
            .trim()
            .trim_start_matches('.')
            .eq_ignore_ascii_case(&extension)
    })
}

/// Shared scan state, mutated from the HTTP thread and the scan worker thread.
#[derive(Debug, Clone)]
pub struct ScanService {
    slot: Arc<Mutex<ScanSlot>>,
    logs: Arc<crate::log::LogService>,
}

impl ScanService {
    /// Create a scan service that writes progress to `logs`.
    pub fn new(logs: Arc<crate::log::LogService>) -> Self {
        ScanService {
            slot: Arc::new(Mutex::new(ScanSlot::new())),
            logs,
        }
    }

    fn slot(&self) -> std::sync::MutexGuard<'_, ScanSlot> {
        lock(&self.slot)
    }

    /// Current state.
    pub fn state(&self) -> WorkState {
        self.slot().state()
    }

    /// Copy of the current progress.
    pub fn progress(&self) -> ScanProgress {
        self.slot().progress()
    }

    /// Terminal status of the last scan.
    pub fn done(&self) -> WorkDone {
        self.slot().done()
    }

    /// Discovered titles.
    pub fn titles(&self) -> TitlesResult {
        self.slot().titles()
    }

    /// Index of the main feature title.
    pub fn main_title(&self) -> u32 {
        self.slot().main_title()
    }

    /// Source of the most recent scan.
    pub fn last_source(&self) -> Option<PathBuf> {
        self.slot().last_source()
    }

    /// Cached scan result for `path`, when that path was the last one scanned.
    pub fn cached_result(&self, path: &str) -> Option<ScanResult> {
        self.slot().cached_result(path).cloned()
    }

    /// Reject a scan that is already running.
    pub fn start(&self, command: &ScanCommand) -> Result<(), String> {
        let (files, cancel) = {
            let mut slot = self.slot();
            if slot.is_running() {
                return Err("a scan is already running".to_string());
            }
            let init = &command.initialise_command;
            let files = slot.expand_paths(
                &command.paths,
                &command.file_exclusion_list,
                &init.exclude_extension_list,
            )?;
            let cancel = slot.begin(command);
            (files, cancel)
        };

        self.logs
            .info(format!("scan started for {} source(s)", files.len()));

        let slot = self.slot.clone();
        let logs = self.logs.clone();
        let command = command.clone();
        std::thread::Builder::new()
            .name("vise-scan".into())
            .spawn(move || run_scan(&slot, &logs, &command, &files, &cancel))
            .map_err(|e| {
                let mut guard = self.slot();
                guard.finish(WorkDone {
                    error: error_codes::FAILED,
                    was_successful: false,
                    message: format!("scan worker could not start: {e}"),
                });
                format!("scan worker could not start: {e}")
            })?;

        Ok(())
    }

    /// Ask a running scan to stop.
    pub fn stop(&self) {
        let slot = self.slot();
        if slot.is_running() {
            slot.cancel().store(true, Ordering::SeqCst);
        }
    }
}

fn run_scan(
    slot: &Arc<Mutex<ScanSlot>>,
    logs: &Arc<crate::log::LogService>,
    command: &ScanCommand,
    files: &[PathBuf],
    cancel: &Arc<AtomicBool>,
) {
    let scanner = FfprobeScanner::default();
    let mut titles = Vec::new();
    let mut failure: Option<String> = None;

    for (position, file) in files.iter().enumerate() {
        if cancel.load(Ordering::SeqCst) {
            failure = Some("scan cancelled by the caller".to_string());
            break;
        }
        {
            let mut guard = lock(slot);
            guard.progress.current_path = file.to_string_lossy().to_string();
            guard.progress.completed = position;
        }
        logs.info(format!("scanning {}", file.display()));

        match scanner.scan_path(file) {
            Ok(result) => {
                let discovered = build_titles(
                    &result,
                    command,
                    command.initialise_command.keep_duplicate_titles,
                );
                logs.info(format!(
                    "found {} title(s) in {}",
                    discovered.title_list.len(),
                    file.display()
                ));
                titles.extend(discovered.title_list);
                {
                    let mut guard = lock(slot);
                    guard.last_source = Some(file.clone());
                    guard.last_result = Some(result);
                }
            }
            Err(e) => {
                let message = format!("could not scan {}: {e}", file.display());
                logs.error(&message);
                failure = Some(message);
            }
        }
        {
            let mut guard = lock(slot);
            guard.progress.completed = position + 1;
            guard.progress.progress =
                files.len() as f64 * (position + 1) as f64 / files.len() as f64;
            guard.progress.title_count = titles.len();
        }
    }

    let main_feature = titles
        .iter()
        .max_by(|a, b| {
            a.duration_seconds
                .partial_cmp(&b.duration_seconds)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|t| t.index)
        .unwrap_or(0);
    for (position, title) in titles.iter_mut().enumerate() {
        title.index = position as u32 + 1;
    }

    let mut guard = lock(slot);
    guard.titles = TitlesResult {
        main_feature,
        title_list: titles,
    };
    match failure {
        None => {
            guard.finish(WorkDone {
                error: error_codes::NONE,
                was_successful: true,
                message: String::new(),
            });
            logs.info("scan completed");
        }
        Some(message) => {
            let cancelled = cancel.load(Ordering::SeqCst);
            let (code, text) = if cancelled {
                (error_codes::CANCELLED, message)
            } else {
                (error_codes::FAILED, message)
            };
            logs.warn(&text);
            guard.finish(WorkDone {
                error: code,
                was_successful: false,
                message: text,
            });
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn default_title(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "title".to_string())
}

/// Convert a core scan result into the worker title list.
pub fn build_titles(
    result: &ScanResult,
    command: &ScanCommand,
    keep_duplicate_titles: bool,
) -> TitlesResult {
    let video = result.streams.iter().find(|s| s.kind == TrackKind::Video);
    let mut titles = Vec::new();

    for (index, title) in result.titles.iter().enumerate() {
        // Reported titles are numbered from one, and a request for zero (or a
        // negative value) asks for every title.
        let reported_index = title.index.max(index + 1) as u32;
        let duration = title
            .duration_seconds
            .or(result.format.duration_seconds)
            .unwrap_or(0.0);
        if !within_duration_bounds(duration, command) {
            continue;
        }
        if command.title_index > 0 && command.title_index as u32 != reported_index {
            continue;
        }

        let streams: Vec<&StreamInfo> = title
            .stream_indices
            .iter()
            .filter_map(|i| result.streams.get(*i))
            .collect();
        let audio: Vec<&StreamInfo> = streams
            .iter()
            .copied()
            .filter(|s| s.kind == TrackKind::Audio)
            .collect();
        let subtitles: Vec<&StreamInfo> = streams
            .iter()
            .copied()
            .filter(|s| s.kind == TrackKind::Subtitle)
            .collect();
        let chapters: Vec<ScanChapter> = title.chapters.iter().map(chapter_to_scan).collect();

        // Foreign-audio detection compares every track's language with the
        // source's primary one, so the answer is the same whichever track is
        // asked about.
        let audio_codes: Vec<&str> = audio
            .iter()
            .filter_map(|track| track.language.as_deref())
            .collect();
        let foreign = vise_core::foreign_audio_indices(&audio_codes);
        let source_kind = vise_core::optical_source_kind(&result.path)
            .map(|kind| kind.as_str().to_string())
            .unwrap_or_default();
        // The HDR facts travel with the title's own video stream, so a title
        // whose picture carries a Dolby Vision RPU or HDR10+ metadata says so.
        let title_video = streams.iter().find(|s| s.kind == TrackKind::Video);
        let dv_profile = title_video
            .and_then(|s| s.dolby_vision_profile)
            .unwrap_or(0) as u32;
        let dv_rpu = title_video.is_some_and(|s| s.dolby_vision_rpu);
        let hdr10_plus = streams.iter().any(|s| s.hdr10_plus);

        titles.push(ScanTitle {
            index: reported_index,
            name: default_title(&result.path),
            container: result
                .format
                .format_name
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            duration_seconds: duration,
            video_codec: video
                .and_then(|s| s.codec.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            width: video.and_then(|s| s.width).unwrap_or(0),
            height: video.and_then(|s| s.height).unwrap_or(0),
            frame_rate: video
                .and_then(|s| s.frame_rate.as_deref())
                .and_then(parse_frame_rate)
                .unwrap_or(0.0),
            angle_count: 1,
            crop: [0, 0, 0, 0],
            source_kind,
            dolby_vision_profile: dv_profile,
            dolby_vision_rpu: dv_rpu,
            hdr10_plus,
            audio_list: audio
                .iter()
                .enumerate()
                .map(|(n, track)| track_to_audio(track, n, &foreign))
                .collect(),
            subtitle_list: subtitles
                .iter()
                .enumerate()
                .map(|(n, s)| track_to_subtitle(s, n as u32 + 1))
                .collect(),
            chapter_list: chapters,
            meta_data: result.format.tags.clone(),
        });
    }

    if !keep_duplicate_titles {
        // Titles that describe identical streams are noise for the UI.
        let mut seen = std::collections::BTreeSet::new();
        titles.retain(|title| {
            seen.insert((
                title.duration_seconds.to_bits(),
                title.width,
                title.height,
                title.audio_list.len(),
                title.chapter_list.len(),
            ))
        });
    }

    let main_feature = titles
        .iter()
        .max_by(|a, b| {
            a.duration_seconds
                .partial_cmp(&b.duration_seconds)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|t| t.index)
        .unwrap_or(0);

    for (position, title) in titles.iter_mut().enumerate() {
        title.index = position as u32 + 1;
    }

    TitlesResult {
        main_feature,
        title_list: titles,
    }
}

fn within_duration_bounds(duration: f64, command: &ScanCommand) -> bool {
    if duration <= 0.0 {
        return true;
    }
    let ms = (duration * 1000.0) as u64;
    let min = command.min_duration.total_milliseconds();
    let max = command.max_duration.total_milliseconds();
    if min > 0 && ms < min {
        return false;
    }
    if max > 0 && ms > max {
        return false;
    }
    true
}

fn chapter_to_scan(chapter: &ChapterInfo) -> ScanChapter {
    ScanChapter {
        index: chapter.index as u32,
        start_seconds: chapter.start_seconds.unwrap_or(0.0),
        end_seconds: chapter.end_seconds.unwrap_or(0.0),
        title: chapter.title.clone().unwrap_or_default(),
    }
}

fn track_to_audio(track: &StreamInfo, position: usize, foreign: &[usize]) -> ScanAudioTrack {
    let language = track.language.clone().unwrap_or_else(|| "und".to_string());
    ScanAudioTrack {
        track_number: position as u32 + 1,
        codec_name: track.codec.clone().unwrap_or_else(|| "unknown".to_string()),
        channel_count: track.channels.unwrap_or(0),
        channel_layout: track
            .channel_layout
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        sample_rate: track.sample_rate.unwrap_or(0),
        bit_rate: track.bit_rate.unwrap_or(0),
        language: language_description(&language),
        language_code: language,
        description: String::new(),
        foreign_language: foreign.contains(&(position + 1)),
    }
}

fn track_to_subtitle(track: &StreamInfo, number: u32) -> ScanSubtitleTrack {
    let language = track.language.clone().unwrap_or_else(|| "und".to_string());
    ScanSubtitleTrack {
        track_number: number,
        codec_name: track.codec.clone().unwrap_or_else(|| "unknown".to_string()),
        language: language_description(&language),
        language_code: language,
        description: track.title.clone().unwrap_or_default(),
    }
}

fn language_description(code: &str) -> String {
    if code == "und" {
        "Undetermined".to_string()
    } else {
        code.to_string()
    }
}

fn parse_frame_rate(text: &str) -> Option<f64> {
    let (num, den) = text.split_once('/')?;
    let num: f64 = num.trim().parse().ok()?;
    let den: f64 = den.trim().parse().ok()?;
    if den == 0.0 {
        return None;
    }
    Some(num / den)
}

/// Metadata map helper used by the router for logging scan summaries.
pub fn tags_to_map(result: &ScanResult) -> BTreeMap<String, String> {
    result.format.tags.clone()
}

/// Apply an init command to shared services.
pub fn apply_init(logs: &crate::log::LogService, init: &InitCommand) {
    logs.set_verbosity(init.log_verbosity);
    logs.set_disk_target(init.log_file_path());
}

#[cfg(test)]
mod tests {
    use super::*;
    use vise_ipc::timespan::TimeSpan;

    fn command() -> ScanCommand {
        ScanCommand::default()
    }

    #[test]
    fn expands_directories_and_filters_names() {
        let dir = std::env::temp_dir().join("vise-worker-scan-expand");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("a.mkv"), b"x").expect("a");
        std::fs::write(dir.join("b.txt"), b"x").expect("b");
        std::fs::write(dir.join("skip.mkv"), b"x").expect("skip");

        let slot = ScanSlot::new();
        let files = slot
            .expand_paths(
                &[dir.to_string_lossy().to_string()],
                &["skip".to_string()],
                &["txt".to_string()],
            )
            .expect("expand");
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("a.mkv"));
    }

    #[test]
    fn reports_unusable_scan_requests() {
        let slot = ScanSlot::new();
        assert!(slot
            .expand_paths(&["no-such-path".to_string()], &[], &[])
            .unwrap_err()
            .contains("does not exist"));
        assert!(slot
            .expand_paths(&[], &[], &[])
            .unwrap_err()
            .contains("no media"));
    }

    #[test]
    fn filters_titles_by_duration_bounds() {
        let mut cmd = command();
        cmd.min_duration = TimeSpan::parse("00:00:05").unwrap();
        cmd.max_duration = TimeSpan::parse("00:00:10").unwrap();
        assert!(within_duration_bounds(7.0, &cmd), "inside the window");
        assert!(
            !within_duration_bounds(1.0, &cmd),
            "shorter than the minimum"
        );
        assert!(
            !within_duration_bounds(30.0, &cmd),
            "longer than the maximum"
        );
        // A title with an unknown duration is kept rather than discarded.
        assert!(within_duration_bounds(0.0, &cmd));
    }

    #[test]
    fn parses_fractional_frame_rates() {
        assert_eq!(parse_frame_rate("30000/1001"), Some(29.970_029_970_029_97));
        assert_eq!(parse_frame_rate("25/1"), Some(25.0));
        assert_eq!(parse_frame_rate("bad"), None);
        assert_eq!(parse_frame_rate("30/0"), None);
    }

    #[test]
    fn builds_titles_from_a_scan_result() {
        let result = vise_ipc::validation::synthetic_scan(12.0, 640, 360, Some(1));
        let titles = build_titles(&result, &command(), false);
        assert_eq!(titles.title_list.len(), 1);
        let title = &titles.title_list[0];
        assert_eq!(title.index, 1);
        assert_eq!(title.width, 640);
        assert_eq!(title.height, 360);
        assert_eq!(titles.main_feature, 1);
        assert_eq!(title.audio_list.len(), 1);
        assert_eq!(title.audio_list[0].channel_count, 1);
    }

    #[test]
    fn honours_the_title_index_filter() {
        let result = vise_ipc::validation::synthetic_scan(12.0, 640, 360, Some(2));
        let mut cmd = command();

        // Zero asks for every title.
        assert_eq!(build_titles(&result, &cmd, false).title_list.len(), 1);

        // One selects the single reported title.
        cmd.title_index = 1;
        assert_eq!(build_titles(&result, &cmd, false).title_list.len(), 1);

        // Any other index selects nothing.
        cmd.title_index = 9;
        assert!(build_titles(&result, &cmd, false).title_list.is_empty());
    }

    #[test]
    fn init_command_updates_log_settings() {
        let logs = crate::log::LogService::default();
        let init = InitCommand {
            log_verbosity: 0,
            ..InitCommand::default()
        };
        apply_init(&logs, &init);
        logs.debug("dropped");
        logs.error("kept");
        assert_eq!(logs.all().len(), 1);
    }
}
