# Lab Guide — Safe Test Values

## Keyless tier (safe, public)

```sh
# Large public, official Discord guild — stable test target
discord-recon invite discord.gg/discord-developers
discord-recon invite discord-developers          # bare code works too

# Discord's own documentation example snowflake (2015-08-10)
discord-recon snowflake 80351110224678912
printf '80351110224678912\n175928847299117063\n' | discord-recon snowflake -

# Orchestrator: resolves the invite, then attempts the guild widget
discord-recon full discord.gg/discord-developers
```

Expected: the invite resolves (DDevs is Discord's official developer
community). The widget lookup will typically report *50004 widget
disabled* — that is the normal case and exercises the graceful path.

## Webhook module

There is intentionally **no public test webhook** — any valid webhook URL
is a live credential. Test with a webhook you create on your own server
(Server Settings → Integrations → Webhooks → Copy Webhook URL), then:

```sh
discord-recon webhook https://discord.com/api/webhooks/<id>/<token>
```

Delete the test webhook afterwards. Never probe webhook URLs found in the
wild; treat them as credentials and report them.

## Bot-token tier

```sh
export DISCORD_RECON_BOT_TOKEN="your-own-bot-token"
discord-recon user 80351110224678912   # any known user ID
```

Without the variable, the module prints the tier explanation and exits
non-zero (covered by the test suite).

## What NOT to do

- Do not use a **user token** — selfbots violate Discord ToS and this tool
  rejects them by design.
- Do not join servers, send messages, or post to webhooks "to verify"
  findings — everything here is read-only.
- Do not run high-rate batch jobs: the 1 req/s default plus `Retry-After`
  handling is a design constraint, not a suggestion.
