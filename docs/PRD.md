# PRD — Brai: Personal AI Agent

**Owner:** Ismail Alam  
**Repo:** git@github.com:itzmail/brai.git  
**Base:** ZeroClaw v0.7.5  
**Target:** VPS 2 core, 2GB RAM (Ubuntu)  
**Channel:** Telegram (phase 1)  
**Status:** In development

---

## 1. Problem

Ismail butuh asisten yang bisa:
1. Bantu kelola VPS dan deploy aplikasi tanpa harus ingat semua command
2. Berikan brief harian dan informasi AI terbaru setiap pagi
3. Trace bug di aplikasi dan propose fix — tapi tidak bertindak sendiri tanpa konfirmasi

Tidak ada tool yang ringan, fully Rust, bisa jalan di VPS kecil dengan semua kapabilitas ini sekaligus.

---

## 2. Goals

| # | Goal | Metric |
|---|------|--------|
| G1 | Agent jalan stabil di VPS 2GB | RAM idle < 50MB |
| G2 | User bisa interaksi via Telegram | Response < 5 detik untuk chat biasa |
| G3 | Semua destructive action butuh konfirmasi | Zero unconfirmed deploy/fix |
| G4 | Morning brief terkirim tepat waktu | 06:00 WIB ±1 menit |
| G5 | Cover letter bisa di-generate dari CV | End-to-end dalam 1 conversation |

---

## 3. Non-Goals (Phase 1)

- Multi-user / multi-tenant
- Channel lain (Discord, Slack, WhatsApp) — phase 2
- Hardware integration (GPIO, STM32)
- WASM plugin system
- Browser automation
- Desktop / Tauri app
- Voice call / TTS
- Embeddings / vector search

---

## 4. Agent Personas

### 4.1 DevSecOps Agent

**Trigger:** User chat soal server, deploy, setup, monitoring

**Kapabilitas:**
- Setup server: nginx, SSL (certbot), systemd service
- Deploy aplikasi: git pull, build, restart service
- Cek status: disk, memory, CPU, logs
- Troubleshoot: cek error di log, restart service

**Workflow deploy (contoh):**
```
User: "Deploy aplikasi api-gateway ke VPS"
Agent: cek git status → build → 
       "Siap deploy. Akan restart service api-gateway. Lanjut?" 
User: "Ya"
Agent: deploy → kirim hasil (sukses/gagal + log)
```

**Security:**
- Autonomy: `supervised`
- Workspace: `/home/ismail/apps/` (tidak bisa akses di luar)
- Semua shell command dengan risiko tinggi (restart, delete, chmod) → wajib konfirmasi

---

### 4.2 Personal Assistant

**Trigger:** Chat umum, ide, todo, email, CV

**Kapabilitas:**

**Morning Brief (cron 06:00 WIB):**
```
- Rangkuman todo/catatan pending dari memory
- Top 3 berita AI terbaru (web search)
- Saran kegiatan hari ini
```

**Catat ide/kegiatan:**
```
User: "Catat: ide bikin SaaS untuk manajemen kost"
Agent: simpan ke memory → konfirmasi tersimpan
```

**Cover Letter + Email:**
```
User: upload CV.pdf ke Telegram
Agent: baca CV → simpan ke /storage/cvs/

User: "Buat cover letter untuk posisi Backend Engineer di Tokopedia"
Agent: baca CV → generate cover letter →
       "Draft selesai: [preview]. Kirim ke recruitment@tokopedia.com?" 
User: "Ya"
Agent: kirim via SMTP
```

**Catatan Penting:**
- Draft email selalu ditampilkan dulu sebelum kirim
- User bisa edit draft: "Ubah paragraf 2 jadi lebih formal"

---

### 4.3 Developer Agent

**Trigger:** User report bug, paste error log, minta code review

**Kapabilitas:**
- Baca error log dari file atau paste user
- Trace root cause (baca source code + git history)
- Propose fix dengan penjelasan
- Apply fix setelah konfirmasi
- Commit dengan pesan deskriptif

**Workflow bug fix:**
```
User: paste error log / "ada bug di /apps/api/src/auth.rs"
Agent: 
  1. baca file + error context
  2. analisa root cause
  3. kirim ke Telegram:
     "Bug ditemukan: [deskripsi]
      Root cause: [penjelasan]
      Proposed fix: [diff/penjelasan]
      Apply fix? [Ya/Tidak/Lihat diff]"
  4. TUNGGU reply user (non-blocking — agent lain tetap jalan)
  5. Kalau Ya → apply → commit → konfirmasi selesai
  6. Kalau Tidak → catat di memory, tidak ada action
```

**Non-blocking:** Approval gate tidak memblokir agent lain. User bisa reply jam berapa saja.

---

## 5. Technical Architecture

```
VPS (2 core, ~1.5GB free RAM)
│
└── brai daemon (systemd)
    ├── Telegram channel (teloxide)
    ├── Cron engine
    │   └── 06:00 WIB → morning_brief SOP
    ├── SOP engine
    │   ├── deploy-flow
    │   ├── cover-letter-flow  
    │   ├── bug-fix-flow (resumable + approval gate)
    │   └── morning-brief-flow
    ├── Tools
    │   ├── shell (supervised)
    │   ├── web_search + web_fetch
    │   ├── pdf_read
    │   ├── file_edit + file_write
    │   ├── email (SMTP)
    │   ├── memory_store + memory_recall
    │   ├── ask_user (approval gate)
    │   └── git_operations
    └── Storage
        ├── SQLite (~/.brai/data/brai.db)
        └── File storage (/home/ismail/brai-storage/)
```

---

## 6. Config (Target)

```toml
# ~/.brai/config.toml

[agent]
name = "Brai"
autonomy = "supervised"
workspace_dir = "/home/ismail/apps"

[providers.models.default]
provider = "openrouter"
model = "anthropic/claude-sonnet-4-6"

[channels.telegram]
bot_token_env = "TELEGRAM_BOT_TOKEN"
allowed_users = ["ismail_alam"]  # whitelist — hanya owner

[memory]
backend = "sqlite"
path = "~/.brai/data/brai.db"

[storage]
file_dir = "/home/ismail/brai-storage"

[email]
smtp_host_env = "SMTP_HOST"
smtp_user_env = "SMTP_USER"
smtp_pass_env = "SMTP_PASS"
from_address = "ismailnuralam@gmail.com"

[cron]
timezone = "Asia/Jakarta"
```

---

## 7. Rebrand Plan

Rename bertahap dari zeroclaw → brai:

| Step | Target | Status |
|------|--------|--------|
| S1 | Update CLAUDE.md + buat PRD | ✅ Done |
| S2 | Rename binary `zeroclaw` → `brai` | ✅ Done |
| S3 | Rename crates `zeroclaw-*` → `brai-*` | ✅ Done |
| S4 | Update Cargo.toml workspace + package | ✅ Done |
| S5 | Update config path `~/.zeroclaw` → `~/.brai` | ✅ Done |
| S6 | Strip unused features (hardware, WASM, TUI, Tauri) | ✅ Done |
| S7 | Implement SOPs (deploy, cover-letter, bug-fix, morning-brief) | ✅ Done |
| S8 | Setup systemd + Telegram di VPS | ✅ Done |

---

## 8. Environment Variables

```env
# LLM
OPENROUTER_API_KEY=sk-or-...

# Telegram
TELEGRAM_BOT_TOKEN=...

# Email
SMTP_HOST=smtp.gmail.com
SMTP_PORT=587
SMTP_USER=ismailnuralam@gmail.com
SMTP_PASS=...  # Gmail App Password

# Optional: web search
WEB_SEARCH_PROVIDER=duckduckgo
WEB_SEARCH_ENABLED=true
```

---

## 9. Phased Rollout

### Phase 1 (sekarang)
- Rebrand zeroclaw → brai
- Strip unused features
- Setup Telegram + LLM provider
- Morning brief cron
- Basic chat (personal assistant mode)

### Phase 2
- DevSecOps SOP (deploy-flow)
- Developer SOP (bug-fix-flow dengan approval gate)
- Cover letter + email flow

### Phase 3
- Tambah channel (WhatsApp atau Discord)
- Monitoring otomatis (cek log error tanpa diminta)
- AI news digest yang lebih pintar

---

## 10. Risks

| Risk | Mitigasi |
|------|----------|
| RAM melebihi 1.5GB | Matikan embeddings, monitor dengan `brai status` |
| LLM cost tidak terkontrol | Set budget limit di config OpenRouter |
| Bot Telegram diakses orang lain | `allowed_users` whitelist wajib diset |
| Shell command berbahaya dieksekusi | Autonomy `supervised` + workspace boundary |
| Email terkirim tanpa konfirmasi | `ask_user` gate wajib sebelum semua SMTP send |
