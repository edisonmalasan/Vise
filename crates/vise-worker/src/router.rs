//! Request dispatch for the worker API.
//!
//! Dispatch order matters: an unknown route is reported as unknown, a known
//! route with the wrong verb is reported as a method error, and only then is
//! the token checked and the command run. Token checking happens for every
//! route except the banner, so an unauthenticated caller cannot probe worker
//! state.

use std::sync::Arc;

use vise_ipc::contract::{
    CommandResult, EncodeCommand, PreviewCommand, PreviewResult, ProgressReport, ScanCommand,
    WorkDone,
};
use vise_ipc::http::{HttpRequest, HttpResponse};
use vise_ipc::route::{Route, UNKNOWN_ROUTE_MESSAGE};

use crate::runtime::WorkerRuntime;

/// Split `?name=value&...` into decoded pairs.
pub fn query_pairs(target: &str) -> Vec<(String, String)> {
    let query = match target.split_once('?') {
        Some((_, query)) => query,
        None => return Vec::new(),
    };
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| match pair.split_once('=') {
            Some((key, value)) => (percent_decode(key), percent_decode(value)),
            None => (percent_decode(pair), String::new()),
        })
        .collect()
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(byte) => {
                        out.push(byte);
                        index += 3;
                    }
                    Err(_) => {
                        out.push(bytes[index]);
                        index += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Handle one request.
pub fn dispatch(runtime: &Arc<WorkerRuntime>, request: &HttpRequest) -> HttpResponse {
    let route = Route::parse(request.path());

    if route.is_unknown() {
        runtime
            .logs()
            .info(format!("rejected unknown endpoint '{}'", request.path()));
        return HttpResponse::text(404, UNKNOWN_ROUTE_MESSAGE);
    }

    if !route.allowed_methods().contains(&request.method.as_str()) {
        let allowed = route.allowed_methods().join(", ");
        return HttpResponse::text(
            405,
            format!(
                "Vise Worker: '{}' accepts {} requests, not {}.",
                route.name(),
                allowed,
                request.method
            ),
        );
    }

    if route.requires_token() {
        if let Err(rejection) = runtime
            .token()
            .authorize(request.header(vise_ipc::route::TOKEN_HEADER))
        {
            runtime.logs().warn(format!(
                "rejected request to '{}': {}",
                route.name(),
                rejection.reason()
            ));
            return HttpResponse::text(rejection.status(), rejection.message(&route));
        }
    }

    match route {
        Route::Root => HttpResponse::json(200, &runtime.banner()),
        Route::IsTokenSet => HttpResponse::json(200, &runtime.token().is_set()),
        Route::RegisterToken => {
            // The worker is always started with its token in the environment,
            // so the token is already set and registration is a no-op that
            // reports "already initialised".
            runtime
                .logs()
                .info("token registration request; the session token was supplied at startup");
            HttpResponse::json(200, &false)
        }
        Route::GetWorkerInfo => HttpResponse::json(200, &runtime.info()),
        Route::StartScan => handle_start_scan(runtime, request),
        Route::StopScan => {
            runtime.scan().stop();
            HttpResponse::json(200, &CommandResult::ok())
        }
        Route::PollScanProgress => HttpResponse::json(200, &scan_report(runtime)),
        Route::GetTitles => HttpResponse::json(200, &runtime.scan().titles()),
        Route::GetMainTitle => HttpResponse::json(200, &runtime.scan().main_title()),
        Route::StartEncode => handle_start_encode(runtime, request),
        Route::PauseEncode => match runtime.encode().pause() {
            Ok(()) => HttpResponse::json(200, &CommandResult::ok()),
            Err(message) => HttpResponse::json(200, &CommandResult::failed(message)),
        },
        Route::ResumeEncode => match runtime.encode().resume() {
            Ok(()) => HttpResponse::json(200, &CommandResult::ok()),
            Err(message) => HttpResponse::json(200, &CommandResult::failed(message)),
        },
        Route::StopEncode => match runtime.encode().stop() {
            Ok(()) => HttpResponse::json(200, &CommandResult::ok()),
            Err(message) => HttpResponse::json(200, &CommandResult::failed(message)),
        },
        Route::PollEncodeProgress => HttpResponse::json(200, &encode_report(runtime)),
        Route::GetPreview => handle_preview(runtime, request),
        Route::GetAllLogMessages => HttpResponse::json(200, &runtime.logs().all()),
        Route::GetLogMessagesFromIndex => {
            let index = query_pairs(&request.target)
                .into_iter()
                .find(|(key, _)| key == "index")
                .and_then(|(_, value)| value.parse::<usize>().ok())
                .unwrap_or(0);
            HttpResponse::json(200, &runtime.logs().from_index(index))
        }
        Route::ResetLogging => {
            runtime.logs().reset();
            HttpResponse::json(200, &CommandResult::ok())
        }
        Route::Shutdown => {
            runtime.logs().info("shutdown requested by the client");
            runtime.request_shutdown();
            HttpResponse::json(200, &CommandResult::ok())
        }
        Route::Unknown(_) => HttpResponse::text(404, UNKNOWN_ROUTE_MESSAGE),
    }
}

fn decode_body<T: serde::de::DeserializeOwned>(
    runtime: &Arc<WorkerRuntime>,
    route: &Route,
    request: &HttpRequest,
) -> Result<T, HttpResponse> {
    let body = request.body_string();
    if body.trim().is_empty() {
        // An empty body means "defaults", which is what every command wants.
        return serde_json::from_str("{}")
            .map_err(|e| bad_request(runtime, route, format!("empty body rejected: {e}")));
    }
    serde_json::from_str(&body)
        .map_err(|e| bad_request(runtime, route, format!("body could not be decoded: {e}")))
}

fn bad_request(runtime: &Arc<WorkerRuntime>, route: &Route, message: String) -> HttpResponse {
    runtime.logs().warn(format!(
        "bad request body for '{}': {message}",
        route.name()
    ));
    HttpResponse::text(
        400,
        format!(
            "Vise Worker: '{}' received an unusable request body. {message}",
            route.name()
        ),
    )
}

fn handle_start_scan(runtime: &Arc<WorkerRuntime>, request: &HttpRequest) -> HttpResponse {
    let command: ScanCommand = match decode_body(runtime, &Route::StartScan, request) {
        Ok(command) => command,
        Err(response) => return response,
    };
    if command.paths.iter().all(|path| path.trim().is_empty()) {
        return HttpResponse::json(
            200,
            &CommandResult::failed("scan request contains no source paths"),
        );
    }
    runtime.apply_init(&command.initialise_command);
    match runtime.scan().start(&command) {
        Ok(()) => HttpResponse::json(200, &CommandResult::ok()),
        Err(message) => HttpResponse::json(200, &CommandResult::failed(message)),
    }
}

fn handle_start_encode(runtime: &Arc<WorkerRuntime>, request: &HttpRequest) -> HttpResponse {
    let command: EncodeCommand = match decode_body(runtime, &Route::StartEncode, request) {
        Ok(command) => command,
        Err(response) => return response,
    };
    runtime.apply_init(&command.initialise_command);

    // Validation against the cached scan result gives an immediate, precise
    // answer for most bad jobs. The paths are looked up here rather than inside
    // the rules, which stay a pure function of the job and these facts.
    let mut facts = vise_ipc::validation::probe_machine_facts(&command.encode_job);
    if let Some(cached) = runtime.scan().cached_result(&command.encode_job.input_path) {
        facts = facts.and_scan(cached);
    }
    let report = vise_ipc::validation::validate_job(&command.encode_job, &facts);
    if !report.is_valid() {
        runtime.logs().error(format!("encode rejected: {report}"));
        return HttpResponse::json(
            200,
            &CommandResult::failed(format!("invalid encode job: {report}")),
        );
    }
    match runtime.encode().start(&command.encode_job) {
        Ok(()) => HttpResponse::json(200, &CommandResult::ok()),
        Err(message) => HttpResponse::json(200, &CommandResult::failed(message)),
    }
}

fn handle_preview(runtime: &Arc<WorkerRuntime>, request: &HttpRequest) -> HttpResponse {
    let command: PreviewCommand = match decode_body(runtime, &Route::GetPreview, request) {
        Ok(command) => command,
        Err(response) => return response,
    };

    let source = if command.path.trim().is_empty() {
        match runtime.scan().last_source() {
            Some(path) => path,
            None => {
                return HttpResponse::json(
                    200,
                    &PreviewResult {
                        was_successful: false,
                        error: "no preview source: scan a source first or pass a path".into(),
                        ..Default::default()
                    },
                )
            }
        }
    } else {
        std::path::PathBuf::from(command.path.trim())
    };

    if !source.is_file() {
        return HttpResponse::json(
            200,
            &PreviewResult {
                was_successful: false,
                error: format!("preview source does not exist: {}", source.display()),
                ..Default::default()
            },
        );
    }

    let scanner = vise_core::FfprobeScanner::default();
    let requested = command.output_file.trim();
    if requested.is_empty() {
        let temporary = std::env::temp_dir().join(format!(
            "vise-preview-{}-{}.jpg",
            std::process::id(),
            command.title_index
        ));
        if let Err(e) = scanner.extract_preview(&source, &temporary, command.time_seconds) {
            return HttpResponse::json(
                200,
                &PreviewResult {
                    was_successful: false,
                    error: format!("preview frame could not be extracted: {e}"),
                    ..Default::default()
                },
            );
        }
        let bytes = std::fs::read(&temporary).unwrap_or_default();
        let _ = std::fs::remove_file(&temporary);
        return HttpResponse::json(
            200,
            &PreviewResult {
                was_successful: true,
                error: String::new(),
                output_file: String::new(),
                image_bytes: bytes.len() as u64,
                image_base64: vise_ipc::base64::encode(&bytes),
            },
        );
    }

    let destination = std::path::PathBuf::from(requested);
    match scanner.extract_preview(&source, &destination, command.time_seconds) {
        Ok(()) => HttpResponse::json(
            200,
            &PreviewResult {
                was_successful: true,
                error: String::new(),
                output_file: destination.to_string_lossy().to_string(),
                image_bytes: std::fs::metadata(&destination)
                    .map(|m| m.len())
                    .unwrap_or(0),
                image_base64: String::new(),
            },
        ),
        Err(e) => HttpResponse::json(
            200,
            &PreviewResult {
                was_successful: false,
                error: format!("preview frame could not be extracted: {e}"),
                output_file: destination.to_string_lossy().to_string(),
                ..Default::default()
            },
        ),
    }
}

fn scan_report(runtime: &Arc<WorkerRuntime>) -> ProgressReport {
    let state = runtime.scan().state();
    ProgressReport {
        state,
        scan: Some(runtime.scan().progress()),
        working: None,
        work_done: if state == vise_ipc::contract::WorkState::WorkDone {
            Some(runtime.scan().done())
        } else {
            None
        },
    }
}

fn encode_report(runtime: &Arc<WorkerRuntime>) -> ProgressReport {
    let state = runtime.encode().state();
    ProgressReport {
        state,
        scan: None,
        working: if state == vise_ipc::contract::WorkState::Working {
            Some(runtime.encode().progress())
        } else {
            None
        },
        work_done: if state == vise_ipc::contract::WorkState::WorkDone {
            Some(runtime.encode().done())
        } else {
            None
        },
    }
}

/// Reply used when a scan or encode has never run.
pub fn idle_work_done() -> WorkDone {
    WorkDone {
        error: vise_ipc::contract::error_codes::INVALID,
        was_successful: false,
        message: "nothing has been started".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::WorkerSettings;
    use vise_ipc::http::HttpRequest;
    use vise_ipc::route::TOKEN_HEADER;

    fn runtime() -> Arc<WorkerRuntime> {
        Arc::new(WorkerRuntime::new(WorkerSettings::for_tests(0, "secret")))
    }

    fn get(runtime: &Arc<WorkerRuntime>, path: &str) -> HttpResponse {
        dispatch(
            runtime,
            &HttpRequest::parse(format!("GET {path} HTTP/1.1\r\n\r\n").as_bytes())
                .expect("request"),
        )
    }

    fn get_with_token(runtime: &Arc<WorkerRuntime>, path: &str, token: &str) -> HttpResponse {
        let raw = format!("GET {path} HTTP/1.1\r\n{TOKEN_HEADER}: {token}\r\n\r\n");
        dispatch(
            runtime,
            &HttpRequest::parse(raw.as_bytes()).expect("request"),
        )
    }

    fn post(runtime: &Arc<WorkerRuntime>, path: &str, body: &str) -> HttpResponse {
        let raw = format!(
            "POST {path} HTTP/1.1\r\n{TOKEN_HEADER}: secret\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        dispatch(
            runtime,
            &HttpRequest::parse(raw.as_bytes()).expect("request"),
        )
    }

    #[test]
    fn rejects_unknown_routes() {
        let runtime = runtime();
        let response = get_with_token(&runtime, "/NoSuchRoute", "secret");
        assert_eq!(response.status, 404);
        assert!(response.body_string().contains("unknown"));
    }

    #[test]
    fn rejects_a_missing_token() {
        let runtime = runtime();
        let response = get(&runtime, "/GetTitles");
        assert_eq!(response.status, 401);
        let body = response.body_string();
        assert!(body.contains("/GetTitles"));
        assert!(body.contains(TOKEN_HEADER));
    }

    #[test]
    fn rejects_an_invalid_token() {
        let runtime = runtime();
        let response = get_with_token(&runtime, "/GetTitles", "wrong");
        assert_eq!(response.status, 401);
        assert!(response.body_string().contains("was not valid"));
    }

    #[test]
    fn rejects_the_wrong_method() {
        let runtime = runtime();
        let response = get_with_token(&runtime, "/StartScan", "secret");
        assert_eq!(response.status, 405);
        assert!(response.body_string().contains("POST"));
    }

    #[test]
    fn serves_the_banner_without_a_token() {
        let runtime = runtime();
        let response = get(&runtime, "/");
        assert_eq!(response.status, 200);
        assert!(response.body_string().contains("loopback"));
    }

    #[test]
    fn answers_token_routes() {
        let runtime = runtime();
        let response = get_with_token(&runtime, "/IsTokenSet", "secret");
        assert_eq!(response.status, 200);
        assert_eq!(response.body_string(), "true");

        let response = post(&runtime, "/RegisterToken", "");
        assert_eq!(response.status, 200);
        assert_eq!(response.body_string(), "false");

        let response = get_with_token(&runtime, "/GetWorkerInfo", "secret");
        assert_eq!(response.status, 200);
        let info: vise_ipc::contract::WorkerInfo =
            serde_json::from_str(&response.body_string()).expect("info");
        assert_eq!(info.pid, std::process::id());
    }

    #[test]
    fn answers_idle_progress_routes() {
        let runtime = runtime();
        let response = get_with_token(&runtime, "/PollScanProgress", "secret");
        let report: ProgressReport = serde_json::from_str(&response.body_string()).expect("report");
        assert_eq!(report.state, vise_ipc::contract::WorkState::Unknown);

        let response = get_with_token(&runtime, "/PollEncodeProgress", "secret");
        let report: ProgressReport = serde_json::from_str(&response.body_string()).expect("report");
        assert_eq!(report.state, vise_ipc::contract::WorkState::Unknown);
    }

    #[test]
    fn answers_title_routes_before_a_scan() {
        let runtime = runtime();
        let response = get_with_token(&runtime, "/GetTitles", "secret");
        assert_eq!(
            response.body_string(),
            "{\"MainFeature\":0,\"TitleList\":[]}"
        );

        let response = get_with_token(&runtime, "/GetMainTitle", "secret");
        assert_eq!(response.body_string(), "0");
    }

    #[test]
    fn rejects_a_scan_with_no_paths() {
        let runtime = runtime();
        let response = post(&runtime, "/StartScan", "{\"Paths\":[]}");
        let result: CommandResult = serde_json::from_str(&response.body_string()).expect("result");
        assert!(!result.was_successful);
        assert!(result.error.contains("no source paths"));
    }

    #[test]
    fn rejects_a_malformed_scan_body() {
        let runtime = runtime();
        let response = post(&runtime, "/StartScan", "{\"MinDuration\":\"99:59:59\"}");
        assert_eq!(response.status, 400);
        assert!(response.body_string().contains("unusable request body"));
    }

    #[test]
    fn rejects_an_invalid_encode_job() {
        let runtime = runtime();
        let response = post(&runtime, "/StartEncode", "{\"EncodeJob\":{}}");
        let result: CommandResult = serde_json::from_str(&response.body_string()).expect("result");
        assert!(!result.was_successful);
        assert!(result.error.contains("invalid encode job"));
    }

    #[test]
    fn preview_without_a_source_is_reported() {
        let runtime = runtime();
        let response = post(&runtime, "/GetPreview", "{}");
        let result: PreviewResult = serde_json::from_str(&response.body_string()).expect("result");
        assert!(!result.was_successful);
        assert!(result.error.contains("scan a source first"));
    }

    #[test]
    fn preview_of_a_missing_file_is_reported() {
        let runtime = runtime();
        let response = post(&runtime, "/GetPreview", "{\"Path\":\"no-such-file.mkv\"}");
        let result: PreviewResult = serde_json::from_str(&response.body_string()).expect("result");
        assert!(!result.was_successful);
        assert!(result.error.contains("does not exist"));
    }

    #[test]
    fn logging_routes_read_and_reset_the_buffer() {
        let runtime = runtime();
        let response = get_with_token(&runtime, "/GetAllLogMessages", "secret");
        let messages: Vec<vise_ipc::contract::LogMessage> =
            serde_json::from_str(&response.body_string()).expect("messages");
        assert!(!messages.is_empty());

        let response = get_with_token(&runtime, "/GetLogMessagesFromIndex?index=1", "secret");
        let page: vise_ipc::contract::LogMessagesFromIndex =
            serde_json::from_str(&response.body_string()).expect("page");
        assert!(page.next_index >= 1);

        let response = post(&runtime, "/ResetLogging", "");
        assert_eq!(response.status, 200);
        let response = get_with_token(&runtime, "/GetAllLogMessages", "secret");
        let messages: Vec<vise_ipc::contract::LogMessage> =
            serde_json::from_str(&response.body_string()).expect("messages");
        assert!(messages.is_empty());
    }

    #[test]
    fn control_routes_report_when_nothing_is_running() {
        let runtime = runtime();
        for route in ["/PauseEncode", "/ResumeEncode", "/StopEncode"] {
            let response = post(&runtime, route, "");
            let result: CommandResult =
                serde_json::from_str(&response.body_string()).expect("result");
            assert!(!result.was_successful, "{route}");
            assert!(result.error.contains("no encode is running"), "{route}");
        }
    }

    #[test]
    fn shutdown_route_sets_the_flag() {
        let runtime = runtime();
        assert!(!runtime.shutdown_requested());
        let response = get_with_token(&runtime, "/Shutdown", "secret");
        assert_eq!(response.status, 200);
        assert!(runtime.shutdown_requested());
    }

    #[test]
    fn parses_query_pairs() {
        assert!(query_pairs("/GetTitles").is_empty());
        let pairs = query_pairs("/GetLogMessagesFromIndex?index=42&flag");
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], ("index".to_string(), "42".to_string()));
        assert_eq!(pairs[1], ("flag".to_string(), String::new()));
    }

    #[test]
    fn percent_decodes_query_values() {
        let pairs = query_pairs("/x?name=a%20b&other=c+d");
        assert_eq!(pairs[0].1, "a b");
        assert_eq!(pairs[1].1, "c d");
    }
}
