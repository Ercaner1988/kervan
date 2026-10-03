<!-- URETILMIS_DURUM_HASH: 0c41bd47d8a8ef383c2b0a0a22f272b06ab50362572069afd7b8c9d224c12389 -->
# DURUM.md — Kervan Aktarım Durum Defteri (ÜRETİLMİŞTİR)

> ⚠️ BU DOSYA OTOMATİK OLARAK ÜRETİLMİŞTİR. ELLE DÜZENLEMEYİNİZ.
> Düzenlemeler kapı ve defter araçları tarafından ezilir.

- **Mevcut Faz:** P10
- **Son Güncelleme:** 2026-10-03T20:28:13.732Z
- **Tüm Kapılar Yeşil mi:** ❌ HAYIR

---

## 1. Kapı Sonuçları (port/durum.json)

| Kapı Adı | Çıkış Kodu | Süre (ms) | Log Dosyası |
|---|:---:|:---:|---|
| makine-sakin | ✅ 0 | 687 | [makine-sakin.log](port\kayit\P10\makine-sakin.log) |
| kabul-haritasi | ❌ 1 | 350840 | [kabul-haritasi.log](port\kayit\P10\kabul-haritasi.log) |
| ulasilabilirlik | ✅ 0 | 2120 | [ulasilabilirlik.log](port\kayit\P10\ulasilabilirlik.log) |
| sabit-yanit | ✅ 0 | 182 | [sabit-yanit.log](port\kayit\P10\sabit-yanit.log) |
| uc-kaydi | ✅ 0 | 15094 | [uc-kaydi.log](port\kayit\P10\uc-kaydi.log) |
| muhur-kontrol | ✅ 0 | 116 | [muhur-kontrol.log](port\kayit\P10\muhur-kontrol.log) |
| rapor-kontrol | ❌ 1 | 95 | [rapor-kontrol.log](port\kayit\P10\rapor-kontrol.log) |
| kulliyat-tara | ✅ 0 | 589 | [kulliyat-tara.log](port\kayit\P10\kulliyat-tara.log) |
| mutasyon-kontrol | ✅ 0 | 52216 | [mutasyon-kontrol.log](port\kayit\P10\mutasyon-kontrol.log) |
| defter-kontrol | ✅ 0 | 100 | [defter-kontrol.log](port\kayit\P10\defter-kontrol.log) |
| envanter | ✅ 0 | 1877 | [envanter.log](port\kayit\P10\envanter.log) |
| dalga-uret | ✅ 0 | 106 | [dalga-uret.log](port\kayit\P10\dalga-uret.log) |

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
