# WhatsApp Connectivity + Contact Approval Gate — Design

Status: approved by user, ready for implementation plan.

## Context

Brai currently talks to its one user (Ismail) over Telegram only, using a Meta
Cloud API–style token flow for any future WhatsApp Cloud API config
(`crates/zeroclaw-channels/src/whatsapp.rs`, feature `channel-whatsapp-cloud`).

A second, already-implemented WhatsApp path exists but is unconfigured:
`crates/zeroclaw-channels/src/whatsapp_web.rs` (feature `whatsapp-web`), built on
the `wa-rs` crate — a fork of `whatsapp-rust` (itself a Rust port of whatsmeow +
Baileys) maintained specifically for **stable Rust compatibility** (no nightly
toolchain requirement). Both features already compile into brai's default
build. Neither is configured in `dev/config.template.toml` or any live config.

This design covers two things, in one delivery:

1. **Turning on WhatsApp Web as a second chat channel for brai**, so the user
   can talk to brai over WhatsApp the same way they do over Telegram.
2. **A new "Contact Approval Gate"** — a channel-agnostic access-control layer
   so that when an unknown contact messages brai (on WhatsApp today, Telegram
   later), the message is held, the contact gets a "your message is under
   review" reply, and the *master* (the owner's own identity on that channel)
   gets asked to approve or deny. This is a new capability; nothing like it
   exists in the codebase today (the existing `request_approval` on the
   `Channel` trait is for *tool-call* approval, not *contact* approval — a
   different concern, reused for inspiration but not for implementation).

Out of scope for this spec (parked separately): a standalone kanban board app
with an MCP server and its own WhatsApp notifier — see
`docs/ideas-kanban-mcp-notifier.md`. That is a different project with
different recipients (arbitrary assignees, not a single owner) and is not
part of this design.

## Decisions (from brainstorming)

- WhatsApp mode: **WhatsApp Web** (`whatsapp_web.rs`, `wa-rs`), not Cloud API.
  No Meta Business token needed.
- Phone number: a **second number**, dedicated to brai (not the user's
  everyday number).
- Linking: **pair-code** (`pair_phone` config field), not QR — chosen because
  the user may not have physical access to the phone at setup time.
- Runs **alongside** Telegram — both channels active simultaneously, not a
  replacement.
- New capability: **Contact Approval Gate**, channel-agnostic by design (WA
  first, Telegram to follow later, reusing the same core).
- Gate is **on by default** (secure by default), with a **chat command** (master
  only) to toggle it off/on at runtime — not just a config file edit.
- Approval decisions are **plain-text replies** (`approve <id>` / `deny <id>`),
  because WhatsApp Web (unofficial protocol via `wa-rs`) has no reliable
  native button/inline-keyboard support. Telegram's future integration can
  layer inline buttons on top of the same core state machine later.
- Approved/denied contacts are **persisted** (SQLite, survives restart) —
  no re-asking on every process restart.
- Denied contacts are **permanently denied** (denylist) — messaging again does
  not re-trigger a pending request; message is silently dropped.
- Only the **master identity** (the owner's own number/account on that
  channel) can approve, deny, or **revoke** a previously approved contact.

## Architecture

### 1. WhatsApp Web channel activation

No new code needed for the channel itself — `whatsapp_web.rs` and its config
schema (`WhatsAppConfig` in `crates/zeroclaw-config/src/schema.rs:7718`) are
already complete. Activation is a config change:

```toml
[channels_config.whatsapp]
enabled = true
pair_phone = "62xxxxxxxxxx"   # brai's dedicated second number
session_path = "~/.brai/whatsapp-session.db"
mode = "personal"
self_chat_mode = true          # optional: always respond in Notes to Self
```

`session_path` being set (and `phone_number_id` absent) makes the orchestrator
select `WhatsAppWebChannel` over the Cloud API channel
(`orchestrator/mod.rs:4988` onward already implements this negotiation).

The existing `allowed_numbers` allowlist becomes the **fallback/manual**
allowlist; the Contact Approval Gate (below) is what actually populates it
dynamically at runtime once live.

### 2. Contact Approval Gate — core state machine

A new small module, channel-agnostic, e.g.
`crates/zeroclaw-channels/src/contact_gate.rs`, backed by a new SQLite table
(new file, e.g. `~/.brai/contact_gate.db`, opened the same way
`SqliteSessionBackend` opens its DB — `Connection::open` + `CREATE TABLE IF
NOT EXISTS` in the constructor, matching the existing pattern in
`crates/zeroclaw-infra/src/session_sqlite.rs`).

**Schema:**

```sql
CREATE TABLE IF NOT EXISTS contact_gate (
    channel      TEXT NOT NULL,   -- e.g. "whatsapp", "telegram"
    identity     TEXT NOT NULL,   -- normalized phone/chat id
    status       TEXT NOT NULL,   -- 'pending' | 'approved' | 'denied'
    requested_at INTEGER NOT NULL,
    decided_at   INTEGER,
    PRIMARY KEY (channel, identity)
);
```

**Per-channel config additions** (new fields on `WhatsAppConfig`, mirrored
later on the Telegram config):

```toml
[channels_config.whatsapp]
contact_gate_enabled = true      # default true; toggled at runtime via command
master_identity = "62xxxxxxxxxx" # the owner's own number on this channel
```

`master_identity` identifies who receives approval prompts and who is allowed
to run gate commands. It is per-channel by design (decision: approval on one
channel does not imply trust on another).

**Message flow (incoming, WhatsApp):**

```
inbound message from `identity`
  |
  +-- identity == master_identity? -> process normally (never gated)
  |
  +-- contact_gate_enabled == false? -> process normally (gate off)
  |
  +-- lookup (channel, identity) in contact_gate:
        - "approved" -> process normally
        - "denied"   -> silently drop, no reply, no re-prompt
        - "pending"  -> silently drop (already awaiting decision), no re-prompt
        - not found  -> NEW CONTACT:
              1. insert row status='pending'
              2. reply to `identity`: "Your message is under review."
              3. send to `master_identity`: "<identity> wants to chat: '<preview>' — reply `approve <identity>` or `deny <identity>`"
              (the original message content is not queued/replayed on approval;
               the contact simply sends again once approved — simplest option,
               avoids a message replay queue for a rare first-contact case)
```

**Master commands** (parsed from messages sent by `master_identity` only):

- `approve <identity>` — set status='approved', decided_at=now. Future
  messages from that identity process normally.
- `deny <identity>` — set status='denied', decided_at=now. Future messages
  from that identity are dropped silently, forever (until revoked).
- `revoke <identity>` — delete the row entirely; identity returns to unknown
  state (next message re-triggers the pending flow).
- `gate on` / `gate off` — sets `contact_gate_enabled` at runtime (persisted
  back to config, same mechanism brai already uses elsewhere for runtime
  config writes — see `add_allowed_identity_runtime` /
  `persist ... to config.toml` pattern in `telegram.rs:791`).

These are handled the same way Telegram's existing `/bind`,`/new`,`/stop`
etc. are handled: intercepted in the channel's message-receive path before
anything reaches the LLM/agent loop.

### 3. Channel-agnostic core, WhatsApp-specific wiring

The state machine (SQLite table + approve/deny/revoke/gate-toggle logic) lives
in one shared module so Telegram can adopt it later without duplicating logic.
What's channel-specific:

- How the "under review" reply and master notification are actually sent
  (`Channel::send`, already implemented per channel).
- How master commands are recognized in transport-specific message text (plain
  text parsing for WhatsApp; Telegram can later use inline keyboard callbacks
  instead of `approve <id>` text, mapped to the same core `approve()` call).

### 4. Error handling / edge cases

- If `master_identity` is unset while `contact_gate_enabled = true`: fail
  fast at channel startup with a clear config error (gate has no one to ask;
  do not silently disable it and do not silently allow all).
- Master's own messages never go through the gate (checked first, before any
  DB lookup).
- Gate DB is opened once at channel construction, same lifetime as the
  channel — no per-message file open/close.

## Testing

- One `#[tokio::test]`-level self-check for the state machine: pending ->
  approve -> subsequent message passes; pending -> deny -> subsequent message
  dropped and no re-prompt; revoke -> back to pending flow on next message.
  No mock framework beyond what channel tests already use in this repo
  (`tests/support/mock_channel.rs`).
- Manual verification against a real WhatsApp Web pairing (pair-code flow)
  before calling this done, per user's own testing preference — this is a
  live external protocol integration, not something a unit test can fully
  cover.

## Non-goals (explicitly deferred)

- Telegram inline-button approval UI — same core, different transport,
  separate follow-up once WhatsApp is live and stable.
- Cross-channel identity linking (approving a contact on WhatsApp does **not**
  approve the "same person" on Telegram) — decision (a) from brainstorming.
- Replaying the contact's original held message automatically after approval
  — contact re-sends manually.
- The kanban/MCP/notifier idea — separate project, tracked in
  `docs/ideas-kanban-mcp-notifier.md`.
