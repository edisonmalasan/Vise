//! The build step for the desktop application.
//!
//! Tauri refuses to build when a resource it has been told about is missing,
//! and the resources here are the media tools and the worker: they are fetched
//! and staged by a separate step, which a plain build or a check never runs.
//! Rather than make every build fetch them, the directories are created empty
//! when they are absent. A build that packages the application fills them in
//! first, and a build that only compiles has nothing to carry.

use std::path::Path;

fn main() {
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let vendor = Path::new(&crate_dir).join("vendor");

    // The staged media tools.
    let tools = vendor.join("ffmpeg").join("staged");
    std::fs::create_dir_all(&tools).unwrap_or_else(|e| {
        panic!("could not create {}: {e}", tools.display());
    });

    // The worker that sits beside the application.
    let sidecar = vendor.join("sidecar");
    std::fs::create_dir_all(&sidecar).unwrap_or_else(|e| {
        panic!("could not create {}: {e}", sidecar.display());
    });

    tauri_build::build()
}
