//! The finished queue as data somebody can do arithmetic on.
//!
//! A queue of forty encodes produces forty answers: how big each file came out,
//! how long it took, what codec it was written with, which ones failed and why.
//! All of that exists, in the window, and none of it can be compared. So it is
//! exported as a table — CSV for a spreadsheet, JSON for a program — and the
//! table is a *deterministic* one.
//!
//! # Deterministic, because a diff is the point
//!
//! Two exports of the same queue produce the same bytes: the rows are in the
//! queue's run order, the columns are in a fixed order, and the JSON is
//! serialised from a struct rather than assembled from a map. Somebody who
//! exports the queue before and after a settings change and compares the two
//! files with a diff tool has to be looking at what changed, not at the order a
//! hash map happened to iterate in.
//!
//! # Escaping, because titles are not identifiers
//!
//! A title called `Episode 1: "The Return", Part 2` is a perfectly ordinary
//! thing for a disc to contain, and both of those punctuation marks are
//! meaningful in CSV. [`csv_field`] quotes a field when it contains a comma, a
//! quote, a line break, or leading and trailing space, and doubles the quotes
//! inside it — the rules a spreadsheet expects. A line break *inside* a value is
//! kept as a line break inside the quotes rather than flattened to a space,
//! because flattening it would change the value: the export would round-trip to
//! something that is not what was encoded.
//!
//! Paths are exported as file names, not as absolute paths. An export is a
//! document that gets attached to something and read by somebody else, and a
//! column of `C:\Users\<name>\...` is the one thing in it that identifies the
//! machine it came from and helps nobody compare two encodes.

use std::path::Path;

use serde::{Deserialize, Serialize};

use vise_core::{Job, JobId, JobSpec};

use crate::windows::queue::{state_name, QueueState};

/// Version of the export document this build writes.
pub const EXPORT_VERSION: u32 = 1;

/// Column headings, in the order they are written.
///
/// One list rather than three written out separately, because a CSV whose header
/// disagrees with its rows is the failure a person finds in a spreadsheet after
/// they have already drawn a conclusion from it.
pub const COLUMNS: [&str; 15] = [
    "position",
    "job",
    "priority",
    "title",
    "source",
    "destination",
    "container",
    "video_codec",
    "audio_codec",
    "quality",
    "preset",
    "size",
    "state",
    "percent",
    "message",
];

/// One row of the table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultRow {
    /// Position in the run order, from one.
    pub position: usize,
    /// The job's identifier in this session.
    pub job: JobId,
    /// What the job was worth, when it was marked.
    pub priority: String,
    /// The output file's name.
    pub title: String,
    /// The input file's name.
    pub source: String,
    /// The output file's name again, for a reader who wants it beside the size.
    pub destination: String,
    /// Container the job wrote.
    pub container: String,
    /// Video codec the job wrote.
    pub video_codec: String,
    /// Audio codec the job wrote.
    pub audio_codec: String,
    /// The quality value the job asked for, when it asked for one.
    pub quality: String,
    /// Encoder speed preset, when one was chosen.
    pub preset: String,
    /// Size of the finished file in bytes; zero when there is no file.
    pub size: u64,
    /// Where the job ended up.
    pub state: String,
    /// How far it got, as a whole percentage.
    pub percent: u32,
    /// What the encoder last said.
    pub message: String,
}

impl ResultRow {
    /// The cells, in [`COLUMNS`] order.
    pub fn cells(&self) -> Vec<String> {
        vec![
            self.position.to_string(),
            self.job.to_string(),
            self.priority.clone(),
            self.title.clone(),
            self.source.clone(),
            self.destination.clone(),
            self.container.clone(),
            self.video_codec.clone(),
            self.audio_codec.clone(),
            self.quality.clone(),
            self.preset.clone(),
            self.size.to_string(),
            self.state.clone(),
            self.percent.to_string(),
            self.message.clone(),
        ]
    }
}

/// The whole table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueResults {
    /// Version of the export document.
    pub version: u32,
    /// How many rows the table holds.
    pub total: usize,
    /// How many finished well.
    pub completed: usize,
    /// How many did not.
    pub failed: usize,
    /// How many are still to do or are running.
    pub unfinished: usize,
    /// The rows, in the queue's run order.
    pub rows: Vec<ResultRow>,
}

/// The measured size of a finished file, and where it came from.
///
/// Injected rather than read here so the table can be built — and checked —
/// without a filesystem, and so a caller that already knows the size does not
/// stat the same file twice.
pub type SizeLookup<'a> = &'a dyn Fn(&JobSpec) -> u64;

impl QueueResults {
    /// Build the table from the jobs and the queue's own order.
    ///
    /// `size` answers "how big is this job's output", and is asked about jobs that
    /// *finished well* only: a job that failed or was cancelled has no output to
    /// measure, and asking anyway would be asking about a path that is not there.
    pub fn of(jobs: &[Job], queue: &QueueState, size: SizeLookup<'_>) -> Self {
        let rows: Vec<ResultRow> = queue
            .run_order(jobs)
            .into_iter()
            .enumerate()
            .map(|(index, job)| {
                let spec = &job.spec;
                let state = job.status.state;
                let wrote_a_file = state == vise_core::JobState::Completed;
                ResultRow {
                    position: index + 1,
                    job: job.id,
                    priority: queue.priority_of(job.id).as_str().to_string(),
                    title: file_name(&spec.output_path),
                    source: file_name(&spec.input_path),
                    destination: file_name(&spec.output_path),
                    container: container_name(spec.container),
                    video_codec: spec.video_codec.label().to_string(),
                    audio_codec: spec.audio_codec.label().to_string(),
                    quality: quality_text(spec),
                    preset: spec.video_encoding.preset.clone().unwrap_or_default(),
                    size: if wrote_a_file { size(spec) } else { 0 },
                    state: state_name(state).to_string(),
                    percent: (job.status.progress.fraction.clamp(0.0, 1.0) * 100.0).round() as u32,
                    message: job.status.message.clone().unwrap_or_default(),
                }
            })
            .collect();
        let completed = rows
            .iter()
            .filter(|row| row.state == state_name(vise_core::JobState::Completed))
            .count();
        let failed = rows
            .iter()
            .filter(|row| row.state == state_name(vise_core::JobState::Failed))
            .count();
        QueueResults {
            version: EXPORT_VERSION,
            total: rows.len(),
            completed,
            failed,
            unfinished: rows.len().saturating_sub(completed + failed),
            rows,
        }
    }

    /// The table as CSV.
    ///
    /// Line breaks between rows are `\n` rather than `\r\n`: the file is read
    /// and compared far more often than it is opened in a spreadsheet that
    /// predates Unix line endings, and both are accepted by everything that
    /// reads CSV today.
    pub fn to_csv(&self) -> String {
        let mut lines: Vec<String> = vec![COLUMNS
            .iter()
            .map(|column| csv_field(column))
            .collect::<Vec<_>>()
            .join(",")];
        for row in &self.rows {
            lines.push(
                row.cells()
                    .iter()
                    .map(|cell| csv_field(cell))
                    .collect::<Vec<_>>()
                    .join(","),
            );
        }
        let mut out = lines.join("\n");
        out.push('\n');
        out
    }

    /// The table as JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// One line describing the export.
    pub fn summary(&self) -> String {
        if self.rows.is_empty() {
            return "The queue is empty, so there is nothing to export.".to_string();
        }
        format!(
            "{} row(s): {} finished, {} failed, {} unfinished",
            self.total, self.completed, self.failed, self.unfinished
        )
    }
}

/// Quote one CSV field, if it needs quoting.
///
/// The rules, in full: a field is quoted when it contains a comma, a double
/// quote, a carriage return or a line feed, or when it begins or ends with a
/// space. Inside a quoted field a double quote is written twice. Nothing else
/// is touched, so a value with none of those characters is written exactly as
/// it is — which is what keeps a diff of two exports readable.
pub fn csv_field(value: &str) -> String {
    let needs_quotes =
        value.contains([',', '"', '\n', '\r']) || value.starts_with(' ') || value.ends_with(' ');
    if !needs_quotes {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for character in value.chars() {
        if character == '"' {
            out.push('"');
        }
        out.push(character);
    }
    out.push('"');
    out
}

/// The last component of a path, which is the part a person recognises.
///
/// A path with no name at all is returned whole rather than as nothing: an empty
/// cell in an export is a question the reader cannot answer, and a folder name is
/// at least a hint about what the job was pointed at.
pub fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| path.to_string())
}

/// The container's own name, as the job model writes it.
///
/// Read from the job model rather than from a second table here, so a container
/// this build does not know about cannot produce a blank cell in an export
/// somebody is about to sort.
fn container_name(container: vise_core::ContainerFormat) -> String {
    serde_json::to_value(container)
        .ok()
        .and_then(|value| match value {
            serde_json::Value::String(name) => Some(name),
            _ => None,
        })
        .unwrap_or_else(|| "unknown".to_string())
}

/// The quality a job asked for, as one cell.
///
/// Several fields carry the value in this codebase's job model because jobs are
/// read from files other builds wrote; one cell is written rather than seven
/// columns of the same number.
fn quality_text(spec: &JobSpec) -> String {
    spec.quality
        .or(spec.cq)
        .or(spec.crf)
        .or(spec.constant_quality_value)
        .map(|value| value.to_string())
        .unwrap_or_else(|| {
            spec.target_size
                .or(spec.target_file_size)
                .or(spec.target_size_bytes)
                .map(|bytes| format!("{bytes} bytes"))
                .or_else(|| {
                    spec.bitrate
                        .or(spec.average_bitrate)
                        .or(spec.abr)
                        .or(spec.target_bitrate)
                        .or(spec.video_bitrate)
                        .map(|rate| format!("{rate} kb/s"))
                })
                .unwrap_or_default()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use vise_core::{ContainerFormat, JobState, JobStatus, Progress};

    fn spec(name: &str) -> JobSpec {
        JobSpec {
            input_path: format!("C:\\media\\raw\\{name}.mkv"),
            output_path: format!("D:\\out\\{name}.mp4"),
            container: ContainerFormat::Mp4,
            video_codec: vise_core::VideoCodec::H265,
            audio_codec: vise_core::AudioCodec::Aac,
            quality: Some(20),
            video_encoding: vise_core::VideoEncodingOptions {
                preset: Some("slow".to_string()),
                ..Default::default()
            },
            ..JobSpec::default()
        }
    }

    fn job(id: JobId, name: &str, state: JobState) -> Job {
        Job {
            id,
            spec: spec(name),
            status: JobStatus {
                state,
                progress: Progress::new(if state == JobState::Completed {
                    1.0
                } else {
                    0.5
                }),
                message: Some(format!("{name} finished")),
            },
        }
    }

    fn no_sizes(_: &JobSpec) -> u64 {
        0
    }

    /// A parser written the way a spreadsheet would write one, so that
    /// "round-trips" is a claim about the format rather than about the writer.
    fn parse_csv(text: &str) -> Vec<Vec<String>> {
        let mut rows = Vec::new();
        let mut row = Vec::new();
        let mut field = String::new();
        let mut quoted = false;
        let mut characters = text.chars().peekable();
        while let Some(character) = characters.next() {
            if quoted {
                if character == '"' {
                    if characters.peek() == Some(&'"') {
                        characters.next();
                        field.push('"');
                    } else {
                        quoted = false;
                    }
                } else {
                    field.push(character);
                }
                continue;
            }
            match character {
                '"' => quoted = true,
                ',' => row.push(std::mem::take(&mut field)),
                '\n' => {
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                }
                other => field.push(other),
            }
        }
        if !field.is_empty() || !row.is_empty() {
            row.push(field);
            rows.push(row);
        }
        rows
    }

    #[test]
    fn the_table_is_the_queue_in_its_own_run_order() {
        let jobs = vec![
            job(1, "One", JobState::Completed),
            job(2, "Two", JobState::Failed),
            job(3, "Three", JobState::Queued),
        ];
        let mut queue = QueueState::default();
        for job in &jobs {
            queue.push(job.id);
        }
        queue.set_priority(3, crate::windows::queue::JobPriority::High);

        let results = QueueResults::of(&jobs, &queue, &no_sizes);
        assert_eq!(results.total, 3);
        assert_eq!(results.completed, 1);
        assert_eq!(results.failed, 1);
        assert_eq!(results.unfinished, 1);
        // The high-priority job runs first, so it is first in the table.
        assert_eq!(
            results.rows.iter().map(|row| row.job).collect::<Vec<_>>(),
            vec![3, 1, 2]
        );
        assert_eq!(results.rows[0].position, 1);
        assert_eq!(results.rows[0].priority, "high");
        assert_eq!(results.rows[2].percent, 50);
        assert!(
            results.summary().contains("3 row(s)"),
            "{}",
            results.summary()
        );
    }

    #[test]
    fn a_title_with_a_comma_and_a_quote_round_trips_through_the_csv() {
        let awkward = Job {
            id: 1,
            spec: JobSpec {
                output_path: "D:\\out\\Episode 1: \"The Return\", Part 2.mkv".into(),
                ..spec("raw")
            },
            status: JobStatus {
                state: JobState::Completed,
                progress: Progress::new(1.0),
                message: Some("encoded \"frame= 120\", then stopped\nand finished".into()),
            },
        };
        let jobs = vec![awkward];
        let mut queue = QueueState::default();
        queue.push(1);
        let results = QueueResults::of(&jobs, &queue, &no_sizes);
        let csv = results.to_csv();
        let parsed = parse_csv(&csv);
        assert_eq!(parsed[0], COLUMNS.to_vec(), "the header is the column list");
        assert_eq!(parsed.len(), 2, "one header and one row");
        assert_eq!(
            parsed[1][3], "Episode 1: \"The Return\", Part 2.mkv",
            "the comma and the quotes survive"
        );
        assert_eq!(
            parsed[1][14], "encoded \"frame= 120\", then stopped\nand finished",
            "and so does the line break inside the value"
        );
    }

    #[test]
    fn a_field_is_quoted_exactly_when_it_has_to_be() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field(""), "");
        assert_eq!(csv_field("has,comma"), "\"has,comma\"");
        assert_eq!(csv_field("has\"quote"), "\"has\"\"quote\"");
        assert_eq!(csv_field("has\nbreak"), "\"has\nbreak\"");
        assert_eq!(csv_field("has\r\nbreak"), "\"has\r\nbreak\"");
        assert_eq!(csv_field(" leading"), "\" leading\"");
        assert_eq!(csv_field("trailing "), "\"trailing \"");
        assert_eq!(csv_field("\"both\""), "\"\"\"both\"\"\"");
        // A quote inside a field that needed quoting anyway is still doubled
        // exactly once, which is the case a naive implementation gets wrong.
        assert_eq!(csv_field("a,\"b\""), "\"a,\"\"b\"\"\"");
    }

    #[test]
    fn the_same_queue_exports_the_same_bytes_twice_over() {
        let jobs = vec![
            job(2, "Two", JobState::Failed),
            job(1, "One", JobState::Completed),
        ];
        let mut queue = QueueState::default();
        for job in &jobs {
            queue.push(job.id);
        }
        queue.set_priority(1, crate::windows::queue::JobPriority::Low);
        let size = |spec: &JobSpec| spec.output_path.len() as u64;

        let first = QueueResults::of(&jobs, &queue, &size);
        let second = QueueResults::of(&jobs, &queue, &size);
        assert_eq!(first.to_csv(), second.to_csv());
        assert_eq!(first.to_json(), second.to_json());
        assert!(!first.to_csv().is_empty());
    }

    #[test]
    fn the_csv_header_names_every_column_and_the_row_has_one_cell_each() {
        let jobs = vec![job(1, "One", JobState::Completed)];
        let mut queue = QueueState::default();
        queue.push(1);
        let results = QueueResults::of(&jobs, &queue, &|_: &JobSpec| 4096);
        let parsed = parse_csv(&results.to_csv());
        assert_eq!(parsed[0].len(), COLUMNS.len());
        assert_eq!(parsed[1].len(), COLUMNS.len());
        assert_eq!(parsed[1][0], "1");
        assert_eq!(parsed[1][11], "4096", "the size the lookup answered with");
        assert_eq!(parsed[1][12], "completed");
        assert_eq!(parsed[1][10], "slow", "the encoder preset is carried");
        assert_eq!(parsed[1][6], "mp4");
        assert_eq!(parsed[1][7], "H.265");
        assert_eq!(parsed[1][9], "20");
    }

    #[test]
    fn the_json_form_round_trips_and_carries_the_same_rows() {
        let jobs = vec![
            job(1, "One", JobState::Completed),
            job(2, "Two", JobState::Failed),
        ];
        let mut queue = QueueState::default();
        for job in &jobs {
            queue.push(job.id);
        }
        let results = QueueResults::of(&jobs, &queue, &|_: &JobSpec| 1024);
        let json = results.to_json();
        let back: QueueResults = serde_json::from_str(&json).expect("round trip");
        assert_eq!(back, results);
        assert!(json.contains("\"container\": \"mp4\""));
    }

    #[test]
    fn only_jobs_that_wrote_a_file_are_asked_about_its_size() {
        let jobs = vec![
            job(1, "One", JobState::Completed),
            job(2, "Two", JobState::Running),
            job(3, "Three", JobState::Failed),
        ];
        let mut queue = QueueState::default();
        for job in &jobs {
            queue.push(job.id);
        }
        let asked = std::cell::RefCell::new(Vec::new());
        let results = {
            let asked = &asked;
            QueueResults::of(&jobs, &queue, &|spec: &JobSpec| {
                asked.borrow_mut().push(spec.output_path.clone());
                100
            })
        };
        assert_eq!(asked.borrow().len(), 1, "only the finished one has a file");
        assert!(asked.borrow()[0].ends_with("One.mp4"));
        let by_job = |id: JobId| {
            results
                .rows
                .iter()
                .find(|row| row.job == id)
                .map(|row| row.size)
                .unwrap()
        };
        assert_eq!(by_job(1), 100);
        assert_eq!(by_job(2), 0);
        assert_eq!(
            by_job(3),
            0,
            "a failed job wrote no file, so nothing is asked about one"
        );
    }

    #[test]
    fn paths_are_exported_as_names_and_not_as_somebody_s_drive_letters() {
        assert_eq!(file_name("D:\\out\\Episode 1.mkv"), "Episode 1.mkv");
        assert_eq!(file_name("/home/user/clip.mkv"), "clip.mkv");
        assert_eq!(file_name("bare.mkv"), "bare.mkv");
        assert_eq!(file_name(""), "");
        assert_eq!(
            file_name("D:\\out\\"),
            "out",
            "a folder names its last part"
        );

        let jobs = vec![job(1, "One", JobState::Completed)];
        let mut queue = QueueState::default();
        queue.push(1);
        let results = QueueResults::of(&jobs, &queue, &no_sizes);
        let csv = results.to_csv();
        assert!(!csv.contains("C:\\"), "{csv}");
        assert!(!csv.contains("D:\\"), "{csv}");
    }

    #[test]
    fn an_empty_queue_exports_a_header_and_nothing_else() {
        let results = QueueResults::of(&[], &QueueState::default(), &no_sizes);
        let csv = results.to_csv();
        assert_eq!(csv.lines().count(), 1, "a header and nothing else");
        assert!(csv.starts_with("position,job,priority,title"));
        assert_eq!(csv, "position,job,priority,title,source,destination,container,video_codec,audio_codec,quality,preset,size,state,percent,message\n");
        assert_eq!(
            results.summary(),
            "The queue is empty, so there is nothing to export."
        );
        let parsed = parse_csv(&results.to_json());
        assert!(!parsed.is_empty());
    }

    #[test]
    fn a_job_with_no_quality_value_says_so_rather_than_guessing_one() {
        let jobs = vec![Job {
            id: 1,
            spec: JobSpec {
                quality: None,
                cq: None,
                crf: None,
                constant_quality_value: None,
                ..spec("One")
            },
            status: JobStatus::default(),
        }];
        let mut queue = QueueState::default();
        queue.push(1);
        let results = QueueResults::of(&jobs, &queue, &no_sizes);
        assert_eq!(results.rows[0].quality, "");

        let sized = vec![Job {
            id: 1,
            spec: JobSpec {
                quality: None,
                cq: None,
                crf: None,
                constant_quality_value: None,
                target_size: Some(4_000_000_000),
                ..spec("One")
            },
            status: JobStatus::default(),
        }];
        let results = QueueResults::of(&sized, &queue, &no_sizes);
        assert_eq!(results.rows[0].quality, "4000000000 bytes");

        let bitrated = vec![Job {
            id: 1,
            spec: JobSpec {
                quality: None,
                cq: None,
                crf: None,
                constant_quality_value: None,
                bitrate: Some(6000),
                ..spec("One")
            },
            status: JobStatus::default(),
        }];
        let results = QueueResults::of(&bitrated, &queue, &no_sizes);
        assert_eq!(results.rows[0].quality, "6000 kb/s");
    }
}
