# ADR 0006: Yapılandırma (`kervan.toml`) ve Durum Veritabanı Senkronizasyonu

- **Tarih:** 2026-10-01
- **Durum:** Kabul Edildi (Accepted)
- **Konu:** Statik kullanıcı ayarları ile dinamik çalışma zamanı durumunun hibrit yönetimi.

## Bağlam
Kervan geleneksel olarak `kervan.toml` dosyası üzerinden ayarlanırken, OmniRoute tüm çalışma zamanı durumunu (oturum çerezleri, devre kesici durumları, kullanım günlükleri, aktif özellik bayrakları) SQLite üzerinde tutmaktadır.

## Karar
1. **İki Katmanlı Yapı:**
   - **Durağan Yapılandırma (`kervan.toml`):** Dinleme portu, varsayılan profil yolları, yerel dosya sistem yolları ve ilk yönetici anahtarları.
   - **Dinamik Durum (SQLite Deposu):** Sağlayıcı devre kesici durumları (`CLOSED`, `DEGRADED`, `OPEN`), hesap oturum bütçeleri, rate-limit pencereleri, çağrı günlükleri ve model önbellekleri.
2. **Kuru Koşu ve Göç (`kervan göç`):**
   - OmniRoute'un `%USERPROFILE%\.omniroute\` veri dizinini Kervan'a aktarmak için bir CLI komutu sunulacaktır.
   - Varsayılan mod **kuru koşu (dry-run)** olacak, hiçbir dosya değiştirilmeden önce satır sayıları ve tablolar raporlanacaktır.
