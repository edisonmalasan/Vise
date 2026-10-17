//! The shipped translation resources, checked as a whole.
//!
//! The unit tests in `i18n` exercise the rules against files built in memory.
//! These check the files that are actually embedded: that each one is complete,
//! that every message is a sentence rather than a key, that the fallback chain
//! reaches the base locale, and that a translated preset description belongs to
//! a preset the catalog ships.

use vise_core::{
    base_locale, description_tables, locale_from_environment, message_tables, preset_description,
    resolve_catalog, resolve_locale, validate_description_files, validate_message_files, Catalog,
    Language, MatchSource, MessageKey, MessageSource, ReadingDirection, SUPPORTED,
};

/// Every locale in the shipped set, in the order the picker lists them.
#[test]
fn the_shipped_locales_are_the_ones_the_table_claims() {
    let tags: Vec<&str> = message_tables().iter().map(|table| table.tag).collect();
    let supported: Vec<&str> = SUPPORTED.iter().map(|locale| locale.tag()).collect();
    assert_eq!(tags, supported, "a locale is listed or has no file");
    assert_eq!(base_locale().tag(), "en");
    assert_eq!(SUPPORTED.len(), 6);
}

#[test]
fn every_shipped_file_is_complete_and_well_formed() {
    let problems = validate_message_files();
    assert!(problems.is_empty(), "{:#?}", problems);
    let problems = validate_description_files();
    assert!(problems.is_empty(), "{:#?}", problems);
}

#[test]
fn every_locale_has_every_key_in_its_own_language() {
    for table in message_tables() {
        let resolved = resolve_catalog(table.tag);
        assert_eq!(resolved.total(), MessageKey::ALL.len());
        assert!(
            resolved.catalog.is_complete(),
            "{}: {:?}",
            table.tag,
            resolved.gap_notes()
        );
        // Nothing in a locale file is allowed to be the key it stands for, and
        // nothing may be blank.
        for key in MessageKey::ALL {
            let text = resolved.catalog.text(key);
            assert!(!text.trim().is_empty(), "{}: {key:?}", table.tag);
            assert_ne!(text, key.name(), "{}: {key:?}", table.tag);
        }
    }
}

#[test]
fn a_language_the_build_does_not_ship_still_gets_a_window() {
    let resolved = resolve_catalog("kl");
    assert_eq!(resolved.locale.tag(), base_locale().tag());
    assert_eq!(resolved.match_source, MatchSource::Base);
    assert!(resolved.catalog.is_complete());
    assert_eq!(resolve_locale("kl").source, MatchSource::Base);
    assert!(
        resolved.note().contains("not a language this build ships"),
        "{}",
        resolved.note()
    );
    // A regional tag reads its own language rather than English.
    let matched = resolve_locale("es_MX");
    assert_eq!(matched.locale.tag(), "es");
    assert_eq!(matched.source, MatchSource::BaseLanguage);
    let regional = resolve_catalog("es_MX");
    assert!(regional.catalog.is_complete());
    assert_eq!(regional.locale.tag(), "es");
    assert_eq!(regional.match_source, MatchSource::BaseLanguage);
}

#[test]
fn the_right_to_left_language_is_a_shipped_one_and_it_is_mirrored() {
    let arabic = SUPPORTED
        .iter()
        .find(|locale| locale.direction() == ReadingDirection::Rtl)
        .expect("a right-to-left language is shipped");
    assert_eq!(arabic.language(), Language::Arabic);
    assert_eq!(arabic.endonym(), "العربية");
    // And its own text is in the table rather than the base language's.
    let resolved = resolve_catalog(arabic.tag());
    let text = resolved.catalog.text(MessageKey::QueueWaiting);
    assert!(text.contains('{'), "{text}");
    assert_ne!(text, Catalog::base().text(MessageKey::QueueWaiting));
}

#[test]
fn a_translated_preset_description_belongs_to_a_preset_and_the_rest_fall_back() {
    let described = description_tables()
        .iter()
        .flat_map(|table| {
            vise_core::MessageFile::parse(table.tag, table.text)
                .expect("a description file is readable")
                .entries()
                .keys()
                .cloned()
                .collect::<Vec<String>>()
        })
        .count();
    assert!(described > 0, "some presets are translated");
    assert!(
        described < vise_core::builtin_entries().len(),
        "and not all of them, which is what the fallback is for"
    );

    // A translated one comes back as a translation.
    let found = vise_core::localized_description("general-very-fast-480-30-h264", "es")
        .expect("the preset is in the catalog");
    assert!(found.translated);
    assert_eq!(found.source, MessageSource::Locale);
    assert_ne!(found.text, "The quickest encode in the ladder");

    // An untranslated one comes back as the catalog's own words, marked as
    // base text rather than as a translation.
    let untranslated = vise_core::localized_description("general-super-hq-480-30-h264", "es")
        .expect("the preset is in the catalog");
    assert!(!untranslated.translated);
    assert_eq!(untranslated.source, MessageSource::Base);
    let entry = vise_core::builtin_entry("general-very-fast-480-30-h264").expect("the preset");
    assert_eq!(
        untranslated.text,
        vise_core::builtin_entry("general-super-hq-480-30-h264")
            .expect("the preset")
            .description
    );

    // The base locale is the catalog, so a lookup there is the catalog.
    let base = vise_core::localized_description("general-very-fast-480-30-h264", "en")
        .expect("the preset is in the catalog");
    assert!(!base.translated);
    assert_eq!(base.text, entry.description);
    assert!(preset_description("en", "general-very-fast-480-30-h264").is_none());

    // Every preset in the catalog has a description in every shipped locale,
    // whether that is a translation or the catalog's own sentence.
    for table in message_tables() {
        for entry in vise_core::builtin_entries() {
            let found = vise_core::localized_description(&entry.id, table.tag)
                .expect("a shipped preset has a description");
            assert!(!found.text.trim().is_empty(), "{} {}", table.tag, entry.id);
        }
    }

    // A preset this build does not ship has no description in any language,
    // which is a different answer from an untranslated one.
    assert!(vise_core::localized_description("no-such-preset", "es").is_none());
}

#[test]
fn the_locale_the_environment_names_is_the_one_the_window_opens_in() {
    // The environment of a test run is not a language this build has to
    // understand, so the assertion is about the shape of the answer rather than
    // about a particular language: whatever is named, it resolves to a shipped
    // locale and never to a missing one.
    let locale = locale_from_environment();
    assert!(SUPPORTED
        .iter()
        .any(|supported| supported.tag() == locale.tag()));
    let resolved = resolve_catalog(locale.tag());
    assert!(resolved.catalog.is_complete());
}
