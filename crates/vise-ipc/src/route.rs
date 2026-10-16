//! Worker route table.
//!
//! Route names are the PascalCase command names shared by the worker server
//! and any client, so both sides dispatch on exactly one identifier.

use std::fmt;

/// Name of the HTTP request header carrying the worker token.
pub const TOKEN_HEADER: &str = "token";

/// Every command the worker exposes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// Loopback banner; answers before initialisation.
    Root,
    /// Reports whether a token has been registered.
    IsTokenSet,
    /// Registers the caller's token during the handshake.
    RegisterToken,
    /// Runtime facts about the running worker.
    GetWorkerInfo,

    StartScan,
    StopScan,
    PollScanProgress,
    GetTitles,
    GetMainTitle,

    StartEncode,
    PauseEncode,
    ResumeEncode,
    StopEncode,
    PollEncodeProgress,

    GetPreview,

    GetAllLogMessages,
    GetLogMessagesFromIndex,
    ResetLogging,

    /// Graceful shutdown; the listener stops after the reply is sent.
    Shutdown,

    /// Anything else.
    Unknown(String),
}

impl Route {
    /// Every route the worker implements, in documentation order.
    pub const ALL: &'static [Route] = &[
        Route::Root,
        Route::IsTokenSet,
        Route::RegisterToken,
        Route::GetWorkerInfo,
        Route::StartScan,
        Route::StopScan,
        Route::PollScanProgress,
        Route::GetTitles,
        Route::GetMainTitle,
        Route::StartEncode,
        Route::PauseEncode,
        Route::ResumeEncode,
        Route::StopEncode,
        Route::PollEncodeProgress,
        Route::GetPreview,
        Route::GetAllLogMessages,
        Route::GetLogMessagesFromIndex,
        Route::ResetLogging,
        Route::Shutdown,
    ];

    /// Parse a request path into a route.
    pub fn parse(path: &str) -> Route {
        let trimmed = path.trim_start_matches('/');
        match trimmed {
            "" | "/" => Route::Root,
            "IsTokenSet" => Route::IsTokenSet,
            "RegisterToken" => Route::RegisterToken,
            "GetWorkerInfo" => Route::GetWorkerInfo,
            "StartScan" => Route::StartScan,
            "StopScan" => Route::StopScan,
            "PollScanProgress" => Route::PollScanProgress,
            "GetTitles" => Route::GetTitles,
            "GetMainTitle" => Route::GetMainTitle,
            "StartEncode" => Route::StartEncode,
            "PauseEncode" => Route::PauseEncode,
            "ResumeEncode" => Route::ResumeEncode,
            "StopEncode" => Route::StopEncode,
            "PollEncodeProgress" => Route::PollEncodeProgress,
            "GetPreview" => Route::GetPreview,
            "GetAllLogMessages" => Route::GetAllLogMessages,
            "GetLogMessagesFromIndex" => Route::GetLogMessagesFromIndex,
            "ResetLogging" => Route::ResetLogging,
            "Shutdown" => Route::Shutdown,
            other => Route::Unknown(other.to_string()),
        }
    }

    /// Path segment for this route, without a leading slash.
    pub fn name(&self) -> &str {
        match self {
            Route::Root => "",
            Route::IsTokenSet => "IsTokenSet",
            Route::RegisterToken => "RegisterToken",
            Route::GetWorkerInfo => "GetWorkerInfo",
            Route::StartScan => "StartScan",
            Route::StopScan => "StopScan",
            Route::PollScanProgress => "PollScanProgress",
            Route::GetTitles => "GetTitles",
            Route::GetMainTitle => "GetMainTitle",
            Route::StartEncode => "StartEncode",
            Route::PauseEncode => "PauseEncode",
            Route::ResumeEncode => "ResumeEncode",
            Route::StopEncode => "StopEncode",
            Route::PollEncodeProgress => "PollEncodeProgress",
            Route::GetPreview => "GetPreview",
            Route::GetAllLogMessages => "GetAllLogMessages",
            Route::GetLogMessagesFromIndex => "GetLogMessagesFromIndex",
            Route::ResetLogging => "ResetLogging",
            Route::Shutdown => "Shutdown",
            Route::Unknown(other) => other.as_str(),
        }
    }

    /// HTTP methods accepted for this route.
    ///
    /// Queries and polling are `GET`; anything that changes worker state or
    /// carries a command body is `POST`.
    pub fn allowed_methods(&self) -> &'static [&'static str] {
        match self {
            Route::StartScan
            | Route::StartEncode
            | Route::PauseEncode
            | Route::ResumeEncode
            | Route::StopEncode
            | Route::StopScan
            | Route::GetPreview
            | Route::RegisterToken
            | Route::ResetLogging => &["POST"],
            Route::Unknown(_) => &["GET", "POST"],
            _ => &["GET"],
        }
    }

    /// Whether this route requires an authenticated token.
    ///
    /// Only the loopback banner is public, and even that reports a 401 when the
    /// token is wrong so an unauthenticated caller cannot probe the worker.
    pub fn requires_token(&self) -> bool {
        !matches!(self, Route::Root)
    }

    /// Whether this route is rejected before token authentication.
    pub fn is_unknown(&self) -> bool {
        matches!(self, Route::Unknown(_))
    }
}

impl fmt::Display for Route {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Reply sent for a route the worker does not implement.
pub const UNKNOWN_ROUTE_MESSAGE: &str =
    "Vise Worker: the requested API endpoint is unknown. See GetWorkerInfo for the route list.";

/// Reply prefix for a rejected token. The route is appended.
pub fn access_denied_message(route: &Route, reason: &str) -> String {
    format!(
        "Vise Worker: access denied to '/{}'. The token supplied in the '{}' header {}.",
        route.name(),
        TOKEN_HEADER,
        reason
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_documented_route() {
        let cases = [
            ("/", Route::Root),
            ("/IsTokenSet", Route::IsTokenSet),
            ("/RegisterToken", Route::RegisterToken),
            ("/GetWorkerInfo", Route::GetWorkerInfo),
            ("/StartScan", Route::StartScan),
            ("/StopScan", Route::StopScan),
            ("/PollScanProgress", Route::PollScanProgress),
            ("/GetTitles", Route::GetTitles),
            ("/GetMainTitle", Route::GetMainTitle),
            ("/StartEncode", Route::StartEncode),
            ("/PauseEncode", Route::PauseEncode),
            ("/ResumeEncode", Route::ResumeEncode),
            ("/StopEncode", Route::StopEncode),
            ("/PollEncodeProgress", Route::PollEncodeProgress),
            ("/GetPreview", Route::GetPreview),
            ("/GetAllLogMessages", Route::GetAllLogMessages),
            ("/GetLogMessagesFromIndex", Route::GetLogMessagesFromIndex),
            ("/ResetLogging", Route::ResetLogging),
            ("/Shutdown", Route::Shutdown),
        ];
        for (path, expected) in cases {
            let route = Route::parse(path);
            assert_eq!(route, expected, "path {path}");
            let reparsed = Route::parse(&format!("/{}", route.name()));
            assert_eq!(reparsed, expected, "round trip for {path}");
        }
    }

    #[test]
    fn unknown_routes_are_flagged() {
        let route = Route::parse("/NoSuchThing");
        assert!(route.is_unknown());
        assert_eq!(route.name(), "NoSuchThing");

        // Matching is exact, so a differently cased path is unknown too.
        let lower = Route::parse("/startscan");
        assert!(lower.is_unknown());
        assert_eq!(lower.name(), "startscan");
    }

    #[test]
    fn method_and_auth_requirements_match_the_contract() {
        assert_eq!(Route::parse("/StartScan").allowed_methods(), &["POST"]);
        assert_eq!(Route::parse("/GetTitles").allowed_methods(), &["GET"]);
        assert!(!Route::Root.requires_token());
        assert!(Route::parse("/Shutdown").requires_token());
    }

    #[test]
    fn messages_are_vise_branded() {
        assert!(UNKNOWN_ROUTE_MESSAGE.starts_with("Vise Worker"));
        let msg = access_denied_message(&Route::parse("/StartEncode"), "was not valid");
        assert!(msg.contains("/StartEncode"));
        assert!(msg.contains("token"));
    }

    #[test]
    fn every_listed_route_parses_and_needs_a_token_except_the_banner() {
        assert_eq!(Route::ALL.len(), 19);
        for route in Route::ALL {
            let reparsed = Route::parse(&format!("/{}", route.name()));
            assert_eq!(&reparsed, route, "route {} must round trip", route.name());
            assert!(!route.is_unknown());
            assert_eq!(
                route.requires_token(),
                !matches!(route, Route::Root),
                "route {}",
                route.name()
            );
        }
    }

    #[test]
    fn every_listed_route_allows_at_least_one_verb() {
        for route in Route::ALL {
            assert!(
                !route.allowed_methods().is_empty(),
                "route {} allows nothing",
                route.name()
            );
        }
    }
}
