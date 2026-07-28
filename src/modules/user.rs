//! User profile intelligence — BOT-TOKEN tier.
//!
//! `GET /api/v10/users/{id}` requires a **bot token** (env var
//! `DISCORD_RECON_BOT_TOKEN`, user-provided). This is an official,
//! read-only API surface available to any bot — no user-token automation
//! (selfbots), which would violate Discord ToS. Returns the public profile:
//! username, global name, avatar/banner hashes, accent color and
//! `public_flags` decoded into badges (ATT&CK T1589 — Gather Victim
//! Identity Information).
//!
//! Without a token the module exits with a clear tier explanation.

use std::env;

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::json;

use super::api::ensure_success;
use crate::http::HttpClient;
use crate::output::ModuleOutput;

const API: &str = "https://discord.com/api/v10";

/// Discord `public_flags` bits → badge names (documented in the API docs).
const PUBLIC_FLAGS: &[(u64, &str)] = &[
    (1 << 0, "Discord Staff"),
    (1 << 1, "Partner"),
    (1 << 2, "HypeSquad Events"),
    (1 << 3, "Bug Hunter Level 1"),
    (1 << 6, "HypeSquad Bravery"),
    (1 << 7, "HypeSquad Brilliance"),
    (1 << 8, "HypeSquad Balance"),
    (1 << 9, "Early Supporter"),
    (1 << 10, "Team User"),
    (1 << 14, "Bug Hunter Level 2"),
    (1 << 16, "Verified Bot"),
    (1 << 17, "Early Verified Bot Developer"),
    (1 << 18, "Moderator Programs Alumni"),
    (1 << 19, "Bot HTTP Interactions"),
    (1 << 22, "Active Developer"),
];

/// Decode `public_flags` into badge names.
pub fn decode_public_flags(flags: u64) -> Vec<&'static str> {
    PUBLIC_FLAGS
        .iter()
        .filter(|(bit, _)| flags & bit != 0)
        .map(|(_, name)| *name)
        .collect()
}

/// Resolve the bot token from the environment (never logged or exported).
pub fn resolve_token() -> Option<String> {
    env::var("DISCORD_RECON_BOT_TOKEN")
        .ok()
        .filter(|t| !t.trim().is_empty())
}

#[derive(Debug, Deserialize)]
struct UserResponse {
    id: Option<String>,
    username: Option<String>,
    global_name: Option<String>,
    discriminator: Option<String>,
    avatar: Option<String>,
    banner: Option<String>,
    accent_color: Option<u64>,
    public_flags: Option<u64>,
    bot: Option<bool>,
}

/// Look up a user's public profile (bot-token tier).
pub fn run(client: &HttpClient, user_id: &str) -> Result<ModuleOutput> {
    let id = user_id.trim();
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
        bail!("invalid user ID {user_id:?}: expected a numeric Discord snowflake");
    }

    let Some(token) = resolve_token() else {
        bail!(
            "the `user` module requires the bot-token tier.\n\
             \n\
             Set a Discord bot token in the environment and retry:\n\
             \n  \
             export DISCORD_RECON_BOT_TOKEN=\"Bot-token-from-dev-portal\"\n\
             \n\
             Create one at https://discord.com/developers/applications \
             (New Application → Bot → Reset Token). Read-only endpoints only — \
             never use a USER token here (selfbots violate Discord ToS)."
        );
    };

    let auth = format!("Bot {token}");
    let resp =
        client.get_with_headers(&format!("{API}/users/{id}"), &[("Authorization", &auth)])?;
    let status = resp.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        bail!("Discord returned 401 Unauthorized — the bot token in DISCORD_RECON_BOT_TOKEN is invalid or revoked");
    }
    let resp = ensure_success(resp)?;
    let u: UserResponse = resp
        .json()
        .with_context(|| format!("parsing user response for {id}"))?;

    let str_opt = |o: &Option<String>| o.clone().unwrap_or_default();
    let badges = decode_public_flags(u.public_flags.unwrap_or(0));
    let avatar_url = u.avatar.as_ref().map(|a| {
        format!(
            "https://cdn.discordapp.com/avatars/{}/{}.png",
            str_opt(&u.id),
            a
        )
    });
    let banner_url = u.banner.as_ref().map(|b| {
        format!(
            "https://cdn.discordapp.com/banners/{}/{}.png",
            str_opt(&u.id),
            b
        )
    });

    let mut rows: Vec<Vec<String>> = vec![
        vec!["User ID".to_string(), str_opt(&u.id)],
        vec!["Username".to_string(), str_opt(&u.username)],
        vec!["Global name".to_string(), str_opt(&u.global_name)],
        vec![
            "Legacy discriminator".to_string(),
            str_opt(&u.discriminator),
        ],
        vec![
            "Is bot".to_string(),
            match u.bot {
                Some(true) => "yes".to_string(),
                _ => "no".to_string(),
            },
        ],
        vec!["Avatar hash".to_string(), str_opt(&u.avatar)],
        vec![
            "Avatar URL".to_string(),
            avatar_url.clone().unwrap_or_default(),
        ],
        vec!["Banner hash".to_string(), str_opt(&u.banner)],
        vec![
            "Banner URL".to_string(),
            banner_url.clone().unwrap_or_default(),
        ],
        vec![
            "Accent color".to_string(),
            u.accent_color
                .map(|c| format!("#{c:06x} ({c})"))
                .unwrap_or_default(),
        ],
        vec![
            "Public flags".to_string(),
            u.public_flags
                .map(|f| format!("{f} (0x{f:x})"))
                .unwrap_or_default(),
        ],
        vec!["Badges".to_string(), badges.join(", ")],
    ];
    rows.retain(|r| !r[1].is_empty());

    Ok(ModuleOutput {
        name: "User profile intelligence (bot tier)",
        json: json!({
            "module": "user",
            "tier": "bot-token",
            "user": {
                "id": u.id,
                "username": u.username,
                "global_name": u.global_name,
                "discriminator": u.discriminator,
                "bot": u.bot,
                "avatar": u.avatar,
                "avatar_url": avatar_url,
                "banner": u.banner,
                "banner_url": banner_url,
                "accent_color": u.accent_color,
                "public_flags": u.public_flags,
                "badges": badges,
            },
        }),
        headers: vec!["Field", "Value"],
        rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_public_flags() {
        let badges = decode_public_flags((1 << 0) | (1 << 6) | (1 << 22));
        assert_eq!(
            badges,
            vec!["Discord Staff", "HypeSquad Bravery", "Active Developer"]
        );

        assert!(decode_public_flags(0).is_empty());

        // All known flags decode without panic.
        let all: u64 = PUBLIC_FLAGS.iter().map(|(b, _)| b).sum();
        assert_eq!(decode_public_flags(all).len(), PUBLIC_FLAGS.len());

        // Unknown bits are ignored.
        assert!(decode_public_flags(1 << 5).is_empty());
    }

    #[test]
    fn parses_user_response() {
        let body = r#"{
            "id": "80351110224678912",
            "username": "nelly",
            "global_name": "Nelly",
            "discriminator": "0",
            "avatar": "a_abc123",
            "banner": "b_def456",
            "accent_color": 16711680,
            "public_flags": 4194560,
            "bot": false
        }"#;
        let u: UserResponse = serde_json::from_str(body).unwrap();
        assert_eq!(u.username.as_deref(), Some("nelly"));
        assert_eq!(u.accent_color, Some(16711680));
        let badges = decode_public_flags(u.public_flags.unwrap());
        assert!(!badges.is_empty());
    }
}
