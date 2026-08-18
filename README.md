# Kervan

Web üzerinden yapay zekâ kullanımını istediğiniz arayüzlere yönlendirmeyi sağlayan bir geçit altyapısı. Rust, tek ikili dosya.

> Çok yoldan yük taşıyan, hangi yolun ne kadar kaldırdığını bilen.

[**Türkçe**](#türkçe) | [**English**](README.en.md) | [**العربية**](README.ar.md) | [**日本語**](README.ja.md)

---

## Türkçe

### Ne işe yarar

API anahtarı olmayan AI hesaplarını (Gemini, Claude, Perplexity web arayüzleri) OpenAI uyumlu tek bir uca dönüştürür. Çerez kopyalamak yerine **gerçek tarayıcı** sürer: bir kez giriş yaparsın, oturum profil dizininde kalır.

Var olan çözümlerden farkı üç noktada:

- **Hesap başına gerçek yalıtım** — her hesap kendi tarayıcı profilinde. Gizli pencere yetmez: aynı tarayıcı örneğindeki tüm gizli pencereler tek profili paylaşır.
- **Kalıcı oturum** — çerez yapıştırma yok, çerez döndüğünde yenileme yok.
- **İnsan kapısı** — doğrulama duvarına çarpıldığında sessizce düşmez; hesabı duraklatır, pencereyi öne getirir, sen çözersin. Bot tespitini yenmeye çalışmaz.

### Durum

Aşama 3 (havuz kısmı) çalışıyor: çok sağlayıcı, **çok hesap**, hız bütçesi, insan kapısı.

| | |
|---|---|
| Tarayıcı | Firefox tabanlı (Zen ya da Firefox), geckodriver ile |
| Uç | `POST /v1/chat/completions`, `GET /v1/models`, `GET /health` |
| Havuz | `GET /accounts`, `POST /accounts/{i}/devam` |
| Bağlanma | yalnız `127.0.0.1` |

### Sağlayıcılar

Tanımlı: `gemini`, `kimi`, `copilot`, `copilot-m365`, `perplexity`, `claude`, `deepseek`, `qwen`, `z.ai`. Her hesap `kervan.toml`'da bir ya da birkaç sağlayıcıya bağlıdır.

#### Sağlayıcı durumu

| Sağlayıcı | Durum |
|---|---|
| `gemini` | ✅ Havuzda — çok hesap, ~11 sn |
| `kimi` | ✅ Havuzda — çok hesap, 25–36 sn |
| `copilot` | ✅ Havuzda — kişisel Microsoft hesabı, ~12 sn |
| `copilot-m365` | ✅ Havuzda — iş/okul hesabı, ~14 sn |
| `perplexity` | ⚠️ Kararsız — bazı hesaplarda Cloudflare |
| `claude` | ❌ Cloudflare — havuzda kapalı |
| `deepseek` | ❌ Cloudflare — hesapların çoğunda |
| `qwen` | ❌ Cloudflare — hesapların tümünde |
| `z.ai` | ❌ Cloudflare — yanıltıcı boş yanıt |

### Kota öğrenici

Web sağlayıcıları kalan kotayı söylemiyor. Kervan her isteğin sonucunu `%LOCALAPPDATA%\kervan\kota.jsonl` dosyasına yazar, limite çarpma anlarını işaretler.

```
curl http://127.0.0.1:20129/kota
```

- **Uydurma yok.** Gözlenmiş bir limit çarpması olmadan tahmin üretmez.
- Limit mesajı **yanıt olarak dönmez** — istemci 502 alır.

### Gereksinimler

- Rust 1.97+
- Firefox tabanlı bir tarayıcı (Zen desteklenir)
- [geckodriver](https://github.com/mozilla/geckodriver/releases) — `%LOCALAPPDATA%\kervan\bin\geckodriver.exe` altına ya da PATH'e

### Kullanım

İlk kez: görünür pencerede giriş yap (bir defalık).

```bash
cargo run -- profil-adi --giris
```

Sonra ağ geçidi olarak koştur:

```bash
cargo run -- --headless --serve
```

Tek soru sorup çıkmak için:

```bash
cargo run -- profil-adi --headless --sor "Mimar Sinan kimdir?"
```

### Yapılandırma (`kervan.toml`)

```toml
bosta_dakika = 15         # bu kadar dokunulmayan tarayıcı kapatılır

[[accounts]]
name = "kullanici@example.com"   # profil dizini = hesabın kendisi
providers = ["gemini", "perplexity"]

[pacing]
min_delay_ms = 3000       # aynı hesaba iki istek arası en az süre
jitter_ms = 2000          # üzerine rastgele sapma
hourly_cap = 30           # hesap başına saatlik tavan
```

### Hermes bağlantısı

`~/.hermes/config.yaml` içinde `custom` sağlayıcı olarak:

```yaml
  - name: kervan
    base_url: http://127.0.0.1:20129/v1
    api_key: kervan-local
    model: kervan/gemini
    models:
      - kervan/gemini
      - kervan/copilot
```

### Güvenlik

- Profil dizinleri çerez ve oturum içerir, `.gitignore`'da.
- Ağ geçidi yalnız `127.0.0.1`'e bağlanır.
- Kervan tek başına oturum açamaz — doğrulama hep insandan geçer.

**Yapılmayanlar:** parmak izi sahteciliği yok, CAPTCHA çözme yok, vekil/IP döndürme yok, otomasyon imzasını gizleme yok.

### Lisans

MIT — bkz. [LICENSE](LICENSE).
