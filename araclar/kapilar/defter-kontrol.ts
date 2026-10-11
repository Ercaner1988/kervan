import { spawnSync } from "child_process";
import fs from "fs";
import os from "os";
import path from "path";

// PRD FR-2.8 / FR-8.2: Defter Kapısı (defter-kontrol.ts). Fail-closed: okunamayan, boş ya da
// ayrıştırılamayan her girdi kırmızıdır; neden yazılır.
// port/defter/dosyalar.jsonl (pencere satırları) ve port/defter/davranislar.jsonl birlikte denetlenir:
//  1. Her satır JSON nesnesidir; id benzersiz, durum bilinen değerlerdendir.
//  2. dosyalar: kaynak ref/omniroute-dbe703a altında VAR olmalı; satir=[bas,son], pencere ≤ 300 satır;
//     kaynağın pencereleri 1'den dosyanın son satırına kadar boşluksuz örtüşmeli.
//  3. 'dogrulandi': hedef VAR olan bir DOSYA, kanıt listesi boş değil, her kanıt ya ÖLÇÜLMÜŞ bir test kimliği
//     (testKanitlariniOlc: bun testi junit raporuyla koşulup GEÇMİŞ; Rust testi `cargo test --no-run` ikilisinin
//     `--list` çıktısında, #[ignore]'suz) ya da VAR olan port/kayit/** ya da port/altin/** dosyası. Kaynak metni
//     ayrıştırılmaz: koşmayan/derlenmeyen test kanıt olamaz.
//     dosyalar'da dogrulandi: kaynağın OKUNMUŞ (planlandi dışı) pencereleri dosyanın tamamını kapsamalı.
//     davranislar'da dogrulandi: beyan edilen kaynak+satir aralığı okunmuş pencerelerle kapsanmalı.
//  4. 'kapsam-disi' / 'insan-karari' gerekçelidir.
//  5. --final: planlandi/okundu/aktarildi satırı kalmamalı; ref envanterindeki (node_modules/.git hariç) HER dosya
//     dosyalar.jsonl'de yer almalı. Final dışı kipte de toplam/kapanmayan sayıları yazılır.

export const DURUMLAR = ["planlandi", "okundu", "aktarildi", "dogrulandi", "kapsam-disi", "insan-karari"] as const;
const KAPANMAYAN = new Set(["planlandi", "okundu", "aktarildi"]);
export const PENCERE_UST_SINIR = 300;
const REF_ONEKI = "ref/omniroute-dbe703a/";

export interface DefterKaydi {
  id: string;
  kaynak?: string;
  tanim?: string;
  satir?: [number, number];
  durum: (typeof DURUMLAR)[number];
  hedef?: string;
  kanit?: string[];
  gerekce?: string;
}

export interface DefterSayimi {
  toplam: number;
  kapanmayan: number;
  durumlar: Record<string, number>;
}

/** JSONL okur; ayrıştırılamayan satırı atlamaz, hataya yazar. */
export function defterSatirlariniOku(dosyaYolu: string, hatalar: string[] = []): DefterKaydi[] {
  const ad = path.basename(dosyaYolu);
  if (!fs.existsSync(dosyaYolu)) {
    hatalar.push(`${ad} bulunamadı: ${dosyaYolu}`);
    return [];
  }
  const kayitlar: DefterKaydi[] = [];
  const satirlar = fs.readFileSync(dosyaYolu, "utf-8").split(/\r?\n/);
  satirlar.forEach((line, i) => {
    if (!line.trim()) return;
    try {
      const v = JSON.parse(line);
      if (v === null || typeof v !== "object" || Array.isArray(v)) throw new Error("nesne değil");
      kayitlar.push(v);
    } catch (e: any) {
      hatalar.push(`${ad}:${i + 1}: satır JSON olarak ayrıştırılamadı (${e.message})`);
    }
  });
  if (kayitlar.length === 0) hatalar.push(`${ad} boş: denetlenecek kayıt yok`);
  return kayitlar;
}

function dosyaSatirSayisi(icerik: string): number {
  if (icerik === "") return 0;
  return icerik.split("\n").length - (icerik.endsWith("\n") ? 1 : 0);
}

/** `yol`, `kok` dizininin İÇİNDE (çözülmüş ve sembolik bağlar açılmış hâliyle) var olan göreli bir yol mu? */
function kokIcindeVarMi(kok: string, yol: string): boolean {
  if (path.isAbsolute(yol) || !fs.existsSync(kok)) return false;
  const tam = path.resolve(kok, yol);
  if (!fs.existsSync(tam)) return false;
  const rel = path.relative(fs.realpathSync(kok), fs.realpathSync(tam));
  return rel !== "" && rel.split(path.sep)[0] !== ".." && !path.isAbsolute(rel);
}

function dosyalariTara(dizin: string, uzanti: (ad: string) => boolean, cikti: string[] = []): string[] {
  if (!fs.existsSync(dizin)) return cikti;
  for (const g of fs.readdirSync(dizin, { withFileTypes: true })) {
    if (g.name === "target" || g.name === "node_modules" || g.name.startsWith(".")) continue;
    const tam = path.join(dizin, g.name);
    if (g.isDirectory()) dosyalariTara(tam, uzanti, cikti);
    else if (uzanti(g.name)) cikti.push(tam);
  }
  return cikti;
}

const dosyaMi = (y: string) => fs.existsSync(y) && fs.statSync(y).isFile();

function tomlOku(yol: string): any {
  return (globalThis as any).Bun.TOML.parse(fs.readFileSync(yol, "utf-8"));
}

export interface OlcumSecenegi {
  /** Her alt süreç (bun test dosyası, cargo, test ikilisi) için süre sınırı. Varsayılan 10 dk (cargo derlemesi). */
  zamanAsimiMs?: number;
  /** cargo için CARGO_TARGET_DIR (testler os.tmpdir altında ortak hedef dizini verir). Varsayılan: cargo'nun kendi seçimi. */
  cargoHedefDizini?: string;
}
const VARSAYILAN_OLCUM_MS = 10 * 60 * 1000;

function kos(komut: string, arg: string[], cwd: string, ms: number, env: NodeJS.ProcessEnv = process.env) {
  return spawnSync(komut, arg, { cwd, env, encoding: "utf-8", timeout: ms, maxBuffer: 256 * 1024 * 1024, windowsHide: true });
}
const surecHatasi = (p: ReturnType<typeof kos>) =>
  p.error ? `başlatılamadı ya da süre aşıldı (${p.error.message})` : p.status !== 0 ? `çıkış ${p.status ?? p.signal}` : "";
const xmlCoz = (s: string) =>
  s.replace(/&#(\d+);/g, (_t, n) => String.fromCodePoint(Number(n))).replace(/&quot;/g, '"').replace(/&apos;/g, "'").replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&");

/**
 * Bun: adı geçen araclar/**\/*.test.ts dosyaları `bun test --reporter=junit` ile GERÇEKTEN koşulur; kanıt yalnız
 * junit raporunda başarısız/atlanmış olmayan testcase adıdır. Dosya sıfırdan farklı çıkarsa (başka test düştü, üst
 * düzey hata, sözdizimi hatası, süre aşımı) ya da rapor yazılmazsa (üst düzey process.exit) o dosyadan kanıt yoktur.
 * ponytail: aday dosya, adı ham metinde geçen dosyadır; adı dinamik kurulan test görülmez (kanıt kırmızı, güvenli yön).
 */
function bunTestleriniOlc(kok: string, adlar: string[], notlar: string[], ms: number): Set<string> {
  const gecen = new Set<string>();
  const adaylar = dosyalariTara(path.join(kok, "araclar"), (a) => /\.test\.ts$/.test(a)).filter((f) => {
    const ic = fs.readFileSync(f, "utf-8");
    return adlar.some((a) => ic.includes(a));
  });
  for (const f of adaylar) {
    const rel = path.relative(kok, f).replace(/\\/g, "/");
    const gecici = fs.mkdtempSync(path.join(os.tmpdir(), "defter-kontrol-junit-"));
    const xml = path.join(gecici, "rapor.xml");
    try {
      const p = kos(process.execPath, ["test", "--reporter=junit", `--reporter-outfile=${xml}`, `./${rel}`], kok, ms, {
        ...process.env,
        BUN_RUNTIME_TRANSPILER_CACHE_PATH: "0",
      });
      const h = surecHatasi(p);
      if (h) {
        notlar.push(`bun test ${rel}: ${h}; bu dosyadan kanıt yok`);
        continue;
      }
      if (!dosyaMi(xml)) {
        notlar.push(`bun test ${rel}: junit raporu yazılmadı (testler koşmadan süreç bitti); bu dosyadan kanıt yok`);
        continue;
      }
      for (const m of fs.readFileSync(xml, "utf-8").matchAll(/<testcase\b([^>]*?)(\/>|>([\s\S]*?)<\/testcase>)/g)) {
        if (m[3] !== undefined && /<(?:failure|error|skipped)\b/.test(m[3])) continue;
        const ad = /\bname="([^"]*)"/.exec(m[1])?.[1];
        if (ad !== undefined) gecen.add(xmlCoz(ad));
      }
    } finally {
      fs.rmSync(gecici, { recursive: true, force: true });
    }
  }
  return gecen;
}

/**
 * Rust: `cargo test --workspace --no-run` ile derlenen test ikilileri (cargo'nun kendi bildirdiği, profile.test)
 * `--list` ile sorulur; `--list --ignored` çıkarılır. Böylece autotests=false, [lib] test=false, workspace.exclude,
 * cfg, makro, #[ignore] ve cfg_attr(ignore) cargo'nun kendisi tarafından çözülür. harness=false hedefin `--list`'i
 * kendi main'ini koşturur (çıktısı ölçüm değildir): o hedefler sayılmaz. Kimlik: `mod::yol::ad`,
 * `<hedef>::mod::yol::ad`, ve tüm ikililerde tekse çıplak ad.
 */
function cargoTestleriniOlc(kok: string, notlar: string[], sec: OlcumSecenegi): Set<string> {
  const ms = sec.zamanAsimiMs ?? VARSAYILAN_OLCUM_MS;
  const env: NodeJS.ProcessEnv = { ...process.env, CARGO_BUILD_JOBS: "2", CARGO_TERM_COLOR: "never" };
  if (sec.cargoHedefDizini) env.CARGO_TARGET_DIR = sec.cargoHedefDizini;
  const kimlikler = new Set<string>();

  const meta = kos("cargo", ["metadata", "--no-deps", "--format-version", "1"], kok, ms, env);
  let paketler: any[];
  try {
    if (surecHatasi(meta)) throw new Error(surecHatasi(meta));
    paketler = JSON.parse(meta.stdout).packages;
    if (!Array.isArray(paketler)) throw new Error("packages yok");
  } catch (e: any) {
    notlar.push(`cargo metadata: ${e.message}; Rust test kanıtı ölçülemedi`);
    return kimlikler;
  }
  // harness=false hedefler: paket kimliği|hedef adı.
  const harnesssiz = new Set<string>();
  for (const pk of paketler) {
    let t: any;
    try {
      t = tomlOku(pk.manifest_path);
    } catch (e: any) {
      notlar.push(`${pk.manifest_path} okunamadı: ${e.message}; Rust test kanıtı ölçülemedi`);
      return kimlikler;
    }
    const lib = t.lib ? [{ ...t.lib, name: t.lib.name ?? String(pk.name).replace(/-/g, "_") }] : [];
    for (const h of [...lib, ...(t.bin ?? []), ...(t.test ?? []), ...(t.bench ?? [])]) {
      if (h?.harness === false) harnesssiz.add(`${pk.id}|${h.name}`);
    }
  }

  const derle = kos("cargo", ["test", "--workspace", "--no-run", "--message-format=json"], kok, ms, env);
  if (surecHatasi(derle)) {
    notlar.push(`cargo test --no-run: ${surecHatasi(derle)} (${(derle.stderr || "").trim().split(/\r?\n/).slice(-3).join(" | ")}); Rust test kanıtı ölçülemedi`);
    return kimlikler;
  }
  const ciplak = new Map<string, number>();
  for (const satir of derle.stdout.split(/\r?\n/)) {
    let j: any;
    try {
      j = JSON.parse(satir);
    } catch {
      continue; // cargo json akışında JSON olmayan satır yok; olursa ikili sayılmaz
    }
    if (j?.reason !== "compiler-artifact" || !j.profile?.test || typeof j.executable !== "string") continue;
    if (harnesssiz.has(`${j.package_id}|${j.target?.name}`)) continue;
    const liste = (arg: string[]) => {
      const p = kos(j.executable, ["--list", ...arg], kok, ms, env);
      if (surecHatasi(p)) throw new Error(`${path.basename(j.executable)} --list ${arg.join(" ")}: ${surecHatasi(p)}`);
      return p.stdout.split(/\r?\n/).flatMap((x) => /^(.+): test$/.exec(x)?.[1] ?? []);
    };
    let tumu: string[], yok: Set<string>;
    try {
      tumu = liste([]);
      yok = new Set(liste(["--ignored"]));
    } catch (e: any) {
      notlar.push(`${e.message}; bu ikiliden kanıt yok`);
      continue;
    }
    for (const t of tumu) {
      if (yok.has(t)) continue;
      kimlikler.add(t);
      kimlikler.add(`${j.target.name}::${t}`);
      const ad = t.split("::").pop()!;
      ciplak.set(ad, (ciplak.get(ad) ?? 0) + 1);
    }
  }
  for (const [ad, n] of ciplak) if (n === 1) kimlikler.add(ad);
  return kimlikler;
}

/**
 * Defterde test adı olarak anılan kanıtları ÖLÇER (statik ayrıştırma yok): Bun testleri koşulur, Rust testleri
 * derlenen ikililerden listelenir. cargo yalnız anılan adlardan biri bir .rs dosyasında geçiyorsa koşar.
 */
export function testKanitlariniOlc(kok: string, adlar: string[], notlar: string[] = [], sec: OlcumSecenegi = {}): Set<string> {
  const ms = sec.zamanAsimiMs ?? VARSAYILAN_OLCUM_MS;
  const kimlikler = adlar.length ? bunTestleriniOlc(kok, adlar, notlar, ms) : new Set<string>();
  const kalan = adlar.filter((a) => !kimlikler.has(a));
  if (kalan.length && dosyaMi(path.join(kok, "Cargo.toml"))) {
    const sonAd = kalan.map((a) => a.split("::").pop()!).filter(Boolean);
    const rsVar = dosyalariTara(kok, (a) => a.endsWith(".rs")).some((f) => {
      const ic = fs.readFileSync(f, "utf-8");
      return sonAd.some((a) => ic.includes(a));
    });
    if (rsVar) for (const k of cargoTestleriniOlc(kok, notlar, sec)) kimlikler.add(k);
  }
  return kimlikler;
}

function kanitGecerliMi(kanit: unknown, kok: string, testler: Set<string>): boolean {
  if (typeof kanit !== "string" || !kanit.trim()) return false;
  const k = kanit.replace(/\\/g, "/");
  for (const on of ["port/kayit/", "port/altin/"]) {
    // Önek dizge olarak değil, çözülen yolla denetlenir: 'port/kayit/../../Cargo.toml' port/kayit dışındadır.
    if (k.startsWith(on)) return kokIcindeVarMi(path.join(kok, on), k.slice(on.length)) && dosyaMi(path.resolve(kok, k));
  }
  if (k.includes("/")) return false; // başka yol kanıt değildir (test dosyası yolu test adı değildir)
  return testler.has(k);
}

function aralikGecerliMi(s: unknown): s is [number, number] {
  return Array.isArray(s) && s.length === 2 && Number.isInteger(s[0]) && Number.isInteger(s[1]) && s[0] >= 1 && s[1] >= s[0];
}

/** [bas,son] aralığının pencerelerle kapsanmayan ilk alt aralıkları. */
function kapsanmayanlar(pencereler: [number, number][], bas: number, son: number): [number, number][] {
  const bos: [number, number][] = [];
  let beklenen = bas;
  for (const [a, b] of [...pencereler].sort((x, y) => x[0] - y[0])) {
    if (b < beklenen) continue;
    if (a > son) break;
    if (a > beklenen) bos.push([beklenen, Math.min(a - 1, son)]);
    beklenen = Math.max(beklenen, b + 1);
    if (beklenen > son) break;
  }
  if (beklenen <= son) bos.push([beklenen, son]);
  return bos;
}

export interface DefterEkSecenek extends OlcumSecenegi {
  davranislarYolu?: string;
  refDizin?: string;
  /** Ölçülmüş test kimlikleri (verilirse ölçüm koşulmaz; yalnız birim testleri için). */
  testAdlari?: Set<string>;
}

export function defteriDenetle(
  dosyalarJsonlYolu?: string,
  finalModu = false,
  kokDizin = process.cwd(),
  ek: DefterEkSecenek = {}
): { uygun: boolean; hatalar: string[]; kayitSayisi: number; sayim: DefterSayimi } {
  const hatalar: string[] = [];
  const kok = path.resolve(kokDizin);
  const dosyalarYolu = dosyalarJsonlYolu ? path.resolve(kok, dosyalarJsonlYolu) : path.join(kok, "port", "defter", "dosyalar.jsonl");
  const davranislarYolu = ek.davranislarYolu
    ? path.resolve(kok, ek.davranislarYolu)
    : path.join(path.dirname(dosyalarYolu), "davranislar.jsonl");
  // Ref yalnız açık argümanla (--ref / ek.refDizin) değişir; ortam değişkeni okunmaz (Bun cwd'deki .env'i kendiliğinden yükler).
  const refDizin = path.resolve(ek.refDizin ?? path.join(kok, "ref", "omniroute-dbe703a"));

  const dosyalar = defterSatirlariniOku(dosyalarYolu, hatalar);
  const davranislar = defterSatirlariniOku(davranislarYolu, hatalar);
  const sayim: DefterSayimi = { toplam: 0, kapanmayan: 0, durumlar: {} };

  const refVar = fs.existsSync(refDizin) && fs.statSync(refDizin).isDirectory();
  if (!refVar) hatalar.push(`OmniRoute ref dizini bulunamadı: ${refDizin} (kaynak varlığı ölçülemiyor)`);

  // Test adı olarak anılan kanıtlar bir kez, birlikte ölçülür (bun test koşusu / cargo --list).
  const anilanAdlar = [...new Set([...dosyalar, ...davranislar].flatMap((k) => (k?.durum === "dogrulandi" && Array.isArray(k.kanit) ? k.kanit : [])).filter((x): x is string => typeof x === "string" && !!x.trim() && !x.replace(/\\/g, "/").includes("/")))];
  const olcumNotlari: string[] = [];
  let testler: Set<string> | undefined = ek.testAdlari;
  const testAdlari = () => {
    if (testler) return testler;
    try {
      testler = testKanitlariniOlc(kok, anilanAdlar, olcumNotlari, ek);
    } catch (e: any) {
      hatalar.push(`test kanıtları ölçülemedi: ${e?.message ?? e}`);
      testler = new Set();
    }
    return testler;
  };

  // Kaynak dosya satır sayıları (önbellek); null = yok/okunamadı.
  const satirSayilari = new Map<string, number | null>();
  const kaynakSatirSayisi = (kaynak: string): number | null => {
    if (satirSayilari.has(kaynak)) return satirSayilari.get(kaynak)!;
    let n: number | null = null;
    const tam = path.resolve(refDizin, kaynak);
    const rel = path.relative(refDizin, tam);
    if (refVar && !rel.startsWith("..") && !path.isAbsolute(rel) && fs.existsSync(tam) && fs.statSync(tam).isFile()) {
      n = dosyaSatirSayisi(fs.readFileSync(tam, "utf-8"));
    }
    satirSayilari.set(kaynak, n);
    return n;
  };
  const kaynakNormalle = (k: string) => {
    const s = k.replace(/\\/g, "/");
    return s.startsWith(REF_ONEKI) ? s.slice(REF_ONEKI.length) : s;
  };

  const tumPencereler = new Map<string, [number, number][]>();
  const okunmusPencereler = new Map<string, [number, number][]>();
  const dogrulananKaynaklar = new Set<string>();
  const gorulenIdler = new Set<string>();

  const ortakDenetim = (k: DefterKaydi, yer: string) => {
    const etiket = `${yer} (${(k as any).id ?? "?"})`;
    if (typeof k.id !== "string" || !k.id.trim()) hatalar.push(`${etiket}: 'id' eksik`);
    else if (gorulenIdler.has(k.id)) hatalar.push(`${etiket}: yinelenen id '${k.id}'`);
    else gorulenIdler.add(k.id);

    if (!(DURUMLAR as readonly string[]).includes(k.durum)) {
      hatalar.push(`${etiket}: bilinmeyen durum '${k.durum}' (beklenen: ${DURUMLAR.join("|")})`);
      return etiket;
    }
    sayim.toplam++;
    sayim.durumlar[k.durum] = (sayim.durumlar[k.durum] ?? 0) + 1;
    if (KAPANMAYAN.has(k.durum)) {
      sayim.kapanmayan++;
      if (finalModu) hatalar.push(`${etiket}: Final modunda kapanmamış durum: '${k.durum}'`);
    }

    if (k.durum === "dogrulandi") {
      if (typeof k.hedef !== "string" || !k.hedef.trim()) {
        hatalar.push(`${etiket}: 'dogrulandi' durumunda ancak 'hedef' dosyası belirtilmemiş`);
      } else if (!kokIcindeVarMi(kok, k.hedef) || !dosyaMi(path.resolve(kok, k.hedef))) {
        hatalar.push(`${etiket}: 'hedef' bir dosya değil ya da bulunamadı: ${k.hedef}`);
      }
      if (!Array.isArray(k.kanit) || k.kanit.length === 0) {
        hatalar.push(`${etiket}: 'dogrulandi' durumunda ancak 'kanit' listesi boş`);
      } else {
        for (const kanit of k.kanit) {
          if (!kanitGecerliMi(kanit, kok, testAdlari())) {
            hatalar.push(
              `${etiket}: Kanıt dosyası bulunamadı ya da kanıt değil: '${kanit}' (kanıt = koşan ve geçen bun testinin ya da derlenen, #[ignore]'suz cargo testinin adı, ya da var olan port/kayit/** veya port/altin/** dosyası)${olcumNotlari.length ? `; ölçüm notları: ${olcumNotlari.slice(0, 3).join(" | ")}` : ""}`
            );
          }
        }
      }
    }
    if ((k.durum === "kapsam-disi" || k.durum === "insan-karari") && (typeof k.gerekce !== "string" || !k.gerekce.trim())) {
      hatalar.push(`${etiket}: '${k.durum}' durumunda gerekçe belirtilmemiş`);
    }
    return etiket;
  };

  /** kaynak + satir doğrular; geçerliyse [normal kaynak, aralık, satır sayısı] döner. */
  const kaynakAraligi = (k: DefterKaydi, etiket: string): [string, [number, number], number] | null => {
    if (typeof k.kaynak !== "string" || !k.kaynak.trim()) {
      hatalar.push(`${etiket}: 'kaynak' eksik`);
      return null;
    }
    if (!aralikGecerliMi(k.satir)) {
      hatalar.push(`${etiket}: 'satir' [bas, son] biçiminde değil (1 ≤ bas ≤ son): ${JSON.stringify(k.satir)}`);
      return null;
    }
    const kaynak = kaynakNormalle(k.kaynak);
    const n = kaynakSatirSayisi(kaynak);
    if (n === null) {
      if (refVar) hatalar.push(`${etiket}: kaynak dosya ref/omniroute-dbe703a içinde yok: ${k.kaynak}`);
      return null;
    }
    if (k.satir[1] > n) {
      // Dosyada olmayan satır okunamaz ya da doğrulanamaz: beyan yanlıştır.
      hatalar.push(`${etiket}: aralık [${k.satir[0]}, ${k.satir[1]}] dosya sonunu (${n} satır) aşıyor: ${kaynak}`);
      return null;
    }
    return [kaynak, k.satir, n];
  };

  dosyalar.forEach((k, i) => {
    const etiket = ortakDenetim(k, `dosyalar.jsonl #${i + 1}`);
    const r = kaynakAraligi(k, etiket);
    if (!r) return;
    const [kaynak, [bas, son]] = r;
    if (son - bas + 1 > PENCERE_UST_SINIR) {
      hatalar.push(`${etiket}: pencere [${bas}, ${son}] ${PENCERE_UST_SINIR} satırı aşıyor (${son - bas + 1})`);
    }
    if (!tumPencereler.has(kaynak)) tumPencereler.set(kaynak, []);
    tumPencereler.get(kaynak)!.push([bas, son]);
    if (k.durum !== "planlandi") {
      if (!okunmusPencereler.has(kaynak)) okunmusPencereler.set(kaynak, []);
      okunmusPencereler.get(kaynak)!.push([bas, son]);
    }
    if (k.durum === "dogrulandi") dogrulananKaynaklar.add(kaynak);
  });

  for (const [kaynak, pencereler] of tumPencereler) {
    const n = satirSayilari.get(kaynak)!;
    for (const [a, b] of kapsanmayanlar(pencereler, 1, n)) {
      hatalar.push(`Kaynak '${kaynak}' içinde satır aralığı boşluğu tespit edildi: [${a}, ${b}] okunmadı! (dosya ${n} satır)`);
    }
  }
  for (const kaynak of dogrulananKaynaklar) {
    const n = satirSayilari.get(kaynak)!;
    for (const [a, b] of kapsanmayanlar(okunmusPencereler.get(kaynak) ?? [], 1, n)) {
      hatalar.push(`Kaynak '${kaynak}' 'dogrulandi' ama [${a}, ${b}] okunmamış (planlandi); dosyanın tamamı okunmadan doğrulanamaz`);
    }
  }

  // --final: ref envanterindeki HER dosya dosyalar.jsonl'de yer almalı (FR-2.8 "her kaynak dosya için"); 10 satırlık defter 23.872 dosyayı kapatamaz.
  if (finalModu && refVar) {
    const anilan = new Set(dosyalar.map((k) => (typeof k.kaynak === "string" ? kaynakNormalle(k.kaynak) : "")));
    const eksik: string[] = [];
    const tara = (dz: string, on: string) => {
      for (const g of fs.readdirSync(dz, { withFileTypes: true })) {
        if (g.name === "node_modules" || g.name === ".git") continue;
        const rel = on ? `${on}/${g.name}` : g.name;
        if (g.isDirectory()) tara(path.join(dz, g.name), rel);
        else if (!anilan.has(rel)) eksik.push(rel);
      }
    };
    tara(refDizin, "");
    if (eksik.length) {
      hatalar.push(`Final: ref envanterindeki ${eksik.length} dosya defterde (dosyalar.jsonl) hiç yok; ilk: ${eksik.slice(0, 20).join(", ")}`);
    }
  }

  davranislar.forEach((k, i) => {
    const etiket = ortakDenetim(k, `davranislar.jsonl #${i + 1}`);
    if (k.durum !== "dogrulandi") return;
    const r = kaynakAraligi(k, etiket);
    if (!r) return;
    const [kaynak, [bas, son]] = r;
    for (const [a, b] of kapsanmayanlar(okunmusPencereler.get(kaynak) ?? [], bas, son)) {
      hatalar.push(`${etiket}: beyan edilen davranış aralığı [${a}, ${b}] (${kaynak}) dosyalar.jsonl'de okunmamış`);
    }
  });

  return { uygun: hatalar.length === 0, hatalar, kayitSayisi: dosyalar.length + davranislar.length, sayim };
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  let finalModu = false;
  let kok = process.cwd();
  let dosyaYolu: string | undefined;
  let refDizin: string | undefined;
  let argHatasi = "";
  for (let i = 0; i < args.length; i++) {
    const a = args[i];
    if (a === "--final") finalModu = true;
    else if (a === "--kok" && args[i + 1]) kok = path.resolve(args[++i]);
    else if (a === "--ref" && args[i + 1]) refDizin = path.resolve(args[++i]);
    else if (!a.startsWith("--")) dosyaYolu = a;
    else argHatasi = `bilinmeyen argüman: ${a}`;
  }

  let sonuc: ReturnType<typeof defteriDenetle>;
  try {
    if (argHatasi) throw new Error(argHatasi);
    sonuc = defteriDenetle(dosyaYolu, finalModu, kok, { refDizin });
  } catch (e: any) {
    console.log(`[defter-kontrol] çöktü: ${e?.message ?? e}`);
    console.log(`HUKUM: kirmizi defter-kontrol çöktü: ${e?.message ?? e}`);
    process.exit(1);
  }
  const d = sonuc.sayim.durumlar;
  console.log(
    `[defter-kontrol] Final modu: ${finalModu ? "AÇIK" : "KAPALI"} | toplam: ${sonuc.sayim.toplam} | kapanmayan: ${sonuc.sayim.kapanmayan} | ` +
      DURUMLAR.map((x) => `${x}=${d[x] ?? 0}`).join(" ")
  );
  if (sonuc.uygun) {
    console.log("HUKUM: yesil");
    process.exit(0);
  }
  console.log(`[defter-kontrol] ${sonuc.hatalar.length} defter ihlali:`);
  for (const h of sonuc.hatalar) console.log(`  - ${h}`);
  console.log(`HUKUM: kirmizi ${sonuc.hatalar.length} defter ihlali; ilki: ${sonuc.hatalar[0].replace(/\s+/g, " ")}`);
  process.exit(1);
}
