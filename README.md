**🌍 [Türkçe](README.md) | [English](README.en.md) | [العربية](README.ar.md) | [日本語](README.ja.md)**

# Kervan

[![CodSpeed](https://img.shields.io/endpoint?url=https://codspeed.io/badge.json)](https://app.codspeed.io/Ercaner1988/kervan?utm_source=badge)

Web üzerinden yapay zekâ kullanımını istediğiniz arayüzlere yönlendirmeyi sağlayan bir geçit altyapısı. Rust, tek ikili dosya.

> Çok yoldan yük taşıyan, hangi yolun ne kadar kaldırdığını bilen.

---

## 📦 Gereksinimler

| Gereksinim | Sürüm / Not |
|---|---|
| Rust | 1.70+ (derleme için) |
| Firefox | Zen veya standart Firefox (geckodriver ile) |
| İşletim sistemi | Windows, Linux, macOS |

## 🛠 İşlev ve Özellikler

API anahtarı olmayan AI hesaplarını (Gemini, Claude, Perplexity web arayüzleri) OpenAI uyumlu tek bir uca dönüştürür. Çerez kopyalamak yerine **gerçek tarayıcı** sürer: bir kez giriş yaparsın, oturum profil dizininde kalır.

Var olan çözümlerden farkı üç noktada:

- **Hesap başına gerçek yalıtım** — her hesap kendi tarayıcı profilinde. Gizli pencere yetmez: aynı tarayıcı örneğindeki tüm gizli pencereler tek profili paylaşır.
- **Kalıcı oturum** — çerez yapıştırma yok, çerez döndüğünde yenileme yok.
- **İnsan kapısı** — doğrulama duvarına çarpıldığında sessizce düşmez; hesabı duraklatır, pencereyi öne getirir, sen çözersin. Bot tespitini yenmeye çalışmaz.

## ⚙ Kurulum

### Kurulum Adımları

1. Depoyu klonlayın:

```bash
git clone https://github.com/Ercaner1988/kervan.git
cd kervan
```

2. Derleyin:

```bash
cargo build --release
```

3. Yapılandırma dosyasını oluşturun:

```bash
cp kervan.toml.example kervan.toml
```

4. Hesaplarınızı `kervan.toml`'da tanımlayın.

### Kullanım

```bash
./target/release/kervan
```

Sunucu `127.0.0.1:20129` üzerinde çalışır. OpenAI uyumlu uç:

```bash
curl http://127.0.0.1:20129/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model": "gemini", "messages": [{"role": "user", "content": "Merhaba"}]}'
```

## 🗺 Durum ve Yol Haritası

| Durum | Konu |
|---|---|
| ✅ Kurulu | Çok sağlayıcı, çok hesap, hız bütçesi, insan kapısı |
| 🔬 Deneysel | Kota öğrenme (gözlemlenen kota sınırları) |
| 📅 Gelecekte | `kervan.toml` şablonları, sağlayıcı başına kota öğrenme |

### Teknik Detaylar

| | |
|---|---|
| Tarayıcı | Firefox tabanlı (Zen ya da Firefox), geckodriver ile |
| Uç | `POST /v1/chat/completions`, `GET /v1/models`, `GET /health` |
| Havuz | `GET /accounts`, `POST /accounts/{i}/devam` |
| Bağlanma | yalnız `127.0.0.1` |

### Sağlayıcılar

Tanımlı: `gemini`, `kimi`, `copilot`, `copilot-m365`, `perplexity`, `claude`, `deepseek`, `qwen`, `z.ai`. Her hesap `kervan.toml`'da bir ya da birkaç sağlayıcıya bağlıdır.

#### Sağlayıcı Durumu

<!-- DURUM:BASLA -->
> Bu blok araclar/kapilar/durum-uret.ts çıktısıdır; elle düzenlemeyin. Kaynak: port/durum.json

- Faz: P3 (2026-10-14T06:28:43.273Z)
- Tüm kapılar yeşil mi: hayır (9/11)
- Fazlar: yok
- Defter: planlandi 5, dogrulandi 1; route_ts 730
<!-- DURUM:BITIS -->

##### Sağlayıcı notları (elle yazılmış; kapı doğrulamaz)

| Sağlayıcı | Durum |
|---|---|
| `gemini` | ✅ Havuzda — çok hesap, ~11 sn |
| `kimi` | ✅ Havuzda — çok hesap, ~24 sn. **Adres `kimi.ai`**, `kimi.com` değil |
| `copilot` | ✅ Havuzda — kişisel Microsoft hesabı, ~12 sn |
| `copilot-m365` | ✅ Havuzda — iş/okul hesabı, ~14 sn |
| `perplexity` | ⚠️ Kararsız — bazı hesaplarda Cloudflare |
| `claude` | ❌ Cloudflare — havuzda devre dışı |
| `deepseek` | ❌ Cloudflare — çoğu hesapta |
| `qwen` | ❌ Cloudflare — tüm hesaplarda |
| `z.ai` | ❌ Cloudflare — yanıltıcı boş yanıt |

## ⚠️ Bilinen Sorunlar / Deneysel Kısımlar

- **Kota öğrenme:** Web sağlayıcıları kota durumunu açıklamaz. Kervan her isteğin sonucunu `%LOCALAPPDATA%\kervan\kota.jsonl`'ye yazar ve kota dolduğunda işaretler.
- **Cloudflare:** Claude, Deepseek, Qwen, Z.ai çoğu hesapta Cloudflare duvarına çarpar.
- **İnsan kapısı:** Doğrulama duvarında hesabı duraklatır ve pencereyi öne getirir — otomatik çözüm yok.

## 🤝 Katkıda Bulunanlar

- [Ercan ER](https://github.com/Ercaner1988) — tasarım ve geliştirme

## 📜 Lisans

MIT — ayrıntı için [LICENSE](LICENSE).