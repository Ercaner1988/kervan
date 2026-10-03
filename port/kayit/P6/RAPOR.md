# Faz P6 Sonu Raporu: Biçimler ve Altın Parite

- **Faz ID:** P6
- **Başlık:** Biçimler ve Altın Parite
- **Tarih:** 2026-10-03
- **Dal:** `omniroute-port`
- **Sorumlu / Rol:** Antigravity (Builder) | Hakem: Claude | Onay: Ercan

---

## 1. Yürütülen ve Teslim Edilen Bileşenler

1. **O7 Ön Koşulu ve Ortam Kurulumu:**
   - Ercan'ın açık onayı ile `C:\Users\buzbe\Desktop\Github\genel\OmniRoute` dizininde `bun install` çalıştırıldı.
   - `C:/Users/buzbe/Desktop/Github/genel/OmniRoute/node_modules/uuid/package.json` ön koşul dosyasının varlığı doğrulandı.
   - `ref/omniroute-dbe703a/node_modules` bağlantısı üzerinden OmniRoute dbe703a referans kodunun yalıtılmış ortamda çalışması temin edildi.

2. **Altın Üretim Aracı (FR-5.1):**
   - [altin-uret.ts](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/parite/altin-uret.ts) aracı oluşturuldu.
   - OmniRoute'un orijinal `openaiToClaudeRequest` ve `openaiToGeminiRequest` fonksiyonları çalıştırılarak 6 sabit test senaryosu için `port/altin/` altında 12 altın dosya üretildi.
   - Canlı OmniRoute ve `~/.omniroute` dizinine dokunulmaksızın geçici `DATA_DIR` kullanıldı.
   - `bun araclar/parite/altin-uret.ts --kontrol` ile iki ardışık koşuda bayt-özdeş çıktılar doğrulandı.
   - Kanıt: [altin-parite.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P6/altin-parite.log), [altin-uret.ts:1](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/parite/altin-uret.ts#L1).

3. **Çeviri Katmanı ve Parite Eşitliği (FR-5.2):**
   - `crates/ceviri` (`kervan-ceviri`) crate'i geliştirildi ve `workspace.members`'a eklendi (toplam ~700 satır, crate sınırı ≤ 1500 kuralına tam uyumlu).
   - Claude ve Gemini için OpenAI istek/yanıt çeviricileri yazıldı: `openai_den_claude_istegi`, `claude_dan_openai_yaniti`, `claude_dan_openai_parcasi`, `openai_den_gemini_istegi`, `gemini_den_openai_yaniti`, `gemini_den_openai_parcasi`.
   - [p6_parite_test.rs](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/ceviri/tests/p6_parite_test.rs) ile 6 girdi senaryosunda Kervan çevirisinin altın referanslarla eşitliği kanıtlandı.
   - `claude-eslemesinde-bir-alani-boz` mutantı [mutantlar.toml:24](file:///C:/Users/buzbe/Desktop/Github/kervan/port/mutantlar.toml#L24) dosyasına eklendi ve [mutasyon-kontrol.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P6/mutasyon-kontrol.log) ile başarıyla öldürüldü.
   - Kasıtlı mimari sapmalar [sapmalar.toml:1](file:///C:/Users/buzbe/Desktop/Github/kervan/port/altin/sapmalar.toml#L1) kütüğüne işlendi.
   - Kanıt: [ceviri-test.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P6/ceviri-test.log), [mutasyon-kontrol.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P6/mutasyon-kontrol.log).

4. **Desteklenmeyen Alanlarda Açık Hata (FR-5.3):**
   - Claude ve Gemini REST sağlayıcılarına `tools` veya çok parçalı içerik gönderildiğinde yukarı akışa istek gitmeden 501/400 JSON hata dönmesi sağlandı.
   - Sahte yukarı akış sunucusunun istek sayacının 0 kaldığı uçtan uca testle doğrulandı.
   - OpenAI biçimli sağlayıcıda aynı `tools` alanının kayıpsız iletildiği kanıtlandı.
   - Kanıt: [p6_unsupported_test.rs:17](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/tests/p6_unsupported_test.rs#L17), [unsupported-test.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P6/unsupported-test.log).

---

## 2. Kabul Kriterleri Doğrulama Matrisi

| Madde | Kural / Açıklama | Kanıt Testi / Dosya:Satır | Sonuç |
|---|---|---|:---:|
| **P6-A1** | altin-uret iki ardışık koşuda bayt-özdeş çıktı verir; başlıklar kaynak SHA'sını taşır; canlı OmniRoute'a dokunulmaz | [altin-uret.ts:145](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/parite/altin-uret.ts#L145) · [altin-parite.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P6/altin-parite.log) | ✅ GEÇTİ |
| **P6-A2** | altı sabit girdi için Kervan çıktısı altınla eşittir (kayıtlı sapmalar hariç); ceviri mutantı testi kırar | [p6_parite_test.rs:13](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/ceviri/tests/p6_parite_test.rs#L13) · [ceviri-test.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P6/ceviri-test.log) · [mutasyon-kontrol.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P6/mutasyon-kontrol.log) | ✅ GEÇTİ |
| **P6-A3** | Claude/Gemini hedefte tools veya çok parçalı içerikte açık hata döner ve sahte sunucu istek sayacı 0 kalır | [p6_unsupported_test.rs:17](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/sunucu/tests/p6_unsupported_test.rs#L17) (`p6_desteklenmeyen_alanlar_testi`) · [unsupported-test.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P6/unsupported-test.log) | ✅ GEÇTİ |

---

## 3. Ek D Sahte-Kapı Listesi Beyanı

1. **Sabit yanıt:** Baktım: [sabit-yanit.ts](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/kapilar/sabit-yanit.ts) ile üretim kodunda hiçbir sabit sahte kalıp bulunmadığı [sabit-yanit.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P6/sabit-yanit.log) kütüğünde belgelenmiştir.
2. **Kendini doğrulayan kontrol:** Baktım: [altin-uret.ts:176](file:///C:/Users/buzbe/Desktop/Github/kervan/araclar/parite/altin-uret.ts#L176) Kervan sürecine `spawnSync` ile bağlanır; [p6_parite_test.rs:13](file:///C:/Users/buzbe/Desktop/Github/kervan/crates/ceviri/tests/p6_parite_test.rs#L13) diskteki altın JSON dosyalarını okuyarak gerçek `kervan-ceviri` fonksiyonlarının çıktılarıyla karşılaştırır.
3. **Ulaşılamayan teslim:** Baktım: [ulasilabilirlik.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P6/ulasilabilirlik.log) ile yeni eklenen `crates/ceviri` dahil olmak üzere 11 workspace crate'inin tamamının kök kervan ikilisine bağlı olduğu belgelenmiştir.
4. **Kanıtı aşan rapor:** Baktım: Bu rapordaki her iddia ve kabul maddesi doğrudan `p6_parite_test.rs` ve `p6_unsupported_test.rs` test satırlarına ve `port/kayit/P6/*.log` kütüklerine bağlanmıştır.
5. **Gevşetme:** Baktım: Hiçbir kapı kuralı gevşetilmemiştir; [muhur-kontrol.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P6/muhur-kontrol.log) mühürlü kapıların ve mutantlar.toml dosyasının yalnız-ekleme ilkesiyle korunduğunu doğrulamıştır.
6. **Başarısız olamayan test:** Baktım: `claude-eslemesinde-bir-alani-boz` mutantı uygulandığında `p6_parite_test.rs` testinin derhal başarısız olduğu ve mutantın öldürüldüğü [mutasyon-kontrol.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P6/mutasyon-kontrol.log) kütüğüyle kanıtlanmıştır.

---

## 4. Defter ve Uç Durumu

- **`port/altin/`**: 12 adet altın referans JSON dosyası üretilmiş ve mühür denetiminden geçmiştir.
- **`port/altin/sapmalar.toml`**: FR-5.3 kapsamındaki `tools` ve `cok_parcali_icerik` sapmaları gerekçeleri ve kaynak satırlarıyla kayıt altına alınmıştır.
- **`port/DURUM.md`**: SHA-256 üretim özeti ile güncellenmiş ve [durum-kontrol.log](file:///C:/Users/buzbe/Desktop/Github/kervan/port/kayit/P6/durum-kontrol.log) ile doğrulanmıştır.
