# Pull Request: OmniRoute Gauntlet Port (P1 — P10)

- **Kaynak Dal:** `omniroute-port`
- **Hedef Dal:** `main`
- **PR Başlığı:** `feat: OmniRoute LLM Ag Gecidi Guvenli ve Dogrulanabilir Rust Portu (Gauntlet P1-P10)`
- **Durum:** Taslak (Draft) — Ercan onayına sunulmuştur (otomatik merge açılmaz).

---

## 1. Genel Bakış ve Kapsam

Bu PR, OmniRoute (`release/v3.8.52`, commit `dbe703a`) LLM ağ geçidi yeteneklerini Gauntlet prensipleri (fail-closed deterministik kapılar, sıfır sahte kod, altın fikstür paritesi, mutasyon öldürme, sandık satır sınırı) doğrultusunda Kervan projesine güvenle taşır.

### Temel Teslimatlar (Faz P1 — P10)

1. **Mühürlü Doğrulama Altyapısı (P1 — P3):**
   - 20 mühürlü deterministik kapı betiği (`araclar/kapilar/*.ts`).
   - `port/muhur.toml` (SHA-256 mühür kütüğü).
   - Kırmızı takım külliyatı: `ac2f18d` sahte teslim kusurlarının 5 sınıfının tamamı otomatik yakalanır.
   - 7 mutasyon kuralı ve 7/7 öldürme güvencesi (`port/mutantlar.toml`).

2. **Çoklu Sağlayıcı REST Geçişi & Dinamik Nonce (P4 — P5):**
   - `crates/yukari`: Hafif `ureq` ve `rustls` tabanlı asenkron REST yukarı akış istemcisi.
   - OpenAI, Anthropic Claude ve Google Gemini sağlayıcıları için kayıpsız istek/yanıt iletimi.
   - Dinamik rastgele nonce doğrulamalı uçtan uca testler (`crates/sunucu/tests/p4_rest_test.rs`).
   - Dinamik `/v1/models` uç noktası; sahte/uydurma OpenAI modelleri temizlendi.

3. **Çeviri Motoru & Altın Parite (P6):**
   - OmniRoute referans kodundan türetilmiş 12 altın fikstür (`port/altin/*.json`).
   - Claude ve Gemini için birebir bayt paritesi; desteklenmeyen alanlar (`tools`, multimodal) için 501 açık koruma freni.

4. **3 Katmanlı Dayanıklılık & Güvenlik Kalkanı (P7):**
   - Devre Kesici (Circuit Breaker): Ardışık 5xx veya ağ hatalarında 503 ile yukarı akışı koruma.
   - Hız Sınırı Soğuma (429 & Retry-After): Otomatik soğuma penceresi.
   - API anahtarı ve iç yol sızıntı sanitizasyonu.

5. **Gerçek SSE Akışı ve Araç Çağrısı (P8):**
   - Asenkron `tokio::sync::mpsc` kanallı gerçek SSE akış motoru (`crates/yukari/src/akis.rs`).
   - İlk parça gecikme ölçümü ($t_{ilk} < 100\text{ms} < t_{toplam}$).
   - Claude/Gemini SSE olaylarının anlık OpenAI `chat.completion.chunk` biçimine dönüştürülmesi ve `[DONE]` sonlanması.
   - OpenAI araç çağrılarının (`tools`, `tool_calls`) akışlı/akışsız kayıpsız geçişi.

6. **Yönlendirme Stratejileri & İkizler (P9):**
   - OmniRoute kümesiyle birebir eşit 20 kamuya açık ve 1 dahili (`quota-share`) strateji (`crates/havuz/src/router.rs`).
   - Deterministik tohumlu altın fikstürler (`port/altin/strateji_girdi*.json`).
   - `random` vs `strict-random` ve `reset-aware` vs `reset-window` ikizlerinin farklı davranış kanıtları.

7. **Envanter, Defter ve Dalga Düzeni (P10):**
   - Deterministik envanter aracı (`araclar/envanter.ts`, `port/envanter.json`).
   - Pencere örtüşmeli defter (`port/defter/{dosyalar,davranislar}.jsonl`).
   - Sonraki 3 dalganın taslak PRD'leri (`port/dalgalar/DALGA-{2,3,4}.md`).

---

## 2. Üç Salt-Okunur Bakış İncelemesi

### 🛠️ Bakımcı Bakışı (Maintainer View)
- **Mimari Disiplini:** Hiçbir crate 1500 kod satırı sınırını aşmamıştır (`kervan-sunucu` ≈1240 satır, `kervan-yukari` ≈450 satır, `kervan-ceviri` ≈880 satır).
- **Kod Kalitesi:** Üretim kodunda sıfır `unwrap()`/`expect()` politikası uygulanmış, tüm hatalar tipli `YukariHatasi` veya `CeviriHatasi` ile döndürülmüştür.
- **Mühür Güvencesi:** `port/muhur.toml` içindeki 20 mühürlü dosyanın SHA-256 özeti korunmuştur.
- **Mutasyon:** Eklenen 7 mutantın tamamı canlı testlerle öldürülmektedir.

### 👤 Benimseyici Bakışı (Adopter View)
- **Geriye Uyumluluk:** Mevcut tarayıcı/eklenti havuz yolu ve yerleşik ayarlar aynen korunmuştur (`[rest.*]` tanımlanmadığında mevcut akış kesintisiz çalışır).
- **Belgelendirme:** `port/DURUM.md` elle değil, kapı çıktılarından SHA-256 imzasıyla otomatik üretilmektedir.
- **Öngörülebilirlik:** Desteklenmeyen alanlar sessizce yutulmaz, net `501 Not Implemented` ile istemciye bildirilir.

### 🚀 Dağıtımcı Bakışı (Deployer View)
- **Sızıntı Güvenliği:** API anahtarları asla kütük dosyalarına, commite veya istemci hata mesajlarına yazılmaz.
- **Yalıtım:** Canlı OmniRoute portuna (`127.0.0.1:20128`) ve kullanıcı dizinine (`~/.omniroute`) asla dokunulmamıştır.
- **Git Temizliği:** `main` dalı hiçbir doğrudan commit almamıştır. Çalışma yalnızca `omniroute-port` dalında yürütülmüştür.

---

## 3. Doğrulama Özeti

| Faz | Açıklama | Kapı Durumu | Log |
|---|---|:---:|---|
| **P1** | Temel kapılar, makine sakinliği, kabul haritası | ✅ 0 | [P1/durum.json](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P1/durum.json) |
| **P2** | Ulaşılabilirlik, sabit yanıt, uç kaydı, mühür | ✅ 0 | [P2/durum.json](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P2/durum.json) |
| **P3** | Kırmızı takım, mutasyon, üretilmiş durum, defter | ✅ 0 | [P3/durum.json](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P3/durum.json) |
| **P4-P5** | REST yukarı akış, dinamik nonce, modeller | ✅ 0 | [P5/durum.json](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P5/durum.json) |
| **P6** | Altın fikstür paritesi, Claude & Gemini çevirisi | ✅ 0 | [P6/durum.json](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P6/durum.json) |
| **P7** | Dayanıklılık, devre kesici, 429 soğuma, sanitizasyon | ✅ 0 | [P7/durum.json](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P7/durum.json) |
| **P8** | Gerçek SSE akışı, araç çağrısı geçişi | ✅ 0 | [P8/durum.json](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P8/durum.json) |
| **P9** | 21 Yönlendirme stratejisi, ikizler ayrımı | ✅ 0 | [P9/durum.json](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P9/durum.json) |
| **P10** | Envanter, defter, dalga PRD'leri, PR taslağı | ✅ 0 | [P10/RAPOR.md](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P10/RAPOR.md) |
