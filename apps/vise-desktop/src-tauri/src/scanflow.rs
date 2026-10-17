//! The source area state machine.
//!
//! One place holds everything the source surface shows: which source is
//! loaded, where the scan stands, the titles the scan found, and the choice
//! inside the chosen title. The transitions are pure — the worker reports are
//! fed in — so the interesting behaviour (progress while scanning, the default
//! title, resetting when a range or track cannot be used) is testable without a
//! worker process.

use vise_core::TrackKind;
use vise_ipc::contract::{ProgressReport, ScanTitle, TitlesResult, WorkState};

use crate::sources::SourceViewModel;
use crate::viewmodel::{
    default_audio_selection, phase_of, progress_view, range_view, title_views, track_selection,
    RangeMode, RangeSelection, RangeViewModel, ScanPhase, ScanProgressViewModel,
    SelectionViewModel, TrackSelectionInput,
};

/// Everything the source surface shows.
#[derive(Debug, Clone, Default)]
pub struct ScanFlow {
    source: Option<SourceViewModel>,
    phase: ScanPhase,
    progress: ScanProgressViewModel,
    titles: TitlesResult,
    selected_title: u32,
    range: RangeSelection,
    audio: Vec<u32>,
    subtitle: Vec<u32>,
    problem: String,
    /// The range a newly chosen title starts in, from the General preferences.
    default_range_mode: RangeMode,
}

impl ScanFlow {
    /// The loaded source, when one is.
    pub fn source(&self) -> Option<&SourceViewModel> {
        self.source.as_ref()
    }

    /// Where the scan stands.
    pub fn phase(&self) -> ScanPhase {
        self.phase
    }

    /// Whether a scan is in flight.
    pub fn is_running(&self) -> bool {
        self.phase == ScanPhase::Scanning
    }

    /// The chosen title, when the scan produced it.
    pub fn current_title(&self) -> Option<&ScanTitle> {
        self.titles
            .title_list
            .iter()
            .find(|title| title.index == self.selected_title)
    }

    /// The chosen tracks as they stand.
    pub fn tracks(&self) -> TrackSelectionInput {
        TrackSelectionInput {
            audio: self.audio.clone(),
            subtitle: self.subtitle.clone(),
        }
    }

    /// Load a scan result directly, used by the in-process fallback.
    pub fn accept_scan(&mut self, titles: TitlesResult) {
        self.begin();
        self.apply_titles(titles);
    }

    /// Accept a source and reset everything the previous scan produced.
    pub fn set_source(&mut self, source: Option<SourceViewModel>) {
        self.source = source;
        self.reset_scan();
    }

    /// The range a newly chosen title starts in.
    ///
    /// Held here rather than read from the preference each time a title is
    /// chosen, because the preference is the answer and this is the state it
    /// puts the window into: one place the window looks, one place the
    /// preference is read.
    pub fn set_default_range_mode(&mut self, mode: RangeMode) {
        self.default_range_mode = mode;
    }

    /// Forget the scan results but keep the loaded source.
    pub fn reset_scan(&mut self) {
        self.phase = ScanPhase::Idle;
        self.progress = ScanProgressViewModel::default();
        self.titles = TitlesResult::default();
        self.selected_title = 0;
        self.range = RangeSelection::default();
        self.audio.clear();
        self.subtitle.clear();
        self.problem.clear();
    }

    /// A scan is starting: clear the old results so nothing stale is shown
    /// while the worker probes the new source.
    pub fn begin(&mut self) {
        self.progress = ScanProgressViewModel::default();
        self.titles = TitlesResult::default();
        self.selected_title = 0;
        self.range = RangeSelection::default();
        self.audio.clear();
        self.subtitle.clear();
        self.problem.clear();
        self.phase = ScanPhase::Scanning;
    }

    /// Feed one worker progress report into the flow.
    ///
    /// Returns `true` when the report says the scan finished, which is the
    /// point at which the caller should fetch the titles.
    pub fn apply_progress(&mut self, report: &ProgressReport) -> bool {
        self.phase = phase_of(report.state, true);
        if let Some(scan) = &report.scan {
            self.progress = progress_view(Some(scan));
        }
        let finished = matches!(report.state, WorkState::ScanDone | WorkState::WorkDone);
        if finished {
            if let Some(done) = &report.work_done {
                if !done.was_successful {
                    self.phase = ScanPhase::Failed;
                    self.problem = if done.message.trim().is_empty() {
                        "The scan did not finish.".to_string()
                    } else {
                        done.message.clone()
                    };
                }
            }
        }
        finished
    }

    /// Record the titles a finished scan produced.
    ///
    /// The main feature is chosen for the user; with no main feature the first
    /// title is, because a source with titles but nothing selected would look
    /// broken.
    pub fn apply_titles(&mut self, titles: TitlesResult) {
        let main_feature = titles.main_feature;
        self.titles = titles;
        self.phase = ScanPhase::Ready;
        self.progress = ScanProgressViewModel {
            fraction: 1.0,
            completed: self.progress.total.max(1),
            total: self.progress.total.max(1),
            status_line: format!("{} title(s) found", self.titles.title_list.len()),
            ..ScanProgressViewModel::default()
        };
        self.selected_title = main_feature;
        if self.selected_title == 0 {
            self.selected_title = self
                .titles
                .title_list
                .first()
                .map(|title| title.index)
                .unwrap_or(0);
        }
        if let Some(title) = self.current_title() {
            self.audio = default_audio_selection(title);
        }
        if self.titles.title_list.is_empty() {
            self.problem = "No titles were found in this source.".to_string();
        }
    }

    /// Record a scan failure with a reason.
    pub fn fail(&mut self, message: impl Into<String>) {
        self.phase = ScanPhase::Failed;
        self.problem = message.into();
    }

    /// Choose a title, resetting the selection that belonged to the old one.
    pub fn select_title(&mut self, index: u32) -> Result<(), String> {
        let Some(title) = self
            .titles
            .title_list
            .iter()
            .find(|title| title.index == index)
        else {
            return Err(format!("There is no title {index} in this source."));
        };
        let title_index = title.index;
        self.selected_title = title_index;
        // The mode the General preferences name, falling back to the whole title
        // when this title cannot use it: a title with no chapters still opens on
        // something usable rather than on a mode that is closed.
        let wanted = self.default_range_mode;
        let start = RangeSelection {
            mode: wanted,
            ..RangeSelection::default()
        };
        self.range = if range_view(title, &start).valid {
            start
        } else {
            RangeSelection::default()
        };
        self.audio = default_audio_selection(title);
        self.subtitle.clear();
        self.problem.clear();
        Ok(())
    }

    /// Apply a range and angle choice.
    ///
    /// A choice that cannot be used is reported back rather than stored, so the
    /// interface can show the reason next to the control the user touched.
    pub fn set_range(&mut self, selection: RangeSelection) -> Result<(), String> {
        let Some(title) = self.current_title() else {
            return Err("Choose a title before setting a range.".to_string());
        };
        let view = range_view(title, &selection);
        if !view.valid {
            return Err(view.problem);
        }
        self.range = selection;
        Ok(())
    }

    /// Apply a track choice for the chosen title.
    pub fn set_tracks(&mut self, input: TrackSelectionInput) -> Result<(), String> {
        let Some(title) = self.current_title() else {
            return Err("Choose a title before choosing tracks.".to_string());
        };
        let reconciled = input.reconcile(title);
        self.audio = reconciled.audio;
        self.subtitle = reconciled.subtitle;
        Ok(())
    }

    /// The range view for the chosen title, when there is one.
    pub fn range_view(&self) -> Option<RangeViewModel> {
        self.current_title()
            .map(|title| range_view(title, &self.range))
    }

    /// The state the interface renders.
    pub fn view_model(&self) -> SelectionViewModel {
        let title = self.current_title();
        let tracks = self.tracks();
        SelectionViewModel {
            source: self.source.clone(),
            phase: self.phase,
            phase_name: self.phase.as_str().to_string(),
            progress: Some(self.progress.clone()),
            titles: title_views(&self.titles),
            main_title_index: self.titles.main_feature,
            selected_title_index: self.selected_title,
            tracks: track_selection(title, &tracks.audio, &tracks.subtitle),
            range: self.range_view(),
            problem: self.problem.clone(),
        }
    }
}

/// Build the contract title list for the in-process fallback.
///
/// The fallback runs only when portable settings switch process isolation off,
/// so it needs the same title shape the worker would have produced.
pub fn titles_from_core_scan(result: &vise_core::ScanResult) -> TitlesResult {
    let container = result
        .format
        .format_name
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let name = result
        .path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("title")
        .to_string();

    let mut title_list: Vec<ScanTitle> = result
        .titles
        .iter()
        .enumerate()
        .map(|(position, title)| {
            let streams: Vec<&vise_core::StreamInfo> = title
                .stream_indices
                .iter()
                .filter_map(|index| result.streams.get(*index))
                .collect();
            let video = streams
                .iter()
                .copied()
                .find(|stream| stream.kind == TrackKind::Video)
                .or_else(|| result.streams.iter().find(|s| s.kind == TrackKind::Video));
            let audio: Vec<vise_ipc::contract::ScanAudioTrack> = streams
                .iter()
                .copied()
                .filter(|stream| stream.kind == TrackKind::Audio)
                .enumerate()
                .map(|(number, stream)| vise_ipc::contract::ScanAudioTrack {
                    track_number: number as u32 + 1,
                    codec_name: stream.codec.clone().unwrap_or_else(|| "unknown".into()),
                    channel_count: stream.channels.unwrap_or(0),
                    channel_layout: stream
                        .channel_layout
                        .clone()
                        .unwrap_or_else(|| "unknown".into()),
                    sample_rate: stream.sample_rate.unwrap_or(0),
                    bit_rate: stream.bit_rate.unwrap_or(0),
                    language: stream.language.clone().unwrap_or_else(|| "und".into()),
                    language_code: stream.language.clone().unwrap_or_else(|| "und".into()),
                    description: stream.title.clone().unwrap_or_default(),
                })
                .collect();
            let subtitles: Vec<vise_ipc::contract::ScanSubtitleTrack> = streams
                .iter()
                .copied()
                .filter(|stream| stream.kind == TrackKind::Subtitle)
                .enumerate()
                .map(|(number, stream)| vise_ipc::contract::ScanSubtitleTrack {
                    track_number: number as u32 + 1,
                    codec_name: stream.codec.clone().unwrap_or_else(|| "unknown".into()),
                    language: stream.language.clone().unwrap_or_else(|| "und".into()),
                    language_code: stream.language.clone().unwrap_or_else(|| "und".into()),
                    description: stream.title.clone().unwrap_or_default(),
                })
                .collect();
            ScanTitle {
                index: title.index.max(position + 1) as u32,
                name: name.clone(),
                container: container.clone(),
                duration_seconds: title
                    .duration_seconds
                    .or(result.format.duration_seconds)
                    .unwrap_or(0.0),
                video_codec: video
                    .and_then(|stream| stream.codec.clone())
                    .unwrap_or_else(|| "unknown".into()),
                width: video.and_then(|stream| stream.width).unwrap_or(0),
                height: video.and_then(|stream| stream.height).unwrap_or(0),
                frame_rate: video
                    .and_then(|stream| stream.frame_rate.clone())
                    .and_then(|text| {
                        text.split_once('/')
                            .and_then(|(num, den)| {
                                Some(num.parse::<f64>().ok()? / den.parse::<f64>().ok()?)
                            })
                            .filter(|rate| rate.is_finite())
                    })
                    .unwrap_or(0.0),
                angle_count: 1,
                crop: [0, 0, 0, 0],
                audio_list: audio,
                subtitle_list: subtitles,
                chapter_list: title
                    .chapters
                    .iter()
                    .map(|chapter| vise_ipc::contract::ScanChapter {
                        index: chapter.index as u32,
                        start_seconds: chapter.start_seconds.unwrap_or(0.0),
                        end_seconds: chapter.end_seconds.unwrap_or(0.0),
                        title: chapter.title.clone().unwrap_or_default(),
                    })
                    .collect(),
                meta_data: result.format.tags.clone(),
            }
        })
        .collect();

    let main_feature = title_list
        .iter()
        .max_by(|a, b| {
            a.duration_seconds
                .partial_cmp(&b.duration_seconds)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|title| title.index)
        .unwrap_or(0);
    for (position, title) in title_list.iter_mut().enumerate() {
        title.index = position as u32 + 1;
    }

    TitlesResult {
        main_feature,
        title_list,
    }
}

/// Merge several title lists into one, renumbering titles from one and picking
/// the longest title as the main feature.
pub fn merge_titles(results: Vec<TitlesResult>) -> TitlesResult {
    let mut merged = Vec::new();
    for result in results {
        merged.extend(result.title_list);
    }
    for (position, title) in merged.iter_mut().enumerate() {
        title.index = position as u32 + 1;
    }
    // Indices were just renumbered, so the main feature is the position of the
    // longest title in the merged list.
    let main_feature = merged
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            a.duration_seconds
                .partial_cmp(&b.duration_seconds)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(position, _)| position as u32 + 1)
        .unwrap_or(0);
    TitlesResult {
        main_feature,
        title_list: merged,
    }
}

/// Files a source expands to when the in-process fallback scans it.
pub fn fallback_files(source: &SourceViewModel) -> Result<Vec<std::path::PathBuf>, String> {
    let path = std::path::PathBuf::from(&source.path);
    if path.is_file() {
        return Ok(vec![path]);
    }
    if path.is_dir() {
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&path)
            .map_err(|e| format!("could not read {}: {e}", source.path))?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|candidate| candidate.is_file())
            .collect();
        files.sort();
        if files.is_empty() {
            return Err(format!("{} holds no files to scan.", source.path));
        }
        return Ok(files);
    }
    Err(format!("{} is no longer available.", source.path))
}

/// Scan a source in process, used only when portable settings switch process
/// isolation off.
///
/// The result is the same contract title list the worker would have produced,
/// so the window behaves the same either way.
pub fn scan_in_process(sources: &[SourceViewModel]) -> Result<TitlesResult, String> {
    let scanner = vise_core::FfprobeScanner::default();
    let mut results = Vec::new();
    for source in sources {
        for file in fallback_files(source)? {
            let result = scanner
                .scan_path(&file)
                .map_err(|e| format!("could not scan {}: {e}", file.display()))?;
            results.push(titles_from_core_scan(&result));
        }
    }
    if results.is_empty() {
        return Err("no media files were found to scan.".to_string());
    }
    Ok(merge_titles(results))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewmodel::RangeMode;
    use vise_ipc::contract::{ScanAudioTrack, ScanChapter, ScanProgress, WorkDone};

    fn audio(track_number: u32, language: &str, code: &str) -> ScanAudioTrack {
        ScanAudioTrack {
            track_number,
            codec_name: "aac".into(),
            channel_count: 2,
            channel_layout: "stereo".into(),
            sample_rate: 48_000,
            bit_rate: 192_000,
            language: language.into(),
            language_code: code.into(),
            description: String::new(),
        }
    }

    fn source() -> SourceViewModel {
        SourceViewModel {
            path: "C:\\media\\movie.mkv".into(),
            name: "movie.mkv".into(),
            location: "C:\\media".into(),
            kind: crate::sources::SourceKind::File,
            label: "movie.mkv · File".into(),
        }
    }

    fn titles() -> TitlesResult {
        let mut short = ScanTitle {
            index: 1,
            name: "movie".into(),
            container: "matroska".into(),
            duration_seconds: 120.0,
            width: 640,
            height: 360,
            frame_rate: 25.0,
            angle_count: 1,
            audio_list: vec![audio(1, "English", "eng")],
            ..ScanTitle::default()
        };
        let long = ScanTitle {
            index: 2,
            name: "movie".into(),
            container: "matroska".into(),
            duration_seconds: 5400.0,
            width: 1920,
            height: 1080,
            frame_rate: 23.976,
            angle_count: 2,
            audio_list: vec![audio(1, "English", "eng"), audio(2, "Japanese", "jpn")],
            subtitle_list: vec![vise_ipc::contract::ScanSubtitleTrack {
                track_number: 1,
                codec_name: "subrip".into(),
                language: "English".into(),
                language_code: "eng".into(),
                description: String::new(),
            }],
            chapter_list: vec![
                ScanChapter {
                    index: 0,
                    start_seconds: 0.0,
                    end_seconds: 60.0,
                    title: "One".into(),
                },
                ScanChapter {
                    index: 1,
                    start_seconds: 60.0,
                    end_seconds: 5400.0,
                    title: "Two".into(),
                },
            ],
            ..ScanTitle::default()
        };
        short.index = 1;
        TitlesResult {
            main_feature: 2,
            title_list: vec![short, long],
        }
    }

    fn scanning_report() -> ProgressReport {
        ProgressReport {
            state: WorkState::Working,
            scan: Some(ScanProgress {
                progress: 0.5,
                completed: 1,
                total: 2,
                current_path: "C:\\media\\movie.mkv".into(),
                title_count: 1,
            }),
            ..Default::default()
        }
    }

    fn done_report(successful: bool, message: &str) -> ProgressReport {
        ProgressReport {
            state: WorkState::WorkDone,
            work_done: Some(WorkDone {
                error: if successful { 0 } else { 2 },
                was_successful: successful,
                message: message.into(),
            }),
            ..Default::default()
        }
    }

    #[test]
    fn a_new_flow_starts_idle_and_empty() {
        let flow = ScanFlow::default();
        let view = flow.view_model();
        assert_eq!(view.phase, ScanPhase::Idle);
        assert!(view.titles.is_empty());
        assert!(view.tracks.audio.is_empty());
        assert!(view.range.is_none());
        assert!(!view.is_usable());
    }

    #[test]
    fn progress_moves_the_flow_through_scanning_to_ready() {
        let mut flow = ScanFlow::default();
        flow.set_source(Some(source()));
        flow.begin();
        assert!(flow.is_running());

        assert!(!flow.apply_progress(&scanning_report()));
        let view = flow.view_model();
        assert_eq!(view.phase, ScanPhase::Scanning);
        assert_eq!(view.progress.expect("progress").percent, 50);

        assert!(flow.apply_progress(&done_report(true, "")));
        flow.apply_titles(titles());
        let view = flow.view_model();
        assert_eq!(view.phase, ScanPhase::Ready);
        assert!(view.is_usable());
        assert_eq!(view.main_title_index, 2);
        assert_eq!(view.selected_title_index, 2, "the main feature is chosen");
        assert_eq!(view.titles.len(), 2);
        assert_eq!(view.tracks.audio.len(), 2);
        assert!(
            view.tracks.audio[0].selected,
            "the first audio track is chosen by default"
        );
        assert!(
            !view.tracks.subtitle[0].selected,
            "no subtitle track is chosen by default"
        );
        assert_eq!(view.range.expect("range").mode, RangeMode::Whole);
    }

    #[test]
    fn a_failed_scan_reports_the_reason_and_clears_results() {
        let mut flow = ScanFlow::default();
        flow.set_source(Some(source()));
        flow.begin();
        assert!(flow.apply_progress(&done_report(false, "could not read the file")));
        flow.apply_titles(titles());
        flow.fail("could not read the file");
        let view = flow.view_model();
        assert_eq!(view.phase, ScanPhase::Failed);
        assert!(view.problem.contains("could not read"));
        assert!(!view.is_usable());
    }

    #[test]
    fn a_scan_that_finished_without_titles_is_a_problem() {
        let mut flow = ScanFlow::default();
        flow.set_source(Some(source()));
        flow.begin();
        flow.apply_titles(TitlesResult::default());
        let view = flow.view_model();
        assert_eq!(view.phase, ScanPhase::Ready);
        assert!(view.problem.contains("No titles"));
        assert!(!view.is_usable());
    }

    #[test]
    fn choosing_a_title_moves_the_track_and_range_selection() {
        let mut flow = ScanFlow::default();
        flow.set_source(Some(source()));
        flow.apply_titles(titles());
        flow.set_range(RangeSelection {
            mode: RangeMode::Chapters,
            start_seconds: Some(60.0),
            end_seconds: Some(120.0),
            angle: Some(2),
        })
        .expect("range");
        flow.set_tracks(TrackSelectionInput {
            audio: vec![2],
            subtitle: vec![1],
        })
        .expect("tracks");
        assert_eq!(flow.view_model().range.expect("range").start_seconds, 60.0);

        flow.select_title(1).expect("title 1");
        assert_eq!(flow.view_model().selected_title_index, 1);
        assert_eq!(flow.tracks().audio, vec![1], "one audio track on title 1");
        assert!(flow.tracks().subtitle.is_empty());
        assert_eq!(
            flow.view_model().range.expect("range").mode,
            RangeMode::Whole,
            "the range was reset"
        );
        assert!(
            !flow.view_model().range.expect("range").has_angles,
            "title 1 reports a single angle"
        );

        assert!(flow.select_title(9).is_err());
    }

    #[test]
    fn an_unusable_range_is_refused_with_its_reason() {
        let mut flow = ScanFlow::default();
        flow.set_source(Some(source()));
        flow.apply_titles(titles());
        let error = flow
            .set_range(RangeSelection {
                mode: RangeMode::PointToPoint,
                start_seconds: Some(90.0),
                end_seconds: None,
                angle: None,
            })
            .expect_err("missing end point");
        assert!(error.contains("start point"));
        assert_eq!(
            flow.view_model().range.expect("range").mode,
            RangeMode::Whole,
            "the old choice stands"
        );

        let error = flow
            .set_range(RangeSelection {
                mode: RangeMode::Whole,
                angle: Some(5),
                ..Default::default()
            })
            .expect_err("angle beyond the source");
        assert!(error.contains("2 angle"));
    }

    #[test]
    fn a_range_without_a_title_is_refused() {
        let mut flow = ScanFlow::default();
        flow.set_source(Some(source()));
        assert!(flow
            .set_range(RangeSelection::default())
            .expect_err("no title")
            .contains("Choose a title"));
        assert!(flow
            .set_tracks(TrackSelectionInput::default())
            .expect_err("no title")
            .contains("Choose a title"));
    }

    #[test]
    fn choosing_tracks_drops_numbers_the_title_does_not_have() {
        let mut flow = ScanFlow::default();
        flow.set_source(Some(source()));
        flow.apply_titles(titles());
        flow.set_tracks(TrackSelectionInput {
            audio: vec![7, 2],
            subtitle: vec![1, 4],
        })
        .expect("tracks");
        assert_eq!(flow.tracks().audio, vec![2]);
        assert_eq!(flow.tracks().subtitle, vec![1]);
        let view = flow.view_model();
        assert!(view.tracks.audio[1].selected);
        assert!(!view.tracks.audio[0].selected);
    }

    #[test]
    fn a_new_source_clears_everything_the_previous_scan_produced() {
        let mut flow = ScanFlow::default();
        flow.set_source(Some(source()));
        flow.apply_titles(titles());
        assert_eq!(flow.view_model().titles.len(), 2);

        flow.set_source(None);
        let view = flow.view_model();
        assert_eq!(view.phase, ScanPhase::Idle);
        assert!(view.titles.is_empty());
        assert!(view.source.is_none());
        assert!(view.tracks.audio.is_empty());
        assert!(view.range.is_none());
    }

    #[test]
    fn the_selected_title_reports_its_own_chapters_and_tracks() {
        let mut flow = ScanFlow::default();
        flow.set_source(Some(source()));
        flow.apply_titles(titles());
        flow.select_title(2).expect("title 2");
        let view = flow.view_model();
        let range = view.range.expect("range");
        assert_eq!(range.chapters.len(), 2);
        assert_eq!(range.chapters[0].label, "Chapter 1 · One · 0:00 – 1:00");
        assert!(range.has_angles);
        assert_eq!(range.modes.len(), 3);
        assert!(
            view.tracks.audio[0].selected,
            "the first track is the default"
        );
        assert!(!view.tracks.subtitle[0].selected);
    }

    #[test]
    fn the_in_process_fallback_produces_titles_the_flow_accepts() {
        let mut flow = ScanFlow::default();
        flow.set_source(Some(source()));
        let result = vise_ipc::validation::synthetic_scan(12.0, 640, 360, Some(1));
        let converted = titles_from_core_scan(&result);
        assert_eq!(converted.title_list.len(), 1);
        assert_eq!(converted.title_list[0].duration_seconds, 12.0);
        assert_eq!(converted.title_list[0].audio_list.len(), 1);
        flow.apply_titles(converted);
        let view = flow.view_model();
        assert_eq!(view.phase, ScanPhase::Ready);
        assert_eq!(view.selected_title_index, 1);
        assert_eq!(view.titles[0].resolution_label, "640x360");
    }

    #[test]
    fn merged_titles_are_renumbered_and_main_feature_is_the_longest() {
        let mut first = titles();
        let mut second = titles();
        for (position, title) in first.title_list.iter_mut().enumerate() {
            title.index = position as u32 + 1;
            title.duration_seconds = 10.0;
        }
        for (position, title) in second.title_list.iter_mut().enumerate() {
            title.index = position as u32 + 1;
            title.duration_seconds = 30.0 + position as f64;
        }
        let merged = merge_titles(vec![first, second]);
        assert_eq!(merged.title_list.len(), 4);
        assert_eq!(
            merged
                .title_list
                .iter()
                .map(|title| title.index)
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );
        assert_eq!(merged.main_feature, 4);
        assert_eq!(merge_titles(Vec::new()).main_feature, 0);
    }

    #[test]
    fn the_fallback_expands_files_and_folders() {
        let dir = std::env::temp_dir().join("vise-desktop-fallback");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("b.mkv"), b"x").expect("b");
        std::fs::write(dir.join("a.mkv"), b"x").expect("a");

        let folder = crate::sources::resolve_source(dir.to_str().unwrap()).expect("folder");
        let files = fallback_files(&folder).expect("files");
        assert_eq!(files.len(), 2, "sorted, so the order is stable");
        assert!(files[0].ends_with("a.mkv"));

        let file =
            crate::sources::resolve_source(dir.join("a.mkv").to_str().unwrap()).expect("file");
        assert_eq!(fallback_files(&file).expect("files").len(), 1);

        let empty = temp_dir("fallback-empty");
        let empty_view = crate::sources::resolve_source(empty.to_str().unwrap()).expect("empty");
        assert!(fallback_files(&empty_view)
            .expect_err("no files")
            .contains("no files"));
    }

    #[test]
    fn the_fallback_scan_reports_a_source_it_cannot_read() {
        // An unreadable payload is refused with a reason instead of an empty
        // title list the interface would have to explain.
        let bogus = SourceViewModel {
            path: "does-not-exist.mkv".into(),
            name: "does-not-exist.mkv".into(),
            location: String::new(),
            kind: crate::sources::SourceKind::File,
            label: "does-not-exist.mkv · File".into(),
        };
        assert!(scan_in_process(&[bogus])
            .expect_err("missing source")
            .contains("no longer available"));
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("vise-desktop-flow-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }
}
