# Discord OPSEC Tradecraft Guide

Operational security guidance for using discord-recon on authorized
engagements and research. Companion to [methodology.md](methodology.md);
the hard ToS boundary there applies here.

## For authorized security research only

Everything in this guide assumes the read-only boundary: no selfbots, no
user tokens, no message scraping, no joining, no posting. OPSEC here means
"how to gather public intelligence without misrepresenting yourself or
breaking the rules" — not how to evade Discord's abuse systems.

## How investigators get identified on Discord

Even passive-looking work leaves fingerprints. Know yours:

- **Bot account fingerprints.** A research bot is itself an observable
  identity: creation date (snowflake), default avatar, zero mutual
  servers, no badges, and an application with a one-word name all read as
  "fresh research bot" to anyone who inspects it. Use a dedicated,
  honestly-named application; never a bot that also serves production
  communities.
- **Invite interaction patterns.** Resolving an invite through the API
  endpoint is what the official client does — but opening dozens of
  invites per minute from one IP is not how humans browse. Space lookups;
  treat `--rate` as a floor, not a target.
- **Snowflake analysis by adversaries.** Everything you can decode about
  them, they can decode about you: your account age, your bot's age, your
  messages' timestamps. Assume every ID you expose is a timestamp leak.
- **Scraping footprints.** Bulk enumeration (sequential ID probing,
  widget polling loops) is trivially distinguishable from normal traffic
  and is exactly what Discord's anti-abuse systems flag. This tool never
  does it; don't script around it to make it do it.
- **Third-party telemetry.** Listing sites (disboard, top.gg) and log
  aggregators have their own analytics. Manual dork-driven research
  through a browser is a different footprint than API automation.

## Public vs. authenticated surfaces

| Surface | Auth | What it exposes | Notes |
|---------|------|-----------------|-------|
| `GET /invites/{code}` | none | Guild identity, approx counts, features, verification level | The exact call the client makes on invite open |
| `GET /guilds/{id}/widget.json` | none | Online members, channels, invite | Only when admins enabled the widget |
| `GET /webhooks/{id}/{token}` | none (URL is the credential) | Webhook name, guild/channel, creator | Metadata read only; never POST |
| `GET /users/{id}` | bot token | Public profile: username, badges, avatar/banner | Official bot surface; tied to your application |
| Messages, members list, presence, DMs | user/bot in-guild | — | Out of scope for this tool, forever |

## Rate-limit etiquette

- Default 1 request/second; honor `Retry-After` on 429 (the client does
  this automatically — do not lower timeouts to force retries).
- `invite --track` snapshots are one request per check. Hourly or daily
  tracking is meaningful; per-minute tracking of someone's community is a
  pattern Discord will notice and is not justified for authorized work.
- Batch inputs (stdin `-`) share the same throttle; large batches take
  real time by design.

## Operational guidance for authorized engagements

1. **Compartment.** Use a dedicated research application/bot and a
   dedicated machine or VM for engagement tooling. Never mix personal and
   engagement identities.
2. **Scope first.** Resolve only invites, guilds, webhooks and users that
   are in the written scope or clearly linked to it. Public ≠ in scope.
3. **Minimize.** Track the fewest snapshots that answer the engagement
   question. The tracking store (`~/.discord-recon/tracking.json`) is
   engagement-sensitive data: protect it, and purge it at closeout.
4. **Attribute honestly.** If a contact with the target is ever required,
   identify yourself and your authorization. This tool exists to make
   contact-free recon sufficient.
5. **Handle findings as credentials.** A live webhook URL is a working
   credential. Record it in the report, do not exercise it, and recommend
   rotation.
6. **Expect reversibility.** Assume the target can learn that their
   invite, widget, or profile was looked up, and that your research
   identity will be inspected in return. Behave so that inspection is
   boring.

## What this tool will never help you do

Join servers, read messages, enumerate members beyond the public widget,
validate tokens, automate user accounts, or post anything anywhere. If
your engagement needs those, the answer is a conversation with the
target's owner — not a workaround.
