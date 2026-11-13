import fs from "fs";
import path from "path";

// PRD FR-8.3: Dalga Planı Oluşturucu (araclar/dalga-uret.ts)
// Dalga taslakları GİRDİLERDEN türetilir; sayı, kapsam ve gerekçe bu betikte sabit değildir:
//   - port/dalgalar/dalga-tanimi.json   → yalnız kimlik sözleşmesi (id, başlık, tema, eşleme kodu, FR adı)
//   - port/envanter.json                → anlık görüntü sayımları
//   - crates/pano-ekran/esleme.toml     → DALGA-2/3/4 olarak işaretlenmiş pano bölümleri
//   - port/defter/{dosyalar,davranislar}.jsonl → açık pencereler, kapsam dışı ve insan kararı sayıları
//   - port/durum.json                   → faz ve kapı durumu (varsa)
// Girdi eksikse betik UYDURMAZ; hata verip durur.

export interface DalgaTaslagi {
  id: string;
  baslik: string;
  kapsam: string;
  kapanmayan_satir_sayisi: number;
  en_riskli_varsayim: string;
  onkosullar: string[];
  markdown: string;
  /** Taslağın dayandığı girdi dosyaları ve defter kayıt kimlikleri (türetim izi). */
  kaynak_izleri: string[];
}

const DALGALAR_DIZINI = path.join("port", "dalgalar");
const TANIM_YOLU = path.join(DALGALAR_DIZINI, "dalga-tanimi.json");
const ENVANTER_YOLU = path.join("port", "envanter.json");
const ESLEME_YOLU = path.join("crates", "pano-ekran", "esleme.toml");
const DURUM_YOLU = path.join("port", "durum.json");
const DEFTER_DOSYALAR = path.join("port", "defter", "dosyalar.jsonl");
const DEFTER_DAVRANISLAR = path.join("port", "defter", "davranislar.jsonl");

interface DalgaKimligi {
  id: string;
  baslik: string;
  tema: string;
  esleme_kodu: string;
  envanter_alanlari: string[];
  fr: { id: string; ad: string };
}

interface Envanter {
  kaynak_sha: string;
  tarih: string;
  route_sayisi: number;
  pano_sayfasi_sayisi: number;
  toplam_sayfa_sayisi: number;
  db_modul_sayisi: number;
  goc_dosyasi_sayisi: number;
  test_dosyasi_sayisi: number;
}

interface DefterSatiri {
  id: string;
  durum: string;
  kaynak?: string;
  satir?: [number, number];
  hedef?: string;
}

const KAPANMAMIS_DURUMLAR = new Set(["planlandi", "okundu", "aktarildi"]);

function dosyaYok(yol: string): never {
  throw new Error(`[dalga-uret] girdi bulunamadı: ${yol} (türetim uydurma ile doldurulmaz)`);
}

function jsonOku<T>(yol: string): T {
  if (!fs.existsSync(yol)) dosyaYok(yol);
  return JSON.parse(fs.readFileSync(yol, "utf-8")) as T;
}

function jsonlOku(yol: string): DefterSatiri[] {
  if (!fs.existsSync(yol)) dosyaYok(yol);
  const satirlar: DefterSatiri[] = [];
  for (const ham of fs.readFileSync(yol, "utf-8").split("\n")) {
    const s = ham.trim();
    if (s.length === 0) continue;
    satirlar.push(JSON.parse(s) as DefterSatiri);
  }
  if (satirlar.length === 0) {
    throw new Error(`[dalga-uret] ${yol} boş; türetim yapılamaz`);
  }
  return satirlar;
}

/** esleme.toml içindeki `[[bolum]]` bloklarını `kaynak` koduna göre sayar. */
export function eslemeSayimlariniAl(icerik: string): Map<string, number> {
  const sayimlar = new Map<string, number>();
  const bloklar = icerik.split("[[bolum]]").slice(1);
  for (const blok of bloklar) {
    const eslesme = blok.match(/^\s*kaynak\s*=\s*"([^"]+)"/m);
    if (!eslesme) continue;
    const kod = eslesme[1];
    sayimlar.set(kod, (sayimlar.get(kod) ?? 0) + 1);
  }
  return sayimlar;
}

function bicimle(sayi: number): string {
  return sayi.toLocaleString("tr-TR");
}

function pencereGenisligi(s: DefterSatiri): number {
  if (!Array.isArray(s.satir) || s.satir.length !== 2) return 0;
  const [bas, son] = s.satir;
  return son >= bas ? son - bas + 1 : 0;
}

export function dalgaPlanlariniUret(kokDizin: string = "."): DalgaTaslagi[] {
  const kok = (...p: string[]) => path.join(kokDizin, ...p);

  const tanim = jsonOku<{ dalgalar: DalgaKimligi[] }>(kok(TANIM_YOLU));
  const envanter = jsonOku<Envanter>(kok(ENVANTER_YOLU));
  if (!fs.existsSync(kok(ESLEME_YOLU))) dosyaYok(kok(ESLEME_YOLU));
  const eslemeSayimlari = eslemeSayimlariniAl(fs.readFileSync(kok(ESLEME_YOLU), "utf-8"));

  const dosyalar = jsonlOku(kok(DEFTER_DOSYALAR));
  const davranislar = jsonlOku(kok(DEFTER_DAVRANISLAR));
  const tumKayitlar: DefterSatiri[] = [...dosyalar, ...davranislar];

  const durum = fs.existsSync(kok(DURUM_YOLU))
    ? jsonOku<{ faz?: string; tum_kapilar_yesil_mi?: boolean }>(kok(DURUM_YOLU))
    : undefined;

  const acikPencereler = dosyalar.filter((s) => KAPANMAMIS_DURUMLAR.has(s.durum));
  const acikSatirToplami = acikPencereler.reduce((t, s) => t + pencereGenisligi(s), 0);
  const kapsamDisi = tumKayitlar.filter((s) => s.durum === "kapsam-disi").length;
  const insanKarari = tumKayitlar.filter((s) => s.durum === "insan-karari").length;
  const dogrulanan = tumKayitlar.filter((s) => s.durum === "dogrulandi").length;

  const envanterDegerleri: Record<string, number> = {
    route_sayisi: envanter.route_sayisi,
    pano_sayfasi_sayisi: envanter.pano_sayfasi_sayisi,
    toplam_sayfa_sayisi: envanter.toplam_sayfa_sayisi,
    db_modul_sayisi: envanter.db_modul_sayisi,
    goc_dosyasi_sayisi: envanter.goc_dosyasi_sayisi,
    test_dosyasi_sayisi: envanter.test_dosyasi_sayisi,
  };

  const dalgalar: DalgaTaslagi[] = tanim.dalgalar.map((kimlik, sira) => {
    for (const alan of kimlik.envanter_alanlari) {
      if (!(alan in envanterDegerleri)) {
        throw new Error(`[dalga-uret] ${kimlik.id}: envanterde '${alan}' alanı yok`);
      }
    }

    const sayilar = kimlik.envanter_alanlari.map((a) => `${bicimle(envanterDegerleri[a])} ${a}`);
    const eslemeAdedi = eslemeSayimlari.get(kimlik.esleme_kodu) ?? 0;
    if (eslemeAdedi === 0) {
      throw new Error(
        `[dalga-uret] ${kimlik.id}: esleme.toml içinde kaynak="${kimlik.esleme_kodu}" bölümü yok`,
      );
    }

    const kapsam =
      `OmniRoute ${envanter.kaynak_sha} anlık görüntüsünden: ${sayilar.join(", ")}; ` +
      `pano eşlemesinde ${eslemeAdedi} bölüm ${kimlik.esleme_kodu} (${kimlik.id}) olarak işaretli; ` +
      `defterde (tüm dalgalar ortak) açık pencere ${acikPencereler.length} kayıtta ${bicimle(
        acikSatirToplami,
      )} satır.`;

    const enRiskliVarsayim =
      `Defterdeki ${tumKayitlar.length} kaydın ${kapsamDisi} tanesi kapsam dışı, ` +
      `${insanKarari} tanesi insan kararına bırakılmış ve ${dogrulanan} tanesi doğrulanmış; ` +
      `bu dışlamaların ${kimlik.id} kapsamını daraltmadığı varsayılır.`;

    const onkosullar: string[] = [];
    if (sira > 0) {
      onkosullar.push(`${tanim.dalgalar[sira - 1].id} tamamlanması`);
    }
    if (durum) {
      onkosullar.push(
        `port/durum.json: faz=${durum.faz ?? "bilinmiyor"}, tum_kapilar_yesil_mi=${String(durum.tum_kapilar_yesil_mi)}`,
      );
    }
    onkosullar.push(
      acikPencereler.length === 0
        ? "defterde açık pencere kalmaması"
        : `defterde açık kalan ${acikPencereler.length} pencerenin (${acikPencereler
            .map((p) => p.id)
            .join(", ")}) kapatılması`,
    );

    const acikListesi =
      acikPencereler.length === 0
        ? "- Açık pencere yok.\n"
        : acikPencereler
            .map(
              (p) =>
                `- ${p.id} · ${p.kaynak ?? "kaynak belirtilmemiş"} [${p.satir?.[0] ?? 0},${
                  p.satir?.[1] ?? 0
                }] → ${p.hedef ?? "hedef yok"} (${p.durum})`,
            )
            .join("\n") + "\n";

    const markdown = `# PRD: ${kimlik.baslik}

## 1. Problem Statement & Scope
${kapsam}
Bu taslak, defterdeki açık pencereler kapatılmadan başlamaz; kapsam ve sayılar her koşuda
port/envanter.json, crates/pano-ekran/esleme.toml ve port/defter/*.jsonl dosyalarından yeniden türetilir.

## 2. Goals and Non-Goals
### 2.1 Goals
- G1: ${kimlik.envanter_alanlari.map((a) => bicimle(envanterDegerleri[a]) + " " + a).join(" ve ")} kaleminin Kervan karşılığına bağlanması
- G2: ${eslemeAdedi} pano bölümünün ${kimlik.esleme_kodu} rozetinden gerçek kaynağa geçirilmesi
- G3: Açık defter pencerelerinin (${acikPencereler.length} kayıt) bu dalga içinde kapatılması

### 2.2 Non-Goals
- Kapsam dışı bırakılmış ${kapsamDisi} defter kaydının bu dalgaya dahil edilmesi
- İnsan kararı bekleyen ${insanKarari} defter kaydının bu dalgada karara bağlanması

## 3. Users and Personas
- **Ercan**: Dalga kapsamının ve varsayımların onayı
- **Builder**: Aktarımı yapan ajan
- **Hakem**: Kapanan pencerelerin kanıtını denetleyen bağımsız değerlendirici

## 4. Functional Requirements
### ${kimlik.fr.id}: ${kimlik.fr.ad}
${kapsam}
*Acceptance:* Envanterdeki ${kimlik.envanter_alanlari.map((a) => bicimle(envanterDegerleri[a]) + " " + a).join(" ve ")} kalemi için karşılaştırma testi yazılır; test, düzeltme geri alındığında kırmızıya döner.

## 5. Türetim İzi
Girdiler: port/dalgalar/dalga-tanimi.json (kimlik) · port/envanter.json (kaynak_sha ${envanter.kaynak_sha}, tarih ${envanter.tarih}) · crates/pano-ekran/esleme.toml (${kimlik.esleme_kodu}: ${eslemeAdedi} bölüm) · port/defter/dosyalar.jsonl (${dosyalar.length} kayıt) · port/defter/davranislar.jsonl (${davranislar.length} kayıt)${durum ? ` · port/durum.json (faz ${durum.faz ?? "bilinmiyor"})` : ""}.

Açık defter pencereleri:
${acikListesi}`;

    const kaynakIzleri = [
      TANIM_YOLU,
      ENVANTER_YOLU,
      ESLEME_YOLU,
      DEFTER_DOSYALAR,
      DEFTER_DAVRANISLAR,
      ...(durum ? [DURUM_YOLU] : []),
      ...acikPencereler.map((p) => `defter:${p.id}`),
    ];

    return {
      id: kimlik.id,
      baslik: kimlik.baslik,
      kapsam,
      kapanmayan_satir_sayisi: acikSatirToplami,
      en_riskli_varsayim: enRiskliVarsayim,
      onkosullar,
      markdown,
      kaynak_izleri: kaynakIzleri,
    };
  });

  const dizin = kok(DALGALAR_DIZINI);
  if (!fs.existsSync(dizin)) {
    fs.mkdirSync(dizin, { recursive: true });
  }

  for (const d of dalgalar) {
    fs.writeFileSync(path.join(dizin, `${d.id}.md`), d.markdown, "utf-8");
  }

  console.log(
    `[dalga-uret] ${dalgalar.length} adet dalga PRD taslağı defter ve envanter girdilerinden türetildi; açık pencere ${acikPencereler.length} kayıt / ${acikSatirToplami} satır.`,
  );
  return dalgalar;
}

if (import.meta.main) {
  dalgaPlanlariniUret();
}
