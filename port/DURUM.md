<!-- URETILMIS_DURUM_HASH: 8a3145bb6112fca5c3c4318460c90bce1571749b6fe0bff70d1fe988717ab108 -->
# DURUM.md — Kervan Aktarım Durum Defteri (ÜRETİLMİŞTİR)

> ⚠️ BU DOSYA OTOMATİK OLARAK ÜRETİLMİŞTİR. ELLE DÜZENLEMEYİNİZ.
> Düzenlemeler kapı ve defter araçları tarafından ezilir.

- **Mevcut Faz:** P2
- **Son Güncelleme:** 2026-10-03T11:42:24.323Z
- **Tüm Kapılar Yeşil mi:** ✅ EVET

---

## 1. Kapı Sonuçları (port/durum.json)

| Kapı Adı | Çıkış Kodu | Süre (ms) | Log Dosyası |
|---|:---:|:---:|---|
| makine-sakin | ✅ 0 | 603 | [makine-sakin.log](port\kayit\P2\makine-sakin.log) |
| kabul-haritasi | ✅ 0 | 19299 | [kabul-haritasi.log](port\kayit\P2\kabul-haritasi.log) |
| ulasilabilirlik | ✅ 0 | 2412 | [ulasilabilirlik.log](port\kayit\P2\ulasilabilirlik.log) |
| sabit-yanit | ✅ 0 | 360 | [sabit-yanit.log](port\kayit\P2\sabit-yanit.log) |
| uc-kaydi | ✅ 0 | 4000 | [uc-kaydi.log](port\kayit\P2\uc-kaydi.log) |
| muhur-kontrol | ✅ 0 | 248 | [muhur-kontrol.log](port\kayit\P2\muhur-kontrol.log) |
| rapor-kontrol | ✅ 0 | 157 | [rapor-kontrol.log](port\kayit\P2\rapor-kontrol.log) |

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
