//! One file somebody can attach to a question.
//!
//! "It is not working" is not a report. This is what turns a session into
//! something that can be answered: the version, the settings, what the machine
//! can do, the recent log, and what the queue was doing — in one archive, with
//! a name that says what is in it.
//!
//! # Nothing secret and nothing identifying
//!
//! Two rules, and they are not negotiable because the file leaves the machine:
//!
//! 1. **A token never goes in.** The worker's session token is redacted
//!    wherever it appears, by value as well as by name, because it turns up in
//!    places a key-name rule would never reach.
//! 2. **An absolute path never goes in whole.** A path is reduced to
//!    `<user-home>/<parent>/<name>` or `<path>/<parent>/<name>`, so that
//!    "D:\Media\clips\Episode 1.mkv" is still recognisable as a media path while
//!    the account it was read from is not. Applied to every entry, and applied
//!    inside free text as well as inside documents, because a log line about an
//!    encode quotes the paths it is complaining about.
//!
//! See [`Redactor`] for the whole policy and the reasoning behind each clause.
//!
//! # The same inputs produce the same archive
//!
//! Reproducible, not timestamped. There is no clock in the archive: entries are
//! written in sorted order with a fixed modification time, and the file is
//! named for the SHA-256 of its own contents. Two exports of the same session
//! are byte-for-byte identical, so one can be compared with the other, and a
//! bundle that is quoted in a bug report can be checked by the person reading
//! it.
//!
//! # Everything in it is untrusted
//!
//! The archive carries a README that says so, and the manifest carries a
//! digest of every entry. Whoever receives one should treat it as input from a
//! stranger: it contains a settings document, a log, and file names, all of
//! which came from media that did not choose to be well behaved.

use std::path::{Path, PathBuf};

use crate::system::updates::sha256_hex;

/// Version of the bundle layout this build writes.
pub const BUNDLE_VERSION: u32 = 1;

/// Prefix of the archive's file name.
pub const BUNDLE_PREFIX: &str = "vise-diagnostics-";

/// What a redacted secret is replaced with.
pub const REDACTED: &str = "<redacted>";

/// What stands in for the user's own folder.
pub const USER_HOME: &str = "<user-home>";

/// What stands in for any other absolute root.
pub const OTHER_ROOT: &str = "<path>";

/// Names whose value is replaced whatever it contains.
///
/// The words are matched against the lower-cased key, so `FeedUrl`, `feed_url`
/// and `FEED-URL` are one rule rather than three.
const SECRET_WORDS: &[&str] = &[
    "token",
    "secret",
    "password",
    "passphrase",
    "credential",
    "authorization",
    "apikey",
    "api_key",
    "cookie",
];

/// Removes secrets and identifying paths from text and documents.
///
/// The same value is redacted by **name** and by **content**, because the two
/// cover different holes: a key called `FeedUrl` is not a secret, and the token
/// that has to stay out of the archive appears inside an error message more
/// often than it appears under a key with an obvious name.
#[derive(Debug, Clone, Default)]
pub struct Redactor {
    /// The user's own folder, when the machine has one.
    user_home: Option<PathBuf>,
    /// Literal values that must never appear, such as the worker token.
    secrets: Vec<String>,
}

impl Redactor {
    /// A redactor for `secrets`, treating `user_home` as the user's folder.
    pub fn new(user_home: Option<PathBuf>, secrets: Vec<String>) -> Self {
        Redactor {
            user_home,
            // Very short values are not redacted: a two-character string is
            // everywhere in a log, and scrubbing it would replace the letters
            // in ordinary words.
            secrets: secrets
                .into_iter()
                .map(|secret| secret.trim().to_string())
                .filter(|secret| secret.len() >= 8)
                .collect(),
        }
    }

    /// The user's folder, as this machine reports it.
    pub fn for_this_machine(secrets: Vec<String>) -> Self {
        Redactor::new(
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from)),
            secrets,
        )
    }

    /// Redact a piece of text.
    pub fn redact(&self, text: &str) -> String {
        let mut out = text.to_string();
        for secret in &self.secrets {
            if out.contains(secret.as_str()) {
                out = out.replace(secret.as_str(), REDACTED);
            }
        }
        redact_paths(&out, self.user_home.as_deref())
    }

    /// Redact a document, keeping its shape.
    ///
    /// Keys are walked as well as strings, so a secret under an innocently named
    /// key is caught; arrays are walked in order; anything that is neither an
    /// object nor an array is a string and is redacted as text.
    pub fn redact_value(&self, value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(map) => serde_json::Value::Object(
                map.iter()
                    .map(|(key, value)| {
                        if is_secret_key(key) {
                            (key.clone(), serde_json::Value::String(REDACTED.to_string()))
                        } else {
                            (key.clone(), self.redact_value(value))
                        }
                    })
                    .collect(),
            ),
            serde_json::Value::Array(items) => {
                serde_json::Value::Array(items.iter().map(|item| self.redact_value(item)).collect())
            }
            serde_json::Value::String(text) => serde_json::Value::String(self.redact(text)),
            other => other.clone(),
        }
    }

    /// Redact a document given as text.
    ///
    /// A document that cannot be read is still redacted as text rather than being
    /// dropped: an export that silently lost the settings because one setting had
    /// a shape nobody expected is worse than one that shows the settings with
    /// every path reduced.
    pub fn redact_document(&self, text: &str) -> String {
        match serde_json::from_str::<serde_json::Value>(text) {
            Ok(value) => serde_json::to_string_pretty(&self.redact_value(&value))
                .unwrap_or_else(|_| self.redact(text)),
            Err(_) => self.redact(text),
        }
    }
}

/// Whether a key's value is a secret whatever it contains.
pub fn is_secret_key(key: &str) -> bool {
    let lowered = key.to_ascii_lowercase().replace(['-', '.'], "_");
    SECRET_WORDS.iter().any(|word| lowered.contains(word))
}

/// Replace absolute paths in `text` with a reduced form.
///
/// Two kinds of path are recognised: a Windows one (`C:\a\b\c`, with backslash
/// or forward slash) and a POSIX one (`/a/b/c`). Each is rewritten as
/// `<root>/<parent>/<name>`, which keeps enough of the shape for somebody
/// reading a bug report to recognise where the file was and removes the part
/// that identifies whose machine it was.
fn redact_paths(text: &str, user_home: Option<&Path>) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes: Vec<char> = text.chars().collect();
    let mut index = 0;
    while index < bytes.len() {
        if let Some(end) = path_end(&bytes, index) {
            let candidate: String = bytes[index..end].iter().collect();
            if let Some(shortened) = shorten(&candidate, user_home) {
                out.push_str(&shortened);
                index = end;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    out
}

/// Where the path starting at `index` ends, if one does.
fn path_end(chars: &[char], index: usize) -> Option<usize> {
    let drive = chars.get(index)?;
    // A Windows path: a letter followed by a colon and a separator. The letter
    // has to start a word, or the `s:` in `https://host/x` reads as a drive.
    if drive.is_ascii_alphabetic() && (index == 0 || !chars[index - 1].is_ascii_alphanumeric()) {
        let colon = chars.get(index + 1)?;
        let separator = chars.get(index + 2)?;
        if *colon == ':' && (*separator == '\\' || *separator == '/') {
            let mut end = index + 3;
            while end < chars.len() && !is_path_break(chars[end]) {
                end += 1;
            }
            return Some(end);
        }
    }
    // A POSIX path: a separator, and something after it that is not another
    // separator. The character before it has to end a word, or `https://host/x`
    // and `and/or` would both be read as paths and rewritten as if they were.
    if *drive == '/'
        && chars
            .get(index + 1)
            .is_some_and(|next| *next != '/' && *next != ':')
        && (index == 0
            || !(chars[index - 1].is_alphanumeric()
                || chars[index - 1] == '/'
                || chars[index - 1] == ':'))
    {
        let mut end = index;
        while end < chars.len() && !is_path_break(chars[end]) {
            end += 1;
        }
        return Some(end);
    }
    None
}

/// Whether a character ends a path written inside a sentence.
fn is_path_break(character: char) -> bool {
    character.is_whitespace() || matches!(character, '"' | '\'' | ',' | ')' | '(' | ']' | '[' | ';')
}

/// The reduced form of a path, or `None` when it is not one.
///
/// A path with fewer than two components is left alone: reducing `C:\` to
/// `<path>` would be redacting a separator rather than a location, and it would
/// do it to every sentence that mentions a drive.
fn shorten(candidate: &str, user_home: Option<&Path>) -> Option<String> {
    let windows = candidate.contains('\\') || candidate.contains(':');
    let (_, tail) = if windows {
        candidate.split_once(['\\', '/'])?
    } else {
        candidate.split_once('/')?
    };
    let components: Vec<&str> = tail
        .split(['\\', '/'])
        .filter(|part| !part.is_empty())
        .collect();
    if components.is_empty() {
        return None;
    }
    let root = if is_inside(candidate, user_home) {
        USER_HOME.to_string()
    } else {
        OTHER_ROOT.to_string()
    };
    let name = components[components.len() - 1];
    let parent = if components.len() > 1 {
        components[components.len() - 2]
    } else {
        ""
    };
    if parent.is_empty() {
        Some(format!("{root}/{name}"))
    } else {
        Some(format!("{root}/{parent}/{name}"))
    }
}

/// Whether `candidate` is inside `home`, compared on components.
///
/// Compared rather than canonicalised: a path in a log may not exist, and a rule
/// that needed the file to be there to decide whether to remove it would leave
/// the answer up to whatever happened to be on the disk.
fn is_inside(candidate: &str, home: Option<&Path>) -> bool {
    let Some(home) = home else {
        return false;
    };
    let home = home.to_string_lossy().replace('\\', "/");
    let candidate = candidate.replace('\\', "/");
    let candidate = candidate.to_ascii_lowercase();
    let home = home.trim_end_matches('/').to_ascii_lowercase();
    !home.is_empty()
        && candidate.len() > home.len()
        && candidate.starts_with(&home)
        && candidate.as_bytes()[home.len()] == b'/'
}

/// One file in the archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleEntry {
    /// Path of the file inside the archive, using forward slashes.
    pub name: String,
    /// Its bytes.
    pub bytes: Vec<u8>,
}

impl BundleEntry {
    /// An entry from text.
    pub fn text(name: &str, text: impl Into<String>) -> Self {
        BundleEntry {
            name: name.to_string(),
            bytes: text.into().into_bytes(),
        }
    }

    /// The entry's digest, as it appears in the manifest.
    pub fn digest(&self) -> String {
        sha256_hex(&self.bytes)
    }
}

/// Everything a bundle is made of, before it is redacted.
#[derive(Debug, Clone, Default)]
pub struct BundleInput {
    /// Application version.
    pub version: String,
    /// The settings document, as JSON text.
    pub settings: String,
    /// The capability report, as JSON text.
    pub capabilities: String,
    /// The capability report, as text.
    pub capabilities_text: String,
    /// The recent activity log, as text.
    pub log: String,
    /// A summary of the queue, as text.
    pub queue: String,
    /// Anything else worth carrying, as name and text.
    pub extra: Vec<(String, String)>,
}

/// The built archive, and what it says about itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticBundle {
    /// The files inside, in the order they were written.
    pub entries: Vec<BundleEntry>,
    /// SHA-256 of the archive's bytes.
    pub content_hash: String,
    /// The name to write it under.
    pub file_name: String,
    /// How many entries were removed by the size bound.
    pub omitted: usize,
}

/// Size one entry may have, in bytes.
///
/// A bundle is something somebody attaches to a message, so it is bounded. The
/// log is the entry that grows without limit and it is the one that is truncated
/// rather than dropped: a bundle with no log in it is not a bundle anybody asked
/// for, and the note that says it was cut is the first line of the log.
pub const MAX_ENTRY_BYTES: usize = 512 * 1024;

impl DiagnosticBundle {
    /// Build an archive from `input`, with everything redacted.
    ///
    /// Entries are written in sorted order and the manifest is written last, so
    /// two builds from the same input produce the same bytes.
    pub fn build(input: &BundleInput, redactor: &Redactor) -> Self {
        let mut entries: Vec<BundleEntry> = Vec::new();
        entries.push(BundleEntry::text(
            "version.json",
            redactor.redact_document(
                &serde_json::json!({
                    "bundleVersion": BUNDLE_VERSION,
                    "application": "Vise Transcoder",
                    "version": input.version,
                })
                .to_string(),
            ),
        ));
        entries.push(BundleEntry::text(
            "settings.json",
            redactor.redact_document(&input.settings),
        ));
        entries.push(BundleEntry::text(
            "capabilities.json",
            redactor.redact_document(&input.capabilities),
        ));
        entries.push(BundleEntry::text(
            "capabilities.txt",
            redactor.redact(&input.capabilities_text),
        ));
        entries.push(BundleEntry::text(
            "queue.txt",
            redactor.redact(&input.queue),
        ));
        let (log, omitted) = bounded(&redactor.redact(&input.log), MAX_ENTRY_BYTES);
        if omitted > 0 {
            entries.push(BundleEntry::text(
                "log.txt",
                format!(
                    "{omitted} earlier byte(s) were left out to keep this bundle a size somebody can send.\n{log}"
                ),
            ));
        } else {
            entries.push(BundleEntry::text("log.txt", log));
        }
        for (name, text) in &input.extra {
            entries.push(BundleEntry::text(name, redactor.redact_document(text)));
        }
        entries.push(BundleEntry::text("README.txt", readme()));
        entries.sort_by(|left, right| left.name.cmp(&right.name));

        let manifest = entries
            .iter()
            .map(|entry| serde_json::json!({ "name": entry.name, "sha256": entry.digest() }))
            .collect::<Vec<_>>();
        entries.push(BundleEntry::text(
            "manifest.json",
            serde_json::to_string_pretty(&serde_json::json!({
                "bundleVersion": BUNDLE_VERSION,
                "entries": manifest,
            }))
            .unwrap_or_else(|_| "{}".to_string()),
        ));

        let bytes = write_archive(&entries);
        let content_hash = sha256_hex(&bytes);
        let file_name = format!("{BUNDLE_PREFIX}{}.zip", &content_hash[..16]);
        DiagnosticBundle {
            entries,
            content_hash,
            file_name,
            omitted,
        }
    }

    /// The archive's bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        write_archive(&self.entries)
    }

    /// The name the bundle is reported under, without a folder.
    pub fn file_name(&self) -> &str {
        &self.file_name
    }

    /// One line describing the bundle.
    pub fn summary(&self) -> String {
        format!(
            "{} file(s) ({}) named for its own contents: {}",
            self.entries.len(),
            self.to_bytes().len(),
            self.file_name
        )
    }
}

/// Cut `text` to `limit` bytes, keeping the **end** of it, and report what was
/// left out.
///
/// The end, because the newest lines of a log are the ones a question is about:
/// cutting the tail would throw away the encode that failed and keep the one
/// that worked four hours earlier. The cut lands on a character boundary and
/// then on a line boundary, because a log line that ends half way through says
/// something that is not true.
fn bounded(text: &str, limit: usize) -> (String, usize) {
    if text.len() <= limit {
        return (text.to_string(), 0);
    }
    let start = text.len() - limit;
    let at = text[start..]
        .char_indices()
        .map(|(index, _)| start + index)
        .next()
        .unwrap_or(start);
    let cut = at + text[at..].find('\n').map(|found| found + 1).unwrap_or(0);
    (text[cut..].to_string(), cut)
}

/// The note that travels with every bundle.
fn readme() -> String {
    [
        "This is a Vise Transcoder diagnostic bundle.",
        "",
        "It holds the version, the settings, what the media tools on this machine can",
        "do, the recent activity log, and a summary of the queue.",
        "",
        "Before it was written: any token and any absolute path was removed, and a",
        "value stored under a secret-sounding key was replaced. So the settings and",
        "the log are useful and the account the files were read from is not in here.",
        "",
        "Treat everything in this archive as untrusted input. It was assembled from a",
        "settings document, a log, and file names that came from media, and none of",
        "those chose to be well behaved. manifest.json carries a SHA-256 of every",
        "file, so a file that has been changed can be spotted.",
        "",
        "The archive is reproducible: the same session produces the same bytes, and",
        "the file name is the digest of its own contents rather than a timestamp.",
    ]
    .join("\n")
}

/* ------------------------------------------------------------------- archive */

/// MS-DOS date and time for 1980-01-01 00:00:00, the epoch a ZIP starts at.
///
/// A fixed stamp is the whole of the reproducibility: an archive whose entries
/// carry the time it was built cannot be compared with one built a minute later.
const DOS_EPOCH_DATE: u16 = 0x0021;
const DOS_EPOCH_TIME: u16 = 0x0000;

/// Write a ZIP archive holding `entries`, stored and uncompressed.
///
/// Stored rather than deflated on purpose: this build carries no compressor, and
/// a bundle of a settings document and a text log compresses by a fraction that
/// does not matter next to a dependency.
pub fn write_archive(entries: &[BundleEntry]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut directory: Vec<u8> = Vec::new();
    for entry in entries {
        let offset = out.len() as u32;
        let name = entry.name.as_bytes();
        let crc = crc32(&entry.bytes);
        let size = entry.bytes.len() as u32;

        put_u32(&mut out, 0x0403_4b50);
        put_u16(&mut out, 20); // version needed
        put_u16(&mut out, 0x0800); // names are UTF-8
        put_u16(&mut out, 0); // stored
        put_u16(&mut out, DOS_EPOCH_TIME);
        put_u16(&mut out, DOS_EPOCH_DATE);
        put_u32(&mut out, crc);
        put_u32(&mut out, size);
        put_u32(&mut out, size);
        put_u16(&mut out, name.len() as u16);
        put_u16(&mut out, 0);
        out.extend_from_slice(name);
        out.extend_from_slice(&entry.bytes);

        put_u32(&mut directory, 0x0201_4b50);
        put_u16(&mut directory, 20); // version made by
        put_u16(&mut directory, 20); // version needed
        put_u16(&mut directory, 0x0800);
        put_u16(&mut directory, 0);
        put_u16(&mut directory, DOS_EPOCH_TIME);
        put_u16(&mut directory, DOS_EPOCH_DATE);
        put_u32(&mut directory, crc);
        put_u32(&mut directory, size);
        put_u32(&mut directory, size);
        put_u16(&mut directory, name.len() as u16);
        put_u16(&mut directory, 0); // extra
        put_u16(&mut directory, 0); // comment
        put_u16(&mut directory, 0); // disk
        put_u16(&mut directory, 0); // internal attributes
        put_u32(&mut directory, 0); // external attributes
        put_u32(&mut directory, offset);
        directory.extend_from_slice(name);
    }
    let directory_offset = out.len() as u32;
    let directory_size = directory.len() as u32;
    out.extend_from_slice(&directory);
    put_u32(&mut out, 0x0605_4b50);
    put_u16(&mut out, 0);
    put_u16(&mut out, 0);
    put_u16(&mut out, entries.len() as u16);
    put_u16(&mut out, entries.len() as u16);
    put_u32(&mut out, directory_size);
    put_u32(&mut out, directory_offset);
    put_u16(&mut out, 0);
    out
}

fn put_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// CRC-32 as a ZIP entry header carries it.
///
/// The reflected IEEE polynomial, checked below against the published value for
/// "123456789" so that a bundle written here opens anywhere.
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (index, entry) in table.iter_mut().enumerate() {
        let mut value = index as u32;
        for _ in 0..8 {
            value = if value & 1 == 1 {
                0xEDB8_8320 ^ (value >> 1)
            } else {
                value >> 1
            };
        }
        *entry = value;
    }
    let mut crc = 0xFFFF_FFFFu32;
    for byte in bytes {
        crc = table[((crc ^ *byte as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The token a worker session would have.
    const TOKEN: &str = "s3cr3t-worker-token-value";
    const HOME: &str = "C:\\Users\\Edison";

    /// One entry of a bundle, as text, for a test that checks its contents.
    fn text_of(bundle: &DiagnosticBundle, name: &str) -> String {
        let entry = bundle
            .entries
            .iter()
            .find(|entry| entry.name == name)
            .unwrap_or_else(|| panic!("{name} is not in the bundle"));
        String::from_utf8(entry.bytes.clone()).expect("the entry is text")
    }

    fn redactor() -> Redactor {
        Redactor::new(Some(PathBuf::from(HOME)), vec![TOKEN.to_string()])
    }

    fn input() -> BundleInput {
        BundleInput {
            version: "0.1.0".to_string(),
            settings: serde_json::json!({
                "output": { "folder": "D:\\Media\\out" },
                "updates": { "feedUrl": "https://example.invalid/feed.xml", "token": TOKEN },
                "advanced": { "logLevel": "info" },
            })
            .to_string(),
            capabilities: serde_json::json!({
                "detected": true,
                "hardwareEncoders": ["hevc_nvenc"],
            })
            .to_string(),
            capabilities_text: "Vise Transcoder 0.1.0\nffmpeg was found\n".to_string(),
            log: format!(
                "[Info] encode started for {HOME}\\Videos\\source.mkv\n[Error] rejected with {TOKEN}\n[Info] wrote {HOME}\\Videos\\clip.mkv\n"
            ),
            queue: "2 job(s): 1 finished, 1 failed\n".to_string(),
            extra: Vec::new(),
        }
    }

    fn bundle_text(bundle: &DiagnosticBundle) -> String {
        bundle
            .entries
            .iter()
            .map(|entry| {
                format!(
                    "--- {} ---\n{}",
                    entry.name,
                    String::from_utf8_lossy(&entry.bytes)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn a_token_appears_nowhere_in_the_bundle() {
        let bundle = DiagnosticBundle::build(&input(), &redactor());
        let text = bundle_text(&bundle);
        assert!(!text.contains(TOKEN), "the token is still in the bundle");
        assert!(bundle
            .to_bytes()
            .windows(TOKEN.len())
            .all(|w| w != TOKEN.as_bytes()));
        assert!(text.contains(REDACTED), "and what replaced it is named");
    }

    #[test]
    fn no_absolute_path_appears_in_the_bundle_unredacted() {
        let bundle = DiagnosticBundle::build(&input(), &redactor());
        let text = bundle_text(&bundle);
        assert!(
            !text.contains(HOME),
            "the user's folder is still in the bundle:\n{text}"
        );
        assert!(
            !text.contains("D:\\Media\\out"),
            "nor another machine's folder:\n{text}"
        );
        assert!(text.contains("<user-home>/Videos/clip.mkv"), "{text}");
        assert!(text.contains("<path>/Media/out"), "{text}");
    }

    #[test]
    fn a_secret_is_removed_by_its_key_as_well_as_by_its_value() {
        let document = serde_json::json!({
            "FeedUrl": "https://example.invalid/feed.xml",
            "api_key": "abcdef123456",
            "innocuous": format!("the value is {TOKEN}"),
        });
        let redacted = redactor().redact_document(&document.to_string());
        assert!(
            redacted.contains("https://example.invalid/feed.xml"),
            "a feed is not a secret"
        );
        assert!(!redacted.contains("abcdef123456"), "{redacted}");
        assert!(
            redacted.contains("\"api_key\": \"<redacted>\""),
            "{redacted}"
        );
        assert!(!redacted.contains(TOKEN), "{redacted}");
    }

    #[test]
    fn a_path_inside_a_sentence_is_reduced_rather_than_the_whole_sentence() {
        let redacted = redactor().redact("encoding C:\\Users\\Edison\\Videos\\clip.mkv now");
        assert_eq!(redacted, "encoding <user-home>/Videos/clip.mkv now");

        let posix = Redactor::new(Some(PathBuf::from("/home/edison")), Vec::new())
            .redact("wrote /home/edison/media/clip.mkv and /srv/share/a.mkv");
        assert_eq!(
            posix,
            "wrote <user-home>/media/clip.mkv and <path>/share/a.mkv"
        );
    }

    #[test]
    fn a_bare_separator_or_a_drive_letter_is_not_a_path_and_is_left_alone() {
        let redactor = redactor();
        assert_eq!(redactor.redact("a / b"), "a / b");
        assert_eq!(redactor.redact("C: was not a drive"), "C: was not a drive");
        assert_eq!(
            redactor.redact("https://example.invalid/x"),
            "https://example.invalid/x"
        );
    }

    #[test]
    fn a_bundle_is_byte_for_byte_the_same_for_the_same_session() {
        let first = DiagnosticBundle::build(&input(), &redactor());
        let second = DiagnosticBundle::build(&input(), &redactor());
        assert_eq!(first.to_bytes(), second.to_bytes());
        assert_eq!(first.content_hash, second.content_hash);
        assert_eq!(first.file_name, second.file_name);
        assert!(first.file_name.starts_with(BUNDLE_PREFIX));
        assert!(first.file_name.ends_with(".zip"));
        assert!(
            first.file_name.contains(&first.content_hash[..16]),
            "the name is the digest of its own contents: {}",
            first.file_name
        );
    }

    #[test]
    fn a_different_session_produces_a_different_bundle() {
        let mut changed = input();
        changed.log.push_str("[Info] one more line\n");
        let first = DiagnosticBundle::build(&input(), &redactor());
        let second = DiagnosticBundle::build(&changed, &redactor());
        assert_ne!(first.content_hash, second.content_hash);
        assert_ne!(first.file_name, second.file_name);
    }

    #[test]
    fn the_entries_are_written_in_a_fixed_order_and_manifested() {
        let bundle = DiagnosticBundle::build(&input(), &redactor());
        let names: Vec<&str> = bundle
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();
        assert_eq!(names.last().copied(), Some("manifest.json"));
        let sorted: Vec<&str> = names[..names.len() - 1].to_vec();
        let mut expected = sorted.clone();
        expected.sort();
        assert_eq!(sorted, expected, "everything before the manifest is sorted");

        let manifest = text_of(&bundle, "manifest.json");
        for name in &sorted {
            assert!(manifest.contains(name), "{name} is not in the manifest");
        }
        let parsed: serde_json::Value = serde_json::from_str(&manifest).expect("manifest json");
        let listed = parsed["entries"].as_array().expect("entries");
        assert_eq!(listed.len(), sorted.len());
        // Every digest in the manifest is the digest of the file it names.
        for item in listed {
            let name = item["name"].as_str().expect("name");
            let digest = item["sha256"].as_str().expect("digest");
            assert_eq!(digest, &sha256_hex(text_of(&bundle, name).as_bytes()));
        }
    }

    #[test]
    fn the_bundle_says_what_it_is_and_what_to_do_with_it() {
        let bundle = DiagnosticBundle::build(&input(), &redactor());
        let readme = text_of(&bundle, "README.txt");
        assert!(readme.contains("untrusted input"));
        assert!(readme.contains("removed"));
        assert!(readme.contains("reproducible"));
        assert!(bundle.summary().contains("file(s)"));
    }

    #[test]
    fn a_log_too_long_to_send_is_cut_on_a_line_and_says_so() {
        let mut long = input();
        long.log = (0..20_000)
            .map(|index| format!("line {index} of the activity log\n"))
            .collect();
        let bundle = DiagnosticBundle::build(&long, &redactor());
        assert!(bundle.omitted > 0);
        let log = text_of(&bundle, "log.txt");
        assert!(log.contains(" earlier byte(s) were left out"), "{log}");
        assert!(
            log.contains("line 19999 of the activity log"),
            "the newest lines are kept"
        );
        assert!(!log.contains("line 0 of the activity log\nline 1"), "{log}");
        assert!(
            bundle.to_bytes().len() < MAX_ENTRY_BYTES * 2,
            "and the bundle stayed a size"
        );
    }

    #[test]
    fn an_extra_document_is_carried_and_redacted_too() {
        let mut with_extra = input();
        with_extra.extra.push((
            "notes.json".to_string(),
            serde_json::json!({ "path": "C:\\Users\\Edison\\Desktop\\x.json" }).to_string(),
        ));
        let bundle = DiagnosticBundle::build(&with_extra, &redactor());
        let notes = text_of(&bundle, "notes.json");
        assert!(!notes.contains("Edison"), "{notes}");
        assert!(notes.contains("<user-home>/Desktop/x.json"), "{notes}");
    }

    #[test]
    fn a_document_that_cannot_be_read_is_still_redacted_rather_than_dropped() {
        let mut broken = input();
        broken.settings = "{ this is not the settings".to_string();
        let bundle = DiagnosticBundle::build(&broken, &redactor());
        let settings = text_of(&bundle, "settings.json");
        assert!(settings.contains("this is not the settings"));
    }

    #[test]
    fn the_crc_is_the_published_one() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn the_archive_is_a_zip_with_one_record_per_entry() {
        let bundle = DiagnosticBundle::build(&input(), &redactor());
        let bytes = bundle.to_bytes();
        assert_eq!(
            &bytes[..4],
            &[0x50, 0x4b, 0x03, 0x04],
            "a local file header"
        );
        assert!(
            bytes
                .windows(4)
                .any(|window| window == [0x50, 0x4b, 0x01, 0x02]),
            "and a central directory"
        );
        assert!(
            bytes
                .windows(4)
                .any(|window| window == [0x50, 0x4b, 0x05, 0x06]),
            "and an end-of-central-directory record"
        );
        // One local header per entry, plus the central directory's, plus the end.
        let locals = bytes
            .windows(4)
            .filter(|window| *window == [0x50, 0x4b, 0x03, 0x04])
            .count();
        assert_eq!(locals, bundle.entries.len());
    }

    #[test]
    fn a_secret_shorter_than_a_word_is_not_scrubbed_out_of_ordinary_text() {
        // A two-character "secret" is every other letter in the log, and
        // replacing it would destroy the log rather than protect it.
        let redactor = Redactor::new(None, vec!["ab".to_string()]);
        assert_eq!(redactor.redact("about"), "about");
    }
}
