//! The language the window is written in, and what changing it means.
//!
//! A language is three answers that have to agree with each other: which
//! message file is drawn from, which way the window is laid out, and what the
//! page puts on the document. They live together here so a switch cannot set one
//! and leave the other two to be inferred later.
//!
//! What a switch deliberately does *not* do is throw away what the user has
//! typed. Choosing a language is not choosing a source again: it re-emits the
//! view models the chrome draws from, and the state behind the configuration
//! tabs is not read, rebuilt, or reset. That is the difference between switching
//! language at runtime and restarting the window, and it is the reason this is a
//! decision with tests rather than a call that rebuilds the desktop state.
//!
//! The preference itself belongs to the preferences record, with the rest of the
//! choices a user makes once. This module holds the *answer* — the locale in
//! force, where it came from, and how much of the interface is in it — so the
//! record stores one tag and nothing here has to be persisted at all.

use serde::{Deserialize, Serialize};
use vise_core::{resolve_catalog, Catalog, MatchSource, ResolvedCatalog, SUPPORTED};

use crate::appearance::{AppearanceState, AppearanceViewModel, DirectionMode};

/// The event the window pushes when the language has changed.
///
/// The page cannot work this out for itself: it has the new text but not the
/// decision about what it should redraw, so the window says what changed and the
/// page applies it. The payload carries the two answers that moved — the language
/// and the layout — and deliberately not the configuration tabs, because
/// redrawing those is what would throw away what the user has typed.
pub const LANGUAGE_EVENT: &str = "language-changed";

/// A language the user can pick, as the picker shows it.
///
/// Each language is named in itself, which is the one case where translating the
/// label makes the picker unusable: somebody who does not read the current
/// language has to be able to find their own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageOptionViewModel {
    /// The tag sent back when this one is chosen.
    pub id: String,
    /// The language's own name for itself.
    pub label: String,
    /// Whether this is the language in force.
    pub selected: bool,
    /// Which way it reads, so the reader can see what choosing it does to the
    /// layout before choosing it rather than finding out afterwards.
    pub direction: String,
    /// How many of this language's messages are its own.
    ///
    /// A count rather than a proportion because the honest answer is "seventy-five
    /// of seventy-five", and a percentage would hide that anything untranslated is
    /// a whole sentence rather than a word.
    pub translated: usize,
    /// How many messages there are in total.
    pub total: usize,
}

/// Everything the page needs to paint the language picker, decided once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageViewModel {
    /// The tag in force.
    pub locale: String,
    /// The language in force, in its own name.
    pub language: String,
    /// Which way it reads, which is the direction `auto` follows.
    pub direction: String,
    /// Whether the layout is following the language.
    ///
    /// False when the user pinned the direction, because a right-to-left language
    /// in a left-to-right window is a choice somebody made and not a mistake.
    pub follows_language: bool,
    /// One line saying which language is in force and how it was arrived at.
    pub summary: String,
    /// How much of the interface is in this language, in this language.
    pub coverage: String,
    /// Every message this language did not have, and what stood in for it.
    ///
    /// Sent rather than only written to the log: a gap that is visible nowhere a
    /// user can reach is a gap that stays a gap.
    pub gaps: Vec<String>,
    /// Every language this build ships.
    pub options: Vec<LanguageOptionViewModel>,
}

/// One language switch, and everything it produced.
#[derive(Debug, Clone, PartialEq)]
pub struct LanguageSwitch {
    /// The language answer, as the picker draws it.
    pub view: LanguageViewModel,
    /// The appearance answer, because a language can move the layout.
    pub appearance: AppearanceViewModel,
    /// The line to write to the activity log.
    pub note: String,
    /// Whether the language in force actually moved.
    pub changed: bool,
}

/// Every language this build ships, as the picker lists them.
pub fn language_options(selected: &Catalog) -> Vec<LanguageOptionViewModel> {
    SUPPORTED
        .iter()
        .map(|locale| {
            let resolved = resolve_catalog(locale.tag());
            LanguageOptionViewModel {
                id: locale.tag().to_string(),
                label: locale.endonym().to_string(),
                selected: locale.tag() == selected.locale_tag(),
                direction: locale.direction().id().to_string(),
                translated: resolved.translated(),
                total: resolved.total(),
            }
        })
        .collect()
}

/// The language answer for a locale, against the direction the user is holding.
pub fn language_view(
    resolved: &ResolvedCatalog,
    direction_mode: DirectionMode,
) -> LanguageViewModel {
    LanguageViewModel {
        locale: resolved.locale.tag().to_string(),
        language: resolved.locale.endonym().to_string(),
        direction: resolved.locale.direction().id().to_string(),
        follows_language: matches!(direction_mode, DirectionMode::Auto),
        summary: summary(resolved),
        coverage: resolved.coverage(),
        gaps: resolved.gap_notes(),
        options: language_options(resolved.catalog),
    }
}

/// One line saying which language is in force and how it got there.
fn summary(resolved: &ResolvedCatalog) -> String {
    let endonym = resolved.locale.endonym();
    match resolved.match_source {
        // Asked for by name and got it: the sentence the language itself carries.
        MatchSource::Exact => resolved.catalog.format(
            vise_core::MessageKey::LanguageSwitched,
            &[("locale", endonym)],
        ),
        // Reached through the chain, so the tag that was asked for is named
        // rather than quietly replaced by the language on screen.
        MatchSource::BaseLanguage => format!(
            "'{}' is not a language this build ships, so it is shown in {endonym}.",
            resolved.requested
        ),
        MatchSource::Base => format!(
            "'{}' is not a language this build ships, so it is shown in the base language ({endonym}).",
            resolved.requested
        ),
    }
}

/// Read a language the interface layer asked for, or say why it was refused.
///
/// Only an empty request is refused. A language this build does not ship is not a
/// mistake: the fallback chain exists for it, it produces a usable window, and the
/// picker says which language it landed on.
pub fn read_request(raw: &str) -> Result<String, String> {
    if raw.trim().is_empty() {
        return Err(format!(
            "no language was named; choose {}.",
            SUPPORTED
                .iter()
                .map(|locale| locale.tag())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    Ok(vise_core::i18n::normalize_locale_tag(raw))
}

/// Choose a language.
///
/// The two halves are separate on purpose. The layout is an *observation* — which
/// way the chosen language reads — and the preference is what decides against it,
/// so a user who pinned the direction gets the words they chose and the layout
/// they chose. That is why a right-to-left language does not drag a left-to-right
/// window with it, and it is the existing direction resolution from Group 14
/// doing the deciding rather than a second rule here.
pub fn switch_language(
    appearance: &mut AppearanceState,
    direction_mode: DirectionMode,
    requested: &str,
) -> Result<LanguageSwitch, String> {
    let requested = read_request(requested)?;
    let resolved = resolve_catalog(&requested);
    appearance.observe_locale(resolved.locale.tag());
    let changed = appearance.use_language(resolved.catalog);
    let view = language_view(&resolved, direction_mode);
    let note = if changed {
        view.summary.clone()
    } else {
        format!("The interface is already in {}.", view.language)
    };
    Ok(LanguageSwitch {
        appearance: appearance.view(),
        view,
        note,
        changed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appearance::{AppearanceSettings, Observation, ThemeMode};
    use std::path::PathBuf;

    /// An appearance state with a pinned direction and nothing observed.
    fn window(direction: DirectionMode) -> AppearanceState {
        AppearanceState::with_observation(
            &PathBuf::from("settings.json"),
            AppearanceSettings {
                theme: ThemeMode::Light,
                direction,
            },
            Observation::default(),
        )
    }

    #[test]
    fn every_shipped_language_is_offered_in_its_own_name() {
        let options = language_options(Catalog::base());
        assert_eq!(options.len(), SUPPORTED.len());
        assert_eq!(options[0].id, "en");
        assert_eq!(options[0].label, "English");
        assert!(options[0].selected, "the language in force is marked");
        assert_eq!(options.last().expect("a language").id, "ar");
        assert!(options
            .iter()
            .any(|option| option.direction == "rtl" && option.label == "العربية"));
        for option in &options {
            assert!(option.total > 0);
            assert_eq!(option.translated, option.total, "{}", option.id);
        }
    }

    #[test]
    fn choosing_a_language_moves_the_layout_through_the_observation() {
        let mut appearance = window(DirectionMode::Auto);
        let switched = switch_language(&mut appearance, DirectionMode::Auto, "ar").expect("arabic");
        assert!(switched.changed);
        assert_eq!(switched.view.locale, "ar");
        assert_eq!(switched.view.direction, "rtl");
        assert!(switched.view.follows_language);
        // Auto follows the language, so the window is mirrored.
        assert_eq!(switched.appearance.direction, "rtl");
        // And the words really are Arabic rather than the base language's.
        assert!(switched
            .appearance
            .direction_options
            .iter()
            .any(|option| option.label == "من اليمين إلى اليسار"));
        assert_eq!(switched.appearance.language, "ar");
    }

    #[test]
    fn a_pinned_direction_survives_a_right_to_left_language() {
        let mut appearance = window(DirectionMode::Ltr);
        let switched = switch_language(&mut appearance, DirectionMode::Ltr, "ar").expect("arabic");
        assert_eq!(
            switched.view.direction, "rtl",
            "the language reads this way"
        );
        assert!(!switched.view.follows_language);
        assert_eq!(switched.appearance.direction, "ltr", "the layout does not");
        assert_eq!(switched.appearance.direction_mode, "ltr");
        // The words are still Arabic: pinning the layout does not pin the language.
        assert!(switched
            .appearance
            .direction_options
            .iter()
            .any(|option| option.label == "حسب لغتي"));
    }

    #[test]
    fn a_left_to_right_language_under_a_pinned_right_to_left_direction_stays_mirrored() {
        let mut appearance = window(DirectionMode::Rtl);
        let switched = switch_language(&mut appearance, DirectionMode::Rtl, "fr").expect("french");
        assert_eq!(switched.view.direction, "ltr");
        assert!(!switched.view.follows_language);
        assert_eq!(switched.appearance.direction, "rtl");
    }

    #[test]
    fn choosing_the_same_language_twice_reports_nothing_changed() {
        let mut appearance = window(DirectionMode::Auto);
        switch_language(&mut appearance, DirectionMode::Auto, "ja").expect("japanese");
        let again = switch_language(&mut appearance, DirectionMode::Auto, "ja").expect("japanese");
        assert!(!again.changed);
        assert_eq!(again.view.locale, "ja");
        assert_eq!(appearance.language(), "ja");
        assert!(again.note.contains("already"), "{}", again.note);
    }

    #[test]
    fn a_language_this_build_does_not_ship_falls_back_and_says_so() {
        let mut appearance = window(DirectionMode::Auto);
        let switched =
            switch_language(&mut appearance, DirectionMode::Auto, "kl").expect("greenlandic");
        assert_eq!(switched.view.locale, "en");
        assert!(
            switched.view.summary.contains("kl"),
            "{}",
            switched.view.summary
        );
        assert!(switched.view.gaps.is_empty(), "the base locale is complete");
        assert_eq!(switched.view.coverage, resolve_catalog("en").coverage());
    }

    #[test]
    fn a_regional_tag_reads_its_own_language() {
        let mut appearance = window(DirectionMode::Auto);
        let switched = switch_language(&mut appearance, DirectionMode::Auto, "es_MX")
            .expect("mexican spanish");
        assert_eq!(switched.view.locale, "es");
        assert_eq!(switched.view.language, "Español");
        assert!(
            switched.view.summary.contains("es-MX"),
            "{}",
            switched.view.summary
        );
    }

    #[test]
    fn an_empty_language_is_refused_with_the_names_that_would_work() {
        let mut appearance = window(DirectionMode::Auto);
        let problem = switch_language(&mut appearance, DirectionMode::Auto, "  ")
            .expect_err("nothing was named");
        assert!(problem.contains("en"), "{problem}");
        assert!(problem.contains("ar"), "{problem}");
    }

    #[test]
    fn the_view_reports_coverage_and_gaps_in_the_language_in_force() {
        let resolved = resolve_catalog("de");
        let view = language_view(&resolved, DirectionMode::Auto);
        assert_eq!(view.locale, "de");
        assert_eq!(view.language, "Deutsch");
        assert_eq!(view.direction, "ltr");
        assert!(view.follows_language);
        assert_eq!(view.coverage, resolved.coverage());
        assert!(view.gaps.is_empty());
        assert_eq!(view.options.iter().filter(|o| o.selected).count(), 1);
    }

    #[test]
    fn the_direction_is_the_locales_own_and_not_a_guess() {
        for locale in SUPPORTED {
            let view = language_view(&resolve_catalog(locale.tag()), DirectionMode::Auto);
            assert_eq!(view.direction, locale.direction().id(), "{}", locale.tag());
            assert_eq!(view.locale, locale.tag());
        }
    }
}
