//! The Vise worker process entry point.
//!
//! The desktop application starts this binary with a port, a session token, and
//! its own process id in the environment; the worker binds the loopback
//! listener, publishes the handshake the parent waits for, watches the parent,
//! and serves until it is asked to shut down or its parent disappears.

use std::process::ExitCode;

use vise_ipc::supervisor::{ENV_PARENT_PID, ENV_PORT, ENV_TOKEN, PORT_SEARCH_SPAN};
use vise_worker::{run, WorkerSettings};

/// Banner printed whenever the worker is started outside the desktop app.
const STANDALONE_NOTICE: &str = "*** Vise worker: this process is started by the Vise desktop application and is not meant to run standalone. ***";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let settings = match resolve_settings(&arguments) {
        Ok(settings) => settings,
        Err(message) => {
            eprintln!("{STANDALONE_NOTICE}");
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };

    match run(settings) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("vise worker stopped with an error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Merge the environment contract with optional command-line overrides.
///
/// The environment is the normal path: the supervisor sets it. Overrides exist
/// so the worker can be launched by hand for diagnostics without an ambient
/// desktop process.
fn resolve_settings(arguments: &[String]) -> Result<WorkerSettings, String> {
    let overrides = parse_arguments(arguments)?;
    let mut settings = match WorkerSettings::from_env() {
        Ok(settings) => settings,
        Err(e) => {
            // A manually launched worker has no environment at all, so fall back
            // to whatever the command line supplied before giving up.
            if overrides.port.is_none() || overrides.token.is_none() {
                return Err(e.to_string());
            }
            let port = overrides.port.unwrap_or(0);
            let token = overrides.token.clone().unwrap_or_default();
            WorkerSettings::from_arguments(port, token).map_err(|e| e.to_string())?
        }
    };

    if let Some(port) = overrides.port {
        settings.requested_port = port;
        settings.port = port;
    }
    if let Some(token) = overrides.token {
        settings.token = token;
    }
    if let Some(parent_pid) = overrides.parent_pid {
        settings.parent_pid = Some(parent_pid);
        settings.monitor_parent = true;
    }
    if let Some(handshake) = overrides.handshake_file {
        settings.handshake_file = Some(handshake);
    }
    Ok(settings)
}

struct Overrides {
    port: Option<u16>,
    token: Option<String>,
    parent_pid: Option<u32>,
    handshake_file: Option<std::path::PathBuf>,
}

fn parse_arguments(arguments: &[String]) -> Result<Overrides, String> {
    let mut overrides = Overrides {
        port: None,
        token: None,
        parent_pid: None,
        handshake_file: None,
    };
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index].as_str();
        let (flag, inline) = match argument.split_once('=') {
            Some((flag, value)) => (flag, Some(value.to_string())),
            None => (argument, None),
        };
        let mut take_value = |name: &str| -> Result<String, String> {
            match inline.clone() {
                Some(value) => Ok(value),
                None => {
                    index += 1;
                    arguments
                        .get(index)
                        .cloned()
                        .ok_or_else(|| format!("{name} needs a value"))
                }
            }
        };
        match flag {
            "--port" => {
                let value = take_value("--port")?;
                let port = value
                    .parse::<u16>()
                    .map_err(|_| format!("--port value '{value}' is not a port number"))?;
                overrides.port = Some(port);
            }
            "--token" => overrides.token = Some(take_value("--token")?),
            "--parent-pid" => {
                let value = take_value("--parent-pid")?;
                overrides.parent_pid =
                    Some(value.parse::<u32>().map_err(|_| {
                        format!("--parent-pid value '{value}' is not a process id")
                    })?);
            }
            "--handshake-file" => {
                overrides.handshake_file =
                    Some(std::path::PathBuf::from(take_value("--handshake-file")?));
            }
            "--help" | "-h" => {
                println!("{STANDALONE_NOTICE}");
                println!("usage: vise-worker [--port N] [--token T] [--parent-pid N] [--handshake-file PATH]");
                println!("environment: {ENV_PORT}, {ENV_TOKEN}, {ENV_PARENT_PID}");
                println!("the listener binds 127.0.0.1 only and walks forward up to {PORT_SEARCH_SPAN} ports when the requested one is busy");
                std::process::exit(0);
            }
            other => return Err(format!("unrecognised argument '{other}'")),
        }
        index += 1;
    }
    Ok(overrides)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_arguments_in_both_forms() {
        let arguments: Vec<String> = [
            "--port=51999",
            "--token",
            "abc",
            "--parent-pid",
            "42",
            "--handshake-file=/tmp/h.json",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let overrides = parse_arguments(&arguments).expect("parse");
        assert_eq!(overrides.port, Some(51999));
        assert_eq!(overrides.token.as_deref(), Some("abc"));
        assert_eq!(overrides.parent_pid, Some(42));
        assert_eq!(
            overrides.handshake_file,
            Some(std::path::PathBuf::from("/tmp/h.json"))
        );
    }

    #[test]
    fn rejects_unusable_arguments() {
        assert!(parse_arguments(&["--port".to_string(), "not-a-port".to_string()]).is_err());
        assert!(parse_arguments(&["--token".to_string()]).is_err());
        assert!(parse_arguments(&["--nonsense".to_string()]).is_err());
    }

    #[test]
    fn command_line_overrides_the_environment() {
        let arguments: Vec<String> = ["--port=1234", "--token=xyz"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        // The environment is normally absent here; both halves come from the
        // command line, which is the supported standalone path.
        let settings = resolve_settings(&arguments).expect("resolve");
        assert_eq!(settings.requested_port, 1234);
        assert_eq!(settings.token, "xyz");
        assert!(!settings.monitor_parent);
    }

    #[test]
    fn refuses_to_start_without_a_port_and_token() {
        let settings = resolve_settings(&[]);
        if std::env::var(ENV_PORT).is_ok() && std::env::var(ENV_TOKEN).is_ok() {
            // A supervisor environment is present; nothing to assert.
            assert!(settings.is_ok());
        } else {
            let message = settings.unwrap_err();
            assert!(message.contains(ENV_PORT) || message.contains(ENV_TOKEN));
        }
    }
}
