# AVIS — CLAUDE.md
> Read this fully. Every session. No exceptions.
> When corrected: fix the code AND add a rule here. Say "Updated CLAUDE.md."

## What This Is
Rust CLI — multi-identity Gmail ops for AI agents. All output is JSON. No mail stored locally.
Binary: `cargo build --release` → `target/release/avis.exe`

## Stack (exact versions matter)
Rust 1.94 · Tokio · Clap 4 · Reqwest 0.13 · ChaCha20-Poly1305 · serde_json
**Gmail REST API for send + read** (not IMAP/SMTP — lettre/imap in Cargo.toml but unused)

## Structure
```
src/
  main.rs          # dispatch only
  cli.rs           # all clap definitions
  config.rs        # home resolution, identity paths
  crypto.rs        # encrypt/decrypt credentials
  errors.rs        # AvisError → always JSON stderr
  output.rs        # print_json() → ONLY stdout writer
  auth/pkce.rs     # PKCE challenge
  auth/refresh.rs  # token exchange + refresh (uses serde_urlencoded, not .form())
  commands/        # one file per command
```

## Rules (never violate)
- `output::print_json()` — ONLY way to write stdout
- `AvisError::bail(code)` — ONLY way to exit on error. Never panic, never unwrap in prod
- All commands: `pub async fn run(home: &Path, ...)` no return value
- `load_credentials()` lives in `send.rs` — import from there in read/wait/extract
- `fetch_message()` lives in `read.rs` — import from there in wait/extract
- Shared types between commands need `pub(crate)` — EmailMessage fields learned this hard way
- reqwest `.query()` broken in this setup — build URLs manually with format!()
- serde_urlencoded not `.form()` for POST bodies — same compatibility issue

## Workflow (Boris Cherny method)
1. **Plan first** — for any task >2 files: write plan as comments before coding
2. **Verify with evidence** — run the binary, show actual output. Never claim it works without running it
3. **Test loop**: `cargo build` → `cargo fmt --check` → `cargo clippy -- -D warnings` → smoke test
4. **One task at a time** — finish and verify before moving on
5. **After every correction** — update this file

## Karpathy Rules
- Stuck after 3 attempts? Add `// TODO: STUCK` and surface it. Don't silently fail
- Don't touch code outside your task scope — no "while I'm here" refactors
- Simplest solution wins. New abstraction = justify it first

## Known Quirks
- `imap` + `lettre` in Cargo.toml — NOT used. Gmail REST API only
- `format_ts`/`days_to_ymd` duplicated in send.rs + read.rs — known tech debt
- `rand = "0.8"` pinned — chacha20poly1305 compatibility, do not upgrade
- CLIENT_ID/CLIENT_SECRET hardcoded in identity.rs — never commit real values

## Commands (shorthand)
```
avis init
avis add id <name> <email>    # OAuth2 PKCE, opens browser
avis ls / show <n> / rm <n>
avis send <n> -t <to> -s <subject> -b <body>
avis read <n> [--latest] [-f <from>] [-s <subject>] [-n <count>]
avis wait <n> [-f <from>] [-s <subject>] [-t <seconds>]
avis extract <n> [--first-code|--codes|--first-link|--links]
```

## Exit Codes
0 success · 1 operator error · 2 system error · 3 wait timeout

## Adding a Command
1. Add variant to `Command` enum in cli.rs
2. Create `src/commands/<name>.rs` with `pub async fn run(...)`
3. Register in `commands/mod.rs`
4. Add match arm in `main.rs`

## Corrections Log
> Add entries here after every mistake. This file improves over time.
- reqwest .query() and .form() don't work — use manual URL building + serde_urlencoded
- EmailMessage and shared types need pub(crate) when used across command modules
- base64 decode needs padding normalization — see decode_base64url() in read.rs
