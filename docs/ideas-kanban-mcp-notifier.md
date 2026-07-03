# Idea: Kanban Board App with MCP + WhatsApp Notifier

Status: **parked idea, not yet designed**. Captured from a `feat-connect-wa` brainstorm on brai — turned out to be a separate project, not a brai feature.

## Origin

Started as "add WhatsApp connectivity to brai". Digging into the actual need revealed
a bigger, separate use case: a kanban board split per division, where a lead assigns
tasks to team members, and the assignee gets notified (via WhatsApp) to work on it.

That doesn't fit brai's scope (personal 1-user assistant on a 2GB VPS, Telegram-only).
It's its own product.

## Concept

- **Standalone kanban board app** — boards split per division, tasks, leads, assignees.
- **MCP server** exposed by the app (e.g. `create_task`, `assign_task`, `notify_lead`,
  `list_board`) so any AI agent (brai, Claude Desktop, others) can drive it as an MCP client.
- **WhatsApp notifier lives inside this app**, not in brai. The app owns the event
  ("task assigned", "task due") and decides when to push a WA notification to the
  assignee's number. Brai (or another agent) just calls MCP tools; it doesn't send WA
  messages itself for this use case.
- Needs its own contact/user mapping: who is in which division, whose WA number is what,
  who is lead vs member. Brai's `allowed_numbers` (single-user allowlist) doesn't fit —
  this is multi-recipient, arbitrary-assignee outbound send.

## Open questions (not yet brainstormed)

- Stack / where it's hosted (own VPS? same VPS as brai?)
- Data model: board, division, task, user, lead relationship
- Notification triggers: on assign only? on due-soon? on overdue? on status change?
- WA sending mode for this app: Cloud API (token, official) vs WhatsApp Web (QR, unofficial) —
  same tradeoff brai faced, decide independently for this app
- Auth/permissions for the MCP server (who can call `assign_task` etc.)
- UI: does it need a web dashboard, or is it MCP/agent-driven only?

## Next step

Brainstorm this as its own project (own repo/spec) when ready — not inside brai's
`docs/superpowers/specs/`.

---

Separately, and unrelated to this: brai itself still gets a direct WhatsApp channel
(for the user to chat with brai over WA, like Telegram) — that's the next thing to
design, using brai's existing (already-coded, unconfigured) WhatsApp Web channel
(`crates/zeroclaw-channels/src/whatsapp_web.rs`, QR/pair-code login, no Meta token).
