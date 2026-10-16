//! Worker token generation and authentication checks.
//!
//! Every worker session gets a fresh token. The parent process generates it,
//! hands it to the child through the environment, and presents it on every
//! request in the `token` header. Requests without a valid token are refused
//! before any worker state is touched.

use std::fmt;

use crate::route::{access_denied_message, Route};

pub use crate::route::TOKEN_HEADER as HEADER_NAME;

/// Length of a generated token in hexadecimal characters.
pub const GENERATED_TOKEN_LEN: usize = 32;

/// Why a presented token was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenRejection {
    /// No `token` header was supplied.
    Missing,
    /// A token was supplied but does not match.
    Invalid,
}

impl TokenRejection {
    /// Phrase used in the 401 body.
    pub fn reason(&self) -> &'static str {
        match self {
            TokenRejection::Missing => "was not supplied",
            TokenRejection::Invalid => "was not valid",
        }
    }

    /// HTTP status for this rejection.
    pub fn status(&self) -> u16 {
        401
    }

    /// Full client-facing message for a route.
    pub fn message(&self, route: &Route) -> String {
        access_denied_message(route, self.reason())
    }
}

impl fmt::Display for TokenRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.reason())
    }
}

/// Holds the token for one worker session and validates presented tokens.
#[derive(Debug, Clone)]
pub struct TokenGuard {
    token: String,
}

impl TokenGuard {
    /// Wrap a caller-supplied token.
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
        }
    }

    /// Whether a non-empty token is registered.
    ///
    /// A worker started without a token cannot serve authenticated routes.
    pub fn is_set(&self) -> bool {
        !self.token.is_empty()
    }

    /// The registered token.
    pub fn token(&self) -> &str {
        &self.token
    }

    /// Validate a header value from a request.
    pub fn authorize(&self, presented: Option<&str>) -> Result<(), TokenRejection> {
        self.authorize_value(presented)
    }

    /// Validate an already-extracted header value.
    pub fn authorize_value(&self, presented: Option<&str>) -> Result<(), TokenRejection> {
        let presented = match presented {
            Some(value) if !value.is_empty() => value,
            _ => return Err(TokenRejection::Missing),
        };
        if self.is_set() && constant_time_eq(self.token.as_bytes(), presented.as_bytes()) {
            Ok(())
        } else {
            Err(TokenRejection::Invalid)
        }
    }
}

impl Default for TokenGuard {
    fn default() -> Self {
        Self::new(generate_token())
    }
}

/// Compare two byte strings without an early exit on the first difference.
///
/// The token protects a loopback-only endpoint against accidental or casual
/// local access, so a constant-time comparison is used as a matter of course.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Generate a fresh session token.
///
/// The value is 128 bits of SplitMix64 output seeded from several per-session
/// and per-call sources (wall-clock nanoseconds, process id, a heap address,
/// and a call counter). That is enough to keep unrelated local processes from
/// reaching the API by accident or by reading the worker's command line, but it
/// is *not* a cryptographically secure random source. Callers that need a
/// stronger guarantee should supply their own token instead of using this.
pub fn generate_token() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let pid = std::process::id() as u64;
    let boxed = Box::new(0u8);
    let heap = (&*boxed as *const u8) as usize as u64;
    let counter = next_counter();

    let high = splitmix64(nanos ^ (pid << 32) ^ heap.rotate_left(17) ^ counter);
    let low = splitmix64(high ^ nanos.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (heap >> 7));
    format!("{high:016x}{low:016x}")
}

fn next_counter() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    COUNTER.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed)
}

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::TOKEN_HEADER;

    #[test]
    fn accepts_the_registered_token() {
        let guard = TokenGuard::new("s3cret");
        assert!(guard.is_set());
        assert!(guard.authorize(Some("s3cret")).is_ok());
    }

    #[test]
    fn rejects_missing_and_invalid_tokens() {
        let guard = TokenGuard::new("s3cret");
        assert_eq!(guard.authorize(None), Err(TokenRejection::Missing));
        assert_eq!(guard.authorize(Some("")), Err(TokenRejection::Missing));
        assert_eq!(guard.authorize(Some("other")), Err(TokenRejection::Invalid));
    }

    #[test]
    fn rejects_everything_when_no_token_is_registered() {
        let guard = TokenGuard::new("");
        assert!(!guard.is_set());
        assert_eq!(
            guard.authorize(Some("anything")),
            Err(TokenRejection::Invalid)
        );
        assert_eq!(guard.authorize(None), Err(TokenRejection::Missing));
    }

    #[test]
    fn rejection_messages_describe_the_route_and_header() {
        let route = Route::parse("/GetTitles");
        let msg = TokenRejection::Invalid.message(&route);
        assert!(msg.contains("/GetTitles"));
        assert!(msg.contains(TOKEN_HEADER));
        assert_eq!(TokenRejection::Missing.status(), 401);
    }

    #[test]
    fn generated_tokens_are_long_and_unique() {
        let a = generate_token();
        let b = generate_token();
        assert_eq!(a.len(), GENERATED_TOKEN_LEN);
        assert_eq!(b.len(), GENERATED_TOKEN_LEN);
        assert_ne!(a, b, "each call must produce a distinct token");
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn constant_time_eq_matches_equality() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
        assert!(constant_time_eq(b"", b""));
    }
}
