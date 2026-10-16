//! Loopback HTTP client for the worker API.
//!
//! The client speaks the same minimal dialect as the worker server: one request
//! per connection, `token` header on everything, and no keep-alive. Every call
//! returns a structured [`ClientError`] so callers can tell "the worker is not
//! running" apart from "the worker rejected this job".

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream};
use std::time::Duration;

use crate::contract::*;
use crate::http::HttpResponse;
use crate::route::TOKEN_HEADER;

/// Default per-request timeout.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// Why a worker call failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientError {
    /// The connection failed: the worker is not listening, or it died.
    Unreachable(String),
    /// The reply could not be parsed.
    Protocol(String),
    /// The worker answered with a non-success status.
    Rejected { status: u16, body: String },
    /// The reply body did not match the expected shape.
    Decode(String),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::Unreachable(m) => write!(f, "worker is unreachable: {m}"),
            ClientError::Protocol(m) => write!(f, "worker protocol error: {m}"),
            ClientError::Rejected { status, body } => {
                write!(f, "worker returned HTTP {status}: {body}")
            }
            ClientError::Decode(m) => write!(f, "worker reply could not be decoded: {m}"),
        }
    }
}

impl std::error::Error for ClientError {}

impl ClientError {
    /// Whether this error means the worker process is gone.
    ///
    /// The supervisor uses this to trigger a restart.
    pub fn is_worker_gone(&self) -> bool {
        matches!(self, ClientError::Unreachable(_))
    }
}

/// A raw reply, exposed for callers that need status codes (and for tests).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawReply {
    pub status: u16,
    pub body: String,
}

/// Connection details for one worker session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerClient {
    port: u16,
    token: String,
    timeout: Duration,
}

impl WorkerClient {
    /// Build a client for `127.0.0.1:port` using `token`.
    pub fn new(port: u16, token: impl Into<String>) -> Self {
        WorkerClient {
            port,
            token: token.into(),
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// Override the per-request timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    /// Loopback address this client talks to.
    pub fn address(&self) -> SocketAddr {
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, self.port))
    }

    /// Send a request with the session token.
    pub fn request(&self, method: &str, path: &str, body: &[u8]) -> Result<RawReply, ClientError> {
        self.request_with_token(method, path, Some(&self.token), body)
    }

    /// Send a request with an explicit token, or no token at all.
    pub fn request_with_token(
        &self,
        method: &str,
        path: &str,
        token: Option<&str>,
        body: &[u8],
    ) -> Result<RawReply, ClientError> {
        let response = self.send(method, path, token, body)?;
        Ok(RawReply {
            status: response.status,
            body: response.body_string(),
        })
    }

    /// Issue a request and fail when the worker answers with an error status.
    fn call<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        path: &str,
        body: &[u8],
    ) -> Result<T, ClientError> {
        let response = self.send(method, path, Some(&self.token), body)?;
        if response.status != 200 {
            return Err(ClientError::Rejected {
                status: response.status,
                body: response.body_string(),
            });
        }
        serde_json::from_slice(&response.body).map_err(|e| ClientError::Decode(e.to_string()))
    }

    fn send(
        &self,
        method: &str,
        path: &str,
        token: Option<&str>,
        body: &[u8],
    ) -> Result<HttpResponse, ClientError> {
        let address = self.address();
        let mut stream = TcpStream::connect_timeout(&address, self.timeout)
            .map_err(|e| ClientError::Unreachable(format!("{address}: {e}")))?;
        let _ = stream.set_read_timeout(Some(self.timeout));
        let _ = stream.set_write_timeout(Some(self.timeout));
        let _ = stream.set_nodelay(true);

        let mut head = format!(
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAccept: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
            self.port,
            body.len()
        );
        if let Some(value) = token {
            head.push_str(&format!("{TOKEN_HEADER}: {value}\r\n"));
        }
        head.push_str("\r\n");

        stream
            .write_all(head.as_bytes())
            .and_then(|_| stream.write_all(body))
            .and_then(|_| stream.flush())
            .map_err(|e| ClientError::Unreachable(format!("send failed: {e}")))?;

        let mut raw = Vec::new();
        stream
            .read_to_end(&mut raw)
            .map_err(|e| ClientError::Unreachable(format!("read failed: {e}")))?;

        // A worker that accepted the connection and then stopped, without
        // writing a reply, closes the socket instead of resetting it. The read
        // succeeds with nothing in it. That is the worker being gone, not a
        // malformed reply, and the supervisor only restarts on `Unreachable`,
        // so reporting it as a protocol error would leave a dead worker in
        // place after a shutdown or a crash.
        if raw.is_empty() {
            return Err(ClientError::Unreachable(format!(
                "{address}: closed the connection without replying"
            )));
        }

        HttpResponse::parse(&raw).map_err(|e| ClientError::Protocol(e.to_string()))
    }
}

fn encode_body<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, ClientError> {
    serde_json::to_vec(value).map_err(|e| ClientError::Protocol(e.to_string()))
}

/// Typed command wrappers over the worker API.
impl WorkerClient {
    /// `GET /` — loopback banner.
    pub fn banner(&self) -> Result<ServerBanner, ClientError> {
        self.call("GET", "/", b"")
    }

    /// `GET /IsTokenSet`.
    pub fn is_token_set(&self) -> Result<bool, ClientError> {
        self.call("GET", "/IsTokenSet", b"")
    }

    /// `POST /RegisterToken` — returns false when the worker is already
    /// initialised with a token, which is the normal handshake outcome.
    pub fn register_token(&self) -> Result<bool, ClientError> {
        self.call("POST", "/RegisterToken", b"")
    }

    /// `GET /GetWorkerInfo`.
    pub fn worker_info(&self) -> Result<WorkerInfo, ClientError> {
        self.call("GET", "/GetWorkerInfo", b"")
    }

    /// `POST /StartScan`.
    pub fn start_scan(&self, command: &ScanCommand) -> Result<CommandResult, ClientError> {
        self.call("POST", "/StartScan", &encode_body(command)?)
    }

    /// `GET /PollScanProgress`.
    pub fn poll_scan_progress(&self) -> Result<ProgressReport, ClientError> {
        self.call("GET", "/PollScanProgress", b"")
    }

    /// `GET /GetTitles`.
    pub fn get_titles(&self) -> Result<TitlesResult, ClientError> {
        self.call("GET", "/GetTitles", b"")
    }

    /// `GET /GetMainTitle`.
    pub fn get_main_title(&self) -> Result<u32, ClientError> {
        self.call("GET", "/GetMainTitle", b"")
    }

    /// `POST /StopScan`.
    pub fn stop_scan(&self) -> Result<CommandResult, ClientError> {
        self.call("POST", "/StopScan", b"")
    }

    /// `POST /StartEncode`.
    pub fn start_encode(&self, command: &EncodeCommand) -> Result<CommandResult, ClientError> {
        self.call("POST", "/StartEncode", &encode_body(command)?)
    }

    /// `POST /PauseEncode`.
    pub fn pause_encode(&self) -> Result<CommandResult, ClientError> {
        self.call("POST", "/PauseEncode", b"")
    }

    /// `POST /ResumeEncode`.
    pub fn resume_encode(&self) -> Result<CommandResult, ClientError> {
        self.call("POST", "/ResumeEncode", b"")
    }

    /// `POST /StopEncode`.
    pub fn stop_encode(&self) -> Result<CommandResult, ClientError> {
        self.call("POST", "/StopEncode", b"")
    }

    /// `GET /PollEncodeProgress`.
    pub fn poll_encode_progress(&self) -> Result<ProgressReport, ClientError> {
        self.call("GET", "/PollEncodeProgress", b"")
    }

    /// `POST /GetPreview`.
    pub fn get_preview(&self, command: &PreviewCommand) -> Result<PreviewResult, ClientError> {
        self.call("POST", "/GetPreview", &encode_body(command)?)
    }

    /// `GET /GetAllLogMessages`.
    pub fn get_all_log_messages(&self) -> Result<Vec<LogMessage>, ClientError> {
        self.call("GET", "/GetAllLogMessages", b"")
    }

    /// `GET /GetLogMessagesFromIndex`.
    pub fn get_log_messages_from_index(
        &self,
        index: usize,
    ) -> Result<LogMessagesFromIndex, ClientError> {
        self.call(
            "GET",
            &format!("/GetLogMessagesFromIndex?index={index}"),
            b"",
        )
    }

    /// `POST /ResetLogging`.
    pub fn reset_logging(&self) -> Result<CommandResult, ClientError> {
        self.call("POST", "/ResetLogging", b"")
    }

    /// `GET /Shutdown` — asks the worker to stop listening.
    pub fn shutdown(&self) -> Result<CommandResult, ClientError> {
        self.call("GET", "/Shutdown", b"")
    }

    /// Cheap liveness probe used by the handshake and by crash detection.
    pub fn ping(&self) -> Result<WorkerInfo, ClientError> {
        self.worker_info()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::HttpRequest;
    use crate::route::Route;
    use crate::PROTOCOL_VERSION;

    #[test]
    fn rejects_unreachable_workers_without_panicking() {
        // Port 1 on loopback is never a Vise worker.
        let client = WorkerClient::new(1, "token").with_timeout(Duration::from_millis(500));
        let err = client.is_token_set().expect_err("must fail");
        assert!(err.is_worker_gone(), "unexpected error: {err}");
    }

    #[test]
    fn builds_loopback_addresses_only() {
        let client = WorkerClient::new(45999, "t");
        assert_eq!(client.address().ip().to_string(), "127.0.0.1");
        assert_eq!(client.address().port(), 45999);
    }

    #[test]
    fn a_worker_that_closes_without_replying_counts_as_gone() {
        use std::net::TcpListener;

        // Stands in for a worker that accepted the request and then stopped
        // before writing anything, which is how a shutdown or a crash looks
        // when it happens mid-request.
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            // Drain the whole request first. Closing while the request is still
            // unread would reset the connection instead of hanging up cleanly,
            // and the client would report a reset rather than an empty reply.
            let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
            let mut buffer = [0_u8; 1024];
            while let Ok(read) = stream.read(&mut buffer) {
                if read == 0 {
                    break;
                }
            }
            // A clean FIN, so the client's read finishes with no bytes rather
            // than with an error.
            let _ = stream.shutdown(std::net::Shutdown::Write);
            std::thread::sleep(Duration::from_millis(100));
        });

        let client = WorkerClient::new(port, "token").with_timeout(Duration::from_secs(5));
        let err = client.is_token_set().expect_err("must fail");
        server.join().expect("server thread");
        assert!(
            err.is_worker_gone(),
            "an empty reply means the worker went away: {err}"
        );
    }

    #[test]
    fn serialises_command_bodies() {
        let cmd = ScanCommand {
            paths: vec!["clip.mkv".into()],
            ..ScanCommand::default()
        };
        let body = encode_body(&cmd).expect("encode");
        assert!(String::from_utf8_lossy(&body).contains("\"Paths\":[\"clip.mkv\"]"));
    }

    #[test]
    fn error_messages_are_distinguishable() {
        assert!(ClientError::Unreachable("refused".into())
            .to_string()
            .contains("unreachable"));
        assert!(ClientError::Rejected {
            status: 401,
            body: "denied".into()
        }
        .to_string()
        .contains("401"));
        assert!(!ClientError::Decode("x".into()).is_worker_gone());
    }

    #[test]
    fn route_paths_used_by_the_client_all_parse() {
        for path in [
            "/",
            "/IsTokenSet",
            "/RegisterToken",
            "/GetWorkerInfo",
            "/StartScan",
            "/PollScanProgress",
            "/GetTitles",
            "/GetMainTitle",
            "/StopScan",
            "/StartEncode",
            "/PauseEncode",
            "/ResumeEncode",
            "/StopEncode",
            "/PollEncodeProgress",
            "/GetPreview",
            "/GetAllLogMessages",
            "/GetLogMessagesFromIndex",
            "/ResetLogging",
            "/Shutdown",
        ] {
            assert!(!Route::parse(path).is_unknown(), "unknown route {path}");
        }
    }

    #[test]
    fn request_builder_round_trips_through_the_http_parser() {
        let mut req = HttpRequest {
            method: "GET".into(),
            target: "/GetTitles".into(),
            ..Default::default()
        };
        req.headers.insert(TOKEN_HEADER.into(), "abc".into());
        let raw = {
            let mut out =
                b"GET /GetTitles HTTP/1.1\r\ntoken: abc\r\nContent-Length: 0\r\n\r\n".to_vec();
            out.extend_from_slice(&req.body);
            out
        };
        let parsed = HttpRequest::parse(&raw).expect("parse");
        assert_eq!(parsed.header("token"), Some("abc"));
        assert_eq!(PROTOCOL_VERSION, 1);
    }
}
