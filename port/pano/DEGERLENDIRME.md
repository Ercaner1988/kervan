# Kervan durum panosu — değerlendirme yöntemi

Bu belge, panodaki yüzdelerin nereden geldiğini ve nereden **gelmediğini** yazar.
Sıra önemli: önce yöntem, sonra sayı.

## 1. İki katman, karıştırılmaz

| Katman | Nereden | Tekrar üretilebilir mi |
|---|---|---|
| **Ölçüm** | disk (dosya, satır, `port/*.toml`, `port/durum.json`), git, derlenmiş ikili | Evet — `bun araclar/pano/olc-topla.ts` her koşuda yeniden okur |
| **Değerlendirme** | `araclar/pano/varsayim.json` içindeki elle verilmiş yüzde, aralık, ağırlık | Hayır — bir yargıdır, tartışılabilir |

Panoda soldaki kutu ölçümü, sağdaki kutu değerlendirmeyi taşır. Üst kuşak bunu her açılışta hatırlatır.
Bu ayrım süs değil: aynı depoda `docs/devir-raporu` türü bir belge "%100" derken aynı veri dosyası
"%64" diyebiliyor; fark, ölçümle yargının aynı cümlede karışmasından doğuyor.

## 2. Neden iki payda

"Toplam işin yüzde kaçı" sorusunun iki makul cevabı var, çünkü "toplam iş" iki farklı şey olabilir:

- **OmniRoute tam portu** (`omni`): Kervan'ın çıktığı iddiadan — "OmniRoute'un bütün yüzeyi Rust'a
  taşınacak". Bu paydada 16 alanın hepsi sayılır, ağırlık toplamı **110**.
- **Kervan v1 manifestosu** (`v1`): Kervan'ın kendi v1 tanımı. `ARKA-UC-ISTEKLERI.md`'deki 7 arka uç
  bu tanımda yok (DALGA-2/3/4'e bırakılmış); ayrıca sağlayıcı alanı v1 hedefine göre daha olgun
  okunuyor. Paydada `arka-uc` düşer, `saglayici` %70 (62–78) olarak yeniden değerlendirilir;
  ağırlık toplamı **101**.

İki sayının birbirine yakın çıkması tesadüf değil: v1 paydası hem bir alanı (ağırlık 9) düşürüyor
hem de kalan en zayıf alanı yukarı çekiyor; iki etki neredeyse birbirini götürüyor.

**Hiçbir yerde tek bir "%X tamamlandı" iddiası yoktur.** İkisi de birer değerlendirmedir.

## 3. Alanlar nasıl puanlandı

Her alan için üç şey verildi:

1. **yüzde** — bugünkü tamamlanma düzeyi (0–100)
2. **alt / ust** — aynı yargının makul belirsizlik bandı
3. **durum** — `çalışıyor` / `kısmi` / `iskelet` / `yok`

`durum` bir süs değil: `çalışıyor` demek için o alanın uçtan uca çalıştığına dair kod veya ölçüm
kanıtı olmalı. `iskelet` = kod var, çalışan yol yok. `yok` = dosya bile yok.

Her alanın `gerekce` ve `kanit` alanları `varsayim.json` içinde durur ve panoda görünür — yani
"neden %40" sorusunun cevabı panonun kendisindedir.

## 4. Amaçlar neden ayrı

Alan yüzdeleri "bu parça ne kadar tamam" der. Amaç yüzdeleri "bunu yapmak istiyorduk, bugün
neredeyiz" der. Bir alan %88 olabilir (kalite kapıları) ama amaca katkısı dolaylıdır.

Beş amaç:

- **A1** tek arayüzden tarayıcı üzerinden her modele erişim
- **A2** var olan API'leri tek biçime indirmek
- **A3** kotayı görmek ve hesapları akıllı dağıtmak
- **A4** masaüstünde tek pano
- **A5** ekosistemin düğümü olmak

## 5. Ölçmediklerim — panonun en dürüst kısmı

Şunlar **ölçülmedi**, dolayısıyla yukarıdaki yüzdelerdeki payları bir yargıdır:

1. Tarayıcı sürücüsünün uçtan uca çalışması (bugün süreç ayakta değil)
2. "358 sağlayıcı" hedefinin gerçekliği — kodda 4 yerleşik sağlayıcı var
3. 7 arka ucun gerçek eforu — hiçbiri başlamadı
4. GUI'nin görsel doğruluğu (ekran görüntüsü karşılaştırması yapılmadı)
5. Tarayıcı eklentisinin canlı davranışı
6. REST yukarı akışın kayıpsızlığı (ölçüm var, ama tam parite yok)
7. 429 / cooldown davranışı

Bu maddelerden biri ölçüldüğünde ilgili alanın yüzdesi ve aralığı `varsayim.json` içinde
güncellenmelidir; pano kendiliğinden "düzelmez", çünkü yüzde ölçümden türetilmiyor.

## 6. Belge gerçekliği

Panoda ayrı bölüm: depodaki belgelerin iddiaları ile ölçümün çeliştiği yerler. Amaç suçlamak değil,
**hangi belgeye güvenilebilir** sorusunu görünür kılmak. Bugün bilinen çelişkiler:

- `PR.md` — kapı sayısı, mutant sayısı, sunucu satır sayısı, "main hiç commit almadı" iddiası
- `port/envanter.json` — OmniRoute JS envanteri; Kervan sayısı değil
- `port/durum.json` — 11 kapı yeşil ama defterde 4 planlanmış iş doğrulanmamış

## 7. Nasıl koşulur

```bash
# 1. veriyi topla
bun araclar/pano/olc-topla.ts

# 2. panoyu aç
bun araclar/pano/sunucu.ts          # http://127.0.0.1:8790/
```

Sunucu yalnız `127.0.0.1` / `localhost` bağlantısı kabul eder; Host başlığı listede değilse 403 döner.
Veri dosyası yoksa `/health` bunu söyler, panoda uyarı çıkar.

## 8. Sınır

Bu pano "Kervan ne kadar tamam" sorusuna **tek sayı** vermez. Verdiği şey:

- iki payda, ikisi de değerlendirme
- her alanın yüzdesi + aralığı + gerekçesi + kanıt yolu
- ölçülen sayıların ayrı ve tekrar üretilebilir listesi
- ölçülmemiş olanların açık listesi

Bu son madde olmadan ilk üçü yanıltıcı olurdu.
