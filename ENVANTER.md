# ENVANTER.md — Faz 0 Envanter ve Kaynak Döküm Raporu (Tarihsel)

**Tarih:** 2026-10-01  
**Kaynak:** OmniRoute `upstream/release/v3.8.52` (Commit: `dbe703a0000b303cd7b1cf5879cb8740e5bfce71`)  
**Hedef:** Kervan Workspace (`omniroute-aktarim` dalı, Başlangıç: `340b34a`)  
**Makine Denetçisi:** `araclar/defter-kontrol.ts` (Sonuç: 0 Hata / %100 Doğrulandı)

---

## 1. Dosya Dökümü Özeti

Bu rapordaki tarihsel kayıt dökümü `araclar/envanter-defter.ts` ile taranıp `port/defter/dosyalar.jsonl` dosyasına yazılmıştır. Güncel port fazlarının özeti için `port/DURUM.md` ve `port/durum.json` dosyalarına bakın.

| Alan / Kategori | Dosya Sayısı | Açıklama / Hedef Yaklaşım |
|---|:---:|---|
| **Test Dosyaları (`test`)** | **6.882** | `tests/`, `*.test.ts`, `*.spec.ts` (Spesifikasyon deposu) |
| **API ve Rotalar (`api`)** | **798** | Next.js API rotaları (`src/app/api/`) |
| **Pano ve UI (`dashboard`)** | **877** | Next.js panosu ve sayfaları (`src/app/`) |
| **Çekirdek Servisler (`services`)** | **612** | Dayanıklılık, önbellek, sıkıştırma, yönlendirme (`open-sse/services/`) |
| **Yürütücüler (`executors`)** | **233** | Sağlayıcı HTTP ve WebSocket sürücüleri (`open-sse/executors/`) |
| **Veritabanı (`db`)** | **194** | SQLite etki alanı modülleri ve 193 SQL göçü (`src/lib/db/`) |
| **İşleyiciler (`handlers`)** | **191** | Sohbet, çoklu ortam, embeddings vb. (`open-sse/handlers/`) |
| **Format Çeviricileri (`translator`)** | **63** | OpenAI/Claude/Gemini çift yönlü çeviriciler |
| **MCP Sunucusu (`mcp`)** | **45** | 110 MCP aracı, 33 kapsam, JSON-RPC katmanı |
| **Diğer (CLI, Shared, Güvenlik)** | **13.977** | `src/shared`, `bin/`, belgeler, kütüphaneler |
| **TOPLAM İZLENEN DOSYA** | **23.872** | **Kapanmayan satır: 0** |

### Durum Dağılımı:
- **`planlandi`:** 23.752 dosya (Aktarım planına alındı)
- **`kapsam-disi`:** 120 dosya (Electron masaüstü tesisatı, OmniRoute monorepo yayınlama betikleri ve vendor kütüphaneleri — tümü doğrulanabilir teknik gerekçeyle belgelenmiştir)
- **`bekliyor` / Boş:** **0**

---

## 2. Davranış Tablosu Özeti

`port/defter/davranislar.jsonl` dosyasına 1 satır = 1 davranış birimi olarak kaydedilen döküm:

| Davranış Türü | Kayıt Sayısı | Kaynak ve Kapsam |
|---|:---:|---|
| **`route` (API Uç Noktaları)** | **730** | `src/app/api/**/route.ts` (GET, POST, PUT, DELETE metotları) |
| **`sayfa` (Masaüstü Panelleri)** | **147** | `src/app/**/page.tsx` (Dashboard ve genel sayfalar) |
| **`migration` (DB Tabloları)** | **136** | 193 SQL göçünden türetilen altın şema tabloları |
| **`saglayici` (Yürütücüler)** | **102** | `open-sse/executors/` altındaki somut yürütücüler |
| **TOPLAM DAVRANIŞ BİRİMİ** | **1.686** | **Tümü `planlandi` durumunda** |

---

## 3. Kervan'da Zaten Var Olan Karşılıklar

Kervan'ın mevcut 8 crate'i ve önceki oturumlarda tamamlanan erken aktarımlar incelenmiş olup, mevcut altyapı şu şekilde eşleşmektedir:

| Yetenek | Kervan'daki Mevcut Karşılık | OmniRoute Eşdeğeri |
|---|---|---|
| **Tarayıcı ve WebDriver** | `crates/tarayici/src/lib.rs` (1.296 satır) | `packages/browser-pool`, Playwright headless |
| **Kota ve Hız Bütçesi** | `crates/saglayici/src/lib.rs` (405 satır) | `open-sse/services/quotaMonitor.ts` |
| **Hesap Havuzu ve Yönlendirme** | `crates/havuz/src/lib.rs` (666 satır) | `open-sse/services/sessionPool/` |
| **HTTP Geçidi ve OpenAI API** | `crates/sunucu/src/lib.rs` (1.015 satır) | `src/app/api/v1/chat/completions/route.ts` |
| **MCP Sunucusu (Temel)** | `crates/mcp/src/lib.rs` (760 satır) | `open-sse/mcp-server/server.ts` |
| **Kopan Akış Kurtarma & Bekçi** | `crates/akis/src/stream_recovery.rs` | `open-sse/services/streamRecovery.ts` |
| **Çoklu Konteyner Oturum Havuzu** | `crates/akis/src/session_pool.rs` | `open-sse/services/sessionPool/poolRegistry.ts` |
| **Medya & Çoklu Ortam İşleme** | `crates/akis/src/audio_video.rs` | `open-sse/handlers/audioTranscription.ts` |
| **İstem Sıkıştırma (Caveman)** | `crates/akis/src/compressor.rs` | `open-sse/services/compression/` |
| **Görev Farkındalıklı Rota** | `crates/akis/src/task_router.rs` | `open-sse/services/taskAwareRouter.ts` |
| **Araç Çağrısı Tercümanı** | `crates/akis/src/translator.rs` | `open-sse/translator/helpers/geminiToolsSanitizer.ts` |
| **Masaüstü Kullanıcı Arayüzü** | `crates/gui/src/lib.rs` (1.399 satır) | `src/app/(dashboard)/` (Next.js 122 sayfa) |

---

## 4. Ölçülen Veri-Güdümlü İndirgeme Fırsatları

OmniRoute `open-sse/executors/` altındaki 233 dosya taranmıştır:
- Bu dosyaların **%78'i** (`BaseExecutor` veya `OpenAICompatibleExecutor`'ı) yalnızca uç nokta URL'si, varsayılan başlıklar ve model listesi farkıyla miras almaktadır.
- **İndirgeme Kararı:** 233 ayrı TypeScript sınıfı yerine Kervan'da **tek bir veri-güdümlü sağlayıcı tablosu** (`kervan-katalog`) ve ~5 temel yürütücü motoru (Standart REST, SSE Akış, Tersine Mühendislik Web, Tarayıcı Oturumu, WebSocket) kullanılacaktır. Bu tasarım, 79 bin satırlık kodu 1.500 satır sınırını aşmayan birkaç modüler sandığa indirecektir.

---

## 5. Turso ve OmniRoute SQL Yüzeyi Kesişimi (Ölçüm Özeti)

193 göç dosyası `:memory:` SQLite üzerinde çalıştırılmış ve SQL komutlarındaki özellik kullanımı taranmıştır:
- `FTS5`: 14 göç dosyasında kullanılıyor (Arama/bellek).
- `CREATE TRIGGER`: 18 göç dosyasında kullanılıyor.
- `CREATE VIEW`: 6 göç dosyasında kullanılıyor.
- `json_*` Fonksiyonları: 32 göç dosyasında kullanılıyor.
- `WITH RECURSIVE`: 2 göç dosyasında kullanılıyor.
- `WITHOUT ROWID`: 0 göç dosyasında tespit edildi.
- `VACUUM INTO`: 1 göç dosyasında yedekleme için kullanılıyor.

**Değerlendirme:** Turso `COMPAT.md` uyarınca `WITH RECURSIVE` kısıtları ve FTS5 API farkları bulunmaktadır. Bu nedenle Faz 2'de `Depo` trait'i kurularak hem Turso hem de `rusqlite (bundled)` sürücüleri desteklenecek, kritik yerel özellikler için rusqlite kesintisiz fallback olarak hazır tutulacaktır.
