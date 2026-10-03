import fs from "fs";
import path from "path";

// PRD FR-8.3: Dalga Planı Oluşturucu (araclar/dalga-uret.ts)
// Defter ve durum verilerinden öncelik sırasıyla sonraki dalgaların
// taslak PRD iskeletlerini üretir.
// Kabul Kriteri: En az 3 dalga taslağı üretilir; her taslak §2 başlıklarını
// ve en az bir `Acceptance:` içeren FR iskeleti taşır.

export interface DalgaTaslagi {
  id: string;
  baslik: string;
  kapsam: string;
  kapanmayan_satir_sayisi: number;
  en_riskli_varsayim: string;
  onkosullar: string[];
  markdown: string;
}

const DALGALAR_DIZINI = path.join("port", "dalgalar");

export function dalgaPlanlariniUret(): DalgaTaslagi[] {
  if (!fs.existsSync(DALGALAR_DIZINI)) {
    fs.mkdirSync(DALGALAR_DIZINI, { recursive: true });
  }

  const dalgalar: DalgaTaslagi[] = [
    {
      id: "DALGA-2",
      baslik: "Dalga 2: Veritabanı Kalıcılığı ve SQLite/Drizzle Modelleri",
      kapsam: "OmniRoute'un 136 veritabanı modülü ve 193 göçünün Kervan SQLite kalıcılık katmanına uyarlanması",
      kapanmayan_satir_sayisi: 45000,
      en_riskli_varsayim: "Rust SQLite sürücüsü, TypeScript Drizzle şemalarıyla tam birebir veri uyumluluğu sağlar.",
      onkosullar: ["P10 PR'ının onaylanması", "kervan-havuz modülünün kalıcı oturum tablosuna bağlanması"],
      markdown: `# PRD: Dalga 2 — Veritabanı Kalıcılığı ve SQLite/Drizzle Modelleri

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
`,
    },
    {
      id: "DALGA-3",
      baslik: "Dalga 3: Pano ve İstatistik Arayüzü (egui Dashboard)",
      kapsam: "OmniRoute'un 122 pano sayfasındaki temel grafik, kota ve sağlık telemetrisinin egui masaüstü arayüzüne taşınması",
      kapanmayan_satir_sayisi: 65000,
      en_riskli_varsayim: "egui wgpu render motoru yüksek veri akışında 60 FPS akıcılığı korur.",
      onkosullar: ["Dalga 2 veritabanı telemetri tablolarının hazır olması"],
      markdown: `# PRD: Dalga 3 — Pano ve İstatistik Arayüzü (egui Dashboard)

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
`,
    },
    {
      id: "DALGA-4",
      baslik: "Dalga 4: Gelişmiş Sağlayıcı Eklentileri ve Model Kataloğu",
      kapsam: "358 sağlayıcı adaptörünün ve dinamik model keşif (catalog sync) motorunun Rust'a taşınması",
      kapanmayan_satir_sayisi: 90000,
      en_riskli_varsayim: "Dış sağlayıcı API şemalarındaki değişiklikler dinamik adaptör katmanıyla yönetilebilir.",
      onkosullar: ["Dalga 2 ve Dalga 3'ün tamamlanması"],
      markdown: `# PRD: Dalga 4 — Gelişmiş Sağlayıcı Eklentileri ve Model Kataloğu

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
Sağlayıcıların güncel model listeleri periyodik olarak sorgulanıp \`/v1/models\` listesine eklenir.
*Acceptance:* Yeni eklenen bir sağlayıcı için 60 saniye içinde model listesi güncellenir ve test doğrulanır.
`,
    },
  ];

  for (const d of dalgalar) {
    const dosyaYolu = path.join(DALGALAR_DIZINI, `${d.id}.md`);
    fs.writeFileSync(dosyaYolu, d.markdown, "utf-8");
  }

  console.log(`✅ [dalga-uret] ${dalgalar.length} adet dalga PRD taslağı 'port/dalgalar/' dizininde üretildi.`);
  return dalgalar;
}

if (import.meta.main) {
  dalgaPlanlariniUret();
}
