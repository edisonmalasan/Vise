//! The loopback HTTP listener.
//!
//! The listener binds `127.0.0.1` only. If the requested port is taken it
//! walks forward a bounded number of ports and then falls back to an ephemeral
//! port, reporting the port it actually bound through the handshake file so the
//! parent never has to guess.

use std::io::{BufReader, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use vise_ipc::http::{HttpRequest, HttpResponse};
use vise_ipc::supervisor::{write_handshake, Handshake, PORT_SEARCH_SPAN};
use vise_ipc::{LOOPBACK_HOST, PROTOCOL_VERSION, WORKER_NAME};

use crate::router;
use crate::runtime::WorkerRuntime;

/// Why the listener could not start.
#[derive(Debug)]
pub enum ServerError {
    /// No port in the search window was free.
    BindFailed(String),
    /// The handshake file could not be written.
    HandshakeFailed(String),
}

impl std::fmt::Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerError::BindFailed(m) => {
                write!(f, "no loopback port could be bound for the worker: {m}")
            }
            ServerError::HandshakeFailed(m) => {
                write!(f, "worker handshake could not be published: {m}")
            }
        }
    }
}

impl std::error::Error for ServerError {}

/// A bound listener plus the runtime it serves.
pub struct WorkerServer {
    listener: TcpListener,
    runtime: Arc<WorkerRuntime>,
}

impl std::fmt::Debug for WorkerServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerServer")
            .field("local_addr", &self.local_addr())
            .field("shutdown", &self.runtime.shutdown_requested())
            .finish()
    }
}

impl WorkerServer {
    /// Bind the loopback listener and prepare the runtime.
    pub fn bind(mut runtime: WorkerRuntime) -> Result<WorkerServer, ServerError> {
        let listener =
            bind_listener(runtime.settings.requested_port).map_err(ServerError::BindFailed)?;
        runtime.settings.port = listener
            .local_addr()
            .map(|addr| addr.port())
            .unwrap_or(runtime.settings.requested_port);
        let runtime = Arc::new(runtime);
        Ok(WorkerServer { listener, runtime })
    }

    /// Address the listener is bound to; always loopback.
    pub fn local_addr(&self) -> std::net::SocketAddr {
        self.listener
            .local_addr()
            .unwrap_or_else(|_| SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0).into())
    }

    /// Shared runtime.
    pub fn runtime(&self) -> Arc<WorkerRuntime> {
        self.runtime.clone()
    }

    /// Publish the handshake so the parent can complete its side.
    pub fn publish_handshake(&self) -> Result<(), ServerError> {
        let path = match &self.runtime.settings.handshake_file {
            Some(path) => path.clone(),
            None => return Ok(()),
        };
        write_handshake(
            &path,
            &Handshake {
                name: WORKER_NAME.to_string(),
                protocol_version: PROTOCOL_VERSION,
                pid: std::process::id(),
                port: self.local_addr().port(),
                token: self.runtime.settings.token.clone(),
            },
        )
        .map_err(|e| ServerError::HandshakeFailed(e.to_string()))
    }

    /// Serve requests until shutdown is requested.
    ///
    /// Each connection is handled on its own thread so a slow client cannot
    /// stall the API.
    pub fn serve(self) {
        let shutdown = self.runtime.shutdown_flag();
        for stream in self.listener.incoming() {
            if shutdown.load(Ordering::SeqCst) {
                break;
            }
            let stream = match stream {
                Ok(stream) => stream,
                Err(_) => continue,
            };
            let runtime = self.runtime.clone();
            let shutdown = shutdown.clone();
            let _ = std::thread::Builder::new()
                .name("vise-worker-conn".into())
                .spawn(move || handle_connection(stream, runtime, shutdown));
        }
    }

    /// Serve on a background thread and return a handle that stops the loop.
    pub fn spawn(self) -> ServerHandle {
        let shutdown = self.runtime.shutdown_flag();
        let runtime = self.runtime.clone();
        let address = self.local_addr();
        let port = address.port();
        let _ = self.publish_handshake();
        let join = std::thread::Builder::new()
            .name("vise-worker-accept".into())
            .spawn(move || self.serve())
            .expect("spawn accept loop");
        ServerHandle {
            port,
            shutdown,
            runtime,
            join: Some(join),
        }
    }
}

/// Handle to a server running on its own thread.
pub struct ServerHandle {
    pub port: u16,
    shutdown: Arc<AtomicBool>,
    runtime: Arc<WorkerRuntime>,
    join: Option<JoinHandle<()>>,
}

impl ServerHandle {
    /// Shared runtime.
    pub fn runtime(&self) -> Arc<WorkerRuntime> {
        self.runtime.clone()
    }

    /// Address the worker is listening on.
    pub fn address(&self) -> std::net::SocketAddr {
        SocketAddrV4::new(Ipv4Addr::LOCALHOST, self.port).into()
    }

    /// Stop the accept loop and wait for it to finish.
    ///
    /// The loop is parked in `accept`, so a loopback connection is used to wake
    /// it up once the shutdown flag is set.
    pub fn stop(mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect((Ipv4Addr::LOCALHOST, self.port));
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect((Ipv4Addr::LOCALHOST, self.port));
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn bind_listener(requested: u16) -> Result<TcpListener, String> {
    let candidates: Vec<u16> = if requested == 0 {
        vec![0]
    } else {
        let end = requested.saturating_add(PORT_SEARCH_SPAN);
        (requested..=end).collect()
    };

    let mut last = String::new();
    for port in candidates {
        match TcpListener::bind((Ipv4Addr::LOCALHOST, port)) {
            Ok(listener) => return Ok(listener),
            Err(e) => last = e.to_string(),
        }
    }
    // Every requested port was busy; fall back to an ephemeral one so the
    // worker can still start, and let the handshake report the real port.
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(|e| {
        format!(
            "{last}; the ephemeral fallback also failed: {e}. Process isolation can be disabled in the advanced preferences"
        )
    })
}

fn handle_connection(stream: TcpStream, runtime: Arc<WorkerRuntime>, shutdown: Arc<AtomicBool>) {
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(30)));
    let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(30)));
    let peer_ok = stream
        .peer_addr()
        .map(|addr| addr.ip().is_loopback())
        .unwrap_or(false);

    let response = if !peer_ok {
        runtime
            .logs()
            .warn("refused a connection that did not originate on the loopback interface");
        HttpResponse::text(
            403,
            "Vise Worker: this API is reachable from the loopback interface only.",
        )
    } else {
        let mut reader = BufReader::new(match stream.try_clone() {
            Ok(clone) => clone,
            Err(_) => return,
        });
        match HttpRequest::read_from(&mut reader) {
            Ok(request) => router::dispatch(&runtime, &request),
            Err(e) => {
                runtime
                    .logs()
                    .warn(format!("unparsable request rejected: {e}"));
                HttpResponse::text(400, format!("Vise Worker: {e}"))
            }
        }
    };

    if let Ok(mut writer) = stream.try_clone() {
        let _ = writer.write_all(&response.to_bytes());
        let _ = writer.flush();
    }

    // Give the reply a chance to reach the client before the socket closes.
    if shutdown.load(Ordering::SeqCst) {
        // No action needed; the accept loop observes the flag.
    }
}

/// Address the worker binds, for documentation and tests.
pub const BIND_ADDRESS: &str = LOOPBACK_HOST;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::WorkerSettings;

    #[test]
    fn binds_loopback_only_and_reports_its_port() {
        let server = WorkerServer::bind(WorkerRuntime::new(WorkerSettings::for_tests(0, "t")))
            .expect("bind");
        let address = server.local_addr();
        assert!(
            address.ip().is_loopback(),
            "address {address} is not loopback"
        );
        assert_ne!(address.port(), 0);
    }

    #[test]
    fn falls_back_when_the_requested_port_is_busy() {
        let first = WorkerServer::bind(WorkerRuntime::new(WorkerSettings::for_tests(0, "t")))
            .expect("first bind");
        let taken = first.local_addr().port();

        let second = WorkerServer::bind(WorkerRuntime::new(WorkerSettings::for_tests(taken, "t")))
            .expect("second bind");
        assert_ne!(
            second.local_addr().port(),
            taken,
            "a busy port must not be reused"
        );
    }

    #[test]
    fn handshake_is_published_when_configured() {
        let dir = std::env::temp_dir().join("vise-worker-handshake");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("handshake.json");

        let mut settings = WorkerSettings::for_tests(0, "handshake-token");
        settings.handshake_file = Some(path.clone());
        let server = WorkerServer::bind(WorkerRuntime::new(settings)).expect("bind");
        server.publish_handshake().expect("publish");

        let handshake = vise_ipc::supervisor::read_handshake(&path).expect("handshake");
        assert_eq!(handshake.token, "handshake-token");
        assert_eq!(handshake.port, server.local_addr().port());
        assert_eq!(handshake.pid, std::process::id());
        assert_eq!(handshake.protocol_version, PROTOCOL_VERSION);
    }

    #[test]
    fn publish_handshake_is_a_no_op_without_a_path() {
        let server = WorkerServer::bind(WorkerRuntime::new(WorkerSettings::for_tests(0, "t")))
            .expect("bind");
        server.publish_handshake().expect("no-op");
    }
}
