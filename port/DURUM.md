<!-- URETILMIS_DURUM_HASH: c6ab852fde8571ee65f06b41db1d35f2426e79e821f93b99fccfad01031d5dc4 -->
# DURUM.md — Kervan Aktarım Durum Defteri (ÜRETİLMİŞTİR)

> ⚠️ BU DOSYA OTOMATİK OLARAK ÜRETİLMİŞTİR. ELLE DÜZENLEMEYİNİZ.
> Düzenlemeler kapı ve defter araçları tarafından ezilir.

- **Mevcut Faz:** P8
- **Son Güncelleme:** 2026-10-03T18:25:52.582Z
- **Tüm Kapılar Yeşil mi:** ✅ EVET

---

## 1. Kapı Sonuçları (port/durum.json)

| Kapı Adı | Çıkış Kodu | Süre (ms) | Log Dosyası |
|---|:---:|:---:|---|
| makine-sakin | ✅ 0 | 503 | [makine-sakin.log](port\kayit\P8\makine-sakin.log) |
| kabul-haritasi | ✅ 0 | 77 | [kabul-haritasi.log](port\kayit\P8\kabul-haritasi.log) |
| ulasilabilirlik | ✅ 0 | 5142 | [ulasilabilirlik.log](port\kayit\P8\ulasilabilirlik.log) |
| sabit-yanit | ✅ 0 | 92 | [sabit-yanit.log](port\kayit\P8\sabit-yanit.log) |
| uc-kaydi | ✅ 0 | 20219 | [uc-kaydi.log](port\kayit\P8\uc-kaydi.log) |
| muhur-kontrol | ✅ 0 | 281 | [muhur-kontrol.log](port\kayit\P8\muhur-kontrol.log) |
| rapor-kontrol | ✅ 0 | 65 | [rapor-kontrol.log](port\kayit\P8\rapor-kontrol.log) |
| kulliyat-tara | ✅ 0 | 414 | [kulliyat-tara.log](port\kayit\P8\kulliyat-tara.log) |
| mutasyon-kontrol | ✅ 0 | 28073 | [mutasyon-kontrol.log](port\kayit\P8\mutasyon-kontrol.log) |
| defter-kontrol | ✅ 0 | 108 | [defter-kontrol.log](port\kayit\P8\defter-kontrol.log) |
| durum-kontrol | ✅ 0 | 88 | [durum-kontrol.log](port\kayit\P8\durum-kontrol.log) |
| sse-ve-tool-test | ✅ 0 | 7463 | [sse-ve-tool-test.log](port\kayit\P8\sse-ve-tool-test.log) |

---

## 2. HTTP Rota Durumu (port/uclar.toml)

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

[[uc]]
yol = "/telemetri"
yontem = "GET"
durum = "kapsam-disi"

[[uc]]
yol = "/bakiye"
yontem = "GET"
durum = "kapsam-disi"

[[uc]]
yol = "/bakiye/hediye"
yontem = "POST"
durum = "kapsam-disi"

[[uc]]
yol = "/ws/eklenti"
yontem = "GET"
durum = "kapsam-disi"

[[uc]]
yol = "/eklenti/sekmeler"
yontem = "GET"
durum = "kapsam-disi"

[[uc]]
yol = "/accounts"
yontem = "GET"
durum = "kapsam-disi"

[[uc]]
yol = "/accounts/{i}/devam"
yontem = "POST"
durum = "kapsam-disi"

[[uc]]
yol = "/accounts/{i}/goster"
yontem = "POST"
durum = "kapsam-disi"

[[uc]]
yol = "/accounts/{i}/gizle"
yontem = "POST"
durum = "kapsam-disi"

[[uc]]
yol = "/kimlikler"
yontem = "GET"
durum = "kapsam-disi"

[[uc]]
yol = "/kimlikler/{k}/goster"
yontem = "POST"
durum = "kapsam-disi"

[[uc]]
yol = "/kimlikler/{k}/gizle"
yontem = "POST"
durum = "kapsam-disi"

[[uc]]
yol = "/kota"
yontem = "GET"
durum = "kapsam-disi"
```
