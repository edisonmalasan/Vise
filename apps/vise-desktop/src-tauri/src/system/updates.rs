//! Looking for a newer release, and refusing to install one.
//!
//! Three things have to be true before a build can tell the user that an update
//! exists, and they are deliberately in this order:
//!
//! 1. **The feed says so.** An appcast is XML fetched from a server, which makes it
//!    untrusted input in the same way a file name from a disc is: it is parsed by
//!    hand, against a small fixed grammar, with no document type, no entities
//!    beyond the five that XML defines for text, no external references of any
//!    kind, and a size limit. An appcast that tries to be clever is refused rather
//!    than half-understood.
//! 2. **The artifact is the one the feed described.** Every item carries a
//!    SHA-256 of the file, and the hash is recomputed over the bytes that arrived
//!    before anything is offered. A mismatch is not a warning: it means the bytes
//!    are not the ones that were described, and nothing is offered.
//! 3. **The user does something about it.** Nothing is downloaded without being
//!    asked, and nothing is installed by this build at all — the availability, the
//!    version, and the feed it came from are shown, and the decision is the user's.
//!
//! The schedule comes from the Updates preferences that already exist, so there is
//! one answer to "how often" rather than two. Whether a check happens at all needs
//! a feed to check, and a build with no feed configured says so instead of
//! contacting somewhere on the user's behalf.

use std::fmt;
use std::ops::Range;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::preferences::updates::CheckFrequency;

/// Most of an appcast this build will read.
///
/// An appcast is a page of release notes; anything larger than this is a file
/// being passed off as one, and reading it would be work no user asked for.
pub const MAX_APPCAST_BYTES: usize = 2 * 1024 * 1024;

/// Longest release notes this build will show.
const MAX_NOTES: usize = 8_000;

/// URL schemes an update may be fetched from.
///
/// Only HTTPS: an appcast that offers a `file://` path is asking this build to
/// read a file on the user's disk because a server said so, and one that offers
/// plain HTTP has not been checked by anybody between here and the file.
const ALLOWED_SCHEMES: [&str; 1] = ["https"];

/// Why an appcast could not be read, or could not be trusted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppcastError {
    /// The feed was larger than this build reads.
    TooLarge { size: usize },
    /// The feed tried to declare a document type or an entity.
    DeclaresEntities,
    /// The feed was not XML this build understands.
    Malformed { reason: String },
    /// The feed offered an item with no version.
    NoVersion,
    /// The feed offered an artifact over a scheme that is not allowed.
    RefusedUrl { url: String },
    /// The feed offered a hash that is not a SHA-256.
    RefusedHash { hash: String },
    /// The bytes that arrived are not the ones the feed described.
    #[allow(
        dead_code,
        reason = "raised by the verification this build offers but does not download"
    )]
    HashMismatch { expected: String, actual: String },
    /// The feed could not be fetched.
    Unavailable { reason: String },
}

impl fmt::Display for AppcastError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppcastError::TooLarge { size } => write!(
                f,
                "the release feed is {size} bytes, which is larger than the {MAX_APPCAST_BYTES} this build reads"
            ),
            AppcastError::DeclaresEntities => write!(
                f,
                "the release feed declares a document type or an entity, which this build does not read"
            ),
            AppcastError::Malformed { reason } => {
                write!(f, "the release feed could not be read: {reason}")
            }
            AppcastError::NoVersion => {
                write!(f, "the release feed offers a release with no version")
            }
            AppcastError::RefusedUrl { url } => write!(
                f,
                "the release feed offers '{url}', which is not an https address"
            ),
            AppcastError::RefusedHash { hash } => write!(
                f,
                "the release feed offers the hash '{hash}', which is not a SHA-256"
            ),
            AppcastError::HashMismatch { expected, actual } => write!(
                f,
                "the downloaded file does not match the release feed: it should have been {expected} and was {actual}, so it will not be used"
            ),
            AppcastError::Unavailable { reason } => {
                write!(f, "the release feed could not be fetched: {reason}")
            }
        }
    }
}

impl std::error::Error for AppcastError {}

/// One release the feed offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppcastItem {
    /// The version, as `major.minor.patch`.
    pub version: String,
    /// The line at the top of the notes.
    pub title: String,
    /// The notes, as the feed wrote them.
    pub notes: String,
    /// Which channel the release belongs to.
    pub channel: String,
    /// Where the artifact is.
    pub url: String,
    /// How many bytes it should be, when the feed says.
    pub length: Option<u64>,
    /// The SHA-256 of the artifact, when the feed gives one.
    pub sha256: Option<String>,
    /// When it was published, as the feed wrote it.
    pub published: String,
}

impl AppcastItem {
    /// Whether this release belongs to `channel`.
    pub fn on_channel(&self, channel: &str) -> bool {
        // A feed item with no channel is on every channel: the feed did not say
        // otherwise, and refusing to offer it would be guessing.
        self.channel.trim().is_empty() || self.channel.eq_ignore_ascii_case(channel)
    }

    /// The version as three numbers, so two versions can be compared.
    pub fn version_numbers(&self) -> (u64, u64, u64) {
        parse_version(&self.version)
    }
}

/// A whole feed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Appcast {
    /// The feed's own title.
    pub title: String,
    /// The releases it offers, in the order it lists them.
    pub items: Vec<AppcastItem>,
}

impl Appcast {
    /// The newest release on `channel`, if the feed offers one.
    pub fn newest_on(&self, channel: &str) -> Option<&AppcastItem> {
        self.items
            .iter()
            .filter(|item| item.on_channel(channel))
            .max_by_key(|item| item.version_numbers())
    }
}

/// Read `text` as an appcast.
///
/// The grammar is the small one: a `<channel>` holding `<item>` elements, each
/// with the fields below. Names are compared without their namespace prefix, so a
/// feed written with `vise:` on its fields is read the same as one without.
pub fn parse(text: &str) -> Result<Appcast, AppcastError> {
    if text.len() > MAX_APPCAST_BYTES {
        return Err(AppcastError::TooLarge { size: text.len() });
    }
    // No document type, no entities: an appcast that needs either is asking this
    // build to resolve something on its behalf, and the only answer to that is not
    // to read it.
    let lowered = text.to_ascii_lowercase();
    if lowered.contains("<!doctype") || lowered.contains("<!entity") {
        return Err(AppcastError::DeclaresEntities);
    }
    if lowered.contains("<!") {
        return Err(AppcastError::Malformed {
            reason: "the feed contains a declaration this build does not read".to_string(),
        });
    }
    let title = element(&lowered, "channel")
        .and_then(|channel| child_text(text, &lowered, &channel, "title"))
        .unwrap_or_default();
    let mut items = Vec::new();
    for tag in tags(&lowered, "item") {
        if !tag.opens {
            // An item written as one tag carries no version, so there is nothing
            // to offer even if it did.
            continue;
        }
        let Some(block) = element_from(&lowered, "item", tag.start) else {
            continue;
        };
        items.push(parse_item(text, &lowered, &block)?);
    }
    if items.is_empty() {
        return Err(AppcastError::Malformed {
            reason: "the feed lists no releases".to_string(),
        });
    }
    Ok(Appcast { title, items })
}

/// The whole of the element named `name` that begins at `start`.
fn element_from(lowered: &str, name: &str, start: usize) -> Option<Range<usize>> {
    let opening = lowered[start..].find('>')? + start;
    let inner = &lowered[start + 1 + name.len()..opening];
    if inner.ends_with('/') {
        return Some(start..opening + 1);
    }
    let closing = format!("</{name}>");
    let close = lowered[opening + 1..].find(&closing)? + opening + 1;
    Some(start..close + closing.len())
}
fn parse_item(
    text: &str,
    lowered: &str,
    block: &Range<usize>,
) -> Result<AppcastItem, AppcastError> {
    let field = |name: &str| child_text(text, lowered, block, name);
    let attribute_of = |name: &str| attribute(text, lowered, block, name);
    let version = field("version")
        .or_else(|| attribute_of("version"))
        .ok_or(AppcastError::NoVersion)?;
    let url = field("url")
        .or_else(|| attribute_of("url"))
        .ok_or_else(|| AppcastError::Malformed {
            reason: "a release has no address".to_string(),
        })?;
    if !is_allowed_url(&url) {
        return Err(AppcastError::RefusedUrl { url });
    }
    let sha256 = field("sha256")
        .or_else(|| attribute_of("sha256"))
        .map(|hash| hash.trim().to_string())
        .filter(|hash| !hash.is_empty());
    if let Some(hash) = &sha256 {
        if !is_sha256(hash) {
            return Err(AppcastError::RefusedHash { hash: hash.clone() });
        }
    }
    let length = field("length")
        .or_else(|| attribute_of("length"))
        .and_then(|value| value.trim().parse::<u64>().ok());
    let notes = field("notes").unwrap_or_default();
    Ok(AppcastItem {
        version: version.trim().to_string(),
        title: field("title").unwrap_or_default(),
        notes: notes.chars().take(MAX_NOTES).collect(),
        channel: field("channel").unwrap_or_default(),
        url,
        length,
        sha256,
        published: field("published").unwrap_or_default(),
    })
}
/// Whether a URL is one this build will fetch an artifact from.
pub fn is_allowed_url(url: &str) -> bool {
    let trimmed = url.trim();
    let Some((scheme, rest)) = trimmed.split_once("://") else {
        return false;
    };
    ALLOWED_SCHEMES
        .iter()
        .any(|allowed| scheme.eq_ignore_ascii_case(allowed))
        && !rest.is_empty()
}

/// Whether a string is a SHA-256 as this build writes it: 64 hexadecimal digits.
pub fn is_sha256(hash: &str) -> bool {
    hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit())
}

/// Verify that `bytes` are the artifact `item` described.
///
/// A feed that gives no hash has said nothing about what it is offering, so there
/// is nothing to verify and the item is refused: a download nobody can check is a
/// download this build does not hand to anybody.
#[allow(
    dead_code,
    reason = "nothing downloads in this build; the check is the gate a downloader would pass through"
)]
pub fn verify(item: &AppcastItem, bytes: &[u8]) -> Result<(), AppcastError> {
    let Some(expected) = item.sha256.as_deref() else {
        return Err(AppcastError::RefusedHash {
            hash: String::new(),
        });
    };
    let actual = sha256_hex(bytes);
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(AppcastError::HashMismatch {
            expected: expected.to_ascii_lowercase(),
            actual,
        })
    }
}

/// What came of a check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckOutcome {
    /// Nothing was checked because no feed is configured.
    NotConfigured,
    /// The feed was read and the build is the newest one it offers.
    UpToDate {
        /// The newest version the feed offers.
        newest: String,
    },
    /// The feed offers something newer.
    Available {
        /// The version it offers.
        version: String,
        /// The line the feed gives for it.
        title: String,
        /// Where the artifact is.
        url: String,
    },
    /// The check did not finish.
    Failed {
        /// What went wrong, in one line.
        reason: String,
    },
}

impl CheckOutcome {
    /// Whether a newer release is on offer.
    pub fn available(&self) -> bool {
        matches!(self, CheckOutcome::Available { .. })
    }

    /// The version, when there is one to name.
    pub fn version(&self) -> Option<String> {
        match self {
            CheckOutcome::Available { version, .. } => Some(version.clone()),
            CheckOutcome::UpToDate { newest } => Some(newest.clone()),
            _ => None,
        }
    }

    /// One line saying what happened.
    pub fn summary(&self) -> String {
        match self {
            CheckOutcome::NotConfigured => {
                "no release feed is configured, so nothing was looked for.".to_string()
            }
            CheckOutcome::UpToDate { newest } => {
                format!("The feed's newest build is {newest}, which is this one.")
            }
            CheckOutcome::Available { version, title, .. } => {
                let what = if title.trim().is_empty() {
                    format!("version {version}")
                } else {
                    format!("version {version}, {title}")
                };
                format!("{what} is on offer; nothing has been downloaded or installed.")
            }
            CheckOutcome::Failed { reason } => {
                format!("The check did not finish: {reason}.")
            }
        }
    }
}

/// What the window knows about the check, for the preferences tab and the tray.
///
/// Derived rather than written out: a window that has not looked for a release has
/// no outcome, no version, and nothing to say, and the defaults say exactly that.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct UpdateState {
    /// What the last check found.
    pub outcome: Option<String>,
    /// What it says in one line.
    pub summary: String,
    /// Whether a newer release is on offer.
    pub available: bool,
    /// The version on offer, when there is one.
    pub version: String,
    /// Whether a feed is configured at all.
    pub configured: bool,
    /// The date of the last check, as `(year, month, day)`.
    ///
    /// Held rather than remembered by the window, because the schedule is a question
    /// about this state and about the preference, and both live here.
    pub checked_on: Option<(u32, u32, u32)>,
}

impl UpdateState {
    /// A window that has not looked for anything.
    pub fn new() -> UpdateState {
        UpdateState::default()
    }

    /// Record what a check found, and when it was looked for.
    pub fn observe(&mut self, outcome: CheckOutcome, checked_on: (u32, u32, u32)) {
        self.checked_on = Some(checked_on);
        self.available = outcome.available();
        self.version = outcome.version().unwrap_or_default();
        self.summary = outcome.summary();
        self.outcome = Some(
            match &outcome {
                CheckOutcome::NotConfigured => "notConfigured",
                CheckOutcome::UpToDate { .. } => "upToDate",
                CheckOutcome::Available { .. } => "available",
                CheckOutcome::Failed { .. } => "failed",
            }
            .to_string(),
        );
    }

    /// Whether a check is due now, from the preference and the last one.
    ///
    /// The answer the window asks before it reaches for the network, so a frequency
    /// of weekly is a fact rather than a setting nobody reads.
    pub fn due(&self, frequency: CheckFrequency, today: (u32, u32, u32)) -> bool {
        is_due(frequency, self.checked_on, today)
    }
}

/// Read a feed and say what it offers `channel` against `installed`.
///
/// The one call the window makes, with the transport this build has. A feed that
/// cannot be read, or an artifact whose bytes do not match its hash, is a failed
/// check rather than an offer: the only two answers this build will act on are
/// "there is something newer" and "there is not".
pub fn check(feed: &str, channel: &str, installed: &str) -> CheckOutcome {
    let feed = feed.trim();
    if feed.is_empty() {
        return CheckOutcome::NotConfigured;
    }
    let source = CurlSource::default();
    let text = match source.fetch(feed) {
        Ok(text) => text,
        Err(e) => {
            return CheckOutcome::Failed {
                reason: one_line(&e.to_string()),
            }
        }
    };
    let appcast = match parse(&text) {
        Ok(appcast) => appcast,
        Err(e) => {
            return CheckOutcome::Failed {
                reason: one_line(&e.to_string()),
            }
        }
    };
    let Some(newest) = appcast.newest_on(channel) else {
        return CheckOutcome::Failed {
            reason: format!("the feed offers nothing on the {channel} channel"),
        };
    };
    if !is_newer(&newest.version, installed) {
        return CheckOutcome::UpToDate {
            newest: newest.version.clone(),
        };
    }
    CheckOutcome::Available {
        version: newest.version.clone(),
        title: newest.title.clone(),
        url: newest.url.clone(),
    }
}

/// Whether an item's hash still matches the bytes that arrived.
///
/// Exposed on its own so that a build which downloads an artifact later can
/// verify it with the same call the offer was decided with.
#[allow(
    dead_code,
    reason = "named for the download this build does not perform"
)]
pub fn verify_download(item: &AppcastItem, bytes: &[u8]) -> Result<(), AppcastError> {
    verify(item, bytes)
}

/// A message squeezed onto one line, for a summary that has to fit.
fn one_line(reason: &str) -> String {
    reason
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<&str>>()
        .join(" ")
}

/// Whether a check is due, given the preference and when the last one happened.
///
/// A check that has never happened is due whenever the frequency is anything but
/// never: the user asked to be told about releases, and the first thing to tell
/// them is what the newest one is.
pub fn is_due(
    frequency: CheckFrequency,
    last_checked: Option<(u32, u32, u32)>,
    today: (u32, u32, u32),
) -> bool {
    let Some(days) = frequency.days() else {
        return false;
    };
    match last_checked {
        // A check that has never happened is due whenever the frequency is anything
        // but never: the user asked to be told about releases, and the first thing
        // to tell them is what the newest one is.
        None => true,
        // The next check falls from the last one rather than from today, so a check
        // made today is not due again today however short the interval is.
        Some(last) => {
            let next = crate::preferences::updates::add_days(last, i64::from(days));
            (today.0, today.1, today.2) >= next
        }
    }
}

/// Whether `candidate` is newer than `installed`.
///
/// Numeric only, and deliberately so: a build called `0.2.0-beta` is not newer
/// than `0.2.0`, because this build does not know what the suffix means and
/// offering it would be a guess about somebody's release process.
pub fn is_newer(candidate: &str, installed: &str) -> bool {
    parse_version(candidate) > parse_version(installed)
}

/// A version as three numbers, ignoring anything after the third.
fn parse_version(version: &str) -> (u64, u64, u64) {
    let mut numbers = [0u64; 3];
    for (index, part) in version.trim().split('.').take(3).enumerate() {
        numbers[index] = part
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .unwrap_or(0);
    }
    (numbers[0], numbers[1], numbers[2])
}

/// Something that can fetch an appcast.
pub trait AppcastSource: Send + Sync {
    /// Whether this build can fetch anything at all here.
    fn available(&self) -> bool;
    /// One line saying what it is, for the log.
    fn describe(&self) -> String;
    /// Fetch `url` and hand back what it said.
    fn fetch(&self, url: &str) -> Result<String, AppcastError>;
}

/// A source that fetches nothing.
///
/// What a build with no transport answers, so that the rest of the checker can be
/// exercised — and so that the button on the preferences tab can say what is
/// missing rather than failing when it is pressed.
#[derive(Debug, Clone, Copy, Default)]
#[allow(
    dead_code,
    reason = "the answer for a build with no network tool, exercised by tests"
)]
pub struct NoSource;

impl AppcastSource for NoSource {
    fn available(&self) -> bool {
        false
    }

    fn describe(&self) -> String {
        "this build has no way to fetch a release feed over the network".to_string()
    }

    fn fetch(&self, _url: &str) -> Result<String, AppcastError> {
        Err(AppcastError::Unavailable {
            reason: "this build has no way to fetch a release feed over the network".to_string(),
        })
    }
}

/// Fetch a feed with the network tool the platform ships.
///
/// The arguments are a vector and the tool is started directly, so a feed address
/// is never part of a command line: the address is one argument, after the options
/// that refuse everything the tool would otherwise do on its own — no redirects to
/// another scheme, no credentials from a file, no cookie jar, no progress meter.
pub struct CurlSource {
    program: String,
}

impl Default for CurlSource {
    fn default() -> Self {
        CurlSource {
            program: "curl.exe".to_string(),
        }
    }
}

impl CurlSource {
    /// A source that runs `program`.
    ///
    /// The program is named rather than found, so a machine whose network tool is
    /// called something else is one constructor call away rather than a new build.
    #[allow(
        dead_code,
        reason = "this build always fetches with the platform's own curl"
    )]
    pub fn new(program: impl Into<String>) -> CurlSource {
        CurlSource {
            program: program.into(),
        }
    }

    /// The arguments a fetch is made with.
    pub fn arguments(url: &str) -> Vec<String> {
        vec![
            "--silent".to_string(),
            "--show-error".to_string(),
            "--fail".to_string(),
            "--location".to_string(),
            "--max-time".to_string(),
            "20".to_string(),
            "--proto".to_string(),
            "=https".to_string(),
            "--no-progress-meter".to_string(),
            "--user-agent".to_string(),
            format!("ViseTranscoder/{}", vise_core::VISE_VERSION),
            "--".to_string(),
            url.to_string(),
        ]
    }

    /// Whether the tool this build would run is on the machine.
    pub fn installed(&self) -> bool {
        which(&self.program).is_some()
    }
}

impl AppcastSource for CurlSource {
    fn available(&self) -> bool {
        self.installed()
    }

    fn describe(&self) -> String {
        format!("release feeds are fetched with {}", self.program)
    }

    fn fetch(&self, url: &str) -> Result<String, AppcastError> {
        if !is_allowed_url(url) {
            return Err(AppcastError::RefusedUrl {
                url: url.to_string(),
            });
        }
        if !self.installed() {
            return Err(AppcastError::Unavailable {
                reason: format!("{} is not installed on this machine", self.program),
            });
        }
        let output = Command::new(&self.program)
            .args(CurlSource::arguments(url))
            .stdin(Stdio::null())
            .output()
            .map_err(|e| AppcastError::Unavailable {
                reason: e.to_string(),
            })?;
        if !output.status.success() {
            return Err(AppcastError::Unavailable {
                reason: String::from_utf8_lossy(&output.stderr)
                    .lines()
                    .next()
                    .unwrap_or("the fetch did not succeed")
                    .to_string(),
            });
        }
        if output.stdout.len() > MAX_APPCAST_BYTES {
            return Err(AppcastError::TooLarge {
                size: output.stdout.len(),
            });
        }
        String::from_utf8(output.stdout).map_err(|e| AppcastError::Unavailable {
            reason: format!("the feed was not text: {e}"),
        })
    }
}

/// Find `program` on the path.
///
/// A directory walk with the platform's own separator, so no shell is involved and
/// nothing in a folder name is ever interpreted.
pub fn which(program: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    let separator = if cfg!(windows) { ';' } else { ':' };
    for folder in std::env::split_paths(&path) {
        let candidate = folder.join(program);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    // Kept for the two platforms whose separator is not in the list above, and for
    // a path variable written by hand with the wrong separator.
    let _ = separator;
    None
}

/// SHA-256, as FIPS 180-4 defines it.
///
/// Written out rather than brought in so that verifying a download costs no
/// dependency in a build whose whole point is to be self-contained; the two are
/// checked against the published vectors below, which is what makes it safe to
/// offer a download on the strength of it.
pub fn sha256_hex(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    let mut message = bytes.to_vec();
    let bit_length = (bytes.len() as u64) * 8;
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_length.to_be_bytes());

    for block in message.chunks(64) {
        let mut w = [0u32; 64];
        for (index, word) in block.chunks(4).enumerate() {
            w[index] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for index in 16..64 {
            let s0 = w[index - 15].rotate_right(7)
                ^ w[index - 15].rotate_right(18)
                ^ (w[index - 15] >> 3);
            let s1 = w[index - 2].rotate_right(17)
                ^ w[index - 2].rotate_right(19)
                ^ (w[index - 2] >> 10);
            w[index] = w[index - 16]
                .wrapping_add(s0)
                .wrapping_add(w[index - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[index])
                .wrapping_add(w[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        for (slot, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(value);
        }
    }
    state
        .iter()
        .map(|word| format!("{word:08x}"))
        .collect::<Vec<String>>()
        .join("")
}

/// One tag found in the text: where it is, how long it is, and what it names.
///
/// The name is kept as the feed wrote it, prefix and all, because the closing tag
/// has to be matched against the same name the opening tag used.
struct Tag {
    /// Byte offset of the `<`.
    start: usize,
    /// Byte offset just past the `>`.
    end: usize,
    /// Whether this tag opens an element, or opens and closes it at once.
    opens: bool,
    /// The element name, without its namespace prefix.
    name: String,
    /// The name as the feed wrote it, which is what the closing tag carries.
    qualified: String,
}

/// Every opening tag of `name` in the lowered text.
///
/// Names are compared without their namespace prefix, so a feed that writes
/// `vise:version` is read the same as one that writes `version`: the prefix says
/// whose field it is, not what it means, and this build does not care whose.
///
/// The text is scanned rather than parsed into a tree: the grammar is small enough
/// that a tree would be more code than the grammar, and the only nesting read is
/// one level, so a feed cannot nest its way into anything unexpected. A closing tag
/// is never matched, because `</item>` does not contain `<item`.
fn tags(lowered: &str, name: &str) -> Vec<Tag> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(offset) = lowered[from..].find('<') {
        let start = from + offset;
        from = start + 1;
        if lowered[start..].starts_with("!--") || lowered[start..].starts_with('?') {
            // A comment or a declaration is not an element, and skipping it here
            // means the caller never has to think about one.
            continue;
        }
        let Some(length) = lowered[start..].find('>') else {
            break;
        };
        let end = start + length + 1;
        let inner = &lowered[start + 1..end - 1];
        let qualified: String = inner
            .chars()
            .take_while(|c| !c.is_whitespace() && *c != '/' && *c != '>')
            .collect();
        let bare = qualified.rsplit(':').next().unwrap_or(&qualified);
        if bare != name {
            continue;
        }
        found.push(Tag {
            start,
            end,
            opens: !inner.trim_end().ends_with('/'),
            name: bare.to_string(),
            qualified,
        });
    }
    found
}

/// The whole of the first element named `name`, as a range in the original text.
fn element(lowered: &str, name: &str) -> Option<Range<usize>> {
    tags(lowered, name)
        .into_iter()
        .find_map(|tag| element_from_tag(lowered, tag))
}

/// The whole of the element the tag opens.
fn element_from_tag(lowered: &str, tag: Tag) -> Option<Range<usize>> {
    if !tag.opens {
        // An element that opens and closes at once has nothing inside it.
        return Some(tag.start..tag.end);
    }
    let closing = format!("</{}>", tag.qualified);
    let close = lowered[tag.end..].find(&closing)? + tag.end;
    let _ = &tag.name;
    Some(tag.start..close + closing.len())
}

/// The text of the first child of `block` named `name`, with XML's five entities
/// resolved and nothing else.
///
/// The search starts inside the block and stops at its end, so a field is read
/// from the item being read rather than from whichever item it happens to sit in.
fn child_text(text: &str, lowered: &str, block: &Range<usize>, name: &str) -> Option<String> {
    let inner = lowered.get(block.start..block.end)?;
    let tag = tags(inner, name).into_iter().find(|tag| tag.opens)?;
    let open_end = block.start + tag.end;
    let closing = format!("</{}>", tag.qualified);
    let close = lowered[open_end..block.end].find(&closing)? + open_end;
    // The offsets were found in the lowered copy and are used against the original:
    // lowercasing maps ASCII letters one to one and changes no byte count, so the
    // two agree exactly, and the notes the user reads keep their own capitals.
    let value = text.get(open_end..close)?;
    Some(unescape(value.trim()))
}

/// The value of `name` as a double-quoted attribute of the element `block` opens.
///
/// Only the opening tag is looked in, and only a double-quoted value is read: a
/// feed that writes an unquoted attribute is a feed this parser does not have to
/// understand, and guessing at it would be the whole of the parsing problem.
fn attribute(text: &str, lowered: &str, block: &Range<usize>, name: &str) -> Option<String> {
    let window = block.end.min(block.start + 400);
    let head = lowered.get(block.start..window)?;
    let needle = format!("{name}=\"");
    let start = head.find(&needle)? + needle.len();
    let end = head[start..].find('"')? + start;
    Some(unescape(text.get(block.start + start..block.start + end)?))
}
/// The five entities XML defines for text, and nothing else.
///
/// A feed cannot make this build resolve a file, a URL, or a character it did not
/// choose to name, because nothing beyond these five is resolved at all: an
/// unknown entity is left as the characters the feed wrote.
fn unescape(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        let Some(end) = tail.find(';') else {
            out.push('&');
            out.push_str(&tail[1..]);
            return out;
        };
        let entity = &tail[1..end];
        let replacement = match entity {
            "amp" => Some("&"),
            "lt" => Some("<"),
            "gt" => Some(">"),
            "quot" => Some("\""),
            "apos" => Some("'"),
            _ => None,
        };
        match replacement {
            Some(replacement) => out.push_str(replacement),
            None => {
                out.push('&');
                out.push_str(entity);
                out.push(';');
            }
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    out
}

/// The elements of the text, for the tests that walk them.
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn feed() -> &'static str {
        r#"<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0" xmlns:vise="https://example.invalid/vise">
  <channel>
    <title>Vise Transcoder releases</title>
    <item>
      <title>0.2.0 — better queues</title>
      <version>0.2.0</version>
      <channel>stable</channel>
      <url>https://downloads.example.invalid/vise-0.2.0.zip</url>
      <length>10485760</length>
      <sha256>9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08</sha256>
      <published>2026-10-01</published>
      <notes>Queue recovery &amp; tray menus</notes>
    </item>
    <item>
      <title>0.3.0 — being tried out</title>
      <vise:version>0.3.0</vise:version>
      <vise:channel>preview</vise:channel>
      <enclosure url="https://downloads.example.invalid/vise-0.3.0.zip" length="2048"
                 sha256="9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08" />
    </item>
  </channel>
</rss>"#
    }

    #[test]
    fn a_feed_is_read_into_the_releases_it_lists() {
        let appcast = parse(feed()).expect("read");
        assert_eq!(appcast.title, "Vise Transcoder releases");
        assert_eq!(appcast.items.len(), 2);
        let stable = &appcast.items[0];
        assert_eq!(stable.version, "0.2.0");
        assert_eq!(stable.channel, "stable");
        assert_eq!(stable.length, Some(10_485_760));
        assert_eq!(stable.published, "2026-10-01");
        assert_eq!(
            stable.notes, "Queue recovery & tray menus",
            "the five entities are resolved"
        );
        assert!(stable
            .sha256
            .as_deref()
            .expect("hash")
            .starts_with("9f86d0"));

        // A field written with a namespace prefix is read the same as one without:
        // the prefix is how the feed says whose field it is, not what it means.
        let preview = &appcast.items[1];
        assert_eq!(preview.version, "0.3.0");
        assert_eq!(
            preview.url,
            "https://downloads.example.invalid/vise-0.3.0.zip"
        );
        assert_eq!(preview.notes, "", "a field the feed did not write is empty");
    }

    #[test]
    fn the_newest_release_on_a_channel_is_the_one_offered() {
        let appcast = parse(feed()).expect("read");
        assert_eq!(
            appcast
                .newest_on("stable")
                .expect("a stable release")
                .version,
            "0.2.0"
        );
        assert_eq!(
            appcast
                .newest_on("preview")
                .expect("a preview release")
                .version,
            "0.3.0"
        );
        // An item with no channel is on every channel: the feed did not say
        // otherwise, and refusing to offer it would be guessing.
        let open = AppcastItem {
            version: "0.4.0".to_string(),
            title: String::new(),
            notes: String::new(),
            channel: String::new(),
            url: "https://example.invalid/x.zip".to_string(),
            length: None,
            sha256: None,
            published: String::new(),
        };
        assert!(open.on_channel("stable"));
        assert!(!appcast.items[0].on_channel("preview"));
    }

    #[test]
    fn a_feed_that_declares_an_entity_is_refused_rather_than_resolved() {
        // The whole reason this parser is hand-written: a feed must not be able to
        // make this build read a file or fetch a URL because it asked nicely.
        for feed in [
            r#"<!DOCTYPE rss [<!ENTITY xxe SYSTEM "file:///C:/Windows/win.ini">]><rss><channel><item><version>1.0</version><url>https://example.invalid/x</url></item></channel></rss>"#,
            r#"<?xml version="1.0"?><!DOCTYPE rss><rss><channel><item><version>1.0</version><url>https://e.invalid/x</url></item></channel></rss>"#,
            r#"<!ENTITY xxe SYSTEM "https://evil.invalid/?x">"#,
        ] {
            assert_eq!(
                parse(feed).expect_err("refused"),
                AppcastError::DeclaresEntities,
                "a feed that declares anything is not read at all"
            );
        }
        // A feed that uses an entity it never declared cannot make this build read
        // anything: the reference is left as the characters the feed wrote, so what
        // reaches the release notes is what a reader would see in the file.
        let used = parse(
            r#"<rss><channel><item><version>1.0</version><url>https://e.invalid/x</url><notes>&xxe;</notes></item></channel></rss>"#,
        )
        .expect("read");
        assert_eq!(used.items[0].notes, "&xxe;");
        // An entity that is not one of the five is left as the characters the feed
        // wrote, rather than resolved into nothing.
        let kept = parse(
            r#"<rss><channel><item><version>1.0</version><url>https://e.invalid/x</url><notes>a &nbsp; b</notes></item></channel></rss>"#,
        )
        .expect("read");
        assert_eq!(kept.items[0].notes, "a &nbsp; b");
    }

    #[test]
    fn a_feed_that_is_not_a_feed_is_refused_with_a_reason() {
        assert!(matches!(
            parse("not xml at all"),
            Err(AppcastError::Malformed { .. })
        ));
        assert!(parse("").is_err());
        assert!(parse("<rss><channel><title>empty</title></channel></rss>")
            .expect_err("no releases")
            .to_string()
            .contains("no releases"));
        let oversized = format!(
            "<rss><channel><item><version>1.0</version><url>https://e.invalid/x</url><notes>{}</notes></item></channel></rss>",
            "x".repeat(MAX_APPCAST_BYTES)
        );
        assert!(matches!(
            parse(&oversized),
            Err(AppcastError::TooLarge { .. })
        ));
    }

    #[test]
    fn a_release_with_no_version_or_no_address_is_refused_by_name() {
        assert_eq!(
            parse("<rss><channel><item><url>https://e.invalid/x</url></item></channel></rss>")
                .expect_err("no version"),
            AppcastError::NoVersion
        );
        assert!(
            parse("<rss><channel><item><version>1.0</version></item></channel></rss>")
                .expect_err("no address")
                .to_string()
                .contains("no address")
        );
    }

    #[test]
    fn an_artifact_that_is_not_over_https_is_refused() {
        // A feed is a page on a server; a `file://` artifact would be this build
        // reading the user's disk because the page said so.
        for url in [
            "file:///C:/Windows/win.ini",
            "http://downloads.example.invalid/vise.zip",
            "\\\\nas\\share\\vise.zip",
            "vise.zip",
            "https://",
        ] {
            let feed = format!(
                "<rss><channel><item><version>1.0</version><url>{url}</url></item></channel></rss>"
            );
            let error = parse(&feed).expect_err("refused");
            assert!(
                matches!(error, AppcastError::RefusedUrl { .. }),
                "{url} was accepted: {error}"
            );
            assert!(!is_allowed_url(url), "{url}");
            assert!(is_allowed_url("https://example.invalid/x.zip"));
            assert!(is_allowed_url("HTTPS://example.invalid/x.zip"));
        }
    }

    #[test]
    fn a_hash_that_is_not_a_sha256_is_refused_rather_than_compared() {
        let feed = "<rss><channel><item><version>1.0</version><url>https://e.invalid/x</url><sha256>not-a-hash</sha256></item></channel></rss>";
        assert!(matches!(parse(feed), Err(AppcastError::RefusedHash { .. })));
        assert!(!is_sha256("abc"));
        assert!(!is_sha256(&"a".repeat(63)));
        assert!(!is_sha256(&"g".repeat(64)));
        assert!(is_sha256(&"A".repeat(64)));
    }

    #[test]
    fn the_hash_matches_the_published_vectors() {
        // The vectors are the ones FIPS 180-4 prints, so this implementation is
        // checked against something other than itself.
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        // Longer than one block, which is where an implementation can go wrong.
        assert_eq!(
            sha256_hex(&vec![b'a'; 1_000_000]),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
        // A hash of the bytes, not of the text of them.
        assert_eq!(sha256_hex(b"abc"), sha256_hex("abc".as_bytes()));
    }

    #[test]
    fn an_artifact_that_does_not_match_the_hash_is_not_offered() {
        let appcast = parse(feed()).expect("read");
        let item = &appcast.items[0];
        let wrong = b"not the release";
        assert!(matches!(
            verify(item, wrong),
            Err(AppcastError::HashMismatch { .. })
        ));
        let message = verify(item, wrong).expect_err("mismatch").to_string();
        assert!(message.contains("will not be used"), "{message}");
        // The right bytes pass, in either case for the digits.
        let digest = sha256_hex(b"abc");
        let matching = AppcastItem {
            sha256: Some(digest.clone()),
            ..item.clone()
        };
        assert_eq!(verify(&matching, b"abc"), Ok(()));
        let lower = AppcastItem {
            sha256: Some(digest.to_uppercase()),
            ..item.clone()
        };
        assert_eq!(verify(&lower, b"abc"), Ok(()));
    }

    #[test]
    fn an_artifact_with_no_hash_to_check_against_is_refused() {
        let item = AppcastItem {
            sha256: None,
            ..AppcastItem {
                version: "1.0".to_string(),
                title: String::new(),
                notes: String::new(),
                channel: String::new(),
                url: "https://example.invalid/x.zip".to_string(),
                length: None,
                sha256: Some(String::new()),
                published: String::new(),
            }
        };
        assert!(matches!(
            verify(&item, b"anything"),
            Err(AppcastError::RefusedHash { .. })
        ));
    }

    #[test]
    fn a_release_is_only_offered_when_it_is_actually_newer() {
        assert!(is_newer("0.2.0", "0.1.9"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
        // A pre-release suffix is not a claim this build can check, so the numbers
        // are what is compared and nothing is guessed about the suffix.
        assert!(!is_newer("0.2.0-beta", "0.2.0"));
        assert!(is_newer("0.2.1", "0.2.0-beta"));
        assert!(!is_newer("nonsense", "0.1.0"));
        assert!(is_newer("0.1.1", "nonsense"));
    }

    #[test]
    fn the_fetch_is_an_argument_vector_and_the_url_is_only_one_of_the_arguments() {
        let arguments = CurlSource::arguments("https://downloads.example.invalid/vise.zip");
        assert_eq!(
            arguments.last().expect("a url").as_str(),
            "https://downloads.example.invalid/vise.zip"
        );
        assert!(
            arguments.contains(&"=https".to_string()),
            "only https is allowed: {arguments:?}"
        );
        assert!(arguments.contains(&"--".to_string()));
        assert!(
            arguments.iter().any(|a| a == "--user-agent"),
            "the feed server is told who is asking"
        );
        assert!(!arguments.join(" ").contains(" -k"), "no insecure options");
        assert!(
            !arguments.join(" ").contains(">"),
            "nothing is redirected into a file"
        );
    }

    #[test]
    fn a_fetch_of_something_that_is_not_an_https_address_does_not_run_at_all() {
        let source = CurlSource::new("curl.exe");
        for url in ["file:///C:/Windows/win.ini", "http://example.invalid/x"] {
            assert!(matches!(
                source.fetch(url),
                Err(AppcastError::RefusedUrl { .. })
            ));
        }
    }

    #[test]
    fn a_build_with_no_transport_says_so_rather_than_pretending_to_look() {
        let none = NoSource;
        assert!(!none.available());
        assert!(
            none.describe().contains("no way to fetch"),
            "{}",
            none.describe()
        );
        assert!(matches!(
            none.fetch("https://example.invalid/feed.xml"),
            Err(AppcastError::Unavailable { .. })
        ));
    }

    #[test]
    fn the_schedule_comes_from_the_preference_that_already_exists() {
        let today = (2026, 10, 17);
        // Never is never due, whatever happened last.
        assert!(!is_due(CheckFrequency::Never, Some((2020, 1, 1)), today));
        // A check that has never happened is due the first time the frequency is
        // anything but never.
        assert!(is_due(CheckFrequency::Weekly, None, today));
        // Once the interval has passed, it is due again.
        // The next check falls a day after the last one, so yesterday is due and
        // today is not.
        assert!(is_due(CheckFrequency::Daily, Some((2026, 10, 16)), today));
        assert!(
            !is_due(CheckFrequency::Daily, Some((2026, 10, 17)), today),
            "a check made today has already happened"
        );
        assert!(!is_due(CheckFrequency::Monthly, Some((2026, 10, 1)), today));
        assert!(is_due(CheckFrequency::Monthly, Some((2026, 7, 1)), today));
    }

    #[test]
    fn what_a_check_found_is_summarised_in_one_line_without_installing_anything() {
        let mut state = UpdateState::new();
        assert!(!state.configured, "no feed is configured yet");

        state.observe(CheckOutcome::NotConfigured, (2026, 10, 17));
        assert!(
            state.summary.contains("no release feed"),
            "{}",
            state.summary
        );
        assert!(!state.available);

        state.observe(
            CheckOutcome::Failed {
                reason: "the network was not there".to_string(),
            },
            (2026, 10, 17),
        );
        assert_eq!(state.outcome.as_deref(), Some("failed"));
        assert!(state.summary.contains("network"), "{}", state.summary);

        state.observe(
            CheckOutcome::UpToDate {
                newest: "0.1.0".to_string(),
            },
            (2026, 10, 17),
        );
        assert!(!state.available);
        assert_eq!(state.version, "0.1.0");

        state.observe(
            CheckOutcome::Available {
                version: "0.2.0".to_string(),
                title: "better queues".to_string(),
                url: "https://downloads.example.invalid/vise-0.2.0.zip".to_string(),
            },
            (2026, 10, 17),
        );
        assert!(state.available);
        assert_eq!(state.version, "0.2.0");
        assert!(
            state
                .summary
                .contains("nothing has been downloaded or installed"),
            "{}",
            state.summary
        );
    }

    #[test]
    fn every_outcome_says_what_happened_in_one_line() {
        let outcomes = [
            CheckOutcome::NotConfigured,
            CheckOutcome::UpToDate {
                newest: "0.1.0".to_string(),
            },
            CheckOutcome::Available {
                version: "0.2.0".to_string(),
                title: "better queues".to_string(),
                url: "https://example.invalid/x".to_string(),
            },
            CheckOutcome::Failed {
                reason: "timed out".to_string(),
            },
        ];
        let mut seen: HashMap<String, usize> = HashMap::new();
        for outcome in outcomes {
            assert!(outcome.summary().ends_with('.') || outcome.summary().contains('.'));
            *seen.entry(outcome.summary()).or_default() += 1;
        }
        assert_eq!(seen.len(), 4, "each outcome reads differently");
    }
}
