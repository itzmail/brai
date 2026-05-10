# Morning Brief Flow

Kirim ringkasan harian ke Telegram setiap pagi jam 06:00 WIB.

## Steps

1. **Ambil pending todos** — Recall semua catatan dan todo yang belum selesai dari memory.
   - tools: memory_recall
   - Cari entries dengan tag `todo` atau `pending` yang dibuat dalam 7 hari terakhir.

2. **Cari berita AI terbaru** — Cari 3 berita AI paling relevan dari hari ini.
   - tools: web_search
   - Query: "AI artificial intelligence news today"
   - Ambil judul + ringkasan 1 kalimat per berita.

3. **Susun saran kegiatan** — Berdasarkan todos pending dan hari dalam minggu, susun 2-3 saran kegiatan produktif untuk hari ini.

4. **Kirim ke Telegram** — Format dan kirim brief ke user via Telegram.
   - Format pesan:
     ```
     🌅 *Morning Brief — {tanggal}*

     📋 *Pending Todos ({n} items):*
     {daftar todos}

     📰 *AI News Hari Ini:*
     1. {berita 1}
     2. {berita 2}
     3. {berita 3}

     💡 *Saran Hari Ini:*
     {saran kegiatan}
     ```
