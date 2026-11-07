# Faz P3 Sonu Raporu: Kırmızı Takım Külliyatı, Mutasyon Kapısı, Üretilmiş Durum ve Defter Doğrulaması

- **Faz ID:** P3
- **Tarih:** 2026-10-14
- **Dal:** `duzelt/guven-koku`
- **Sorumlu:** Claude Sonnet 5.5 | **Onay:** Ercan (imzalı `muhur-onay` etiketi)

Bu rapordaki her sonuç cümlesi aynı satırdaki bir `hepsi.ts` kapı günlüğüne bağlıdır; günlüğü olmayan iddia yazılmamıştır. Bilinen sınırlar 4. bölümdedir.

---

## 1. Teslim Edilenler

1. **Külliyat taraması (FR-2.5):** `ac2f18d` teslimindeki 5 kusur sınıfı kırmızı yakalanır, temiz ağaç yeşildir. Kanıt: [kulliyat-tara.log](kulliyat-tara.log), kod: `araclar/kapilar/kulliyat-tara.ts:1`.
2. **Mutasyon kapısı (FR-2.6):** bildirilmiş mutantlar öldürülür, yaşayan mutant ve bağsız parite kırmızıdır. Kanıt: [mutasyon-kontrol.log](mutasyon-kontrol.log), mutant listesi: `port/mutantlar.toml:1`.
3. **Üretilmiş durum (FR-2.7):** `port/durum.json` gerçek `hepsi.ts` koşusundan gelir, `port/DURUM.md` ve README durum blokları `durum-uret` çıktısıyla bayt-özdeştir. Kanıt: [durum-kontrol.log](durum-kontrol.log), kod: `araclar/kapilar/durum-uret.ts:1`.
4. **Defter kapısı (FR-2.8):** defter gerçek ref kaynaklarına ve boşluksuz pencerelere bağlıdır. Kanıt: [defter-kontrol.log](defter-kontrol.log), veri: `port/defter/dosyalar.jsonl:1`.
5. **Mühür: KIRMIZI (2026-11-06 itibarıyla 9 ihlal).** Mühürlü kümenin 6 dosyası listeden farklı, `onay_bekliyor = true`, imzalı mühür commit'i `ae7991b` HEAD'in atası değil. Ayrıntı ve sınıfı §6.4'te. Kanıt: [muhur-kontrol.log](muhur-kontrol.log).

---

## 2. Kabul Kriterleri Doğrulama Matrisi

Dört kabul maddesinin dördü de gerçek `bun test` koşusunda geçmiştir; kanıt [kabul-haritasi.log](kabul-haritasi.log) (yeşil).

| Madde | Kural | Test (dosya:satır) | Sonuç |
|---|---|---|:---:|
| **P3-A1** | `kulliyat-tara` 5 sahte teslim sınıfını yakalar, temiz ağaçta yeşildir | `araclar/kapilar/p3.test.ts:12` · [kabul-haritasi.log](kabul-haritasi.log) | ✅ geçti |
| **P3-A2** | `mutasyon-kontrol` yaşayan mutantta ve bağsız paritede kırmızıdır | `araclar/kapilar/p3.test.ts:29` · [kabul-haritasi.log](kabul-haritasi.log) | ✅ geçti |
| **P3-A3** | `durum-kontrol` elle değiştirilmiş DURUM.md ve eksik README başlığında kırmızıdır | `araclar/kapilar/p3.test.ts:83` · [kabul-haritasi.log](kabul-haritasi.log) | ✅ geçti |
| **P3-A4** | `defter-kontrol` sahte hedef, sahte kanıt, pencere boşluğu ve gerekçesiz kapsam-dışında kırmızıdır | `araclar/kapilar/p3.test.ts:114` · [kabul-haritasi.log](kabul-haritasi.log) | ✅ geçti |

---

## 3. Ek D Sahte-Kapı Listesi Beyanı

1. **Sabit yanıt:** Baktım: üretim kodu taraması [sabit-yanit.log](sabit-yanit.log) günlüğünde, tarama kuralı `araclar/kapilar/sabit-yanit.ts:1`.
2. **Kendini doğrulayan kontrol:** Baktım: bağsız sahte parite betiği oluşturulup kırmızıya düşürülür, `araclar/kapilar/p3.test.ts:51`; mutantın yaşaması kapıyı kırmızı yapar, `araclar/kapilar/p3.test.ts:66`.
3. **Ulaşılamayan teslim:** Baktım: crate ulaşılabilirliği [ulasilabilirlik.log](ulasilabilirlik.log) günlüğünde.
4. **Kanıtı aşan rapor:** Baktım: bu raporun sonuç satırları [kabul-haritasi.log](kabul-haritasi.log) ve [kulliyat-tara.log](kulliyat-tara.log) günlüklerine bağlıdır; bağsız iddia denetimi `araclar/kapilar/rapor-kontrol.ts:1` içindedir.
5. **Gevşetme:** Baktım: iki gevşetme vardır ve açıkça yazılmıştır. (a) `durum-kontrol` koşu içinde yalnız koşan kapıdan ÖNCEKİ kapıların günlüğünü doğrular, `araclar/kapilar/durum-kontrol.ts:153`; tek başına koşuda tüm günlükler doğrulanır. (b) `hepsi.ts` bu bilgiyi `KERVAN_KOSU_KAPI` ile iletir, `araclar/kapilar/hepsi.ts:205`. İkisi de Ercan'ın imzalı mührüyle onaylandı. Eşik, mutant listesi ya da kural gevşetilmemiştir; `port/mutantlar.toml` yalnız ekleme alır.
6. **Başarısız olamayan test:** Baktım: A2 ve A4 testleri bilerek bozulmuş girdiyle kırmızı beklenir, `araclar/kapilar/p3.test.ts:66` ve `araclar/kapilar/p3.test.ts:114`.

---

## 4. Bilinen Sınırlar

- `kulliyat-tara --temiz` temiz ağaçta kırmızıdır: `defter-kontrol` final kipi, ref'teki 25.364 dosyanın hepsinin defterde kapanmasını ister. Defterde yalnız birkaç kaynak vardır. Bu P3 kabul maddesi değildir.
- DOS-6 ve DOS-7 (`openai-to-claude.ts` ve `openai-to-gemini.ts` kalan satırları) `planlandi` durumundadır; altın üretecine yeni girdi ve yeniden mühür gerektirir.
- P3-A1 süre duyarlıdır (`araclar/kapilar/p3.test.ts:12`, sınır 300 sn): ağır makine yükünde bir koşuda düşmüştür; yük azalınca bu tekrarlanmamıştır.

---

## 5. Mühür ve P4 Geçişi

- Mühür `port/muhur.toml` ile kilitlidir; değişiklik yalnız `muhur-uret.ts` ve Ercan'ın imzalı `muhur-onay` etiketiyle yapılır.

---

## 6. 2026-11-06 Durum Eki (ölçülmüş; üstteki bölümleri değiştirmez)

Bu ek, 2026-11-06 P3 koşusunun ölçtüğü hâli yazar. Üstteki bölümler 2026-10-14 tarihli metindir; çelişki varsa bu ek geçerlidir.

- **6.1 mutasyon-kontrol:** [mutasyon-kontrol.log](mutasyon-kontrol.log) — iki mutantın hedef dosyası `crates/sunucu/src/chat.rs` olarak düzeltildi, 12 mutantın 10'u öldürüldü, kapı yeşil.
- **6.2 durum-kontrol:** [durum-kontrol.log](durum-kontrol.log) — koşu → `durum-uret` → koşu sırası uygulandı; gecikmeli denetim çelişkisi bu sırayla kapandı.
- **6.3 kabul-haritasi:** [kabul-haritasi.log](kabul-haritasi.log) — dört kabul maddesinin dördü aynı koşuda doğrulandı.
- **6.4 muhur-kontrol: KIRMIZI, 9 ihlal.** İkisi insan kararı bekliyor: (a) imzalı mühür commit'i `ae7991b` HEAD'in atası değil (yalnız `dalga2/turso` ve `duzelt/guven-koku` dallarında), (b) `port/muhur.toml` içinde `onay_bekliyor = true` ve `onay` alanı boş. Kalan yedisi SHA uyuşmazlığı ya da mühür dosyasının imzalı sürümden farkı. Ölçülen altı dosya: `araclar/kapilar/p3.test.ts`, `araclar/uretec/strateji-altin-uret.ts`, `port/altin/strateji_girdi1..4.json`. Ajan imza atmadı, mühür dosyasını onaylı hâle getirmedi. Kanıt: [muhur-kontrol.log](muhur-kontrol.log).
- **6.5 Kapıda bulunan boşluk:** `rapor-kontrol` bir satırı ancak belirli sözcükler içerdiğinde iddia sayar (`araclar/kapilar/rapor-kontrol.ts`). §1 madde 5'teki eski "eşleşir" cümlesi kırmızı mühür günlüğüne dayanıyordu ve tetikleyici sözcük taşımadığı için kırmızıya düşmüyordu; cümle bu ek ile gerçeğe çekildi. Ölçülen ikinci boşluk: sıradan Türkçe çekimler de iddia sayılıyor (renk adının çekimli hâli ve "geçmek" fiilinin zarf-fiil biçimi); bu ek yazılırken ikisi de yanlış pozitif üretti. Kapının sözcük listesini sertleştirmek mühürlü dosyaya dokunmak demektir; Ercan kararı bekliyor.
- **6.6 Günlük kota penceresi eklendi (Ercan kararı, 2026-11-06):** Kota duyarlı dört strateji artık günlük doluluğu da hesaba katar. Ölçülen değişiklik: `KotaSnapshot.gunluk` + `gunluk_doluluk/gunluk_kalan` (`crates/havuz/src/kota_duyarli.rs`), `headroom(util5h, util7d, util_gun)`, `KotaSnapshot::anlik_goruntu` (sayı → doluluk eşlemesi, saf fonksiyon), `HeadroomSinyali.util_gun` (`crates/havuz/src/router.rs`), günlük pencere beslemesi 24 saatlik geriye dönük dilim (`crates/sunucu/src/server.rs`, `kota_anlik_goruntuleri`), `[kota] tavan_gun_istek` ve `[kota.saglayici_tavan.X] tavan_gun_istek` (`crates/config/src/lib.rs`, 0 değeri açılış hatası). Penceresi tanımlı olmayan hedefte günlük doluluk 0,0'dır — "yarı dolu" değil; bu yüzden tavan yazılmayan yerde eski davranış korunur. Takvim günü sıfırlama anı bilinmediği için pencere geriye dönük 24 saattir ve sağlayıcıdan daha sıkı davranır. Ölçüm: `cargo test --workspace` 0 hata, `cargo clippy --workspace --all-targets --all-features -D warnings` 0 uyarı, `cargo fmt --all -- --check` temiz, `araclar/kapilar/p1.test.ts` 4/4. Çalışan ikiliden alınan çıktı: `--serve` açılışı çıkış kodu 0; `/kota` yanıtında `kota.saglayici_tavan.openrouter.tavan_gun_istek = 50` alanı görünüyor. Bu çıktı bir `hepsi.ts` koşu günlüğü değildir (elle çalıştırıldı), bu yüzden burada kapı sonucu olarak sayılmaz. P3'ün tamamı bu ekten SONRA koşulmadı; §6.4'teki mühür durumu değişmedi.
- **6.7 OpenRouter uçtan uca ölçümü (2026-11-07, elle koşu; kapı koşusu değil):** Amaç, günlük pencerenin gerçek bir REST hedefinde dolduğunu görmekti. Kurulum dosyası (`C:\Users\buzbe\Desktop\mcp-tools\kervan\kervan.toml`) değiştirilmedi; `KERVAN_CONFIG` ile geçici bir kopya (`%TEMP%\kervan-olcum.toml`) verildi ve sunucu `--serve --port 20288 --headless` ile başlatıldı. Kopyaya eklenen bölüm: `[rest.openrouter]` (`bicim = "openai"`, `taban_url = "https://openrouter.ai/api"`, `anahtar_env = "OPENROUTER_API_KEY"`, `model = "nvidia/nemotron-3-ultra-550b-a55b:free"`, `yol = "/v1/chat/completions"`) ve `[kota.saglayici_tavan.openrouter] tavan_gun_istek = 50`. Açılış günlüğünde `yapilandirma : C:\Users\buzbe\AppData\Local\Temp\kervan-olcum.toml` satırı okundu, `--serve` süreci ayakta kaldı. `GET /health` yanıt gövdesi iki karakterdi; `GET /v1/models` listesinde `kervan/openrouter` kimliği bir kez göründü; `GET /kota` yanıtındaki `rest[0].tavan` nesnesi `tavan_5saat_istek: null`, `tavan_hafta_istek: null`, `tavan_gun_istek: 50` verdi. İstek öncesi `rest[0].kullanim` tüm pencerelerde 0'dı. `POST /v1/chat/completions` gövdesi `{"model":"kervan/openrouter", ...}` ile atıldı: HTTP durum kodu 200, gövdede `finish_reason: "stop"`, `model: "nvidia/nemotron-3-ultra-550b-a55b:free"`, `usage.cost: 0`, `usage.total_tokens: 237`, geçen süre 2561 ms. İstek sonrası `rest[0].kullanim` değerleri `son_saat: 1`, `son_gun: 1`, `son_hafta: 1`, `toplam: 1` oldu; `%LOCALAPPDATA%\kervan\kota.jsonl` dosyasına bir satır eklendi (132 → 133 satır), eklenen satırın `hesap` ve `saglayici` alanları `openrouter`, `sonuc` alanı olumlu sonuç değeri, `t` alanı 1794025286. Yani günlük pencere sayacı gerçek bir istekte 0'dan 1'e çıktı; tavan 50 olduğu için doluluk oranı 1/50 = 0,02. Süreç `Stop-Process` ile kapatıldı, geçici yapılandırma silindi, `OPENROUTER_API_KEY` ve `KERVAN_CONFIG` süreç ortamından düşürüldü. Anahtarın kendisi hiçbir dosyaya yazılmadı; yalnız o sürecin ortamında durdu. Sınanmayan: 429 davranışı (50 istek tüketilmedi) ve çoklu istekte cooldown/devre kesici tepkisi.
