# Kervan

Tarayıcı sürülü, kota öğrenen AI ağ geçidi. Rust, tek ikili dosya.

> Çok yoldan yük taşıyan, hangi yolun ne kadar kaldırdığını bilen.

## Ne işe yarar

API anahtarı olmayan AI hesaplarını (Gemini, Claude, Perplexity web arayüzleri)
OpenAI uyumlu tek bir uca dönüştürür. Çerez kopyalamak yerine **gerçek tarayıcı**
sürer: bir kez giriş yaparsın, oturum profil dizininde kalır.

Var olan çözümlerden farkı üç noktada:

- **Hesap başına gerçek yalıtım** — her hesap kendi tarayıcı profilinde. Gizli pencere
  yetmez: aynı tarayıcı örneğindeki tüm gizli pencereler tek profili paylaşır.
- **Kalıcı oturum** — çerez yapıştırma yok, çerez döndüğünde yenileme yok.
- **İnsan kapısı** — doğrulama duvarına çarpıldığında sessizce düşmez; hesabı
  duraklatır, pencereyi öne getirir, sen çözersin. Bot tespitini yenmeye çalışmaz.

## Durum

Aşama 2 tamam: tek sağlayıcı (Gemini web), tek hesap, uçtan uca çalışıyor.

| | |
|---|---|
| Tarayıcı | Firefox tabanlı (Zen ya da Firefox), geckodriver ile |
| Uç | `POST /v1/chat/completions`, `GET /v1/models`, `GET /health` |
| Bağlanma | yalnız `127.0.0.1` |

Sırada: çok hesap havuzu, kota öğrenici, TUI, ek sağlayıcılar.

## Gereksinimler

- Rust 1.97+
- Firefox tabanlı bir tarayıcı (Zen desteklenir)
- [geckodriver](https://github.com/mozilla/geckodriver/releases) —
  `%LOCALAPPDATA%\kervan\bin\geckodriver.exe` altına ya da PATH'e

## Kullanım

İlk kez: görünür pencerede giriş yap (bir defalık).

```
cargo run -- ercan-gemini-1
```

Sonra ağ geçidi olarak koştur:

```
cargo run -- ercan-gemini-1 --headless --serve
```

Tek soru sorup çıkmak için:

```
cargo run -- ercan-gemini-1 --headless --sor "Mimar Sinan kimdir?"
```

### Ortam değişkenleri

| Değişken | Varsayılan |
|---|---|
| `KERVAN_BROWSER` | Zen → Firefox sırasıyla aranır |
| `KERVAN_GECKODRIVER` | `%LOCALAPPDATA%\kervan\bin\geckodriver.exe`, sonra PATH |
| `KERVAN_PROFILES` | `%LOCALAPPDATA%\kervan\profiles` |

## Bilinen sınırlama

OpenAI API durumsuz, tarayıcı sohbeti durumlu. v1 yalnızca **son kullanıcı mesajını**
gönderir; süreklilik tarayıcının kendi konuşma dizisinden gelir. Farklı konuşmalar aynı
diziye karışır — çok hesap havuzu geldiğinde her istemci oturumuna ayrı sekme verilecek.

## Güvenlik

Profil dizinleri çerez ve oturum içerir, `.gitignore`'da. Ağ geçidi yalnız
`127.0.0.1`'e bağlanır. Kervan tek başına oturum açamaz — doğrulama hep insandan geçer.
