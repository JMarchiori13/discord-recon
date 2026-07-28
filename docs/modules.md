# Module Reference

Every module returns a `ModuleOutput` (structured JSON + flat table rows)
rendered as console tables, `--json`/`--csv` exports, or `--stdout` JSONL.
All targets accept `-` to read one target per line from stdin.

## `invite` — invite intelligence (KEYLESS)

| | |
|---|---|
| Source | `GET https://discord.com/api/v10/invites/{code}?with_counts=true&with_expiration=true` |
| Auth | none |
| ATT&CK | T1593.001 (Social Media) |
| Input | bare code, `discord.gg/<code>`, `discord.com/invite/<code>` (query strings tolerated) |
| Output fields | guild name/ID/description, channel, approximate member/online counts, inviter (when exposed), expiration, NSFW flag, verification level, boost tier + boost count, vanity URL, features |
| Errors | Discord `10006 Unknown Invite` (invalid/expired/revoked) is decoded into a clear message; non-zero exit for single targets |
| Limitations | Inviter is only exposed on some invite types. Counts are approximate by design. Some vanity invites omit expiration. |

## `guild` — guild widget (KEYLESS)

| | |
|---|---|
| Source | `GET https://discord.com/api/v10/guilds/{id}/widget.json` |
| Auth | none — works only when the guild admins enabled the server widget |
| ATT&CK | T1593.001 (Social Media) |
| Output fields | guild name, instant invite URL, presence count, channels (name/ID/position), online members (ID, username, avatar hash, status, activity) |
| Errors | Discord `50004 Widget Disabled` → graceful, actionable message (most large servers disable the widget) |
| Limitations | Only *online* members are listed, capped by Discord. No message/role data. |

## `snowflake` — ID decoding (KEYLESS, offline)

| | |
|---|---|
| Source | none — pure bit math (`DISCORD_EPOCH = 1420070400000`) |
| ATT&CK | T1589 / T1593 (timeline reconstruction) |
| Output fields | UTC timestamp, relative age, worker ID, process ID, increment |
| Input | one or more IDs positionally, or `-` for stdin batch |
| Limitations | IDs below `1 << 22` are rejected as non-snowflakes. Timestamps are creation times, not activity times. |

## `webhook` — webhook metadata (KEYLESS, read-only)

| | |
|---|---|
| Source | `GET https://discord.com/api/webhooks/{id}/{token}` |
| Auth | none (the webhook URL itself is the credential — treat found URLs as sensitive) |
| ATT&CK | T1593 |
| Output fields | name, avatar hash, type (Incoming / Channel Follower / Application), guild/channel IDs, creator application ID, creator user (when exposed) |
| Boundary | **Metadata only — this tool never POSTs to a webhook.** |
| Limitations | Deleted/rotated webhooks return `10027 Unknown Webhook` (decoded). Leaked webhook URLs are sensitive findings: report, don't use. |

## `user` — user profiles (BOT-TOKEN tier)

| | |
|---|---|
| Source | `GET https://discord.com/api/v10/users/{id}` |
| Auth | `DISCORD_RECON_BOT_TOKEN` env var (your own bot; read-only) |
| ATT&CK | T1589.001 (Identity) |
| Output fields | username, global name, legacy discriminator, bot flag, avatar/banner hashes + CDN URLs, accent color, `public_flags` raw + decoded badges (Staff, Partner, HypeSquad houses, Bug Hunter 1/2, Early Supporter, Verified Bot, Active Developer, …) |
| Errors | missing token → clear tier explanation; 401 → token invalid/revoked; `10013 Unknown User` decoded |
| Limitations | Only public profile fields. Presence/status is not exposed by this endpoint. |

## `full` — smart orchestrator

Detects the target type and chains modules:

| Target looks like | Runs |
|---|---|
| webhook URL or `id/token` | `webhook` |
| invite URL/code | `invite` → `guild` (chained on the resolved guild ID) |
| numeric snowflake | `snowflake` → `guild` (widget attempt) → `user` (bot tier only) |

Individual module failures degrade to warnings; the orchestrator always
exits with whatever succeeded.
