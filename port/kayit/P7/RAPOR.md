# Faz P7 Sonu Raporu: Dayanıklılık ve Hata Temizliği

- **Faz ID:** P7
- **Başlık:** Dayanıklılık ve Hata Temizliği
- **Tarih:** 2026-10-03
- **Dal:** `omniroute-port`
- **Sorumlu / Rol:** Antigravity (Builder) | Hakem: Claude | Onay: Ercan

---

## 1. Yürütülen ve Teslim Edilen Bileşenler

1. **`kervan-direnc` Dayanıklılık Kütüphanesi (FR-6.1):**
   - `crates/direnc` (`kervan-direnc`) sandığı oluşturuldu ve `[workspace].members` listesine eklendi.
   - 3 katmanlı mimari uygulandı:
     - `devre_kesici.rs`: KAPALI → BOZULMUŞ → AÇIK → YARI-AÇIK durum makinesi (`DevreKesiciYapilandirma`, `hata_esigi: 3`, `bozulma_esigi: 2`, `soguma_suresi`).
     - `soguma.rs`: 429 Too Many Requests ve aşırı yüklenme için `Retry-After` ve üstel geri çekilme (`SogumaTakipcisi`).
     - `kilit.rs`: 404 Not Found modeller için geçici ve kalıcı rotasyon dışı bırakma (`ModelKilidi`).
     - `saat.rs`: Zaman enjeksiyonu (`Saat`, `GercekSaat`, `TestSaati`) ile gerçek uyuma olmaksızın deterministik zaman ilerletme sağlandı.
     - `lib.rs`: Devre kesici, soğuma takipçisi ve model kilidini orkestre eden `DirencYoneticisi`.
   - Sandık satır sayısı: 563 satır (sandık sınırı ≤ 1500 kuralına tam uyumlu). Üretim kodunda sıfır `unwrap`/`expect`.
   - Kanıt: [direnc-test.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P7/direnc-test.log), [lib.rs:1](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/direnc/src/lib.rs#L1).

2. **`kervan-yukari` Dayanıklılık Entegrasyonu ve Hata Temizliği (FR-6.1, FR-6.2):**
   - `crates/yukari` içine `Arc<Mutex<DirencYoneticisi>>` entegre edildi (`Yukari::new`, `Yukari::yeni_direncli`).
   - İstek öncesi `d.istek_izni_al(&hedef.ad)` denetimi:
     - Devre açıksa `YukariHatasi::DevreAcik` döner; yukarı akışa istek gitmez.
     - Soğumadaysa `YukariHatasi::Sogumada` döner; yukarı akışa istek gitmez.
   - İstek sonrası durum güncellemesi:
     - 2xx: `d.basari_kaydet(&hedef.ad)` ile sayaçlar sıfırlanır.
     - 429: `Retry-After` başlığı okunarak `d.hata_kaydet(&hedef.ad, Some(429), retry_after)` ile soğuma başlatılır (devre kesiciyi beslemez).
     - 5xx / Ağ Hatası: `d.hata_kaydet(&hedef.ad, durum, None)` ile devre kesici beslenir; 3 ardışık hatada devre açılır.
     - 4xx (429 hariç): Devre kesiciyi veya soğumayı beslemez.
   - Hata Temizliği (`temizle_hata`):
     - Bilinen API anahtarları `[GIZLENDI]` ile maskelenir.
     - Bearer token kalıpları maskelenir.
     - Windows (`C:\...`) ve Unix (`/home/...`) dosya yolları `[GIZLI_YOL]` ile maskelenir.
     - Azami 500 karakter ile kısaltılır.
   - Kanıt: [dayaniklilik-test.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P7/dayaniklilik-test.log), [lib.rs:88](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/yukari/src/lib.rs#L88).

3. **Sunucu Uçtan Uca REST Hata ve Soğuma Kancası (FR-6.1, FR-6.2):**
   - `crates/sunucu/src/server.rs` içinde `DevreAcik` hatasında yukarı akışa gitmeden `503 Service Unavailable` dönülür.
   - `Sogumada` ve 429 yanıtlarında istemciye `429 Too Many Requests` ve `Retry-After: <sn>` başlığı iletilir.
   - Yankılanan test anahtarları ve iç sistem yollarının istemci yanıtına asla sızmadığı uçtan uca testle kanıtlandı.
   - Kanıt: [resilience-test.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P7/resilience-test.log), [server.rs:493](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/src/server.rs#L493).

4. **Mutasyon Kapısı ve Yeni Mutant (FR-2.6):**
   - `direnc-hata-kaydini-atla` mutantı [mutantlar.toml:31](file:///C:/Users/buzbe/Desktop/Github/kervan/port/mutantlar.toml#L31) kütüğüne eklendi.
   - `bun araclar/kapilar/mutasyon-kontrol.ts` çalıştırılarak tüm 5 mutantın öldürüldüğü kanıtlandı (5/5 mutant öldürüldü).
   - Kanıt: [mutasyon-kontrol.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P7/mutasyon-kontrol.log).

---

## 2. Kabul Kriterleri Doğrulama Matrisi

| Madde | Kural / Açıklama | Kanıt Testi / Dosya:Satır | Sonuç |
|---|---|---|:---:|
| **P7-A1** | ardışık 5xx kesiciyi açar ve sonraki istek yukarı akışa gitmeden 503 döner; 429 Retry-After ile soğuma ve süre sonunda yeniden deneme çalışır | [p7_dayaniklilik_test.rs:52](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/yukari/tests/p7_dayaniklilik_test.rs#L52) · [p7_dayaniklilik_test.rs:114](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/yukari/tests/p7_dayaniklilik_test.rs#L114) · [p7_resilience_test.rs:55](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/tests/p7_resilience_test.rs#L55) · [p7_resilience_test.rs:175](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/tests/p7_resilience_test.rs#L175) · [dayaniklilik-test.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P7/dayaniklilik-test.log) · [resilience-test.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P7/resilience-test.log) | ✅ GEÇTİ |
| **P7-A2** | yankılanan test anahtarı istemci yanıtında, stdout/stderr'de ve günlüklerde yoktur | [p7_dayaniklilik_test.rs:240](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/yukari/tests/p7_dayaniklilik_test.rs#L240) · [p7_resilience_test.rs:290](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/tests/p7_resilience_test.rs#L290) · [dayaniklilik-test.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P7/dayaniklilik-test.log) · [resilience-test.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P7/resilience-test.log) | ✅ GEÇTİ |

---

## 3. Ek D Sahte-Kapı Listesi Beyanı

1. **Sabit yanıt:** Baktım: [sabit-yanit.ts](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/sabit-yanit.ts) ile üretim kodunda hiçbir sabit sahte kalıp bulunmadığı [sabit-yanit.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P7/sabit-yanit.log) kütüğünde belgelenmiştir.
2. **Kendini doğrulayan kontrol:** Baktım: [p7_dayaniklilik_test.rs:52](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/yukari/tests/p7_dayaniklilik_test.rs#L52) ve [p7_resilience_test.rs:55](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/tests/p7_resilience_test.rs#L55) gerçek ağ yuvaları (`TcpListener`) açarak gelen istek sayılarını atomik sayaçlarla doğrulamış; 5xx sonrası yukarı akışa giden istek sayısının sabit kaldığını kanıtlamıştır.
3. **Ulaşılamayan teslim:** Baktım: [ulasilabilirlik.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P7/ulasilabilirlik.log) ile yeni eklenen `crates/direnc` dahil olmak üzere 12 workspace crate'inin tamamının kök kervan ikilisine bağlı olduğu belgelenmiştir.
4. **Kanıtı aşan rapor:** Baktım: Bu rapordaki her iddia doğrudan `p7_dayaniklilik_test.rs`, `p7_resilience_test.rs` test satırlarına ve `port/kayit/P7/*.log` kütüklerine bağlanmıştır.
5. **Gevşetme:** Baktım: Hiçbir kapı kuralı gevşetilmemiştir; [muhur-kontrol.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P7/muhur-kontrol.log) mühürlü kapıların ve mutantlar.toml dosyasının yalnız-ekleme ilkesiyle korunduğunu doğrulamıştır.
6. **Başarısız olamayan test:** Baktım: `direnc-hata-kaydini-atla` mutantı uygulandığında devre kesicinin açılmadığı ve `test_p7_ardisik_5xx_kesiciyi_acar` testinin başarısız olarak mutantı öldürdüğü [mutasyon-kontrol.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P7/mutasyon-kontrol.log) kütüğüyle kanıtlanmıştır.

---

## 4. Defter ve Durum

- **`crates/direnc`**: 563 satır, %100 güvenli Rust.
- **`crates/yukari`**: 387 satır, %100 güvenli Rust.
- **`port/DURUM.md`**: SHA-256 üretim özeti ile güncellenmiş ve [durum-kontrol.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P7/durum-kontrol.log) ile doğrulanmıştır.
