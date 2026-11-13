# Faz P10 Sonu Raporu: Envanter, Defter, Dalga Düzeni ve PR Taslağı

- **Faz ID:** P10
- **Başlık:** Envanter, Defter, Dalga Düzeni ve PR Taslağı (FR-8.1, FR-8.2, FR-8.3, FR-3.2)
- **Tarih:** 2026-11-13 (ilk taslak 2026-10-03; bu sürüm P10 kapı koşusunun günlüklerine bağlandı)
- **Dal:** `omniroute-port`
- **Sorumlu / Rol:** Antigravity (Builder) | Hakem: Claude | Onay: Ercan

---

## 1. Yürütülen ve Teslim Edilen Bileşenler

1. **Kod Tabanı Envanteri (FR-8.1):**
   - `araclar/envanter.ts`, OmniRoute `ref/omniroute-dbe703a` anlık görüntüsünü tarayıp `port/envanter.json` çıktısını üretir; sayımlar taramadan gelir, betikte sabit değildir.
   - Sayımlar: 730 `route.ts/js`, 122 pano sayfası, 193 veri tabanı modülü, 252 göç/şema tanımı, 6543 test dosyası.
   - İki ardışık koşuda bayt-özdeş çıktı: `test_p10_envanter_deterministik` bu özelliği ölçer.
   - Kanıt: [envanter.ts:1](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/envanter.ts#L1), [envanter.json:1](file:///C:/Users/buzbe/Desktop/Github/kervan/port/envanter.json#L1), [kabul-haritasi.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P10/kabul-haritasi.log).

2. **Kervan Aktarım Defteri (FR-8.2):**
   - `port/defter/dosyalar.jsonl` ve `port/defter/davranislar.jsonl` oluşturuldu.
   - `dogrulandi` durumundaki satırlar var olan hedef dosyalara (`crates/ceviri/src/claude.rs`, `crates/ceviri/src/gemini.rs`, `crates/havuz/src/router.rs`) ve var olan altın kanıtlara bağlandı.
   - Kapsam dışı ve insan kararı maddelerine gerekçeler eklendi.
   - Pencere örtüşmesi ve şema denetimi: [defter-kontrol.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P10/defter-kontrol.log).
   - Final modu (`--final`) 12 ihlal verdi: dokuzu kapanmamış pencere (DOS-1, DOS-2, DOS-9, DAV-4, DAV-5 `aktarildi`; DOS-6, DOS-6-2, DOS-7, DOS-7-2, DOS-7-3, DAV-6 `planlandi`), biri ref envanterindeki 23865 dosyanın defterde bulunmaması. Bu, DALGA-2..4 aktarımı henüz yürümediği için beklenen durumdur; kırmızı biten günlük [defter-kontrol-final.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P10/defter-kontrol-final.log).
   - Kanıt: [dosyalar.jsonl:1](file:///C:/Users/buzbe/Desktop/Github/kervan/port/defter/dosyalar.jsonl#L1), [davranislar.jsonl:1](file:///C:/Users/buzbe/Desktop/Github/kervan/port/defter/davranislar.jsonl#L1), [defter-kontrol.ts:35](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/defter-kontrol.ts#L35).

3. **Sonraki Dalgaların PRD İskeletleri (FR-8.3):**
   - `araclar/dalga-uret.ts` sabit metin taşımaz; kapsam, sayılar ve açık pencere listesi
     `port/dalgalar/dalga-tanimi.json` (kimlik), `port/envanter.json`,
     `crates/pano-ekran/esleme.toml`, `port/defter/dosyalar.jsonl`,
     `port/defter/davranislar.jsonl` ve `port/durum.json` dosyalarından türetilir. Girdi
     eksikse üretim durur (uydurma ile doldurulmaz).
   - Çalıştırılarak `port/dalgalar/` dizininde 3 adet dalga PRD taslağı üretildi:
     - `DALGA-2.md`: Veritabanı Kalıcılığı ve SQLite/Drizzle Modelleri.
     - `DALGA-3.md`: Pano ve İstatistik Arayüzü (egui Dashboard).
     - `DALGA-4.md`: Gelişmiş Sağlayıcı Eklentileri ve Model Kataloğu.
   - Her taslak PRD §2 başlıklarını, varsayım analizini, `Acceptance:` içeren işlevsel
     gereksinimleri (FR) ve §5 Türetim İzi bölümünü taşır.
   - Türetim bağımsız testle ölçülür: `test_p10_dalga_taslaklari_defterden_turetiliyor`
     defter kimliklerini, pano eşleme sayısını ve envanter değerlerini taslak metniyle
     karşılaştırır. Taslak metnindeki "10 bölüm D2" değeri "11" yapıldığında test düşer;
     bu, testin türetilmiş değeri gerçekten okuduğunu gösterir.
   - Kanıt: [dalga-uret.ts:1](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/dalga-uret.ts#L1), [DALGA-2.md:1](file:///C:/Users/buzbe/Desktop/Github/kervan/port/dalgalar/DALGA-2.md#L1), [p10_teslim_test.rs:124](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/havuz/tests/p10_teslim_test.rs#L124), [kabul-haritasi.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P10/kabul-haritasi.log).

4. **Dal ve PR Taslağı (FR-3.2):**
   - Çalışma `omniroute-port` dalında yürütüldü; `main` dalına bu faz sırasında doğrudan commit yazılmadı.
   - `omniroute-aktarim` dalı `main`'e birleştirilmedi.
   - `PR.md` dosyası Bakımcı, Benimseyici ve Dağıtımcı üç salt-okunur bakışıyla hazırlandı; otomatik PR açılmadı, Ercan'ın incelemesine sunuldu.
   - Kanıt: [PR.md:1](file:///C:/Users/buzbe/Desktop/Github/kervan/PR.md#L1), [kabul-haritasi.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P10/kabul-haritasi.log).

---

## 2. Kabul Kriterleri Doğrulama Matrisi

| Madde | Kural / Açıklama | Kanıt Testi / Dosya:Satır | Sonuç |
|---|---|---|---|
| **P10-A1** | araclar/envanter.ts anlık görüntüden dosya/rota/göç/sayfa/modül sayımını üretir; iki koşuda özdeş çıktı verir | [envanter.ts:1](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/envanter.ts#L1) · [envanter.json:1](file:///C:/Users/buzbe/Desktop/Github/kervan/port/envanter.json#L1) · [kabul-haritasi.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P10/kabul-haritasi.log) | kabul maddesi karşılandı |
| **P10-A2** | port/defter/{dosyalar,davranislar}.jsonl şemaya uygundur; dogrulandi satırları kanıtlarıyla bağlıdır; defter-kontrol geçer | [dosyalar.jsonl:1](file:///C:/Users/buzbe/Desktop/Github/kervan/port/defter/dosyalar.jsonl#L1) · [davranislar.jsonl:1](file:///C:/Users/buzbe/Desktop/Github/kervan/port/defter/davranislar.jsonl#L1) · [defter-kontrol.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P10/defter-kontrol.log) · [kabul-haritasi.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P10/kabul-haritasi.log) | KISMİ — şema ve pencere denetimi ayrı kapıda; final kapısı açık pencereler nedeniyle kırmızı |
| **P10-A3** | araclar/dalga-uret.ts defterden en az 3 dalga taslağı üretir; her taslak §2 başlıklarını ve en az bir Acceptance: içeren FR iskeleti taşır; taslak sayıları ve defter kimlikleri girdilerden türetilir | [dalga-uret.ts:1](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/dalga-uret.ts#L1) · [DALGA-2.md:1](file:///C:/Users/buzbe/Desktop/Github/kervan/port/dalgalar/DALGA-2.md#L1) · [p10_teslim_test.rs:124](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/havuz/tests/p10_teslim_test.rs#L124) · [kabul-haritasi.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P10/kabul-haritasi.log) | kabul maddesi karşılandı |
| **P10-A4** | main doğrudan commit almamıştır; PR.md taslak olarak yazılmıştır ve insan onayına bırakılmıştır | [PR.md:1](file:///C:/Users/buzbe/Desktop/Github/kervan/PR.md#L1) · [kabul-haritasi.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P10/kabul-haritasi.log) | kabul maddesi karşılandı |

---

## 3. Ek D Sahte-Kapı Listesi Beyanı

1. **Sabit yanıt:** Baktım: [sabit-yanit.ts](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/sabit-yanit.ts) ile üretim kodunda hiçbir sabit sahte kalıp bulunmadığı [sabit-yanit.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P10/sabit-yanit.log) kütüğünde belgelenmiştir.
2. **Kendini doğrulayan kontrol:** Baktım: [envanter.ts:1](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/envanter.ts#L1) ve [dalga-uret.ts:1](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/dalga-uret.ts#L1) bağımsız dosyaları tarayarak ve defter şemasını okuyarak çıktı üretmektedir.
3. **Ulaşılamayan teslim:** Baktım: [ulasilabilirlik.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P10/ulasilabilirlik.log) ile tüm sandıkların kök kervan ikilisine bağlı olduğu belgelenmiştir.
4. **Kanıtı aşan rapor:** Baktım: [envanter.json:1](file:///C:/Users/buzbe/Desktop/Github/kervan/port/envanter.json#L1) ve [dosyalar.jsonl:1](file:///C:/Users/buzbe/Desktop/Github/kervan/port/defter/dosyalar.jsonl#L1) satırları buradaki her sayının kaynağını taşır.
5. **Gevşetme:** Baktım: Hiçbir kapı kuralı gevşetilmedi; [muhur-kontrol.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P10/muhur-kontrol.log) mühürlü kapıların korunduğunu belgeler.
6. **Başarısız olamayan test:** Baktım: 12 mutantın 10'u öldürüldü, ikisi emekliye ayrıldı; [mutasyon-kontrol.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P10/mutasyon-kontrol.log) kütüğü ham dökümü taşır.
