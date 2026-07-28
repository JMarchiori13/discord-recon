//! Webhook metadata — KEYLESS, read-only.
//!
//! `GET /api/webhooks/{id}/{token}` returns a webhook's metadata (name,
//! avatar, owning guild/channel, creator application) — the same call a
//! chat client makes to display webhook info. This module **only** reads
//! metadata and will **never** POST to the webhook (ATT&CK T1593).

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::json;

use super::api::ensure_success;
use crate::http::HttpClient;
use crate::output::ModuleOutput;

const API: &str = "https://discord.com/api";

/// Extract (id, token) from a webhook URL or an `id/token` pair.
pub fn parse_webhook(raw: &str) -> Result<(String, String)> {
    let t = raw.trim().trim_end_matches('/');
    let path = t
        .strip_prefix("https://discord.com/api/webhooks/")
        .or_else(|| t.strip_prefix("https://discordapp.com/api/webhooks/"))
        .or_else(|| t.strip_prefix("https://canary.discord.com/api/webhooks/"))
        .or_else(|| t.strip_prefix("https://ptb.discord.com/api/webhooks/"))
        .unwrap_or(t);
    let mut parts = path.split('/');
    let id = parts.next().unwrap_or("");
    let token = parts.next().unwrap_or("").split('?').next().unwrap_or("");
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
        bail!("invalid webhook {raw:?}: missing numeric webhook ID");
    }
    if token.is_empty() {
        bail!("invalid webhook {raw:?}: missing webhook token");
    }
    Ok((id.to_string(), token.to_string()))
}

#[derive(Debug, Deserialize)]
struct WebhookResponse {
    #[serde(rename = "type")]
    wtype: Option<u32>,
    name: Option<String>,
    avatar: Option<String>,
    channel_id: Option<String>,
    guild_id: Option<String>,
    application_id: Option<String>,
    user: Option<WebhookUser>,
}

#[derive(Debug, Deserialize)]
struct WebhookUser {
    id: Option<String>,
    username: Option<String>,
    global_name: Option<String>,
}

fn webhook_type_name(t: Option<u32>) -> String {
    match t {
        Some(1) => "1 — Incoming".to_string(),
        Some(2) => "2 — Channel Follower".to_string(),
        Some(3) => "3 — Application".to_string(),
        _ => "(unknown)".to_string(),
    }
}

/// Read webhook metadata (never posts to it).
pub fn run(client: &HttpClient, raw: &str) -> Result<ModuleOutput> {
    let (id, token) = parse_webhook(raw)?;
    let url = format!("{API}/webhooks/{id}/{token}");

    let resp = client.get(&url)?;
    let resp = ensure_success(resp).map_err(|e| {
        anyhow::anyhow!("webhook lookup failed for {id}: {e:#} (invalid or deleted)")
    })?;
    let w: WebhookResponse = resp
        .json()
        .with_context(|| format!("parsing webhook response for {id}"))?;

    let str_opt = |o: &Option<String>| o.clone().unwrap_or_default();
    let rows: Vec<Vec<String>> = vec![
        vec!["Webhook ID".to_string(), id.clone()],
        vec!["Name".to_string(), str_opt(&w.name)],
        vec!["Type".to_string(), webhook_type_name(w.wtype)],
        vec!["Avatar hash".to_string(), str_opt(&w.avatar)],
        vec!["Guild ID".to_string(), str_opt(&w.guild_id)],
        vec!["Channel ID".to_string(), str_opt(&w.channel_id)],
        vec!["Creator app ID".to_string(), str_opt(&w.application_id)],
        vec![
            "Creator user".to_string(),
            w.user
                .as_ref()
                .map(|u| {
                    format!(
                        "{} ({})",
                        u.global_name
                            .clone()
                            .or_else(|| u.username.clone())
                            .unwrap_or_default(),
                        u.id.clone().unwrap_or_default()
                    )
                })
                .unwrap_or_else(|| "(not exposed)".to_string()),
        ],
    ];

    Ok(ModuleOutput {
        name: "Webhook metadata",
        json: json!({
            "module": "webhook",
            "note": "Metadata only — this tool never posts to webhooks.",
            "id": id,
            "name": w.name,
            "type": w.wtype,
            "avatar": w.avatar,
            "guild_id": w.guild_id,
            "channel_id": w.channel_id,
            "application_id": w.application_id,
            "creator_user": w.user.as_ref().map(|u| json!({
                "id": u.id, "username": u.username, "global_name": u.global_name,
            })),
        }),
        headers: vec!["Field", "Value"],
        rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_webhook_url_variants() {
        let (id, token) =
            parse_webhook("https://discord.com/api/webhooks/123456789012345678/AbCdEf-Token_123")
                .unwrap();
        assert_eq!(id, "123456789012345678");
        assert_eq!(token, "AbCdEf-Token_123");

        let (id, token) = parse_webhook("123456789012345678/AbCdEf-Token_123").unwrap();
        assert_eq!(id, "123456789012345678");
        assert_eq!(token, "AbCdEf-Token_123");

        let (_id, token) =
            parse_webhook("https://canary.discord.com/api/webhooks/123456789012345678/tok/")
                .unwrap();
        assert_eq!(token, "tok");

        let (_, token) =
            parse_webhook("https://discord.com/api/webhooks/123456789012345678/tok?wait=true")
                .unwrap();
        assert_eq!(token, "tok");
    }

    #[test]
    fn rejects_invalid_webhooks() {
        assert!(parse_webhook("https://discord.com/api/webhooks/notanumber/tok").is_err());
        assert!(parse_webhook("https://discord.com/api/webhooks/123456789012345678").is_err());
        assert!(parse_webhook("").is_err());
    }

    #[test]
    fn names_webhook_types() {
        assert_eq!(webhook_type_name(Some(1)), "1 — Incoming");
        assert_eq!(webhook_type_name(Some(3)), "3 — Application");
        assert_eq!(webhook_type_name(None), "(unknown)");
    }
}
