//! Convenience helpers layered over [`WorkerClient`].
//!
//! The raw client exposes one function per route. These helpers add the small
//! amount of policy the desktop application needs: bounded polling until a
//! terminal state, and a friendly classification of "the worker disappeared".

use std::time::{Duration, Instant};

use crate::client::{ClientError, WorkerClient};
use crate::contract::{CommandResult, ProgressReport, WorkState};

/// How long [`poll_until_stable`] waits before giving up.
pub const DEFAULT_POLL_TIMEOUT: Duration = Duration::from_secs(300);

/// Delay between polls.
pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Terminal-state helper: is this report finished?
pub fn is_terminal(report: &ProgressReport) -> bool {
    report.state == WorkState::WorkDone
}

/// True when the report describes a successful finish.
pub fn is_success(report: &ProgressReport) -> bool {
    is_terminal(report)
        && report
            .work_done
            .as_ref()
            .map(|d| d.was_successful)
            .unwrap_or(false)
}

/// Poll `read` until the report reaches a terminal state or the timeout expires.
///
/// Returns the last report seen, so a caller can tell a timeout apart from a
/// finished operation.
pub fn poll_until_stable<F>(
    read: &mut F,
    timeout: Duration,
    interval: Duration,
) -> Result<ProgressReport, ClientError>
where
    F: FnMut() -> Result<ProgressReport, ClientError>,
{
    let deadline = Instant::now() + timeout;
    loop {
        let report = read()?;
        if is_terminal(&report) {
            return Ok(report);
        }
        if Instant::now() >= deadline {
            return Ok(report);
        }
        std::thread::sleep(interval);
    }
}

/// Poll `PollEncodeProgress` until the encode finishes.
pub fn wait_for_encode(
    client: &WorkerClient,
    timeout: Duration,
) -> Result<ProgressReport, ClientError> {
    let mut read = || client.poll_encode_progress();
    poll_until_stable(&mut read, timeout, DEFAULT_POLL_INTERVAL)
}

/// Poll `PollScanProgress` until the scan finishes.
pub fn wait_for_scan(
    client: &WorkerClient,
    timeout: Duration,
) -> Result<ProgressReport, ClientError> {
    let mut read = || client.poll_scan_progress();
    poll_until_stable(&mut read, timeout, DEFAULT_POLL_INTERVAL)
}

/// Poll `PollEncodeProgress` until `predicate` holds or the timeout expires.
pub fn poll_encode_until<F>(
    client: &WorkerClient,
    timeout: Duration,
    mut predicate: F,
) -> Result<ProgressReport, ClientError>
where
    F: FnMut(&ProgressReport) -> bool,
{
    let deadline = Instant::now() + timeout;
    loop {
        let report = client.poll_encode_progress()?;
        if predicate(&report) || Instant::now() >= deadline {
            return Ok(report);
        }
        std::thread::sleep(DEFAULT_POLL_INTERVAL);
    }
}

/// Wait for the worker to answer `GetWorkerInfo`, returning `false` on timeout.
///
/// Used by the handshake and by crash detection, both of which need to know
/// whether the worker is alive without raising on the way.
pub fn wait_until_ready(client: &WorkerClient, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        match client.ping() {
            Ok(_) => return true,
            Err(_) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => return false,
        }
    }
}

/// Describe why a command failed in one line.
pub fn describe(result: &CommandResult) -> String {
    if result.was_successful {
        "ok".to_string()
    } else if result.error.is_empty() {
        "failed without a reported reason".to_string()
    } else {
        result.error.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{error_codes, WorkDone};

    #[test]
    fn recognises_terminal_and_successful_reports() {
        let mut counter = 0;
        let mut read = || {
            counter += 1;
            if counter < 3 {
                Ok(ProgressReport {
                    state: WorkState::Working,
                    ..Default::default()
                })
            } else {
                Ok(ProgressReport::done(WorkDone {
                    error: error_codes::NONE,
                    was_successful: true,
                    message: String::new(),
                }))
            }
        };
        let report = poll_until_stable(&mut read, Duration::from_secs(2), Duration::from_millis(1))
            .expect("poll");
        assert!(is_terminal(&report));
        assert!(is_success(&report));
    }

    #[test]
    fn reports_timeout_without_failing() {
        let mut read = || {
            Ok(ProgressReport {
                state: WorkState::Working,
                ..Default::default()
            })
        };
        let report = poll_until_stable(
            &mut read,
            Duration::from_millis(30),
            Duration::from_millis(5),
        )
        .expect("poll");
        assert!(!is_terminal(&report));
    }

    #[test]
    fn propagates_worker_death() {
        let mut read = || Err(ClientError::Unreachable("gone".into()));
        let err = poll_until_stable(
            &mut read,
            Duration::from_millis(10),
            Duration::from_millis(1),
        )
        .unwrap_err();
        assert!(err.is_worker_gone());
    }

    #[test]
    fn describes_command_results() {
        assert_eq!(describe(&CommandResult::ok()), "ok");
        assert_eq!(describe(&CommandResult::failed("no input")), "no input");
        assert!(describe(&CommandResult {
            was_successful: false,
            error: String::new()
        })
        .contains("without a reported reason"));
    }
}
