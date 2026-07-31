//! Invite intelligence — KEYLESS.
//!
//! `GET /api/v10/invites/{code}?with_counts=true&with_expiration=true` is a
//! public endpoint (it's what Discord's own client calls when you open an
//! invite link). Returns guild identity, channel, approximate member/online
//! counts, inviter, expiration and server features — no authentication,
//! read-only (ATT&CK T1593 — Search Open Websites/Domains: Social Media).

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::json;

use super::api::ensure_success;
use crate::http::HttpClient;
use crate::output::ModuleOutput;

const API: &str = "https://discord.com/api/v10";

/// Extract the invite code from a code or any Discord invite URL form.
pub fn parse_invite_code(raw: &str) -> Result<String> {
    let t = raw.trim().trim_end_matches('/');
    let code = t
        .strip_prefix("https://discord.gg/")
        .or_else(|| t.strip_prefix("http://discord.gg/"))
        .or_else(|| t.strip_prefix("https://www.discord.gg/"))
        .or_else(|| t.strip_prefix("discord.gg/"))
        .or_else(|| t.strip_prefix("www.discord.gg/"))
        .or_else(|| t.strip_prefix("https://discord.com/invite/"))
        .or_else(|| t.strip_prefix("https://discordapp.com/invite/"))
        .or_else(|| t.strip_prefix("https://www.discord.com/invite/"))
        .or_else(|| t.strip_prefix("https://canary.discord.com/invite/"))
        .or_else(|| t.strip_prefix("https://ptb.discord.com/invite/"))
        .unwrap_or(t);
    let code = code.split(['?', '#']).next().unwrap_or(code).trim();
    if code.is_empty()
        || code.contains('/')
        || !code
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!("invalid invite {raw:?}: expected a code or discord.gg URL");
    }
    Ok(code.to_string())
}

#[derive(Debug, Deserialize)]
struct InviteResponse {
    #[serde(rename = "type")]
    _itype: Option<u32>,
    expires_at: Option<String>,
    approximate_member_count: Option<u64>,
    approximate_presence_count: Option<u64>,
    guild: Option<InviteGuild>,
    channel: Option<InviteChannel>,
    inviter: Option<InviteUser>,
}

#[derive(Debug, Deserialize)]
struct InviteGuild {
    id: Option<String>,
    name: Option<String>,
    icon: Option<String>,
    description: Option<String>,
    nsfw: Option<bool>,
    nsfw_level: Option<u32>,
    verification_level: Option<u32>,
    premium_tier: Option<u32>,
    premium_subscription_count: Option<u32>,
    #[serde(default)]
    features: Vec<String>,
    vanity_url_code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct InviteChannel {
    id: Option<String>,
    name: Option<String>,
    #[serde(rename = "type")]
    _ctype: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct InviteUser {
    id: Option<String>,
    username: Option<String>,
    global_name: Option<String>,
    avatar: Option<String>,
}

fn verification_level_name(level: Option<u32>) -> String {
    match level {
        Some(0) => "0 — None".to_string(),
        Some(1) => "1 — Low (verified email)".to_string(),
        Some(2) => "2 — Medium (registered >5 min)".to_string(),
        Some(3) => "3 — High (member >10 min)".to_string(),
        Some(4) => "4 — Very High (verified phone)".to_string(),
        _ => "(unknown)".to_string(),
    }
}

fn boost_tier_name(tier: Option<u32>) -> String {
    match tier {
        Some(0) => "0 — None".to_string(),
        Some(1) => "1 — Level 1".to_string(),
        Some(2) => "2 — Level 2".to_string(),
        Some(3) => "3 — Level 3".to_string(),
        _ => "(unknown)".to_string(),
    }
}

/// Look up an invite code (or URL) on the public invites endpoint.
pub fn run(client: &HttpClient, raw_code: &str) -> Result<ModuleOutput> {
    let code = parse_invite_code(raw_code)?;
    let url = format!("{API}/invites/{code}?with_counts=true&with_expiration=true");

    let resp = client.get(&url)?;
    let resp = ensure_success(resp).map_err(|e| {
        anyhow::anyhow!("invite lookup failed for {code}: {e:#} (invalid, expired, or revoked)")
    })?;
    let inv: InviteResponse = resp
        .json()
        .with_context(|| format!("parsing invite response for {code}"))?;

    let g = inv.guild.as_ref();
    let bool_str = |o: Option<bool>| match o {
        Some(true) => "yes".to_string(),
        Some(false) => "no".to_string(),
        None => "(unknown)".to_string(),
    };

    let mut rows: Vec<Vec<String>> = vec![
        vec!["Invite code".to_string(), code.clone()],
        vec![
            "Guild".to_string(),
            format!(
                "{} ({})",
                g.and_then(|g| g.name.clone()).unwrap_or_default(),
                g.and_then(|g| g.id.clone()).unwrap_or_default()
            ),
        ],
        vec![
            "Description".to_string(),
            g.and_then(|g| g.description.clone()).unwrap_or_default(),
        ],
        vec![
            "Channel".to_string(),
            inv.channel
                .as_ref()
                .map(|c| {
                    format!(
                        "#{} ({})",
                        c.name.clone().unwrap_or_default(),
                        c.id.clone().unwrap_or_default()
                    )
                })
                .unwrap_or_default(),
        ],
        vec![
            "Approx. members".to_string(),
            inv.approximate_member_count
                .map(|n| n.to_string())
                .unwrap_or_default(),
        ],
        vec![
            "Approx. online".to_string(),
            inv.approximate_presence_count
                .map(|n| n.to_string())
                .unwrap_or_default(),
        ],
        vec![
            "Inviter".to_string(),
            inv.inviter
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
        vec![
            "Expires at".to_string(),
            inv.expires_at
                .clone()
                .unwrap_or_else(|| "(never)".to_string()),
        ],
        vec!["NSFW".to_string(), bool_str(g.and_then(|g| g.nsfw))],
        vec![
            "Verification level".to_string(),
            verification_level_name(g.and_then(|g| g.verification_level)),
        ],
        vec![
            "Boost tier".to_string(),
            format!(
                "{} ({} boosts)",
                boost_tier_name(g.and_then(|g| g.premium_tier)),
                g.and_then(|g| g.premium_subscription_count)
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "?".to_string())
            ),
        ],
        vec![
            "Vanity URL".to_string(),
            g.and_then(|g| g.vanity_url_code.clone())
                .unwrap_or_default(),
        ],
        vec![
            "Features".to_string(),
            g.map(|g| g.features.join(", ")).unwrap_or_default(),
        ],
    ];
    rows.retain(|r| !r[1].is_empty());

    Ok(ModuleOutput {
        name: "Invite intelligence",
        json: json!({
            "module": "invite",
            "code": code,
            "guild": g.map(|g| json!({
                "id": g.id,
                "name": g.name,
                "icon": g.icon,
                "description": g.description,
                "nsfw": g.nsfw,
                "nsfw_level": g.nsfw_level,
                "verification_level": g.verification_level,
                "premium_tier": g.premium_tier,
                "premium_subscription_count": g.premium_subscription_count,
                "features": g.features,
                "vanity_url_code": g.vanity_url_code,
            })),
            "channel": inv.channel.as_ref().map(|c| json!({"id": c.id, "name": c.name})),
            "approximate_member_count": inv.approximate_member_count,
            "approximate_presence_count": inv.approximate_presence_count,
            "inviter": inv.inviter.as_ref().map(|u| json!({
                "id": u.id, "username": u.username,
                "global_name": u.global_name, "avatar": u.avatar,
            })),
            "expires_at": inv.expires_at,
        }),
        headers: vec!["Field", "Value"],
        rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_invite_code_variants() {
        assert_eq!(
            parse_invite_code("discord-developers").unwrap(),
            "discord-developers"
        );
        assert_eq!(
            parse_invite_code("https://discord.gg/python").unwrap(),
            "python"
        );
        assert_eq!(
            parse_invite_code("http://discord.gg/python").unwrap(),
            "python"
        );
        assert_eq!(parse_invite_code("discord.gg/python/").unwrap(), "python");
        assert_eq!(
            parse_invite_code("https://discord.com/invite/python").unwrap(),
            "python"
        );
        assert_eq!(
            parse_invite_code("https://discord.gg/python?foo=bar").unwrap(),
            "python"
        );
        assert_eq!(
            parse_invite_code("https://discordapp.com/invite/python").unwrap(),
            "python"
        );
        assert_eq!(
            parse_invite_code("https://www.discord.gg/python").unwrap(),
            "python"
        );
        assert_eq!(
            parse_invite_code("www.discord.gg/python").unwrap(),
            "python"
        );
        assert_eq!(
            parse_invite_code("https://canary.discord.com/invite/python").unwrap(),
            "python"
        );
        assert_eq!(
            parse_invite_code("https://ptb.discord.com/invite/python").unwrap(),
            "python"
        );
    }

    #[test]
    fn rejects_invalid_invites() {
        assert!(parse_invite_code("").is_err());
        assert!(parse_invite_code("   ").is_err());
        assert!(parse_invite_code("foo/bar").is_err());
        assert!(parse_invite_code("bad code!").is_err());
    }

    #[test]
    fn names_verification_and_boost_levels() {
        assert_eq!(
            verification_level_name(Some(4)),
            "4 — Very High (verified phone)"
        );
        assert_eq!(verification_level_name(None), "(unknown)");
        assert_eq!(boost_tier_name(Some(3)), "3 — Level 3");
        assert_eq!(boost_tier_name(Some(0)), "0 — None");
    }
}
