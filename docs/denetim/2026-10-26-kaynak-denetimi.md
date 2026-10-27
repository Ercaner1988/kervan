# Kaynak ve iş akışı denetimi — 2026-10-26

Ölçüm anı: 2026-10-26. Depo: `Ercaner1988/kervan`, dal `main`,
HEAD `ac1989f120f17a16c806bfe130caf809d8f9780e` (= `origin/main`).
Çalışma ağacı bu denetim sırasında KİRLİYDİ (21 izlenen dosya değişik,
20 izlenmeyen girdi). Aşağıdaki her hüküm bir komut çıktısına dayanır;
ölçülemeyen satırlar "ölçülmedi" olarak işaretlidir.

Sınıflar:
- **DOĞRULANMIŞ SORUN** — komutla kanıtlandı, düzeltme dar kapsamlı.
- **RİSK / İYİLEŞTİRME** — çalışmayı bozmuyor ama sessiz hata üretme yolu var.
- **İNSAN KARARI** — ajanın yetkisi dışında (imza, onay, kayıt silme).

---

## 1. DOĞRULANMIŞ SORUN — CI tarayıcı eklentisini hiç ölçmüyordu

**Kanıt (önce):** `.github/workflows/ci.yml` yalnız üç adım koşuyordu:
`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets
--all-features -- -D warnings`, `cargo test --workspace --all-features`.
`eklenti/test` paketi (Bun) hiçbir adımda yoktu.

**Ölçüm:** `bun install --frozen-lockfile` → 22 kurulum, değişiklik yok;
`bun test` (`eklenti/test`) → **26 pass / 0 fail**, 73 expect, 6 dosya,
~50 sn, çıkış 0. Yani paket sağlıklı, ama CI onu görmüyordu: içerik betiği
(menü okuma, seçim doğrulama, yeniden bağlanma) kırılsa CI yeşil kalırdı.

**Düzeltme (yapıldı):** `ci.yml`'e `oven-sh/setup-bun@v2` +
`bun install --frozen-lockfile` + `bun test`, `working-directory: eklenti/test`.
`kapi` kitinin `ci.yml`deki `run: cargo …` satırlarını okuduğu için mevcut
üç satırın biçimi korundu; yeni adımlar sonraya eklendi.

**Ölçülemeyen:** GitHub Actions üzerinde koşmadı (bu makinede CI yok).
Yerelde aynı üç komut yeşil; YAML adımlarının sırası okundu, çalıştırılmadı.

---

## 2. DOĞRULANMIŞ SORUN — `/kervan/secenekler` ölçümü yok sayıyordu

**Kanıt (önce):** `secenekler` yalnız `kervan_secim::secenek_kaydi(s.model, …)`
döndürüyordu; yani panel "kervan.toml ne diyorsa" onu görüyordu. Oysa aynı
gün ölçülen defter (`%LOCALAPPDATA%\kervan\olcum.json`, 7 hesap) şunu
gösteriyor: `gemini` için `kervan.toml` "dusuk/orta/yuksek + kilitli 3-1-pro,
4-argon" diyor, gerçekte üç hesapta YALNIZ `otomatik` seçilebiliyor, iki
Eğitim hesabında düşünme ekseni hiç yok, iki Pro hesapta dört model var.
Panel bu farkı göremiyordu.

**Düzeltme (yapıldı):**
- `crates/sunucu/src/olcum.rs`: `panel_kaydi(kayit, olculen)` — yapılandırma
  listesi `kaynak = "yapilandirma"` etiketiyle (VARSAYIM olduğu açık) ve her
  hesabın ölçülmüş kataloğu + özellikleri `olcum.hesaplar` altında ayrı verilir.
  Ölçüm yoksa `olcum` alanı **hiç eklenmez** (boş alan "ölçüldü, bir şey yok"
  diye okunurdu).
- `panel/main.js`: ölçüm varsa model listesi `<optgroup>` ile ayrılır —
  "yapılandırma (varsayım)" ve "ölçüldü · hesap". Seçenek kimliği
  `data-kimlik` taşır; istek gövdesi artık kodu yeniden kurmuyor, bu kimliği
  kullanıyor (hesap eki `@profil` böylece korunur).
- 5 sn'lik tazeleme imzasına ölçüm defteri de girdi: yeni ölçüm panelde
  görünür, eski liste takılı kalmaz.

**Testler (yeni, `kervan-sunucu` lib):** `panel_kaydi_olcumu_ayri_tasir`,
`panel_kaydi_olcum_yoksa_alan_eklemez`,
`panel_kaydi_yapilandirma_yoksa_olcumu_yine_verir`,
`panel_kaydi_kilit_olcumden_gelir`. Ayrıca defter tarafı:
`eski_defter_ozellik_alani_olmadan_okunur`,
`ozellikler_gidis_donus_ve_bos_yazilmaz`,
`birlestirme_olculmemis_alani_korur`,
`birlestirme_iki_alan_doluysa_yeniyi_secer`.

**Kanıt:** `cargo test -p kervan-sunucu --lib --all-features` → 29 passed,
0 failed. Tam paket: `cargo test --workspace --all-features` → `TEST5_EXIT=0`,
`test result: FAILED` sayısı 0.

---

## 3. DOĞRULANMIŞ SORUN (KAPANDI) — ölçüm defterinde özellik ekseni yoktu

Model ekseni ölçülüp yazılıyordu, araç menüsündeki özellikler (Derin
Araştırma, Canvas, Görüntü/Müzik oluştur) hiç kalıcılaşmıyordu. Bu yüzden
"Deep Research bu hesapta var mı" sorusu her turda yeniden ekran
görüntüsüyle cevaplanıyordu.

**Düzeltme (yapıldı):** defter şemasına `HesapOlcumu.ozellikler:
Vec<OlcuOzellik>` (`kod`, `etiket`, `isaretli`, `tam`) eklendi;
`/kervan/olc?ozellik=<seçici>&kaydet=1` ile yazılıyor.
`kaydet` artık **alan bazında** birleştiriyor: yalnız özellik ölçen bir tur,
o hesabın model listesini silmiyor; tersi de geçerli.
Eski defterler `#[serde(default)]` ile okunuyor; boş özellik listesi dosyaya
yazılmıyor (`skip_serializing_if`) — "ölçülmedi" ile "ölçüldü ve boş" ayrımı
dosyada görünür kalıyor.

**Sınır (dürüst):** "kilitli özellik" hükmü deftere YAZILMIYOR. Kilit
göstergesi menü metninden güvenilir okunamıyor; bu ancak tıklama denemesiyle
kanıtlanır ve o ölçüm (`dene=`/`tum=1`) henüz özellik paneli için yazılmadı.
Defter şu an "menüde ne göründü, hangisi işaretliydi" der; "açık/kilitli"
demez.

---

## 4. RİSK — `pre-push`, cargo bulunamazsa YEŞİL dönüyor

**Kanıt:** `.githooks/pre-push` satır 53-56:
`pre-push: cargo bulunamadi — kapi OLCULMEDI (yesil degil). CI son savunma.`
mesajı yazılıyor ama `exit 0`. `kapi` bulunamazsa ise `exit 1` (satır 76-80).

Yani kapı, en sık iki ortamdan birinde (GitHub Desktop'ın dar ortamı, ya da
cargo yolu PATH'te yok) hiçbir şey ölçmeden push'a izin veriyor. Mesaj
dürüst, ama çıkış kodu "geçti" diyor; `git push` çıktısında bu satır gözden
kaçarsa kullanıcı kapıdan geçtiğini sanır.

**Neden dokunulmadı:** Bu davranış bilinçli olabilir (GitHub Desktop'ı
kilitlememek). Sertleştirmek — `exit 1` — cargo'suz her push'u bloke eder.
Karar Ercan'ın: (a) olduğu gibi bırak, (b) `KERVAN_SKIP_GATE=1` gibi açık bir
onay iste, (c) `exit 1` yap. Önerim (b): ölçülmediyse push sürer ama
"ölçülmedi" çıktısı renkli/tek satır olur ve `kapi` yokluğuyla aynı muameleyi
görmez.

---

## 5. RİSK — `kapi.toml` boyut beyanı ile gerçek ölçüm arasında fark

`kapi.toml` 16 boyutun 15'ini "uygulanır" diyor; `veritabani = "uygulanmaz"`.
Ölçüm (`kapi .`, aynı dal, kirli ağaç):

- yeşil: `sandık` (13 crate sınır altında), `karakter yazımı`,
  `önyüz (egui)`, `tedarik`, `onyuz`, `tedarik_guvenligi`, `karakter_yazimi`
- **KIRMIZI: `hata_disiplini`** — `--guvenilen-kod` verilmediği için clippy
  koşmadı; bu bir yapılandırma kapısı, kod kusuru değil.
- ölçülmedi (8 boyut): `arkayuz`, `makine_zararsizligi`, `bellek_guvenligi`,
  `kimlik_yolu`, `yayina_hazirlik`, `kaynak_tuketimi`,
  `birlikte_calisabilirlik`, `test_gercekciligi`, `veri_dayanikliligi`,
  `surum_izi`, `belge_dogrulugu`, `mutasyon ve gerçek girdi`.

Yani "15 boyut uygulanır" beyanı, beyanın kendisiyle uyumlu ama **ölçümle**
uyumlu değil: bu oturumda 12 boyut ölçülmedi. Boyut beyanını "uygulanır"
yazmak, ölçülmemiş boyutu ölçülmüş gibi göstermiyor (kapi çıktısı "ölçülmedi"
diyor) — yine de bir okuyucu `kapi.toml`a bakıp hepsinin koştuğunu sanabilir.
Öneri: beyanı `uygulanir` yerine `olculur` / `olculmedi` ayrımıyla yaz, ya da
`--guvenilen-kod` ile tam tur koşup çıktıyı `port/kayit`e koy.

---

## 6. RİSK — `kervan-sunucu` sandık sınırını AŞIYOR (kırmızı)

**Kanıt (kapi):** `KIRMIZI sandık kervan-sunucu — 2239 satır; sınır 1500. Böl.`

**Kaynağı netleştirildi:** HEAD ağacının arşivi ayrı dizine açılıp aynı kapı
koşuldu: `yeşil sandık kervan-sunucu — 1481/1500 satır`. Yani HEAD sınırın
19 satır altındaydı. Bu oturumda crate'e eklenen `olc.rs` (467 satır ham) ve
`olcum.rs` (473 satır ham) sınırı aştı; ölçüm 1481 → 2239 (+758 kod satırı).

**Benim payım:** bu iki dosyaya eklediğim özellik ekseni + `panel_kaydi` +
birleştirme + testler ≈ 150 kod satırı. Sınır aşımının ana sebebi değil ama
sınırı daha da ileri taşıdı.

**Neden bölünmedi:** çözüm yeni bir crate (`kervan-olcum`) ve `olc`'nin
`AppState`ten ayrılması demek — yani `kervan-sunucu → kervan-olcum` yönünde
yeni bir sınır. Bu, "dar kapsamlı yerel düzeltme" değil, yapısal bir taşıma;
`secim` crate'inin doğuşuyla aynı sınıf iş. Karar Ercan'ın.

**Öneri (sırayla):** (1) `olcum.rs`'i olduğu gibi `crates/olcum`'a taşı —
axum bağımlılığı yok, mekanik; (2) `olc.rs`'teki saf çekirdeği
`olc_hesapla(hub, saglayicilar, sorgu)` olarak dışa aç, axum sarmalayıcısı
sunucu'da kalsın. (1) tek başına 2239 → ~1900 (hâlâ kırmızı); (2) ile
~1450 (yeşil, ~50 satır pay).

---

## 7. İNSAN KARARI — `muhur-kontrol` kırmızı (3 ihlal)

**Kanıt:** `bun araclar/kapilar/muhur-kontrol.ts` → `HUKUM: kirmizi 3 ihlal`:

1. `port/muhur.toml` içinde `onay_bekliyor = true` ve `onay` alanı boş.
   (İmzalı sürümde `onay_bekliyor = false`.)
2. `port/muhur.toml` imzalı mühür commit'indeki sürümden farklı.
   `git diff ae7991b HEAD -- port/muhur.toml` yalnız iki satırı değiştiriyor:
   `onay_bekliyor false → true` ve `onay` alanı. Mühürlü dosya listesi
   DEĞİŞMEMİŞ.
3. Mühür commit'i `ae7991ba1e518d65696e83188e2fc21ecff3ee99`
   (`2026-10-14 08:13:03 muhur: Ercan onayı`) HEAD'in atası değil:
   `git merge-base --is-ancestor ae7991b HEAD` → çıkış 1. Commit yalnız
   `dalga2/turso` ve `duzelt/guven-koku` dallarında.

**İmza durumu:** `muhur-onay` etiketi mevcut, `tag` nesnesi `9909c97`,
gösterdiği commit `ae7991b`, imzalayan `Ercan Er 2026-10-14`. Yani kırmızı
"imza yok" değil, "onay eski dalda, mühür dosyası HEAD'de yeniden üretilmiş".

**Gereken insan girdisi:** (a) `port/muhur.toml` onay alanlarını Ercan
doldurur (`onay_bekliyor = false`, `onay = "<ad, tarih>"`), (b) `ae7991b`
tabanı HEAD'e birleştirilir **veya** yeni mühür tabanı seçilir, (c) HEAD
üzerinde `git tag -s muhur-onay` yeniden imzalanır. Ajan imza üretmedi,
sahte mühür yazmadı.

---

## 8. İNSAN KARARI — `durum-kontrol` kırmızı (11 ihlal)

**Kanıt:** `bun araclar/kapilar/durum-kontrol.ts` → `[durum-kontrol] 11 ihlal`.
`port/kayit/P3/` altında yalnız `makine-sakin.log` var ve o da tamamlanmamış
(son iki satırı `KAPI BİTTİ` / `KOŞUCU HÜKMÜ` değil). Diğer 10 kapının
günlüğü diskte YOK. `port/durum.json` ise `2026-10-24T14:06:52.377Z` P3
koşusunu ve 10/11 yeşil sonucu iddia ediyor.

Yani durum dosyası, günlükleri artık var olmayan bir koşuyu raporluyor.
Bu oturumda `port/durum.json`, `port/DURUM.md`, `port/kayit/P3/kosu.json`
değiştirilmiş (izlenen, kirli) — ayrıştırılmadı, üzerine yazılmadı.

**Gereken insan girdisi:** ya tam bir P3 koşusu yeniden koşulup günlükler
üretilir, ya da `durum.json` dürüstçe "koşu yok" durumuna çekilir. Ajan
günlük uydurmadı, durum dosyasını "yeşil"e çekmedi.

---

## 9. Doğrulanmış ama düzeltilmeyen: iki tanık kuralının canlı yolu

`crates/secim/src/lib.rs` (1043 satır) baştan sona okundu. 400/501/402
ayrımı, `zaten_secili`/`secim_dogrulandi` güçlü-zayıf kanıt kuralları,
`hedef_indeks` üç geçişli eşleştirme ve iki tanıklı `model_sec` birim
testleriyle kapsanıyor (29 test `kervan-sunucu` lib'de, ayrıca `kervan-secim`
kendi testleri). **Kapsanmayan:** retry/iptal ve sekme kapanışı yollarının
CANLI davranışı — bunlar yalnız gerçek tarayıcıyla ölçülür, bu oturumda
ölçülmedi.

---

## Ölçüm özeti (komut → sonuç)

| Komut | Sonuç |
|---|---|
| `bun install --frozen-lockfile` (`eklenti/test`) | temiz, 22 kurulum |
| `bun test` (`eklenti/test`) | 26 pass / 0 fail, 6 dosya, ~50 sn |
| `bun build eklenti-openchamber/panel/main.js --target=browser` | 1 modül, 10 ms, çıkış 0 (sözdizimi temiz) |
| `cargo fmt --all -- --check` | çıkış 0 |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | çıkış 0 — 5 turda (5 + 1 + 2 hata düzeltildi); son tur `CLIPPY5_EXIT=0` |
| `cargo test -p kervan-sunucu --lib --all-features` | 29 passed, 0 failed |
| `cargo test --workspace --all-features` | son tur `TEST6_EXIT=0` (clippy düzeltmelerinden sonra), FAILED 0 |
| `kapi .` | sandık `kervan-sunucu` KIRMIZI; `hata_disiplini` ölçülmedi |
| `kapi .` (HEAD arşivi, ayrı dizin) | sandık `kervan-sunucu` yeşil 1481/1500 |
| `bun araclar/kapilar/muhur-kontrol.ts` | kırmızı, 3 ihlal |
| `bun araclar/kapilar/durum-kontrol.ts` | kırmızı, 11 ihlal |

## Ölçülmeyenler (iddia edilmedi)

- GitHub Actions üzerinde gerçek CI koşusu (bu makinede yok).
- Tarayıcı eklentisinin canlı uçtan uca davranışı (uzantı yenilemesi
  Ercan'ın onayına bağlı; `content_common.js` değişikliği devreye girmedi).
- Alt menü içeriği 7 hesabın yalnız 1'inde görüntüyle kanıtlandı; hover ile
  açılan alt menü `dene=` tıklamasıyla kapanıyor.
- `retry` / `iptal` / sekme kapanışı yollarının canlı davranışı.
- Copilot/Hotmail profili (`buzbey_@hotmail.com`) — ertelendi.
