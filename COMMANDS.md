# AVIS Commands

All output goes to **stdout** as JSON with `schema_version: "1"`. Errors go to **stderr** as JSON, then the process exits with a non-zero code.

**Exit codes:** 0 success · 1 operator error · 2 system error · 3 wait timeout

---

## `avis init`

Initialize the AVIS home directory and write `settings.json`.

```
avis init [--home <path>]
```

| Flag | Description |
|------|-------------|
| `--home <path>` | Override default home path (default: `~/.avis`) |

**Output:**
```json
{
  "schema_version": "1",
  "created": true,
  "home": "/home/user/.avis"
}
```

`created` is `false` if the directory already existed.

---

## `avis add id`

Add a new Gmail identity via OAuth2 PKCE flow. Opens a browser, waits for Google to redirect back, exchanges the code for a refresh token, and stores encrypted credentials on disk.

```
avis add id <name> <email>
```

| Argument | Description |
|----------|-------------|
| `name` | Short identifier for this identity (e.g. `ops`, `personal`) |
| `email` | Gmail address to authenticate |

Progress messages are written to **stderr** during the OAuth flow. On success:

**Output:**
```json
{
  "schema_version": "1",
  "identity": "ops",
  "email": "ops@example.com",
  "status": "ready"
}
```

---

## `avis ls`

List all identities stored in the AVIS home directory.

```
avis ls
avis list        # alias
```

**Output:**
```json
{
  "schema_version": "1",
  "identities": [
    { "name": "ops", "email": "ops@example.com" }
  ]
}
```

---

## `avis show`

Show configuration details for a single identity.

```
avis show <name>
```

| Argument | Description |
|----------|-------------|
| `name` | Identity name |

**Output:**
```json
{
  "schema_version": "1",
  "name": "ops",
  "email": "ops@example.com",
  "provider": "gmail",
  "status": "ready"
}
```

---

## `avis rm`

Remove an identity. Prompts for confirmation (`[y/N]`) on **stderr** before deleting.

```
avis rm <name>
avis remove <name>    # alias
avis delete <name>    # alias
```

| Argument | Description |
|----------|-------------|
| `name` | Identity name to delete |

**Output (confirmed):**
```json
{ "schema_version": "1", "deleted": true }
```

**Output (declined):**
```json
{ "schema_version": "1", "deleted": false }
```

---

## `avis send`

Send a plain-text email from the given identity via the Gmail REST API.

```
avis send <identity> -t <to> -s <subject> -b <body>
```

| Argument / Flag | Description |
|----------------|-------------|
| `identity` | Identity name to send from |
| `-t`, `--to` | Recipient email address |
| `-s`, `--subject` | Subject line |
| `-b`, `--body` | Message body (plain text) |

Retries up to 3 times on transient failures (delays: 1s, 2s, 4s).

**Output (success):**
```json
{
  "schema_version": "1",
  "sent": true,
  "from": "ops@example.com",
  "to": "user@example.com",
  "subject": "Hello",
  "message_id": "<abc123.ops.at.example.com@avis.local>",
  "ts": "2026-03-24T12:00:00Z"
}
```

**Output (failure):** exits with code 2.
```json
{
  "schema_version": "1",
  "sent": false,
  "error": "smtp_failure",
  "message": "HTTP 403: ..."
}
```

---

## `avis read`

Read inbox messages for the given identity via the Gmail REST API.

```
avis read <identity> [--latest] [-f <from>] [-s <subject>] [-n <count>] [--verbose]
```

| Argument / Flag | Description |
|----------------|-------------|
| `identity` | Identity name |
| `--latest` | Return only the single most recent matching message |
| `-f`, `--from` | Filter by sender (case-insensitive substring) |
| `-s`, `--subject` | Filter by subject (case-insensitive substring) |
| `-n`, `--count` | Number of messages to return (default: `10`) |
| `--verbose` | Reserved for future full-header output (currently a no-op) |

Body is stripped of quoted lines, `On … wrote:` lines, and trailing blank lines. Capped at 2000 characters (appends `...[truncated]` if exceeded).

**Output:**
```json
{
  "schema_version": "1",
  "messages": [
    {
      "id": "18f3a...",
      "from": "Sender Name <sender@example.com>",
      "subject": "Your OTP",
      "body": "Your code is 482910.",
      "ts": "2026-03-24T11:59:00Z"
    }
  ]
}
```

---

## `avis wait`

Poll the inbox until a matching new message arrives, then emit it and exit. Ignores messages already present when the command starts.

```
avis wait <identity> [-f <from>] [-s <subject>] [-t <seconds>]
```

| Argument / Flag | Description |
|----------------|-------------|
| `identity` | Identity name |
| `-f`, `--from` | Match on sender (case-insensitive substring) |
| `-s`, `--subject` | Match on subject (case-insensitive substring) |
| `-t`, `--timeout` | Seconds to wait before timing out (default: `60`) |

Polls every 1 second. Refreshes the access token each iteration.

**Output (match found):** same `EmailMessage` shape as a single `read` message (no `messages` array wrapper):
```json
{
  "schema_version": "1",
  "id": "18f3b...",
  "from": "noreply@service.com",
  "subject": "Verify your account",
  "body": "Your verification code is 7291.",
  "ts": "2026-03-24T12:01:05Z"
}
```

**Output (timeout):** exits with code 3.
```json
{
  "schema_version": "1",
  "matched": false,
  "timeout": 60
}
```

---

## `avis extract`

Extract OTP codes or URLs from an email. Defaults to the latest inbox message; use `--id` to target a specific one. Exactly one of the selector flags is required.

```
avis extract <identity> [--id <msg_id>] (--codes | --links | --first-code | --first-link)
```

| Argument / Flag | Description |
|----------------|-------------|
| `identity` | Identity name |
| `--id <msg_id>` | Target a specific message by Gmail message ID (default: latest inbox message) |
| `--codes` | Extract all numeric codes 4–8 digits long |
| `--links` | Extract all `http://` / `https://` URLs |
| `--first-code` | Extract only the first numeric code found |
| `--first-link` | Extract only the first URL found |

The four selector flags are mutually exclusive.

**Code extraction rules:** sequences of 4–8 consecutive ASCII digits that are not part of a longer number.

**Link extraction rules:** whitespace-delimited tokens starting with `http://` or `https://`; leading/trailing non-alphanumeric, non-`/`, non-`:` characters are stripped.

**Output:**
```json
{
  "schema_version": "1",
  "message_id": "18f3c...",
  "codes": ["482910"],
  "links": []
}
```

`codes` is empty when `--links` / `--first-link` is used, and `links` is empty when `--codes` / `--first-code` is used.
