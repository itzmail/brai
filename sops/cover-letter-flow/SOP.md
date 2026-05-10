# Cover Letter Flow

Generate cover letter dari CV + deskripsi posisi, lalu kirim email setelah approval user.

## Steps

1. **Kumpulkan informasi** — Tanyakan ke user: posisi yang dilamar, nama perusahaan, email tujuan, dan apakah ada CV baru yang perlu di-upload.
   - tools: ask_user
   - requires_confirmation: true

2. **Baca CV** — Baca file CV terbaru dari `/home/ismail/brai-storage/cvs/`. Jika belum ada, minta user upload via Telegram.
   - tools: file_read, memory_recall
   - Ekstrak: nama, skills utama, pengalaman relevan, pendidikan.

3. **Cari info perusahaan** — Web search untuk info singkat perusahaan: produk, stack teknologi, kultur kerja.
   - tools: web_search
   - Query: "{nama_perusahaan} engineering team tech stack culture"

4. **Generate cover letter** — Buat cover letter yang dipersonalisasi berdasarkan CV dan info perusahaan.
   - Tone: profesional tapi personal, bahasa Indonesia atau Inggris sesuai permintaan user.
   - Panjang: 3-4 paragraf.
   - Sertakan: alasan tertarik posisi ini, skill yang relevan, kontribusi yang bisa diberikan.

5. **Tampilkan draft** — Kirim draft cover letter ke user via Telegram untuk review.
   - Sertakan opsi: "Kirim", "Edit", atau "Batal".
   - requires_confirmation: true

6. **Proses feedback** — Jika user minta edit, terapkan perubahan dan kembali ke step 5. Jika "Batal", hentikan SOP.
   - tools: ask_user

7. **Kirim email** — Kirim cover letter via SMTP ke email tujuan dengan subject: "Lamaran Posisi {posisi} — Ismail Alam".
   - tools: email
   - Lampirkan CV.pdf jika ada.

8. **Konfirmasi terkirim** — Lapor ke user: email terkirim ke {alamat_email} pada {waktu}.
   - Simpan ke memory: tanggal lamar, posisi, perusahaan, email tujuan.
   - tools: memory_store
