use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use vise_core::{
    apply_preset, builtin_presets, export_preset, import_preset, load_settings, save_settings,
    AppSettings, JobSpec, PresetSpec, UserPresetStore,
};

fn tmp_path(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("vise-test-{name}-{nanos}"))
}

#[test]
fn builtin_preset_exists() {
    let presets = builtin_presets();
    assert!(!presets.is_empty());
    assert!(presets.iter().any(|p| p.id == "general-fast-1080-30-h264"));
    // An identifier an earlier build stored still resolves, because it is in
    // other people's settings files.
    let older = vise_core::builtin_preset("fast-1080p").expect("the older identifier");
    assert_eq!(older.id, "general-fast-1080-30-h264");
}

#[test]
fn applying_preset_changes_job_fields() {
    let base = JobSpec::default();
    let preset = PresetSpec {
        container: Some(vise_core::ContainerFormat::Mkv),
        video_codec: Some(vise_core::VideoCodec::H265),
        quality: Some(18),
        ..Default::default()
    };
    let updated = apply_preset(&preset, &base);
    assert_eq!(updated.container, vise_core::ContainerFormat::Mkv);
    assert_eq!(updated.video_codec, vise_core::VideoCodec::H265);
    assert_eq!(updated.quality, Some(18));
}

#[test]
fn user_create_edit_delete_persists() {
    let path = tmp_path("presets").join("presets.json");
    let _ = fs::create_dir_all(path.parent().unwrap());
    let store = UserPresetStore::new(&path);
    let created = store.create("My Preset", PresetSpec::default()).unwrap();
    assert_eq!(store.list().unwrap().len(), 1);
    let edited = store.edit(&created.id, Some("Renamed"), None).unwrap();
    assert_eq!(edited.name, "Renamed");
    let reloaded = UserPresetStore::new(&path).list().unwrap();
    assert_eq!(reloaded[0].name, "Renamed");
    store.delete(&created.id).unwrap();
    assert!(UserPresetStore::new(&path).list().unwrap().is_empty());
}

#[test]
fn roundtrip_import_export() {
    let path = tmp_path("preset.json");
    let spec = PresetSpec {
        format: Some(vise_core::ContainerFormat::Webm),
        ..Default::default()
    };
    export_preset(&path, &spec).unwrap();
    let imported = import_preset(&path).unwrap();
    assert_eq!(imported.format, Some(vise_core::ContainerFormat::Webm));
}

#[test]
fn settings_persistence() {
    let path = tmp_path("settings.json");
    let s = AppSettings {
        default_preset: Some("fast-1080p".to_string()),
        ..AppSettings::default()
    };
    save_settings(&path, &s).unwrap();
    let loaded = load_settings(&path).unwrap();
    assert_eq!(loaded, s);
}
