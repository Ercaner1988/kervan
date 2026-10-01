# ADR 0004: Hata Sınıflandırma, Temizleme ve Güvenlik Sanitizasyonu

- **Tarih:** 2026-10-01
- **Durum:** Kabul Edildi (Accepted)
- **Konu:** OmniRoute Sert Kural #12 uyarınca iç hata detaylarının istemciye sızdırılmaması ve tipli hata mimarisi.

## Bağlam
OmniRoute Sert Kural #12 şunu emreder: "Never return raw err.stack / err.message in HTTP / SSE / executor responses — always route through buildErrorBody() or sanitizeErrorMessage()".
Kervan'da da dış dünyaya açılan HTTP/SSE ve MCP yanıtlarında iç dosya yolları, işletim sistemi hataları veya hassas ortam değişkenleri asla bulunmamalıdır.

## Karar
1. **Hata Katmanları:**
   - Kütüphane sandıklarında (`crates/*`): Tipli, `thiserror` tabanlı açık enum hataları (`KervanHata::AgaBaglanamadi`, `KervanHata::ModelBulunamadi`).
   - Uç ve sunucu katmanında (`kervan-sunucu`): İstemciye standart OpenAI hata JSON formatında (`{ "error": { "message": "...", "type": "...", "code": "..." } }`) dönen `hata_temizle` fonksiyonu.
2. **Yığın İzi Sızıntı Koruması:** `at /` veya Windows yol kalıpları (`C:\...`) içeren yığın izleri yanıt gövdesinden otomatik olarak budanacaktır.
3. **Parite Testi:** Hata yanıtlarının yığın izi içermediği birim testlerle (`assert!(!hata.contains("at /"))`) doğrulanacaktır.
