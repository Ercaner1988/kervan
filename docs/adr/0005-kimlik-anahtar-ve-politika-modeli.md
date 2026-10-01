# ADR 0005: Kimlik, API Anahtarları ve Güvenlik Politikası

- **Tarih:** 2026-10-01
- **Durum:** Kabul Edildi (Accepted)
- **Konu:** OmniRoute'un API anahtarı doğrulama, JWT oturumu, hız politikaları ve loopback güvenlik mimarisi.

## Bağlam
OmniRoute; API anahtarı politikaları (IP kısıtlaması, model allowlist, maliyet bütçesi) ve admin JWT oturumları ile çalışmaktadır. Ayrıca Sert Kural #15 ve #17 gereğince çocuk süreç başlatan veya makineyi etkileyen rotaların ağa açılması kesinlikle yasaktır (`127.0.0.1` loopback zorunluluğu).

## Karar
1. **Varsayılan Loopback Güvencesi (`127.0.0.1`):** Kervan varsayılan olarak yalnızca yerel `127.0.0.1` adresine bağlanacaktır (§5-1). `0.0.0.0` dinlemesi ancak açık CLI bayrağı (`--host 0.0.0.0`) ve güçlü bir API anahtarı zorunluluğu ile mümkün olacaktır.
2. **Kimlik Doğrulama Katmanı (`kervan-kimlik`):**
   - API anahtarları `kervan_` önekiyle SHA-256 hash'i olarak SQLite deposunda saklanacaktır.
   - Her anahtar için model politikası, saatlik kota ve rate limit uygulanacaktır.
   - Idempotency anahtarları çağrı yapan API anahtarına göre ad alanlanacaktır (v3.8.51 düzeltmesi #15128).
3. **Public Upstream Credentials (Sert Kural #11):** Halka açık OAuth istemci kimlikleri kod içine düz metin gömülmeyecek, `resolve_public_cred` üzerinden çalışma zamanında dinamik çözümlenecektir.
