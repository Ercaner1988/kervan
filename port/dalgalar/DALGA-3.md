# PRD: Dalga 3 — Pano ve İstatistik Arayüzü (egui Dashboard)

## 1. Problem Statement & Scope
122 Next.js pano sayfasındaki telemetri ve yönetim görünümleri Kervan'ın egui penceresine yerleştirilecektir.

## 2. Goals and Non-Goals
### 2.1 Goals
- G1: Sağlayıcı sağlık durumu ve gecikme grafiklerinin görselleştirilmesi
- G2: Aktif kombo ve strateji seçici panelleri
- G3 (OpenCode esinli): 358 sağlayıcılık liste yerine `katla` destekli süzülen tek komut/arama kutusu ile sağlayıcı ekleme
- G4 (usage_monitor esinli): Yerel CLI oturumlarından (Claude Code, Codex) API anahtarsız kota ve kalan süre senkronizasyonu
- G5: Kompakt alt telemetri ve kota sıfırlanma sayacı şeridi (`render_canli_akis`)

### 2.2 Non-Goals
- Web tarayıcı dashboard'u, Tauri veya WebView (yalnızca yerel egui + kilim-tema).

## 3. Users and Personas
- **Ercan**: UI/UX denetimi
- **Builder**: egui bileşen geliştiricisi

## 4. Functional Requirements
### FR-3.1: Canlı Metrik Grafiği & Kota Sayacı
Sağlayıcı gecikmeleri, ardışık hata oranları ve kalan kota sıfırlanma süreleri egui panelinde canlı çizilir.
*Acceptance:* Simüle edilen 5xx hatalarında kesici açıldığında grafik kırmızıya döner; kota bittiğinde `usage_monitor` tarzı geri sayım sayacı çalışır.

### FR-3.2: Kompakt Sağlayıcı Seçici
358 sağlayıcıyı listeleyen hantal ekran kaldırılır; klavye dostu, aranabilir açılır liste üzerinden hesap eklenir.
*Acceptance:* Arama kutusuna yazıldıkça sağlayıcılar filtrelenir; seçim sonrası tarayıcı profili Enter'sız bağlanır.
