# Bug Fix Flow

Trace bug dari error log atau laporan user, propose fix, apply setelah approval. Non-blocking.

## Steps

1. **Kumpulkan konteks bug** — Baca error log atau deskripsi bug dari user. Identifikasi: file yang terlibat, stack trace, kondisi saat error terjadi.
   - tools: file_read, ask_user
   - Jika user paste error log langsung, gunakan itu sebagai input.

2. **Baca source code** — Baca file yang disebutkan di stack trace. Cari fungsi/method yang menjadi titik kegagalan.
   - tools: file_read, shell
   - Jalankan `grep -n "{error_keyword}" {file_path}` untuk lokasi cepat.

3. **Cek git history** — Periksa commit terakhir yang mengubah file terkait.
   - tools: git_operations, shell
   - Jalankan `git log --oneline -10 -- {file_path}` dan `git blame {file_path}`.

4. **Analisa root cause** — Identifikasi root cause berdasarkan source code dan git history. Buat penjelasan yang jelas.

5. **Propose fix** — Buat proposed fix dalam bentuk diff/penjelasan perubahan yang diperlukan.
   - Kirim ke user via Telegram:
     ```
     🐛 *Bug Ditemukan*
     File: {file_path}:{line}
     Root cause: {penjelasan}

     🔧 *Proposed Fix:*
     {diff atau penjelasan perubahan}

     Apply fix? [Ya / Tidak / Lihat diff lengkap]
     ```
   - requires_confirmation: true

6. **Tunggu approval** — Tunggu reply user. SOP ini non-blocking — agent lain tetap bisa berjalan.
   - tools: ask_user
   - Jika "Tidak": catat di memory sebagai `bug_deferred`, hentikan SOP.
   - Jika "Lihat diff lengkap": kirim diff lengkap, kembali ke step 6.

7. **Apply fix** — Terapkan perubahan ke file sesuai proposed fix.
   - tools: file_edit
   - Verifikasi perubahan dengan membaca ulang file.

8. **Commit perubahan** — Buat git commit dengan pesan deskriptif.
   - tools: git_operations, shell
   - Format commit: `fix({scope}): {deskripsi singkat}`
   - Tampilkan commit hash ke user.

9. **Lapor selesai** — Kirim konfirmasi ke user: fix applied, commit hash, ringkasan perubahan.
   - tools: memory_store
   - Simpan ke memory: bug yang diperbaiki, file, commit hash, tanggal.
