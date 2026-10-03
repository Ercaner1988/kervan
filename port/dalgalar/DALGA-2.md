# PRD: Dalga 2 — Veritabanı Kalıcılığı ve SQLite/Drizzle Modelleri

## 1. Problem Statement & Scope
OmniRoute anlık görüntüsündeki 136 DB modülünün temel veri şemaları (sağlayıcılar, modeller, kombolar, kullanım istatistikleri) Kervan'ın gömülü SQLite motoruna aktarılacaktır.

## 2. Goals and Non-Goals
### 2.1 Goals
- G1: Sağlayıcı ve hesap kayıtlarının SQLite tablosunda atomik saklanması
- G2: Kombo ve yönlendirme ayarlarının yeniden başlatmalarda korunması

### 2.2 Non-Goals
- Uzak PostgreSQL / Turso senkronizasyonu v2 kapsamı dışındadır.

## 3. Users and Personas
- **Ercan**: Veritabanı şema onayı
- **Builder**: SQLite göç ve sorgu yazıcısı

## 4. Functional Requirements
### FR-2.1: Şema Göç Motoru
Kervan ilk açılışta gömülü SQLite göçlerini sırayla uygular.
*Acceptance:* Temiz bir veritabanında tüm tablolar hatasız oluşturulur ve sürüm tablosunda kaydedilir.
