**🌍 [Türkçe](README.md) | [English](README.en.md) | [العربية](README.ar.md) | [日本語](README.ja.md)**

# Kervan

[![CodSpeed](https://img.shields.io/endpoint?url=https://codspeed.io/badge.json)](https://app.codspeed.io/Ercaner1988/kervan?utm_source=badge)
[![Altın Kapı](https://img.shields.io/badge/alt%C4%B1n--kap%C4%B1-16%20boyut-brightgreen)](#altın-kapı-ve-ekdenetle-disiplini)

Yapay zekâ kullanımını tek bir çatı altında birleştiren, yüksek başarımlı ve sıfır-maliyetli evrensel yapay zekâ geçidi ve yönlendirme motoru. Saf Rust, modüler çalışma alanı (20 sandık), yerel egui masaüstü arayüzü ve Bun araç takımı.

> Çok yoldan yük taşıyan, hangi yolun ne kadar kaldırdığını bilen.

---

## 📦 Özellikler ve Mimari

Kervan, hem API anahtarı olmayan web oturumlarını (Gemini, Claude, Perplexity vb.) hem de 276'dan fazla resmi/açık kaynaklı sağlayıcıyı (OpenAI, Anthropic, Google, DeepSeek, Qwen, Mistral, Ollama, Groq, vLLM...) tek bir OpenAI uyumlu uçta toplar.

* **276 Sağlayıcı ve 5 Birleşik Yürütücü:** Standart REST, Claude Messages, Gemini Content, Tarayıcı Oturumu ve OAuth2 belirteç yenileme motoru.
* **19 Kanonik Yönlendirme Stratejisi:** Öncelik, Round-Robin, Ağırlıklı, En Az Kullanılan, Maliyet Odaklı, Bağlam Odaklı, Anlamsal Önbellek Odaklı, Kota Paylaşımlı vb.
* **3 Katmanlı Dayanıklılık (Resilience):** Devre Kesici (Circuit Breaker), Üstel Geri Çekilme Soğuma Takipçisi ve Geçici/Kalıcı Model Kilidi.
* **İki Yönlü Biçim Çeviricisi:** OpenAI ↔ Claude ↔ Gemini format ve SSE akış parçacığı dönüşümleri (alt-mikrosaniye gecikme).
* **Güvenli Depo ve 193 SQL Göçü:** AES-256-GCM + scrypt şifrelemeli yerel SQLite veritabanı, 134 tablo altın şeması ve `VACUUM INTO` sıcak yedekleme.
* **A2A Protokolü (v0.3):** JSON-RPC 2.0 `tasks.create`, `tasks.get`, `tasks.cancel` ve `/.well-known/agent.json` ajan kartı keşfi.
* **Sert Kural #20 Koruma Kalkanı:** PII maskeleme (varsayılan kapalı / opt-in), hassas kimlik gizleme (`[REDACTED_API_KEY]`) ve istem enjeksiyonu sezgisel savunması.
* **Modüler egui Masaüstü Panelleri:** Gösterge Paneli, Model Yönetimi (276 sağlayıcı / filtreleme) ve Güvenlik/PII paneli.
* **CLI İşletim Araçları:** `kervan goc` (kuru koşu varsayılan; `--uygula` ile güvenli göç), `kervan yedekle`, `kervan dogrula`.

---

## 🚀 Başarım Ölçümleri (CodSpeed / Divan)

Kervan çekirdek operasyonları mikrosaniyenin altındaki sürelerde icra edilir:

| Operasyon | Ölçülen Medyan Süre | Açıklama |
|---|---|---|
| **OpenAI ➔ Claude Çevirisi** | `~384 ns` | İstek şeması normalizasyonu ve sistem talimatı ayırma |
| **OpenAI ➔ Gemini Çevirisi** | `~315 ns` | İstek ve çok modlu içerik dönüşümü |
| **19 Kombo Yönlendirme** | `~799 ns` | 3 sağlayıcı / model arasında maliyet & bağlam sıralaması |
| **AES-256-GCM + scrypt Şifreleme** | `~1.39 µs` | Hassas anahtar ve belirteç şifreleme/çözme |

---

## ⚙ Kurulum ve Çalıştırma

### Gereksinimler
* Rust 1.80+ (2024 edition)
* Windows, Linux veya macOS
* Opsiyonel: Tarayıcı oturumları için Firefox veya Zen Browser

### Derleme

```bash
# Depoyu klonlayın
git clone https://github.com/Ercaner1988/kervan.git
cd kervan

# Çalışma alanını derleyin
cargo build --release

# Testleri ve kapı denetimini koşun
cargo test --workspace --all-features
```

### Kullanım

```bash
# Sunucuyu başlatın (varsayılan 127.0.0.1:20129)
./target/release/kervan

# Masaüstü grafik arayüzü ile başlatmak için:
cargo run --features gui
```

### Örnek API İsteği

```bash
curl http://127.0.0.1:20129/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "claude-3-7-sonnet",
    "messages": [{"role": "user", "content": "Merhaba Kervan!"}]
  }'
```

---

## 🛡 Altın Kapı ve Ekdenetle Disiplini

Kervan projesi katı mühendislik kurallarına tabidir:
1. **Sandık Boyut Sınırı:** Çalışma alanındaki 20 sandığın her biri istisnasız **en fazla 1.500 kod satırıdır**.
2. **Sıfır Panik:** Üretim kodunda hiçbir `unwrap` veya `expect` bulunmaz.
3. **Loopback Güvenliği:** Yalnızca yerel döngü (`127.0.0.1`) üzerinden dinler; dış ağlara kapalıdır.
4. **Temiz Hata Disiplini:** İstemciye ham veritabanı hatası ya da iç çağrı yığını (stack trace) asla sızdırılmaz.

---

## 📄 Lisans

MIT License © 2026 Ercan ER