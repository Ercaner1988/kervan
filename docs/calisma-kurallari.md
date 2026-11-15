# Kervan — Elle Sınama Kuralları

Tarih: 2026-11-15
Durum: Yürürlükte. Bu kurallar tarayıcı/eklenti davranışının **elle**
doğrulandığı her koşu için bağlayıcıdır. Otomatik testler (`cargo test`,
`bun test eklenti/test`) bu kapsamın dışındadır; onlar her koşuda zaten koşar.

Neden var: 2026-11-15'e kadarki elle sınamalar, sınama kurulumunun kendisi
ölçülmeden yapıldı. Sonuç: aynı hesap iki ayrı tarayıcı oturumunda açıkken
"çok hesap çalışmıyor" sanıldı, tekrar tekrar aynı sınama körlemesine
koşturuldu ve her koşu farklı bir değişkeni değiştirdiği için hiçbir sonuç
birikmedi.

## 1. Körlemesine tekrar sınama yasağı

Bir sınama başarısız olduğunda **aynı kurulumu tekrar koşma**. Sıra:

1. **Dur** — başarısızlığı tek cümleyle yaz (ne bekleniyordu, ne görüldü).
2. **Ölç** — hangi değişkenin şüpheli olduğunu belirle ve onu **tek başına**
   değiştir. Aynı anda iki değişkeni değiştiren koşu bilgi üretmez.
3. **Kanıt al** — sonucu dosyaya/görüntüye bağla (bkz. §2). Sözlü hatırlama
   kanıt değildir.
4. **Yaz** — koşunun sonucunu (başarılı ya da değil) `.harness/progress.md`
   veya ilgili rapora işle; "denedim, olmadı" tek başına kayıt değildir.

Üç koşudan sonra hâlâ başarısızsa sınama **durur** ve kök neden koda bakılarak
aranır. Sınama sayısını artırmak, hatayı görünür kılmaz.

## 2. Sınama öncesi masaüstü kontrolü ve ekran görüntüsü

Sınama başlatmadan **önce**:

- Açık tarayıcı pencerelerini say ve hangi profilde olduklarını not et
  (`gemini-hesap.ps1 -Durum` bunu söyler).
- Aynı hesabın başka bir pencerede/ profilde açık olmadığını doğrula.
  İki yerde açık bir oturum, "iki hesap" gibi görünen ama tek hesap olan
  sahte bir sonuç üretir.
- Kervan'ın koştuğunu doğrula: `curl http://127.0.0.1:20129/eklenti/sekmeler`
  → `sayi` beklenen değerde mi?

Koşu **sırasında** her kritik adımda ekran görüntüsü al
(`Win+Shift+S` yeterli). Görüntü, sonradan "orada iki sekme vardı" gibi
doğrulanamaz iddiaların tek panzehiri.

Koşu **sonunda** sonucu şu üçlüyle yaz: ne yapıldı / ne görüldü / ne beklenirdi.

## 3. Her Google oturumu AYRI Zen private profili

Sebep: Kervan'da hesap anahtarı `provider:profile`'dur ve kimlik
`kervan-profil.json`'dan gelir. Aynı profilde iki Google hesabı açmak
mümkün değildir; farklı profiller ise aynı `cookieStoreId`'yi
(`firefox-default`) bildirdiği için kimlik dosyası olmadan TEK kayda çöker
(`eklenti/kimlik.js:1-12`, 2026-10-24 ölçümü).

Kural:

- Her hesap → kendi Zen profili: `C:\Program Files\Zen Browser\zen.exe
  -profile "%LOCALAPPDATA%\kervan\profiles\<hesap>"`. Bunu elle yazma;
  `gemini-hesap.ps1 -Ac <hesap>` kullan.
- Profil dizinindeki `kervan-profil.json` (`{"ad": "<hesap>"}`) o profilin
  kimliğidir. **Silme**; eklenti güncellenirken korunur (yalnız `*.js` +
  `manifest.json` kopyalanır).
- Sınama için "private/özel pencere" kullanacaksan her oturum **ayrı**
  private pencere olmalı; aynı private pencerede ikinci hesaba geçmek
  önceki oturumun çerezlerini taşır ve ölçümü bozar.

## 4. Aynı oturumda farklı sağlayıcı sınama yasağı

Bir Zen profilinde aynı anda birden çok sağlayıcı sekmesi açmak, hesap
eşlemesini okunamaz hâle getirir: iki sekme aynı `provider:profile`
anahtarını taşıyorsa eşleme hangi sekmeye gittiğini konsoldan ayırt etmek
güçleşir (bu belirsizlik 2026-11-15'te `eklenti/background.js` içinde
yapışkan seçim düzeltmesinin konusuydu).

Kural: bir koşuda **tek sağlayıcı**. Sağlayıcıyı değiştireceksen önce
sekmeleri kapat, `curl .../eklenti/sekmeler` ile `sayi`'nin düştüğünü gör,
sonra yenisini aç.

Ayrıca hesapları **tek bir Google hesabına bağlama**: bir sınama için açtığın
profilde, o koşuya ait olmayan hesapla oturum açma. Karışan oturum, sonraki
koşularda "hangi hesap nerede" sorusunu cevapsız bırakır.

## 5. Kapsam ve atıflar

- M2 (çok hesap) ve M3 (OpenChamber paneli) adımları:
  `.harness/kullanici-dogrulama.md`.
- Profil açma/kapama aracı: `gemini-hesap.ps1` (`-Durum`, `-Ac`, `-Kapat`,
  `-Onar`, `-Hepsi`).
- Çok hesap mimarisi ve ilkeler: `docs/plan/2026-10-19-tek-yerden-yonlendirme-plani.md`.
- Otomatik kapılar: `cargo test -p kervan-eklenti -p kervan-sunucu`,
  `bun test eklenti/test`, `kapi.exe . --guvenilen-kod`.
