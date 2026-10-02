# DURUM.md — Kervan OmniRoute Aktarım Takip Defteri

Son güncelleme: 2026-10-02T03:16:00+03:00

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

### Faz 2 — Depo Katmanı (TAMAMLANDI ✅)
- **Sandık:** `crates/depo` (`kervan-depo`) oluşturuldu ve kök Cargo çalışma alanına bağlandı (557 satır, 1.500 satır kuralına tam uyum).
- **Soyutlama:** `Depo` trait tanımlandı (`gocleri_calistir`, `tablo_sayisi`, `saglik_kontrolu`, `yedekle`, `deger_getir`, `deger_kaydet`).
- **Şifreleme:** AES-256-GCM + scrypt (`enc:v1:<iv>:<ciphertext>:<tag>`) OmniRoute şifreleme formatı ile %100 parite sağlandı. TypeScript ortamında üretilen gerçek test vektörü Rust testinde birebir çözüldü (`test_omniroute_test_vektoru_cozumu`).
- **193 SQL Göçü:** 193 göç dosyasının tümü hatasız ve sıralı işletildi. OmniRoute'un TypeScript tarafında yaptığı `isSchemaAlreadyApplied`, `ensureUsageHistoryColumns`, `ensureProviderConnectionsColumns`, `ensureProxyLogsColumns` ve `API_KEY_COLUMN_FALLBACKS` şema tamamlayıcıları Rust'ta (`gocler.rs`) tam olarak inşa edildi.
- **Bütünlük & Şema Paritesi:** `PRAGMA integrity_check` başarıyla "ok" döndü ve 134 çekirdek alan tablosunun altın şemayla birebir örtüştüğü doğrulandı.
- **Kapı:** `kapi.toml` içinde `veritabani = "uygulanir"` aktive edildi. `kapi` denetimi 10 sandığın tamamı için yeşil (`yeşil 5 · kırmızı 0`).

---

## 2. Sıradaki İş (Faz 3 — Çekirdek Geçit)
- `crates/cekirdek` (`kervan-cekirdek`) veya `crates/sunucu` üzerinden `/v1` OpenAI/Anthropic/Gemini uç noktaları.
- Format çeviricileri: OpenAI ↔ Anthropic ↔ Gemini ↔ Ollama şema dönüşümleri.
- 19 kombo stratejisi (fallback, round-robin, weighted, cascade vb.).
- 3 katmanlı dayanıklılık: Devre kesici (circuit breaker), cooldown ve lockout durum makineleri.
- Hız sınırlayıcı (rate limiter) ve anlamsal önbellek (semantic cache).

---

## 3. Açık Engeller ve İnsan Kararları
- Herhangi bir engel veya insan kararı gerektiren tıkanıklık yoktur.
