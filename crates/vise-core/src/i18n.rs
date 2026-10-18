//! Languages, translation resources, and the rules that resolve one.
//!
//! Three things live here because they are one decision rather than three: which
//! languages the build ships, what each of them calls a piece of the interface,
//! and what happens when a language does not have a piece of it. Splitting them
//! is how a window ends up showing a key, or a blank, or English in a list that
//! claims to be translated.
//!
//! A string is reached by [`MessageKey`], which is an enum rather than a name.
//! That is the whole mechanism for "a missing translation is a build error
//! rather than a blank label": a key that was never declared cannot be spelled,
//! and [`MessageKey::ALL`] is the list a locale file is checked against, so a
//! locale that has not been brought forward fails a test instead of shipping.
//!
//! Each locale is a file under `data/locales`, embedded into the binary with
//! `include_str!`. The base locale is a file too, not a set of `match` arms,
//! so "every key is present" means the same thing in every language and adding
//! a key is one line in six files rather than six arms to keep in step.
//!
//! Resolution is a chain with three rungs and it is recorded, not guessed:
//!
//! 1. the locale itself (`Source::Locale`);
//! 2. the same language in another region (`Source::BaseLanguage`), so a
//!    `de-AT` window can read `de` rather than falling straight to English;
//! 3. the base locale (`Source::Base`), which every key has, because the base
//!    file is checked like the others.
//!
//! A key that is in none of them still has an answer: [`MessageKey::label`],
//! which is a readable name rather than the key. A user must never be shown an
//! identifier. Every rung the chain took is available afterwards through
//! [`Catalog::gaps`], so a half-translated locale is a report the build can
//! print rather than a surprise somebody notices on screen.
//!
//! Preset descriptions are handled the same way and for the same reason. The
//! catalog carries the base text, the locale files carry translations of a
//! subset of it, and a preset nobody has translated falls back to the catalog's
//! own words. Machine-translating a hundred and forty presets would ship
//! sentences nobody has read; a mechanism that falls back honestly is worth
//! more than a hundred and forty invented ones.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::OnceLock;

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

/// The locale everything else falls back to.
///
/// English, because it is the language this project was written in and because
/// it is the only one whose text a contributor can be certain of without a
/// translator having checked it.
pub const BASE_LOCALE: &str = "en";

/* ------------------------------------------------------------------ locales */

/// Which way a language reads.
///
/// This is a property of the language rather than of the window: an Arabic
/// window is laid out right to left whatever the page reports, and a script
/// subtag settles it before the language does, so a Kurdish locale written in
/// the Arabic script is mirrored even though the language alone would not be.
/// An empty tag, a wildcard, or a tag with nothing recognisable in it reads
/// left to right, which keeps a window usable when the locale is unknown
/// rather than mirroring it at random.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReadingDirection {
    /// Left to right.
    Ltr,
    /// Right to left.
    Rtl,
}

impl ReadingDirection {
    /// Value the page puts on the document as `dir`.
    pub fn id(self) -> &'static str {
        match self {
            ReadingDirection::Ltr => "ltr",
            ReadingDirection::Rtl => "rtl",
        }
    }

    /// Whether this direction reads right to left.
    pub fn is_rtl(self) -> bool {
        matches!(self, ReadingDirection::Rtl)
    }
}

/// Which way an arbitrary language tag reads.
///
/// The rules are the ones [`Locale::direction`] uses, expressed once and
/// callable with a tag this build does not ship, because the answer to "which
/// way does this machine read" has to be available before the locale has been
/// matched against the list of languages the build offers.
pub fn reading_direction(tag: &str) -> ReadingDirection {
    let mut subtags = tag.split(['-', '_']).map(str::trim);
    let language = subtags.next().unwrap_or_default().to_ascii_lowercase();
    if language.is_empty() || language == "*" {
        return ReadingDirection::Ltr;
    }
    for subtag in subtags {
        let lowered = subtag.to_ascii_lowercase();
        if lowered.len() == 4 && RTL_SCRIPTS.contains(&lowered.as_str()) {
            return ReadingDirection::Rtl;
        }
    }
    if RTL_LANGUAGES.contains(&language.as_str()) {
        return ReadingDirection::Rtl;
    }
    ReadingDirection::Ltr
}

/// Primary language subtags written right to left.
///
/// The list is deliberately wider than the languages this build ships: a locale
/// on the machine that is not in the picker still has to be laid out the way it
/// reads.
const RTL_LANGUAGES: &[&str] = &[
    "ar",  // Arabic
    "arc", // Aramaic
    "ckb", // Central Kurdish
    "dv",  // Divehi
    "fa",  // Persian
    "he",  // Hebrew
    "khw", // Khowar
    "ks",  // Kashmiri
    "ku",  // Kurdish
    "ps",  // Pashto
    "sd",  // Sindhi
    "ug",  // Uyghur
    "ur",  // Urdu
    "yi",  // Yiddish
];

/// Script subtags that read right to left whatever language they are paired
/// with, so a language written in more than one script resolves by its script
/// rather than by the language alone.
const RTL_SCRIPTS: &[&str] = &[
    "adlm", // Adlam
    "arab", // Arabic
    "hebr", // Hebrew
    "mand", // Mandaic
    "mend", // Mende Kikakui
    "nkoo", // N'Ko
    "rohg", // Hanifi Rohingya
    "samr", // Samaritan
    "syrc", // Syriac
    "thaa", // Thaana
];

/// One language the build can show.
///
/// The set is chosen rather than maximal. Six languages cover the scripts and
/// the plural habits that break a naive pipeline, and every one of them has
/// been translated for the whole key set rather than a fraction of it, which is
/// the trade this group wanted: a language that is three-quarters translated
/// reads worse than one that is complete. Adding a language is a file, an entry
/// in [`SUPPORTED`], and the tests that come with the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// English.
    English,
    /// Spanish.
    Spanish,
    /// German.
    German,
    /// French.
    French,
    /// Japanese.
    Japanese,
    /// Arabic, which is here to keep the right-to-left path exercised by a
    /// shipped language rather than only by a test fixture.
    Arabic,
}

impl Language {
    /// Every language, in the order the picker lists them.
    pub const ALL: [Language; 6] = [
        Language::English,
        Language::Spanish,
        Language::German,
        Language::French,
        Language::Japanese,
        Language::Arabic,
    ];

    /// The language's own tag.
    pub fn tag(self) -> &'static str {
        match self {
            Language::English => "en",
            Language::Spanish => "es",
            Language::German => "de",
            Language::French => "fr",
            Language::Japanese => "ja",
            Language::Arabic => "ar",
        }
    }

    /// What a speaker of the language calls it.
    ///
    /// A proper name, and therefore not a translatable string: a language is
    /// listed in the language it is written in, which is the one case where
    /// translating the label makes the picker unusable.
    pub fn endonym(self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Spanish => "Español",
            Language::German => "Deutsch",
            Language::French => "Français",
            Language::Japanese => "日本語",
            Language::Arabic => "العربية",
        }
    }

    /// Which way it reads.
    pub fn direction(self) -> ReadingDirection {
        reading_direction(self.tag())
    }

    /// The language a tag names, if it names one of these.
    pub fn of_tag(tag: &str) -> Option<Self> {
        let language = primary_subtag(tag);
        Language::ALL
            .into_iter()
            .find(|item| item.tag() == language)
    }
}

/// A language, with the tag the interface layer sends back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub struct Locale {
    tag: &'static str,
    language: Language,
}

impl Locale {
    const fn new(tag: &'static str, language: Language) -> Self {
        Self { tag, language }
    }

    /// The tag, as the picker and the settings file write it.
    pub fn tag(self) -> &'static str {
        self.tag
    }

    /// The language behind the tag.
    pub fn language(self) -> Language {
        self.language
    }

    /// Which way this locale reads.
    pub fn direction(self) -> ReadingDirection {
        self.language.direction()
    }

    /// What a speaker of this locale calls it.
    pub fn endonym(self) -> &'static str {
        self.language.endonym()
    }

    /// Whether this is the locale everything else falls back to.
    pub fn is_base(self) -> bool {
        self.tag == BASE_LOCALE
    }
}

/// Every locale the build ships, in the order the picker lists them.
pub const SUPPORTED: &[Locale] = &[
    Locale::new("en", Language::English),
    Locale::new("es", Language::Spanish),
    Locale::new("de", Language::German),
    Locale::new("fr", Language::French),
    Locale::new("ja", Language::Japanese),
    Locale::new("ar", Language::Arabic),
];

/// The base locale, which every fallback chain ends at.
pub fn base_locale() -> Locale {
    SUPPORTED
        .iter()
        .copied()
        .find(|locale| locale.is_base())
        .expect("the base locale is in the supported set")
}

/// How a tag matched the set of shipped locales.
///
/// The chain is the same one [`Catalog`] uses for a message, applied to the
/// language itself: exact, then the same language in another region, then the
/// base. It is reported rather than implied so the language picker can say
/// "this machine asked for something this build does not have" instead of
/// silently showing a different language as though it had been chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchSource {
    /// The tag named a shipped locale exactly.
    Exact,
    /// The language was shipped under a different tag.
    BaseLanguage,
    /// Nothing matched, so the base locale is in force.
    Base,
}

impl MatchSource {
    /// One line saying what happened, for the picker and the activity log.
    pub fn note(self, tag: &str, locale: Locale) -> String {
        match self {
            MatchSource::Exact => format!("The interface is in {}.", locale.endonym()),
            MatchSource::BaseLanguage => format!(
                "'{tag}' is not a language this build ships, so it is shown in {}.",
                locale.endonym()
            ),
            MatchSource::Base => format!(
                "'{tag}' is not a language this build ships, so it is shown in the base language ({}).",
                locale.endonym()
            ),
        }
    }
}

/// A tag matched against the shipped locales.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocaleMatch {
    /// The locale in force.
    pub locale: Locale,
    /// Which rung of the chain produced it.
    pub source: MatchSource,
}

/// The tag a locale string carries, reduced to what decides anything.
///
/// A POSIX locale brings an encoding and a modifier that have nothing to do
/// with language or direction, so `ar_EG.UTF-8` is read as `ar-EG`. The
/// reduction is here rather than in the shell because the shell is not the only
/// thing that has been handed a locale by the platform.
pub fn normalize_locale_tag(raw: &str) -> String {
    let trimmed = raw.trim();
    let base = trimmed.split(['.', '@', ':']).next().unwrap_or_default();
    let base = base.replace('_', "-");
    if base.is_empty() {
        BASE_LOCALE.to_string()
    } else {
        base
    }
}

/// The primary language subtag of a tag, folded to lower case.
fn primary_subtag(tag: &str) -> String {
    normalize_locale_tag(tag)
        .split('-')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// Resolve a tag against the shipped locales, reporting which rung answered.
pub fn resolve_locale(tag: &str) -> LocaleMatch {
    let normalized = normalize_locale_tag(tag);
    if let Some(locale) = SUPPORTED
        .iter()
        .copied()
        .find(|locale| locale.tag.eq_ignore_ascii_case(&normalized))
    {
        return LocaleMatch {
            locale,
            source: MatchSource::Exact,
        };
    }
    let language = primary_subtag(&normalized);
    if let Some(locale) = SUPPORTED
        .iter()
        .copied()
        .find(|locale| locale.language.tag() == language)
    {
        return LocaleMatch {
            locale,
            source: MatchSource::BaseLanguage,
        };
    }
    LocaleMatch {
        locale: base_locale(),
        source: MatchSource::Base,
    }
}

/// The locale to start in when nothing has been chosen.
///
/// Read from the same environment variables the layout direction has always
/// been read from, so a machine that has never opened the preferences gets the
/// same answer for its layout as for its words.
pub fn locale_from_environment() -> Locale {
    let mut fallback = base_locale();
    for name in LOCALE_VARIABLES {
        let Ok(value) = std::env::var(name) else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() || value == "C" || value == "POSIX" {
            continue;
        }
        fallback = resolve_locale(value).locale;
        break;
    }
    fallback
}

/// Environment variables that name the user's locale, most specific first.
///
/// `LC_ALL` and `LC_MESSAGES` override `LANG` in the POSIX order. The first one
/// is ours and exists so the language can be exercised on a machine whose locale
/// is not the one under test.
const LOCALE_VARIABLES: &[&str] = &["VISE_UI_LOCALE", "LC_ALL", "LC_MESSAGES", "LANG"];

/* -------------------------------------------------------------------- keys */

/// One piece of translatable text.
///
/// Every key is a variant, so a key that was never declared cannot be written
/// down, and [`MessageKey::ALL`] is what a locale file is checked against. The
/// name of a variant is the identifier used in the files, which is why the
/// files are checked for it rather than for an arbitrary string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MessageKey {
    /// Window title.
    AppTitle,
    /// One line under the title.
    AppTagline,
    /// Confirmation.
    CommonOk,
    /// Refusal of a change.
    CommonCancel,
    /// Dismiss a window or a sheet.
    CommonClose,
    /// Keep a change.
    CommonSave,
    /// Put a change into effect.
    CommonApply,
    /// Put every value back.
    CommonReset,
    /// Agreement.
    CommonYes,
    /// Refusal.
    CommonNo,
    /// Add something.
    CommonAdd,
    /// Take something away.
    CommonRemove,
    /// Look at something.
    CommonOpen,
    /// Open a source.
    ActionAddSource,
    /// Scan a source.
    ActionScan,
    /// Put a title into the queue.
    ActionAddToQueue,
    /// Open the queue window.
    ActionOpenQueue,
    /// Open the preset browser.
    ActionOpenPresets,
    /// Open the preferences.
    ActionOpenPreferences,
    /// Open the preview window.
    ActionOpenPreview,
    /// The summary tab.
    TabSummary,
    /// The dimensions tab.
    TabDimensions,
    /// The filters tab.
    TabFilters,
    /// The video tab.
    TabVideo,
    /// The audio tab.
    TabAudio,
    /// The subtitles tab.
    TabSubtitles,
    /// The chapters tab.
    TabChapters,
    /// The metadata tab.
    TabMetadata,
    /// Nothing is open.
    SourceNone,
    /// Nothing has been opened yet.
    SourceChoose,
    /// A scan is running. Takes `{name}`.
    SourceScanning,
    /// A scan finished. Takes `{count}`.
    SourceScanned,
    /// The source could not be read.
    SourceFailed,
    /// The queue holds nothing.
    QueueEmpty,
    /// What the queue is holding. Takes `{count}`.
    QueueWaiting,
    /// The job being encoded.
    QueueRunning,
    /// A finished job.
    QueueFinished,
    /// A job that did not finish.
    QueueFailed,
    /// A queue held back.
    QueuePaused,
    /// Heading over the list of problems.
    ErrorTitle,
    /// The General tab.
    PrefsGeneral,
    /// The Output tab.
    PrefsOutput,
    /// The Advanced tab.
    PrefsAdvanced,
    /// The Hardware tab.
    PrefsHardware,
    /// The Updates tab.
    PrefsUpdates,
    /// The When-done tab.
    PrefsWhenDone,
    /// Heading over the theme question.
    AppearanceTheme,
    /// Heading over the direction question.
    AppearanceDirection,
    /// Heading over the language question.
    AppearanceLanguage,
    /// Follow the operating system.
    AppearanceThemeSystem,
    /// The light theme.
    AppearanceThemeLight,
    /// The dark theme.
    AppearanceThemeDark,
    /// What following the system resolves to. Takes `{asked}` and `{shown}`.
    AppearanceThemeSystemHint,
    /// What overriding to light means.
    AppearanceThemeLightHint,
    /// What overriding to dark means.
    AppearanceThemeDarkHint,
    /// How a language that reads left to right reads. Fills both `{reads}` and
    /// `{flows}`: the two are the same in every language this build ships, and
    /// inventing a key pair for one phrase in two slots would only make a
    /// translator invent a difference that is not there.
    AppearanceReadsLtr,
    /// How a language that reads right to left reads.
    AppearanceReadsRtl,
    /// Follow the language.
    AppearanceDirectionAuto,
    /// Always left to right.
    AppearanceDirectionLtr,
    /// Always right to left.
    AppearanceDirectionRtl,
    /// What following the language resolved to. Takes `{locale}`, `{reads}`,
    /// and `{flows}`.
    AppearanceDirectionAutoHint,
    /// What overriding to left to right means.
    AppearanceDirectionFixedLtr,
    /// What overriding to right to left means.
    AppearanceDirectionFixedRtl,
    /// The option that follows the machine's own language.
    LanguageFollowSystem,
    /// How much of the interface is translated. Takes `{translated}`,
    /// `{total}`, and `{base}`.
    LanguageCoverage,
    /// What a switch did. Takes `{locale}`.
    LanguageSwitched,
    /// A language this build does not ship, and which one is in force instead.
    /// Takes `{requested}` and `{locale}`.
    LanguageNotShipped,
    /// The command line was interrupted.
    CliCancelled,
    /// The container name is not one this build writes. Takes `{value}`.
    CliUnknownFormat,
    /// The video codec is not one this build writes. Takes `{value}`.
    CliUnknownVideoCodec,
    /// The audio codec is not one this build writes. Takes `{value}`.
    CliUnknownAudioCodec,
    /// The jobs file could not be read. Takes `{problem}`.
    CliReadingJobs,
    /// The transcode failed. Takes `{problem}`.
    CliTranscodeFailed,
    /// The old output could not be removed. Takes `{problem}`.
    CliRemovingOutput,
    /// The output could not be moved into place. Takes `{problem}`.
    CliRenamingOutput,
    /// The batch finished with failures. Takes `{summary}`.
    CliBatchFailures,
    /// The diagnostics window title.
    DiagnosticsTitle,
    /// The quality sweep tool.
    DiagnosticsSweep,
    /// The hardware capability report.
    DiagnosticsCapabilities,
    /// The per-encode log files.
    DiagnosticsEncodeLogs,
    /// The bundle of reports.
    DiagnosticsBundle,
    /// Start a quality sweep.
    DiagnosticsStartSweep,
    /// Stop a quality sweep.
    DiagnosticsCancelSweep,
    /// Write the queue's results out.
    DiagnosticsExportResults,
    /// Write the capability report out.
    DiagnosticsExportCapabilities,
    /// Build the diagnostic bundle.
    DiagnosticsExportBundle,
    /// Ask the machine what it can do again.
    DiagnosticsRecheck,
    /// The column a sweep result's quality value sits under.
    DiagnosticsQuality,
    /// The column a sweep result's size sits under.
    DiagnosticsSize,
    /// The column a sweep result's size against the smallest run sits under.
    DiagnosticsRelative,
    /// Confirm a range wider than this build will sweep on its own.
    DiagnosticsConfirmSweep,
    /// Write the queue's results out as a spreadsheet.
    DiagnosticsExportResultsCsv,
    /// Write the queue's results out for a program.
    DiagnosticsExportResultsJson,
}

impl MessageKey {
    /// Every key, in the order the files are written in.
    pub const ALL: [MessageKey; 93] = [
        MessageKey::AppTitle,
        MessageKey::AppTagline,
        MessageKey::CommonOk,
        MessageKey::CommonCancel,
        MessageKey::CommonClose,
        MessageKey::CommonSave,
        MessageKey::CommonApply,
        MessageKey::CommonReset,
        MessageKey::CommonYes,
        MessageKey::CommonNo,
        MessageKey::CommonAdd,
        MessageKey::CommonRemove,
        MessageKey::CommonOpen,
        MessageKey::ActionAddSource,
        MessageKey::ActionScan,
        MessageKey::ActionAddToQueue,
        MessageKey::ActionOpenQueue,
        MessageKey::ActionOpenPresets,
        MessageKey::ActionOpenPreferences,
        MessageKey::ActionOpenPreview,
        MessageKey::TabSummary,
        MessageKey::TabDimensions,
        MessageKey::TabFilters,
        MessageKey::TabVideo,
        MessageKey::TabAudio,
        MessageKey::TabSubtitles,
        MessageKey::TabChapters,
        MessageKey::TabMetadata,
        MessageKey::SourceNone,
        MessageKey::SourceChoose,
        MessageKey::SourceScanning,
        MessageKey::SourceScanned,
        MessageKey::SourceFailed,
        MessageKey::QueueEmpty,
        MessageKey::QueueWaiting,
        MessageKey::QueueRunning,
        MessageKey::QueueFinished,
        MessageKey::QueueFailed,
        MessageKey::QueuePaused,
        MessageKey::ErrorTitle,
        MessageKey::PrefsGeneral,
        MessageKey::PrefsOutput,
        MessageKey::PrefsAdvanced,
        MessageKey::PrefsHardware,
        MessageKey::PrefsUpdates,
        MessageKey::PrefsWhenDone,
        MessageKey::AppearanceTheme,
        MessageKey::AppearanceDirection,
        MessageKey::AppearanceLanguage,
        MessageKey::AppearanceThemeSystem,
        MessageKey::AppearanceThemeLight,
        MessageKey::AppearanceThemeDark,
        MessageKey::AppearanceThemeSystemHint,
        MessageKey::AppearanceThemeLightHint,
        MessageKey::AppearanceThemeDarkHint,
        MessageKey::AppearanceReadsLtr,
        MessageKey::AppearanceReadsRtl,
        MessageKey::AppearanceDirectionAuto,
        MessageKey::AppearanceDirectionLtr,
        MessageKey::AppearanceDirectionRtl,
        MessageKey::AppearanceDirectionAutoHint,
        MessageKey::AppearanceDirectionFixedLtr,
        MessageKey::AppearanceDirectionFixedRtl,
        MessageKey::LanguageFollowSystem,
        MessageKey::LanguageCoverage,
        MessageKey::LanguageSwitched,
        MessageKey::LanguageNotShipped,
        MessageKey::CliCancelled,
        MessageKey::CliUnknownFormat,
        MessageKey::CliUnknownVideoCodec,
        MessageKey::CliUnknownAudioCodec,
        MessageKey::CliReadingJobs,
        MessageKey::CliTranscodeFailed,
        MessageKey::CliRemovingOutput,
        MessageKey::CliRenamingOutput,
        MessageKey::CliBatchFailures,
        MessageKey::DiagnosticsTitle,
        MessageKey::DiagnosticsSweep,
        MessageKey::DiagnosticsCapabilities,
        MessageKey::DiagnosticsEncodeLogs,
        MessageKey::DiagnosticsBundle,
        MessageKey::DiagnosticsStartSweep,
        MessageKey::DiagnosticsCancelSweep,
        MessageKey::DiagnosticsExportResults,
        MessageKey::DiagnosticsExportCapabilities,
        MessageKey::DiagnosticsExportBundle,
        MessageKey::DiagnosticsRecheck,
        MessageKey::DiagnosticsQuality,
        MessageKey::DiagnosticsSize,
        MessageKey::DiagnosticsRelative,
        MessageKey::DiagnosticsConfirmSweep,
        MessageKey::DiagnosticsExportResultsCsv,
        MessageKey::DiagnosticsExportResultsJson,
    ];

    /// The identifier the locale files use.
    ///
    /// It is the variant name folded to the file's dotted convention, and
    /// [`MessageKey::of_name`] is its exact inverse, so a file and the enum
    /// cannot drift apart: the round trip is what the validation checks.
    pub fn name(self) -> &'static str {
        NAMES[self as usize]
    }

    /// The key a name in a file refers to.
    pub fn of_name(name: &str) -> Option<Self> {
        MessageKey::ALL.into_iter().find(|key| key.name() == name)
    }

    /// A readable name for the key, used when no locale has it.
    ///
    /// The guarantee this exists for: a key that is somehow in no file at all
    /// must still produce something a person can read. Returning the key would
    /// put `QueueWaiting` on a button, which is worse than an English sentence
    /// because it looks like a bug and is not one the user can act on.
    pub fn label(self) -> String {
        let words = self.name().replace('.', " ");
        let mut out = String::with_capacity(words.len());
        for (index, word) in words.split(' ').enumerate() {
            if index == 0 {
                out.push_str(&capitalise(word));
            } else {
                out.push(' ');
                out.push_str(word);
            }
        }
        out
    }

    /// The placeholders this key's text is allowed to carry.
    pub fn placeholders(self) -> &'static [&'static str] {
        match self {
            MessageKey::SourceScanning => &["name"],
            MessageKey::SourceScanned | MessageKey::QueueWaiting => &["count"],
            MessageKey::AppearanceDirectionAutoHint => &["locale", "reads", "flows"],
            MessageKey::AppearanceThemeSystemHint => &["asked", "shown"],
            MessageKey::LanguageCoverage => &["translated", "total", "base"],
            MessageKey::LanguageSwitched => &["locale"],
            MessageKey::LanguageNotShipped => &["requested", "locale"],
            MessageKey::CliUnknownFormat
            | MessageKey::CliUnknownVideoCodec
            | MessageKey::CliUnknownAudioCodec => &["value"],
            MessageKey::CliReadingJobs
            | MessageKey::CliTranscodeFailed
            | MessageKey::CliRemovingOutput
            | MessageKey::CliRenamingOutput => &["problem"],
            MessageKey::CliBatchFailures => &["summary"],
            _ => &[],
        }
    }
}

/// Variant names in declaration order, so `name` and `of_name` cannot drift.
///
/// The length is checked against [`MessageKey::ALL`] by a test rather than by
/// the type system, because an array length is not part of an enum's identity.
const NAMES: [&str; 93] = [
    "app.title",
    "app.tagline",
    "common.ok",
    "common.cancel",
    "common.close",
    "common.save",
    "common.apply",
    "common.reset",
    "common.yes",
    "common.no",
    "common.add",
    "common.remove",
    "common.open",
    "action.addSource",
    "action.scan",
    "action.addToQueue",
    "action.openQueue",
    "action.openPresets",
    "action.openPreferences",
    "action.openPreview",
    "tab.summary",
    "tab.dimensions",
    "tab.filters",
    "tab.video",
    "tab.audio",
    "tab.subtitles",
    "tab.chapters",
    "tab.metadata",
    "source.none",
    "source.choose",
    "source.scanning",
    "source.scanned",
    "source.failed",
    "queue.empty",
    "queue.waiting",
    "queue.running",
    "queue.finished",
    "queue.failed",
    "queue.paused",
    "error.title",
    "prefs.general",
    "prefs.output",
    "prefs.advanced",
    "prefs.hardware",
    "prefs.updates",
    "prefs.whenDone",
    "appearance.theme",
    "appearance.direction",
    "appearance.language",
    "appearance.themeSystem",
    "appearance.themeLight",
    "appearance.themeDark",
    "appearance.themeSystemHint",
    "appearance.themeLightHint",
    "appearance.themeDarkHint",
    "appearance.readsLtr",
    "appearance.readsRtl",
    "appearance.directionAuto",
    "appearance.directionLtr",
    "appearance.directionRtl",
    "appearance.directionAutoHint",
    "appearance.directionFixedLtr",
    "appearance.directionFixedRtl",
    "language.followSystem",
    "language.coverage",
    "language.switched",
    "language.notShipped",
    "cli.cancelled",
    "cli.unknownFormat",
    "cli.unknownVideoCodec",
    "cli.unknownAudioCodec",
    "cli.readingJobs",
    "cli.transcodeFailed",
    "cli.removingOutput",
    "cli.renamingOutput",
    "cli.batchFailures",
    "diagnostics.title",
    "diagnostics.sweep",
    "diagnostics.capabilities",
    "diagnostics.encodeLogs",
    "diagnostics.bundle",
    "diagnostics.startSweep",
    "diagnostics.cancelSweep",
    "diagnostics.exportResults",
    "diagnostics.exportCapabilities",
    "diagnostics.exportBundle",
    "diagnostics.recheck",
    "diagnostics.quality",
    "diagnostics.size",
    "diagnostics.relative",
    "diagnostics.confirmSweep",
    "diagnostics.exportResultsCsv",
    "diagnostics.exportResultsJson",
];

/// Upper-case the first letter of a word, leaving the rest as written.
fn capitalise(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/* ----------------------------------------------------------------- resources */

/// One locale file, as it was embedded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageTable {
    /// The locale the file is for.
    pub tag: &'static str,
    /// The file, as text.
    pub text: &'static str,
}

const EN_MESSAGES: &str = include_str!("../data/locales/en.json");
const ES_MESSAGES: &str = include_str!("../data/locales/es.json");
const DE_MESSAGES: &str = include_str!("../data/locales/de.json");
const FR_MESSAGES: &str = include_str!("../data/locales/fr.json");
const JA_MESSAGES: &str = include_str!("../data/locales/ja.json");
const AR_MESSAGES: &str = include_str!("../data/locales/ar.json");

/// Every message file the build embeds, one per shipped locale.
///
/// Ordered by the locale list so the picker, the validation, and the report of
/// coverage all walk the same order.
pub fn message_tables() -> &'static [MessageTable] {
    static TABLES: &[MessageTable] = &[
        MessageTable {
            tag: "en",
            text: EN_MESSAGES,
        },
        MessageTable {
            tag: "es",
            text: ES_MESSAGES,
        },
        MessageTable {
            tag: "de",
            text: DE_MESSAGES,
        },
        MessageTable {
            tag: "fr",
            text: FR_MESSAGES,
        },
        MessageTable {
            tag: "ja",
            text: JA_MESSAGES,
        },
        MessageTable {
            tag: "ar",
            text: AR_MESSAGES,
        },
    ];
    TABLES
}

/// A message file, parsed.
///
/// The map is read by a visitor rather than by `BTreeMap` directly because a
/// duplicate key in a translation file is otherwise invisible: serde keeps the
/// last one and the translator who fixed the first spelling has no way of
/// knowing their work was thrown away.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MessageFile {
    entries: BTreeMap<String, String>,
}

impl MessageFile {
    /// The messages, by key name.
    pub fn entries(&self) -> &BTreeMap<String, String> {
        &self.entries
    }

    /// The text stored under a name.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.entries.get(name).map(String::as_str)
    }

    /// Whether the file holds nothing at all.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Read a file, saying which file and what was wrong with it.
    pub fn parse(tag: &str, text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|problem| format!("{tag}: {problem}"))
    }
}

struct MessageFileVisitor;

impl<'de> Visitor<'de> for MessageFileVisitor {
    type Value = MessageFile;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a JSON object of message names to messages")
    }

    fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut entries: BTreeMap<String, String> = BTreeMap::new();
        while let Some(name) = access.next_key::<String>()? {
            let text: String = access.next_value()?;
            if entries.insert(name.clone(), text).is_some() {
                return Err(de::Error::custom(format!(
                    "'{name}' appears more than once, so one of the two would be lost"
                )));
            }
        }
        Ok(MessageFile { entries })
    }
}

impl<'de> Deserialize<'de> for MessageFile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(MessageFileVisitor)
    }
}

/* ------------------------------------------------------------------ catalog */

/// Which file a resolved message came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageSource {
    /// The locale itself.
    Locale,
    /// The same language under another tag.
    BaseLanguage,
    /// The base locale.
    Base,
    /// Nothing had it, so the key's own readable name was used.
    Label,
}

impl MessageSource {
    /// Whether this is the locale's own text, as opposed to a stand-in.
    pub fn is_translated(self) -> bool {
        matches!(self, MessageSource::Locale)
    }

    /// One line saying what happened to one key.
    pub fn note(self, key: &str, requested: &str) -> String {
        match self {
            MessageSource::Locale => format!("'{key}' is translated for {requested}."),
            MessageSource::BaseLanguage => format!(
                "'{key}' has no text for {requested}, so the language's base text is shown."
            ),
            MessageSource::Base => {
                format!("'{key}' has no text for {requested}, so the base language is shown.")
            }
            MessageSource::Label => {
                format!("'{key}' is in no locale file, so the key's own name is shown.")
            }
        }
    }
}

/// A message, and where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedMessage {
    /// The text to show.
    pub text: String,
    /// Which file answered.
    pub source: MessageSource,
}

/// One key the locale did not have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gap {
    /// The key.
    pub key: MessageKey,
    /// Which file answered instead.
    pub source: MessageSource,
}

/// One locale's resolved messages.
///
/// Every key has an answer, and which answer it is has been recorded, so
/// nothing here can quietly disagree with what is on screen: the text, the
/// fallback it used, and the coverage figure come from one walk of one table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    locale: Locale,
    requested: String,
    match_source: MatchSource,
    messages: Vec<(MessageKey, ResolvedMessage)>,
}

impl Catalog {
    /// The base locale's catalog, with nothing resolved through a fallback.
    pub fn base() -> &'static Catalog {
        base_catalog()
    }

    /// The locale these messages are in.
    pub fn locale(&self) -> Locale {
        self.locale
    }

    /// The locale's tag.
    pub fn locale_tag(&self) -> &'static str {
        self.locale.tag()
    }

    /// Every message in this locale, by key name.
    ///
    /// For the layer that cannot hold a catalog of its own — the page, which has a
    /// document and no Rust — and which therefore needs to be handed the words rather
    /// than asked to compose them. The page addresses them by name and leaves an
    /// element alone when the name is not in here, so a missing message costs a
    /// sentence in the base language rather than an empty control.
    pub fn strings(&self) -> BTreeMap<String, String> {
        self.messages
            .iter()
            .map(|(key, message)| (key.name().to_string(), message.text.clone()))
            .collect()
    }

    /// One message, from the locale or from whichever rung answered.
    pub fn message(&self, key: MessageKey) -> &ResolvedMessage {
        &self.messages[key as usize].1
    }

    /// One message's text.
    pub fn text(&self, key: MessageKey) -> &str {
        &self.message(key).text
    }

    /// One message's text with `{name}` placeholders filled in.
    ///
    /// A placeholder the caller did not supply is left exactly as written rather
    /// than replaced with nothing: the same rule the pipeline applies to `%name%`
    /// in a command, and for the same reason — a hole in a message is a fact
    /// somebody has to be able to see.
    pub fn format(&self, key: MessageKey, args: &[(&str, &str)]) -> String {
        let text = self.text(key);
        if !text.contains('{') {
            return text.to_string();
        }
        let mut out = text.to_string();
        for (name, value) in args {
            out = out.replace(&format!("{{{name}}}"), value);
        }
        out
    }

    /// How many of the keys are the locale's own text.
    pub fn translated(&self) -> usize {
        self.messages
            .iter()
            .filter(|(_, message)| message.source.is_translated())
            .count()
    }

    /// How many keys there are in total.
    pub fn total(&self) -> usize {
        self.messages.len()
    }

    /// Whether every key is the locale's own text.
    pub fn is_complete(&self) -> bool {
        self.translated() == self.total()
    }

    /// Every key that is not the locale's own text, and what stood in for it.
    ///
    /// This is how a gap stays visible. A half-translated locale is not a
    /// failure — the fallback chain exists precisely so it is usable — but it is
    /// a list somebody has to be able to print, and the window prints it in the
    /// activity log when a language is chosen.
    pub fn gaps(&self) -> Vec<Gap> {
        self.messages
            .iter()
            .filter(|(_, message)| !message.source.is_translated())
            .map(|(key, message)| Gap {
                key: *key,
                source: message.source,
            })
            .collect()
    }

    /// One line per gap, naming the key and what answered it.
    pub fn gap_notes(&self) -> Vec<String> {
        self.gaps()
            .into_iter()
            .map(|gap| gap.source.note(gap.key.name(), &self.requested))
            .collect()
    }

    /// The coverage sentence, in the language in force.
    pub fn coverage(&self) -> String {
        let translated = self.translated().to_string();
        let total = self.total().to_string();
        self.format(
            MessageKey::LanguageCoverage,
            &[
                ("translated", &translated),
                ("total", &total),
                ("base", base_locale().endonym()),
            ],
        )
    }

    /// Build a catalog out of tables this build did not embed.
    ///
    /// The chain is the same one the shipped locales go through and is walked
    /// per key rather than per file, because a locale file brought half forward
    /// is exactly the case this has to answer. The only thing that changes is
    /// where the text came from, which is what makes a deliberately incomplete
    /// locale testable without shipping one.
    pub fn build(requested: &str, tables: &[(String, MessageFile)]) -> Catalog {
        let matched = resolve_locale(requested);
        let wanted = normalize_locale_tag(requested);
        // Rung one: the tag itself, spelled as it was asked for.
        let exact = tables
            .iter()
            .find(|(tag, _)| normalize_locale_tag(tag).eq_ignore_ascii_case(&wanted));
        // Rung two: the same language under another tag, which is how a regional
        // request reads its own language rather than English.
        let language = tables.iter().find(|(tag, _)| {
            primary_subtag(tag) == primary_subtag(&wanted)
                && primary_subtag(tag) != BASE_LOCALE
                && !normalize_locale_tag(tag).eq_ignore_ascii_case(&wanted)
        });
        // Rung three: the base locale, the one file every build has.
        let base = tables
            .iter()
            .find(|(tag, _)| primary_subtag(tag) == BASE_LOCALE);
        let messages = MessageKey::ALL
            .into_iter()
            .map(|key| {
                let (text, source) =
                    if let Some(text) = exact.and_then(|(_, file)| file.get(key.name())) {
                        (text.to_string(), MessageSource::Locale)
                    } else if let Some(text) = language.and_then(|(_, file)| file.get(key.name())) {
                        (text.to_string(), MessageSource::BaseLanguage)
                    } else if let Some(text) = base.and_then(|(_, file)| file.get(key.name())) {
                        (text.to_string(), MessageSource::Base)
                    } else {
                        (key.label(), MessageSource::Label)
                    };
                (key, ResolvedMessage { text, source })
            })
            .collect();
        Catalog {
            locale: matched.locale,
            requested: wanted,
            match_source: matched.source,
            messages,
        }
    }
}

/// Every embedded message file, read once.
fn embedded_tables() -> &'static [(String, MessageFile)] {
    static TABLES: OnceLock<Vec<(String, MessageFile)>> = OnceLock::new();
    TABLES.get_or_init(|| {
        message_tables()
            .iter()
            .map(|table| {
                let file = MessageFile::parse(table.tag, table.text).unwrap_or_else(|problem| {
                    panic!("a shipped message file is unreadable: {problem}")
                });
                (table.tag.to_string(), file)
            })
            .collect()
    })
}

/// The base locale's file, read once.
fn base_file() -> &'static MessageFile {
    static BASE: OnceLock<MessageFile> = OnceLock::new();
    BASE.get_or_init(|| {
        embedded_tables()
            .iter()
            .find(|(tag, _)| primary_subtag(tag) == BASE_LOCALE)
            .map(|(_, file)| file.clone())
            .expect("the base locale has a file")
    })
}

fn base_catalog() -> &'static Catalog {
    static BASE: OnceLock<Catalog> = OnceLock::new();
    BASE.get_or_init(|| Catalog::build(BASE_LOCALE, embedded_tables()))
}

fn catalog_for_tag(tag: &str) -> Result<&'static Catalog, String> {
    static CATALOGS: OnceLock<Vec<(&'static str, Catalog)>> = OnceLock::new();
    let catalogs = CATALOGS.get_or_init(|| {
        let tables = embedded_tables();
        message_tables()
            .iter()
            .map(|table| (table.tag, Catalog::build(table.tag, tables)))
            .collect()
    });
    let matched = resolve_locale(tag);
    catalogs
        .iter()
        .find(|(tag, _)| tag.eq_ignore_ascii_case(matched.locale.tag()))
        .map(|(_, catalog)| catalog)
        .ok_or_else(|| {
            format!(
                "'{tag}' resolved to {} but this build has no message file for it.",
                matched.locale.tag()
            )
        })
}

/// A catalog together with the answer to "which language did that tag reach?".
///
/// The catalog is per shipped locale, so it cannot by itself say whether the
/// tag the user typed named that locale or reached it through the chain. This
/// is the pair, and it is the one callers want: the picker needs the catalog to
/// draw with and the chain's answer to explain itself with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCatalog {
    /// The catalog the resolved locale's file resolved into.
    pub catalog: &'static Catalog,
    /// The tag that was asked for, reduced.
    pub requested: String,
    /// The locale in force.
    pub locale: Locale,
    /// Which rung of the chain answered.
    pub match_source: MatchSource,
}

impl ResolvedCatalog {
    /// Whether the tag asked for is the locale in force.
    pub fn is_exact(&self) -> bool {
        self.match_source == MatchSource::Exact
    }

    /// One line saying what happened, for a log line or the picker.
    pub fn note(&self) -> String {
        self.match_source.note(&self.requested, self.locale)
    }

    /// How many of this catalog's keys are the locale's own text.
    pub fn translated(&self) -> usize {
        self.catalog.translated()
    }

    /// How many keys there are in total.
    pub fn total(&self) -> usize {
        self.catalog.total()
    }

    /// Every key that is not the locale's own text.
    pub fn gaps(&self) -> Vec<Gap> {
        self.catalog.gaps()
    }

    /// One line per gap.
    pub fn gap_notes(&self) -> Vec<String> {
        self.catalog.gap_notes()
    }

    /// The coverage sentence, in the language in force.
    pub fn coverage(&self) -> String {
        self.catalog.coverage()
    }
}

/// Resolve a tag to the catalog it should be drawn from, and say how.
///
/// This is the only entry point, because the two answers cannot come apart: a
/// caller holding a catalog without the chain's answer would have to guess
/// whether the language it is showing is the one that was asked for.
pub fn resolve_catalog(tag: &str) -> ResolvedCatalog {
    let matched = resolve_locale(tag);
    let catalog = catalog_for_tag(matched.locale.tag())
        .unwrap_or_else(|problem| panic!("a shipped locale has a catalog: {problem}"));
    ResolvedCatalog {
        catalog,
        requested: normalize_locale_tag(tag),
        locale: matched.locale,
        match_source: matched.source,
    }
}

/* --------------------------------------------------------------- validation */

/// Something wrong with a shipped resource, named precisely enough to fix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// The locale the problem is in, or the resource it is about.
    pub resource: String,
    /// One line saying what is wrong.
    pub detail: String,
}

impl Problem {
    /// One line, for a test failure or an activity log.
    pub fn note(&self) -> String {
        format!("{}: {}", self.resource, self.detail)
    }
}

/// Check every shipped message file against the key set.
///
/// This is the load-time validation the build runs in its tests, and the reason
/// a key that was added without a translation fails the build rather than
/// reaching a screen. It is deliberately total: a locale that has not been
/// brought forward is reported per key, not summarised.
pub fn validate_message_files() -> Vec<Problem> {
    let mut problems = Vec::new();
    for table in message_tables() {
        let label = format!("messages/{}", table.tag);
        let file = match MessageFile::parse(table.tag, table.text) {
            Ok(file) => file,
            Err(problem) => {
                problems.push(Problem {
                    resource: label,
                    detail: problem,
                });
                continue;
            }
        };
        let base = (table.tag != BASE_LOCALE).then_some(base_file());
        problems.extend(validate_message_file(&label, &file, base));
    }
    problems
}

/// Everything wrong with one locale file.
///
/// Split out from [`validate_message_files`] and public so that a test can ask
/// the same questions about a file that is deliberately wrong, rather than
/// asserting against a second, weaker copy of the rules.
pub fn validate_message_file(
    label: &str,
    file: &MessageFile,
    base: Option<&MessageFile>,
) -> Vec<Problem> {
    let mut problems = Vec::new();
    for key in MessageKey::ALL {
        let Some(text) = file.get(key.name()) else {
            problems.push(Problem {
                resource: label.to_string(),
                detail: format!("'{}' has no text", key.name()),
            });
            continue;
        };
        let trimmed = text.trim();
        if trimmed.is_empty() {
            problems.push(Problem {
                resource: label.to_string(),
                detail: format!("'{}' is blank", key.name()),
            });
        }
        if trimmed == key.name() {
            problems.push(Problem {
                resource: label.to_string(),
                detail: format!("'{}' is the key itself rather than a sentence", key.name()),
            });
        }
        if trimmed.contains("{}") {
            problems.push(Problem {
                resource: label.to_string(),
                detail: format!("'{}' has an unnamed placeholder", key.name()),
            });
        }
        let wanted: BTreeSet<&str> = key.placeholders().iter().copied().collect();
        let found = placeholders(text);
        if wanted != found {
            problems.push(Problem {
                resource: label.to_string(),
                detail: format!(
                    "'{}' fills {found:?} where {wanted:?} is wanted",
                    key.name()
                ),
            });
        }
    }
    for name in file.entries().keys() {
        if MessageKey::of_name(name).is_none() {
            problems.push(Problem {
                resource: label.to_string(),
                detail: format!("'{name}' is not a key this build knows"),
            });
        }
    }
    // A translation that is still the base sentence is the commonest silent
    // failure in a table like this, so it is worth one question rather than
    // none. Two kinds of text are exempt, and both are exempt for the same
    // reason: they are not sentences that could have been translated. A proper
    // noun is the same in every language — the application name is a
    // trademark, and a build that translated it would be showing another
    // product. A single word is often the same word in several languages
    // ("Audio", "OK"), and asking a translator to change it would only make the
    // table worse.
    if let Some(base) = base {
        for key in MessageKey::ALL {
            let (Some(text), Some(base_text)) = (file.get(key.name()), base.get(key.name())) else {
                continue;
            };
            let is_sentence = base_text.split_whitespace().count() > 1;
            if text == base_text && is_sentence && !is_a_proper_noun(key) {
                problems.push(Problem {
                    resource: label.to_string(),
                    detail: format!(
                        "'{}' is still the base sentence rather than a translation",
                        key.name()
                    ),
                });
            }
        }
    }
    problems
}

/// Whether a key's text is a name that must not be translated.
///
/// The application name is one: it is a trademark, and a build that shows a
/// translated name for it is showing a different product.
fn is_a_proper_noun(key: MessageKey) -> bool {
    matches!(key, MessageKey::AppTitle)
}

/// The `{name}` placeholders in a message, in order.
pub fn placeholders(text: &str) -> BTreeSet<&str> {
    let mut found = BTreeSet::new();
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'{' {
            index += 1;
            continue;
        }
        let Some(end) = text[index + 1..].find('}').map(|at| index + 1 + at) else {
            break;
        };
        found.insert(&text[index + 1..end]);
        index = end + 1;
    }
    found
}

/* ------------------------------------------------------- preset descriptions */

/// One preset description, in a language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetDescription {
    /// The description to show.
    pub text: String,
    /// Which file answered.
    pub source: MessageSource,
}

/// A preset description file, as it was embedded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DescriptionTable {
    /// The locale the file is for.
    pub tag: &'static str,
    /// Preset identifiers to descriptions.
    pub text: &'static str,
}

const ES_DESCRIPTIONS: &str = include_str!("../data/preset-descriptions/es.json");
const DE_DESCRIPTIONS: &str = include_str!("../data/preset-descriptions/de.json");
const FR_DESCRIPTIONS: &str = include_str!("../data/preset-descriptions/fr.json");
const JA_DESCRIPTIONS: &str = include_str!("../data/preset-descriptions/ja.json");
const AR_DESCRIPTIONS: &str = include_str!("../data/preset-descriptions/ar.json");

/// The preset description files this build embeds.
///
/// A subset of the catalog on purpose. The catalog is the base text for every
/// preset, so a preset nobody has translated is still a preset somebody can
/// read; what these files add is coverage where a translator has done the work,
/// and what they must not do is leave a preset with nothing at all.
pub fn description_tables() -> &'static [DescriptionTable] {
    static TABLES: &[DescriptionTable] = &[
        DescriptionTable {
            tag: "es",
            text: ES_DESCRIPTIONS,
        },
        DescriptionTable {
            tag: "de",
            text: DE_DESCRIPTIONS,
        },
        DescriptionTable {
            tag: "fr",
            text: FR_DESCRIPTIONS,
        },
        DescriptionTable {
            tag: "ja",
            text: JA_DESCRIPTIONS,
        },
        DescriptionTable {
            tag: "ar",
            text: AR_DESCRIPTIONS,
        },
    ];
    TABLES
}

/// The translated description of a preset, if this build has one.
///
/// The chain is the locale, then the same language under another tag, then
/// nothing: the caller answers with the catalog's own description, which is the
/// base text of the same sentence rather than a different one.
pub fn preset_description(tag: &str, id: &str) -> Option<PresetDescription> {
    let matched = resolve_locale(tag);
    if matched.locale.is_base() {
        return None;
    }
    let wanted = matched.locale.tag();
    for table in description_tables() {
        if table.tag != wanted {
            continue;
        }
        let file = MessageFile::parse(table.tag, table.text).ok()?;
        let text = file.get(id)?.to_string();
        return Some(PresetDescription {
            text,
            source: MessageSource::Locale,
        });
    }
    None
}

/// Check that every translated description belongs to a preset in the catalog.
///
/// A translation of an identifier the catalog does not have is a sentence nobody
/// will ever be shown, and it is the kind of mistake that survives until a
/// preset is renamed.
pub fn validate_description_files() -> Vec<Problem> {
    let mut problems = Vec::new();
    for table in description_tables() {
        let label = format!("preset-descriptions/{}", table.tag);
        let file = match MessageFile::parse(table.tag, table.text) {
            Ok(file) => file,
            Err(problem) => {
                problems.push(Problem {
                    resource: label,
                    detail: problem,
                });
                continue;
            }
        };
        for (id, text) in file.entries() {
            if crate::catalog::builtin_entry(id).is_none() {
                problems.push(Problem {
                    resource: label.clone(),
                    detail: format!("'{id}' is not a preset in the catalog"),
                });
            }
            if text.trim().is_empty() {
                problems.push(Problem {
                    resource: label.clone(),
                    detail: format!("'{id}' has a blank description"),
                });
            }
        }
        if file.is_empty() {
            problems.push(Problem {
                resource: label,
                detail: "no preset has a translated description".to_string(),
            });
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file with every key, taken from the base one.
    fn complete_copy(tag: &str) -> MessageFile {
        MessageFile {
            entries: base_file()
                .entries()
                .iter()
                .map(|(name, text)| {
                    (
                        name.clone(),
                        if tag == BASE_LOCALE {
                            text.clone()
                        } else {
                            format!("{text} ({tag})")
                        },
                    )
                })
                .collect(),
        }
    }

    #[test]
    fn every_key_has_a_name_and_every_name_is_a_key() {
        assert_eq!(
            MessageKey::ALL.len(),
            NAMES.len(),
            "the name table is short"
        );
        for key in MessageKey::ALL {
            assert_eq!(
                MessageKey::of_name(key.name()),
                Some(key),
                "{} does not round trip",
                key.name()
            );
            assert_eq!(key, MessageKey::ALL[key as usize], "order is the index");
        }
        assert_eq!(MessageKey::of_name("nope.nothing"), None);
        let names: BTreeSet<&str> = MessageKey::ALL.into_iter().map(|key| key.name()).collect();
        assert_eq!(names.len(), MessageKey::ALL.len(), "names are unique");
    }

    #[test]
    fn the_shipped_files_are_complete_and_well_formed() {
        let problems = validate_message_files();
        assert!(problems.is_empty(), "{:#?}", problems);
    }

    #[test]
    fn every_shipped_locale_resolves_every_key_its_own_way() {
        for table in message_tables() {
            let resolved = resolve_catalog(table.tag);
            assert!(resolved.is_exact(), "{} is exact", table.tag);
            assert_eq!(resolved.locale.tag(), table.tag);
            assert_eq!(resolved.total(), MessageKey::ALL.len());
            assert!(
                resolved.catalog.is_complete(),
                "{} falls back for {:?}",
                table.tag,
                resolved.gap_notes()
            );
        }
    }

    #[test]
    fn a_resolved_catalog_carries_both_answers_together() {
        let resolved = resolve_catalog("kl");
        assert_eq!(resolved.requested, "kl");
        assert_eq!(resolved.locale.tag(), BASE_LOCALE);
        assert_eq!(resolved.match_source, MatchSource::Base);
        assert!(!resolved.is_exact());
        assert!(resolved.catalog.is_complete());
        assert!(resolved.note().contains("not a language this build ships"));
        assert_eq!(resolved.translated(), resolved.total());
    }

    #[test]
    fn a_key_no_file_has_is_named_rather_than_shown_as_a_key() {
        let empty = MessageFile::default();
        let catalog = Catalog::build("en", &[("en".to_string(), empty)]);
        let message = catalog.message(MessageKey::QueueWaiting);
        assert_eq!(message.source, MessageSource::Label);
        assert_eq!(message.text, "Queue waiting");
        assert!(!message.text.contains("QueueWaiting"));
        assert!(
            !message.text.contains('.'),
            "a key is never shown as written"
        );
    }

    #[test]
    fn the_fallback_chain_is_exact_then_language_then_base() {
        assert_eq!(resolve_locale("fr").source, MatchSource::Exact);
        assert_eq!(resolve_locale("FR").locale.tag(), "fr");
        assert_eq!(resolve_locale("de_AT").locale.tag(), "de");
        assert_eq!(resolve_locale("de_AT").source, MatchSource::BaseLanguage);
        assert_eq!(resolve_locale("fr_CA.UTF-8").locale.tag(), "fr");
        let nowhere = resolve_locale("kl");
        assert_eq!(nowhere.locale.tag(), BASE_LOCALE);
        assert_eq!(nowhere.source, MatchSource::Base);
        assert_eq!(resolve_locale("").locale.tag(), BASE_LOCALE);
        assert_eq!(resolve_locale("C").locale.tag(), BASE_LOCALE);
    }

    #[test]
    fn an_incomplete_locale_falls_back_and_says_so() {
        let base = complete_copy(BASE_LOCALE);
        let mut partial = complete_copy("es");
        partial.entries.remove("queue.paused");
        partial.entries.remove("tab.metadata");
        let catalog = Catalog::build(
            "es",
            &[
                ("en".to_string(), base.clone()),
                ("es".to_string(), partial),
            ],
        );
        assert!(!catalog.is_complete());
        assert_eq!(catalog.translated(), MessageKey::ALL.len() - 2);
        let gaps = catalog.gaps();
        assert_eq!(gaps.len(), 2);
        assert!(gaps.iter().all(|gap| gap.source == MessageSource::Base));
        // The text still comes from the base language, never from the key.
        assert_eq!(
            catalog.text(MessageKey::QueuePaused),
            base.get("queue.paused").expect("base text")
        );
        assert!(!catalog
            .text(MessageKey::QueuePaused)
            .contains("queue.paused"));
        let notes = catalog.gap_notes();
        assert_eq!(notes.len(), 2);
        assert!(notes[0].contains("base language"), "{:?}", notes);
        assert!(catalog
            .coverage()
            .contains(&format!("{} of", MessageKey::ALL.len() - 2)));
    }

    #[test]
    fn a_region_tag_reads_its_own_language_before_the_base() {
        let base = complete_copy(BASE_LOCALE);
        let mut region = complete_copy("de-AT");
        region.entries.remove("tab.video");
        // With no file for the language itself, the base answers.
        let catalog = Catalog::build(
            "de-AT",
            &[
                ("en".to_string(), base.clone()),
                ("de-AT".to_string(), region.clone()),
            ],
        );
        assert_eq!(
            catalog.message(MessageKey::TabVideo).source,
            MessageSource::Base
        );
        // With one, the language answers rather than English, and the fact that
        // it was a rung of the chain rather than the tag itself is recorded.
        let catalog = Catalog::build(
            "de-AT",
            &[
                ("en".to_string(), base),
                ("de".to_string(), complete_copy("de")),
                ("de-AT".to_string(), region),
            ],
        );
        let message = catalog.message(MessageKey::TabVideo);
        assert_eq!(message.source, MessageSource::BaseLanguage);
        assert!(message.text.ends_with("(de)"), "{}", message.text);
        // The one key the regional file was missing is the only gap, and it is
        // reported rather than quietly filled.
        let gaps = catalog.gaps();
        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0].key, MessageKey::TabVideo);
        assert_eq!(gaps[0].source, MessageSource::BaseLanguage);
        // The tag's own answer to "which language is this" is the language it
        // was resolved to, which is a different question from the one above.
        assert_eq!(resolve_locale("de-AT").source, MatchSource::BaseLanguage);
        assert_eq!(catalog.locale.tag(), "de");
    }

    #[test]
    fn a_duplicate_key_is_refused_rather_than_silently_dropped() {
        let duplicate = r#"{"tab.video":"one","tab.video":"two"}"#;
        let error = MessageFile::parse("xx", duplicate).expect_err("a duplicate is not a file");
        assert!(error.contains("more than once"), "{error}");
        assert!(error.contains("tab.video"), "{error}");
    }

    #[test]
    fn a_malformed_file_is_refused_with_the_file_named() {
        let error = MessageFile::parse("xx", "{").expect_err("not json");
        assert!(error.starts_with("xx:"), "{error}");
        let error = MessageFile::parse("xx", r#"{"tab.video":7}"#).expect_err("not a string");
        assert!(error.starts_with("xx:"), "{error}");
    }

    #[test]
    fn a_placeholder_that_does_not_match_the_key_is_a_problem() {
        let mut file = complete_copy(BASE_LOCALE);
        file.entries
            .insert("source.scanned".to_string(), "Found them".to_string());
        file.entries
            .insert("tab.video".to_string(), "Video {name}".to_string());
        file.entries
            .insert("app.title".to_string(), "   ".to_string());
        file.entries
            .insert("tab.audio".to_string(), "tab.audio".to_string());
        file.entries
            .insert("queue.paused".to_string(), "Paused {}".to_string());
        let problems = validate_message_file("messages/xx", &file, None);
        let details: Vec<&str> = problems.iter().map(|p| p.detail.as_str()).collect();
        assert!(
            details
                .iter()
                .any(|d| d.contains("'source.scanned' fills {} where")),
            "{details:#?}"
        );
        assert!(
            details
                .iter()
                .any(|d| d.contains("'tab.video' fills {\"name\"}")),
            "{details:#?}"
        );
        assert!(
            details.iter().any(|d| d.contains("'app.title' is blank")),
            "{details:#?}"
        );
        assert!(
            details.iter().any(|d| d.contains("is the key itself")),
            "{details:#?}"
        );
        assert!(
            details.iter().any(|d| d.contains("unnamed placeholder")),
            "{details:#?}"
        );
    }

    #[test]
    fn a_missing_key_and_an_unknown_key_are_both_reported() {
        let mut file = complete_copy("xx");
        file.entries.remove("tab.video");
        file.entries
            .insert("tab.nonsense".to_string(), "No".to_string());
        let problems = validate_message_file("messages/xx", &file, None);
        let details: Vec<&str> = problems.iter().map(|p| p.detail.as_str()).collect();
        assert!(
            details
                .iter()
                .any(|d| d.contains("'tab.video' has no text")),
            "{details:#?}"
        );
        assert!(
            details
                .iter()
                .any(|d| d.contains("'tab.nonsense' is not a key this build knows")),
            "{details:#?}"
        );
    }

    #[test]
    fn a_translation_that_is_still_the_base_sentence_is_reported() {
        let base = base_file()
            .get("source.choose")
            .expect("base text")
            .to_string();
        let mut file = complete_copy("xx");
        file.entries
            .insert("source.choose".to_string(), base.clone());
        let problems = validate_message_file("messages/xx", &file, Some(base_file()));
        assert!(
            problems
                .iter()
                .any(|p| p.detail.contains("still the base sentence")),
            "{problems:#?}"
        );
        // A word that happens to be the same in both languages is not a missing
        // translation, so asking for it would only make the table worse.
        let mut file = complete_copy("xx");
        file.entries
            .insert("tab.video".to_string(), "Video".to_string());
        let problems = validate_message_file("messages/xx", &file, Some(base_file()));
        assert!(
            !problems
                .iter()
                .any(|p| p.detail.contains("still the base sentence")),
            "{problems:#?}"
        );
        // The application name is the same in every language, so it is exempt
        // as well.
        let mut file = complete_copy("xx");
        file.entries.insert(
            "app.title".to_string(),
            base_file().get("app.title").expect("base").to_string(),
        );
        let problems = validate_message_file("messages/xx", &file, Some(base_file()));
        assert!(
            !problems
                .iter()
                .any(|p| p.detail.contains("still the base sentence")),
            "{problems:#?}"
        );
    }

    #[test]
    fn the_shipped_catalogs_answer_every_question_about_direction() {
        let arabic = Language::of_tag("ar").expect("arabic is shipped");
        assert_eq!(arabic.direction(), ReadingDirection::Rtl);
        assert_eq!(arabic.endonym(), "العربية");
        let english = Language::of_tag("en-GB").expect("english is shipped");
        assert_eq!(english.direction(), ReadingDirection::Ltr);
        assert_eq!(reading_direction("ar-EG"), ReadingDirection::Rtl);
        assert_eq!(reading_direction("he"), ReadingDirection::Rtl);
        assert_eq!(reading_direction("ku-Arab"), ReadingDirection::Rtl);
        // A script the rules do not name does not override the language, so a
        // tag in a script this build has never heard of still resolves the way
        // its language reads rather than being split down the middle.
        assert_eq!(reading_direction("ku-Latn"), ReadingDirection::Rtl);
        assert_eq!(reading_direction("sr-Latn"), ReadingDirection::Ltr);
        assert_eq!(reading_direction("fa_AF.UTF-8"), ReadingDirection::Rtl);
        assert_eq!(reading_direction(""), ReadingDirection::Ltr);
        assert_eq!(reading_direction("*"), ReadingDirection::Ltr);
        assert!(!reading_direction("de").is_rtl());
        assert!(SUPPORTED.iter().any(|locale| locale.direction().is_rtl()));
    }

    #[test]
    fn formatting_fills_what_it_is_given_and_leaves_the_rest_visible() {
        let catalog = Catalog::base();
        let line = catalog.format(MessageKey::SourceScanned, &[("count", "12")]);
        assert!(line.contains("12"), "{line}");
        assert!(!line.contains("{count}"), "{line}");
        let partial = catalog.format(MessageKey::SourceScanned, &[]);
        assert!(partial.contains("{count}"), "{partial}");
        assert_eq!(
            catalog.format(MessageKey::QueueWaiting, &[("count", "3")]),
            catalog.format(MessageKey::QueueWaiting, &[("count", "3")])
        );
    }

    #[test]
    fn preset_descriptions_fall_back_to_the_catalog() {
        // Every id in the files is a real preset and every file has something
        // in it, which the validation proves; what is checked here is the shape
        // of the lookup and the refusal to invent a translation.
        let problems = validate_description_files();
        assert!(problems.is_empty(), "{problems:#?}");
        // The base locale is answered by the catalog's own description, so
        // asking for a translation of it would be asking for a second copy of
        // the same sentence.
        assert!(
            preset_description("en", "general-very-fast-480-30-h264").is_none(),
            "the base locale is answered by the catalog itself"
        );
        assert!(preset_description("es", "no-such-preset").is_none());
        assert!(
            preset_description("es", "general-very-fast-480-30-h264").is_some(),
            "a preset this build has translated is translated"
        );
        // A language with no file of its own falls back to the catalog rather
        // than to another language's file.
        assert!(preset_description("kl", "general-very-fast-480-30-h264").is_none());
        // Arabic ships four translated descriptions and not the rest, which is
        // what an unfinished translation looks like in practice.
        assert!(preset_description("ar", "matroska-hevc-1080-30").is_some());
        assert!(preset_description("ar", "professional-ffv1-pcm").is_none());
    }
}
