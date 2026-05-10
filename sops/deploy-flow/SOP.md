# Deploy Flow

Deploy aplikasi ke VPS dengan approval gate sebelum setiap destructive step.

## Steps

1. **Verifikasi target** — Tanyakan ke user: nama aplikasi, branch/tag yang akan di-deploy, dan konfirmasi environment (production/staging).
   - tools: ask_user
   - requires_confirmation: true

2. **Cek status git** — Jalankan `git status` dan `git log --oneline -5` di direktori aplikasi untuk verifikasi branch dan commit terkini.
   - tools: shell, git_operations

3. **Pull latest code** — Jalankan `git pull origin {branch}` di direktori aplikasi.
   - tools: shell, git_operations
   - Jika ada conflict, hentikan dan lapor ke user.

4. **Build aplikasi** — Jalankan build command sesuai stack (contoh: `cargo build --release` atau `npm run build`).
   - tools: shell
   - Tampilkan output build. Jika gagal, hentikan dan lapor error ke user.

5. **Konfirmasi restart service** — Tunjukkan diff perubahan file binary/assets. Minta konfirmasi sebelum restart.
   - tools: ask_user
   - requires_confirmation: true

6. **Restart systemd service** — Jalankan `sudo systemctl restart {service_name}` dan tunggu 3 detik.
   - tools: shell

7. **Verifikasi health** — Cek `systemctl status {service_name}` dan pastikan service `active (running)`. Jika gagal, otomatis rollback ke binary sebelumnya.
   - tools: shell

8. **Lapor hasil** — Kirim ringkasan deploy ke Telegram: versi, waktu deploy, status service.
