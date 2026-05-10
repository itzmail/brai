# CLAUDE.md — Brai (Claude Code)

> **Shared instructions live in [`AGENTS.md`](./AGENTS.md).**
> This file contains only Claude Code-specific directives.

## Claude Code Settings

Claude Code should read and follow all instructions in `AGENTS.md` at the repository root for project conventions, commands, risk tiers, workflow rules, and anti-patterns.

## Project Identity

**Brai** is a personal AI agent forked from ZeroClaw, customized for a low-resource VPS (2 core, 2GB RAM).
It serves one user (Ismail Alam) via Telegram with three agent personas:

- **DevSecOps** — deploy apps, setup server, manage VPS
- **Personal Assistant** — morning brief 06:00 WIB, catat ide, AI news, cover letter + email
- **Developer** — trace bug, propose fix, konfirmasi ke user sebelum apply

Forked from: `https://github.com/zeroclaw-labs/zeroclaw`
Repository: `git@github.com:itzmail/brai.git`
Author: Ismail Alam

## Rebrand Status

Internal crates being renamed `zeroclaw-*` → `brai-*` progressively.
Binary: `brai` (was `zeroclaw`).
Do not introduce new `zeroclaw` references — use `brai` for all new code.

## Key Decisions

- Channel: Telegram only (phase 1)
- LLM: OpenRouter (configurable via env)
- Autonomy: `supervised` — semua destructive action wajib approval user
- Deployment: systemd service on VPS
- Storage: SQLite only (no embeddings — hemat RAM)
- Tools enabled: `shell`, `web_search`, `web_fetch`, `pdf_read`, `file_edit`, `file_write`, `email`, `memory_store`, `memory_recall`, `ask_user`, `git_operations`
- Tools disabled: hardware, WASM plugins, TUI, browser, desktop, Tauri

## Hooks

_No custom hooks defined yet._

## Slash Commands

_No custom slash commands defined yet._
