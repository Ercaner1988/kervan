# PRD: Dalga 3 — Pano ve İstatistik Arayüzü (egui Dashboard)

## 1. Problem Statement & Scope
122 Next.js pano sayfasındaki telemetri ve yönetim görünümleri Kervan'ın egui penceresine yerleştirilecektir.

## 2. Goals and Non-Goals
### 2.1 Goals
- G1: Sağlayıcı sağlık durumu ve gecikme grafiklerinin görselleştirilmesi
- G2: Aktif kombo ve strateji seçici panelleri

### 2.2 Non-Goals
- Web tarayıcı dashboard'u (yalnızca yerel egui penceresi).

## 3. Users and Personas
- **Ercan**: UI/UX denetimi
- **Builder**: egui bileşen geliştiricisi

## 4. Functional Requirements
### FR-3.1: Canlı Metrik Grafiği
Sağlayıcı gecikmeleri ve ardışık hata oranları egui panelinde canlı grafik olarak çizilir.
*Acceptance:* Simüle edilen 5xx hatalarında kesici açıldığında grafik kırmızıya döner ve bildirim paneli güncellenir.
