# Faz P2 Sonu Raporu: Temel Sahte Teslim Kapıları, Mühür Aracı, Rapor Sözleşmesi

- **Faz ID:** P2
- **Başlık:** Temel Sahte Teslim Kapıları, Mühür Aracı, Rapor Sözleşmesi
- **Tarih:** 2026-10-03
- **Dal:** `omniroute-port`
- **Sorumlu / Rol:** Antigravity (Builder) | Hakem: Claude | Onay: Ercan

---

## 1. Yürütülen ve Teslim Edilen Bileşenler

1. **`araclar/kapilar/ulasilabilirlik.ts` (FR-2.1):**
   - `cargo metadata --all-features` bağımlılık grafını analiz ederek `kervan` ikilisine bağlı olmayan veya `port/kutuphane-izni.toml` içinde izinli olmayan bağsız crate'leri yakalayan kapı.
   - Kanıt: [ulasilabilirlik.ts](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/ulasilabilirlik.ts), [ulasilabilirlik.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P2/ulasilabilirlik.log).

2. **`araclar/kapilar/sabit-yanit.ts` (FR-2.2):**
   - Üretim kodunda (`src/` altındaki `.rs` dosyalarında, `#[cfg(test)]` blokları hariç) bilinen sahte veya sabit yanıt kalıplarını (`chatcmpl-123`, `1700000000`, `selamlar!`, `Simüle edilmiş`) tespit eden statik analiz aracı.
   - Kanıt: [sabit-yanit.ts](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/sabit-yanit.ts), [sabit-yanit.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P2/sabit-yanit.log).

3. **`araclar/kapilar/uc-kaydi.ts` ve `port/uclar.toml` (FR-2.3):**
   - Kervan HTTP rotalarını `port/uclar.toml` ile eşleştiren, `gercek` uçlar için `cargo test --list` üzerinde test varlığını ve test içinde `nonce` doğrulaması arayan kapı.
   - Kanıt: [uc-kaydi.ts](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/uc-kaydi.ts), [uclar.toml](file:///C:/Users/buzbe/Desktop/Github/kervan/port/uclar.toml), [uc-kaydi.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P2/uc-kaydi.log).

4. **`araclar/kapilar/muhur-kontrol.ts` (FR-2.4):**
   - Mühürlü dosyaların (`port/muhur.toml`) SHA-256 bütünlüğünü, `port/mutantlar.toml` dosyasının yalnız-ekleme (append-only) kuralını ve altın dosyaların üretici başlıklarını denetleyen kapı.
   - Kanıt: [muhur-kontrol.ts](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/muhur-kontrol.ts), [muhur-kontrol.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P2/muhur-kontrol.log).

5. **`araclar/kapilar/rapor-kontrol.ts` (FR-3.1):**
   - Faz sonu raporlarında bağsız başarı iddialarını yakalayan ve Ek D sahte-kapı listesinin 6 maddesinin eksiksiz yanıtlanmasını zorunlu kılan format denetleyicisi.
   - Kanıt: [rapor-kontrol.ts](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/rapor-kontrol.ts), [rapor-kontrol.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P2/rapor-kontrol.log).

6. **`araclar/kapilar/p2.test.ts` (Birim ve Kabul Test Paketi):**
   - 5 kabul kriterinin tamamını (P2-A1, P2-A2, P2-A3, P2-A4, P2-A5) negatif ve pozitif senaryolarla sınayan 5 test vakası (19 onay).
   - Kanıt: [p2.test.ts](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/p2.test.ts).

---

## 2. Kabul Kriterleri Doğrulama Matrisi

| Madde | Kural / Açıklama | Kanıt Testi / Dosya:Satır | Sonuç |
|---|---|---|:---:|
| **P2-A1** | `ulasilabilirlik` bağsız crate eklenmiş sahte ağaçta kırmızı, temiz ağaçta yeşildir | [p2.test.ts:13](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/p2.test.ts#L13) (`p2::a1`) · [ulasilabilirlik.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P2/ulasilabilirlik.log) | ✅ GEÇTİ |
| **P2-A2** | `sabit-yanit` test-dışı src içindeki kalıpta kırmızı, cfg(test) içinde yeşildir | [p2.test.ts:20](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/p2.test.ts#L20) (`p2::a2`) · [sabit-yanit.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P2/sabit-yanit.log) | ✅ GEÇTİ |
| **P2-A3** | `uc-kaydi` kayıtsız rota, var olmayan test kimliği ve nonce geçmeyen gercek testinde kırmızıdır | [p2.test.ts:55](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/p2.test.ts#L55) (`p2::a3`) · [uc-kaydi.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P2/uc-kaydi.log) | ✅ GEÇTİ |
| **P2-A4** | `muhur-kontrol` mühürlü fixture değişikliğinde, mutantlar.toml silinmesinde ve elle düzenlenmiş altında kırmızı; ekleme yeşildir | [p2.test.ts:91](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/p2.test.ts#L91) (`p2::a4`) · [muhur-kontrol.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P2/muhur-kontrol.log) | ✅ GEÇTİ |
| **P2-A5** | `rapor-kontrol` bağsız geçti cümlesinde ve cevapsız liste maddesinde kırmızıdır | [p2.test.ts:124](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/p2.test.ts#L124) (`p2::a5`) · [rapor-kontrol.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P2/rapor-kontrol.log) | ✅ GEÇTİ |

---

## 3. Ek D Sahte-Kapı Listesi Beyanı

1. **Sabit yanıt:** Baktım: [sabit-yanit.ts](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/sabit-yanit.ts#L20) kod tabanını tarayarak `chatcmpl-123`, `1700000000`, `selamlar!`, `Simüle edilmiş` kalıplarını üretim kodunda denetlemiş ve 102 dosyanın temiz olduğunu [sabit-yanit.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P2/sabit-yanit.log) çıktısında belgelemiştir.
2. **Kendini doğrulayan kontrol:** Baktım: [p2.test.ts](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/p2.test.ts) içindeki testler yapay fixture'lar oluşturup dosyayı bozarak kapının kırmızıya düştüğünü ve düzelince yeşile döndüğünü kanıtlamıştır.
3. **Ulaşılamayan teslim:** Baktım: `ulasilabilirlik.ts` ile `cargo metadata --all-features` üzerinden 9 workspace crate'inin tamamının kök ikiliye bağlandığı [ulasilabilirlik.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P2/ulasilabilirlik.log) dosyasında doğrulanmıştır.
4. **Kanıtı aşan rapor:** Baktım: Bu rapordaki her iddia ve sonuç doğrudan `p2.test.ts:13-160` satırlarına ve `port/kayit/P2/*.log` kütüklerine bağlanmıştır.
5. **Gevşetme:** Baktım: Hiçbir test, kapı veya eşik gevşetilmemiştir; `muhur-kontrol.ts` SHA-256 doğrulamasıyla gelecekteki gevşetmeleri engelleyecek mekanizma kurulmuştur.
6. **Başarısız olamayan test:** Baktım: P2-A1 (bağsız crate simülasyonu), P2-A2 (üretim kodunda sahte kalıp tespiti), P2-A3 (eksik rota ve sahte test kimliği tespiti), P2-A4 (bozuk SHA-256 tespiti) ve P2-A5 (bağsız iddia ve eksik madde tespiti) negatif testleriyle başarısız olma kabiliyeti [p2.test.ts](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/p2.test.ts) ile kanıtlanmıştır.

---

## 4. Ertelenenler (Deferrals)

- Kırmızı takım külliyatı (`ac2f18d`) beş sınıf denetimi: Plan gereği P3'te icra edilecektir.
- Mühürleme dosyası (`port/muhur.toml`): Plan gereği P3'ün insan kapısında Ercan'la oluşturulacaktır.
