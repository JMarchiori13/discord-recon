# discord-recon

**Discord OSINT reconnaissance CLI in Rust — read-only, public/authorized API surfaces only.**

[![MITRE ATT&CK T1593](https://img.shields.io/badge/MITRE%20ATT%26CK-T1593%20Social%20Media-red)](https://attack.mitre.org/techniques/T1593/)
[![MITRE ATT&CK T1589](https://img.shields.io/badge/MITRE%20ATT%26CK-T1589%20Identity-red)](https://attack.mitre.org/techniques/T1589/)
[![CI](https://github.com/JMarchiori13/discord-recon/actions/workflows/ci.yml/badge.svg)](https://github.com/JMarchiori13/discord-recon/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/rust-1.95%2B-orange?logo=rust)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

> **⚠️ DISCLAIMER — READ-ONLY, FOR AUTHORIZED SECURITY RESEARCH ONLY**
>
> This tool is **read-only** and uses only public or authorized Discord API
> surfaces. It contains **no selfbot functionality, no user-token
> automation, no token validation/checking, no message scraping, and no
> mass actions** — these violate the
> [Discord Terms of Service](https://discord.com/terms) and this project's
> ethics, and they will never be implemented here. Unauthorized
> reconnaissance against third parties may additionally violate **Brazil's
> Lei nº 12.737/2012** and the **U.S. Computer Fraud and Abuse Act (CFAA)**.
> You are solely responsible for how you use this software.

## Demo

<!-- Placeholder: terminal GIF demo (like osint-recon's docs/assets/demo.gif) will land here. -->

```
$ discord-recon invite discord.gg/discord-developers

Invite intelligence
-------------------
Field               Value
------------------  -------------------------------------------------------------
Guild               Discord Developers (613425648685547541)
Approx. members     294043
Approx. online      58212
Verification level  3 — High (member >10 min)
Boost tier          3 — Level 3 (61 boosts)
```

## Overview

`discord-recon` automates the passive, read-only portion of Discord OSINT
for authorized engagements and research: resolving public invites into
guild intelligence, decoding snowflakes into creation timestamps, reading
public guild widgets and webhook metadata, and (with your own bot token)
enriching user profiles. It maps to MITRE ATT&CK **T1593 (Search Open
Websites/Domains: Social Media)** and **T1589 (Gather Victim Identity
Information)**.

## Two auth tiers

| Tier | How | Modules |
|------|-----|---------|
| **Keyless** | works out of the box — public endpoints only | `invite`, `guild`, `snowflake`, `webhook`, `full` |
| **Bot token** | `export DISCORD_RECON_BOT_TOKEN=...` — your own bot from the [Developer Portal](https://discord.com/developers/applications) (New Application → Bot → Reset Token), read-only endpoints only | adds `user` |

The token is read from the environment only, never logged, never written to
exports. **Never use a user token** — that would be a selfbot (ToS
violation); this tool does not accept one.

## Features

- 📨 **Invite intelligence** (keyless) — guild name/ID, channel, approximate member/online counts, inviter (when exposed), expiration, NSFW flag, verification level, boost tier, features
- 🏰 **Guild widget** (keyless) — name, instant invite, channels, online members with status/activities; graceful "widget disabled" (50004) handling
- ❄️ **Snowflake decoding** (keyless, offline) — UTC timestamp + relative age, worker/process/increment bits; batch + stdin
- 🪝 **Webhook metadata** (keyless) — name, avatar, guild/channel IDs, creator app. Metadata only — **never posts**
- 👤 **User profiles** (bot-token tier) — username, global name, avatar/banner URLs, accent color, `public_flags` decoded to badges
- 🧠 **Smart `full` orchestrator** — detects invite vs snowflake vs webhook and chains the applicable modules
- 📤 **JSON/CSV export, `--stdout` JSONL mode, `-` stdin batch targets**
- 🐢 **Polite by design** — 1 req/s throttle, Discord 429 handling with `Retry-After`, timeouts, retries

## Installation

```sh
# from source
cargo install --git https://github.com/JMarchiori13/discord-recon

# or build locally
git clone https://github.com/JMarchiori13/discord-recon.git
cd discord-recon
cargo build --release
```

Prebuilt binaries for Windows, Linux and macOS (plus sha256 checksums) are
attached to every [GitHub Release](https://github.com/JMarchiori13/discord-recon/releases).

## Usage

```sh
# KEYLESS tier
discord-recon invite discord.gg/discord-developers   # or just: invite discord-developers
discord-recon guild 613425648685547541               # needs widget enabled on the server
discord-recon snowflake 80351110224678912 175928847299117063
discord-recon webhook https://discord.com/api/webhooks/<id>/<token>
discord-recon full discord.gg/discord-developers     # auto-detect & chain

# BOT-TOKEN tier
export DISCORD_RECON_BOT_TOKEN="your-bot-token"
discord-recon user 80351110224678912

# Composability
discord-recon snowflake 80351110224678912 --stdout | jq .
cat ids.txt | discord-recon snowflake -
```

Global options:

| Option | Default | Description |
|--------|---------|-------------|
| `--rate <rps>` | `1.0` | Politeness rate limit (requests/second) |
| `--timeout <s>` | `15` | Per-request timeout (seconds) |
| `--retries <n>` | `2` | Retries per request after the first attempt |
| `--json <file>` | — | Export results as JSON |
| `--csv <file>` | — | Export results as CSV |
| `--stdout` | — | Emit JSONL on stdout; banner/logs move to stderr |
| `-q, --quiet` | — | Suppress the authorization banner |

## Modules & ATT&CK mapping

| Module | Tier | Source | ATT&CK |
|--------|------|--------|--------|
| `invite` | keyless | `GET /api/v10/invites/{code}?with_counts` | T1593.001 — Social Media |
| `guild` | keyless | `GET /api/v10/guilds/{id}/widget.json` | T1593.001 — Social Media |
| `snowflake` | keyless | offline decode (Discord epoch math) | T1589 / T1593 |
| `webhook` | keyless | `GET /api/webhooks/{id}/{token}` (read-only) | T1593 |
| `user` | bot token | `GET /api/v10/users/{id}` | T1589.001 — Credentials/Identity |
| `full` | both | orchestrator chaining the above | — |

See [docs/modules.md](docs/modules.md) for per-module output fields and
limitations, [docs/methodology.md](docs/methodology.md) for tradecraft and
the ToS/ethics boundary, and [docs/lab.md](docs/lab.md) for safe test values.

## Roadmap

- [ ] Guild discovery research (disboard/top.gg public listing analysis)
- [ ] Invite analytics & tracking (count deltas over time, vanity mapping)
- [ ] Discord OPSEC tradecraft guide (docs)
- [ ] Badge/flag history inference from public data
- [ ] telegram-recon — sibling tool idea (same playbook, Telegram public surfaces)

## Project structure

```
discord-recon/
├── Cargo.toml
├── .github/workflows/
│   ├── ci.yml                  # fmt / clippy -D warnings / test / build
│   └── release.yml             # tag v* → win/linux/mac binaries + checksums
├── src/
│   ├── main.rs                 # clap CLI: invite/guild/snowflake/webhook/user/full
│   ├── http.rs                 # shared client: UA rotation, 1 req/s, 429 Retry-After
│   ├── output.rs               # JSON + CSV + JSONL export, console tables
│   └── modules/
│       ├── api.rs              # Discord error-shape handling (10006/50004/401)
│       ├── invite.rs           # keyless invite intelligence
│       ├── guild.rs            # keyless guild widget
│       ├── snowflake.rs        # offline snowflake decoder
│       ├── webhook.rs          # keyless webhook metadata (never posts)
│       ├── user.rs             # bot-token user profiles + flag decoding
│       └── full.rs             # target-type detection & module chaining
├── docs/
│   ├── methodology.md          # Discord OSINT tradecraft + ToS/ethics boundary
│   ├── modules.md              # per-module sources, output fields, limitations
│   └── lab.md                  # safe test values
├── tests/cli.rs                # network-free CLI integration tests
├── CONTRIBUTING.md
├── LICENSE                     # MIT + research-use notice
└── README.md
```

## Development

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --release
cargo build --release
```

Contributions welcome — see [CONTRIBUTING.md](CONTRIBUTING.md). The
**read-only scope is non-negotiable**: PRs adding selfbots, user-token
automation, scraping or mass actions will be rejected.

## License

[MIT](LICENSE) © 2026 JMarchiori13 — see the research-use notice in the
LICENSE file and the disclaimer above.
