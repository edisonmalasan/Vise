//! The built-in catalog, its version, and what it means on a machine.
//!
//! These are the checks the catalog itself has to pass before a build that
//! carries it is worth shipping: that every entry is a valid preset, that every
//! entry survives being cleaned and written and read back, that the version
//! answers the three questions it exists for, that a copy is a copy, and that a
//! preset written for a chip behaves sensibly on a machine without one.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use vise_core::{
    builtin_entries, builtin_entry, builtin_preset, builtin_presets, category_counts, clone_entry,
    clone_user_preset, export_user_preset, import_user_preset, move_user_preset, place_user_preset,
    read_preset_document, user_preset_order, AudioCodec, BuiltInPreset, ContainerFormat, EncoderId,
    EncodingMode, HardwareAcceleration, HardwareApi, HardwareCapabilities, HardwareFamily,
    PresetCategory, PresetEntry, PresetSpec, PresetVersion, UserPreset, VideoCodec,
};

fn temp_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("vise-catalog-{name}-{nanos}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// A machine with every encoder the catalog asks for.
fn everything() -> HardwareCapabilities {
    HardwareCapabilities {
        ffmpeg_path: Some(PathBuf::from("ffmpeg")),
        hwaccels: vec![
            "cuda".to_string(),
            "qsv".to_string(),
            "amf".to_string(),
            "d3d11va".to_string(),
            "videotoolbox".to_string(),
        ],
        decoders: vec!["h264".to_string()],
        encoders: vec![
            "libx264  H.264".to_string(),
            "libx265  HEVC".to_string(),
            "libaom-av1  AV1".to_string(),
            "libvpx-vp9  VP9".to_string(),
            "prores_ks".to_string(),
            "dnxhd".to_string(),
            "ffv1".to_string(),
            "h264_nvenc".to_string(),
            "hevc_nvenc".to_string(),
            "av1_nvenc".to_string(),
            "h264_qsv".to_string(),
            "hevc_qsv".to_string(),
            "av1_qsv".to_string(),
            "h264_amf".to_string(),
            "hevc_amf".to_string(),
            "av1_amf".to_string(),
            "h264_videotoolbox".to_string(),
            "hevc_videotoolbox".to_string(),
        ],
    }
}

/// A machine with only the software encoders.
fn software_only() -> HardwareCapabilities {
    let mut caps = everything();
    caps.hwaccels.clear();
    caps.encoders
        .retain(|name| name.starts_with("lib") || name == "ffv1");
    caps
}

/// A machine whose media tools could not be found at all.
fn unprobed() -> HardwareCapabilities {
    HardwareCapabilities::default()
}

#[test]
fn the_catalog_reads_and_says_which_version_it_is() {
    let catalog = vise_core::builtin_catalog();
    assert_eq!(
        catalog.version,
        vise_core::PRESET_SCHEMA_VERSION,
        "the asset and the constant have to agree"
    );
    assert_eq!(
        catalog.version.label(),
        "1.0.0",
        "the version is shown to people, so it reads as three numbers"
    );
    assert!(
        !catalog.presets.is_empty(),
        "a build that ships no presets has nothing to browse"
    );
    assert!(
        catalog.template.container.is_some(),
        "the template is what an incomplete preset is filled from, so it has to say something"
    );
    // The asset is embedded, so a build carries its presets rather than looking
    // for them at run time.
    assert!(vise_core::BUILTIN_CATALOG_JSON.contains("\"presets\""));
}

#[test]
fn all_six_categories_are_filed_under_and_nothing_is_empty() {
    let counts = category_counts();
    assert_eq!(counts.len(), 6, "{counts:?}");
    for (category, count) in &counts {
        assert!(*count > 0, "{} has no presets", category.label());
    }
    let names: Vec<&str> = counts.iter().map(|(c, _)| c.id()).collect();
    assert_eq!(
        names,
        vec![
            "general",
            "web",
            "devices",
            "matroska",
            "hardware",
            "professional"
        ]
    );
    let total: usize = counts.iter().map(|(_, count)| count).sum();
    assert_eq!(total, builtin_entries().len());
}

#[test]
fn every_entry_is_a_valid_preset() {
    let mut seen = BTreeSet::new();
    for entry in builtin_entries() {
        entry
            .validate()
            .unwrap_or_else(|problems| panic!("{}: {problems}", entry.id));
        assert!(!entry.name.trim().is_empty(), "{}", entry.id);
        assert!(
            !entry.description.trim().is_empty(),
            "'{}' explains nothing",
            entry.id
        );
        assert!(
            seen.insert(entry.id.clone()),
            "'{}' is in the catalog twice",
            entry.id
        );
    }
    assert_eq!(seen.len(), builtin_entries().len());
}

#[test]
fn the_general_ladder_covers_the_four_quality_families_and_three_codecs() {
    let ladder = ["480p30", "576p25", "720p30", "1080p30", "2160p60"];
    let families = ["Very Fast", "Fast", "HQ", "Super HQ"];
    let codecs = [VideoCodec::H264, VideoCodec::H265, VideoCodec::Av1];
    let general: Vec<&PresetEntry> = builtin_entries()
        .iter()
        .filter(|entry| entry.category == PresetCategory::General)
        .collect();
    for family in families {
        for resolution in ladder {
            for codec in codecs {
                let wanted = format!("{family} {resolution} (");
                assert!(
                    general.iter().any(|entry| entry.name.starts_with(&wanted)
                        && entry.spec.video_codec_of() == Some(codec)),
                    "the general ladder is missing {family} {resolution} {codec:?}"
                );
            }
        }
    }
    // Each rung is a different encoder speed, which is what makes it a family
    // rather than four names for the same encode.
    let speeds: BTreeSet<String> = general
        .iter()
        .filter(|entry| entry.spec.video_codec_of() == Some(VideoCodec::H264))
        .filter_map(|entry| {
            entry
                .spec
                .video_encoding
                .as_ref()
                .and_then(|encoding| encoding.preset.clone())
        })
        .collect();
    assert!(
        speeds.len() >= 4,
        "the H.264 family should differ by encoder speed, saw {speeds:?}"
    );
}

#[test]
fn the_web_presets_target_a_creator_size_or_a_social_file_size() {
    let web = vise_core::builtin_entries_in(PresetCategory::Web);
    let creators: Vec<&&PresetEntry> = web
        .iter()
        .filter(|entry| entry.name.starts_with("Creator "))
        .collect();
    for resolution in ["720p60", "1080p60", "1440p60", "2160p60"] {
        assert!(
            creators.iter().any(|entry| entry.name.contains(resolution)),
            "the creator series has no {resolution}"
        );
    }
    let social: Vec<&&PresetEntry> = web
        .iter()
        .filter(|entry| entry.name.starts_with("Social "))
        .collect();
    assert_eq!(social.len(), 8, "two sizes by four duration caps");
    for entry in &social {
        assert_eq!(
            entry.spec.encoding_mode.map(|mode| mode.is_target_size()),
            Some(true),
            "{} is size-targeted",
            entry.id
        );
        assert!(entry.spec.target_size.is_some(), "{}", entry.id);
        let cap = entry.spec.duration_cap_seconds.expect("a duration cap");
        assert!((30..=300).contains(&cap), "{} caps at {cap}s", entry.id);
    }
    let sizes: BTreeSet<u64> = social
        .iter()
        .filter_map(|entry| entry.spec.target_size)
        .collect();
    assert_eq!(
        sizes,
        BTreeSet::from([10_000_000, 25_000_000]),
        "the social series is 10 MB and 25 MB, in the same megabytes the queue uses"
    );
}

#[test]
fn the_device_presets_cover_the_seven_families_and_say_what_they_are_for() {
    let devices = vise_core::builtin_entries_in(PresetCategory::Devices);
    let families = [
        ("Amazon Fire", "fire"),
        ("Android", "android"),
        ("Apple", "apple"),
        ("Chromecast", "chromecast"),
        ("Playstation", "playstation"),
        ("Roku", "roku"),
        ("Xbox", "xbox"),
    ];
    for (label, key) in families {
        let mine: Vec<&&PresetEntry> = devices
            .iter()
            .filter(|entry| entry.id.starts_with(&format!("device-{key}-")))
            .collect();
        assert!(!mine.is_empty(), "no device presets for {label}");
        for entry in &mine {
            let compatibility = entry.compatibility.as_deref().unwrap_or("");
            assert!(
                !compatibility.trim().is_empty(),
                "'{}' is a device preset and says nothing about devices",
                entry.id
            );
        }
    }
}

#[test]
fn the_matroska_presets_cover_the_four_codecs() {
    let mkv = vise_core::builtin_entries_in(PresetCategory::Matroska);
    for codec in [
        VideoCodec::H264,
        VideoCodec::H265,
        VideoCodec::Vp9,
        VideoCodec::Av1,
    ] {
        assert!(
            mkv.iter()
                .any(|entry| entry.spec.video_codec_of() == Some(codec)),
            "no Matroska preset for {codec:?}"
        );
    }
    for entry in &mkv {
        assert_eq!(entry.spec.container_of(), Some(ContainerFormat::Mkv));
    }
}

#[test]
fn the_hardware_presets_cover_the_five_families_and_name_a_chip() {
    let hardware = vise_core::builtin_entries_in(PresetCategory::Hardware);
    for family in [
        HardwareFamily::Qsv,
        HardwareFamily::Nvenc,
        HardwareFamily::Vcn,
        HardwareFamily::MediaFoundation,
        HardwareFamily::VideoToolbox,
    ] {
        assert!(
            hardware.iter().any(|entry| entry.family == Some(family)),
            "no hardware presets for {}",
            family.label()
        );
    }
    for entry in &hardware {
        let encoder = entry.encoder.expect("a hardware preset names an encoder");
        assert_eq!(
            entry.encoder.and_then(|encoder| encoder.hardware_family()),
            if entry.family == Some(HardwareFamily::MediaFoundation) {
                None
            } else {
                entry.family
            },
            "'{}' names an encoder from another family",
            entry.id
        );
        assert!(
            encoder.is_hardware() || entry.family == Some(HardwareFamily::MediaFoundation),
            "'{}' is in the hardware category without a hardware encoder",
            entry.id
        );
    }
}

#[test]
fn the_professional_presets_cover_the_three_families() {
    let pro = vise_core::builtin_entries_in(PresetCategory::Professional);
    for encoder in [EncoderId::ProRes, EncoderId::Dnxhr, EncoderId::Ffv1] {
        assert!(
            pro.iter().any(|entry| entry.encoder == Some(encoder)),
            "no professional preset for {}",
            encoder.label()
        );
    }
    for entry in &pro {
        if entry.encoder == Some(EncoderId::ProRes) {
            assert_eq!(entry.spec.video_codec_of(), Some(VideoCodec::ProRes));
        }
    }
    let preserved: Vec<&&PresetEntry> = pro
        .iter()
        .filter(|entry| entry.encoder == Some(EncoderId::Ffv1))
        .collect();
    assert!(preserved.len() >= 2, "preservation is more than one choice");
    for entry in preserved {
        assert_eq!(
            entry.spec.encoding_mode.map(|mode| mode.is_lossless()),
            Some(true),
            "'{}' is a preservation master",
            entry.id
        );
        let audio = entry.spec.audio_codec_of().expect("audio");
        assert!(
            audio.is_lossless(),
            "'{}' preserves the video and throws the audio away",
            entry.id
        );
    }
}

#[test]
fn every_built_in_preset_normalizes_the_same_way_twice() {
    let template = vise_core::preset_template();
    for entry in builtin_entries() {
        let once = entry.spec.normalized(&template);
        let twice = once.normalized(&template);
        assert_eq!(
            once, twice,
            "'{}' is not normalized once and for all",
            entry.id
        );
        assert_eq!(once.cleaned(), once, "'{}' is not clean", entry.id);
    }
}

#[test]
fn every_built_in_preset_round_trips_through_a_file() {
    let dir = temp_dir("round-trip");
    for entry in builtin_entries() {
        let path = dir.join(format!("{}.json", entry.id));
        vise_core::export_preset_entry(&path, entry).expect("export");
        let read = vise_core::import_preset_entry(&path).expect("import");
        assert_eq!(read.entry.id, entry.id);
        assert_eq!(read.entry.name, entry.name);
        assert_eq!(read.entry.category, entry.category);
        assert_eq!(read.entry.encoder, entry.encoder);
        assert_eq!(
            read.entry.spec, entry.spec,
            "'{}' came back different from what was written",
            entry.id
        );
        // Writing what was read has to produce the same file, or the round trip
        // is only lossless by accident.
        let again = dir.join(format!("{}-again.json", entry.id));
        vise_core::export_preset_entry(&again, &read.entry).expect("re-export");
        assert_eq!(
            fs::read_to_string(&path).expect("first"),
            fs::read_to_string(&again).expect("second"),
            "'{}' is not stable through a second write",
            entry.id
        );
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_stored_preset_survives_a_round_trip_through_a_file() {
    let dir = temp_dir("user-round-trip");
    let path = dir.join("mine.json");
    let mut stored = UserPreset {
        id: "user-1".to_string(),
        name: "Mine".to_string(),
        description: Some("For the test.".to_string()),
        preset: PresetSpec {
            container: Some(ContainerFormat::Mkv),
            video_codec: Some(VideoCodec::Av1),
            audio_codec: Some(AudioCodec::Flac),
            quality: Some(19),
            encoder: Some(EncoderId::AmfAv1),
            encoding_mode: Some(EncodingMode::TargetSize),
            target_size: Some(25 * 1024 * 1024),
            duration_cap_seconds: Some(120),
            ..Default::default()
        },
    };
    stored.preset.name = Some("Mine".to_string());
    export_user_preset(&path, &stored).expect("export");
    let raw = fs::read_to_string(&path).expect("read");
    assert!(
        raw.contains("\"major\""),
        "the file says which build wrote it"
    );
    let read = import_user_preset(&path).expect("import");
    assert_eq!(read, stored, "a round trip through a file lost something");

    // A stored preset that still names the same encoder and rate control has to
    // put them back on a job, not just compare equal as data.
    let job = stored.preset.apply_to(&vise_core::JobSpec::default());
    assert_eq!(
        job.hardware_acceleration.encode.encoder.as_deref(),
        Some("av1_amf")
    );
    assert_eq!(job.hardware_acceleration.encode.api, HardwareApi::Amf);
    assert!(job.target_size_mode);
    assert_eq!(job.target_size, Some(25 * 1024 * 1024));
    assert_eq!(job.video_codec, VideoCodec::Av1);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_file_from_a_later_minor_is_read_and_keeps_what_this_build_does_not_know() {
    let document = serde_json::json!({
        "version": { "major": 1, "minor": 3, "micro": 0 },
        "entry": {
            "id": "theirs",
            "name": "Theirs",
            "description": "Written by a build that knows one more thing.",
            "category": "general",
            "encoder": "libx265",
            "spec": { "container": "mkv", "video_codec": "h265" },
            "somethingNew": { "kept": true }
        },
        "documentMember": "also kept"
    });
    let read = read_preset_document(&document).expect("read");
    assert_eq!(read.version, PresetVersion::new(1, 3, 0));
    assert_eq!(read.entry.spec.video_codec_of(), Some(VideoCodec::H265));
    assert_eq!(
        read.extra.get("documentMember"),
        Some(&serde_json::json!("also kept")),
        "a member a later build added has to survive being read"
    );
}

#[test]
fn a_file_from_a_later_major_is_refused_by_name_rather_than_half_read() {
    let document = serde_json::json!({
        "version": { "major": 2, "minor": 0, "micro": 0 },
        "entry": {
            "id": "theirs",
            "name": "Theirs",
            "description": "From a build this one knows nothing about.",
            "category": "general",
            "spec": { "container": "mp4" }
        }
    });
    let error = read_preset_document(&document).expect_err("refused");
    assert!(
        error.to_string().contains("2.0.0"),
        "the refusal names the version: {error}"
    );
}

#[test]
fn a_file_written_before_the_version_existed_is_migrated_and_kept() {
    let legacy = serde_json::json!({
        "id": "old",
        "name": "Old",
        "description": "Written before versions and before the settings were renamed.",
        "category": "general",
        "preset": {
            "format": "webm",
            "video": "vp9",
            "audio": "opus",
            "quality": 30,
            "video_encoding": { "preset": "good" }
        }
    });
    let read = read_preset_document(&legacy).expect("read");
    assert_eq!(read.version, vise_core::PRESET_SCHEMA_VERSION);
    assert_eq!(read.entry.spec.container_of(), Some(ContainerFormat::Webm));
    assert_eq!(read.entry.spec.video_codec_of(), Some(VideoCodec::Vp9));
    assert_eq!(read.entry.spec.audio_codec_of(), Some(AudioCodec::Opus));
    assert_eq!(
        read.entry
            .spec
            .video_encoding
            .as_ref()
            .and_then(|e| e.preset.clone()),
        Some("good".to_string()),
        "a member that was not renamed has to survive the migration"
    );
    // Once migrated, the file it came from can be written and read again.
    let dir = temp_dir("legacy");
    let path = dir.join("old.json");
    vise_core::export_preset_entry(&path, &read.entry).expect("write");
    let again = vise_core::import_preset_entry(&path).expect("read");
    assert_eq!(again.entry.spec, read.entry.spec);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_preset_file_that_is_not_a_preset_is_refused_with_a_reason() {
    assert!(read_preset_document(&serde_json::json!([1, 2, 3]))
        .expect_err("an array is not a document")
        .to_string()
        .contains("JSON object"));
    assert!(read_preset_document(&serde_json::json!({"version": 1}))
        .expect_err("no preset named")
        .to_string()
        .contains("preset"));
    assert!(
        read_preset_document(&serde_json::json!({
            "entry": { "id": "x", "name": "X", "description": "d", "category": "general", "spec": {} },
            "extra": {"broken": true}
        }))
        .is_ok(),
        "unknown members do not stop a document being read"
    );
}

#[test]
fn cleaning_folds_the_duplicated_members_and_can_be_run_twice() {
    let untidy = PresetSpec {
        format: Some(ContainerFormat::Webm),
        video: Some(VideoCodec::Vp9),
        audio: Some(AudioCodec::Opus),
        subtitles: Some(vise_core::SubtitleMode::BurnIn),
        title: Some(3),
        name: Some("  Mine  ".to_string()),
        description: Some("   ".to_string()),
        scale_to: Some((0, 100)),
        crop: Some((0, 0, 0, 0)),
        ..Default::default()
    };
    let clean = untidy.cleaned();
    assert_eq!(clean.container, Some(ContainerFormat::Webm));
    assert_eq!(
        clean.format, None,
        "the copy this build does not read is dropped"
    );
    assert_eq!(clean.video_codec, Some(VideoCodec::Vp9));
    assert_eq!(clean.audio_codec, Some(AudioCodec::Opus));
    assert_eq!(clean.subtitle_mode, Some(vise_core::SubtitleMode::BurnIn));
    assert_eq!(clean.subtitles, None);
    assert_eq!(clean.title_index, Some(3));
    assert_eq!(clean.name.as_deref(), Some("Mine"));
    assert_eq!(clean.description, None, "blank text is not a description");
    assert_eq!(clean.scale_to, None, "a zero width is not a size");
    assert_eq!(clean.crop, None);
    assert_eq!(clean.cleaned(), clean);

    // When both spellings are present, the one this build reads wins.
    let both = PresetSpec {
        container: Some(ContainerFormat::Mp4),
        format: Some(ContainerFormat::Webm),
        ..Default::default()
    };
    assert_eq!(both.cleaned().container, Some(ContainerFormat::Mp4));
}

#[test]
fn normalizing_fills_from_the_template_and_prunes_what_the_template_answers() {
    let template = vise_core::preset_template();
    assert_eq!(
        template.quality,
        Some(23),
        "the test uses the shipped template"
    );
    let empty = PresetSpec::default();
    let filled = empty.normalized(&template);
    assert_eq!(filled.container, Some(ContainerFormat::Mp4));
    assert_eq!(filled.video_codec, Some(VideoCodec::H264));
    assert_eq!(filled.audio_codec, Some(AudioCodec::Aac));
    assert_eq!(
        filled.quality, None,
        "the template's quality is not the preset's own"
    );
    assert_eq!(filled.normalized(&template), filled);

    // The container and the codecs are what the preset is, so they are never
    // pruned even when the template agrees with them.
    let agreeing = PresetSpec {
        container: Some(ContainerFormat::Mp4),
        video_codec: Some(VideoCodec::H264),
        audio_codec: Some(AudioCodec::Aac),
        quality: Some(23),
        ..Default::default()
    };
    let normalized = agreeing.normalized(&template);
    assert_eq!(normalized.container, Some(ContainerFormat::Mp4));
    assert_eq!(normalized.video_codec, Some(VideoCodec::H264));
    assert_eq!(normalized.quality, None);

    let own = PresetSpec {
        quality: Some(17),
        ..Default::default()
    };
    assert_eq!(own.normalized(&template).quality, Some(17));
}

#[test]
fn every_preset_is_available_on_a_machine_that_has_everything() {
    for entry in builtin_entries() {
        let resolution = entry.resolve(&everything(), &HardwareAcceleration::auto());
        assert!(
            resolution.available,
            "'{}' is unavailable on a machine that has every encoder: {}",
            entry.id, resolution.reason
        );
        assert!(
            !resolution.fallback,
            "'{}' fell back on a machine that has everything",
            entry.id
        );
    }
}

#[test]
fn a_software_only_machine_falls_back_rather_than_refusing() {
    let all_hardware = vise_core::builtin_entries_in(PresetCategory::Hardware);
    let hardware: Vec<&PresetEntry> = all_hardware
        .into_iter()
        .filter(|entry| entry.family != Some(HardwareFamily::MediaFoundation))
        .collect();
    assert!(!hardware.is_empty());
    for entry in &hardware {
        let wanted = entry.encoder.expect("an encoder");
        let resolution = entry.resolve(&software_only(), &HardwareAcceleration::auto());
        assert!(
            resolution.available,
            "'{}' refused a job it could do",
            entry.id
        );
        assert!(
            resolution.fallback,
            "'{}' did not say it substituted",
            entry.id
        );
        assert!(!resolution.hardware);
        assert_eq!(
            resolution.encoder,
            wanted.software_stand_in().expect("a software stand-in"),
            "'{}' substituted the wrong encoder",
            entry.id
        );
        assert!(
            resolution.note.contains(wanted.identifier()),
            "the note names what was missing: {}",
            resolution.note
        );
    }
    // A machine with the chip gets the chip, and is told so.
    for entry in &hardware {
        let resolution = entry.resolve(&everything(), &HardwareAcceleration::auto());
        assert!(resolution.hardware, "'{}' ran in software", entry.id);
        assert!(!resolution.fallback);
        assert_eq!(resolution.encoder, entry.encoder.unwrap());
    }
}

#[test]
fn switching_hardware_off_substitutes_rather_than_refuses() {
    let entry = builtin_entry("hardware-nvenc-hevc-1080-30").expect("an NVENC preset");
    let resolution = entry.resolve(&everything(), &HardwareAcceleration::disabled());
    assert!(resolution.available);
    assert!(resolution.fallback);
    assert_eq!(resolution.encoder, EncoderId::X265);
    assert!(
        resolution.note.contains("switched off"),
        "the note says the user did it: {}",
        resolution.note
    );
}

#[test]
fn an_unprobed_machine_closes_nothing_and_says_it_has_not_checked() {
    for entry in builtin_entries() {
        let resolution = entry.resolve(&unprobed(), &HardwareAcceleration::auto());
        assert!(
            resolution.available,
            "'{}' was closed on the strength of a probe that never ran",
            entry.id
        );
    }
}

#[test]
fn an_encoder_this_machine_lacks_at_all_is_unavailable_with_a_reason() {
    // A media-tools build without ProRes has nothing to fall back to: a proxy is
    // not an interchange master, so the preset is refused rather than given
    // something else.
    let entry = builtin_entry("professional-prores-422-hq").expect("a ProRes preset");
    let mut caps = software_only();
    caps.encoders.retain(|name| name != "prores_ks");
    let resolution = entry.resolve(&caps, &HardwareAcceleration::auto());
    assert!(!resolution.available, "{}", resolution.note);
    assert!(
        resolution.reason.contains("prores_ks"),
        "the reason names the encoder that is missing: {}",
        resolution.reason
    );
    assert!(
        resolution.reason.contains("No other encoder"),
        "{}",
        resolution.reason
    );

    // A software codec the media tools do not have is refused as well: there is
    // nothing else in this build that writes H.265 either.
    let mut without_x265 = entry.clone();
    without_x265.encoder = Some(EncoderId::X265);
    without_x265.spec.video_codec = Some(VideoCodec::H265);
    let mut caps = software_only();
    caps.encoders.retain(|name| !name.contains("libx265"));
    let resolution = without_x265.resolve(&caps, &HardwareAcceleration::auto());
    assert!(
        !resolution.available,
        "no libx265 on the machine, but it ran anyway"
    );
    assert!(
        resolution.reason.contains("libx265"),
        "{}",
        resolution.reason
    );
    assert!(
        resolution.reason.contains("No other encoder"),
        "{}",
        resolution.reason
    );
}

#[test]
fn a_copied_entry_and_a_copied_stored_preset_share_nothing_with_the_original() {
    let original = builtin_entry("general-super-hq-720-30-h264").expect("a catalog entry");
    let mut copy = clone_entry(original, "copy-1", "");
    assert_eq!(copy.id, "copy-1");
    assert!(copy.name.ends_with("(copy)"), "{}", copy.name);
    copy.spec.quality = Some(5);
    copy.spec.scale_to = Some((320, 240));
    assert_ne!(copy.spec, original.spec);
    assert_eq!(
        original.spec.quality,
        builtin_entry("general-super-hq-720-30-h264")
            .expect("still there")
            .spec
            .quality,
        "editing the copy changed the preset it was copied from"
    );

    // The same for a stored preset, all the way through the file.
    let dir = temp_dir("clone");
    let path = dir.join("presets.json");
    let store = vise_core::UserPresetStore::new(&path);
    let created = store
        .create(
            "Mine",
            PresetSpec {
                container: Some(ContainerFormat::Mp4),
                video_codec: Some(VideoCodec::H264),
                quality: Some(22),
                ..Default::default()
            },
        )
        .expect("create");
    let copy = clone_user_preset(&path, &created.id, None).expect("clone");
    assert_ne!(copy.id, created.id);
    assert_eq!(copy.name, "Mine (copy)");
    store
        .update(
            &copy.id,
            Some("Changed"),
            Some(PresetSpec {
                quality: Some(5),
                ..Default::default()
            }),
        )
        .expect("update the copy");
    let stored = vise_core::load_user_presets(&path).expect("load");
    let original_again = stored
        .iter()
        .find(|preset| preset.id == created.id)
        .expect("the original");
    assert_eq!(
        original_again.preset.quality,
        Some(22),
        "editing the copy changed the preset it was copied from"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_library_can_be_reordered_without_losing_anything() {
    let dir = temp_dir("order");
    let path = dir.join("presets.json");
    let store = vise_core::UserPresetStore::new(&path);
    let first = store.create("One", PresetSpec::default()).expect("one");
    let second = store.create("Two", PresetSpec::default()).expect("two");
    let third = store.create("Three", PresetSpec::default()).expect("three");
    assert_eq!(
        user_preset_order(&store.list().expect("list")),
        vec![first.id.clone(), second.id.clone(), third.id.clone()]
    );

    assert!(move_user_preset(&path, &third.id, -1).expect("move"));
    assert_eq!(
        user_preset_order(&store.list().expect("list")),
        vec![first.id.clone(), third.id.clone(), second.id.clone()]
    );
    // A move past either end changes nothing rather than wrapping round.
    assert!(!move_user_preset(&path, &first.id, -1).expect("past the top"));
    assert!(!move_user_preset(&path, &second.id, 1).expect("past the bottom"));
    assert!(!move_user_preset(&path, "missing", 1).expect("no such preset"));
    assert_eq!(
        user_preset_order(&store.list().expect("list")),
        vec![first.id.clone(), third.id.clone(), second.id.clone()]
    );
    assert!(place_user_preset(&path, &second.id, 0).expect("place"));
    assert_eq!(
        user_preset_order(&store.list().expect("list")),
        vec![second.id.clone(), first.id.clone(), third.id.clone()]
    );
    assert_eq!(store.list().expect("list").len(), 3, "nothing was lost");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn every_entry_is_already_clean() {
    // The shipped data is the file people review, so it has to be in the form the
    // clean operation produces: no entry should need cleaning to be compared
    // with another one.
    for entry in builtin_entries() {
        assert_eq!(entry.cleaned(), *entry, "'{}' is not clean", entry.id);
    }
}

#[test]
fn cleaning_an_entry_folds_the_software_family_and_blank_text() {
    let entry = builtin_entry("general-fast-1080-30-h264")
        .expect("a catalog entry")
        .clone();
    let untidy = PresetEntry {
        id: format!("  {}  ", entry.id),
        name: format!("  {}  ", entry.name),
        description: format!("  {}  ", entry.description),
        family: Some(HardwareFamily::Software),
        ..entry.clone()
    };
    let clean = untidy.cleaned();
    assert_eq!(clean.id, entry.id);
    assert_eq!(clean.name, entry.name);
    assert_eq!(clean.description, entry.description);
    assert_eq!(
        clean.family, None,
        "naming the software family says the same thing as leaving it out"
    );
    assert_eq!(clean.compatibility, entry.compatibility);
    assert_eq!(clean.cleaned(), clean);
    assert_eq!(
        clean, entry,
        "a preset that named the software family is the preset that did not"
    );

    // A blank compatibility note is not guidance, and dropping it makes a preset
    // that wrote one comparable with a preset that wrote none.
    let blank = PresetEntry {
        compatibility: Some("   ".to_string()),
        ..entry.clone()
    };
    assert_eq!(blank.cleaned().compatibility, None);
}

#[test]
fn the_identifiers_older_builds_stored_still_resolve() {
    // A settings file written by an earlier build names a preset this build has
    // to still know, or the default preset silently becomes nothing.
    for (older, current) in vise_core::LEGACY_PRESET_ALIASES {
        let preset: BuiltInPreset = builtin_preset(older)
            .unwrap_or_else(|| panic!("'{older}' is no longer a preset this build knows"));
        assert_eq!(preset.id, current, "'{older}' means something else now");
        assert!(!preset.name.is_empty());
        // The alias is an alias rather than a second row: the browser lists the
        // entry once.
        assert_eq!(
            builtin_entries()
                .iter()
                .filter(|entry| entry.id == current)
                .count(),
            1
        );
        assert!(
            !builtin_entries().iter().any(|entry| entry.id == older),
            "'{older}' is in the catalog as well as in the alias table"
        );
    }
    assert_eq!(vise_core::resolve_builtin_id("no-such-preset"), None);
    assert_eq!(
        vise_core::resolve_builtin_id("fast-1080p"),
        Some("general-fast-1080-30-h264")
    );
}

#[test]
fn a_catalog_entry_that_breaks_a_rule_is_reported_rather_than_shipped() {
    let good = builtin_entries()[0].clone();
    let mut broken = good.clone();
    broken.id = "Not An Id".to_string();
    assert!(broken
        .validate()
        .expect_err("an id with spaces")
        .contains("id"));

    let mut no_encoder = good.clone();
    no_encoder.encoder = None;
    assert!(no_encoder
        .validate()
        .expect_err("no encoder")
        .contains("encoder"));

    let mut mismatched = good.clone();
    mismatched.encoder = Some(EncoderId::X265);
    assert!(mismatched
        .validate()
        .expect_err("encoder and codec disagree")
        .contains("codec"));

    let mut odd_size = good.clone();
    odd_size.spec.scale_to = Some((1281, 720));
    assert!(odd_size
        .validate()
        .expect_err("odd width")
        .contains("odd size"));

    let mut no_rate_control = good.clone();
    no_rate_control.spec.encoding_mode = None;
    no_rate_control.spec.target_size = Some(1024);
    assert!(no_rate_control
        .validate()
        .expect_err("a target size with no rate control")
        .contains("target size"));

    let mut silent_device = good.clone();
    silent_device.category = PresetCategory::Devices;
    silent_device.compatibility = None;
    assert!(silent_device
        .validate()
        .expect_err("a device preset with no guidance")
        .contains("device"));

    assert!(
        good.validate().is_ok(),
        "the entry it was copied from is fine"
    );
}

#[test]
fn an_encoder_is_named_by_the_word_the_media_tools_use() {
    assert_eq!(EncoderId::NvencH265.identifier(), "hevc_nvenc");
    assert_eq!(EncoderId::parse("hevc_nvenc"), Some(EncoderId::NvencH265));
    assert_eq!(EncoderId::parse(" HEVC_NVENC "), Some(EncoderId::NvencH265));
    assert_eq!(EncoderId::parse("h265_vaapi"), None);
    assert_eq!(EncoderId::AmfAv1.hardware_api(), HardwareApi::Amf);
    assert_eq!(
        EncoderId::VtH265.hardware_family(),
        Some(HardwareFamily::VideoToolbox)
    );
    assert!(!EncoderId::X264.is_hardware());
    assert_eq!(EncoderId::ProRes.software_stand_in(), None);
    assert_eq!(EncoderId::QsvAv1.software_stand_in(), Some(EncoderId::Av1));
    // The name in a file is the same word everywhere else.
    let written = serde_json::to_string(&EncoderId::QsvH265).expect("write");
    assert_eq!(written, "\"hevc_qsv\"");
    assert!(
        serde_json::from_str::<EncoderId>("\"libx264\"").is_ok(),
        "every identifier is readable back"
    );
    assert!(
        serde_json::from_str::<EncoderId>("\"made_up\"").is_err(),
        "an encoder this build does not know is refused"
    );
}

#[test]
fn the_categories_are_named_the_way_a_person_would_name_them() {
    assert_eq!(PresetCategory::General.label(), "General");
    assert_eq!(
        PresetCategory::parse("  PROFESSIONAL "),
        Some(PresetCategory::Professional)
    );
    assert_eq!(PresetCategory::parse("nonsense"), None);
    assert_eq!(vise_core::CATEGORIES.len(), 6);
    for category in vise_core::CATEGORIES {
        assert!(
            !category.description().is_empty(),
            "{} has nothing to say about itself",
            category.label()
        );
    }
}

#[test]
fn the_catalog_asset_is_readable_and_well_formed_on_its_own() {
    let value: serde_json::Value =
        serde_json::from_str(vise_core::BUILTIN_CATALOG_JSON).expect("the asset is JSON");
    let presets = value["presets"].as_array().expect("a list of presets");
    assert!(presets.len() > 100, "{} presets", presets.len());
    for entry in presets {
        assert!(entry["id"].is_string(), "every entry has an id");
        assert!(
            entry["description"].is_string(),
            "every entry explains itself"
        );
        assert!(entry["encoder"].is_string(), "every entry names an encoder");
        assert!(entry["spec"].is_object(), "every entry has its settings");
    }
    assert!(value["template"].is_object());
    assert_eq!(value["version"]["major"], 1);
}

/// The catalog has to be checked as data as well as in memory, because that is
/// how a reviewer reads it.
#[test]
fn the_asset_has_no_preset_that_says_nothing() {
    for entry in builtin_entries() {
        let words = entry.description.split_whitespace().count();
        assert!(
            words >= 8,
            "'{}' says {} words: {}",
            entry.id,
            words,
            entry.description
        );
    }
}

/// A preset that is applied has to say something to the job, not merely compare
/// equal as data.
#[test]
fn applying_a_preset_from_every_category_reaches_the_job() {
    let job = vise_core::JobSpec::default();
    for entry in builtin_entries() {
        let preset = builtin_preset(&entry.id).expect("a built-in preset");
        let applied = preset.preset.apply_to(&job);
        assert_eq!(
            applied.container,
            entry.spec.container_of().unwrap_or(ContainerFormat::Mp4),
            "'{}' did not set the container",
            entry.id
        );
        assert_eq!(
            applied.video_codec,
            entry.spec.video_codec_of().unwrap_or(VideoCodec::H264),
            "'{}' did not set the video codec",
            entry.id
        );
        assert_eq!(
            applied.audio_codec,
            entry.spec.audio_codec_of().unwrap_or(AudioCodec::Aac),
            "'{}' did not set the audio codec",
            entry.id
        );
        if let Some((width, height)) = entry.spec.scale_to {
            assert_eq!(applied.scale_to, Some((width, height)), "{}", entry.id);
        }
    }
}

/// A preset with a hardware encoder must put the encoder where the pipeline
/// reads it, or the catalog would be a list of hardware-shaped intentions.
#[test]
fn a_hardware_preset_reaches_the_encoder_the_pipeline_reads() {
    let preset = builtin_preset("hardware-qsv-hevc-1080-30").expect("a QSV preset");
    let job = preset.preset.apply_to(&vise_core::JobSpec::default());
    assert_eq!(job.video_codec, VideoCodec::H265);
    assert_eq!(
        job.hardware_acceleration.encode.encoder.as_deref(),
        Some("hevc_qsv")
    );
    assert_eq!(job.hardware_acceleration.encode.api, HardwareApi::Qsv);
    assert!(job.hardware_acceleration.encode.enabled);
    // A software preset does not switch the machine's hardware on.
    let software = builtin_preset("general-super-hq-720-30-h264").expect("a software preset");
    let job = software.preset.apply_to(&vise_core::JobSpec::default());
    assert!(!job.hardware_acceleration.enabled);
    assert_eq!(
        software
            .preset
            .video_encoding
            .as_ref()
            .and_then(|encoding| encoding.preset.clone()),
        Some("slow".to_string()),
        "the quality family reaches the encoder"
    );
}

/// The catalog has to keep working when it is read through the older entry point
/// the rest of the workspace uses.
#[test]
fn the_older_entry_points_still_answer_from_the_same_data() {
    let all = builtin_presets();
    assert_eq!(all.len(), builtin_entries().len());
    let one = builtin_preset(builtin_entries()[0].id.as_str()).expect("by id");
    assert_eq!(one.name, builtin_entries()[0].name);
    assert_eq!(
        BuiltInPreset::by_id(&one.id).expect("by id").name,
        one.name,
        "the two ways of asking agree"
    );
    assert!(BuiltInPreset::all().len() > 100);
    assert!(builtin_preset("no-such-preset").is_none());
}

#[test]
fn a_directory_of_exported_presets_can_be_imported_again() {
    let dir = temp_dir("many");
    let path: &Path = &dir.join("preset.json");
    for entry in builtin_entries().iter().take(12) {
        vise_core::export_preset_entry(path, entry).expect("export");
        let read = vise_core::import_preset_entry(path).expect("import");
        assert_eq!(read.entry.id, entry.id);
        assert_eq!(read.entry.description, entry.description);
        if let Some(compatibility) = &entry.compatibility {
            assert_eq!(
                read.entry.compatibility.as_deref(),
                Some(compatibility.as_str())
            );
        }
    }
    let _ = fs::remove_dir_all(&dir);
}
