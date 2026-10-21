<!-- URETILMIS_DURUM_HASH: 1df41c1eb006ee43103590a228ec0ff35676fd44be468af2ef3381017daf4ae0 -->
# DURUM.md — Kervan Aktarım Durum Defteri (ÜRETİLMİŞTİR)

> ⚠️ BU DOSYA araclar/kapilar/durum-uret.ts ÇIKTISIDIR. ELLE DÜZENLEMEYİNİZ.
> durum-kontrol bu dosyayı port/durum.json'dan yeniden üretip bayt bayt karşılaştırır.

- **Mevcut Faz:** P3
- **Üretim Zamanı (durum.json):** 2026-10-21T09:49:12.637Z
- **Üretici Özeti:** sha256:a9b4a56d5170899a65c47db3b903d5561728b859572da9ad9ffc547b6f9f0612
- **Tüm Kapılar Yeşil mi:** ❌ HAYIR

---

## 1. Kapı Sonuçları

| Kapı Adı | Çıkış Kodu | Süre (ms) | Log Dosyası |
|---|:---:|:---:|---|
| makine-sakin | ✅ 0 | 60188 | port\kayit\P3\makine-sakin.log |
| kabul-haritasi | ❌ 1 | 580483 | port\kayit\P3\kabul-haritasi.log |
| ulasilabilirlik | ❌ 1 | 1030 | port\kayit\P3\ulasilabilirlik.log |
| sabit-yanit | ✅ 0 | 119 | port\kayit\P3\sabit-yanit.log |
| uc-kaydi | ✅ 0 | 5321 | port\kayit\P3\uc-kaydi.log |
| muhur-kontrol | ❌ 1 | 16936 | port\kayit\P3\muhur-kontrol.log |
| kulliyat-tara | ✅ 0 | 8870 | port\kayit\P3\kulliyat-tara.log |
| mutasyon-kontrol | ✅ 0 | 103020 | port\kayit\P3\mutasyon-kontrol.log |
| durum-kontrol | ❌ 1 | 459 | port\kayit\P3\durum-kontrol.log |
| defter-kontrol | ✅ 0 | 27396 | port\kayit\P3\defter-kontrol.log |
| rapor-kontrol | ❌ 1 | 160 | port\kayit\P3\rapor-kontrol.log |

## 2. Fazlar

| Faz | Commit | Kanıt |
|---|---|---|
| (yok) | | |

## 3. Sayımlar

- route_ts: 730
- defter_planlandi: 5
- defter_dogrulandi: 1

## 4. HTTP Rota Durumu (port/uclar.toml)

```toml
# port/uclar.toml
# Kervan HTTP Rota Kayıt Defteri

[[uc]]
yol = "/v1/chat/completions"
yontem = "POST"
durum = "gercek"
test = "p5_e2e_nonce_testi"
testDosyasi = "crates/sunucu/tests/p5_e2e_test.rs"

[[uc]]
yol = "/v1/models"
yontem = "GET"
durum = "gercek"
test = "p5_models_testi"
testDosyasi = "crates/sunucu/tests/p5_e2e_test.rs"

[[uc]]
yol = "/health"
yontem = "GET"
durum = "kapsam-disi"
gerekce = "Canlılık yoklaması: sabit \"ok\" döner, iş mantığı ve yukarı akış yok"

[[uc]]
yol = "/telemetri"
yontem = "GET"
durum = "kapsam-disi"
gerekce = "Kervan'ın kendi sayaç/telemetri dökümü; OmniRoute'ta karşılığı yok, parite ölçülemez"

[[uc]]
yol = "/bakiye"
yontem = "GET"
durum = "kapsam-disi"
gerekce = "Kervan'a özgü bakiye/hediye token takibi (zcode tarzı); OmniRoute'ta karşılığı yok"

[[uc]]
yol = "/bakiye/hediye"
yontem = "POST"
durum = "kapsam-disi"
gerekce = "Kervan'a özgü bakiye/hediye token takibi (zcode tarzı); OmniRoute'ta karşılığı yok"

[[uc]]
yol = "/ws/eklenti"
yontem = "GET"
durum = "kapsam-disi"
gerekce = "Tarayıcı eklentisi WebSocket köprüsü (WebDriver'sız); OmniRoute'ta karşılığı yok"

[[uc]]
yol = "/eklenti/sekmeler"
yontem = "GET"
durum = "kapsam-disi"
gerekce = "Eklentiye bağlı sekmelerin listesi; OmniRoute'ta karşılığı yok"

[[uc]]
yol = "/eklenti/token"
yontem = "GET"
durum = "kapsam-disi"
gerekce = "Eklenti kimlik köprüsü: yalnız uzantı Origin'ine paylaşılan token dağıtır; OmniRoute'ta karşılığı yok (davranış crates/sunucu/tests/p10_eklenti_kimlik_test.rs'te ölçülür)"

[[uc]]
yol = "/eklenti/token"
yontem = "OPTIONS"
durum = "kapsam-disi"
gerekce = "Tarayıcı özel-ağ (private-network) CORS ön denetimi; OmniRoute'ta karşılığı yok"

[[uc]]
yol = "/accounts"
yontem = "GET"
durum = "kapsam-disi"
gerekce = "Kervan'a özgü tarayıcı havuzu ucu; OmniRoute'ta karşılığı yok, parite ölçülemez (hesap durumu, insan kapısında bekleyenler)"

[[uc]]
yol = "/accounts/{i}/devam"
yontem = "POST"
durum = "kapsam-disi"
gerekce = "Kervan'a özgü tarayıcı havuzu ucu; OmniRoute'ta karşılığı yok, parite ölçülemez (insan kapısı çözülünce hesabı geri alır)"

[[uc]]
yol = "/accounts/{i}/duraklat"
yontem = "POST"
durum = "kapsam-disi"
gerekce = "Kervan'a özgü tarayıcı havuzu ucu; OmniRoute'ta karşılığı yok, parite ölçülemez (elle duraklatma; pool.rs birim testiyle)"

[[uc]]
yol = "/accounts/{i}/goster"
yontem = "POST"
durum = "kapsam-disi"
gerekce = "Kervan'a özgü tarayıcı havuzu ucu; OmniRoute'ta karşılığı yok, parite ölçülemez (gizli tarayıcı penceresini öne getirir)"

[[uc]]
yol = "/accounts/{i}/gizle"
yontem = "POST"
durum = "kapsam-disi"
gerekce = "Kervan'a özgü tarayıcı havuzu ucu; OmniRoute'ta karşılığı yok, parite ölçülemez (pencereyi gizler ve insanın elinden bırakır)"

[[uc]]
yol = "/kimlikler"
yontem = "GET"
durum = "kapsam-disi"
gerekce = "Kervan'a özgü tarayıcı havuzu ucu; OmniRoute'ta karşılığı yok, parite ölçülemez (profil bazlı kimlik listesi)"

[[uc]]
yol = "/kimlikler/{k}/goster"
yontem = "POST"
durum = "kapsam-disi"
gerekce = "Kervan'a özgü tarayıcı havuzu ucu; OmniRoute'ta karşılığı yok, parite ölçülemez (profil adıyla pencereyi öne getirir)"

[[uc]]
yol = "/kimlikler/{k}/gizle"
yontem = "POST"
durum = "kapsam-disi"
gerekce = "Kervan'a özgü tarayıcı havuzu ucu; OmniRoute'ta karşılığı yok, parite ölçülemez (profil adıyla pencereyi gizler)"

[[uc]]
yol = "/kota"
yontem = "GET"
durum = "kapsam-disi"
gerekce = "Kervan'ın kota öğrenicisinin gözlemi (gerçek kullanım, çıkarılan limit); OmniRoute'ta karşılığı yok"

[[uc]]
yol = "/yonlendirme"
yontem = "GET"
durum = "kapsam-disi"
gerekce = "Akıllı yönlendirici durum ve aday dökümü; OmniRoute'ta karşılığı yok (p9_auto_rest_test.rs'te ölçülür)"

[[uc]]
yol = "/yonlendirme"
yontem = "POST"
durum = "kapsam-disi"
gerekce = "Çalışma anı strateji değişimi; OmniRoute'ta karşılığı yok (geçersiz ad reddi p9_auto_rest_test.rs'te ölçülür)"
```
