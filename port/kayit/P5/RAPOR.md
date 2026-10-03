# Faz P5 Sonu Raporu: Nonce Uçtan Uca, Sahte Kod Temizliği ve Model Listesi

- **Faz ID:** P5
- **Başlık:** Nonce Uçtan Uca, Sahte Kod Temizliği ve Model Listesi
- **Tarih:** 2026-10-03
- **Dal:** `omniroute-port`
- **Sorumlu / Rol:** Antigravity (Builder) | Hakem: Claude | Onay: Ercan

---

## 1. Yürütülen ve Teslim Edilen Bileşenler

1. **Uçtan Uca Nonce Doğrulaması (FR-4.4):**
   - Gerçek `server::router` kısa ömürlü bir TCP soketinde `axum::serve` ile ayağa kaldırıldı.
   - Sahte yukarı akış HTTP sunucusu her koşuda rastgele 128-bitlik bir `nonce` üreterek yanıta yerleştirdi; istemciye dönen yanıtta bu rastgele nonce'un varlığı doğrulandı.
   - Kanıt: [p5_e2e_test.rs](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/tests/p5_e2e_test.rs), [kabul-haritasi.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P5/kabul-haritasi.log).

2. **Sahte Kodun Silinmesi ve Uç Terfisi (FR-4.5):**
   - Eski sahte yanıt kalıbı olan `Kervan yanıtı: [` metninin kod tabanında bulunmadığı `git grep` ile denetlendi.
   - `port/uclar.toml` dosyasında `/v1/chat/completions` ve `/v1/models` uçları `gercek` statüsüne terfi ettirildi ve nonce testleriyle mühürlendi.
   - Kanıt: [uclar.toml](file:///C:/Users/buzbe/Desktop/Github/kervan/port/uclar.toml), [uc-kaydi.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P5/uc-kaydi.log).

3. **Gerçek Model Listesi (FR-4.6):**
   - `/v1/models` ucu REST sağlayıcılarını `kervan/<ad>` olarak listelerken, `gpt-4o` gibi uydurma modellerden tamamen arındırıldı.
   - Kanıt: [server.rs:437](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/src/server.rs#L437), [p5_e2e_test.rs](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/tests/p5_e2e_test.rs).

---

## 2. Kabul Kriterleri Doğrulama Matrisi

| Madde | Kural / Açıklama | Kanıt Testi / Dosya:Satır | Sonuç |
|---|---|---|:---:|
| **P5-A1** | gerçek server::router axum::serve ile açılır; sahte yukarı akış rastgele nonce üretir ve istemciye dönen yanıtta nonce doğrulanır | [p5_e2e_test.rs:11](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/tests/p5_e2e_test.rs#L11) (`p5_e2e_nonce_testi`) · [kabul-haritasi.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P5/kabul-haritasi.log) | ✅ GEÇTİ |
| **P5-A2** | sahte yanıt kalıpları depoda bulunmaz; port/uclar.toml kayıtlı rotalarla birebir eşleşir | [p5_e2e_test.rs:11](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/tests/p5_e2e_test.rs#L11) · [uclar.toml:4](file:///C:/Users/buzbe/Desktop/Github/kervan/port/uclar.toml#L4) · [uc-kaydi.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P5/uc-kaydi.log) | ✅ GEÇTİ |
| **P5-A3** | /v1/models REST sağlayıcılarını kervan/<ad> olarak listeler, gpt-4o gibi uydurma modeller bulunmaz | [p5_e2e_test.rs:167](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/tests/p5_e2e_test.rs#L167) (`p5_models_testi`) · [uc-kaydi.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P5/uc-kaydi.log) | ✅ GEÇTİ |

---

## 3. Ek D Sahte-Kapı Listesi Beyanı

1. **Sabit yanıt:** Baktım: [sabit-yanit.ts](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/sabit-yanit.ts) ile üretim kodunda hiçbir sabit sahte kalıp bulunmadığı [sabit-yanit.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P5/sabit-yanit.log) kütüğünde belgelenmiştir.
2. **Kendini doğrulayan kontrol:** Baktım: [p5_e2e_test.rs:11-155](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/tests/p5_e2e_test.rs#L11-L155) her koşuda rastgele bir UUID/nonce üretip yanıtın içinde bunu arayarak sabit metinle geçilmesini imkansız kılmıştır.
3. **Ulaşılamayan teslim:** Baktım: [ulasilabilirlik.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P5/ulasilabilirlik.log) ile 10 workspace crate'inin tamamının kök kervan ikilisine bağlı olduğu belgelenmiştir.
4. **Kanıtı aşan rapor:** Baktım: Bu rapordaki her iddia ve kabul maddesi doğrudan `p5_e2e_test.rs` satırlarına ve `port/kayit/P5/*.log` kütüklerine bağlanmıştır.
5. **Gevşetme:** Baktım: Hiçbir kapı kuralı gevşetilmemiştir; [muhur-kontrol.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P5/muhur-kontrol.log) mühürlü kapıların ve mutantlar.toml dosyasının korunduğunu doğrulamıştır.
6. **Başarısız olamayan test:** Baktım: Sahte sunucu istek almadığında veya dönen yanıtta nonce bulunmadığında testin başarısız olacağı [p5_e2e_test.rs:146-155](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/tests/p5_e2e_test.rs#L146-L155) satırlarındaki açık `assert!` kontrolleriyle teminat altına alınmıştır.

---

## 4. Defter ve Uç Durumu

- **`port/uclar.toml`**: 16 rota kayıtlıdır. `/v1/chat/completions` ve `/v1/models` uçları `gercek` statüsündedir ve uçtan uca testleri mevcuttur.
- **`port/DURUM.md`**: SHA-256 üretim özeti ile güncellenmiş ve `durum-kontrol` ile doğrulanmıştır.
