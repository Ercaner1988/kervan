# Kervan

Web üzerinden yapay zekâ kullanımını istediğiniz arayüzlere yönlendirmeyi sağlayan bir
geçit altyapısı. Rust, tek ikili dosya.

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

Aşama 3 (havuz kısmı) çalışıyor: tek sağlayıcı (Gemini web), **çok hesap**, hız bütçesi,
insan kapısı.

| | |
|---|---|
| Tarayıcı | Firefox tabanlı (Zen ya da Firefox), geckodriver ile |
| Uç | `POST /v1/chat/completions`, `GET /v1/models`, `GET /health` |
| Havuz | `GET /accounts`, `POST /accounts/{i}/devam` |
| Bağlanma | yalnız `127.0.0.1` |

Sırada: TUI, ek sağlayıcılar.

## Kota öğrenici

Web sağlayıcıları kalan kotayı söylemiyor. "Tam kullanım hakkım ne?" sorusu ancak
ölçülerek cevaplanır: Kervan her isteğin sonucunu `%LOCALAPPDATA%\kervan\kota.jsonl`
dosyasına yazar, limite çarpma anlarını işaretler ve iki sayıyı gözlemden çıkarır —
pencere başına kaç istek, pencere kaç dakika.

```
curl http://127.0.0.1:20129/kota
```

- **Uydurma yok.** Gözlenmiş bir limit çarpması olmadan tahmin üretmez; `kanit: 0`
  ise iki alan da `null` döner. "Henüz bilmiyorum" geçerli bir cevaptır.
- Limit mesajı **yanıt olarak dönmez** — istemci 502 alır. Yoksa Hermes "limitine
  ulaştınız" cümlesini modelin cevabı sanardı ve öğrenici olayı hiç görmezdi.
- Limite çarpan hesap duraklatılmaz (insan çözecek bir şey yok), öğrenilen pencere
  kadar bekletilir. Pencere henüz bilinmiyorsa `kota.bilinmeyen_pencere_dk`.
- Defter **ekleme-yalnızca JSONL**: yarım yazılmış son satır atlanır, dosya kendini
  onarır. SQLite'ın C bağımlılığı ve bozulma riski bu hacim için gereksiz.
- Başarılı yanıtların içeriği **asla** yazılmaz. Yalnız limit mesajından 200 karakterlik
  kesit saklanır — sağlayıcının gerçek ifadesini öğrenmek için.

Limit ifadeleri yapılandırmadan gelir, çünkü Gemini'nin gerçek metnini henüz
gözlemlemedik. Gördüğünde ekle, kod değişmesin:

```toml
[kota]
isaretler = ["reached your limit", "sınırına ulaştınız"]
bilinmeyen_pencere_dk = 60
```

### Havuz nasıl davranır

- Oturumlar **tembel** açılır — 5 hesap tanımlamak 5 tarayıcı başlatmak değil.
  Bir hesap ilk kez seçildiğinde açılır, sonra açık kalır.
- Yönlendirme **en az kullanılanı** seçer, ama önce hız bütçesine bakar: boşta olsa
  bile penceresi dolmuş hesap seçilmez.
- Hız bütçesi istek **bittiğinde** başlar, başladığında değil. İnsan da cevabı okuyup
  sonra yazar.
- Hesap açılırken oturum durumu ölçülür. Giriş yapılmamışsa hesap duraklatılır,
  istek başka hesaba düşer — havuzun kalanı bundan etkilenmez.
- **Bir konuşmanın tüm turları aynı hesaba gider.** Tarayıcı sohbeti durumlu olduğu
  için ikinci tur başka hesaba düşerse o hesabın dizisinde birinci tur yoktur.
- Üst üste 5 hata, sebebi ne olursa olsun insan kapısını açar. Ölü bir oturuma
  sonsuza kadar istek atmak hesabı boşuna yıpratır.
- Aynı profili iki süreç açamaz. Geçit bir hesabı tutarken o hesapla ikinci bir
  Kervan başlatırsan tarayıcının modal "zaten çalışıyor" penceresi yerine terminalde
  açık bir hata alırsın.
- Kervan ölünce tarayıcılar da ölür — zorla öldürülse bile (Windows job object).
  Sızan tarayıcı profili kilitli bırakıp bir sonraki koşuyu o modal pencerede asardı.

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

Sonra ağ geçidi olarak koştur — hesaplar `kervan.toml`'dan gelir, komut satırındaki
hesap adı sunucu kipinde kullanılmaz:

```
cargo run -- --headless --serve
```

Tek soru sorup çıkmak için:

```
cargo run -- ercan-gemini-1 --headless --sor "Mimar Sinan kimdir?"
```

### Toplu hesap kurulumu

Birden çok hesabı sırayla kurmak için:

```
cargo run -- --kur
```

E-postaları alt alta yazarsın, boş satır listeyi bitirir (ya da
`--kur --eposta a@x.com,b@y.com`). Kervan her hesap için sırayla:

1. Profil adını e-postadan türetir (`gemini-ad-alan-com`),
2. Zaten girişliyse **atlar** — yarım kalan kurulum baştan çalıştırılabilir,
3. Tarayıcıyı, o hesabın giriş ekranında **e-posta önceden doldurulmuş** olarak açar,
4. Sen şifreyi girip pencereyi kapatınca girişi doğrular,
5. Başarılı olanları `kervan.toml`'a ekler.

Sana kalan tek iş şifre ve iki adımlı doğrulama. **Kervan şifre saklamaz, yazmaz,
görmez** — bu bir eksiklik değil karar: giriş anı insan denetiminden geçer.

Doğrulanamayan hesap yapılandırmaya **yazılmaz**; özet hangisinin neden düştüğünü söyler.

### Tek hesap ekleme

```
cargo run -- ercan-gemini-2 --giris
```

Tarayıcı **otomasyon dışında** açılır. Giriş yap, pencereyi düzgün kapat (çerezler o
sırada diske yazılıyor), terminale dönüp Enter'a bas — Kervan girişi doğrular ve
`kervan.toml`'a ekleyeceğin satırı basar.

Neden otomasyon dışında: Google, WebDriver ile sürülen tarayıcıda oturum açmayı
engelliyor — *"Bu tarayıcı veya uygulama güvenli olmayabilir"*. Sayfayı **kullanmayı**
engellemiyor; zaten açılmış oturum otomasyonda sorunsuz çalışıyor. Engeli aşmanın
teknik yolu otomasyon imzasını gizlemek olurdu; Kervan bunu yapmaz (bkz. Güvenlik).
Bunun yerine giriş gerçekten bir insan tarafından, gerçekten sıradan bir tarayıcıda
yapılır.

Girişi yapılmamış bir hesap havuzu bozmaz: ilk seçilişinde duraklatılır, istek başka
hesaba düşer. Sonradan elle çözdüysen:

```
curl -X POST http://127.0.0.1:20129/accounts/1/devam
```

### Yapılandırma (`kervan.toml`)

```toml
[[accounts]]
name = "ercan-gemini-1"   # profil dizini adı; aynı ad = aynı oturum
provider = "gemini"

[pacing]
min_delay_ms = 3000       # aynı hesaba iki istek arası en az süre (bitişten sayılır)
jitter_ms = 2000          # üzerine rastgele sapma; sabit aralık en belirgin bot izidir
hourly_cap = 30           # hesap başına saatlik tavan; 0 = sınırsız (önerilmez)
```

Dosya yoksa tek hesaplı muhafazakâr bir varsayılanla çalışır. `pacing` bloğu eksikse
"sınırsız" değil, güvenli varsayılan uygulanır.

### Ortam değişkenleri

| Değişken | Varsayılan |
|---|---|
| `KERVAN_BROWSER` | Zen → Firefox sırasıyla aranır |
| `KERVAN_GECKODRIVER` | `%LOCALAPPDATA%\kervan\bin\geckodriver.exe`, sonra PATH |
| `KERVAN_PROFILES` | `%LOCALAPPDATA%\kervan\profiles` |
| `KERVAN_CONFIG` | `kervan.toml` (çalışma dizininde) |

## Bilinen sınırlama

OpenAI API durumsuz, tarayıcı sohbeti durumlu. v1 yalnızca **son kullanıcı mesajını**
gönderir; süreklilik tarayıcının kendi konuşma dizisinden gelir.

Konuşmayı hesaba sabitlemek bunu çalışır kılıyor: **ilk kullanıcı mesajının** özeti
konuşma kimliği sayılır (her turda gönderildiği için sabit kalır) ve o konuşma hep aynı
hesaba yönlendirilir. Sistem istemi kasten ize katılmaz — içine tarih/saat gibi her
istekte değişen şeyler giriyor, katılsaydı sabitleme sessizce işlemez olurdu.

Kalan sınırlar:

- Aynı cümleyle başlayan iki ayrı konuşma aynı ize düşer. Sonucu yalnızca aynı hesaba
  yönlenmeleridir; yanlış yanıt değil, tek tarayıcı dizisinde karışmalarıdır.
- İstemci geçmişi yeniden yazarsa (Hermes'te compaction) iz değişir, konuşma başka
  hesaba taşınabilir.
- Sabitlenen hesabın saatlik tavanı dolarsa süreklilik yerine yanıt vermek seçilir:
  konuşma başka hesaba geçer ve bu günlüğe yazılır (sessizce değil).
- Sabitler bellekte tutulur; yeniden başlatınca düşer.

`usage` alanı bilerek sıfır döner: tarayıcı arayüzü token saymıyor, uydurma sayı kota
hesabını bozar.

## Seçici kırılınca

Site arayüzü değişince seçiciler kırılır. Ne eşleştiğini tahmin etmek yerine ölç:

```
cargo run -- ercan-gemini-1 --headless --sor "Sadece 'elma' yaz." --dom
```

Her seçicinin kaç öğe eşlediğini ve sonuncusunun metnini basar.

## Güvenlik

Profil dizinleri çerez ve oturum içerir, `.gitignore`'da. Ağ geçidi yalnız
`127.0.0.1`'e bağlanır. Kervan tek başına oturum açamaz — doğrulama hep insandan geçer.

**Yapılmayanlar.** Kervan insan hızında davranır (gecikme, sapma, saatlik tavan,
üstel geri çekilme, kalıcı oturum) ama bot tespitini yenmeye çalışmaz: parmak izi
sahteciliği yok, CAPTCHA çözme yok, vekil/IP döndürme yok, otomasyon imzasını gizleme
yok. Doğrulama duvarına çarpınca hesap duraklatılır ve duvar kullanıcıya gösterilir.
Google'ın giriş engeli de bu yüzden aşılmaz, `--giris` ile *etrafından* dolaşılır:
giriş otomasyonun dışında, gerçek bir insan tarafından yapılır.

## Lisans

MIT — bkz. [LICENSE](LICENSE).
