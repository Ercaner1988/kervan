<!-- URETILMIS_DURUM_HASH: 9d347c4c6919ae0379967670135401dcf8320881436511215b86c80b2cc9e6bb -->
# DURUM.md — Kervan Aktarım Durum Defteri (ÜRETİLMİŞTİR)

> ⚠️ BU DOSYA OTOMATİK OLARAK ÜRETİLMİŞTİR. ELLE DÜZENLEMEYİNİZ.
> Düzenlemeler kapı ve defter araçları tarafından ezilir.

- **Mevcut Faz:** P3
- **Son Güncelleme:** 2026-10-03T11:55:26.925Z
- **Tüm Kapılar Yeşil mi:** ✅ EVET

---

## 1. Kapı Sonuçları (port/durum.json)

| Kapı Adı | Çıkış Kodu | Süre (ms) | Log Dosyası |
|---|:---:|:---:|---|
| makine-sakin | ✅ 0 | 580 | [makine-sakin.log](port\kayit\P3\makine-sakin.log) |
| kabul-haritasi | ✅ 0 | 14829 | [kabul-haritasi.log](port\kayit\P3\kabul-haritasi.log) |
| ulasilabilirlik | ✅ 0 | 2809 | [ulasilabilirlik.log](port\kayit\P3\ulasilabilirlik.log) |
| sabit-yanit | ✅ 0 | 388 | [sabit-yanit.log](port\kayit\P3\sabit-yanit.log) |
| uc-kaydi | ✅ 0 | 3987 | [uc-kaydi.log](port\kayit\P3\uc-kaydi.log) |
| muhur-kontrol | ✅ 0 | 212 | [muhur-kontrol.log](port\kayit\P3\muhur-kontrol.log) |
| rapor-kontrol | ✅ 0 | 158 | [rapor-kontrol.log](port\kayit\P3\rapor-kontrol.log) |
| kulliyat-tara | ✅ 0 | 939 | [kulliyat-tara.log](port\kayit\P3\kulliyat-tara.log) |
| mutasyon-kontrol | ✅ 0 | 7083 | [mutasyon-kontrol.log](port\kayit\P3\mutasyon-kontrol.log) |
| durum-kontrol | ✅ 0 | 211 | [durum-kontrol.log](port\kayit\P3\durum-kontrol.log) |
| defter-kontrol | ✅ 0 | 145 | [defter-kontrol.log](port\kayit\P3\defter-kontrol.log) |

---

## 2. HTTP Rota Durumu (port/uclar.toml)

```toml
# port/uclar.toml
# Kervan HTTP Rota Kayıt Defteri

[[uc]]
yol = "/v1/chat/completions"
yontem = "POST"
durum = "kapsam-disi"
# Not: P4/P5 fazlarında 'gercek' durumuna terfi ettirilecek ve nonce'lu test kimliği atanacaktır.

[[uc]]
yol = "/v1/models"
yontem = "GET"
durum = "kapsam-disi"
# Not: P5 fazında 'gercek' durumuna terfi ettirilecektir.

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
