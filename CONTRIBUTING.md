# Contributing to discord-recon

Thanks for your interest. This project is a security research tool;
contributions must preserve its **read-only** scope and ethical framing.

## Ground rules

- **Read-only only.** No selfbots, no user-token automation, no token
  checking, no message scraping, no mass actions, no posting to webhooks.
  PRs adding any of these will be rejected.
- **Public/authorized surfaces only.** New modules must use keyless public
  endpoints or official bot-token endpoints — documented per module.
- **Ethics framing stays prominent.** Keep the disclaimer and banner
  language intact.

## Development workflow

1. Fork and create a feature branch.
2. Idiomatic Rust with doc comments on public items.
3. Every external call: timeout + graceful failure (warn & continue, never
   panic on network errors; decode Discord error codes where known).
4. Verify before submitting:
   ```sh
   cargo fmt --check
   cargo clippy --all-targets -- -D warnings
   cargo test --release
   cargo build --release
   ```
5. Open a PR describing the module, its API surface, and its ATT&CK mapping.

## Adding a module

- Create `src/modules/<name>.rs` returning `ModuleOutput`; register in
  `src/modules/mod.rs` and wire a subcommand in `src/main.rs`.
- Respect the shared `HttpClient` (rate limit, 429 handling) — never bypass.
- Add network-free unit tests for parsing/validation logic.
- Document sources, output fields and limitations in `docs/modules.md`.
