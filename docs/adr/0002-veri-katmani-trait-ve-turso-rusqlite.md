# ADR 0002: Veri Katmanı Soyutlaması (`Depo` Trait) ve Turso / rusqlite Stratejisi

- **Tarih:** 2026-10-01
- **Durum:** Kabul Edildi (Accepted)
- **Konu:** OmniRoute'un 193 SQL göçü ve 136 tabloluk veritabanı şemasının Rust tarafında güvenli ve esnek yönetimi.

## Bağlam
OmniRoute; kullanıcı ayarları, oturumlar, API anahtarları, çağrı günlükleri, harcama telemetrisi, devre kesici durumları, FTS arama ve vektör bellekleri için SQLite kullanmaktadır (193 SQL göçü).
Turso (`0.8.0-pre.11`), libSQL motoru üzerinde çalışan güçlü bir yerel/bulut veri sürücüsüdür ancak `COMPAT.md` uyarınca `WITH RECURSIVE` kısıtları, `INSTEAD OF` tetikleyici eksikliği ve Windows platformunda `os error 33` kilitlenme hassasiyetleri barındırmaktadır.

## Karar
1. **`Depo` Trait Arkasında İzolasyon:** Tüm iş mantığı ve API uçları doğrudan bir SQL sürücüsüne değil, `Depo` trait'ine bağlanacaktır. Hiçbir rota veya işleyici ham SQL çalıştırmayacaktır (Sert Kural #5).
2. **Çift Sürücü Mimarisi (Zopay Deseni):**
   - `rusqlite (bundled)`: Tüm SQLite FTS5, tetikleyici, `WITH RECURSIVE` ve `VACUUM INTO` yeteneklerini sıfır dış bağımlılıkla garanti eden temel yerel motor.
   - `turso`: Yüksek eşzamanlılık ve uzak/vektör senkronizasyonu gerektiren alanlar için ölçüm kapısından geçen modern motor.
3. **Şifreleme Uyumluluğu:** OmniRoute'un kimlik bilgilerini şifrelediği AES-256-GCM formatı (`STORAGE_ENCRYPTION_KEY`) Rust tarafında `aes-gcm` sandığı ile birebir uyumlu okunup yazılacaktır.
4. **Bağlantı Yönetimi:** Windows `os error 33` kilitlenmelerini önlemek amacıyla bağlantılar kısa ömürlü açılıp bırakılacak veya tek yazıcı / çok okuyucu WAL kuralı uygulanacaktır.
