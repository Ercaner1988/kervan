<!-- URETILMIS_DURUM_HASH: 13a38e77f66a5626559aa2a00d1c4f723ed41addab3c5027402d81570c72da53 -->
# DURUM.md — Kervan Aktarım Durum Defteri (ÜRETİLMİŞTİR)

> ⚠️ BU DOSYA araclar/kapilar/durum-uret.ts ÇIKTISIDIR. ELLE DÜZENLEMEYİNİZ.
> durum-kontrol bu dosyayı port/durum.json'dan yeniden üretip bayt bayt karşılaştırır.

- **Mevcut Faz:** P3
- **Üretim Zamanı (durum.json):** 2026-10-13T22:36:05.287Z
- **Üretici Özeti:** sha256:5e89ad0927f0977ac3956950facc362696202bc8bb589a30d30d946b247dafd6
- **Tüm Kapılar Yeşil mi:** ❌ HAYIR

---

## 1. Kapı Sonuçları

| Kapı Adı | Çıkış Kodu | Süre (ms) | Log Dosyası |
|---|:---:|:---:|---|
| makine-sakin | ✅ 0 | 60149 | port\kayit\P3\makine-sakin.log |
| kabul-haritasi | ❌ 124 | 900554 | port\kayit\P3\kabul-haritasi.log |
| ulasilabilirlik | ✅ 0 | 1479 | port\kayit\P3\ulasilabilirlik.log |
| sabit-yanit | ✅ 0 | 139 | port\kayit\P3\sabit-yanit.log |
| uc-kaydi | ✅ 0 | 2653 | port\kayit\P3\uc-kaydi.log |
| muhur-kontrol | ✅ 0 | 16968 | port\kayit\P3\muhur-kontrol.log |
| kulliyat-tara | ✅ 0 | 9419 | port\kayit\P3\kulliyat-tara.log |
| mutasyon-kontrol | ✅ 0 | 198529 | port\kayit\P3\mutasyon-kontrol.log |
| durum-kontrol | ❌ 1 | 542 | port\kayit\P3\durum-kontrol.log |
| defter-kontrol | ✅ 0 | 30121 | port\kayit\P3\defter-kontrol.log |
| rapor-kontrol | ❌ 1 | 150 | port\kayit\P3\rapor-kontrol.log |

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
```
