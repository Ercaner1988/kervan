# Tek Yerden Yönlendirme ve Çok Hesap Planı

Tarih: 2026-10-19
Durum: Dilim A–D uygulandı. Ölçüm kayıtları: `.harness/progress.md` ve
`feature-list.json` (M0–M3). Kalan iki elle doğrulama: Dilim C'nin iki
`cookieStoreId`'li gerçek tarayıcı koşusu; Dilim D'nin gerçek OpenChamber
host'unda panel listelemesi (`.harness/kullanici-dogrulama.md`).

## 1. Amaç ve ilkeler

**Amaç:** Kervan'ı tek bir OpenAI uyumlu ağ geçidi yapmak; yerel
(anahtarlı, `[rest.*]`) ve web (oturumlu: tarayıcı) sağlayıcıları **aynı
yönlendirme kararında** kullanmak. OmniRoute'un bıraktığı ihtiyaç buydu;
OmniRoute bir aktarma hattı olarak reddedildi (karmaşıklık + kararsızlık),
genişlik ihtiyacı Kervan'ın kendisinde karşılanır.

**İlkeler:**

1. Arayüz kararı (2026-10-19): masaüstü GUI'si devre dışı. Kontrol yüzeyi
   OpenChamber uzantısı olacak; sunucu arayüzsüz koşar (`kervan.bat`,
   `baslat_kervan.ps1` güncellendi).
2. Hesap seçimi: **en-az-kullanılan** (onaylı ilke). Round-robin hesap
   düzeyinde istenmiyor; strateji çeşitliliği sağlayıcı düzeyinde
   (SmartRouter).
3. Açık sözlülük: yapılandırmaya yazılan ama davranışı bağlanmamış her alan
   belgelenir (bkz. `yedek_deneme`).

## 2. Bugünkü ölçülmüş durum

| Yol | Ne | Çok hesap | Not |
|---|---|---|---|
| REST (`[rest.*]`) | `kervan-yukari`, anahtar env'de, openai/claude/gemini biçimi | anahtar değil hedef | kendi devre kesicisi `kervan-direnc` |
| Havuz (Playwright) | profil başına tarayıcı (`tara.rs:83`), pacing + saatlik tavan | **evet**, `acquire_for` en-az-kullanılan (`pool.rs:474/550`) | kimlik mutex'i + sağlayıcı kapısı |
| Eklenti köprüsü | WebSocket `ws/eklenti`, tarayıcı uzantısı | **evet** — hub `provider:profile` anahtarlı (Dilim C) | ikinci oturum ayrı hesap; `hesap_sec` en-az-kullanılan |

Sunucu: model `kervan/<ad>`; `auto` → SmartRouter; sağlayıcı kapısı
(`kapilar`) bilinçli olarak sağlayıcı başına 1 eşzamanlı.

**Eski tutarsızlık (eklenti; Dilim C ile kapatıldı):** `background.js`
`TAB_REGISTER` eskiden yalnız `provider` ile yazıp `"sor"` aramasında
`cookieStoreId`'yi düşürüyordu; artık kayıt ve arama `provider:profile`
tam anahtarıyla çalışıyor (bkz. §5).

## 3. Dilim A — yönlendiricinin bağlanması (YAPILDI)

- `kervan.toml` `[yonlendirme] strateji = "lkgp"` (varsayılan); bilinmeyen ad
  açılışta reddedilir (`main.rs`, `from_str_canonical`, sesli hata).
- `GET /yonlendirme`: strateji, destek durumu, adaylar, 20 geçerli ad.
  `POST /yonlendirme {"strateji":"..."}`: çalışma anında değiştirir; yalnız
  kanonik ad, sessiz priority düşüşü yok.
- `auto` adayları üç kaynaklı: eklenti sekmeleri + havuz + REST (`server.rs`,
  `chat`). REST seçimi yukarı akışa devreder (`rest_gonder`).
- REST yolu artık başarı/hata sayacı yazıyor (`rest_gonder`): devre kesici
  ve LKGP tablosu REST hedeflerini de görür.
- `kervan.toml.example`, `kervan.bat`, `baslat_kervan.ps1` güncellendi.

**Kanıt:** `cargo test -p kervan-tarayici -p kervan-havuz -p kervan-sunucu`
(tümü ok; 3 yeni `[yonlendirme]` testi), `cargo test -p kervan` (p7 REST
testi ok), canlı duman: sunucu `--port 20131` açıldı, `GET /yonlendirme` →
`lkgp` + adaylar `[copilot, gemini]`, `POST round-robin` → `ok`, bilinmeyen
ad → `400`, kapanış temiz.

## 4. Dilim B — birleşik hedef sicili + yedek deneme (YAPILDI)

**Problem:** `chat` tek sağlayıcıya karar verip yola girer; o sağlayıcı kota
duvarına takılırsa istek hata döner — sıradaki adaya geçilmez. "Hepsini bir
anda kullanmak"ın asıl karşılığı budur.

**Tasarım:**

1. `sunucu` içinde `HedefKayit` sicili: `{ ad, kaynak: Rest|Havuz|Eklenti,
   saglik }`. Kaynak, `chat`'teki üç ayrı gönderme bloğunun yerine tek
   `dispatch(hedef, istem)` işlevini seçer.
2. Aday sıralaması SmartRouter'dan (`sirala` zaten kararlı sıra döndürür);
   `select_provider` yerine `aday_sirasi(&[String]) -> Vec<String>` benzeri
   bir tam sıra ucu eklenecek (`select_provider` ilk elemanı döner, gerisi
   atılıyor — bu sıra yedek denemenin sırasıdır).
3. Deneme döngüsü: `yedek_deneme = true` ise aday listesinde sıradakine
   geçilir; `azami_deneme` tavan. **Yalnız akış başlamadan önce** yeniden
   denenebilir; SSE akışı başladıktan sonra istemciye bayt gitmiştir,
   geri alınamaz — akışta yeniden deneme yok, açık hata döner.
4. Kota duvarı (`Sonuc::Kota`) ve devre kesici açıklığı yeniden denemeyi
   meşru kılan iki durumdur; kullanıcı iptali (`Durdur`) ve zaman aşımı
   yarım yanıtla (`YarimYanit`) yeniden deneme YAPMAZ — dikme mantığı
   (`stream_recovery`) aksin.

**Kabul:** bir sağlayıcının tüm hesapları kapalıyken `auto` isteği sıradaki
sağlayıcıdan yanıt dönerir (entegrasyon testi: sahte upstream + sahte havuz);
`yedek_deneme = false` davranışı bugünküyle birebir aynıdır (kayıt testi).

**Açık nokta (kapatıldı):** `[yonlendirme] yedek_deneme` ve `azami_deneme`
alanları Dilim A'da yapılandırmaya yazılmıştı; bu dilimle davranışa bağlandı
(`server.rs` `chat` döngüsü; `AppState.yedek_deneme/azami_deneme`).
Sapma kaydı: kabul ölçütündeki "tüm havuz hesapları kapalı" senaryosu,
sahte upstream'li REST adayları üzerinden ölçüldü (p9
`auto_ilk_aday_bozuksa_yedege_gecer`); havuz-kapalı yol ayrıca ölçülmedi.
Akış başladıktan sonra yeniden deneme yok (belge sözleşmede duruyor);
akış başarısı terminal olayda yazılır (`rest.rs` poll_fn + `akis.rs`).

## 5. Dilim C — eklenti köprüsünde çok hesap (YAPILDI — otomatik testler yeşil; elle tarayıcı doğrulaması bekliyor)

**Problem:** hub haritası sağlayıcı anahtarlı (`eklenti.rs:105`); aynı
sağlayıcının ikinci oturumu (ör. ikinci Google hesabı, Firefox container'ı)
birincinin kaydını ezer. `BagliSekme.profile` alanı var ama anahtara
girmiyor. Playwright yolu zaten doğru; eksik yalnız eklenti.

**Tasarım:**

1. **Anahtar:** `sekmeler` haritası `provider:profile` anahtarına geçer.
   `sekme_kaydet` imzası hazır (profile zaten parametre); `handle_eklenti_socket`
   (`server.rs:141-158`) ayrılışta anahtarı izlemeli (bugün yalnız
   `provider` tutuluyor).
2. **Seçim (sunucu tarafı):** `sor(provider, ...)` sağlayıcının TÜM bağlı
   profilleri arasından **en az kullanılanı** seçer — `BagliSekme`'ye
   `son_kullanim: Instant` eklenir, her `sor`'da güncellenir. Onaylanan
   ilke hesap düzeyinde de aynı: en-az-kullanılan.
3. **Model kimliği bozulmaz:** `bagli_sekmeler()` benzersiz sağlayıcı
   adlarını dönmeye devam eder (`/v1/models` ve `auto` bunu okur). Yeni
   `bagli_baglantilar()` ucu `(provider, profile)` listesini görünürlük
   için döner (`/eklenti/sekmeler`).
4. **Tel protokolü:** `Sor` mesajına `profile: Option<String>` eklenir
   (`#[serde(default)]` — eski eklenti ile uyumlu). Sunucu seçtiği profili
   yazar; `None` = herhangi biri (geriye uyumluluk).
5. **background.js:** anahtar her yerde `${provider}:${cookieStoreId}` olur;
   `TAB_REGISTER` `sender.tab.cookieStoreId`'yi alır (bugün düşüyor);
   `"sor"` gelirse `msg.profile` varsa tam anahtar, yoksa sunucu zaten
   profili yazmış olur — tam anahtarla bakılır, bulunamazsa yeniden tara,
   sonra `"durdur"` için `id -> tabId` haritası (çok sekmeli durumda
   hangi sekmenin duracağı `provider`'dan çıkmaz).
6. **manifest.json:** `tab.cookieStoreId` okumak için ek izin gerekmez;
   container hesap izolasyonu Firefox'un kendi düzeneğidir.

**Kabul:** iki container'da iki hesap açıkken `kervan/gemini` istekleri
profiller arasında en-az-kullanılan ölüsünce dönüşümlü gider (elle tarayıcı
testi — otomasyonu yok, eklenti tarafı elle doğrulanır); tek hesap açıkken
davranış bugünküyle aynı. **Durum:** Rust tarafı ölçüldü
(`en_az_kullanilan_hesap_seciliyor`,
`eski_eklenti_profilsiz_calismaya_devam_eder`; ayrıca S1–S6 sağlamlaştırma:
token/Host/Origin kapısı `p10_eklenti_kimlik_test`, istemci kopması Drop
guard'ı, 64'lü giden kanalı). İki `cookieStoreId`'li gerçek tarayıcı koşusu
bekliyor (`.harness/kullanici-dogrulama.md`).

**Risk:** köprü bugün tek hesapla ÇALIŞIYOR; protokol değişikliği kırarsa
günlük akış bozulur. Bu yüzden `profile` alanı opsiyonel ve geriye uyumlu
tutulur; JS tarafı elle doğrulanmadan "tamam" DENMEZ.

## 6. Dilim D — OpenChamber uzantısı (YAPILDI — panel + Node shim; gerçek host listelemesi bekliyor)

**Sınırlar (ölçülmüş):** OpenChamber uzantıları sağlayıcı kaydedemez,
transport'a karışamaz. `contributes.service` yalnız Node girişi başlatır
(`service/main.js`, host `execPath` + `ELECTRON_RUN_AS_NODE`), Rust
ikilisini host adına spawn etmez.

**Mimari:**

```
OpenCode ──HTTP──> Kervan (127.0.0.1:20129/v1)   ← sağlayıcı yolu SABİT kalır
Uzantı paneli ──serviceRequest──> service/main.js (Node shim) ──127.0.0.1:20129──> Kervan
```

- Node shim ince kalır: Kervan'a proxy + "kapalıysa başlat" yardımı
  (`exec` izni ile `kervan.bat`). Domain mantığı Kervan'da kalır.
- Panel işlevleri (hepsi Kervan'ın mevcut uçları): `/accounts` (duraklat/
  devam/göster/gizle), `/kota`, `/yonlendirme` (strateji seçici — Dilim A
  ucu), `/telemetri`, `/bakiye`, `/eklenti/sekmeler`.
- Model listesi: `/v1/models` zaten karışık yüzey (`kervan/gemini`,
  `kervan/<rest-ad>`, `auto`); OpenChamber'da Custom provider olarak
  `@ai-sdk/openai-compatible` + `baseURL http://127.0.0.1:20129/v1` (tüm
  modeller için `capabilities.tools: true` + `limit` dolu).

**Kabul:** panel, duraklatılmış hesabı gösterip devam ettirebiliyor;
strateji değişikliği panelde seçiliyor ve `/yonlendirme` GET'i onaylıyor;
Kervan kapalıyken panel "başlat" düğmesi süreci kaldırıp `/health` ucunu
yeşile çeviriyor. **Durum:** üç davranış da gerçek servis + gerçek Kervan
üzerinden ölçüldü (host taklidi); manifest OpenChamber'ın kendi
`parse.js`'iyle doğrulandı; uzantı `extensions.json`'a kayıtlı. Gerçek
OpenChamber host'unda panel listelemesi için uygulamanın yeniden yüklenmesi
bekliyor (`.harness/kullanici-dogrulama.md`).

## 7. Riskler ve açık doğruluklar

- **Araç taklidi:** tarayıcı yolunda araçlar isteme içine dikilir
  (`arac_yonergesi` / `arac_cagirilarini_ayikla`); agentic döngüler
  kırılgandır. REST hedeflerinde araçlar gerçektir. **Durum (Dilim B ile
  bağlandı):** araç (`tools`) içeren isteklerde REST adayı varsa tarayıcı
  adayları elenir; REST adayı yoksa tarayıcı yolu denenir ve uyarı basılır.
- **Sağlayıcı kapısı:** `kapilar` sağlayıcı başına 1 eşzamanlı. 8 gemini
  hesabı olsa da eşzamanlılık 1'dir; hesap başına kapı bu misyonda
  ele alınmadı (bilinçli tasarım, sessizce değiştirilmez). S2 yalnız
  bekleme disiplinini sağladı: semafor hesap seçildikten sonra alınır,
  kapıda bekleme 180 sn ile sınırlı.
- **Cloudflare duvarı:** claude/deepseek/qwen/z.ai eklenti yolunda engelli
  (README durumu); plan bu sağlayıcıların çalışırlığını vaat etmez.

## 8. Önerilen sıra

B önce: tamamen sunucu tarafında, mevcut entegrasyon test donanımıyla
ölçülebilir, tarayıcı/yeniden yükleme gerektirmez, "hepsini bir anda"
hedefinin özüdür. C ardından: protokol değişikliği + elle tarayıcı
doğrulaması ister. D en son: B ve C'deki uçlar hazır olduğunda panel
boş kalmaz.
