# Kervan Arka Uç İstekleri (Pano Gereksinimleri)

> **Standart:** `KERVAN-PANO-ANTIGRAVITY-ISTEMI.md` §7  
> **Muhatap:** Claude (arka uç mimarı & DALGA planlamacısı)  
> **Durum:** D2, D3, D4 olarak işaretlenmiş pano bölümlerinin gerçek Kervan arka uç uçlarına dönüştürülmesi için hazırlanan resmi istek defteri.

---

## İ-1: Kalıcı Model Kombinasyonları ve Zincir Kalıcılığı
- **Ekran / alan:** Ekran 2 → Kombolar & Yedek Zincirleri
- **OmniRoute karşılığı:** `src/app/(dashboard)/dashboard/combos`
- **İstenen:** `GET /combos` ve `POST /combos` (HTTP uçları)
- **Yanıt biçimi (öneri):**
  ```json
  [
    {
      "ad": "akilli-kodlama",
      "birincil_model": "kervan/gemini-2.5-pro",
      "yedek_modeller": ["kervan/kimi-k1.5", "kervan/deepseek-chat"],
      "tetik_kosullari": ["429", "5xx", "timeout"]
    }
  ]
  ```
- **Neden şimdi:** Ekran 2'de kombinasyon yönetimi DALGA-2 rozetinden canlıya geçebilsin.

---

## İ-2: Kaos Testi ve Hata Enjeksiyon Sinyalleri
- **Ekran / alan:** Ekran 2 → Dayanıklılık & Kaos Testi
- **OmniRoute karşılığı:** `src/app/(dashboard)/dashboard/chaos`
- **İstenen:** `POST /chaos/enjekte` (gecikme simülasyonu, 503 enjeksiyonu)
- **Yanıt biçimi (öneri):**
  ```json
  {
    "aktif": true,
    "gecikme_ms": 2500,
    "hata_orani": 0.1
  }
  ```
- **Neden şimdi:** Dayanıklılık testlerini arayüz üzerinden tetikleyebilmek için (DALGA-2).

---

## İ-3: Kalıcı SQLite/Turso Kullanım ve Maliyet Defteri
- **Ekran / alan:** Ekran 3 → Maliyet Analitiği ve Token Sayaçları
- **OmniRoute karşılığı:** `src/app/(dashboard)/dashboard/costs` & `tokens`
- **İstenen:** `GET /analitik/maliyet` (haftalık, aylık token dökümü)
- **Yanıt biçimi (öneri):**
  ```json
  {
    "toplam_girdi_token": 1420500,
    "toplam_cikti_token": 350200,
    "tahmini_tasarruf_usd": 14.85,
    "gunluk_dagilim": [
      { "tarih": "2026-10-08", "token": 450000, "maliyet_usd": 0.0 }
    ]
  }
  ```
- **Neden şimdi:** Ekran 3'te boş kalan grafik alanının gerçek veriyle doldurulması için (DALGA-2).

---

## İ-4: Kalıcı Denetim ve Arşiv Günlükleri
- **Ekran / alan:** Ekran 3 → Denetim ve Günlük Dışa Aktarma
- **OmniRoute karşılığı:** `src/app/(dashboard)/dashboard/audit` & `log-export`
- **İstenen:** `GET /gunluk/arsiv` ve `GET /gunluk/disa-aktar` (JSON / CSV akışı)
- **Yanıt biçimi (öneri):**
  ```json
  [
    {
      "id": "evt-1092",
      "zaman": "2026-10-09T14:22:01Z",
      "seviye": "WARN",
      "olay": "429_kota_asimi",
      "saglayici": "copilot"
    }
  ]
  ```
- **Neden şimdi:** Güvenlik ve denetim kayıtlarının dışa aktarımı için (DALGA-2).

---

## İ-5: Radar Sağlayıcı Hız ve Gecikme Metrikleri
- **Ekran / alan:** Ekran 3 → Radar Sağlayıcı Analitiği
- **OmniRoute karşılığı:** `src/app/(dashboard)/dashboard/radar`
- **İstenen:** `GET /telemetri/radar` (sağlayıcı bazında p50, p90, p99 gecikme)
- **Yanıt biçimi (öneri):**
  ```json
  [
    { "saglayici": "gemini", "p50_ms": 820, "p95_ms": 1450, "hata_orani": 0.001 },
    { "saglayici": "kimi", "p50_ms": 640, "p95_ms": 1100, "hata_orani": 0.002 }
  ]
  ```
- **Neden şimdi:** Ekran 3 görsel hız radarı grafiği için (DALGA-3).

---

## İ-6: Bağlam Sıkıştırma Kipleri ve Başarı Oranları
- **Ekran / alan:** Ekran 4 → Bağlam Sıkıştırma (Context Compression)
- **OmniRoute karşılığı:** `src/app/(dashboard)/dashboard/context` & `compression`
- **İstenen:** `GET /baglam/sikistirma/durum` ve `POST /baglam/kip-sec`
- **Yanıt biçimi (öneri):**
  ```json
  {
    "aktif_kip": "rtk_caveman",
    "toplam_orijinal_token": 850000,
    "toplam_sikistirilmis_token": 290000,
    "tasarruf_orani": 0.658
  }
  ```
- **Neden şimdi:** Ekran 4 bağlam paneli DALGA-4 rozetinden canlı kontrole geçebilsin.

---

## İ-7: Harici Ajanlar ve A2A Protokol Durumu
- **Ekran / alan:** Ekran 2 → A2A & Ajan Orkestrasyonu
- **OmniRoute karşılığı:** `src/app/(dashboard)/dashboard/a2a` & `acp-agents`
- **İstenen:** `GET /a2a/ajanlar` (bağlı ajanlar ve yetkileri)
- **Yanıt biçimi (öneri):**
  ```json
  [
    { "ajan_id": "arastirma-ajani-1", "protokol": "a2a_v1", "durum": "bosta" }
  ]
  ```
- **Neden şimdi:** Ekran 2 ajan yönlendirme sekmesi için (DALGA-4).
