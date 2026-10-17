//! Reading, writing, and migrating the settings document.
//!
//! The settings file is the one place a user configures the application, so the
//! interesting cases are all about what happens to a file that is not exactly
//! what this build writes: no version, an old version, a version from the
//! future, one member of the wrong shape, and a file that is not a document at
//! all. Each of those has one right answer, and losing somebody's settings to a
//! version number is the failure this file exists to prevent.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use vise_core::{
    load_settings, read_settings, save_settings, AppSettings, SETTINGS_UNVERSIONED, SETTINGS_VERSION,
};

fn temp_path(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("vise-settings-{name}-{nanos}.json"));
    let _ = fs::remove_file(&path);
    path
}

/// Write a settings file whose contents are exactly `body`.
fn write_raw(path: &PathBuf, body: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent");
    }
    fs::write(path, body).expect("write settings");
}

#[test]
fn a_saved_document_says_which_build_wrote_it_and_reads_back_unchanged() {
    let path = temp_path("round-trip");
    let mut written = AppSettings::default();
    written.default_preset = Some("high-quality".to_string());
    written.output_directory = Some("C:\\media\\out".to_string());
    written
        .extra
        .insert("preferences".to_string(), serde_json::json!({"logLevel": "info"}));
    save_settings(&path, &written).expect("saved");

    let body: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("json");
    assert_eq!(body["version"], SETTINGS_VERSION);

    let loaded = load_settings(&path).expect("loaded");
    assert_eq!(loaded, written, "a document read back is the document saved");
    assert_eq!(loaded.written_version, SETTINGS_VERSION);
    assert!(!loaded.needs_migration());
    assert!(!loaded.from_the_future());
}

#[test]
fn a_file_that_declares_no_version_is_read_as_the_oldest_one_and_migrated_forward() {
    let path = temp_path("unversioned");
    write_raw(
        &path,
        r#"{"default_preset":"fast-1080p","output_directory":"C:\\out","recent_sources":["C:\\a.mkv"]}"#,
    );

    let loaded = load_settings(&path).expect("loaded");
    assert_eq!(loaded.written_version, SETTINGS_UNVERSIONED);
    assert!(loaded.needs_migration());
    assert!(!loaded.from_the_future());
    // Nothing is lost on the way in, including a record this build has never
    // heard of.
    assert_eq!(loaded.default_preset.as_deref(), Some("fast-1080p"));
    assert_eq!(loaded.output_directory.as_deref(), Some("C:\\out"));
    assert!(loaded.extra.contains_key("recent_sources"));

    // Saving it moves it forward, and only the declaration changes: every member
    // the older file carried is written back out.
    save_settings(&path, &loaded).expect("saved");
    let body: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("json");
    assert_eq!(body["version"], SETTINGS_VERSION);
    assert_eq!(body["default_preset"], "fast-1080p");
    assert_eq!(body["output_directory"], "C:\\out");
    assert!(body.get("recent_sources").is_some());

    let again = load_settings(&path).expect("loaded again");
    assert!(!again.needs_migration());
    assert_eq!(again, loaded);
}

#[test]
fn a_file_from_a_newer_build_is_never_downgraded() {
    let path = temp_path("future");
    let newer = SETTINGS_VERSION + 4;
    write_raw(
        &path,
        &format!(
            r#"{{"version":{newer},"output_directory":"C:\\out","something_new":{{"kept":true}}}}"#
        ),
    );

    let loaded = load_settings(&path).expect("loaded");
    assert!(loaded.from_the_future());
    assert!(!loaded.needs_migration());
    assert_eq!(loaded.version, newer, "the number found is the number kept");
    assert_eq!(
        loaded.extra.get("something_new"),
        Some(&serde_json::json!({"kept": true})),
        "a member this build does not know survives being read"
    );

    // Writing it back must not claim to be a version it is not, or the build
    // that wrote it would no longer recognise its own file.
    save_settings(&path, &loaded).expect("saved");
    let body: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("json");
    assert_eq!(body["version"], newer);
    assert_eq!(body["something_new"]["kept"], true);
}

#[test]
fn one_member_of_the_wrong_shape_costs_that_member_and_not_the_file() {
    let document = serde_json::json!({
        "version": SETTINGS_VERSION,
        "default_preset": 42,
        "output_directory": "C:\\out",
        "recent_sources": ["C:\\a.mkv"],
    });
    let loaded = read_settings(&document).expect("read");
    assert_eq!(loaded.default_preset, None, "a number is not a preset name");
    assert_eq!(loaded.output_directory.as_deref(), Some("C:\\out"));
    assert!(loaded.extra.contains_key("recent_sources"));
}

#[test]
fn a_version_that_is_not_a_whole_number_is_treated_as_unversioned() {
    let document = serde_json::json!({"version": "two", "output_directory": "C:\\out"});
    let loaded = read_settings(&document).expect("read");
    assert_eq!(loaded.written_version, SETTINGS_UNVERSIONED);
    assert!(loaded.needs_migration());
    assert_eq!(loaded.version, SETTINGS_VERSION);
    assert_eq!(loaded.output_directory.as_deref(), Some("C:\\out"));
}

#[test]
fn a_document_that_is_not_an_object_is_refused_rather_than_overwritten() {
    for body in ["[]", "\"dark\"", "12", "null"] {
        let document: serde_json::Value = serde_json::from_str(body).expect("json");
        assert!(
            read_settings(&document).is_err(),
            "a settings file of '{body}' is not something to read"
        );
    }
}

#[test]
fn a_missing_empty_or_broken_file_reads_as_the_defaults_or_an_error() {
    let missing = temp_path("missing");
    assert_eq!(load_settings(&missing).expect("missing"), AppSettings::default());
    assert_eq!(load_settings(&PathBuf::from("no/such/folder/settings.json")).expect("absent")
        .written_version, 0);

    let empty = temp_path("empty");
    write_raw(&empty, "   \n");
    assert_eq!(load_settings(&empty).expect("empty"), AppSettings::default());

    let broken = temp_path("broken");
    write_raw(&broken, "{ not json");
    let error = load_settings(&broken).expect_err("broken");
    assert!(error.to_string().contains("JSON"), "{error}");

    // A defaults object is what a caller that was handed an error falls back to,
    // and it already carries the current version so the next save is stamped.
    assert_eq!(AppSettings::default().version, SETTINGS_VERSION);
    assert_eq!(AppSettings::default().written_version, 0);
}

#[test]
fn a_partly_missing_document_keeps_every_member_it_does_have() {
    let document = serde_json::json!({
        "version": SETTINGS_VERSION,
        "user_presets_path": "C:\\presets.json",
        "appearance": {"theme": "dark", "direction": "rtl"},
    });
    let loaded = read_settings(&document).expect("read");
    assert_eq!(loaded.user_presets_path.as_deref(), Some("C:\\presets.json"));
    assert_eq!(loaded.default_preset, None);
    assert_eq!(loaded.output_directory, None);
    assert_eq!(loaded.recent_input, None);
    // A record this build does not know is bagged whole, not half-read.
    assert_eq!(
        loaded.extra.get("appearance"),
        Some(&serde_json::json!({"theme": "dark", "direction": "rtl"}))
    );
}

#[test]
fn the_version_questions_are_answered_from_the_file_and_not_from_the_clock() {
    let current = AppSettings {
        written_version: SETTINGS_VERSION,
        ..AppSettings::default()
    };
    assert!(!current.needs_migration());
    assert!(!current.from_the_future());
    assert_eq!(current.written_version_label(), format!("version {SETTINGS_VERSION}"));

    let never_read = AppSettings::default();
    assert!(!never_read.needs_migration());
    assert!(!never_read.from_the_future());
    assert_eq!(never_read.written_version_label(), "no version");

    let old = AppSettings {
        written_version: SETTINGS_UNVERSIONED,
        ..AppSettings::default()
    };
    assert!(old.needs_migration());

    let future = AppSettings {
        written_version: SETTINGS_VERSION + 1,
        ..AppSettings::default()
    };
    assert!(!future.needs_migration());
    assert!(future.from_the_future());
}