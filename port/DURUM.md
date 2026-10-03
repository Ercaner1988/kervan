<!-- URETILMIS_DURUM_HASH: 81657e666d40abc2788c41305ebbaecf231be195454e30cf7a07daf72a331690 -->
# DURUM.md — Kervan Aktarım Durum Defteri (ÜRETİLMİŞTİR)

> ⚠️ BU DOSYA OTOMATİK OLARAK ÜRETİLMİŞTİR. ELLE DÜZENLEMEYİNİZ.
> Düzenlemeler kapı ve defter araçları tarafından ezilir.

- **Mevcut Faz:** P9
- **Son Güncelleme:** 2026-10-03T19:59:09.007Z
- **Tüm Kapılar Yeşil mi:** ✅ EVET

---

## 1. Kapı Sonuçları (port/durum.json)

| Kapı Adı | Çıkış Kodu | Süre (ms) | Log Dosyası |
|---|:---:|:---:|---|
| makine-sakin | ✅ 0 | 594 | [makine-sakin.log](port\kayit\P9\makine-sakin.log) |
| kabul-haritasi | ✅ 0 | 39438 | [kabul-haritasi.log](port\kayit\P9\kabul-haritasi.log) |
| ulasilabilirlik | ✅ 0 | 2006 | [ulasilabilirlik.log](port\kayit\P9\ulasilabilirlik.log) |
| sabit-yanit | ✅ 0 | 139 | [sabit-yanit.log](port\kayit\P9\sabit-yanit.log) |
| uc-kaydi | ✅ 0 | 86544 | [uc-kaydi.log](port\kayit\P9\uc-kaydi.log) |
| muhur-kontrol | ✅ 0 | 433 | [muhur-kontrol.log](port\kayit\P9\muhur-kontrol.log) |
| rapor-kontrol | ✅ 0 | 271 | [rapor-kontrol.log](port\kayit\P9\rapor-kontrol.log) |
| kulliyat-tara | ✅ 0 | 2358 | [kulliyat-tara.log](port\kayit\P9\kulliyat-tara.log) |
| mutasyon-kontrol | ✅ 0 | 67786 | [mutasyon-kontrol.log](port\kayit\P9\mutasyon-kontrol.log) |
| defter-kontrol | ✅ 0 | 138 | [defter-kontrol.log](port\kayit\P9\defter-kontrol.log) |
| strateji-kumesi | ✅ 0 | 109 | [strateji-kumesi.log](port\kayit\P9\strateji-kumesi.log) |
| p9-strateji-test | ✅ 0 | 7068 | [p9-strateji-test.log](port\kayit\P9\p9-strateji-test.log) |

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
