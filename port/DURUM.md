# DURUM.md — Kervan OmniRoute Aktarım Takip Defteri

Son güncelleme: 2026-10-01T19:02:00+03:00

---

## 1. Tamamlanan Fazlar

### Faz 0 — Sabitleme, Envanter ve Taban (TAMAMLANDI ✅)
- **Sabitleme:** `upstream/release/v3.8.52` (SHA `dbe703a0000b303cd7b1cf5879cb8740e5bfce71`, sürüm 3.8.52) çekildi ve `ref/omniroute-dbe703a/` dizinine salt-okunur anlık görüntü olarak çıkartıldı (`port/defter/SABIT.md`).
- **Envanter ve Defter Motoru:** Bun çalışma alanında `araclar/envanter.ts` ve `araclar/defter-kontrol.ts` yazıldı ve başarıyla çalıştırıldı.
- **Envanter Çıktıları:**
  - `port/envanter/dosyalar.json` (23.872 dosya)
  - `port/envanter/routes.json` (730 Next.js API rotası)
  - `port/envanter/sayfalar.json` (147 sayfa)
  - `port/envanter/veritabani.json` (193 SQL göçü, 136 tablo altın şeması)
  - `port/envanter/katalog.json` (102 yürütücü, ortam değişkenleri)
  - `port/envanter/testler.json` (6.882 test dosyası, ~49.100 test vakası)
  - `port/defter/dosyalar.jsonl` (23.872 kayıt: 23.752 planlandı, 120 gerekçeli kapsam-dışı)
  - `port/defter/davranislar.jsonl` (1.686 davranış birimi: rotalar, sayfalar, tablolar, yürütücüler)
- **Defter Denetimi:** `bun araclar/defter-kontrol.ts` çalıştırıldı, çıkış kodu `0` (hatasız, eksiksiz, sıfır boş satır).
- **Sert Kurallar:** OmniRoute 23 Sert Kuralı incelendi ve `port/defter/sert-kurallar.md` tablosuna Rust karşılıklarıyla işlendi.
- **Kervan Tabanı:** HEAD `340b34a` üzerinde `kapi` denetimi yapıldı; `cargo fmt`, `clippy -D warnings`, `cargo test` ve 9 sandık satır sınırları (%100 yeşil) doğrulandı.

### Faz 1 — Kararlar ve İskelet (TAMAMLANDI ✅)
- **ADR Kayıtları (`docs/adr/`):** 9 temel mimari karar kaydı yazıldı ve onaylandı:
  - ADR 0001: Crate topolojisi ve 1.500 satır sandık sınırı
  - ADR 0002: Veri katmanı soyutlaması (`Depo` trait) ve Turso / rusqlite stratejisi
  - ADR 0003: Veri-güdümlü sağlayıcı kataloğu ve yürütücü indirgemesi
  - ADR 0004: Hata sınıflandırma, temizleme ve güvenlik sanitizasyonu
  - ADR 0005: Kimlik, API anahtarları ve güvenlik politikası
  - ADR 0006: Yapılandırma (`kervan.toml`) ve durum veritabanı senkronizasyonu
  - ADR 0007: Bun araç katmanı ve isteğe bağlı dış süreç stratejisi
  - ADR 0008: egui masaüstü mimarisi, panel kayıt sistemi ve WGPU render kuralı
  - ADR 0009: Parite sistemi, altın referanslar ve mutasyon kontrolü
- **Parite İskeleti:**
  - `araclar/parite/kayit.ts`: Altın senaryo kaydedici
  - `araclar/parite/kos.ts`: Parite koşucusu ve mutasyon kontrolcüsü
  - `port/altin/merhaba.json`: İlk altın sözleşme dosyası
- **Mutasyon Doğrulaması:** `bun araclar/parite/kos.ts --mutasyon` çalıştırıldı; şablon kasten bozulduğunda kapının başarıyla düştüğü kanıtlandı ("Düşemeyen kapı kapı değildir").
- **Kapı Durumu:** `kapi` tüm 9 sandık için yeşil.

---

## 2. Sıradaki İş (Faz 2 — Depo Katmanı)
- `crates/depo` (`kervan-depo`) crate'inin kurulması ve `Depo` trait tanımları.
- `rusqlite (bundled)` sürücüsü ile 193 SQL göçünün derlenmesi ve altın şema ile birebir paritesi.
- AES-256-GCM kimlik bilgisi şifreleme/çözümleme vektörleri.
- Turso sürücüsü ölçümü ve Windows kilitleme senaryoları.

---

## 3. Açık Engeller ve İnsan Kararları
- Şu an için bloklayıcı engel bulunmamaktadır.
