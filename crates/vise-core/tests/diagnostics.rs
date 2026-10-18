//! Group 20 through the public API of `vise_core`.
//!
//! The unit tests live beside the code they pin. This file exists for the other
//! question: that the sweep and the capability report are reachable from outside
//! the crate under the names the rest of the workspace uses, and that the two
//! behave the same way from out here as they do from in there.

use vise_core::{
    sweep::plan, CapabilityReport, ContainerFormat, HardwareCapabilities, HdrCapabilities, JobSpec,
    SweepExecutor, SweepOutcome, SweepPlan, SweepRange, SweepRequest, SweepState, CONFIRMED_SPAN,
    MAX_SWEEP_RUNS, SWEEP_FOLDER,
};

fn source(name: &str) -> JobSpec {
    JobSpec {
        input_path: format!("C:\\media\\{name}.mkv"),
        output_path: format!("D:\\out\\{name}.mp4"),
        container: ContainerFormat::Mp4,
        ..JobSpec::default()
    }
}

fn request(range: SweepRange) -> SweepRequest {
    SweepRequest {
        base: source("raw"),
        range,
        output_folder: "D:\\out".to_string(),
        confirmed: true,
    }
}

#[test]
fn a_sweep_is_plannable_and_runnable_from_outside_the_crate() {
    let planned: SweepPlan = plan(&request(SweepRange::new(18, 24, 2)));
    assert!(planned.is_runnable(), "{}", planned.refusal);
    assert_eq!(planned.len(), 4);

    let mut sweep = SweepExecutor::start(&planned);
    let mut qualities = Vec::new();
    while let Some(run) = sweep.next_run().cloned() {
        qualities.push(run.quality);
        assert!(
            run.job.output_path.contains(SWEEP_FOLDER),
            "{}",
            run.job.output_path
        );
        assert!(sweep.finish_run(SweepOutcome::succeeded(run.quality, 500, 3.0)));
    }
    assert_eq!(qualities, vec![18, 20, 22, 24]);
    assert_eq!(sweep.progress().state, SweepState::Finished);
    let results = sweep.results();
    assert_eq!(results.rows.len(), 4);
    assert_eq!(results.succeeded, 4);
    assert_eq!(results.rows[3].relative_size, 1.0);
}

#[test]
fn the_bounds_are_reachable_as_constants_and_are_enforced() {
    let mut widest_without_confirming: u32 = 0;
    for span in 1..=52u32 {
        let mut request = request(SweepRange::new(0, span - 1, 1));
        request.confirmed = false;
        if plan(&request).is_runnable() {
            widest_without_confirming = span;
        }
    }
    assert!(
        widest_without_confirming <= CONFIRMED_SPAN,
        "a sweep of {widest_without_confirming} values ran without confirmation"
    );
    assert!(plan(&request(SweepRange::new(0, 51, 1))).len() <= MAX_SWEEP_RUNS);
}

#[test]
fn a_capability_report_can_be_built_from_a_detection_the_caller_owns() {
    let detection = HardwareCapabilities {
        ffmpeg_path: Some(std::path::PathBuf::from("/opt/ffmpeg")),
        hwaccels: vec!["vaapi".into(), "d3d11va".into()],
        decoders: vec!["h264".into(), "hevc".into(), "av1".into()],
        encoders: vec!["libx264".into(), "libx265".into(), "h264_vaapi".into()],
    };
    let hdr = HdrCapabilities {
        dolby_vision_rpu: true,
        hdr10_plus_sei: false,
    };
    let report = CapabilityReport::from_detected(&detection, &hdr);
    assert!(report.detected);
    assert_eq!(report.hardware_encoders, vec!["h264_vaapi"]);
    assert_eq!(report.accelerators, vec!["d3d11va", "vaapi"]);
    assert!(report.hdr_dolby_vision_rpu);
    assert!(!report.hdr_hdr10_plus_sei);
    assert!(report.notes.iter().any(|note| note.contains("HDR10+")));
    // Both forms parse back into something a program can read.
    let back: CapabilityReport = serde_json::from_str(&report.to_json()).expect("json");
    assert_eq!(back, report);
    assert!(report.to_text().contains("hevc"));
}

#[test]
fn a_machine_with_no_media_tools_is_reported_rather_than_left_out() {
    let report = CapabilityReport::from_detected(
        &HardwareCapabilities::default(),
        &HdrCapabilities::default(),
    );
    assert!(!report.detected);
    assert!(report.to_json().contains("\"detected\": false"));
    assert!(report.summary().contains("not found"));
}
