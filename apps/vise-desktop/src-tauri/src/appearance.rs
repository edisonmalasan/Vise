//! Theme and layout direction: what the window decided, and why.
//!
//! Both are settings the user makes once and then forget, so both are held the
//! way the other application settings are: in the one settings file, through
//! the core settings API that presets and the recent source list already use.
//! There is no second store and no page-side preference to disagree with.
//!
//! A preference is not a resolved value. `system` and `auto` are answers that
//! have to be looked up every time something is painted, and the lookup can
//! change while the window is open: the operating system can be switched to its
//! dark theme, or the page can report a locale that reads the other way. So the
//! window holds the preference, the observation, and the resolution, and the
//! page is handed the resolved pair. What the page draws is therefore the same
//! answer on every surface, including the secondary windows, because they are
//! all one document reading one pair of values.
//!
//! Reading a stored value that is not one of the named options falls back to the
//! safe default rather than failing: a settings file edited by hand, or written
//! by a newer build, must not stop the window from opening.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use vise_core::{load_settings, resolve_catalog, save_settings, Catalog, MessageKey};

/// Settings member that holds the appearance preferences.
pub const APPEARANCE_KEY: &str = "appearance";

/// Default theme when nothing has been chosen.
///
/// Following the operating system is the only choice that cannot be wrong about
/// the machine the window is on.
pub const DEFAULT_THEME_MODE: ThemeMode = ThemeMode::System;

/// Default direction when nothing has been chosen.
pub const DEFAULT_DIRECTION_MODE: DirectionMode = DirectionMode::Auto;

/// The locale used when nothing names one.
///
/// The tag itself, the languages that read right to left, and the environment
/// variables a machine publishes its locale in all live in `vise_core::i18n`,
/// because the language the window is written in and the language the layout is
/// mirrored in are the same question. Answering it in two places is how a
/// window ends up showing Arabic text laid out left to right.
pub use vise_core::i18n::BASE_LOCALE as FALLBACK_LOCALE;

/// What the user asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    /// Whatever the operating system is set to.
    #[default]
    System,
    /// Always the light theme.
    Light,
    /// Always the dark theme.
    Dark,
}

impl ThemeMode {
    /// Every mode, in the order the picker lists them.
    pub const ALL: [ThemeMode; 3] = [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark];

    /// Stable identifier the interface layer sends back.
    pub fn id(self) -> &'static str {
        match self {
            ThemeMode::System => "system",
            ThemeMode::Light => "light",
            ThemeMode::Dark => "dark",
        }
    }

    /// Human-readable label, in the language in force.
    pub fn label(self, catalog: &Catalog) -> String {
        catalog
            .text(match self {
                ThemeMode::System => MessageKey::AppearanceThemeSystem,
                ThemeMode::Light => MessageKey::AppearanceThemeLight,
                ThemeMode::Dark => MessageKey::AppearanceThemeDark,
            })
            .to_string()
    }

    /// What choosing this means, in the words of the current observation.
    ///
    /// The line says which of the two themes it will actually produce, because
    /// "match the system" on a machine that is in dark mode is a dark window and
    /// a reader deciding what to pick needs to know that.
    pub fn hint(self, system: SystemTheme, catalog: &Catalog) -> String {
        let word = |theme: ResolvedTheme| {
            catalog.text(match theme {
                ResolvedTheme::Light => MessageKey::AppearanceThemeLight,
                ResolvedTheme::Dark => MessageKey::AppearanceThemeDark,
            })
        };
        match self {
            ThemeMode::System => {
                let resolved = self.resolve(system);
                catalog.format(
                    MessageKey::AppearanceThemeSystemHint,
                    &[("asked", word(resolved)), ("shown", word(resolved))],
                )
            }
            ThemeMode::Light => catalog
                .text(MessageKey::AppearanceThemeLightHint)
                .to_string(),
            ThemeMode::Dark => catalog
                .text(MessageKey::AppearanceThemeDarkHint)
                .to_string(),
        }
    }

    /// Read a mode the interface layer sent, or say why it was refused.
    pub fn parse(raw: &str) -> Result<Self, String> {
        let wanted = raw.trim().to_ascii_lowercase();
        ThemeMode::ALL
            .into_iter()
            .find(|mode| mode.id() == wanted)
            .ok_or_else(|| {
                format!(
                    "'{raw}' is not a theme; choose {}, {} or {}.",
                    ThemeMode::System.id(),
                    ThemeMode::Light.id(),
                    ThemeMode::Dark.id()
                )
            })
    }

    /// The theme this mode produces against an observation.
    ///
    /// A forced theme ignores the observation entirely; that is the whole point
    /// of forcing it.
    pub fn resolve(self, system: SystemTheme) -> ResolvedTheme {
        match self {
            ThemeMode::System => match system {
                SystemTheme::Light => ResolvedTheme::Light,
                SystemTheme::Dark => ResolvedTheme::Dark,
            },
            ThemeMode::Light => ResolvedTheme::Light,
            ThemeMode::Dark => ResolvedTheme::Dark,
        }
    }
}

/// The theme the window is actually painted in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResolvedTheme {
    /// The light theme.
    Light,
    /// The dark theme.
    Dark,
}

impl ResolvedTheme {
    /// Value the page puts on the document, which is also the value the
    /// stylesheet keys its colour tokens off.
    pub fn id(self) -> &'static str {
        match self {
            ResolvedTheme::Light => "light",
            ResolvedTheme::Dark => "dark",
        }
    }
}

/// What the operating system is set to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SystemTheme {
    /// The operating system is in its light theme.
    #[default]
    Light,
    /// The operating system is in its dark theme.
    Dark,
}

impl SystemTheme {
    /// The observation a `prefers-color-scheme` answer describes.
    ///
    /// The page sends whether the media query says dark rather than a name, so
    /// the window never has to parse a CSS keyword to get an answer it already
    /// had in the form of a boolean.
    pub fn of_dark(dark: bool) -> Self {
        if dark {
            SystemTheme::Dark
        } else {
            SystemTheme::Light
        }
    }

    /// Lower-case name, as the page and the stylesheet both spell it.
    pub fn id(self) -> &'static str {
        match self {
            SystemTheme::Light => "light",
            SystemTheme::Dark => "dark",
        }
    }

    /// The word to put in a sentence about it.
    pub fn word(self) -> &'static str {
        match self {
            SystemTheme::Light => "light",
            SystemTheme::Dark => "dark",
        }
    }

    /// The observation the platform's own theme type corresponds to.
    ///
    /// The platform reports the operating system's theme with its own type, and
    /// this is the one place that type is named, so nothing else in the window
    /// has to know about it. The type is `#[non_exhaustive]`: anything that is
    /// not the dark theme is read as light, because light is the answer a window
    /// can recover from.
    pub fn of_window(theme: tauri::Theme) -> Self {
        if matches!(theme, tauri::Theme::Dark) {
            SystemTheme::Dark
        } else {
            SystemTheme::Light
        }
    }
}

/// What the user asked for about reading direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DirectionMode {
    /// Whatever the locale reads.
    #[default]
    Auto,
    /// Always left to right.
    Ltr,
    /// Always right to left.
    Rtl,
}

impl DirectionMode {
    /// Every mode, in the order the picker lists them.
    pub const ALL: [DirectionMode; 3] =
        [DirectionMode::Auto, DirectionMode::Ltr, DirectionMode::Rtl];

    /// Stable identifier the interface layer sends back.
    pub fn id(self) -> &'static str {
        match self {
            DirectionMode::Auto => "auto",
            DirectionMode::Ltr => "ltr",
            DirectionMode::Rtl => "rtl",
        }
    }

    /// Human-readable label, in the language in force.
    pub fn label(self, catalog: &Catalog) -> String {
        catalog
            .text(match self {
                DirectionMode::Auto => MessageKey::AppearanceDirectionAuto,
                DirectionMode::Ltr => MessageKey::AppearanceDirectionLtr,
                DirectionMode::Rtl => MessageKey::AppearanceDirectionRtl,
            })
            .to_string()
    }

    /// What choosing this means, in the words of the current observation.
    ///
    /// The words inside the sentence are translated too, because a sentence with
    /// an English phrase in the middle of an Arabic one is not a translation of
    /// anything.
    pub fn hint(self, locale: &str, system: SystemDirection, catalog: &Catalog) -> String {
        let word = system.word(catalog);
        match self {
            DirectionMode::Auto => catalog.format(
                MessageKey::AppearanceDirectionAutoHint,
                &[
                    ("locale", locale),
                    ("reads", word),
                    // The layout follows the reading direction in every language
                    // this build ships, so the same word fills both slots.
                    ("flows", word),
                ],
            ),
            DirectionMode::Ltr => catalog
                .text(MessageKey::AppearanceDirectionFixedLtr)
                .to_string(),
            DirectionMode::Rtl => catalog
                .text(MessageKey::AppearanceDirectionFixedRtl)
                .to_string(),
        }
    }

    /// Read a mode the interface layer sent, or say why it was refused.
    pub fn parse(raw: &str) -> Result<Self, String> {
        let wanted = raw.trim().to_ascii_lowercase();
        DirectionMode::ALL
            .into_iter()
            .find(|mode| mode.id() == wanted)
            .ok_or_else(|| {
                format!(
                    "'{raw}' is not a layout direction; choose {}, {} or {}.",
                    DirectionMode::Auto.id(),
                    DirectionMode::Ltr.id(),
                    DirectionMode::Rtl.id()
                )
            })
    }

    /// The direction this mode produces against a locale.
    pub fn resolve(self, system: SystemDirection) -> ResolvedDirection {
        match self {
            DirectionMode::Auto => match system {
                SystemDirection::Ltr => ResolvedDirection::Ltr,
                SystemDirection::Rtl => ResolvedDirection::Rtl,
            },
            DirectionMode::Ltr => ResolvedDirection::Ltr,
            DirectionMode::Rtl => ResolvedDirection::Rtl,
        }
    }
}

/// The direction the window is actually laid out in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResolvedDirection {
    /// Left to right.
    Ltr,
    /// Right to left.
    Rtl,
}

impl ResolvedDirection {
    /// Value the page puts on the document as `dir`.
    ///
    /// This is the whole mirroring mechanism: the page sets one attribute and
    /// every logical property in the stylesheet follows it, so there is no
    /// second stylesheet to keep in step with the first.
    pub fn id(self) -> &'static str {
        match self {
            ResolvedDirection::Ltr => "ltr",
            ResolvedDirection::Rtl => "rtl",
        }
    }
}

/// Which way a locale reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SystemDirection {
    /// The locale reads left to right.
    #[default]
    Ltr,
    /// The locale reads right to left.
    Rtl,
}

impl SystemDirection {
    /// The message that names this direction in the language in force.
    ///
    /// A word is a message like any other: a sentence in Arabic with an English
    /// phrase inside it is not a translation, and the direction is the one thing
    /// in this window a user cannot guess the meaning of.
    pub fn word(self, catalog: &Catalog) -> &str {
        catalog.text(match self {
            SystemDirection::Ltr => MessageKey::AppearanceReadsLtr,
            SystemDirection::Rtl => MessageKey::AppearanceReadsRtl,
        })
    }

    /// The direction a language tag resolves to.
    ///
    /// The rules are the ones the translation resources use, in the one place
    /// that decides them: a named right-to-left script is consulted before the
    /// language, so a Kurdish or Punjabi locale written in the Arabic script is
    /// mirrored even though the language alone would not be. A script that is
    /// not one of the right-to-left ones does not override the language, so a
    /// tag this build does not recognise still resolves to the way its language
    /// reads rather than being split down the middle by an unrecognised script.
    pub fn of_locale(tag: &str) -> Self {
        match vise_core::i18n::reading_direction(tag) {
            vise_core::i18n::ReadingDirection::Ltr => SystemDirection::Ltr,
            vise_core::i18n::ReadingDirection::Rtl => SystemDirection::Rtl,
        }
    }
}

/// The locale tag reduced to what decides a direction.
///
/// A POSIX locale carries an encoding and a modifier that have nothing to do
/// with reading order, so `ar_EG.UTF-8` is read as `ar-EG`. This is the same
/// reduction the translation resources use, so the tag the window stores, the tag
/// the messages are looked up under, and the tag the direction is decided from
/// are one tag.
pub fn locale_tag(raw: &str) -> String {
    vise_core::i18n::normalize_locale_tag(raw)
}

/// The locale the environment names, as a tag.
///
/// Reading this at start-up gives the window an answer before the page has said
/// anything, which is what keeps a mirrored window from opening left to right
/// for as long as it takes the first round trip. A locale that names a language
/// this build does not ship is reduced to the one it does ship, which is the
/// same fallback chain the messages go through.
pub fn locale_from_environment() -> String {
    vise_core::i18n::locale_from_environment().tag().to_string()
}

/// The two preferences, as they are stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AppearanceSettings {
    /// What the user asked for.
    pub theme: ThemeMode,
    /// Which way they asked for the layout to read.
    pub direction: DirectionMode,
}

impl AppearanceSettings {
    /// The preferences a machine that has never chosen anything has.
    pub fn new() -> Self {
        Self {
            theme: DEFAULT_THEME_MODE,
            direction: DEFAULT_DIRECTION_MODE,
        }
    }

    /// Read the preferences out of the settings file.
    ///
    /// A file that is missing, unreadable, or not an appearance record at all
    /// gives the defaults. A record that is an appearance record but holds a
    /// value this build does not recognise also gives the defaults, because a
    /// name that cannot be resolved is not a reason to refuse to open.
    pub fn load(settings_path: &Path) -> Self {
        let Ok(settings) = load_settings(settings_path) else {
            return Self::new();
        };
        settings
            .extra
            .get(APPEARANCE_KEY)
            .cloned()
            .and_then(|value| serde_json::from_value::<Self>(value).ok())
            .unwrap_or_default()
    }

    /// Write the preferences back into the settings file.
    pub fn save(&self, settings_path: &Path) -> Result<(), String> {
        let mut settings = load_settings(settings_path).unwrap_or_default();
        let value = serde_json::to_value(self)
            .map_err(|e| format!("appearance settings could not be encoded: {e}"))?;
        settings.extra.insert(APPEARANCE_KEY.to_string(), value);
        save_settings(settings_path, &settings)
            .map_err(|e| format!("settings could not be saved: {e}"))
    }
}

/// What the window has observed about the machine it is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    /// The operating system's theme.
    pub theme: SystemTheme,
    /// The locale tag the direction is decided from.
    pub locale: String,
    /// Whether the page has reported the theme, as opposed to the window having
    /// asked the platform.
    ///
    /// The page is the authority on `prefers-color-scheme`, because that is the
    /// query the stylesheet itself is written against; the platform is the
    /// authority before the page has run. Both write the same observation, so
    /// the two answers cannot produce two themes.
    pub theme_from_page: bool,
    /// Whether the locale was chosen rather than read from the environment.
    ///
    /// Both answers are deliberate: the page reporting `navigator.language` and the
    /// user picking a language from the preferences are the same kind of evidence,
    /// and the note says "by this window" for either of them. What it is
    /// distinguishing is a deliberate answer from whatever the machine happened to
    /// be set to when the window opened.
    pub locale_chosen: bool,
}

impl Default for Observation {
    fn default() -> Self {
        Self {
            theme: SystemTheme::Light,
            locale: FALLBACK_LOCALE.to_string(),
            theme_from_page: false,
            locale_chosen: false,
        }
    }
}

impl Observation {
    /// The direction the observed locale resolves to.
    pub fn direction(&self) -> SystemDirection {
        SystemDirection::of_locale(&self.locale)
    }
}

/// One way of answering the appearance question.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceOptionViewModel {
    /// Stable identifier used by the interface layer.
    pub id: String,
    /// Human-readable label.
    pub label: String,
    /// What choosing this means right now.
    pub facts: String,
    /// Whether this can be chosen.
    pub available: bool,
    /// Why it cannot, when it cannot.
    pub reason: String,
    /// Whether this is the chosen answer.
    pub selected: bool,
}

/// Everything the page needs to paint itself, decided once.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceViewModel {
    /// The theme preference the user is holding.
    pub theme_mode: String,
    /// The direction preference the user is holding.
    pub direction_mode: String,
    /// The theme to paint in, after resolving against the system.
    pub theme: String,
    /// The direction to lay out in, after resolving against the locale.
    pub direction: String,
    /// Whether the theme follows the operating system.
    pub follows_system_theme: bool,
    /// Whether the direction follows the locale.
    pub follows_locale: bool,
    /// The theme the operating system is asking for, whichever mode is held.
    pub system_theme: String,
    /// The locale tag the direction is decided from.
    pub locale: String,
    /// The language the window is written in.
    ///
    /// Sent rather than looked up by the page, because the tag on the document is
    /// this value and the page has no way to know it: a locale that is not in the
    /// shipped set has to reach the browser as the language it was resolved to,
    /// or the browser picks a font for a language nobody is writing in.
    pub language: String,
    /// How the language reads, in that language.
    ///
    /// Its own message rather than a word from the layout code, so a sentence
    /// about direction is not half in one language.
    pub direction_language: String,
    /// The theme preference as a list of options.
    pub theme_options: Vec<AppearanceOptionViewModel>,
    /// The direction preference as a list of options.
    pub direction_options: Vec<AppearanceOptionViewModel>,
    /// One line saying what is being shown and why.
    pub summary: String,
    /// The direction glyph the page shows next to the summary.
    ///
    /// The glyph is sent rather than chosen here because it is a direction the
    /// text flows in, and a glyph that implies one direction has to mirror with
    /// the layout rather than be replaced by another one.
    pub flow_glyph: String,
    /// Where the two observations came from, in one line.
    pub observation_note: String,
}

impl AppearanceViewModel {
    /// The answer, resolved and explained.
    fn build(settings: &AppearanceSettings, observed: &Observation, catalog: &Catalog) -> Self {
        let theme_mode = settings.theme;
        let direction_mode = settings.direction;
        let theme = theme_mode.resolve(observed.theme);
        let system_direction = observed.direction();
        let direction = direction_mode.resolve(system_direction);
        Self {
            theme_mode: theme_mode.id().to_string(),
            direction_mode: direction_mode.id().to_string(),
            theme: theme.id().to_string(),
            direction: direction.id().to_string(),
            follows_system_theme: matches!(theme_mode, ThemeMode::System),
            follows_locale: matches!(direction_mode, DirectionMode::Auto),
            system_theme: observed.theme.id().to_string(),
            locale: observed.locale.clone(),
            language: catalog.locale_tag().to_string(),
            direction_language: catalog
                .text(match system_direction {
                    SystemDirection::Ltr => MessageKey::AppearanceReadsLtr,
                    SystemDirection::Rtl => MessageKey::AppearanceReadsRtl,
                })
                .to_string(),
            theme_options: ThemeMode::ALL
                .into_iter()
                .map(|mode| AppearanceOptionViewModel {
                    id: mode.id().to_string(),
                    label: mode.label(catalog),
                    facts: mode.hint(observed.theme, catalog),
                    available: true,
                    reason: String::new(),
                    selected: mode == theme_mode,
                })
                .collect(),
            direction_options: DirectionMode::ALL
                .into_iter()
                .map(|mode| AppearanceOptionViewModel {
                    id: mode.id().to_string(),
                    label: mode.label(catalog),
                    facts: mode.hint(&observed.locale, system_direction, catalog),
                    available: true,
                    reason: String::new(),
                    selected: mode == direction_mode,
                })
                .collect(),
            summary: summary(
                theme_mode,
                observed.theme,
                direction_mode,
                &observed.locale,
                system_direction,
                catalog,
            ),
            flow_glyph: FLOW_GLYPH.to_string(),
            observation_note: observation_note(observed),
        }
    }
}

/// One line saying where the two observations came from.
///
/// Named rather than shown as two bare values because "dark" on its own does not
/// say whether the window or the machine decided it, and the whole point of the
/// preference is that the user can.
fn observation_note(observed: &Observation) -> String {
    let theme_source = if observed.theme_from_page {
        "reported by this window"
    } else {
        "read from the operating system"
    };
    let locale_source = if observed.locale_chosen {
        "reported by this window"
    } else {
        "read from the environment"
    };
    format!(
        "Your system is in its {} theme, {}; your locale is {}, {}.",
        observed.theme.word(),
        theme_source,
        observed.locale,
        locale_source
    )
}

/// The arrow the summary puts beside the layout direction.
///
/// One glyph for both directions on purpose: the page mirrors it with the
/// layout rather than being handed a different one, so the arrow is always the
/// direction the layout actually flows and cannot disagree with it.
const FLOW_GLYPH: &str = "\u{2192}";

/// One line saying what is on screen and why it is that.
fn summary(
    theme_mode: ThemeMode,
    theme: SystemTheme,
    direction_mode: DirectionMode,
    locale: &str,
    system_direction: SystemDirection,
    catalog: &Catalog,
) -> String {
    let theme_answer = match theme_mode {
        ThemeMode::System => catalog
            .format(
                MessageKey::AppearanceThemeSystemHint,
                &[
                    ("asked", catalog.text(theme_word(theme))),
                    ("shown", catalog.text(theme_word(theme))),
                ],
            )
            .to_string(),
        ThemeMode::Light => catalog
            .text(MessageKey::AppearanceThemeLightHint)
            .to_string(),
        ThemeMode::Dark => catalog
            .text(MessageKey::AppearanceThemeDarkHint)
            .to_string(),
    };
    let reads = system_direction.word(catalog);
    let direction_answer = match direction_mode {
        DirectionMode::Auto => catalog.format(
            MessageKey::AppearanceDirectionAutoHint,
            &[("locale", locale), ("reads", reads), ("flows", reads)],
        ),
        DirectionMode::Ltr => catalog
            .text(MessageKey::AppearanceDirectionFixedLtr)
            .to_string(),
        DirectionMode::Rtl => catalog
            .text(MessageKey::AppearanceDirectionFixedRtl)
            .to_string(),
    };
    format!(
        "{} / {}",
        theme_answer.trim_end_matches('.'),
        direction_answer.trim_end_matches('.')
    )
}

/// Which message names a theme.
fn theme_word(theme: SystemTheme) -> MessageKey {
    match theme {
        SystemTheme::Light => MessageKey::AppearanceThemeLight,
        SystemTheme::Dark => MessageKey::AppearanceThemeDark,
    }
}

/// The appearance state the window keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppearanceState {
    /// What the user chose.
    settings: AppearanceSettings,
    /// What the machine is doing.
    observed: Observation,
    /// Where the settings live.
    settings_path: PathBuf,
    /// The language the window is written in.
    ///
    /// Held rather than derived from the observed locale because the two are
    /// different questions and can legitimately have different answers: a user can
    /// read Arabic and still want the window laid out left to right, and a user
    /// whose machine is in Japanese can choose English for the words. Which
    /// catalog is in force is the language module's decision; this state only
    /// draws its labels from it.
    catalog: &'static Catalog,
}

impl Default for AppearanceState {
    /// The preferences and the observation a machine starts with, with nowhere
    /// to save to.
    ///
    /// Only ever used by the tests and by a window built without a settings
    /// file: `main` loads the real one, so a preference a user made is never
    /// dropped because a state was built the cheap way.
    fn default() -> Self {
        Self {
            settings: AppearanceSettings::new(),
            observed: Observation::default(),
            settings_path: PathBuf::new(),
            catalog: Catalog::base(),
        }
    }
}

impl AppearanceState {
    /// Read the preferences and take the first observation.
    ///
    /// The theme starts from the light theme because that is the safe answer to
    /// an unknown one: a window that opens light and then corrects itself is
    /// readable, and a window that opens dark on a light machine is not.
    pub fn load(settings_path: &Path, theme: SystemTheme, locale: Option<String>) -> Self {
        let (locale, locale_chosen) = match locale {
            Some(reported) => (locale_tag(&reported), true),
            None => (locale_from_environment(), false),
        };
        Self {
            settings: AppearanceSettings::load(settings_path),
            observed: Observation {
                theme,
                locale: locale.clone(),
                theme_from_page: false,
                locale_chosen,
            },
            settings_path: settings_path.to_path_buf(),
            catalog: resolve_catalog(&locale).catalog,
        }
    }

    /// The state a test wants, holding an observation no file or environment would
    /// produce on this machine.
    #[cfg(test)]
    pub fn with_observation(
        settings_path: &Path,
        settings: AppearanceSettings,
        observed: Observation,
    ) -> Self {
        Self {
            settings,
            catalog: resolve_catalog(&observed.locale).catalog,
            observed,
            settings_path: settings_path.to_path_buf(),
        }
    }

    /// Draw the labels from another language's messages.
    ///
    /// Reports whether the language moved, so a caller that repaints on every
    /// change is not repainting a window that did not. This is the only thing a
    /// language switch does to this state: the layout is decided by
    /// [`AppearanceState::observe_locale`], and which direction that produces is
    /// decided by the preference, so choosing a language cannot override a user
    /// who pinned the direction the other way.
    pub fn use_language(&mut self, catalog: &'static Catalog) -> bool {
        if self.catalog.locale_tag() == catalog.locale_tag() {
            return false;
        }
        self.catalog = catalog;
        true
    }

    /// The language the labels are drawn from.
    ///
    /// Read-only and answered by the view model, because the page asks for the
    /// language through [`AppearanceViewModel::language`] and a second accessor on
    /// the state would be a second answer to the same question.
    pub fn language(&self) -> &'static str {
        self.catalog.locale_tag()
    }

    /// Record what the operating system is doing.
    ///
    /// Reports whether the observation moved, so a caller that repaints on every
    /// report is not repainting a window that did not change.
    pub fn observe_theme(&mut self, theme: SystemTheme, from_page: bool) -> bool {
        // The page watches the same media query the stylesheet is written
        // against, so once it has spoken it stays the authority: a platform
        // report arriving afterwards is the weaker witness and is not allowed to
        // move a window the colours have already settled on.
        if !from_page && self.observed.theme_from_page {
            return false;
        }
        if self.observed.theme == theme && self.observed.theme_from_page == from_page {
            return false;
        }
        self.observed.theme = theme;
        self.observed.theme_from_page = from_page;
        true
    }

    /// Record a deliberate answer about the locale.
    ///
    /// Both callers are the same kind of witness: the page reporting what it is
    /// showing, and the user choosing a language from the preferences. An empty tag
    /// is ignored rather than stored: it would resolve to left to right for no
    /// reason, and the page asking again on its next start would not fix it.
    ///
    /// This is an observation rather than a preference, which is the whole reason a
    /// right-to-left language does not force a right-to-left window: the direction
    /// preference is what decides, and it is resolved against this.
    pub fn observe_locale(&mut self, locale: &str) -> bool {
        if locale.trim().is_empty() {
            return false;
        }
        let tag = locale_tag(locale);
        if self.observed.locale_chosen && self.observed.locale == tag {
            return false;
        }
        self.observed.locale = tag;
        self.observed.locale_chosen = true;
        true
    }

    /// The direction preference the user is holding.
    ///
    /// Read rather than written so a language switch can be resolved against what
    /// the user chose about the layout without this module having to know that
    /// there are two preferences rather than one.
    pub fn direction_mode(&self) -> DirectionMode {
        self.settings.direction
    }

    /// What the window has observed.
    #[cfg(test)]
    pub fn observation(&self) -> &Observation {
        &self.observed
    }

    /// The answer, resolved and explained.
    pub fn view(&self) -> AppearanceViewModel {
        AppearanceViewModel::build(&self.settings, &self.observed, self.catalog)
    }

    /// Choose a theme and keep it.
    pub fn set_theme_mode(&mut self, raw: &str) -> Result<AppearanceViewModel, String> {
        let mode = ThemeMode::parse(raw)?;
        if mode != self.settings.theme {
            self.settings.theme = mode;
            self.settings.save(&self.settings_path)?;
        }
        Ok(self.view())
    }

    /// Choose a direction and keep it.
    pub fn set_direction_mode(&mut self, raw: &str) -> Result<AppearanceViewModel, String> {
        let mode = DirectionMode::parse(raw)?;
        if mode != self.settings.direction {
            self.settings.direction = mode;
            self.settings.save(&self.settings_path)?;
        }
        Ok(self.view())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use vise_core::AppSettings;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vise-desktop-appearance-{name}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp dir");
        dir.join("settings.json")
    }

    fn dark_rtl() -> Observation {
        Observation {
            theme: SystemTheme::Dark,
            locale: "ar-EG".to_string(),
            theme_from_page: true,
            locale_chosen: true,
        }
    }

    /// A state holding exactly this observation, writing to a scratch file so a
    /// test that chooses a preference does not leave one behind.
    fn observed(settings: AppearanceSettings, seen: Observation) -> AppearanceState {
        AppearanceState::with_observation(&temp_dir("observe"), settings, seen)
    }

    /// The view as the base language builds it.
    ///
    /// The language in force is a separate question with its own tests in
    /// `locale.rs`; what matters here is the resolution, so every one of these
    /// tests draws its labels from the same catalog.
    fn build(settings: &AppearanceSettings, seen: &Observation) -> AppearanceViewModel {
        AppearanceViewModel::build(settings, seen, Catalog::base())
    }

    #[test]
    fn the_default_preferences_follow_the_machine() {
        let settings = AppearanceSettings::new();
        assert_eq!(settings.theme, DEFAULT_THEME_MODE);
        assert_eq!(settings.direction, DEFAULT_DIRECTION_MODE);
        let view = build(&settings, &dark_rtl());
        assert_eq!(view.theme, "dark");
        assert_eq!(view.direction, "rtl");
        assert!(view.follows_system_theme);
        assert!(view.follows_locale);
    }

    #[test]
    fn a_forced_theme_ignores_the_system() {
        for system in [SystemTheme::Light, SystemTheme::Dark] {
            assert_eq!(
                ThemeMode::Light.resolve(system),
                ResolvedTheme::Light,
                "light is light whatever the system asks for"
            );
            assert_eq!(
                ThemeMode::Dark.resolve(system),
                ResolvedTheme::Dark,
                "dark is dark whatever the system asks for"
            );
        }
        // Only the automatic mode answers to the observation.
        assert_eq!(
            ThemeMode::System.resolve(SystemTheme::Dark),
            ResolvedTheme::Dark
        );
        assert_eq!(
            ThemeMode::System.resolve(SystemTheme::Light),
            ResolvedTheme::Light
        );
    }

    #[test]
    fn a_forced_direction_ignores_the_locale() {
        assert_eq!(
            DirectionMode::Ltr.resolve(SystemDirection::Rtl),
            ResolvedDirection::Ltr
        );
        assert_eq!(
            DirectionMode::Rtl.resolve(SystemDirection::Ltr),
            ResolvedDirection::Rtl
        );
        assert_eq!(
            DirectionMode::Auto.resolve(SystemDirection::Rtl),
            ResolvedDirection::Rtl
        );
        assert_eq!(
            DirectionMode::Auto.resolve(SystemDirection::Ltr),
            ResolvedDirection::Ltr
        );
    }

    #[test]
    fn languages_and_scripts_both_decide_a_direction() {
        for tag in [
            "ar", "ar-EG", "he", "he-IL", "fa-IR", "ur", "ps", "yi", "dv", "sd", "ug",
        ] {
            assert_eq!(
                SystemDirection::of_locale(tag),
                SystemDirection::Rtl,
                "{tag} reads right to left"
            );
        }
        for tag in [
            "en",
            "en-GB",
            "de-DE",
            "ja-JP",
            "zh-Hans-CN",
            "ru-RU",
            "tr-TR",
        ] {
            assert_eq!(
                SystemDirection::of_locale(tag),
                SystemDirection::Ltr,
                "{tag} reads left to right"
            );
        }
        // A named right-to-left script mirrors a language that does not name one.
        for tag in ["ku-Arab", "az-Arab", "pa-Arab", "ms-Arab", "en-Hebr"] {
            assert_eq!(
                SystemDirection::of_locale(tag),
                SystemDirection::Rtl,
                "{tag} names a right-to-left script"
            );
        }
        // A script this build does not recognise does not override the language,
        // so right-to-left text is never split down the middle. A language written
        // in both scripts is therefore mirrored unless the tag names a
        // right-to-left script this build does not know.
        assert_eq!(SystemDirection::of_locale("he-Latn"), SystemDirection::Rtl);
        assert_eq!(SystemDirection::of_locale("ku-Latn"), SystemDirection::Rtl);
        // A locale that says nothing usable is read left to right rather than
        // mirrored at random.
        for tag in ["", "   ", "*", "x"] {
            assert_eq!(SystemDirection::of_locale(tag), SystemDirection::Ltr);
        }
        // The POSIX spelling of the same locale gives the same answer.
        assert_eq!(SystemDirection::of_locale("ar_EG"), SystemDirection::Rtl);
    }

    #[test]
    fn a_posix_locale_is_reduced_to_what_decides_a_direction() {
        assert_eq!(locale_tag("ar_EG.UTF-8"), "ar-EG");
        assert_eq!(locale_tag("he_IL.ISO-8859-8"), "he-IL");
        assert_eq!(locale_tag("  en_GB  "), "en-GB");
        assert_eq!(locale_tag(""), FALLBACK_LOCALE);
        assert_eq!(locale_tag("   "), FALLBACK_LOCALE);
    }

    #[test]
    fn a_preference_is_read_and_written_in_the_settings_file() {
        let path = temp_dir("round-trip");
        assert_eq!(AppearanceSettings::load(&path), AppearanceSettings::new());

        let mut state = AppearanceState::load(&path, SystemTheme::Light, Some("he-IL".into()));
        state.set_theme_mode("dark").expect("dark theme");
        state.set_direction_mode("rtl").expect("rtl layout");

        let stored = AppearanceSettings::load(&path);
        assert_eq!(stored.theme, ThemeMode::Dark);
        assert_eq!(stored.direction, DirectionMode::Rtl);
        let mut settings: AppSettings = load_settings(&path).expect("settings read back");
        assert!(settings.extra.contains_key(APPEARANCE_KEY));

        // Another writer of the same file leaves the preferences alone.
        settings.default_preset = Some("high-quality".to_string());
        save_settings(&path, &settings).expect("settings saved");
        assert_eq!(AppearanceSettings::load(&path), stored);
    }

    #[test]
    fn a_stored_value_this_build_does_not_know_falls_back_to_the_default() {
        let path = temp_dir("invalid");
        let mut settings = AppSettings::default();
        settings.extra.insert(
            APPEARANCE_KEY.to_string(),
            serde_json::json!({"theme": "sepia", "direction": "upwards"}),
        );
        save_settings(&path, &settings).expect("settings saved");
        let loaded = AppearanceSettings::load(&path);
        assert_eq!(loaded.theme, DEFAULT_THEME_MODE);
        assert_eq!(loaded.direction, DEFAULT_DIRECTION_MODE);

        // A member this build does not recognise costs the whole record rather
        // than half of it: a preference the window cannot name is not a
        // preference it can half-apply.
        settings.extra.insert(
            APPEARANCE_KEY.to_string(),
            serde_json::json!({"theme": "sepia", "direction": "rtl"}),
        );
        save_settings(&path, &settings).expect("settings saved");
        assert_eq!(AppearanceSettings::load(&path), AppearanceSettings::new());

        // A member that is simply absent keeps its default and costs the others
        // nothing.
        settings.extra.insert(
            APPEARANCE_KEY.to_string(),
            serde_json::json!({"direction": "rtl"}),
        );
        save_settings(&path, &settings).expect("settings saved");
        let partial = AppearanceSettings::load(&path);
        assert_eq!(partial.theme, DEFAULT_THEME_MODE);
        assert_eq!(partial.direction, DirectionMode::Rtl);

        // A record that is not a record at all is ignored the same way.
        for junk in [
            serde_json::json!("dark"),
            serde_json::json!([1, 2, 3]),
            serde_json::json!(null),
        ] {
            settings.extra.insert(APPEARANCE_KEY.to_string(), junk);
            save_settings(&path, &settings).expect("settings saved");
            assert_eq!(AppearanceSettings::load(&path), AppearanceSettings::new());
        }
    }

    #[test]
    fn a_settings_file_that_is_not_json_reads_as_the_defaults() {
        let path = temp_dir("broken");
        fs::create_dir_all(path.parent().unwrap()).expect("dir");
        fs::write(&path, "{ not json").expect("write");
        assert_eq!(AppearanceSettings::load(&path), AppearanceSettings::new());
        assert_eq!(
            AppearanceSettings::load(Path::new("no/such/file.json")),
            AppearanceSettings::new()
        );
    }

    #[test]
    fn an_unknown_choice_is_refused_with_the_names_that_would_work() {
        assert!(ThemeMode::parse("sepia").is_err());
        assert!(DirectionMode::parse("upwards").is_err());
        assert!(ThemeMode::parse("").is_err());
        let error = DirectionMode::parse("diagonal").expect_err("refused");
        assert!(error.contains("diagonal"), "{error}");
        assert!(error.contains("auto"), "{error}");
        // Surrounding space and case are the interface's problem, not the user's.
        assert_eq!(ThemeMode::parse("  DARK ").expect("dark"), ThemeMode::Dark);
        assert_eq!(
            DirectionMode::parse("RTL").expect("rtl"),
            DirectionMode::Rtl
        );
    }

    #[test]
    fn a_chosen_theme_paints_in_both_observations() {
        let mut state = observed(AppearanceSettings::new(), dark_rtl());
        assert_eq!(state.view().theme, "dark");

        assert!(state.observe_theme(SystemTheme::Light, true));
        assert_eq!(
            state.view().theme,
            "light",
            "following the system follows it both ways"
        );

        state.set_theme_mode("dark").expect("forced dark");
        assert!(state.observe_theme(SystemTheme::Dark, true));
        assert_eq!(
            state.view().theme,
            "dark",
            "a forced theme does not move when the system does"
        );
        assert!(!state.view().follows_system_theme);

        // And it does not move when the system goes the other way either.
        assert!(state.observe_theme(SystemTheme::Light, true));
        assert_eq!(state.view().theme, "dark");
    }

    #[test]
    fn an_observation_that_did_not_change_says_so() {
        let mut state = observed(AppearanceSettings::new(), dark_rtl());
        assert!(!state.observe_theme(SystemTheme::Dark, true));
        assert!(state.observe_theme(SystemTheme::Light, true));
        assert!(!state.observe_locale("ar-EG"));
        assert!(state.observe_locale("he-IL"));
        assert_eq!(state.view().direction, "rtl");
        // An empty tag from the page is ignored rather than stored.
        assert!(!state.observe_locale(""));
        assert_eq!(state.observation().locale, "he-IL");
    }

    #[test]
    fn a_platform_report_does_not_override_the_page() {
        // Before the page has said anything the platform is the only witness.
        let mut state = observed(
            AppearanceSettings::new(),
            Observation {
                theme: SystemTheme::Light,
                locale: "en-GB".to_string(),
                theme_from_page: false,
                locale_chosen: false,
            },
        );
        assert!(state.observe_theme(SystemTheme::Dark, true));
        assert_eq!(state.view().theme, "dark");

        // After that the page is the authority, because it is the side reading
        // the same media query the stylesheet is written against.
        assert!(!state.observe_theme(SystemTheme::Light, false));
        assert_eq!(state.view().theme, "dark");
        assert!(
            state
                .view()
                .observation_note
                .contains("reported by this window"),
            "{}",
            state.view().observation_note
        );
    }

    #[test]
    fn the_chosen_options_are_the_ones_marked_selected() {
        let settings = AppearanceSettings {
            theme: ThemeMode::Light,
            direction: DirectionMode::Rtl,
        };
        let view = build(&settings, &dark_rtl());
        assert_eq!(view.theme_options.len(), ThemeMode::ALL.len());
        assert_eq!(view.direction_options.len(), DirectionMode::ALL.len());
        let chosen: Vec<&str> = view
            .theme_options
            .iter()
            .filter(|option| option.selected)
            .map(|option| option.id.as_str())
            .collect();
        assert_eq!(chosen, vec!["light"]);
        let chosen: Vec<&str> = view
            .direction_options
            .iter()
            .filter(|option| option.selected)
            .map(|option| option.id.as_str())
            .collect();
        assert_eq!(chosen, vec!["rtl"]);
        assert!(view.theme_options.iter().all(|option| option.available));
        assert!(view.direction_options.iter().all(|option| option.available));
    }

    #[test]
    fn the_summary_says_which_theme_and_which_direction_are_on_screen() {
        let following = build(&AppearanceSettings::new(), &dark_rtl());
        assert!(
            following.summary.contains("will be Dark"),
            "{}",
            following.summary
        );
        assert!(
            following.summary.contains("system asks"),
            "{}",
            following.summary
        );
        assert!(following.summary.contains("ar-EG"), "{}", following.summary);
        assert!(
            following.summary.contains("right to left"),
            "{}",
            following.summary
        );

        let forced = build(
            &AppearanceSettings {
                theme: ThemeMode::Light,
                direction: DirectionMode::Ltr,
            },
            &dark_rtl(),
        );
        assert!(
            forced.summary.contains("Always light"),
            "{}",
            forced.summary
        );
        assert!(
            forced.summary.contains("whatever the system asks for"),
            "{}",
            forced.summary
        );
        assert!(
            forced.summary.contains("whatever the language reads"),
            "{}",
            forced.summary
        );
        assert!(!forced.follows_system_theme);
        assert!(!forced.follows_locale);
    }

    #[test]
    fn the_page_receives_the_answer_it_paints_from() {
        // The page is handed the resolved pair, never a preference, so every
        // surface reads the same two values.
        let payload = serde_json::to_value(build(
            &AppearanceSettings {
                theme: ThemeMode::Dark,
                direction: DirectionMode::Auto,
            },
            &dark_rtl(),
        ))
        .expect("answer");
        assert_eq!(payload["theme"], "dark");
        assert_eq!(payload["direction"], "rtl");
        assert_eq!(payload["themeMode"], "dark");
        assert_eq!(payload["directionMode"], "auto");
        assert!(payload["followsSystemTheme"].as_bool().is_some());
        assert!(payload["flowGlyph"].as_str().is_some());
        assert!(payload["summary"].as_str().is_some());
        // The page puts this on the document, so it has to be there: the language
        // the messages were resolved from, not the tag the layout was decided
        // from.
        assert_eq!(payload["language"], "en");
        assert_eq!(payload["locale"], "ar-EG");
        assert_eq!(payload["directionLanguage"], "right to left");
    }

    #[test]
    fn the_labels_are_drawn_from_the_language_in_force() {
        let settings = AppearanceSettings::new();
        let english = build(&settings, &dark_rtl());
        assert_eq!(english.direction_options[0].label, "Match my language");
        let arabic = AppearanceViewModel::build(
            &settings,
            &dark_rtl(),
            vise_core::resolve_catalog("ar").catalog,
        );
        assert_eq!(arabic.language, "ar");
        assert_eq!(arabic.direction_options[0].label, "حسب لغتي");
        // The hint sentence is in that language too, including the words inside
        // it: an English phrase in the middle of an Arabic sentence is not a
        // translation.
        assert!(
            arabic.direction_options[0]
                .facts
                .contains("من اليمين إلى اليسار"),
            "{}",
            arabic.direction_options[0].facts
        );
        assert!(!arabic.direction_options[0].facts.contains("reads"));
        assert_eq!(arabic.theme_options[0].label, "حسب النظام");
    }

    #[test]
    fn choosing_the_same_language_twice_is_not_a_change() {
        let mut state = observed(AppearanceSettings::new(), dark_rtl());
        let catalog = vise_core::resolve_catalog("de").catalog;
        assert!(state.use_language(catalog), "the first choice moves");
        assert!(!state.use_language(catalog), "the second one does not");
        assert_eq!(state.view().language, "de");
    }
}
