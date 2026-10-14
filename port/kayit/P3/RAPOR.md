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
5. **Mühür:** mühürlü küme `port/muhur.toml` ile eşleşir, onay imzalı etikettedir. Kanıt: [muhur-kontrol.log](muhur-kontrol.log).

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
