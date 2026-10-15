use vise_core::{display_aspect_ratio, scale_to_fit, JobSpec, Session};

#[test]
fn job_spec_roundtrips_json() {
    let spec = JobSpec {
        input_path: "in.mp4".into(),
        output_path: "out.mkv".into(),
        container: vise_core::ContainerFormat::Mkv,
        title_index: Some(2),
        crop: Some((10, 10, 100, 50)),
        ..Default::default()
    };
    let json = serde_json::to_string(&spec).unwrap();
    let decoded: JobSpec = serde_json::from_str(&json).unwrap();
    assert_eq!(spec, decoded);
}

#[test]
fn session_lifecycle() {
    let mut s = Session::new();
    let id = s.add_job(JobSpec::default());
    assert_eq!(s.len(), 1);
    assert!(s.job(id).is_some());
    s.remove_job(id).unwrap();
    assert!(s.is_empty());
}

#[test]
fn geometry_scale() {
    assert_eq!(scale_to_fit(1920, 1080, 1280, 720), (1280, 720));
    assert_eq!(display_aspect_ratio(720, 480, 32.0 / 27.0), (16, 9));
}
