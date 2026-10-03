/**
 * Kervan Envanter ve Defter Üretim Aracı (Bun-first)
 *
 * OmniRoute upstream release/v3.8.52 (SHA dbe703a) kod tabanını
 * makine düzeyinde tarayarak port/envanter/*.json ve port/defter/*.jsonl üretir.
 */

import { readdir, readFile, stat } from "node:fs/promises";
import { join, relative, resolve } from "node:path";
import { Database } from "bun:sqlite";

const REPO_ROOT = resolve(import.meta.dir, "..").replace(/\\/g, "/");
const REF_DIR = join(REPO_ROOT, "ref", "omniroute-dbe703a").replace(/\\/g, "/");
const PORT_DIR = join(REPO_ROOT, "port").replace(/\\/g, "/");
const ENVANTER_DIR = join(PORT_DIR, "envanter").replace(/\\/g, "/");
const DEFTER_DIR = join(PORT_DIR, "defter").replace(/\\/g, "/");

interface DosyaKaydi {
  id: string;
  yol: string;
  satir: number;
  boyut: number;
  tur: "kaynak" | "test" | "belge" | "veri" | "arac" | "ucuncu_taraf";
  alan: string;
  durum: "planlandi" | "okundu" | "aktarildi" | "dogrulandi" | "kapsam-disi" | "insan-karari";
  hedef: string | null;
  okuma: {
    ajan: string | null;
    araliklar: [number, number][];
    kart: string | null;
  };
  kanit: string[];
  gerekce: string | null;
}

interface DavranisKaydi {
  id: string;
  tur:
    | "route"
    | "sayfa"
    | "saglayici"
    | "strateji"
    | "mcp-arac"
    | "a2a-beceri"
    | "migration"
    | "ayar"
    | "env"
    | "cli"
    | "bayrak"
    | "olay"
    | "sert-kural"
    | "test-grubu";
  ad: string;
  kaynak: string[];
  hedef: string | null;
  test: string[];
  durum: "planlandi" | "okundu" | "aktarildi" | "dogrulandi" | "kapsam-disi" | "insan-karari";
  kanit: string[];
}

// ── Yardımcı Fonksiyonlar ─────────────────────────────────────────────

async function tumDosyalariTara(dizin: string): Promise<string[]> {
  const dosyalar: string[] = [];
  async function tara(mevcut: string) {
    const elemanlar = await readdir(mevcut, { withFileTypes: true });
    for (const el of elemanlar) {
      const tamYol = join(mevcut, el.name).replace(/\\/g, "/");
      if (el.isDirectory()) {
        if (el.name === ".git" || el.name === "node_modules" || el.name === ".build") continue;
        await tara(tamYol);
      } else if (el.isFile()) {
        dosyalar.push(tamYol);
      }
    }
  }
  await tara(dizin);
  return dosyalar;
}

function turBelirle(relPath: string): "kaynak" | "test" | "belge" | "veri" | "arac" | "ucuncu_taraf" {
  const p = relPath.toLowerCase().replace(/\\/g, "/");
  if (p.includes("tests/") || p.includes("__tests__/") || p.endsWith(".test.ts") || p.endsWith(".test.tsx") || p.endsWith(".spec.ts")) {
    return "test";
  }
  if (p.startsWith("docs/") || p.endsWith(".md") || p.endsWith(".txt")) {
    return "belge";
  }
  if (p.startsWith("scripts/") || p.startsWith(".github/") || p.includes("config/") || p.endsWith(".config.mjs") || p.endsWith(".config.ts") || p.endsWith("dockerfile") || p.endsWith(".yml") || p.endsWith(".yaml")) {
    return "arac";
  }
  if (p.includes("vendor/") || p.startsWith("@omniroute/")) {
    return "ucuncu_taraf";
  }
  if (p.endsWith(".sql") || (p.endsWith(".json") && !p.startsWith("src/"))) {
    return "veri";
  }
  return "kaynak";
}

function alanBelirle(relPath: string): string {
  const p = relPath.replace(/\\/g, "/");
  if (p.startsWith("open-sse/services/")) return "services";
  if (p.startsWith("open-sse/executors/")) return "executors";
  if (p.startsWith("open-sse/translator/")) return "translator";
  if (p.startsWith("open-sse/handlers/")) return "handlers";
  if (p.startsWith("open-sse/mcp-server/")) return "mcp";
  if (p.startsWith("open-sse/")) return "open-sse";
  if (p.startsWith("src/app/api/")) return "api";
  if (p.startsWith("src/app/")) return "dashboard";
  if (p.startsWith("src/lib/db/")) return "db";
  if (p.startsWith("src/lib/a2a/")) return "a2a";
  if (p.startsWith("src/lib/guardrails/")) return "guardrails";
  if (p.startsWith("src/lib/oauth/")) return "oauth";
  if (p.startsWith("src/lib/")) return "lib";
  if (p.startsWith("src/shared/")) return "shared";
  if (p.startsWith("bin/")) return "cli";
  if (p.startsWith("tests/")) return "test";
  if (p.startsWith("docs/")) return "docs";
  return "diger";
}

// ── 1. Dosya Dökümü ──────────────────────────────────────────────────

async function dosyalarEnvanteriUret(dosyaYollari: string[]): Promise<DosyaKaydi[]> {
  const kayitlar: DosyaKaydi[] = [];
  let sayac = 1;

  for (const absPath of dosyaYollari) {
    const rel = relative(REF_DIR, absPath).replace(/\\/g, "/");
    const st = await stat(absPath);
    let satir = 0;
    try {
      const icerik = await readFile(absPath, "utf-8");
      satir = icerik.split(/\r?\n/).length;
    } catch {
      // ikili dosya olabilir
      satir = 0;
    }

    const tur = turBelirle(rel);
    const alan = alanBelirle(rel);

    // Otomatik kapsam-dışı belirleme: Electron, Next.js build tesisatı, vendor kopyaları
    let durum: DosyaKaydi["durum"] = "planlandi";
    let gerekce: string | null = null;
    let hedef: string | null = null;

    if (rel.startsWith("electron/")) {
      durum = "kapsam-disi";
      gerekce = "Electron yerine saf egui masaüstü penceresi ve sistem tepsisi kullanılmaktadır (§4, §5-8)";
    } else if (rel.startsWith("scripts/release/") || rel.startsWith("scripts/build/") || rel.includes(".mergify.yml")) {
      durum = "kapsam-disi";
      gerekce = "OmniRoute'un kendi monorepo CI/CD ve paketleme betikleri Kervan çalışma mantığı dışındadır (§4)";
    } else if (rel.startsWith("open-sse/vendor/")) {
      durum = "kapsam-disi";
      gerekce = "Üçüncü taraf tersine mühendislik kütüphaneleri Kervan'ın saf Rust mimarisine doğrudan dahil edilmez (§5-14)";
    }

    kayitlar.push({
      id: `F${sayac.toString().padStart(6, "0")}`,
      yol: rel,
      satir,
      boyut: st.size,
      tur,
      alan,
      durum,
      hedef,
      okuma: {
        ajan: null,
        araliklar: [],
        kart: null,
      },
      kanit: [],
      gerekce,
    });
    sayac++;
  }

  return kayitlar;
}

// ── 2. Route Tablosu ─────────────────────────────────────────────────

async function routeTablosuUret(dosyaYollari: string[]): Promise<any[]> {
  const routes: any[] = [];
  const routeFiles = dosyaYollari.filter((p) => p.endsWith("route.ts") && p.includes("src/app"));

  for (const absPath of routeFiles) {
    const rel = relative(REF_DIR, absPath).replace(/\\/g, "/");
    // e.g. src/app/api/v1/chat/completions/route.ts -> /api/v1/chat/completions
    const match = rel.match(/src\/app\/(.+)\/route\.ts/);
    if (!match) continue;
    const urlPath = "/" + match[1];

    const icerik = await readFile(absPath, "utf-8");
    const methods: string[] = [];
    for (const m of ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"]) {
      if (icerik.includes(`export async function ${m}`) || icerik.includes(`export const ${m}`) || icerik.includes(`export function ${m}`)) {
        methods.push(m);
      }
    }

    routes.push({
      dosya: rel,
      url: urlPath,
      metotlar: methods,
    });
  }

  return routes;
}

// ── 3. Sayfa Tablosu ─────────────────────────────────────────────────

async function sayfaTablosuUret(dosyaYollari: string[]): Promise<any[]> {
  const sayfalar: any[] = [];
  const pageFiles = dosyaYollari.filter((p) => p.endsWith("page.tsx") && p.includes("src/app"));

  for (const absPath of pageFiles) {
    const rel = relative(REF_DIR, absPath).replace(/\\/g, "/");
    // e.g. src/app/(dashboard)/settings/page.tsx -> /settings
    let url = rel
      .replace(/^src\/app\//, "")
      .replace(/\/page\.tsx$/, "")
      .replace(/\(.*?\)\/?/g, ""); // parantezli route gruplarını kaldır

    if (!url.startsWith("/")) url = "/" + url;
    if (url === "") url = "/";

    sayfalar.push({
      dosya: rel,
      url,
      kategori: rel.includes("(dashboard)") ? "dashboard" : "genel",
    });
  }

  return sayfalar;
}

// ── 4. Veritabanı ve Göçler ──────────────────────────────────────────

async function veritabaniEnvanteriUret(dosyaYollari: string[]): Promise<any> {
  const migrationFiles = dosyaYollari
    .filter((p) => p.includes("src/lib/db/migrations") && p.endsWith(".sql"))
    .sort();

  const db = new Database(":memory:");
  let basariliGoc = 0;
  let hataliGoc = 0;
  const hatalar: string[] = [];

  const ozellikSayimi = {
    fts5: 0,
    with_recursive: 0,
    instead_of: 0,
    without_rowid: 0,
    vacuum_into: 0,
    pragma: 0,
    json_funcs: 0,
    triggers: 0,
    views: 0,
  };

  for (const f of migrationFiles) {
    const rel = relative(REF_DIR, f).replace(/\\/g, "/");
    const sql = await readFile(f, "utf-8");
    const lower = sql.toLowerCase();

    if (lower.includes("fts5")) ozellikSayimi.fts5++;
    if (lower.includes("with recursive")) ozellikSayimi.with_recursive++;
    if (lower.includes("instead of")) ozellikSayimi.instead_of++;
    if (lower.includes("without rowid")) ozellikSayimi.without_rowid++;
    if (lower.includes("vacuum into")) ozellikSayimi.vacuum_into++;
    if (lower.includes("pragma ")) ozellikSayimi.pragma++;
    if (lower.includes("json_")) ozellikSayimi.json_funcs++;
    if (lower.includes("create trigger")) ozellikSayimi.triggers++;
    if (lower.includes("create view")) ozellikSayimi.views++;

    try {
      db.run(sql);
      basariliGoc++;
    } catch (e: any) {
      hataliGoc++;
      hatalar.push(`${rel}: ${e.message}`);
    }
  }

  const altinSema = db.query("SELECT type, name, sql FROM sqlite_master WHERE type IN ('table', 'view', 'trigger', 'index') ORDER BY name").all();
  db.close();

  return {
    toplamGoc: migrationFiles.length,
    basariliGoc,
    hataliGoc,
    hatalar,
    ozellikSayimi,
    tabloSayisi: altinSema.filter((x: any) => x.type === "table").length,
    indeksSayisi: altinSema.filter((x: any) => x.type === "index").length,
    gorunumSayisi: altinSema.filter((x: any) => x.type === "view").length,
    tetikleyiciSayisi: altinSema.filter((x: any) => x.type === "trigger").length,
    altinSema,
  };
}

// ── 5. Katalog ve Sağlayıcılar ───────────────────────────────────────

async function katalogUret(dosyaYollari: string[]): Promise<any> {
  const executorFiles = dosyaYollari
    .filter((p) => p.includes("open-sse/executors/") && (p.endsWith(".ts") || p.endsWith(".js")) && !p.includes(".test."))
    .map((p) => relative(REF_DIR, p).replace(/\\/g, "/"));

  // Ortam değişkenleri taraması
  const envSet = new Set<string>();
  for (const f of dosyaYollari) {
    if (!f.endsWith(".ts") && !f.endsWith(".tsx") && !f.endsWith(".mjs") && !f.endsWith(".js") && !f.endsWith(".example")) continue;
    try {
      const icerik = await readFile(f, "utf-8");
      const matches = icerik.matchAll(/process\.env\.([A-Z0-9_]+)/g);
      for (const m of matches) {
        if (m[1]) envSet.add(m[1]);
      }
    } catch {}
  }

  return {
    toplamExecutorDosyasi: executorFiles.length,
    executorListesi: executorFiles,
    ortamDegiskenleriSayisi: envSet.size,
    ortamDegiskenleri: Array.from(envSet).sort(),
  };
}

// ── 6. Test Envanteri ────────────────────────────────────────────────

async function testlerEnvanteriUret(dosyaYollari: string[]): Promise<any> {
  const testFiles = dosyaYollari.filter((p) => {
    const rel = relative(REF_DIR, p).replace(/\\/g, "/");
    return rel.includes("tests/") || rel.includes(".test.") || rel.includes(".spec.");
  });

  const testKayitlari: any[] = [];
  let toplamTestBloğu = 0;

  for (const f of testFiles) {
    const rel = relative(REF_DIR, f).replace(/\\/g, "/");
    try {
      const icerik = await readFile(f, "utf-8");
      const describeMatches = icerik.match(/describe\s*\(\s*["'`](.*?)["'`]/g) || [];
      const testMatches = icerik.match(/(it|test)\s*\(\s*["'`](.*?)["'`]/g) || [];
      const sayi = testMatches.length;
      toplamTestBloğu += sayi;

      // Basit statik import yakalama
      const importMatches = Array.from(icerik.matchAll(/from\s+["'](.*?)["']/g)).map((m) => m[1]);

      testKayitlari.push({
        dosya: rel,
        testSayisi: sayi,
        describeSayisi: describeMatches.length,
        importlar: importMatches.filter((i) => i.startsWith(".") || i.startsWith("@/")),
      });
    } catch {}
  }

  return {
    toplamTestDosyasi: testFiles.length,
    tahminiToplamBirimTest: toplamTestBloğu,
    testler: testKayitlari,
  };
}

// ── 7. Davranışlar Defteri Üretimi ───────────────────────────────────

function davranislarUret(routes: any[], sayfalar: any[], dbEnvanter: any, katalog: any): DavranisKaydi[] {
  const davranislar: DavranisKaydi[] = [];
  let sayac = 1;

  // 1. Rotalar
  for (const r of routes) {
    for (const m of r.metotlar) {
      davranislar.push({
        id: `B${sayac.toString().padStart(6, "0")}`,
        tur: "route",
        ad: `${m} ${r.url}`,
        kaynak: [`${r.dosya}`],
        hedef: null,
        test: [],
        durum: "planlandi",
        kanit: [],
      });
      sayac++;
    }
  }

  // 2. Sayfalar
  for (const s of sayfalar) {
    davranislar.push({
      id: `B${sayac.toString().padStart(6, "0")}`,
      tur: "sayfa",
      ad: `Sayfa: ${s.url} (${s.kategori})`,
      kaynak: [`${s.dosya}`],
      hedef: null,
      test: [],
      durum: "planlandi",
      kanit: [],
    });
    sayac++;
  }

  // 3. Veritabanı Tabloları
  for (const t of dbEnvanter.altinSema.filter((x: any) => x.type === "table")) {
    davranislar.push({
      id: `B${sayac.toString().padStart(6, "0")}`,
      tur: "migration",
      ad: `DB Tablo: ${t.name}`,
      kaynak: [`src/lib/db/migrations`],
      hedef: null,
      test: [],
      durum: "planlandi",
      kanit: [],
    });
    sayac++;
  }

  // 4. Sağlayıcı Yürütücüleri
  for (const ex of katalog.executorListesi) {
    davranislar.push({
      id: `B${sayac.toString().padStart(6, "0")}`,
      tur: "saglayici",
      ad: `Yürütücü: ${ex.replace(/^open-sse\/executors\//, "").replace(/\.ts$/, "")}`,
      kaynak: [`${ex}`],
      hedef: null,
      test: [],
      durum: "planlandi",
      kanit: [],
    });
    sayac++;
  }

  return davranislar;
}

// ── ANA KOŞUCU ───────────────────────────────────────────────────────

async function main() {
  console.log("=== KERVAN ENVANTER VE DEFTER ÜRETİM MOTORU ===");
  console.log(`Kaynak: ${REF_DIR}`);

  const tumDosyalar = await tumDosyalariTara(REF_DIR);
  console.log(`Tarandı: ${tumDosyalar.length} dosya.`);

  console.log("1. Dosya dökümü üretiliyor...");
  const dosyalar = await dosyalarEnvanteriUret(tumDosyalar);

  console.log("2. Route tablosu taranıyor...");
  const routes = await routeTablosuUret(tumDosyalar);

  console.log("3. Sayfa tablosu taranıyor...");
  const sayfalar = await sayfaTablosuUret(tumDosyalar);

  console.log("4. Veritabanı göçleri ve altın şema derleniyor...");
  const dbEnvanter = await veritabaniEnvanteriUret(tumDosyalar);

  console.log("5. Sağlayıcı kataloğu ve ortam değişkenleri taranıyor...");
  const katalog = await katalogUret(tumDosyalar);

  console.log("6. Test envanteri taranıyor...");
  const testler = await testlerEnvanteriUret(tumDosyalar);

  console.log("7. Davranışlar tablosu derleniyor...");
  const davranislar = davranislarUret(routes, sayfalar, dbEnvanter, katalog);

  // Dosyalara yaz
  await Bun.write(join(ENVANTER_DIR, "dosyalar.json"), JSON.stringify(dosyalar, null, 2));
  await Bun.write(join(ENVANTER_DIR, "routes.json"), JSON.stringify(routes, null, 2));
  await Bun.write(join(ENVANTER_DIR, "sayfalar.json"), JSON.stringify(sayfalar, null, 2));
  await Bun.write(join(ENVANTER_DIR, "veritabani.json"), JSON.stringify(dbEnvanter, null, 2));
  await Bun.write(join(ENVANTER_DIR, "katalog.json"), JSON.stringify(katalog, null, 2));
  await Bun.write(join(ENVANTER_DIR, "testler.json"), JSON.stringify(testler, null, 2));

  // JSONL Defter dosyaları
  const dosyalarJsonl = dosyalar.map((d) => JSON.stringify(d)).join("\n") + "\n";
  const davranislarJsonl = davranislar.map((b) => JSON.stringify(b)).join("\n") + "\n";

  await Bun.write(join(DEFTER_DIR, "dosyalar.jsonl"), dosyalarJsonl);
  await Bun.write(join(DEFTER_DIR, "davranislar.jsonl"), davranislarJsonl);

  console.log("=== TAMAMLANDI ===");
  console.log(`Toplam Dosya: ${dosyalar.length}`);
  console.log(`Toplam Route: ${routes.length}`);
  console.log(`Toplam Sayfa: ${sayfalar.length}`);
  console.log(`Toplam DB Tablosu: ${dbEnvanter.tabloSayisi} (Göç: ${dbEnvanter.toplamGoc})`);
  console.log(`Toplam Test Dosyası: ${testler.toplamTestDosyasi} (~${testler.tahminiToplamBirimTest} test)`);
  console.log(`Toplam Davranış Birimi: ${davranislar.length}`);
}

main().catch((err) => {
  console.error("HATA:", err);
  process.exit(1);
});
