# Kervan Sade Pano — Bileşen Listesi ve Tasarım Kılavuzu

> **Belge:** `docs/pano/BILESENLER.md`  
> **Standart:** `KERVAN-PANO-ANTIGRAVITY-ISTEMI.md` + `KERVAN-ARAYUZ-ISTEMI.md` + `KERVAN-TANI-ISTEMI.md`  
> **Tema Paleti:** `kilim-tema` (Ortak egui teması: Paslı Beyin, bge-embed-rs, kervan, el-fihrist)  
> **Çözünürlük Hedefi:** 1280 × 800 piksel (pencere ölçeklenebilir)

---

## 1. Renk Paleti (kilim-tema)

### Açık Tema (Gündüz / Kâğıt Lifli)
- **Zemin (`zem`):** `#F4ECDB` (Doğal kâğıt tabanı)
- **Yüzey (`yuzey`):** `#FCF8EF` (Kart ve panel arka planı)
- **Çizgi (`cizgi`):** `#D8CBB2` (Panel ayrımı, hafif kenarlık)
- **Koyu Çizgi (`cizgi_koyu`):** `#B9A88C` (Vurgulu kenarlık)
- **Mürekkep (`murekkep`):** `#3A2D21` (Ana metin - yüksek kontrast)
- **Soluk (`soluk`):** `#7A6A55` (İkincil metin, etiketler, ipuçları)
- **Çini (`cini`):** `#16537E` (Vurgu, bağlantı, seçili sekme)
- **Mercan (`mercan`):** `#C04F42` (Uyarı, hata, doğrulama duvarı)
- **Hardal (`hardal`):** `#D9A441` (Kota bekleme, geçici durum)
- **Yeşil (`yesil`):** `#2E7D4F` (Hazır, başarılı, canlı bağlantı)

### Koyu Tema (Gece / Kök Boya)
- **Zemin (`zem`):** `#241F19` (Koyu zemin)
- **Yüzey (`yuzey`):** `#2E2820` (Kart ve panel yüzeyi)
- **Çizgi (`cizgi`):** `#4A4034` (Koyu panel ayrımı)
- **Koyu Çizgi (`cizgi_koyu`):** `#6E604C` (Vurgulu kenarlık)
- **Mürekkep (`murekkep`):** `#E8DFD0` (Açık krem metin)
- **Soluk (`soluk`):** `#9E8F7A` (İkincil metin)
- **Çini (`cini`):** `#4A88B4` (Açık çini vurgu)
- **Mercan (`mercan`):** `#E06A5D` (Açık mercan hata)
- **Hardal (`hardal`):** `#E5B654` (Açık hardal uyarı)
- **Yeşil (`yesil`):** `#48A26B` (Canlı açık yeşil hazır)

---

## 2. Ana Pencere Düzeni ve Bölgeler

```
┌── Sol Menü ──┬── Üst Şerit (Tüm ekranlar ortak) ────────────────────────────────────────────────────────┐
│ [1] Oturumlar│ Kervan · 127.0.0.1:20129  ● açık   Strateji: [least-used ▾]   [Tanı al]   Tema: ◐   ⚙ │
│ [2] Rota     ├─────────────────────────────────────────┬────────────────────────────────────────────────┤
│ [3] İzleme   │ Ekran Ana İçeriği                       │ Sağ Ayrıntı Paneli (veya genişletilmiş alan)   │
│ [4] Bağlam   │ (Seçili ekrana göre değişir)            │ - Ekran 1'de: Hesap detay, kota, tanı kartı    │
│ [5] Ayarlar  │                                         │ - Ekran 2-5'te: Yan ayrıntı / ayar detayı      │
│              ├─────────────────────────────────────────┴────────────────────────────────────────────────┤
│              │ Alt Şerit: Canlı Akış (son 50 istek: model · hesap · süre · durum)                       │
└──────────────┴──────────────────────────────────────────────────────────────────────────────────────────┘
```

### Bölge Ölçüleri:
- **Sol Gezinti Şeridi:** Genişlik 64 px (dar ikon + kısa ad) veya 160 px (geniş mod)
- **Üst Şerit:** Yükseklik 44 px (sabit)
- **Alt Şerit (Canlı Akış):** Yükseklik 80 px (katlanabilir, son N istek özeti)
- **Orta Çalışma Alanı:** 1280 - 64 = 1216 px genişlik, 800 - 44 - 80 = 676 px yükseklik

---

## 3. Beş Ekranın Bileşen Mimarisi

### Ekran 1: Oturumlar ve Sağlayıcılar
1. **Hesap Listesi Paneli (Sol):**
   - Hesap başlık satırı ve `[+ Hesap Ekle]` boncuk düğmesi.
   - Her hesap satırı:
     - 4 Durumlu Gösterge Noktası:
       - `●` Yeşil: Hazır (`Ready`)
       - `◐` Hardal: Kota Bekliyor (`KotaBekliyor`, kalan süre örn. "41 dk ⏳")
       - `○` Soluk Gri: Giriş Gerekli (`GirisGerekli`, tek tıkla `[Giriş]` butonu)
       - `⚠` Mercan: İnsan Kapısı (`InsanKapisi`, `[Pencereyi Göster]` butonu)
     - Hesap adı, bağlı sağlayıcı rozetleri (`gemini`, `kimi`, `copilot`), hızlı eylem düğmeleri (`↻` Yenile, `👁` Aç).
2. **Sağ Ayrıntı Paneli:**
   - Seçili hesap kota penceresi (`Pacing.hourly_cap`'ten okunan tavan; boşsa "henüz ölçülmedi").
   - Son 5 Hata (ham neden zinciri, anahtar/cookie sırlarından arındırılmış).
   - Bağlı Eklenti Sekmeleri (`/eklenti/sekmeler`).
   - **Tanı Kartı (`TaniKarti`):** Sınıf adı rozeti (`secici_kirildi`, `cloudflare` vb.), kanıt satırları, ekran görüntüsü önizleme kutucuğu, eylem butonu (`[Giriş]`, `[Pencereyi göster]`, `[Seçici Önerisini Kopyala]`).
   - **Yetenek Kartı (`YetenekKarti`):** Model seçici, dosya yükleme, görsel üretimi var/yok listesi ve seçici kanıtları.
3. **[+ Hesap Ekle] Diyaloğu (Modal / Popover):**
   - Hesap adı / e-posta kutusu.
   - **Aranabilir Sağlayıcı Seçici:** Yazdıkça süzülen 358 sağlayıcı arama kutusu (`katla` ile Türkçe uyumlu, örn. "cop" → Copilot).
   - Enter'sız otomatik oturum başlatma butonu.

### Ekran 2: Yönlendirme (Routing)
1. **Strateji Seçici Kartı:**
   - 20 kamuya açık + 1 dahili (`quota-share`) yönlendirme stratejisi seçimi (`least-used`, `random`, `round-robin`, `cost-optimized` vb.).
2. **Kervan Komboları & Yedek Zincirleri:**
   - Model eşleşmeleri ve yedek rotalar.
   - Kervan'da henüz olmayan kombo kalıcılığı için dürüst `[Kervan'da henüz yok — DALGA-2]` rozeti.
3. **Dayanıklılık (`kervan-direnc`) Kartı:**
   - Devre kesici durumu: `KAPALI` (Yeşil), `YARI-AÇIK` (Hardal), `AÇIK` (Mercan).
   - 429 soğuma sayaçları.
4. **Çevirici & Oyun Alanı (Playground):**
   - `/v1/chat/completions` uçtan uca hızlı test alanı.
   - İstem kutusu, model seçimi, canlı yanıt alanı.

### Ekran 3: İzleme (Monitoring & Telemetri)
1. **Canlı Telemetri Kartları:**
   - Toplam İstek Sayacı, Başarı Oranı (%99.4), Ortalama Yanıt Süresi (ms), Aktif Yuvalar.
2. **Hata Kayıt Defteri:**
   - Sır temizlenmiş son 50 hata kaydı, filtreleme.
3. **Maliyet & Kullanım Grafikleri:**
   - Token tüketim dökümü.
   - Kervan'da kalıcı DB katmanı gelene kadar dürüst `[Kervan'da henüz yok — DALGA-2 / DALGA-3]` rozeti.

### Ekran 4: Bağlam ve Araçlar (Context & Tools)
1. **Eklenti Sekmeleri Paneli:**
   - Aktif bağlı tarayıcı sekmeleri, durumları, ping gecikmeleri.
2. **Bağlam Sıkıştırma (Context Compression):**
   - Caveman, LLMLingua vb. kipler için `[Kervan'da henüz yok — DALGA-4]` rozeti.
3. **MCP Sunucu Durumu:**
   - Kervan yerel MCP sunucusu bağlı istemciler listesi.

### Ekran 5: Ayarlar (Settings)
1. **Görünüm & Tema:**
   - `kilim-tema` tema seçici: Açık / Koyu / Sistem.
   - Varyant seçici: Cam Göbeği - Altın / Kırmızı - Yeşil.
2. **Kimlikler ve REST Sağlayıcılar:**
   - `[rest.*]` sağlayıcıları tablosu.
   - **Güvenlik Kuralı:** Anahtar değerleri ASLA gösterilmez; yalnız ortam değişkeni adı (örn. `OPENAI_API_KEY`) rozet olarak listelenir.
3. **Sistem ve Çalışma Zamanı:**
   - Kervan sürümü: `v0.1.0`.
   - wgpu Arka Ucu: `Vulkan` (veya `DX12`).
   - Uptime, bellek ayak izi.
