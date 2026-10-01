# DURUM.md — Kervan OmniRoute Aktarım Takip Defteri

Son güncelleme: 2026-10-01T18:55:00+03:00

---

## 1. Şimdiki Durum (Faz 0 Tamamlandı)
- **Sabitleme:** OmniRoute `upstream/release/v3.8.52` (SHA `dbe703a0000b303cd7b1cf5879cb8740e5bfce71`, sürüm 3.8.52) çekildi ve `ref/omniroute-dbe703a/` dizinine salt-okunur anlık görüntü olarak çıkartıldı (`port/defter/SABIT.md`).
- **Envanter ve Defter Motoru:** Bun çalışma alanında `araclar/envanter.ts` ve `araclar/defter-kontrol.ts` yazıldı ve başarıyla çalıştırıldı.
- **Envanter Çıktıları:**
  - `port/envanter/dosyalar.json` (23.872 dosya taranarak sınıflandırıldı)
  - `port/envanter/routes.json` (730 Next.js API rotası ve HTTP metotları)
  - `port/envanter/sayfalar.json` (147 dashboard ve genel sayfa)
  - `port/envanter/veritabani.json` (193 SQL göçü `:memory:` SQLite üzerinde çalıştırıldı, 136 tablo altın şeması çıkarıldı)
  - `port/envanter/katalog.json` (102 executor ve ortam değişkenleri haritası)
  - `port/envanter/testler.json` (6.882 test dosyası, ~49.100 test vakası)
  - `port/defter/dosyalar.jsonl` (23.872 dosya satırı: 23.752 planlandı, 120 gerekçeli kapsam-dışı)
  - `port/defter/davranislar.jsonl` (1.686 davranış birimi: 730 rota, 147 sayfa, 136 DB tablosu, 102 yürütücü)
- **Defter Denetimi:** `bun araclar/defter-kontrol.ts` çalıştırıldı, çıkış kodu `0` (hatasız, eksiksiz, sıfır boş satır).
- **Sert Kurallar:** OmniRoute 23 Sert Kuralı incelendi ve `port/defter/sert-kurallar.md` tablosuna Rust karşılıklarıyla işlendi.
- **Kervan Tabanı:** HEAD `340b34a` üzerinde `kapi` denetimi yapıldı; `cargo fmt`, `clippy -D warnings`, `cargo test` ve 9 sandık satır sınırları (%100 yeşil) doğrulandı.

---

## 2. Sıradaki İş (Faz 1 — Kararlar ve İskelet)
- `kervan/docs/adr/` altında mimari karar kayıtlarının (ADR) oluşturulması:
  - ADR 0001: Crate topolojisi ve modüler mimari (≤1500 kod satırı kuralı)
  - ADR 0002: Veri katmanı (`Depo` trait, Turso/rusqlite stratejisi)
  - ADR 0003: Sağlayıcı kataloğu ve veri güdümlü yürütücü modeli
  - ADR 0004: HTTP yüzeyi, axum yönlendirme ve hata temizleme
  - ADR 0005: Parite sistemi ve altın senaryolar
- `araclar/parite/kayit.ts` ve `araclar/parite/kos.ts` parite koşucusu iskeletinin kurulması.
- Uçtan uca "Merhaba" parite senaryosunun (`port/altin/merhaba.json`) mutasyon kontrollü çalıştırılması.

---

## 3. Açık Engeller ve İnsan Kararları
- Şu an için bloklayıcı engel bulunmamaktadır.
- Canlı `.omniroute` verisine dokunulmamaktadır.
- Tüm testler sentetik ve sahte anahtarlarla yürütülmektedir.
