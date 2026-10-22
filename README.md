**🌍 [Türkçe](README.md) | [English](README.en.md) | [العربية](README.ar.md) | [日本語](README.ja.md)**

# Kervan

[![CodSpeed](https://img.shields.io/endpoint?url=https://codspeed.io/badge.json)](https://app.codspeed.io/Ercaner1988/kervan?utm_source=badge)

Web üzerinden yapay zekâ kullanımını istediğiniz arayüzlere yönlendirmeyi sağlayan bir geçit altyapısı. Rust, tek ikili dosya.

> Çok yoldan yük taşıyan, hangi yolun ne kadar kaldırdığını bilen.

---

## 📦 Gereksinimler

| Gereksinim | Sürüm / Not |
|---|---|
| Rust | 1.88+ — `edition = "2024"` ve let-chains kullanılıyor; daha eski araç zincirleri derleyemez |
| Firefox | Zen veya standart Firefox |
| geckodriver | Sunucu açılışta arar: `KERVAN_GECKODRIVER` ortam değişkeni ya da `%LOCALAPPDATA%\kervan\bin\geckodriver.exe` (eklenti köprüsünü tek başına kullananlar dâhil) |
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
./target/release/kervan --serve
```

Sunucu `127.0.0.1:20129` üzerinde çalışır. OpenAI uyumlu uç:

```bash
curl http://127.0.0.1:20129/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model": "gemini", "messages": [{"role": "user", "content": "Merhaba"}]}'
```

Çalıştırma kipleri:

| Kip | Ne yapar |
|---|---|
| `--serve` | Sunucuyu başlatır (uç + eklenti köprüsü). `--port <n>` ile port değişir. |
| `--giris <hesap>` | Hesabın tarayıcı penceresini açar; bir kez elle giriş yapılır. |
| `--tara` | Tanımlı hesapların oturum durumunu yeniden tarar. |
| (bayraksız) | Tek oturum kipi: profili açar ve Enter bekler — sunucu başlatmaz. |

Hesap tanımları ve tüm alanlar için `kervan.toml.example`.

### Eklenti köprüsü (WebSocket)

Tarayıcı eklentisi `eklenti/` altındadır ve Kervan'a WebDriver olmadan bağlanır:
`ws://127.0.0.1:20129/ws/eklenti` (sunucu açılışı yine de makinede geckodriver
arar — bkz. Gereksinimler). Eklenti her sekmenin oturumunu `provider:profile`
anahtarıyla bildirir; profil = Firefox'un `cookieStoreId`'si, yoksa `varsayilan`.

**İlk kurulum (bir kez):**

1. Firefox → `about:debugging#/runtime/this-firefox` → **Geçici Eklenti Yükle…**
   → `kervan/eklenti/manifest.json` seç.
2. Kervan `--serve` ile açık olmalı; sunucu kapalıyken eklenti 3 saniyede bir
   yeniden bağlanmayı dener.
3. Sağlayıcı sayfasını (örn. `gemini.google.com`) aç ve giriş yap — sekme
   köprüye otomatik kaydolur.

Geçici yükleme tarayıcı kapanınca düşer; kalıcı kullanım için imzalı bir XPI
paketi gerekir (Firefox ESR tercih ediliyorsa `xpinstall.signatures.required`
ayarıyla). Yeniden yükleme sonrası eski kayıtlar düşer, yeni bağlantı kendini
yeniden kaydeder.

**Çok hesap:** İkinci hesap, sağlayıcıyı **ikinci bir Firefox container'ında**
açmaktır (Multi-Account Containers eklentisi). Aynı container'daki ikinci sekme
ayrı hesap **değildir** — son açılan öncekinin kaydının yerine geçer; ayrı
hesap için ayrı `cookieStoreId` gerekir. Seçim hesap düzeyinde en-az-kullanılan
ilkesiyle yapılır.

Köprü artık kimlik ister:

- `Host` yalnız `127.0.0.1`, `localhost` ya da `[::1]` olabilir (DNS rebinding'e karşı).
- `Origin` yoksa (yerel istemci) ya da uzantı şemasıysa (`chrome-extension://`,
  `moz-extension://`) kabul edilir; `http(s)://` sayfalar reddedilir.
- Paylaşılan token zorunludur. Kervan açılışta yolunu basar:
  `%LOCALAPPDATA%\kervan\eklenti.token` (Linux/macOS'ta
  `~/.local/state/kervan/eklenti.token`; `HOME` da yoksa çalışma dizinindeki
  `kervan/eklenti.token`). Eklenti token'ı yalnız uzantı kaynaklı isteklere açık
  olan `GET /eklenti/token` ucundan alır ve `storage.local`'da saklar — kullanıcı
  için ek iş yok.

Eski eklenti sürümü token göndermediği için bağlanamaz; `eklenti/` klasörünü
yukarıdaki adımlarla yeniden yükleyin.

### Doğrudan REST sağlayıcıları

Tarayıcı sekmesi açmadan kendi API'sine gitmek isteyen sağlayıcı `[rest.<ad>]`
ile tanımlanır (`bicim`: `openai`, `claude`, `gemini`). Anahtar ortam
değişkeninden okunur; dosyaya yazılmaz ve `/v1/models` listesine `kervan/<ad>`
olarak girer. Ayrıntı ve ad çakışma kuralları `kervan.toml.example` içinde.

Bilinen sınırlama: istemci bağlantıyı kopardığında tarayıcı yolu üretimi
durdurulur, ama gider durumdaki bir REST çağrısı iptal edilemez (bloklayan
HTTP istemcisi); çağrı `zaman_asimi`'na kadar sürer ve sonucu çöpe gider.
Kalıcı çözüm eşzamansız istemciye geçmektir; bilinçli olarak ertelendi.

### Yönlendirme

`model: "auto"` üç kaynağı yarıştırır: bağlı eklenti sekmeleri, tarayıcı havuzu
ve `[rest.*]` hedefleri. Strateji `[yonlendirme]` ile seçilir, çalışma anında
`GET`/`POST /yonlendirme` ile okunur ve değiştirilir. 20 ad kabul edilir;
üretimde sırayı fiilen değiştiren yalnız 7'si (`weighted`, `round-robin`, `p2c`,
`random`, `strict-random`, `least-used`, `lkgp`). Kalan 13'ü: 8'i henüz
taşınmadı (açılışta uyarı), 5'i uyarısız ama etkisiz (`cost-optimized`,
`context-optimized` — fiyat/bağlam tablosu boş; `priority`, `fill-first`,
`context-relay` — çekirdek sırayı dokunmaz, adaylar zaten alfabetik gelir).
Liste ve gerekçeler `kervan.toml.example`'da. Araç (`tools`) içeren isteklerde
REST adayı varsa tarayıcı adayları elenir.

## 🗺 Durum ve Yol Haritası

| Durum | Konu |
|---|---|
| ✅ Kurulu | Çok sağlayıcı, çok hesap, hız bütçesi, insan kapısı |
| 🔬 Deneysel | Kota öğrenme (gözlemlenen kota sınırları) |
| 📅 Gelecekte | `kervan.toml` şablonları, sağlayıcı başına kota öğrenme |

### Teknik Detaylar

| | |
|---|---|
| Tarayıcı | Firefox tabanlı (Zen ya da Firefox), geckodriver ile; ya da `eklenti/` WebSocket köprüsü |
| Uç | `POST /v1/chat/completions`, `GET /v1/models`, `GET /health` |
| Havuz | `GET /accounts`, `POST /accounts/{i}/devam`, `POST /accounts/{i}/duraklat` |
| Yönlendirme | `GET /yonlendirme`, `POST /yonlendirme` |
| Eklenti | `GET /ws/eklenti` (token + Origin), `GET /eklenti/token`, `GET /eklenti/sekmeler` |
| Bağlanma | yalnız `127.0.0.1` |

Tam uç kayıt defteri: `port/uclar.toml` (`/telemetri`, `/bakiye`, `/kimlikler`,
`/kota` ve hesap görünürlük uçları dâhil).

### Sağlayıcılar

Tanımlı: `gemini`, `kimi`, `copilot`, `copilot-m365`, `perplexity`, `claude`, `deepseek`, `qwen`, `z.ai`. Her hesap `kervan.toml`'da bir ya da birkaç sağlayıcıya bağlıdır.

#### Sağlayıcı Durumu

<!-- DURUM:BASLA -->
> Bu blok araclar/kapilar/durum-uret.ts çıktısıdır; elle düzenlemeyin. Kaynak: port/durum.json

- Faz: P3 (2026-10-22T12:26:54.082Z)
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