//! Release and update metadata models for Vise.
//!
//! These are plain data structures describing a versioned release and the
//! optional update-delivery mechanism. They intentionally contain no network or
//! filesystem logic; frontends/packaging tooling decide how to act on them.

use serde::{Deserialize, Serialize};

/// The current release version of Vise, matching the Cargo workspace version.
pub const VISE_VERSION: &str = "0.1.0";

/// A versioned release of Vise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseInfo {
    /// Semantic version string (e.g. "0.1.0").
    pub version: String,
    /// Human-readable release notes body.
    pub notes: String,
    /// Public URL for the release artifacts (platform-specific).
    pub artifacts_url: String,
}

/// Optional update-delivery mechanism model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateManifest {
    /// Version advertised by the update server.
    pub latest_version: String,
    /// Whether an update is considered available for the installed version.
    pub update_available: bool,
    /// Artifact URL for the update package.
    pub package_url: String,
    /// Optional SHA-256 checksum of the update package.
    pub sha256: Option<String>,
}

impl UpdateManifest {
    /// Decide whether `installed` should be updated based on the advertised `latest`.
    pub fn from_versions(installed: &str, latest: &str, package_url: String) -> Self {
        Self {
            latest_version: latest.to_string(),
            update_available: version_less_than(installed, latest),
            package_url,
            sha256: None,
        }
    }
}

fn version_less_than(a: &str, b: &str) -> bool {
    // Simple numeric major.minor.patch comparison; ignores pre-release tags.
    let parse = |v: &str| -> (u64, u64, u64) {
        let mut parts = v.trim().split('.');
        let major = parts.next().unwrap_or("0").parse().unwrap_or(0);
        let minor = parts.next().unwrap_or("0").parse().unwrap_or(0);
        let patch = parts.next().unwrap_or("0").parse().unwrap_or(0);
        (major, minor, patch)
    };
    parse(a) < parse(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_available_when_latest_newer() {
        let m = UpdateManifest::from_versions("0.1.0", "0.1.1", "u".into());
        assert!(m.update_available);
    }

    #[test]
    fn no_update_when_same() {
        let m = UpdateManifest::from_versions("0.1.0", "0.1.0", "u".into());
        assert!(!m.update_available);
    }

    #[test]
    fn vise_version_constant_matches() {
        assert_eq!(VISE_VERSION, "0.1.0");
    }
}
