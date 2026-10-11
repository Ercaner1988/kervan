import { spawnSync } from "child_process";
import fs from "fs";
import os from "os";
import path from "path";

// PRD FR-1.3: Kabul haritası denetleyicisi
// Her kabul maddesini >=1 gerçek test kimliğine bağlar. "Gerçek" = bu koşuda
// (a) `cargo test -- --list` ile derlenmiş listede `ad: test` satırı olan, ya da
// (b) `bun test` koşusunda GEÇEN (JUnit raporunda hatasız) test.
// Elle yazılmış liste dosyası (port/testler.txt) kapı yolunda kabul edilmez.
// Fail-closed: cargo/bun koşamazsa, çıktı ayrıştırılamazsa, liste boşsa kırmızı.

interface KabulMaddesi {
  id: string;
  metin?: string;
  clause?: string;
  kanit?: unknown;
}

interface KabulDosyasi {
  faz: string;
  maddeler: KabulMaddesi[];
}

const DAKIKA = 60 * 1000;

/** Cargo'yu koşan her kapı bu ortamı kullanır (makine tek ağır süreç kaldırıyor). */
export function cargoOrtami(): NodeJS.ProcessEnv {
  return { ...process.env, CARGO_BUILD_JOBS: "2" };
}

/** `cargo test -- --list` çıktısından yalnız `<ad>: test` satırlarını alır. */
export function cargoListesiniAyristir(metin: string): Set<string> {
  const kimlikler = new Set<string>();
  for (const satir of metin.split(/\r?\n/)) {
    const m = satir.match(/^(\S+): test$/);
    if (m) kimlikler.add(m[1]);
  }
  return kimlikler;
}

function sonSatirlar(metin: string | null | undefined, n = 8): string {
  return (metin ?? "").trim().split(/\r?\n/).slice(-n).join(" | ");
}

export function cargoTestListesiniAl(kokDizin = process.cwd()): { kimlikler: Set<string>; hata?: string } {
  const r = spawnSync("cargo", ["test", "--workspace", "--all-targets", "--all-features", "--", "--list"], {
    cwd: kokDizin,
    encoding: "utf-8",
    maxBuffer: 64 * 1024 * 1024,
    env: cargoOrtami(),
    timeout: 60 * DAKIKA,
  });
  if (r.error || r.status !== 0) {
    return {
      kimlikler: new Set(),
      hata: `cargo test -- --list başarısız (çıkış ${r.status ?? r.error?.message}): ${sonSatirlar(r.stderr)}`,
    };
  }
  const kimlikler = cargoListesiniAyristir(r.stdout ?? "");
  if (kimlikler.size === 0) return { kimlikler, hata: "cargo test -- --list hiç test listelemedi" };
  return { kimlikler };
}

function xmlCoz(s: string): string {
  return s
    .replace(/&#(\d+);/g, (_, n) => String.fromCodePoint(Number(n)))
    .replace(/&quot;/g, '"')
    .replace(/&apos;/g, "'")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&amp;/g, "&");
}

/** JUnit raporundaki testcase'leri geçen/düşen diye ayırır. Ad hem `name` hem `classname > name` olarak eklenir. */
export function junitAyristir(xml: string): { gecenler: Set<string>; dusenler: Set<string> } {
  const gecenler = new Set<string>();
  const dusenler = new Set<string>();
  const re = /<testcase\b([^>]*?)(\/>|>([\s\S]*?)<\/testcase>)/g;
  let m;
  while ((m = re.exec(xml)) !== null) {
    const nit = m[1];
    const ad = nit.match(/\bname="([^"]*)"/);
    if (!ad) continue;
    const sinif = nit.match(/\bclassname="([^"]*)"/);
    const adlar = [xmlCoz(ad[1])];
    if (sinif && sinif[1]) adlar.push(`${xmlCoz(sinif[1])} > ${xmlCoz(ad[1])}`);
    const gecti = m[2] === "/>" || !/<(failure|error|skipped)\b/.test(m[3] ?? "");
    for (const a of adlar) (gecti ? gecenler : dusenler).add(a);
  }
  return { gecenler, dusenler };
}

export function bunTestleriniKos(
  kokDizin = process.cwd(),
  hedef = path.join("araclar", "kapilar")
): { gecenler: Set<string>; dusenler: Set<string>; hata?: string } {
  const gecici = fs.mkdtempSync(path.join(os.tmpdir(), "kervan-kabul-junit-"));
  const xmlYolu = path.join(gecici, "junit.xml");
  try {
    const r = spawnSync("bun", ["test", hedef, "--reporter=junit", `--reporter-outfile=${xmlYolu}`], {
      cwd: kokDizin,
      encoding: "utf-8",
      maxBuffer: 64 * 1024 * 1024,
      timeout: 30 * DAKIKA,
    });
    if (r.error) return { gecenler: new Set(), dusenler: new Set(), hata: `bun test koşamadı: ${r.error.message}` };
    if (!fs.existsSync(xmlYolu)) {
      return { gecenler: new Set(), dusenler: new Set(), hata: `bun test JUnit raporu üretmedi (çıkış ${r.status}): ${sonSatirlar(r.stderr)}` };
    }
    const sonuc = junitAyristir(fs.readFileSync(xmlYolu, "utf-8"));
    if (sonuc.gecenler.size + sonuc.dusenler.size === 0) {
      return { ...sonuc, hata: "bun test JUnit raporunda hiç test yok" };
    }
    return sonuc;
  } finally {
    fs.rmSync(gecici, { recursive: true, force: true });
  }
}

export interface GercekTestler {
  gecenler: Set<string>;
  dusenler: Set<string>;
  hatalar: string[];
}

/**
 * Gerçek test kimlikleri. `testListDosyasi` yalnız birim testlerde enjeksiyon içindir
 * (yalnız `ad: test` satırları); kapı CLI'ı bu yolu kabul etmez.
 */
export function gercekTestKimlikleriniTopla(testListDosyasi?: string, kokDizin = process.cwd()): GercekTestler {
  if (testListDosyasi !== undefined) {
    if (!fs.existsSync(testListDosyasi)) {
      return { gecenler: new Set(), dusenler: new Set(), hatalar: [`Test listesi dosyası yok: ${testListDosyasi}`] };
    }
    const gecenler = cargoListesiniAyristir(fs.readFileSync(testListDosyasi, "utf-8"));
    return { gecenler, dusenler: new Set(), hatalar: gecenler.size ? [] : [`Test listesi boş: ${testListDosyasi}`] };
  }

  const hatalar: string[] = [];
  const cargo = cargoTestListesiniAl(kokDizin);
  if (cargo.hata) hatalar.push(cargo.hata);
  const bun = bunTestleriniKos(kokDizin);
  if (bun.hata) hatalar.push(bun.hata);
  const gecenler = new Set<string>([...cargo.kimlikler, ...bun.gecenler]);
  return { gecenler, dusenler: bun.dusenler, hatalar };
}

export function kabulHaritasiniDogrula(
  kabulDosyaYolu: string,
  testListDosyasi?: string,
  kokDizin = process.cwd()
): {
  gecerli: boolean;
  faz: string;
  maddeSayisi: number;
  hatalar: string[];
} {
  const kirmizi = (faz: string, hata: string) => ({ gecerli: false, faz, maddeSayisi: 0, hatalar: [hata] });

  if (!fs.existsSync(kabulDosyaYolu)) return kirmizi("bilinmeyen", `Kabul dosyası bulunamadı: ${kabulDosyaYolu}`);

  let veri: KabulDosyasi;
  try {
    veri = JSON.parse(fs.readFileSync(kabulDosyaYolu, "utf-8"));
  } catch (e: any) {
    return kirmizi("bilinmeyen", `Kabul dosyası JSON olarak ayrıştırılamadı: ${e.message}`);
  }
  const faz = typeof veri?.faz === "string" && veri.faz ? veri.faz : "bilinmeyen";
  if (!veri || !Array.isArray(veri.maddeler) || veri.maddeler.length === 0) {
    return kirmizi(faz, "Kabul maddeleri listesi boş veya eksik");
  }

  const hatalar: string[] = [];
  const testler = gercekTestKimlikleriniTopla(testListDosyasi, kokDizin);
  hatalar.push(...testler.hatalar);

  for (const madde of veri.maddeler) {
    if (!madde || typeof madde.id !== "string" || !madde.id) {
      hatalar.push("Madde ID'si eksik");
      continue;
    }
    const kanit = madde.kanit;
    if (!Array.isArray(kanit) || kanit.length === 0) {
      hatalar.push(`Madde ${madde.id} için test kimliği (kanit) belirtilmemiş`);
      continue;
    }
    for (const testId of kanit) {
      if (typeof testId !== "string" || !testId.trim()) {
        hatalar.push(`Madde ${madde.id}: boş ya da dizge olmayan kanıt kimliği`);
      } else if (testler.dusenler.has(testId)) {
        hatalar.push(`Madde ${madde.id}: '${testId}' testi bu koşuda KIRMIZI (düşen test kanıt sayılmaz)`);
      } else if (!testler.gecenler.has(testId)) {
        hatalar.push(`Madde ${madde.id}: '${testId}' kimlikli test gerçek testler arasında bulunamadı`);
      }
    }
  }

  return { gecerli: hatalar.length === 0, faz, maddeSayisi: veri.maddeler.length, hatalar };
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  const bitir = (hatalar: string[], faz = "?") => {
    console.error(`❌ [kabul-haritasi] Faz ${faz} kabul haritası doğrulanamadı:`);
    for (const h of hatalar) console.error(`  - ${h}`);
    console.log(`HUKUM: kirmizi ${hatalar[0]}`);
    process.exit(1);
  };

  if (args.includes("--test-list-dosyasi")) {
    bitir(["--test-list-dosyasi kaldırıldı: elle yazılabilir test listesi kanıt sayılmaz; liste her koşuda cargo/bun'dan üretilir"]);
  }
  let dosyaYolu = args.find((a) => !a.startsWith("--")) ?? "";
  if (!dosyaYolu) {
    console.error("Kullanım: bun araclar/kapilar/kabul-haritasi.ts <port/kabul/P1.json | P1>");
    bitir(["kabul dosyası verilmedi"]);
  }
  if (!dosyaYolu.endsWith(".json")) dosyaYolu = path.join("port", "kabul", `${dosyaYolu}.json`);

  const sonuc = kabulHaritasiniDogrula(dosyaYolu);
  if (!sonuc.gecerli) bitir(sonuc.hatalar, sonuc.faz);
  console.log(`✅ [kabul-haritasi] Faz ${sonuc.faz}: ${sonuc.maddeSayisi} kabul maddesi gerçek testlerle doğrulandı.`);
  console.log("HUKUM: yesil");
  process.exit(0);
}
