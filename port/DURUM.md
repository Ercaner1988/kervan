<!-- URETILMIS_DURUM_HASH: a609e1f6009723ce363ef2b756d14a062deb97545f7e2738f5901dfe3406e441 -->
# DURUM.md — Kervan Aktarım Durum Defteri (ÜRETİLMİŞTİR)

> ⚠️ BU DOSYA OTOMATİK OLARAK ÜRETİLMİŞTİR. ELLE DÜZENLEMEYİNİZ.
> Düzenlemeler kapı ve defter araçları tarafından ezilir.

- **Mevcut Faz:** P4
- **Son Güncelleme:** 2026-10-03T12:32:58.247Z
- **Tüm Kapılar Yeşil mi:** ✅ EVET

---

## 1. Kapı Sonuçları (port/durum.json)

| Kapı Adı | Çıkış Kodu | Süre (ms) | Log Dosyası |
|---|:---:|:---:|---|
| makine-sakin | ✅ 0 | 717 | [makine-sakin.log](port\kayit\P4\makine-sakin.log) |
| kabul-haritasi | ✅ 0 | 195 | [kabul-haritasi.log](port\kayit\P4\kabul-haritasi.log) |
| ulasilabilirlik | ✅ 0 | 5891 | [ulasilabilirlik.log](port\kayit\P4\ulasilabilirlik.log) |
| sabit-yanit | ✅ 0 | 479 | [sabit-yanit.log](port\kayit\P4\sabit-yanit.log) |
| uc-kaydi | ✅ 0 | 5089 | [uc-kaydi.log](port\kayit\P4\uc-kaydi.log) |
| muhur-kontrol | ✅ 0 | 360 | [muhur-kontrol.log](port\kayit\P4\muhur-kontrol.log) |
| mutasyon-kontrol | ✅ 0 | 10750 | [mutasyon-kontrol.log](port\kayit\P4\mutasyon-kontrol.log) |
| durum-kontrol | ✅ 0 | 213 | [durum-kontrol.log](port\kayit\P4\durum-kontrol.log) |
| defter-kontrol | ✅ 0 | 144 | [defter-kontrol.log](port\kayit\P4\defter-kontrol.log) |

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
