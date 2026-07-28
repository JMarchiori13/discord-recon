//! Discord API error shape handling shared by API-backed modules.

use anyhow::{bail, Result};
use reqwest::blocking::Response;
use serde::Deserialize;

/// Discord's standard error body: `{"code": 10006, "message": "Unknown Invite"}`.
#[derive(Debug, Deserialize)]
pub struct DiscordError {
    pub code: i64,
    pub message: String,
}

/// Parse a Discord error body, tolerating non-JSON responses.
pub fn parse_error_body(body: &str) -> Option<DiscordError> {
    serde_json::from_str::<DiscordError>(body).ok()
}

/// Check a response for success; on failure, bail with the decoded Discord
/// error code/message (e.g. `Discord error 10006: Unknown Invite`).
pub fn ensure_success(resp: Response) -> Result<Response> {
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    let body = resp.text().unwrap_or_default();
    if let Some(err) = parse_error_body(&body) {
        bail!("Discord error {}: {}", err.code, err.message);
    }
    bail!("Discord API returned {status}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_discord_error_shapes() {
        let err = parse_error_body(r#"{"code": 10006, "message": "Unknown Invite"}"#).unwrap();
        assert_eq!(err.code, 10006);
        assert_eq!(err.message, "Unknown Invite");

        let err = parse_error_body(r#"{"code": 50004, "message": "Widget Disabled"}"#).unwrap();
        assert_eq!(err.code, 50004);

        assert!(parse_error_body("<html>cloudflare</html>").is_none());
        assert!(parse_error_body("").is_none());
    }
}
