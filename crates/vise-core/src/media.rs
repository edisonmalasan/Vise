use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackKind {
    Video,
    Audio,
    Subtitle,
    Data,
}

/// A contiguous block of media data (decoded frame or muxed content).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaBuffer {
    pub data: Vec<u8>,
    pub pts: i64,
    pub dts: i64,
    pub keyframe: bool,
    pub track: Option<usize>,
}

impl MediaBuffer {
    pub fn new(data: Vec<u8>, pts: i64, dts: i64) -> Self {
        Self {
            data,
            pts,
            dts,
            keyframe: false,
            track: None,
        }
    }

    pub fn with_keyframe(mut self, keyframe: bool) -> Self {
        self.keyframe = keyframe;
        self
    }

    pub fn with_track(mut self, track: usize) -> Self {
        self.track = Some(track);
        self
    }
}

/// An encoded unit ready for muxing or transmission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Packet {
    pub data: Vec<u8>,
    pub pts: i64,
    pub dts: i64,
    pub keyframe: bool,
    pub track: usize,
    pub track_kind: TrackKind,
}

impl Packet {
    pub fn new(data: Vec<u8>, pts: i64, dts: i64, track: usize, track_kind: TrackKind) -> Self {
        Self {
            data,
            pts,
            dts,
            keyframe: false,
            track,
            track_kind,
        }
    }
}
