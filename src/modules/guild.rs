//! Guild widget intelligence — KEYLESS (when the server widget is enabled).
//!
//! `GET /api/v10/guilds/{id}/widget.json` is public for any guild whose
//! admins enabled the server widget: name, instant invite, channels and
//! **online members** (ids, usernames, avatars, status, activities). When
//! the widget is disabled Discord returns error 50004 — reported gracefully
//! (ATT&CK T1593.001 — Social Media).

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::json;

use super::api::{ensure_success, parse_error_body};
use crate::http::HttpClient;
use crate::output::ModuleOutput;

const API: &str = "https://discord.com/api/v10";

#[derive(Debug, Deserialize)]
struct WidgetResponse {
    name: Option<String>,
    instant_invite: Option<String>,
    presence_count: Option<u64>,
    #[serde(default)]
    channels: Vec<WidgetChannel>,
    #[serde(default)]
    members: Vec<WidgetMember>,
}

#[derive(Debug, Deserialize)]
struct WidgetChannel {
    id: Option<String>,
    name: Option<String>,
    position: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct WidgetMember {
    id: Option<String>,
    username: Option<String>,
    avatar: Option<String>,
    status: Option<String>,
    #[serde(default)]
    activity: Option<WidgetActivity>,
}

#[derive(Debug, Deserialize)]
struct WidgetActivity {
    name: Option<String>,
}

/// Fetch the public widget data for a guild ID.
pub fn run(client: &HttpClient, guild_id: &str) -> Result<ModuleOutput> {
    let id = guild_id.trim();
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
        bail!("invalid guild ID {guild_id:?}: expected a numeric Discord snowflake");
    }
    let url = format!("{API}/guilds/{id}/widget.json");

    let resp = client.get(&url)?;
    if resp.status() == reqwest::StatusCode::FORBIDDEN {
        let body = resp.text().unwrap_or_default();
        if let Some(err) = parse_error_body(&body) {
            if err.code == 50004 {
                bail!("widget disabled for guild {id} (Discord error 50004) — the server admins have not enabled the public widget; try the `invite` module if you have an invite code");
            }
            bail!("Discord error {}: {}", err.code, err.message);
        }
        bail!("access forbidden for guild {id} widget");
    }
    let resp = ensure_success(resp)?;
    let w: WidgetResponse = resp
        .json()
        .with_context(|| format!("parsing widget response for guild {id}"))?;

    let mut rows: Vec<Vec<String>> = vec![
        vec!["Guild name".to_string(), w.name.clone().unwrap_or_default()],
        vec!["Guild ID".to_string(), id.to_string()],
        vec![
            "Online members (widget)".to_string(),
            w.presence_count
                .map(|n| n.to_string())
                .unwrap_or_else(|| w.members.len().to_string()),
        ],
        vec![
            "Instant invite".to_string(),
            w.instant_invite
                .clone()
                .unwrap_or_else(|| "(none)".to_string()),
        ],
    ];
    for ch in &w.channels {
        rows.push(vec![
            "Channel".to_string(),
            format!(
                "#{} ({})",
                ch.name.clone().unwrap_or_default(),
                ch.id.clone().unwrap_or_default()
            ),
        ]);
    }
    for m in &w.members {
        rows.push(vec![
            "Member (online)".to_string(),
            format!(
                "{} ({}) — {}{}",
                m.username.clone().unwrap_or_default(),
                m.id.clone().unwrap_or_default(),
                m.status.clone().unwrap_or_default(),
                m.activity
                    .as_ref()
                    .and_then(|a| a.name.clone())
                    .map(|n| format!(", activity: {n}"))
                    .unwrap_or_default()
            ),
        ]);
    }

    Ok(ModuleOutput {
        name: "Guild widget intelligence",
        json: json!({
            "module": "guild",
            "guild_id": id,
            "name": w.name,
            "instant_invite": w.instant_invite,
            "presence_count": w.presence_count,
            "channels": w.channels.iter().map(|c| json!({
                "id": c.id, "name": c.name, "position": c.position,
            })).collect::<Vec<_>>(),
            "members": w.members.iter().map(|m| json!({
                "id": m.id,
                "username": m.username,
                "avatar": m.avatar,
                "status": m.status,
                "activity": m.activity.as_ref().and_then(|a| a.name.clone()),
            })).collect::<Vec<_>>(),
        }),
        headers: vec!["Field", "Value"],
        rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_widget_response() {
        let body = r#"{
            "name": "Example Guild",
            "instant_invite": "https://discord.com/invite/abcd",
            "presence_count": 42,
            "channels": [{"id": "111", "name": "general", "position": 0}],
            "members": [
                {"id": "222", "username": "alice", "status": "online",
                 "activity": {"name": "Visual Studio Code"}},
                {"id": "333", "username": "bob", "status": "idle"}
            ]
        }"#;
        let w: WidgetResponse = serde_json::from_str(body).unwrap();
        assert_eq!(w.name.as_deref(), Some("Example Guild"));
        assert_eq!(w.channels.len(), 1);
        assert_eq!(w.members.len(), 2);
        assert_eq!(
            w.members[0].activity.as_ref().and_then(|a| a.name.clone()),
            Some("Visual Studio Code".to_string())
        );
    }

    #[test]
    fn tolerates_empty_widget() {
        let w: WidgetResponse =
            serde_json::from_str(r#"{"name": "Empty", "channels": [], "members": []}"#).unwrap();
        assert!(w.members.is_empty());
        assert!(w.instant_invite.is_none());
    }
}
