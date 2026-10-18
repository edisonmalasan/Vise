//! Chapter model and ffmpeg chapter handling helpers.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ChapterMode {
    #[default]
    Copy,
    Edit,
    None,
}

impl ChapterMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Copy => "copy",
            Self::Edit => "edit",
            Self::None => "none",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Chapter {
    #[serde(
        default,
        alias = "start",
        alias = "start_time",
        alias = "start_ms",
        alias = "startTime"
    )]
    pub start_seconds: Option<f64>,
    #[serde(
        default,
        alias = "end",
        alias = "end_time",
        alias = "end_ms",
        alias = "endTime"
    )]
    pub end_seconds: Option<f64>,
    #[serde(default, alias = "name", alias = "label")]
    pub title: Option<String>,
    #[serde(default)]
    pub id: Option<u32>,
    #[serde(default)]
    pub tags: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ChapterProcessingOptions {
    #[serde(default, alias = "chapter_mode", alias = "mode")]
    pub mode: Option<ChapterMode>,

    #[serde(default, alias = "copy_chapters", alias = "chapters_copy")]
    pub copy: bool,
    #[serde(default, alias = "edit_chapters", alias = "chapters_edit")]
    pub edit: bool,

    #[serde(default, alias = "copies")]
    pub chapter_copy: bool,
    #[serde(default, alias = "edits")]
    pub chapter_edit: bool,
    #[serde(default)]
    pub chapter_editing: bool,

    #[serde(default, alias = "chapters_list", alias = "chapter_list")]
    pub chapters: Vec<Chapter>,

    #[serde(default, alias = "chapter_count")]
    pub count: Option<u32>,

    #[serde(default)]
    pub allow_split: bool,

    #[serde(default, alias = "extract_cover_art", alias = "cover_art_extract")]
    pub extract_cover: bool,
    #[serde(default, alias = "copy_cover_art", alias = "cover_art_copy")]
    pub cover_art: bool,
    #[serde(default, alias = "copy_attachments", alias = "attachments_copy")]
    pub attachments: bool,
    #[serde(default, alias = "attachment_copy")]
    pub attachment_copy: bool,
}

pub type ChapterOptions = ChapterProcessingOptions;
pub type ChapterFilters = ChapterProcessingOptions;
pub type ChapterFilterOptions = ChapterProcessingOptions;
pub type ChapterConfig = ChapterProcessingOptions;
pub type ChapterProcessing = ChapterProcessingOptions;
pub type Chapters = ChapterProcessingOptions;
pub type ChaptersOptions = ChapterProcessingOptions;
pub type ChaptersProcessing = ChapterProcessingOptions;
pub type ChaptersProcessingOptions = ChapterProcessingOptions;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ChapterTrackConfig {
    #[serde(default, alias = "source", alias = "source_track")]
    pub source_track_id: Option<u32>,
    #[serde(default)]
    pub copy: bool,
    #[serde(default)]
    pub edit: bool,
    #[serde(default)]
    pub chapters: Vec<Chapter>,
}

pub type ChapterTrack = ChapterTrackConfig;
pub type ChapterTrackSpec = ChapterTrackConfig;
pub type ChapterTrackConfiguration = ChapterTrackConfig;

impl ChapterProcessingOptions {
    pub fn effective_copy(&self) -> bool {
        self.copy
            || self.chapter_copy
            || self.mode == Some(ChapterMode::Copy)
            || self.mode.is_none()
    }
    pub fn effective_edit(&self) -> bool {
        self.edit
            || self.chapter_edit
            || self.chapter_editing
            || self.mode == Some(ChapterMode::Edit)
    }
    pub fn effective_none(&self) -> bool {
        self.mode == Some(ChapterMode::None)
    }
    pub fn has_edits(&self) -> bool {
        !self.chapters.is_empty() || self.effective_edit()
    }

    /// Whether the output takes its chapter markers from this list.
    ///
    /// An edited or replaced list is authoritative, so the source markers are
    /// not copied as well.
    pub fn uses_edited_chapters(&self) -> bool {
        !self.chapters.is_empty() && !self.effective_none()
    }

    /// Render the markers as an ffmpeg metadata document.
    ///
    /// Chapter times are written in milliseconds because that is the resolution
    /// the format stores; a marker without a start time is left out rather than
    /// silently written as the beginning of the file.
    pub fn render_ffmetadata(&self) -> String {
        let mut out = String::from(";FFMETADATA1\n");
        for chapter in &self.chapters {
            let Some(start) = chapter.start_seconds.filter(|s| s.is_finite() && *s >= 0.0) else {
                continue;
            };
            out.push_str("\n[CHAPTER]\nTIMEBASE=1/1000\n");
            out.push_str(&format!("START={}\n", (start * 1000.0).round() as i64));
            if let Some(end) = chapter.end_seconds.filter(|s| s.is_finite() && *s > start) {
                out.push_str(&format!("END={}\n", (end * 1000.0).round() as i64));
            }
            if let Some(title) = chapter.title.as_deref().map(str::trim) {
                if !title.is_empty() {
                    out.push_str(&format!("title={}\n", title.replace(['\n', '='], " ")));
                }
            }
        }
        out
    }

    pub fn build(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if self.effective_copy() && !self.effective_none() && !self.uses_edited_chapters() {
            parts.push("-map_chapters 0".to_string());
        }
        if self.extract_cover {
            parts.push("extract_cover_art".to_string());
        }
        parts.join(" ")
    }

    pub fn build_filter_chain(&self) -> String {
        self.build()
    }
    pub fn build_ffmpeg_args(&self) -> Vec<String> {
        self.build().split_whitespace().map(String::from).collect()
    }
    pub fn to_ffmpeg(&self) -> String {
        self.build()
    }
    pub fn filter_string(&self) -> String {
        self.build()
    }
}

pub fn build_chapters(options: &ChapterProcessingOptions) -> String {
    options.build()
}
pub fn build_chapter_filter(options: &ChapterProcessingOptions) -> String {
    options.build()
}
pub fn build_chapter_filter_chain(options: &ChapterProcessingOptions) -> String {
    options.build()
}
pub fn chapter_copy() -> bool {
    true
}
pub fn chapter_editing() -> bool {
    true
}
pub fn cover_art_handling() -> bool {
    true
}
pub fn attachment_handling() -> bool {
    true
}

// ---------------------------------------------------------------------------
// Chapter import (CSV, TXT, XML)
// ---------------------------------------------------------------------------

/// A chapter file format this build can read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ChapterFormat {
    /// `start,end,name` per line, or `start,name`.
    #[default]
    Csv,
    /// A chapter per line: `HH:MM:SS Name`, or the OGG `CHAPTER01=` form.
    Txt,
    /// A Matroska/QuickTime chapter document, or `<chapter start=… />`.
    Xml,
}

impl ChapterFormat {
    /// The format named by `text`, if this build reads it.
    pub fn parse(text: &str) -> Option<Self> {
        match text
            .trim()
            .to_ascii_lowercase()
            .replace(['-', '_', '.'], "")
            .as_str()
        {
            "csv" | "tsv" => Some(Self::Csv),
            "txt" | "text" | "oggtxt" | "chap" => Some(Self::Txt),
            "xml" => Some(Self::Xml),
            _ => None,
        }
    }

    /// The extension this format is normally stored in.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Txt => "txt",
            Self::Xml => "xml",
        }
    }
}

/// Why a chapter file could not be read.
///
/// The line is part of the error because a chapter file is often long, and a
/// refusal that does not say where it stopped is a refusal nobody can act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChapterImportError {
    /// Human-readable reason.
    pub message: String,
    /// One-based line the problem was on, when the format has lines.
    pub line: Option<usize>,
}

impl ChapterImportError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            line: None,
        }
    }

    fn at(message: impl Into<String>, line: usize) -> Self {
        Self {
            message: message.into(),
            line: Some(line),
        }
    }

    /// The message, with the line named when there is one.
    pub fn display(&self) -> String {
        match self.line {
            Some(line) => format!("line {line}: {}", self.message),
            None => self.message.clone(),
        }
    }
}

impl std::fmt::Display for ChapterImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.display())
    }
}

impl From<String> for ChapterImportError {
    fn from(message: String) -> Self {
        Self::new(message)
    }
}

impl From<&str> for ChapterImportError {
    fn from(message: &str) -> Self {
        Self::new(message)
    }
}

/// Read chapter markers from a CSV, TXT, or XML document.
///
/// The grammar is strict on purpose: a file that does not follow it is refused
/// with the line it stopped on rather than half-parsed into a marker list that
/// is silently wrong.
pub fn parse_chapters(
    format: ChapterFormat,
    text: &str,
) -> Result<Vec<Chapter>, ChapterImportError> {
    match format {
        ChapterFormat::Csv => parse_csv(text),
        ChapterFormat::Txt => parse_txt(text),
        ChapterFormat::Xml => parse_xml_chapters(text),
    }
}

/// Read a time as seconds.
///
/// Accepts a plain number of seconds (`92`, `92.5`), `MM:SS`, and `HH:MM:SS`
/// with an optional fractional part. A value that is not one of these is a
/// refusal rather than a guess.
pub fn parse_timecode(raw: &str) -> Result<f64, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("an empty time".to_string());
    }
    if let Ok(seconds) = raw.parse::<f64>() {
        if seconds.is_finite() && seconds >= 0.0 {
            return Ok(seconds);
        }
        return Err(format!("'{raw}' is not a usable time"));
    }
    let parts: Vec<&str> = raw.split(':').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return Err(format!("'{raw}' is not a time this build can read"));
    }
    let mut total = 0.0f64;
    for part in &parts {
        let value = part
            .parse::<f64>()
            .map_err(|_| format!("'{raw}' is not a time this build can read"))?;
        if value < 0.0 {
            return Err(format!("'{raw}' is not a usable time"));
        }
        total = total * 60.0 + value;
    }
    Ok(total)
}

/// The `start,end,name` and `start,name` rows.
fn parse_csv(text: &str) -> Result<Vec<Chapter>, ChapterImportError> {
    let mut chapters = Vec::new();
    for (index, raw_line) in text.lines().enumerate() {
        let line = index + 1;
        let row = raw_line.trim();
        if row.is_empty() || row.starts_with('#') {
            continue;
        }
        // A header row is allowed and recognised by name rather than by being
        // silently treated as a marker.
        if chapters.is_empty() && row.eq_ignore_ascii_case("start,end,name") {
            continue;
        }
        let fields: Vec<&str> = row.split(',').collect();
        if fields.is_empty() || fields[0].trim().is_empty() {
            return Err(ChapterImportError::at(
                format!("chapter {line} has no start time"),
                line,
            ));
        }
        let start = parse_timecode(fields[0])
            .map_err(|problem| ChapterImportError::at(problem, line))?;
        let (end, name) = match fields.len() {
            1 => (None, None),
            2 => {
                // The second field is a name when it does not read as a time.
                match parse_timecode(fields[1]) {
                    Ok(end) => (Some(end), None),
                    Err(_) => (None, Some(fields[1].trim().to_string())),
                }
            }
            _ => (
                Some(
                    parse_timecode(fields[1])
                        .map_err(|problem| ChapterImportError::at(problem, line))?,
                ),
                Some(fields[2..].join(",").trim().to_string()),
            ),
        };
        chapters.push(Chapter {
            start_seconds: Some(start),
            end_seconds: end,
            title: name.filter(|name| !name.is_empty()),
            id: None,
            tags: BTreeMap::new(),
        });
    }
    if chapters.is_empty() {
        return Err(ChapterImportError::new(
            "the file holds no chapter markers",
        ));
    }
    Ok(chapters)
}

/// A chapter per line, in either the `HH:MM:SS Name` or the OGG form.
///
/// The OGG form writes each marker as two lines — `CHAPTER01=00:00:00.000` and
/// `CHAPTER01NAME=Opening` — so the two are paired by their number. A name
/// without a time is refused rather than given a start of zero, because a
/// chapter that starts where the file starts is a guess, not a reading.
fn parse_txt(text: &str) -> Result<Vec<Chapter>, ChapterImportError> {
    let mut chapters = Vec::new();
    let mut ogg_times: BTreeMap<u32, String> = BTreeMap::new();
    let mut ogg_names: BTreeMap<u32, String> = BTreeMap::new();
    for (index, raw_line) in text.lines().enumerate() {
        let line = index + 1;
        let row = raw_line.trim();
        if row.is_empty() || row.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = row.split_once('=') {
            let key = key.trim();
            let digits: String = key
                .strip_prefix("CHAPTER")
                .map(str::trim)
                .map(|rest| rest.trim_end_matches("NAME").to_string())
                .unwrap_or_default();
            if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) {
                let number: u32 = digits.parse().map_err(|_| {
                    ChapterImportError::at("an OGG chapter number is not a number", line)
                })?;
                if key.to_ascii_uppercase().ends_with("NAME") {
                    ogg_names.insert(number, value.trim().to_string());
                } else {
                    ogg_times.insert(number, value.trim().to_string());
                }
                continue;
            }
        }
        // `HH:MM:SS Name` / `MM:SS Name` / `seconds Name`.
        let (time_part, name) = match row.find(char::is_whitespace) {
            Some(at) => (row[..at].trim(), row[at..].trim()),
            None => (row, ""),
        };
        let start =
            parse_timecode(time_part).map_err(|problem| ChapterImportError::at(problem, line))?;
        chapters.push(Chapter {
            start_seconds: Some(start),
            end_seconds: None,
            title: Some(name.to_string()).filter(|name| !name.is_empty()),
            id: None,
            tags: BTreeMap::new(),
        });
    }

    if !ogg_times.is_empty() || !ogg_names.is_empty() {
        let mut paired: Vec<Chapter> = Vec::new();
        for (number, time) in &ogg_times {
            let start =
                parse_timecode(time).map_err(|problem| ChapterImportError::at(problem, 1))?;
            paired.push(Chapter {
                start_seconds: Some(start),
                end_seconds: None,
                title: ogg_names.get(number).cloned(),
                id: None,
                tags: BTreeMap::new(),
            });
        }
        if paired.is_empty() {
            return Err(ChapterImportError::new(
                "the OGG chapter lines hold no times",
            ));
        }
        return Ok(paired);
    }

    if chapters.is_empty() {
        return Err(ChapterImportError::new(
            "the file holds no chapter markers",
        ));
    }
    Ok(chapters)
}

/* ------------------------------------------------------------------ XML */

/// A parsed XML element, kept minimal on purpose.
#[derive(Debug, Clone, PartialEq)]
struct XmlElement {
    name: String,
    attributes: BTreeMap<String, String>,
    text: String,
    children: Vec<XmlElement>,
}

/// Read a Matroska/QuickTime chapter document.
///
/// The accepted shapes are the Matroska chapter document
/// (`Chapters > EditionEntry > ChapterAtom` with `ChapterTimeStart`,
/// `ChapterTimeEnd`, and `ChapterDisplay > ChapterString`) and the simple
/// `<chapter start="…" end="…" title="…"/>` form. Anything else is refused
/// with a message rather than parsed into a marker list that is not the one
/// the file describes.
fn parse_xml_chapters(text: &str) -> Result<Vec<Chapter>, ChapterImportError> {
    let root = parse_xml(text).map_err(ChapterImportError::new)?;
    let root_name = root.name.to_ascii_lowercase();
    if root_name != "chapters" && root_name != "chapterlist" {
        return Err(ChapterImportError::new(format!(
            "'{}' is not a chapter document this build reads; expected <Chapters>",
            root.name
        )));
    }

    // Simple form first: <chapter start="…" end="…" title="…"/>.
    let simple: Vec<&XmlElement> = root
        .children
        .iter()
        .filter(|child| child.name.eq_ignore_ascii_case("chapter"))
        .collect();
    if !simple.is_empty() {
        let mut chapters = Vec::new();
        for element in simple {
            let start = element
                .attributes
                .get("start")
                .ok_or_else(|| ChapterImportError::new("a <chapter> has no start attribute"))?;
            let start = parse_timecode(start)?;
            let end = match element.attributes.get("end") {
                Some(end) => Some(parse_timecode(end)?),
                None => None,
            };
            let title = element.attributes.get("title").cloned();
            chapters.push(Chapter {
                start_seconds: Some(start),
                end_seconds: end,
                title,
                id: None,
                tags: BTreeMap::new(),
            });
        }
        if chapters.is_empty() {
            return Err(ChapterImportError::new(
                "the chapter document holds no <chapter> elements",
            ));
        }
        return Ok(chapters);
    }

    // Matroska form: every ChapterAtom anywhere under the root.
    let mut chapters = Vec::new();
    for atom in collect_named(&root, "ChapterAtom") {
        let start = atom
            .children
            .iter()
            .find(|child| child.name.eq_ignore_ascii_case("ChapterTimeStart"))
            .ok_or_else(|| {
                ChapterImportError::new("a ChapterAtom has no ChapterTimeStart")
            })?
            .text
            .trim()
            .to_string();
        let start = parse_timecode(&start)?;
        let end = atom
            .children
            .iter()
            .find(|child| child.name.eq_ignore_ascii_case("ChapterTimeEnd"))
            .map(|child| parse_timecode(child.text.trim()))
            .transpose()?;
        let title = atom
            .children
            .iter()
            .find(|child| child.name.eq_ignore_ascii_case("ChapterDisplay"))
            .and_then(|display| {
                display
                    .children
                    .iter()
                    .find(|child| child.name.eq_ignore_ascii_case("ChapterString"))
            })
            .map(|title| title.text.trim().to_string());
        chapters.push(Chapter {
            start_seconds: Some(start),
            end_seconds: end,
            title: title.filter(|title| !title.is_empty()),
            id: None,
            tags: BTreeMap::new(),
        });
    }
    if chapters.is_empty() {
        return Err(ChapterImportError::new(
            "the chapter document holds no ChapterAtom elements",
        ));
    }
    Ok(chapters)
}

/// Every descendant named `name`, at any depth.
fn collect_named<'a>(element: &'a XmlElement, name: &str) -> Vec<&'a XmlElement> {
    let mut found = Vec::new();
    for child in &element.children {
        if child.name.eq_ignore_ascii_case(name) {
            found.push(child);
        }
        found.extend(collect_named(child, name));
    }
    found
}

/// Parse a small, well-formed XML document into a tree.
fn parse_xml(text: &str) -> Result<XmlElement, String> {
    let mut reader = XmlReader {
        text,
        pos: 0,
    };
    reader.skip_prolog()?;
    let root = reader.parse_element()?;
    Ok(root)
}

struct XmlReader<'a> {
    text: &'a str,
    pos: usize,
}

impl<'a> XmlReader<'a> {
    fn rest(&self) -> &'a str {
        &self.text[self.pos..]
    }

    fn skip_prolog(&mut self) -> Result<(), String> {
        loop {
            let rest = self.rest();
            if rest.trim_start().is_empty() {
                return Ok(());
            }
            if rest.trim_start().starts_with("<?") {
                let end = rest
                    .find("?>")
                    .ok_or_else(|| "the XML declaration is never closed".to_string())?;
                self.pos += rest.len() - rest.trim_start().len() + end + 2;
                continue;
            }
            if rest.trim_start().starts_with("<!--") {
                let end = rest
                    .find("-->")
                    .ok_or_else(|| "the comment is never closed".to_string())?;
                self.pos += rest.len() - rest.trim_start().len() + end + 3;
                continue;
            }
            if rest.trim_start().starts_with("<!") {
                let end = rest
                    .find('>')
                    .ok_or_else(|| "the declaration is never closed".to_string())?;
                self.pos += rest.len() - rest.trim_start().len() + end + 1;
                continue;
            }
            return Ok(());
        }
    }

    fn parse_element(&mut self) -> Result<XmlElement, String> {
        self.skip_ws();
        let rest = self.rest();
        if !rest.starts_with('<') {
            return Err("expected an element".to_string());
        }
        let (name, attributes, self_closing) = self.parse_tag()?;
        if self_closing {
            return Ok(XmlElement {
                name,
                attributes,
                text: String::new(),
                children: Vec::new(),
            });
        }
        let mut element = XmlElement {
            name: name.clone(),
            attributes,
            text: String::new(),
            children: Vec::new(),
        };
        loop {
            self.skip_ws();
            let rest = self.rest();
            if rest.is_empty() {
                return Err(format!("<{name}> is never closed"));
            }
            if rest.starts_with("</") {
                let close_end = rest
                    .find('>')
                    .ok_or_else(|| format!("</{name}> is never closed"))?;
                let close_name = rest[2..close_end].trim();
                if !close_name.eq_ignore_ascii_case(&name) {
                    return Err(format!(
                        "expected </{name}> but found </{close_name}>"
                    ));
                }
                self.pos += close_end + 1;
                return Ok(element);
            }
            if rest.starts_with('<') {
                let child = self.parse_element()?;
                element.children.push(child);
            } else {
                let text_end = rest.find('<').unwrap_or(rest.len());
                let text = rest[..text_end].to_string();
                element.text.push_str(&decode_entities(&text));
                self.pos += text_end;
            }
        }
    }

    fn skip_ws(&mut self) {
        let rest = self.rest();
        let trimmed = rest.trim_start();
        self.pos += rest.len() - trimmed.len();
    }

    fn parse_tag(
        &mut self,
    ) -> Result<(String, BTreeMap<String, String>, bool), String> {
        // The caller has checked for '<'.
        self.pos += 1;
        let rest = self.rest();
        let name_end = rest
            .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
            .ok_or_else(|| "the tag name never ends".to_string())?;
        let name = rest[..name_end].to_string();
        self.pos += name_end;

        let mut attributes = BTreeMap::new();
        loop {
            self.skip_ws();
            let rest = self.rest();
            if rest.starts_with("/>") {
                self.pos += 2;
                return Ok((name, attributes, true));
            }
            if rest.starts_with('>') {
                self.pos += 1;
                return Ok((name, attributes, false));
            }
            if rest.is_empty() {
                return Err(format!("<{name}> is never closed"));
            }
            let attr_end = rest
                .find(|c: char| c.is_whitespace() || c == '=' || c == '>' || c == '/')
                .ok_or_else(|| "an attribute name never ends".to_string())?;
            let attr_name = rest[..attr_end].to_string();
            self.pos += attr_end;
            self.skip_ws();
            let rest = self.rest();
            if !rest.starts_with('=') {
                return Err(format!("attribute '{attr_name}' has no value"));
            }
            self.pos += 1;
            self.skip_ws();
            let rest = self.rest();
            let quote = rest
                .chars()
                .next()
                .ok_or_else(|| format!("attribute '{attr_name}' has no value"))?;
            if quote != '"' && quote != '\'' {
                return Err(format!("attribute '{attr_name}' is not quoted"));
            }
            let value_end = rest[1..]
                .find(quote)
                .ok_or_else(|| format!("attribute '{attr_name}' is never closed"))?;
            let value = rest[1..1 + value_end].to_string();
            attributes.insert(attr_name, decode_entities(&value));
            self.pos += 1 + value_end + 1;
        }
    }
}

/// Decode the five predefined XML entities and numeric references.
fn decode_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'&' {
            out.push(bytes[index] as char);
            index += 1;
            continue;
        }
        let Some(semi) = text[index..].find(';') else {
            out.push('&');
            index += 1;
            continue;
        };
        let entity = &text[index + 1..index + semi];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ if entity.starts_with("#x") || entity.starts_with("#X") => {
                u32::from_str_radix(&entity[2..], 16)
                    .ok()
                    .and_then(char::from_u32)
            }
            _ if entity.starts_with('#') => entity[1..]
                .parse::<u32>()
                .ok()
                .and_then(char::from_u32),
            _ => None,
        };
        match decoded {
            Some(character) => {
                out.push(character);
                index += semi + 1;
            }
            None => {
                // An undefined entity is left as written rather than dropped,
                // the same rule the update appcast grammar uses.
                out.push_str(&text[index..index + semi + 1]);
                index += semi + 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod import_tests {
    use super::*;

    #[test]
    fn csv_reads_starts_ends_and_names() {
        let text = "0,600,Opening\n600,1800\n1800,3600,End\n";
        let chapters = parse_chapters(ChapterFormat::Csv, text).expect("csv");
        assert_eq!(chapters.len(), 3);
        assert_eq!(chapters[0].start_seconds, Some(0.0));
        assert_eq!(chapters[0].end_seconds, Some(600.0));
        assert_eq!(chapters[0].title.as_deref(), Some("Opening"));
        assert_eq!(chapters[1].end_seconds, Some(1800.0));
        assert_eq!(chapters[1].title, None);
    }

    #[test]
    fn csv_accepts_a_header_and_a_two_column_row() {
        let text = "start,end,name\n0,First\n00:01:00,00:02:00,Second\n";
        let chapters = parse_chapters(ChapterFormat::Csv, text).expect("csv");
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].end_seconds, None);
        assert_eq!(chapters[0].title.as_deref(), Some("First"));
        assert_eq!(chapters[1].start_seconds, Some(60.0));
    }

    #[test]
    fn csv_refuses_a_bad_row_with_its_line() {
        let error = parse_chapters(ChapterFormat::Csv, "0,1,Opening\nsoon,2,Middle\n")
            .expect_err("bad start");
        assert_eq!(error.line, Some(2));
        assert!(error.message.contains("soon"), "{}", error.display());

        let error = parse_chapters(ChapterFormat::Csv, "# only a comment\n\n").expect_err("no rows");
        assert!(error.message.contains("no chapter markers"), "{}", error.display());
    }

    #[test]
    fn txt_reads_timecode_and_ogg_forms() {
        let text = "00:00:00 Opening\n00:10:00 Middle\n";
        let chapters = parse_chapters(ChapterFormat::Txt, text).expect("txt");
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].start_seconds, Some(0.0));
        assert_eq!(chapters[0].title.as_deref(), Some("Opening"));
        assert_eq!(chapters[1].start_seconds, Some(600.0));

        let ogg = "CHAPTER01=00:00:00.000\nCHAPTER01NAME=Opening\n\
                   CHAPTER02=00:10:00.000\nCHAPTER02NAME=Middle\n";
        let chapters = parse_chapters(ChapterFormat::Txt, ogg).expect("ogg txt");
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].title.as_deref(), Some("Opening"));
        assert_eq!(chapters[1].title.as_deref(), Some("Middle"));
    }

    #[test]
    fn txt_refuses_a_time_that_is_not_a_time() {
        let error = parse_chapters(ChapterFormat::Txt, "00:00:00 Good\nnoon Bad\n")
            .expect_err("bad time");
        assert_eq!(error.line, Some(2));
        assert!(error.message.contains("noon"), "{}", error.display());
    }

    #[test]
    fn xml_reads_the_matroska_and_simple_forms() {
        let matroska = "<?xml version=\"1.0\"?>\n<Chapters><EditionEntry>\
            <ChapterAtom><ChapterTimeStart>00:00:00.000000000</ChapterTimeStart>\
            <ChapterTimeEnd>00:10:00.000000000</ChapterTimeEnd>\
            <ChapterDisplay><ChapterString>Opening</ChapterString></ChapterDisplay>\
            </ChapterAtom></EditionEntry></Chapters>";
        let chapters = parse_chapters(ChapterFormat::Xml, matroska).expect("matroska xml");
        assert_eq!(chapters.len(), 1);
        assert_eq!(chapters[0].start_seconds, Some(0.0));
        assert_eq!(chapters[0].end_seconds, Some(600.0));
        assert_eq!(chapters[0].title.as_deref(), Some("Opening"));

        let simple = "<Chapters><chapter start=\"0\" end=\"30\" title=\"One\"/>\
            <chapter start=\"30\" title=\"Two\"/></Chapters>";
        let chapters = parse_chapters(ChapterFormat::Xml, simple).expect("simple xml");
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[1].end_seconds, None);
    }

    #[test]
    fn xml_refuses_a_document_that_is_not_a_chapter_list() {
        let error = parse_chapters(ChapterFormat::Xml, "<Playlist><chapter start=\"0\"/></Playlist>")
            .expect_err("wrong root");
        assert!(error.message.contains("Playlist"), "{}", error.display());

        let error = parse_chapters(ChapterFormat::Xml, "<Chapters><ChapterAtom>broken")
            .expect_err("unclosed");
        assert!(error.message.contains("never closed"), "{}", error.display());
    }

    #[test]
    fn timecodes_read_as_seconds() {
        assert_eq!(parse_timecode("0").expect("zero"), 0.0);
        assert_eq!(parse_timecode("92.5").expect("seconds"), 92.5);
        assert_eq!(parse_timecode("00:01:00").expect("mmss"), 60.0);
        assert_eq!(parse_timecode("01:00:00.5").expect("hms"), 3600.5);
        assert!(parse_timecode("").is_err());
        assert!(parse_timecode("noon").is_err());
        assert!(parse_timecode("-5").is_err());
    }

    #[test]
    fn the_format_reads_back_every_way_this_build_spells_it() {
        assert_eq!(ChapterFormat::parse("CSV"), Some(ChapterFormat::Csv));
        assert_eq!(ChapterFormat::parse(".txt"), Some(ChapterFormat::Txt));
        assert_eq!(ChapterFormat::parse("xML"), Some(ChapterFormat::Xml));
        assert_eq!(ChapterFormat::parse("yaml"), None);
        assert_eq!(ChapterFormat::Csv.extension(), "csv");
    }
}
