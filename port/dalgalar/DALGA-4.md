# PRD: Dalga 4 — Gelişmiş Sağlayıcı Eklentileri ve Model Kataloğu

## 1. Problem Statement & Scope
OmniRoute sağlayıcı genişleme mimarisi (providers/services) Kervan'ın modüler yapısına entegre edilecektir.

## 2. Goals and Non-Goals
### 2.1 Goals
- G1: Çoklu sağlayıcı kimlik doğrulama ve anahtar rotasyonu
- G2: Model yetenek matrisinin (tool_calls, reasoning, multimodal) otomatik keşfi

### 2.2 Non-Goals
- Üçüncü parti sağlayıcıların tescilli hesap oluşturma otomasyonları.

## 3. Users and Personas
- **Ercan**: Sağlayıcı entegrasyon onayı
- **Builder**: Adaptör geliştiricisi

## 4. Functional Requirements
### FR-4.1: Dinamik Model Keşif Ucu
Sağlayıcıların güncel model listeleri periyodik olarak sorgulanıp `/v1/models` listesine eklenir.
*Acceptance:* Yeni eklenen bir sağlayıcı için 60 saniye içinde model listesi güncellenir ve test doğrulanır.
