# Faz P9 Sonu Raporu: Yönlendirme Stratejileri

- **Faz ID:** P9
- **Başlık:** Yönlendirme Stratejileri (FR-7.1, FR-7.2, FR-7.3)
- **Tarih:** 2026-10-03
- **Dal:** `omniroute-port`
- **Sorumlu / Rol:** Antigravity (Builder) | Hakem: Claude | Onay: Ercan

---

## 1. Yürütülen ve Teslim Edilen Bileşenler

1. **Strateji Küme Eşitliği ve Dahili Ayrımı (FR-7.1):**
   - `araclar/kapilar/strateji-kumesi.ts` oluşturuldu. OmniRoute referansındaki (`ref/omniroute-dbe703a/src/shared/constants/routingStrategies.ts`) `ROUTING_STRATEGY_VALUES` (20 kamuya açık) ve `INTERNAL_ROUTING_STRATEGY_VALUES` (`quota-share`) kümesi ile Kervan'ın `crates/havuz/src/router.rs` içindeki `RoutingStrategy` sabitleri karşılaştırıldı.
   - Toplam 20 kamuya açık ve 1 dahili olmak üzere 21 stratejinin birebir eşitliği doğrulandı.
   - Kanıt: [p9_strateji_test.rs:16](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/havuz/tests/p9_strateji_test.rs#L16), [router.rs:40](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/havuz/src/router.rs#L40), [strateji-kumesi.ts:73](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/strateji-kumesi.ts#L73).

2. **Deterministik Strateji Sıralama Motoru ve Altın Parite (FR-7.2):**
   - `crates/havuz/src/router.rs` içine `RoutingStrategy::sirala(&self, adaylar: &[HedefAdayi], tohum: Option<u64>) -> Vec<HedefAdayi>` metodu ve `HedefAdayi` veri yapısı eklendi.
   - `araclar/parite/strateji-altin-uret.ts` aracı ile 21 stratejinin tamamı 2 sabit aday kümesi üzerinde çalıştırılarak `port/altin/strateji_girdi1.json` ve `port/altin/strateji_girdi2.json` altın dosyaları üretildi.
   - `crates/havuz/tests/p9_strateji_test.rs` entegrasyon testi ile her iki girdi için 21 stratejinin altın referansla bayt ve sıra eşitliği kanıtlandı.
   - Kanıt: [p9_strateji_test.rs:52](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/havuz/tests/p9_strateji_test.rs#L52), [router.rs:123](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/havuz/src/router.rs#L123), [strateji_girdi1.json:1](file:///C:/Users/buzbe/Desktop/Github/kervan/port/altin/strateji_girdi1.json#L1).

3. **İkiz Stratejiler Davranış Ayrımı (FR-7.3):**
   - `random` vs `strict-random`: `random` tam Fisher-Yates karıştırması yaparken, `strict-random` desteden kart çeker gibi ilk kazananı seçip kalanı executionKey'e göre sıralar. Sabit girdide farklı sıralamalar ürettikleri kanıtlandı (`[B, A, C]` vs `[B, C, A]`).
   - `reset-aware` vs `reset-window`: `reset-aware` kota/sıfırlanma süresi oranını skorlarken, `reset-window` penceresi en yakın hedefe öncelik verir. Girdi üzerinde farklı hedefleri ilk sıraya aldıkları kanıtlandı (`openai/gpt-4o` vs `claude/claude-3-5-sonnet`).
   - Kanıt: [p9_strateji_test.rs:88](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/havuz/tests/p9_strateji_test.rs#L88).

4. **Mutasyon Kapısı ve Yeni Mutant (FR-2.6):**
   - `strateji-least-used-yonunu-ters-cevir` mutantı [mutantlar.toml:47](file:///C:/Users/buzbe/Desktop/Github/kervan/port/mutantlar.toml#L47) kütüğüne eklendi.
   - `bun araclar/kapilar/mutasyon-kontrol.ts` çalıştırılarak toplam 7 mutantın tamamının öldürüldüğü kanıtlandı (7/7 mutant öldürüldü).
   - Kanıt: [mutasyon-kontrol.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P9/mutasyon-kontrol.log).

---

## 2. Kabul Kriterleri Doğrulama Matrisi

| Madde | Kural / Açıklama | Kanıt Testi / Dosya:Satır | Sonuç |
|---|---|---|:---:|
| **P9-A1** | Kervan'ın herkese açık strateji kümesi ROUTING_STRATEGY_VALUES ile küme eşittir (20); dahili quota-share ayrı işaretlidir ve kullanıcıya açılmaz | [p9_strateji_test.rs:16](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/havuz/tests/p9_strateji_test.rs#L16) · [router.rs:40](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/havuz/src/router.rs#L40) · [strateji-kumesi.ts:73](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/strateji-kumesi.ts#L73) | ✅ GEÇTİ |
| **P9-A2** | 21 stratejinin her biri için iki sabit girdi ve altın dosyayla Kervan sıralaması eşittir | [p9_strateji_test.rs:52](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/havuz/tests/p9_strateji_test.rs#L52) · [strateji_girdi1.json:1](file:///C:/Users/buzbe/Desktop/Github/kervan/port/altin/strateji_girdi1.json#L1) · [strateji_girdi2.json:1](file:///C:/Users/buzbe/Desktop/Github/kervan/port/altin/strateji_girdi2.json#L1) | ✅ GEÇTİ |
| **P9-A3** | random/strict-random ve reset-aware/reset-window çiftleri için farklı çıktı veren girdi kanıtı | [p9_strateji_test.rs:88](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/havuz/tests/p9_strateji_test.rs#L88) · [strateji_girdi1.json:1](file:///C:/Users/buzbe/Desktop/Github/kervan/port/altin/strateji_girdi1.json#L1) | ✅ GEÇTİ |

---

## 3. Ek D Sahte-Kapı Listesi Beyanı

1. **Sabit yanıt:** Baktım: [sabit-yanit.ts](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/sabit-yanit.ts) ile üretim kodunda hiçbir sabit sahte kalıp bulunmadığı [sabit-yanit.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P9/sabit-yanit.log) kütüğünde belgelenmiştir.
2. **Kendini doğrulayan kontrol:** Baktım: [p9_strateji_test.rs:52](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/havuz/tests/p9_strateji_test.rs#L52) gerçek Rust router motoru çağrılarak altın çıktılarla test edilmiştir.
3. **Ulaşılamayan teslim:** Baktım: [ulasilabilirlik.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P9/ulasilabilirlik.log) ile tüm sandıkların kök kervan ikilisine bağlı olduğu belgelenmiştir.
4. **Kanıtı aşan rapor:** Baktım: Bu rapordaki her iddia doğrudan `p9_strateji_test.rs` test satırlarına ve `port/kayit/P9/*.log` kütüklerine bağlanmıştır.
5. **Gevşetme:** Baktım: Hiçbir kapı kuralı gevşetilmemiştir; [muhur-kontrol.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P9/muhur-kontrol.log) mühürlü kapıların korunduğunu doğrulamıştır.
6. **Başarısız olamayan test:** Baktım: `strateji-least-used-yonunu-ters-cevir` mutantı uygulandığında `test_fr72_altin_parite_girdi1_ve_girdi2` testinin başarısız olarak mutantı öldürdüğü [mutasyon-kontrol.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P9/mutasyon-kontrol.log) kütüğüyle kanıtlanmıştır.

