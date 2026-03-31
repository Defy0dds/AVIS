# AVIS — Thesis

## The Problem

AI agents need email. Every agent that signs up for a service, receives a verification code, manages a support inbox, or communicates on behalf of a business needs to send and receive email.

The current solutions are wrong in the same direction: they route your email through third-party infrastructure you don't control.

**AgentMail** is an API-first email platform (YC S25, $6M seed from General Catalyst) that provisions and manages inboxes for AI agents — including custom domain support. Their infrastructure handles the email routing, storage, and delivery. The issue isn't their addresses — it's that your credentials and email data flow through their servers. One platform compromise and every customer's agent email is exposed.

**gws CLI** (Google's own tool, 21k GitHub stars, released March 2026) supports exactly one authenticated Google account per machine. Agents that need to operate across multiple identities have no supported path.

Both miss the same thing: **your credentials shouldn't leave your machine.**

---

## The Insight

Agents operating as your business identity should not require you to hand your credentials to a third-party platform.

A company's `support@acme.com`. A founder's personal Gmail. A recruiter's Outlook. These are already trusted. They already have deliverability reputation. They already run on Google's infrastructure — 20 years of uptime, SPF/DKIM/DMARC, planetary scale.

AVIS doesn't provision email infrastructure. It doesn't route mail through its own servers. It delegates to Google directly — no middleman infrastructure. If Google's SMTP is up, AVIS works. If Google goes down, you have bigger problems than AVIS.

---

## What AVIS Is

A stateless, multi-identity email operations layer for AI agents. `Identity delegation for autonomous agents`

```bash
# Agent signs up for a service, waits for OTP, extracts it
avis send ops -t service@example.com -s "Register" -b "Please verify"
avis wait ops -f service@example.com -t 60
avis extract ops --first-code
# → {"codes":["482910"]}
```

Three commands. Any number of identities. Zero mail stored locally. Credentials never leave the machine.

---

## Differentiation

| | AgentMail | gws CLI | AVIS |
|---|---|---|---|
| Infrastructure | AgentMail's cloud | Gmail | Gmail direct (no middleman) |
| Credential custody | Their servers | Your machine | Your machine (ChaCha20-encrypted) |
| Custom domains | Yes (via their DNS/infra) | N/A | Yes (via your Gmail/Workspace) |
| Single point of failure | AgentMail infra + Google | Google | Google only |
| Multi-identity | API-managed | ❌ one account | ✅ unlimited per machine |
| Agent primitives (wait/extract) | ❌ | ❌ | ✅ built-in |
| MCP integration | ✅ | ❌ | On roadmap |
| SDKs | Python, TypeScript, Go | N/A | CLI-only (SDKs planned) |
| Output | REST API (JSON) | Verbose JSON | Minimal flat JSON for agents |
| Compliance | SOC 2 Type II | N/A | Local-only (nothing to audit) |

---

## Why Local-First

"Local-first" isn't a technical constraint — it's a trust model.

Your refresh token is encrypted with ChaCha20-Poly1305 and stored in `~/.avis/identities/<n>/credentials.enc`. The encryption key is `master.key` in the same directory. Neither file ever leaves your machine. AVIS has no server. There is no AVIS cloud. There is nothing to breach.

This matters for:
- Developers who don't want to trust a startup with access to their Gmail
- Companies with data residency requirements
- Agents acting as real humans with real reputations on the line

---

## The Technical Bet

Gmail REST API + OAuth2 PKCE, not IMAP/SMTP.

IMAP/SMTP with OAuth2 is theoretically correct but practically broken in most Rust libraries (lettre's XOAUTH2 mechanism fails in practice). Gmail's REST API is stable, well-documented, and handles all the auth complexity cleanly. The tradeoff is Gmail-only for v1 — which is fine, because Gmail has 1.8 billion users and is where most business email lives.

Outlook/Microsoft is v2. The architecture is provider-agnostic by design.

---

## The Market

AgentMail raised $6M from General Catalyst in March 2026 with 500+ B2B customers and 100M+ emails delivered. They validated the thesis: agents need email infrastructure.

AgentMail built a managed platform — they own the infrastructure your agents email through. AVIS takes the opposite architectural bet: zero infrastructure, zero custody. Your credentials stay on your machine, your email flows through Google directly, and there is no AVIS server to breach. These are different trust models serving different customer segments. AgentMail is right for teams that want managed infrastructure. AVIS is right for teams that won't hand their email credentials to a third party — regulated industries, privacy-conscious developers, companies with data residency requirements.

gws CLI is Google's own answer to Workspace automation — but it's a general-purpose tool for humans with a one-account ceiling. It's not built for agents, doesn't handle multiple identities, and has no wait/extract primitives.

---

## v1 Scope

- Gmail only
- Send, read, wait, extract
- Multi-identity on one machine
- OAuth2 PKCE authentication
- Stateless — no mail cached locally
- Machine-readable JSON output
- Windows + Linux + macOS

## Future

- v2: Microsoft/Outlook provider (same architecture, different OAuth endpoints)
- Provider trait abstraction inside one codebase — no fork
- ~~Attachment support~~ ✓ (send with `-a`, read shows metadata, `download` command)
- MCP server wrapper so any MCP-compatible agent can use AVIS directly
