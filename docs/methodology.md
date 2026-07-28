# Discord OSINT Methodology

`discord-recon` implements the **read-only** portion of Discord
reconnaissance for authorized engagements. This document describes the
tradecraft, the MITRE ATT&CK mapping, and the hard ToS/ethics boundary.

## The hard boundary (non-negotiable)

Discord's Terms of Service prohibit automating user accounts ("selfbots"),
scraping, and using the API for surveillance. This tool therefore:

- **NEVER** accepts or uses a user token (selfbot automation).
- **NEVER** validates, checks, or brute-forces tokens of any kind.
- **NEVER** reads or scrapes message content.
- **NEVER** performs mass actions (joins, DMs, friend requests, posts).
- **NEVER** posts to webhooks — the `webhook` module reads metadata only.
- **ONLY** calls public endpoints (the same ones Discord's own client uses
  to render an invite page or a server widget) or official bot-token
  endpoints with the user's own bot token.

Contributions crossing this boundary are rejected (see CONTRIBUTING.md).

## Legal frame

Unauthorized reconnaissance against third parties may violate **Brazil's
Lei nº 12.737/2012**, the **U.S. Computer Fraud and Abuse Act (CFAA)**, and
computer-misuse statutes elsewhere. Personal data collected during
engagements is subject to **LGPD/GDPR**. Run this tool only where you have
authorization or a legitimate research basis, and handle outputs under the
engagement's data-handling rules.

## What the modules touch

| Module | Endpoint | Why it's public/authorized |
|--------|----------|----------------------------|
| `invite` | `GET /api/v10/invites/{code}?with_counts=true` | The exact call Discord's client makes when a user opens an invite link. Keyless. |
| `guild` | `GET /api/v10/guilds/{id}/widget.json` | Public by design for guilds whose admins enabled the widget; 50004 when disabled (handled gracefully). Keyless. |
| `snowflake` | none | Pure math on the ID itself (Discord epoch 1420070400000). No request at all. |
| `webhook` | `GET /api/webhooks/{id}/{token}` | The read call a chat client makes to render webhook info. Keyless; metadata only, never POST. |
| `user` | `GET /api/v10/users/{id}` | Official bot-token surface — any bot may read public user profiles. Your own bot only. |

## ATT&CK mapping

- **T1593.001** — Search Open Websites/Domains: Social Media (invite, guild, webhook)
- **T1589** — Gather Victim Identity Information (user profiles, snowflake-derived account ages)

## Tradecraft notes

- **Snowflakes are timelines.** Every Discord ID encodes its creation time.
  Account age is a first-order signal for sockpuppet/infrastructure
  analysis; guild age contextualizes an invite's target.
- **Invites leak posture.** Verification level, boost tier, features and
  NSFW flags describe a community's moderation and maturity before you ever
  join it (this tool never joins).
- **Approximate counts are enough.** `with_counts` gives approximate
  member/presence numbers — directionally useful for sizing a community
  without touching it.
- **Widgets are opt-in.** Most large servers disable the widget; a 50004
  is the normal case, not a failure. The `full` orchestrator treats it as
  a graceful skip.
- **Rate limits are a signal.** Discord 429s are honored via `Retry-After`;
  combined with the default 1 req/s throttle, batch runs stay polite.

## OPSEC

- Requests rotate common browser user-agents and are indistinguishable from
  an ordinary invite-page load.
- The bot-token tier ties requests to *your* application — use a dedicated
  research bot, not a production one.
- Exported JSON/CSV may contain personal data (usernames, IDs) — treat it
  as engagement-sensitive material and purge at closeout.
