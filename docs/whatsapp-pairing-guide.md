# WhatsApp Pairing & Contact Gate — Operator Guide

How to connect brai's dedicated WhatsApp number and manage the Contact
Approval Gate afterward. This is a usage guide, not a design doc — see
`docs/superpowers/specs/2026-07-03-whatsapp-connectivity-design.md` and
`docs/superpowers/specs/2026-07-03-whatsapp-onboard-pairing-design.md` for
the design rationale.

## 1. Prerequisites

- A second phone number dedicated to brai (not your personal number) — a
  physical SIM or a virtual number that can receive a pairing code.
- Your personal WhatsApp number, which becomes the **master identity** (the
  only number allowed to approve/deny contacts and toggle the gate).

## 2. Run onboard

```bash
brai onboard
```

When the wizard reaches channel setup, add **WhatsApp** and fill in:

- **`pair_phone`** — the dedicated number, international format (e.g. `+62...`)
- **`master_identity`** — your personal WhatsApp number (owner/approver)
- `session_path` — default is usually fine

## 3. Live pairing step

Right after the config is saved, `brai onboard` connects once to WhatsApp
Web and prints the pair code directly in the terminal:

```
WhatsApp pair code: ABCD-1234

On the dedicated WhatsApp number's phone: Settings → Linked Devices
→ Link a Device → Link with phone number instead → enter this code.
```

The code is valid ~60 seconds; connecting after entering it has its own
60-second window.

On the dedicated number's phone: **Settings → Linked Devices → Link a
Device → Link with phone number instead** → enter the code.

## 4. Confirmation

On success:

```
WhatsApp connected successfully.
Restart the brai service now (e.g. `systemctl restart brai`) to start using WhatsApp.
```

Run `systemctl restart brai` on the VPS.

On failure or timeout, the wizard asks "Try WhatsApp pairing again?" — yes
retries immediately; no means retry later via a manual service restart.

## 5. Contact Approval Gate

Enabled by default (`contact_gate_enabled = true`). When a number other
than `master_identity` messages the dedicated WhatsApp number for the
first time, its messages are held as **pending** — not processed — until
the master identity approves or denies it. These commands only work from
a **DM to brai from the master identity's number** (group chats are
ignored, so gate administration can't leak into a group).

| Command | Effect |
|---|---|
| `approve <id>` | Allow that contact; their messages are now processed |
| `deny <id>` | Reject that contact; future messages are dropped silently |
| `revoke <id>` | Remove a previously approved/denied contact, back to no record (next message becomes pending again) |
| `gate on` | Re-enable the Contact Approval Gate |
| `gate off` | Disable the gate — all senders are processed unfiltered |

Commands are case-insensitive. `<id>` is the sender's identity as shown in
the pending notification (typically their phone number).

`gate on`/`gate off` persists across restarts (written back to config).
