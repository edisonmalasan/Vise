//! Minimal HTTP/1.1 message layer for the loopback worker API.
//!
//! The worker API is reachable only from `127.0.0.1` and authenticated by a
//! per-session token, so the transport deliberately stays small: one request
//! per connection, no keep-alive, and no chunked transfer encoding. Both the
//! server and the client in this crate speak exactly this dialect.

use std::collections::BTreeMap;
use std::fmt;
use std::io::Read;

/// Upper bound on an accepted request body (16 MiB).
///
/// Job specifications with embedded previews are the largest realistic payload;
/// anything beyond this is a bug or an attempt to exhaust worker memory.
pub const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;

/// Reason phrases for the status codes the worker emits.
pub fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Status",
    }
}

/// Why an inbound request could not be understood.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpParseError {
    /// The request head was empty or truncated.
    MalformedHead(String),
    /// The request line was not `METHOD TARGET VERSION`.
    MalformedRequestLine,
    /// A header line was not `name: value`.
    MalformedHeader(String),
    /// The declared `Content-Length` was missing, negative, or too large.
    InvalidContentLength(String),
    /// The declared body length did not arrive in full.
    TruncatedBody,
}

impl fmt::Display for HttpParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HttpParseError::MalformedHead(m) => write!(f, "malformed request head: {m}"),
            HttpParseError::MalformedRequestLine => write!(f, "malformed request line"),
            HttpParseError::MalformedHeader(h) => write!(f, "malformed header line: {h}"),
            HttpParseError::InvalidContentLength(v) => write!(f, "invalid content length: {v}"),
            HttpParseError::TruncatedBody => write!(f, "request body ended early"),
        }
    }
}

impl std::error::Error for HttpParseError {}

/// A parsed inbound request.
///
/// Header names are stored lowercased so lookups are case-insensitive without
/// allocating per comparison.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HttpRequest {
    pub method: String,
    /// Request target including any query string, exactly as received.
    pub target: String,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    /// Parse a complete request (head plus body) from raw bytes.
    pub fn parse(input: &[u8]) -> Result<Self, HttpParseError> {
        let head_end = find_head_end(input)
            .ok_or_else(|| HttpParseError::MalformedHead("no header terminator".into()))?;
        let head = std::str::from_utf8(&input[..head_end])
            .map_err(|e| HttpParseError::MalformedHead(e.to_string()))?;
        let rest = &input[head_end..];

        let mut lines = head.split("\r\n");
        let request_line = lines.next().ok_or(HttpParseError::MalformedRequestLine)?;
        let mut parts = request_line.split(' ').filter(|p| !p.is_empty());
        let method = parts
            .next()
            .ok_or(HttpParseError::MalformedRequestLine)?
            .to_ascii_uppercase();
        let target = parts
            .next()
            .ok_or(HttpParseError::MalformedRequestLine)?
            .to_string();
        if method.is_empty() || target.is_empty() {
            return Err(HttpParseError::MalformedRequestLine);
        }

        let mut headers = BTreeMap::new();
        for line in lines {
            if line.is_empty() {
                continue;
            }
            let (name, value) = line
                .split_once(':')
                .ok_or_else(|| HttpParseError::MalformedHeader(line.to_string()))?;
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }

        let expected = match headers.get("content-length") {
            None => 0usize,
            Some(raw) => {
                let parsed = raw
                    .parse::<usize>()
                    .map_err(|_| HttpParseError::InvalidContentLength(raw.clone()))?;
                if parsed > MAX_BODY_BYTES {
                    return Err(HttpParseError::InvalidContentLength(raw.clone()));
                }
                parsed
            }
        };
        if rest.len() < expected {
            return Err(HttpParseError::TruncatedBody);
        }

        Ok(HttpRequest {
            method,
            target,
            headers,
            body: rest[..expected].to_vec(),
        })
    }

    /// Parse a complete request from a stream.
    ///
    /// Only the head and the declared body are consumed, so the peer does not
    /// have to close its write side before the request is served. Reading stops
    /// as soon as a complete request is available.
    pub fn read_from<R: Read>(reader: &mut R) -> Result<Self, HttpParseError> {
        let mut buffer = Vec::with_capacity(1024);
        let mut chunk = [0u8; 4096];
        let ceiling = MAX_BODY_BYTES + 16 * 1024;
        loop {
            if let Ok(request) = Self::parse(&buffer) {
                return Ok(request);
            }
            let read = reader
                .read(&mut chunk)
                .map_err(|e| HttpParseError::MalformedHead(e.to_string()))?;
            if read == 0 {
                // The peer stopped sending; report whatever state we reached.
                return Self::parse(&buffer);
            }
            buffer.extend_from_slice(&chunk[..read]);
            if buffer.len() > ceiling {
                return Err(HttpParseError::InvalidContentLength(format!(
                    "more than {MAX_BODY_BYTES} bytes"
                )));
            }
        }
    }

    /// Path portion of the request target, with any query string removed.
    pub fn path(&self) -> &str {
        match self.target.split_once('?') {
            Some((path, _)) => path,
            None => &self.target,
        }
    }

    /// Case-insensitive header lookup.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(|v| v.as_str())
    }

    /// Body decoded as UTF-8, or an empty string when the body is not text.
    pub fn body_string(&self) -> String {
        String::from_utf8_lossy(&self.body).to_string()
    }
}

fn find_head_end(input: &[u8]) -> Option<usize> {
    input
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|i| i + 4)
}

/// A response ready to be written to the socket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub content_type: String,
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// Serialize `value` as a JSON response body.
    pub fn json<T: serde::Serialize>(status: u16, value: &T) -> Self {
        let body = serde_json::to_vec(value).unwrap_or_else(|e| {
            format!("{{\"WasSuccessful\":false,\"Error\":\"response serialization failed: {e}\"}}")
                .into_bytes()
        });
        HttpResponse {
            status,
            content_type: "application/json".into(),
            body,
        }
    }

    /// Plain-text response body.
    pub fn text(status: u16, body: impl Into<String>) -> Self {
        HttpResponse {
            status,
            content_type: "text/plain; charset=utf-8".into(),
            body: body.into().into_bytes(),
        }
    }

    /// Body decoded as UTF-8 (lossy).
    pub fn body_string(&self) -> String {
        String::from_utf8_lossy(&self.body).to_string()
    }

    /// Wire representation, including status line and headers.
    pub fn to_bytes(&self) -> Vec<u8> {
        let head = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            self.status,
            reason_phrase(self.status),
            self.content_type,
            self.body.len()
        );
        let mut out = head.into_bytes();
        out.extend_from_slice(&self.body);
        out
    }

    /// Parse a response from raw bytes; used by the client and by tests.
    pub fn parse(input: &[u8]) -> Result<Self, HttpParseError> {
        let head_end = find_head_end(input)
            .ok_or_else(|| HttpParseError::MalformedHead("no header terminator".into()))?;
        let head = std::str::from_utf8(&input[..head_end])
            .map_err(|e| HttpParseError::MalformedHead(e.to_string()))?;
        let body = input[head_end..].to_vec();

        let status_line = head
            .lines()
            .next()
            .ok_or(HttpParseError::MalformedHead("empty response".into()))?;
        let mut parts = status_line.split(' ').filter(|p| !p.is_empty());
        let _version = parts.next();
        let status = parts
            .next()
            .ok_or(HttpParseError::MalformedHead("missing status code".into()))?
            .parse::<u16>()
            .map_err(|_| HttpParseError::MalformedHead("non-numeric status code".into()))?;

        let mut content_type = "application/octet-stream".to_string();
        for line in head.lines().skip(1) {
            if let Some((name, value)) = line.split_once(':') {
                if name.trim().eq_ignore_ascii_case("content-type") {
                    content_type = value.trim().to_string();
                }
            }
        }

        Ok(HttpResponse {
            status,
            content_type,
            body,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_post_with_body_and_lowercases_headers() {
        let raw = b"POST /StartScan HTTP/1.1\r\nToken: abc\r\nContent-Length: 5\r\n\r\nhello";
        let req = HttpRequest::parse(raw).expect("parse");
        assert_eq!(req.method, "POST");
        assert_eq!(req.path(), "/StartScan");
        assert_eq!(req.header("token"), Some("abc"));
        assert_eq!(req.body_string(), "hello");
    }

    #[test]
    fn strips_query_string_from_path() {
        let raw = b"GET /GetTitles?x=1 HTTP/1.1\r\n\r\n";
        let req = HttpRequest::parse(raw).expect("parse");
        assert_eq!(req.path(), "/GetTitles");
    }

    #[test]
    fn rejects_truncated_body() {
        let raw = b"POST /StartEncode HTTP/1.1\r\nContent-Length: 10\r\n\r\nshort";
        assert_eq!(HttpRequest::parse(raw), Err(HttpParseError::TruncatedBody));
    }

    #[test]
    fn rejects_oversized_content_length() {
        let raw = format!(
            "POST /x HTTP/1.1\r\nContent-Length: {}\r\n\r\n",
            MAX_BODY_BYTES + 1
        );
        assert!(matches!(
            HttpRequest::parse(raw.as_bytes()),
            Err(HttpParseError::InvalidContentLength(_))
        ));
    }

    #[test]
    fn rejects_missing_header_terminator() {
        assert!(matches!(
            HttpRequest::parse(b"GET / HTTP/1.1"),
            Err(HttpParseError::MalformedHead(_))
        ));
    }

    #[test]
    fn round_trips_response_bytes() {
        let resp = HttpResponse::json(200, &serde_json::json!({"ok": true}));
        let parsed = HttpResponse::parse(&resp.to_bytes()).expect("parse");
        assert_eq!(parsed.status, 200);
        assert_eq!(parsed.content_type, "application/json");
        assert_eq!(parsed.body_string(), "{\"ok\":true}");
    }
}
