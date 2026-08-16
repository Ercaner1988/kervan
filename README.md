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

## Sağlayıcılar

Tanımlı: `gemini`, `kimi`, `copilot`, `copilot-m365`, `perplexity`, `claude`, `deepseek`,
`qwen`, `z.ai`. Bunların hepsi **havuzda değil** — aşağıdaki tabloya bak. Her hesap
`kervan.toml`'da bir ya da birkaç sağlayıcıya bağlıdır; havuzda hepsi yan yana durabilir.
Model kimliği `kervan/<sağlayıcı>`:

```
curl http://127.0.0.1:20129/v1/models
```

Yalnızca **hesabı olan** sağlayıcılar listelenir — çalışmayan bir modeli katalogda
tutmak istemciye var olmayan bir yol göstermek olurdu. İstenen model tanınmazsa istek
sessizce başka bir modele düşmez, 400 alır.

Model kimliği sürüm eki taşıyabilir: `kervan/gemini-3.1-pro` de `gemini`'ye gider,
böylece var olan Hermes yapılandırması kırılmaz.

### Seçicilerin durumu — dürüst tablo

| Sağlayıcı | Durum |
|---|---|
| `gemini` | ✅ Havuzda — 7 hesap, 7/7 doğru, ~11 sn |
| `kimi` | ✅ Havuzda — 7 hesap, 3/3 doğru, 25–36 sn. Girişsiz de cevaplıyor, o yüzden giriş işareti güvenilmez |
| `copilot` | ✅ Havuzda — 1 hesap (kişisel Microsoft), ~12 sn |
| `copilot-m365` | ✅ Havuzda — 1 hesap (iş/okul), ~14 sn |
| `perplexity` | ⚠️ Yalnız 1 hesapta — diğer altısında Cloudflare. Kararsız |
| `claude` | ❌ Cloudflare — havuzda kapalı |
| `deepseek` | ❌ Cloudflare — 7 hesabın hepsinde |
| `qwen` | ❌ Cloudflare — 7 hesabın hepsinde |
| `z.ai` | ❌ Cloudflare — belirtisi farklı: duvar görünmez, istem gider, asistan balonu **boş** kalır. 1/7 |

`copilot` (kişisel Microsoft hesabı) ile `copilot-m365` (iş/okul) **ayrı ürün**: ayrı
adres, ayrı kota, ayrı oturum. Model kimliği eşleşmesi içerme + en-uzun-ad kuralıyla
çalıştığı için `kervan/copilot` kişisele, `kervan/copilot-m365` iş hesabına gider.

### Cloudflare duvarı — hangi sağlayıcılar neden dışarıda

Ölçüldü (2026-08-16, yedi Google kimliğinin **hepsinde**): claude, deepseek, qwen ve
z.ai, WebDriver ile sürülen tarayıcıda Cloudflare doğrulamasına takılıyor. Claude'da
duvar elle çözülse bile bir sonraki açılışta yeniden çıkıyor.

Sebep açık: Cloudflare'in verdiği `cf_clearance` izni tarayıcının parmak izine bağlı,
WebDriver'la sürülen Firefox ise kendini otomasyon olarak bildiriyor
(`navigator.webdriver`), dolayısıyla izin yapışmıyor. Takılmayanlar Google (gemini),
Microsoft (copilot) ve Moonshot (kimi) — yani sorun sağlayıcıya değil, **Cloudflare +
WebDriver** bileşimine özgü.

Bunu yapıştırmanın tek yolu otomasyon imzasını gizlemek, yani **parmak izi
sahteciliği** — planın "yapılmayacaklar" listesinin ilk maddesi. Duvarı aşmıyoruz,
gösteriyoruz. Bu hesapları havuzda tutmak sonuç vermeden aynı siteye tekrar tekrar
otomatik istek atmak olurdu; hem boşuna, hem o hesapları gereksiz yere işaretletir.

`z.ai` ayrıca **yanıltıcı**: duvar görünmüyor, istem gidiyor, asistan balonu açılıyor
ama boş kalıyor — yani "yavaş" gibi görünüp aslında engelli. Havuz üzerinden 0/3,
toplam 1/7. Her istek 120 saniye boşa yakıyordu.

Sağlayıcı tanımları kodda duruyor. Duvar kalkarsa `kervan.toml`'da `providers` listesine
adı eklemek yeterli.

Seçiciler koda gömülü değil. Bir sağlayıcıda hesap açtıktan sonra ölç:

```
cargo run -- <hesap> --saglayici claude --headless --dom
```

Çıktı, sayfadaki aday seçicileri ve `data-testid` değerlerini sayar. Doğru olanı
`kervan.toml`'a yaz, yeniden derleme gerekmez:

```toml
[providers.claude]
response = "[data-testid='assistant-turn']"
login_hint = "a[href*='/login']"
```

Bilinen tuzak: `login_hint` yanlışsa `detect_login` istem kutusuna bakıp giriş
yapılmamış hesaba "girişli" der. Gemini'de bu ölçülüp düzeltildi; diğer ikisinde
`login_hint`'i doğrulamadan güvenme.

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

### Profil = kimlik, sağlayıcılar sekme

Bir Google oturumu, o profildeki **bütün** sağlayıcılara yetiyor: Gemini'ye girilmiş bir
profilde Perplexity'ye "Continue with Google" ile şifresiz giriliyor (ölçüldü). Bu yüzden
profil sağlayıcı başına değil **kimlik** başına açılır; her sağlayıcı o tarayıcıda kendi
sekmesinde oturur.

```toml
[[accounts]]
name = "ercan-gemini-3"          # profil dizini = bir Google kimliği
providers = ["gemini", "perplexity"]
```

- 3 kimlik × 3 sağlayıcı = 9 değil **3** tarayıcı.
- Sekme yerine aynı sekmede adres değiştirmek Gemini'de sohbeti sıfırlar, yani
  konuşma sabitlemesini kırardı.
- Kota ve hız bütçesi *(kimlik, sağlayıcı)* başına ayrı: Gemini'nin tavanı dolunca
  aynı profildeki Perplexity durmaz.

### Hangi profilde kim var — `--tara`

Profil adı kullanıcının *niyetinden* türer, gerçekte giren hesaptan değil. Elle girişte
sıralar karışabiliyor. Ölçerek öğren:

```
cargo run -- --tara --headless
```

Her profili tek tarayıcıda açar, sağlayıcıları sekmelerde yoklar, Google hesabını
**sayfadan okur** ve `kervan.toml` için hesap bloğunu basar. Hiçbir istem göndermez —
kota doluyken de güvenle çalışır.

### Gizli kip ve pencereyi öne çekme

Gizli kip **headless değil**: pencere gerçek, sadece ekran dışında duruyor.

- Headless bir tarayıcı çalışırken görünür hâle **getirilemez**, dolayısıyla
  "istediğim an öne çek" headless'la mümkün değil.
- Pencereyi **küçültmek** de olmuyor: küçültülmüş pencerede tarayıcı çizimi kısılıyor
  ve Gemini'nin yanıt balonu DOM'a hiç düşmüyor (ölçüldü: 120 sn zaman aşımı; aynı
  istem görünür pencerede 5.9 sn).

```
curl -X POST http://127.0.0.1:20129/accounts/3/goster
```

Pencereyi öne getirir — model/düşünme düzeyi seçmek, bir istemi durdurup düzeltmek
ya da doğrulama çözmek için.

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
bosta_dakika = 15         # bu kadar dokunulmayan tarayıcı kapatılır; 0 = hiç kapatma

[[accounts]]
name = "ercan.er@medeniyet.edu.tr"   # profil dizini = hesabın kendisi
providers = ["gemini"]               # aynı profilde birden fazla sağlayıcı olabilir

[pacing]
min_delay_ms = 3000       # aynı hesaba iki istek arası en az süre (bitişten sayılır)
jitter_ms = 2000          # üzerine rastgele sapma; sabit aralık en belirgin bot izidir
hourly_cap = 30           # hesap başına saatlik tavan; 0 = sınırsız (önerilmez)
```

Dosya yoksa tek hesaplı muhafazakâr bir varsayılanla çalışır. `pacing` bloğu eksikse
"sınırsız" değil, güvenli varsayılan uygulanır.

### Aynı sağlayıcıya aynı anda tek istek

Havuz varsayılan olarak her sağlayıcıya **sırayla** gider: sekiz hesap tanımlı olsa da
aynı siteye aynı anda yalnız bir oturum bağlanır.

Bu bir performans tercihi değil, ölçülmüş bir hasarın karşılığı. 2026-08-16'da sekiz
hesaplık havuz claude'u denerken üç hesaba aynı anda gitti; dakikalar içinde aynı IP'den
aynı Cloudflare sitesine yedi taze oturum açıldı ve Cloudflare tam o dakikada interaktif
sınamaya geçti (çerezlerde `cf_chl_rc_ni` 14:22'de belirdi). Oysa 11:25'te aynı
WebDriver'lı tarayıcı sorunsuz `cf_clearance` almıştı — **yani sorun otomasyon imzası
değil, ani yoğunluktu.**

Tek bir insan aynı siteye aynı anda yedi hesapla girmez. Çok hesaplı olmanın amacı
eşzamanlılık değil **süreklilik**: biri tükenince sıradaki uyanır, hepsi birden değil.

Ölçüm (üç istek aynı anda gönderildi):

```
31x3  başla  0.0s  bitiş 16.1s  -> 93
42x5  başla  0.0s  bitiş 32.9s  -> 210
53x7  başla  0.0s  bitiş 48.6s  -> 371
```

Cloudflare arkasında olmayan bir sağlayıcıda hız gerekiyorsa açılabilir:

```toml
[providers.gemini]
eszamanli = 3
```

0 yazmak sağlayıcıyı kilitlerdi; en az 1'e yuvarlanır.

### Hesap sayısının tavanı — kod değil bellek

Havuzda kaç hesap olabileceğine dair kodda bir sınır **yok**; sınır makinenin belleği.
Ölçüldü: 7 kimlik aynı anda açıkken 21 süreç, **2.4 GB** (kimlik başına ~344 MB).

Bu yüzden `bosta_dakika` var: `bosta_dakika` süresince istek almayan bir kimliğin
tarayıcısı kapatılır, bir sonraki istekte yeniden açılır. Oturum diske yazılı olduğu
için kapatmak veri kaybettirmez — yalnız ~5 saniyelik yeniden açılma bedeli öder.
Böylece bellek maliyeti *tanımlı* hesap sayısıyla değil, *o an çalışan* hesap sayısıyla
orantılı olur.

Süpürücünün **dokunmadığı** iki durum var:

1. **İstek işleyen ya da insanın elindeki pencere.** `goster` ile öne çekilmiş bir
   pencere `gizle` denene kadar kapatılmaz. (Bu bir hatanın karşılığı: süpürücü bir
   kez kullanıcı elle giriş yaparken pencereyi altından kapattı.)
2. **Cloudflare arkasındaki bir sekmenin açık olduğu tarayıcı.** Her yeniden açılış o
   site için taze bir oturum, yani yeni bir bot-risk değerlendirmesi; bellek için doğru
   olan şey burada sınamayı davet ediyor. Hangi sağlayıcıların Cloudflare arkasında
   olduğu ölçüldü ve kodda işaretli; `[providers.x] cloudflare = false` ile ezilebilir.

### Ortam değişkenleri

| Değişken | Varsayılan |
|---|---|
| `KERVAN_BROWSER` | Zen → Firefox sırasıyla aranır |
| `KERVAN_GECKODRIVER` | `%LOCALAPPDATA%\kervan\bin\geckodriver.exe`, sonra PATH |
| `KERVAN_PROFILES` | `%LOCALAPPDATA%\kervan\profiles` |
| `KERVAN_CONFIG` | `kervan.toml` (çalışma dizininde) |

## Hermes bağlantısı

`~/.hermes/config.yaml` içinde `custom` sağlayıcı olarak duruyor:

```yaml
  - name: kervan
    base_url: http://127.0.0.1:20129/v1
    api_key: kervan-local        # Kervan doğrulama yapmıyor; yer tutucu
    model: kervan/gemini-3.1-pro
    models:
      - kervan/gemini
      - kervan/copilot
      - kervan/copilot-m365
      - kervan/perplexity
```

```
hermes -z "..." --provider custom:kervan -m kervan/gemini
```

Ölçüldü (2026-08-16): Hermes'in gerçek istemi **2768 karakter** (sistem istemi + araçlar).
İlk denemede tuş tuş yazım bunun 2697 karakterini kutuya sokabildi ve yazım koruması
isteği haklı olarak göndermedi. Uzun istemler artık tarayıcının kendi metin girme yolu
(`execCommand('insertText')`) ile tek seferde yazılıyor — 3003 karakterlik ölçümde
5.1 sn, doğru yanıt. Bu bir gizleme değil: editörün `input` olaylarını normal biçimde
tetikliyor, yalnızca tuşları teker teker göndermiyor.

## Süreklilik: devam eden dizi vs. boş dizi

OpenAI API durumsuz, tarayıcı sohbeti durumlu. Normalde süreklilik tarayıcının kendi
konuşma dizisinden gelir, o yüzden yalnızca **son kullanıcı mesajı** gönderilir.

Ama dizi boşsa — hesap değişti, kota bitti, tarayıcı boşta kapanıp yeniden açıldı — model
önceki turları hiç görmemiş olur. O durumda Kervan geçmişi **tohumlar**: istemcinin zaten
her istekte gönderdiği konuşmanın tamamı (sistem yönergesi dahil) tek bir istem olarak
yazılır, sonuna asıl soru eklenir.

Özet çıkarmaya gerek yok — geçmiş elimizde, atıyorduk. Elde olanı kullanmak ek model
çağrısından, ek kotadan ve özetleme kaybından ucuz.

Karar "sabit var mı" diye değil **"bu sekme o konuşmayı taşıyor mu"** diye veriliyor.
Sabitler bellekte, tarayıcı dizisi ekranda: boşta kapatma tarayıcıyı kapatınca sekme yok
olur ve yeniden açılışta sıfırdan bir sohbet başlar, ama sabit tablosunda kayıt durur. O
kayda bakıp "devam ediyoruz" deseydik geçmişsiz cevap alırdık.

Ölçüm — aynı konuşma üç sağlayıcıda, her biri sıfırdan dizide:

```
1. tur gemini              -> Tamam            (sayı: 6283)
2. tur kimi   (yeni dizi)  -> 6283
3. tur copilot(yeni dizi)  -> 12566
```

Tohum `24 000` karakterle sınırlı. Aşılırsa **en eski** turlar düşer (yakın bağlam
değerlidir) ve düşenlerin sayısı tohumun içine yazılır — sessiz kırpma yok.

### Çok satırlı istem

Bu editörlerde Enter **gönder** demektir. Tuş tuş yazımda her `\n` istemi ortasından
yollar; ölçüldü, 7 satırlık bir tohumda kutuda tam 7 karakter eksik çıktı (yazım koruması
yakaladı). Bu yüzden çok satırlı ve 500 karakterden uzun istemler tarayıcının kendi metin
girme yoluyla (`execCommand('insertText')`) tek seferde yazılıyor. Çok satırlı bir istemde
bu yol başarısız olursa Kervan **tuş yoluna düşmez, hata verir** — yarım göndermektense
hiç göndermemek doğru.

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
