//! Smart orchestrator: detect the target type and chain modules.
//!
//! Detection order:
//! 1. Webhook URL (`discord.com/api/webhooks/...` or `id/token` with a
//!    non-numeric token segment) → `webhook`
//! 2. Invite (discord.gg URL, /invite/ URL, or a short alphanumeric code
//!    that is not purely numeric) → `invite` (+ `guild` if it resolves)
//! 3. Numeric snowflake → `snowflake` + `guild` (widget attempt; graceful
//!    if disabled) + `user` (only when the bot tier is available)

use colored::Colorize;
use serde_json::json;

use super::{guild, invite, snowflake, user, webhook};
use crate::http::HttpClient;
use crate::output::ModuleOutput;

/// What a target string looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    Webhook,
    Invite,
    Snowflake,
}

/// Classify a target string (webhook URL, invite, or bare snowflake).
pub fn detect_target(raw: &str) -> TargetKind {
    let t = raw.trim();
    if t.contains("/api/webhooks/") {
        return TargetKind::Webhook;
    }
    if t.contains("discord.gg/") || t.contains("/invite/") {
        return TargetKind::Invite;
    }
    let compact = t.trim_end_matches('/');
    if !compact.is_empty() && compact.bytes().all(|b| b.is_ascii_digit()) {
        return TargetKind::Snowflake;
    }
    // `id/token` pair without the API prefix.
    if let Some((id, token)) = compact.split_once('/') {
        if !id.is_empty()
            && id.bytes().all(|b| b.is_ascii_digit())
            && !token.is_empty()
            && token
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return TargetKind::Webhook;
        }
    }
    TargetKind::Invite
}

/// Run every module applicable to the detected target type.
pub fn run(client: &HttpClient, target: &str) -> Vec<ModuleOutput> {
    let kind = detect_target(target);
    let mut outputs = Vec::new();

    match kind {
        TargetKind::Webhook => {
            eprintln!(
                "{} target looks like a webhook — running `webhook`",
                "[info]".blue()
            );
            match webhook::run(client, target) {
                Ok(out) => outputs.push(out),
                Err(e) => eprintln!("{} webhook module failed: {e:#}", "[warn]".yellow()),
            }
        }
        TargetKind::Invite => {
            eprintln!(
                "{} target looks like an invite — running `invite`",
                "[info]".blue()
            );
            match invite::run(client, target) {
                Ok(out) => {
                    // Chain: widget lookup for the resolved guild.
                    let guild_id = out
                        .json
                        .get("guild")
                        .and_then(|g| g.get("id"))
                        .and_then(|v| v.as_str())
                        .map(str::to_string);
                    outputs.push(out);
                    if let Some(id) = guild_id {
                        eprintln!(
                            "{} chaining `guild` widget lookup for {id}",
                            "[info]".blue()
                        );
                        match guild::run(client, &id) {
                            Ok(out) => outputs.push(out),
                            Err(e) => {
                                eprintln!("{} guild widget unavailable: {e:#}", "[warn]".yellow())
                            }
                        }
                    }
                }
                Err(e) => eprintln!("{} invite module failed: {e:#}", "[warn]".yellow()),
            }
        }
        TargetKind::Snowflake => {
            eprintln!(
                "{} target looks like a snowflake — running `snowflake`, `guild`, `user`",
                "[info]".blue()
            );
            let id = target.trim().to_string();
            match snowflake::run(std::slice::from_ref(&id)) {
                Ok(out) => outputs.push(out),
                Err(e) => eprintln!("{} snowflake module failed: {e:#}", "[warn]".yellow()),
            }
            match guild::run(client, &id) {
                Ok(out) => outputs.push(out),
                Err(e) => eprintln!("{} guild widget unavailable: {e:#}", "[warn]".yellow()),
            }
            if user::resolve_token().is_some() {
                match user::run(client, &id) {
                    Ok(out) => outputs.push(out),
                    Err(e) => eprintln!("{} user lookup failed: {e:#}", "[warn]".yellow()),
                }
            } else {
                eprintln!(
                    "{} no DISCORD_RECON_BOT_TOKEN set — skipping `user` (bot-token tier)",
                    "[info]".blue()
                );
            }
        }
    }

    if outputs.is_empty() {
        outputs.push(ModuleOutput {
            name: "Full reconnaissance",
            json: json!({
                "module": "full",
                "target": target,
                "error": "no module produced results",
            }),
            headers: vec!["Field", "Value"],
            rows: vec![vec![
                "error".to_string(),
                "no module produced results for this target".to_string(),
            ]],
        });
    }
    outputs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_target_kinds() {
        assert_eq!(
            detect_target("https://discord.com/api/webhooks/123456789012345678/abc-token"),
            TargetKind::Webhook
        );
        assert_eq!(
            detect_target("123456789012345678/abc-token_XyZ"),
            TargetKind::Webhook
        );
        assert_eq!(
            detect_target("https://discord.gg/python"),
            TargetKind::Invite
        );
        assert_eq!(
            detect_target("https://discord.com/invite/discord-developers"),
            TargetKind::Invite
        );
        assert_eq!(detect_target("discord-developers"), TargetKind::Invite);
        assert_eq!(detect_target("80351110224678912"), TargetKind::Snowflake);
    }
}
