<!-- URETILMIS_DURUM_HASH: 807c2bd4c4f1d61367af792c7f6c952602f23f3d8429e5793d9c0446264a7101 -->
# DURUM.md — Kervan Aktarım Durum Defteri (ÜRETİLMİŞTİR)

> ⚠️ BU DOSYA OTOMATİK OLARAK ÜRETİLMİŞTİR. ELLE DÜZENLEMEYİNİZ.
> Düzenlemeler kapı ve defter araçları tarafından ezilir.

- **Mevcut Faz:** P7
- **Son Güncelleme:** 2026-10-03T13:56:09.841Z
- **Tüm Kapılar Yeşil mi:** ✅ EVET

---

## 1. Kapı Sonuçları (port/durum.json)

| Kapı Adı | Çıkış Kodu | Süre (ms) | Log Dosyası |
|---|:---:|:---:|---|
| makine-sakin | ✅ 0 | 546 | [makine-sakin.log](port\kayit\P7\makine-sakin.log) |
| kabul-haritasi | ✅ 0 | 100 | [kabul-haritasi.log](port\kayit\P7\kabul-haritasi.log) |
| ulasilabilirlik | ✅ 0 | 1843 | [ulasilabilirlik.log](port\kayit\P7\ulasilabilirlik.log) |
| sabit-yanit | ✅ 0 | 185 | [sabit-yanit.log](port\kayit\P7\sabit-yanit.log) |
| uc-kaydi | ✅ 0 | 3735 | [uc-kaydi.log](port\kayit\P7\uc-kaydi.log) |
| muhur-kontrol | ✅ 0 | 160 | [muhur-kontrol.log](port\kayit\P7\muhur-kontrol.log) |
| rapor-kontrol | ✅ 0 | 119 | [rapor-kontrol.log](port\kayit\P7\rapor-kontrol.log) |
| kulliyat-tara | ✅ 0 | 783 | [kulliyat-tara.log](port\kayit\P7\kulliyat-tara.log) |
| mutasyon-kontrol | ✅ 0 | 37727 | [mutasyon-kontrol.log](port\kayit\P7\mutasyon-kontrol.log) |
| defter-kontrol | ✅ 0 | 64 | [defter-kontrol.log](port\kayit\P7\defter-kontrol.log) |
| durum-kontrol | ✅ 0 | 82 | [durum-kontrol.log](port\kayit\P7\durum-kontrol.log) |
| direnc-test | ✅ 0 | 784 | [direnc-test.log](port\kayit\P7\direnc-test.log) |
| dayaniklilik-test | ✅ 0 | 4827 | [dayaniklilik-test.log](port\kayit\P7\dayaniklilik-test.log) |
| resilience-test | ✅ 0 | 7920 | [resilience-test.log](port\kayit\P7\resilience-test.log) |

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
