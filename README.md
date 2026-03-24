# AVIS

A stateless, multi-identity email operations layer for AI agents — built on Gmail, not around it.

## Why

AI agents need email. Current solutions give agents disposable platform-owned addresses. AVIS takes the opposite approach: agents operate as **existing identities** (`support@acme.com`, your personal Gmail) via OAuth2, inheriting their trust and deliverability. No mail is stored locally. No AVIS server exists. Credentials never leave your machine.

## Install

Requires Rust 1.75+.

```bash
git clone https://github.com/yourorg/avis.git
cd avis
cargo build --release
# Binary at target/release/avis (or avis.exe on Windows)
```

## Quickstart

```bash
# 1. Add an identity (opens browser for OAuth2 — creates ~/.avis automatically)
avis add id ops

# 2. Send an email
avis send ops -t recipient@example.com -s "Hello" -b "Message body"

# 3. Read latest messages
avis read ops --latest -n 5

# 4. Wait for a specific email (poll until it arrives or timeout)
avis wait ops -f service@example.com -s "Verification" -t 60

# 5. Extract OTP code from the latest matching email
avis extract ops --first-code
# → {"codes":["482910"]}
```

All output is flat JSON, designed for machine consumption.

## Commands

| Command | Description |
|---------|-------------|
| `avis init` | Create `~/.avis` directory structure |
| `avis add id <name>` | Add identity via OAuth2 PKCE (email fetched automatically) |
| `avis ls` | List all identities |
| `avis show <name>` | Show identity details |
| `avis rm <name>` | Remove an identity |
| `avis send <name> -t <to> -s <subj> -b <body>` | Send email |
| `avis read <name> [--latest] [-f from] [-s subj] [-n count]` | Read inbox |
| `avis wait <name> [-f from] [-s subj] [-t seconds]` | Poll for matching email |
| `avis extract <name> [--first-code\|--codes\|--first-link\|--links]` | Extract OTPs or links |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Operator error (bad args, missing identity) |
| 2 | System error (network, auth failure) |
| 3 | Wait timeout |

## How It Works

AVIS authenticates against Gmail via OAuth2 PKCE and uses the Gmail REST API for all operations. No IMAP, no SMTP. Refresh tokens are encrypted with ChaCha20-Poly1305 and stored locally in `~/.avis/identities/<name>/credentials.enc`. There is no server component.

## Limitations (v1)

- Gmail only (Outlook/Microsoft planned for v2)
- Requires Google OAuth2 client credentials

## License

MIT
