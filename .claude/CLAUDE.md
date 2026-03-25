# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

> Read this fully. Every session. No exceptions.
> When corrected: fix the code AND add a rule here. Say "Updated CLAUDE.md."

## What This Is
Rust CLI — multi-identity Gmail ops for AI agents. All output is JSON. No mail stored locally.
See `THESIS.md` for product rationale and competitive positioning.

## Build & Verify
```bash
cargo build --release                    # binary → target/release/avis.exe
cargo fmt --check                        # formatting
cargo clippy -- -D warnings              # lints (treat warnings as errors)
```
No test suite exists yet. Verify changes by building + running the binary with real output.
Full check cycle: `cargo build && cargo fmt --check && cargo clippy -- -D warnings`

## Stack (exact versions matter)
Rust 1.94 · Tokio · Clap 4 · Reqwest 0.13 · ChaCha20-Poly1305 · serde_json
**Gmail REST API for send + read** (not IMAP/SMTP — lettre/imap in Cargo.toml but unused)

## Architecture

### Data flow (every command that touches Gmail)
```
load_credentials(home, identity)     # send.rs — decrypt OAuth creds from disk
  → crypto::load_master_key()        # read master.key
  → crypto::decrypt()                # ChaCha20-Poly1305 decrypt credentials.enc
  → OAuthCredentials { refresh_token, client_id, client_secret }

refresh::get_access_token(&creds)    # auth/refresh.rs — exchange refresh → access token
  → POST https://oauth2.googleapis.com/token (serde_urlencoded body, NOT .form())

Gmail REST API calls                 # per-command logic
  → build URL with format!(), NOT .query()
  → bearer_auth(&token.access_token)
```

### Output contract
- stdout: `output::print_json()` — ONLY way to write. All responses include `schema_version: "1"`
- stderr: `AvisError::bail(code)` — ONLY way to exit on error. JSON error to stderr, then exit

### On-disk identity layout
```
~/.avis/identities/<name>/
  config.json       # IdentityConfig { name, email, provider }
  master.key        # 32-byte ChaCha20 key
  credentials.enc   # [12-byte nonce][encrypted OAuthCredentials]
```

### Module structure
```
src/
  main.rs          # dispatch only — match cli.command → commands::*
  cli.rs           # all Clap definitions (Cli, Command enums)
  config.rs        # home resolution, identity paths, IdentityConfig
  crypto.rs        # encrypt/decrypt credentials (ChaCha20-Poly1305)
  errors.rs        # AvisError with named constructors
  output.rs        # print_json() + SCHEMA_VERSION
  auth/pkce.rs     # PKCE challenge generation
  auth/refresh.rs  # token exchange + refresh (OAuthCredentials, AccessToken)
  commands/        # one file per command (incl. download.rs for attachments)
```

## Rules (never violate)
- `output::print_json()` — ONLY way to write stdout
- `AvisError::bail(code)` — ONLY way to exit on error. Never panic, never unwrap in prod
- All commands: `pub async fn run(home: &Path, ...)` no return value
- `load_credentials()` lives in `send.rs` — import from there in read/wait/extract
- `fetch_message()` lives in `read.rs` — import from there in wait/extract
- Shared types between commands need `pub(crate)` — EmailMessage fields learned this hard way
- reqwest `.query()` broken in this setup — build URLs manually with `format!()`
- `serde_urlencoded` not `.form()` for POST bodies — same compatibility issue
- base64 decode needs padding normalization — see `decode_base64url()` in read.rs

## Workflow
1. **Plan before any new feature** — before writing any code, explain in plain English: the end-to-end flow, which files will be touched and why, and what the expected behavior looks like from the user's perspective. Wait for explicit approval before writing any code.
2. **Plan first** — for any task >2 files: write plan as comments before coding
3. **Verify with evidence** — run the binary, show actual output. Never claim it works without running it
4. **Test loop**: `cargo build` → `cargo fmt --check` → `cargo clippy -- -D warnings` → smoke test
5. **One task at a time** — finish and verify before moving on
6. **After every correction** — update this file
7. Stuck after 3 attempts? Add `// TODO: STUCK` and surface it. Don't silently fail
8. Don't touch code outside your task scope — no "while I'm here" refactors
9. Simplest solution wins. New abstraction = justify it first

## Build-time environment variables
`AVIS_CLIENT_ID` and `AVIS_CLIENT_SECRET` must be set as environment variables
before building. The build will fail with a clear error if they are missing.

```bash
AVIS_CLIENT_ID=<your-client-id> AVIS_CLIENT_SECRET=<your-client-secret> cargo build --release
```

In GitHub Actions these are injected from repository secrets — never commit
real credential values to the repo.

## Version Tagging
`Cargo.toml` version must match the git tag before pushing a release. Current version: `0.1.0` (matches tag `v0.1.0`). Before tagging a new release:
1. Update `version` in `Cargo.toml`
2. Commit the change
3. Push the tag: `git tag v<version> && git push origin v<version>`

Never push a version tag without first updating `Cargo.toml` to match.

## Known Quirks
- `imap` + `lettre` + `native-tls` removed from Cargo.toml — Gmail REST API only
- `format_ts`/`days_to_ymd` duplicated in send.rs + read.rs — known tech debt
- `rand = "0.8"` pinned — chacha20poly1305 compatibility, do not upgrade

## Commands
```
avis add <name>               # OAuth2 PKCE, opens browser; email fetched from Google; auto-inits ~/.avis
avis ls / show <n> / rm <n>
avis send <n> -t <to> -s <subject> -b <body> [-a <file>]...
avis read <n> [--latest] [-f <from>] [-s <subject>] [-n <count>] [--verbose] [--download-dir <path>]
avis wait <n> [-f <from>] [-s <subject>] [-t <seconds>] [--download-dir <path>]
avis extract <n> [--first-code|--codes|--first-link|--links] [--id <msg_id>]
avis download <n> [--id <msg_id>] [-d <dir>]
```

**Agent extract pattern** — always pass `--id` from a prior `read`/`wait` result:
```bash
avis wait <identity> -f <sender> -t 60   # capture .id from output
avis extract <identity> --first-code --id <id>
```
Relying on the default latest message is a race condition: a newer unrelated email may arrive between `wait` and `extract`.

## Exit Codes
0 success · 1 operator error · 2 system error · 3 wait timeout

## Adding a Command
1. Add variant to `Command` enum in `cli.rs`
2. Create `src/commands/<name>.rs` with `pub async fn run(home: &Path, ...)`
3. Register in `commands/mod.rs`
4. Add match arm in `main.rs`

## Corrections Log
> Add entries here after every mistake. This file improves over time.
- `avis add id` no longer takes an `email` arg — email is fetched from `GET /gmail/v1/users/me/profile` after OAuth. Update any references to the old `<name> <email>` signature.
- `avis add id <name>` collapsed to `avis add <name>` — `AddTarget` enum removed. No subcommand between `add` and the name arg.
- `avis init` removed — `avis add` auto-initializes `~/.avis`. Do not reference or re-add `init`.
- `load_credentials` now returns `AvisError::identity_not_found` early if the identity directory is missing, instead of a raw `fs_error` about `master.key`. This covers all commands that call `load_credentials` (read, wait, extract, download, send).
- Cargo.toml was at `1.0.0` while the released git tag was `v0.1.0` — corrected to `0.1.0`. Always keep these in sync.
