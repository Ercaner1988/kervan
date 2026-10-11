import { spawnSync, type SpawnSyncReturns } from "child_process";
import crypto from "crypto";
import fs from "fs";
import os from "os";
import path from "path";
import { cargoOrtami, cargoTestListesiniAl } from "./kabul-haritasi.ts";

// PRD FR-2.6: Mutasyon ve Parite Bağlantısı Kapısı (mutasyon-kontrol.ts)
// 1. port/mutantlar.toml'daki her mutant için:
//    a. test hedefi gerçekten var mı (cargo test --list / cargo metadata test hedefi / dosya varlığı),
//    b. TABAN: mutasyonsuz koşuda test yeşil mi (değilse 'taban kırmızı'),
//    c. mutasyon uygulanır (yedek alınır), test koşulur, dosya yedekten bayt bayt geri yazılır
//       ve geri yazıldığı doğrulanır,
//    d. 'öldü' YALNIZ gerçek test hatasıdır (cargo 'test result: FAILED', bun test düşen test,
//       kapı 'HUKUM: kirmizi'); derleme hatası, zaman aşımı, 0 test, tanınmayan çıktı = 'belirsiz' -> kırmızı.
//    Her koşunun ham çıktısı argümanla verilen günlük dizinine yazılır.
// 2. araclar/parite/*.ts karşılaştırıcıları deponun geçici kopyasında, süreç/ağ çağrılarını
//    kaydeden bir ön yükleyiciyle üç kez koşulur; Kervan çağrısı (cargo run/test -p <paket>, --bin)
//    stdin girdisi port/altin/*.json altınlarından biriyle eşlenip sahte sonuç alır: esit = altının
//    çıktısı birebir (karşılaştırıcı 0 vermeli), fark = altında tek alan nonce (sıfır-dışı ve farkta
//    nonce), hata = çıkış 101 (sıfır-dışı). Kip karşılaştırıcının görebildiği hiçbir yerde yazılı değil.
// Son satır: 'HUKUM: yesil' ya da 'HUKUM: kirmizi <neden>'.

export interface MutantTanimi {
  ad: string;
  dosya: string;
  bul: string;
  yerine: string;
  oldurmesi_gereken_test: string[];
}

export const ASGARI_MUTANT_SAYISI = 8;
/** PRD FR-2.6-a: listede en az bu iki mutant bulunur. */
export const ZORUNLU_MUTANTLAR = ["rest-kancasini-kaldir", "claude-eslemesinde-bir-alani-boz"];
const VARSAYILAN_ZAMAN_ASIMI_MS = 60 * 60 * 1000;

/** mutantlar.toml'u Bun.TOML ile okur. Dosya yoksa, ayrıştırılamazsa ya da bir girdi eksikse hata atar. */
export function mutantlarTomlOku(dosyaYolu = path.join("port", "mutantlar.toml")): MutantTanimi[] {
  if (!fs.existsSync(dosyaYolu)) throw new Error(`Mutant listesi bulunamadı: ${dosyaYolu}`);
  let veri: any;
  try {
    veri = Bun.TOML.parse(fs.readFileSync(dosyaYolu, "utf-8"));
  } catch (e: any) {
    throw new Error(`${dosyaYolu} ayrıştırılamadı: ${e.message}`);
  }
  const girdiler = veri?.mutant;
  if (!Array.isArray(girdiler)) throw new Error(`${dosyaYolu} içinde [[mutant]] listesi yok`);

  const adlar = new Set<string>();
  return girdiler.map((g: any, i: number) => {
    const yer = `${dosyaYolu} [[mutant]] #${i + 1}${typeof g?.ad === "string" ? ` (${g.ad})` : ""}`;
    for (const alan of ["ad", "dosya", "bul"]) {
      if (typeof g?.[alan] !== "string" || !g[alan]) throw new Error(`${yer}: '${alan}' eksik ya da boş`);
    }
    if (typeof g.yerine !== "string") throw new Error(`${yer}: 'yerine' eksik`);
    const testler = g.oldurmesi_gereken_test;
    if (!Array.isArray(testler) || testler.length === 0 || testler.some((t: any) => typeof t !== "string" || !t.trim())) {
      throw new Error(`${yer}: 'oldurmesi_gereken_test' boş olmayan dizge listesi olmalı`);
    }
    if (adlar.has(g.ad)) throw new Error(`${yer}: '${g.ad}' adı yineleniyor`);
    adlar.add(g.ad);
    return { ad: g.ad, dosya: g.dosya, bul: g.bul, yerine: g.yerine, oldurmesi_gereken_test: testler.map((t: string) => t.trim()) };
  });
}

export function mutantKumesiniDenetle(mutantlar: MutantTanimi[]): string[] {
  const hatalar: string[] = [];
  if (mutantlar.length < ASGARI_MUTANT_SAYISI) {
    hatalar.push(`Mutant sayısı ${mutantlar.length}; en az ${ASGARI_MUTANT_SAYISI} gerekli`);
  }
  for (const ad of ZORUNLU_MUTANTLAR) {
    if (!mutantlar.some((m) => m.ad === ad)) hatalar.push(`Zorunlu mutant listede yok: '${ad}'`);
  }
  return hatalar;
}

// ---------------------------------------------------------------- parite bağlantısı (çalışma zamanı)

/**
 * Karşılaştırıcının içine yüklenen kayıtçı: her süreç başlatma (Bun.spawn/spawnSync; child_process
 * de bunlardan geçer) ve fetch çağrısının argv'sini kayda yazar. Gerçek cargo ve ağ hiç koşmaz.
 * Kervan çağrısı (kervanCagrisiMi) engellenmez; stdin'deki girdi port/altin/*.json'daki bir
 * altının `girdi`siyle eşlenir ve kipe göre SAHTE sonuç döner:
 *  - esit: çıkış 0, stdout altının `cikti`sı birebir (karşılaştırıcı 0 vermeli),
 *  - fark: çıkış 0, stdout altının `cikti`sında rastgele TEK alan nonce'la değiştirilmiş hâli
 *          (karşılaştırıcı sıfır-dışı vermeli ve farkta nonce görünmeli),
 *  - hata: çıkış 101, boş stdout.
 * Yapılandırma ortam değişkeniyle değil, kayıtçının içine gömülü ve okunur okunmaz silinen bir
 * dosyayla gelir; --preload bayrağı process.execArgv'den silinir. Kip, karşılaştırıcının
 * görebildiği hiçbir yerde yazılı değildir; üç kip aynı ortamla koşar. Çağrı kaydı kipten bağımsızdır
 * (bozulan alan yazılmaz) ve kopyanın atası OLMAYAN, adı rastgele ayrı bir mkdtemp dizininde durur;
 * her kipten sonra kayıt, yapılandırma ve kayıtçı silinir.
 * ponytail: süreç içi kanca; Bun.spawnSync.toString() gibi yansıtmayla sezilebilir. Üst sınır:
 * karşılaştırıcıyı ayrı bir sahte `cargo` ikilisi (PATH) ile koşmak.
 */
const kayitciKaynagi = (yapilandirma: string) => `
import fs from "fs";
import { kervanCagrisiMi } from ${JSON.stringify(Bun.pathToFileURL(import.meta.path).href)};
const y = JSON.parse(fs.readFileSync(${JSON.stringify(yapilandirma)}, "utf-8"));
fs.rmSync(${JSON.stringify(yapilandirma)}, { force: true });
process.execArgv.length = 0;
const paketler = y.paketler.map((p) => ({ ad: p.ad, binHedefleri: new Set(p.bin), testHedefleri: new Set() }));
const yaz = (o) => { try { fs.appendFileSync(y.kayit, JSON.stringify(o) + "\\n"); } catch {} };
const argvAl = (a) => (Array.isArray(a) ? a : Array.isArray(a && a.cmd) ? a.cmd : [String(a)]);
const metin = (v) => (v == null || typeof v === "string" && ["pipe", "inherit", "ignore"].includes(v) ? "" : typeof v === "string" ? v : Buffer.from(v).toString("utf-8"));
const stdinAl = (a, b) => { const s = (a && !Array.isArray(a) ? a : b) || {}; return metin(s.stdin ?? (Array.isArray(s.stdio) ? s.stdio[0] : undefined)); };
const kanonik = (v) => JSON.stringify(v, (k, d) => (d && typeof d === "object" && !Array.isArray(d) ? Object.fromEntries(Object.keys(d).sort().map((x) => [x, d[x]])) : d));
const altinBul = (argv, girdi) => {
  let g; try { g = kanonik(JSON.parse(girdi)); } catch { return undefined; }
  let adaylar = y.altinlar.filter((a) => kanonik(a.girdi) === g);
  if (new Set(adaylar.map((a) => kanonik(a.cikti))).size > 1) adaylar = adaylar.filter((a) => argv.some((p) => p.length > 2 && a.dosya.toLowerCase().includes(p.toLowerCase())));
  return new Set(adaylar.map((a) => kanonik(a.cikti))).size === 1 ? adaylar[0] : undefined;
};
const yapraklar = (v, yol = []) => (v && typeof v === "object" ? Object.entries(v).flatMap(([k, d]) => yapraklar(d, [...yol, k])) : [yol]);
const boz = (cikti) => {
  const l = yapraklar(cikti);
  const sec = l[Math.floor(Math.random() * l.length)];
  if (sec.length === 0) return { cikti: y.nonce, yol: "(kök)" };
  const kopya = JSON.parse(JSON.stringify(cikti));
  let o = kopya; for (const k of sec.slice(0, -1)) o = o[k];
  o[sec[sec.length - 1]] = y.nonce;
  return { cikti: kopya, yol: sec.join(".") };
};
const sahte = (argv, girdi) => {
  const altin = altinBul(argv, girdi);
  if (y.kip === "hata") { yaz({ kervan: argv, altin: altin?.dosya ?? null }); return { kod: 101, cikti: "", hata: "error: process didn't exit successfully (exit code: 101)\\n" }; }
  if (!altin) {
    yaz({ kervan: argv, altin: null });
    return y.kip === "fark" ? { kod: 0, cikti: JSON.stringify({ [y.nonce]: y.nonce }) + "\\n", hata: "" } : { kod: 101, cikti: "", hata: "error: girdi tanınmadı\\n" };
  }
  if (y.kip === "esit") { yaz({ kervan: argv, altin: altin.dosya }); return { kod: 0, cikti: JSON.stringify(altin.cikti) + "\\n", hata: "" }; }
  const b = boz(altin.cikti);
  yaz({ kervan: argv, altin: altin.dosya });
  return { kod: 0, cikti: JSON.stringify(b.cikti) + "\\n", hata: "" };
};
const karar = (a, b) => {
  const argv = argvAl(a).map(String);
  if (!kervanCagrisiMi(argv, paketler)) { yaz({ diger: argv }); throw new Error("Kervan dışı süreç başlatma engellendi: " + argv.join(" ")); }
  return sahte(argv, stdinAl(a, b));
};
Bun.spawnSync = (a, b) => {
  const s = karar(a, b);
  return { pid: 1, success: s.kod === 0, exitCode: s.kod, signalCode: null, stdout: Buffer.from(s.cikti), stderr: Buffer.from(s.hata), exitedDueToTimeout: false, exitedDueToMaxBuffer: false };
};
Bun.spawn = (a, b) => {
  const s = karar(a, b);
  const secenek = (a && !Array.isArray(a) ? a : b) || {};
  if (typeof secenek.onExit === "function") queueMicrotask(() => secenek.onExit(undefined, s.kod, null));
  return { pid: 1, stdin: null, stdout: new Response(s.cikti).body, stderr: new Response(s.hata).body, exited: Promise.resolve(s.kod), exitCode: s.kod, signalCode: null, killed: false, kill() {}, ref() {}, unref() {} };
};
globalThis.fetch = async (u) => { yaz({ diger: ["fetch", String((u && u.url) || u)] }); throw new Error("ağ engellendi"); };
`;

/** port/altin altındaki {girdi, cikti} altınlarını okur; bozuk JSON hata atar. */
function altinlariOku(kok: string): { dosya: string; girdi: unknown; cikti: unknown }[] {
  const dizin = path.join(kok, "port", "altin");
  if (!fs.existsSync(dizin)) return [];
  return fs.readdirSync(dizin).filter((d) => d.endsWith(".json")).flatMap((d) => {
    let v: any;
    try {
      v = JSON.parse(fs.readFileSync(path.join(dizin, d), "utf-8"));
    } catch (e: any) {
      throw new Error(`port/altin/${d} ayrıştırılamadı: ${e.message}`);
    }
    return v && typeof v === "object" && "girdi" in v && "cikti" in v ? [{ dosya: d, girdi: v.girdi, cikti: v.cikti }] : [];
  });
}

const KABUKLAR = new Set(["cmd", "sh", "bash", "powershell", "pwsh"]);
const programAdi = (s: string) => path.basename(s).replace(/\.(?:exe|cmd|bat)$/i, "").toLowerCase();

/** Kayıtlı argv, workspace'teki bir Kervan ikilisini ya da paketini mi çalıştırıyor? */
export function kervanCagrisiMi(argv: string[], paketler: PaketBilgisi[]): boolean {
  let a = argv;
  if (a.length > 1 && KABUKLAR.has(programAdi(a[0]))) {
    a = a[a.length - 1].split(/\s+/).map((p) => p.replace(/^["']|["']$/g, "")).filter(Boolean);
  }
  if (a.length === 0) return false;
  const binler = new Set(paketler.flatMap((p) => [...p.binHedefleri]));
  const program = programAdi(a[0]);
  if (binler.has(program)) return true;
  if (program !== "cargo" || !["run", "test", "bench"].includes(a[1])) return false;
  const sonra = (bayraklar: string[]) => {
    const k = a.findIndex((p) => bayraklar.includes(p));
    return k === -1 ? undefined : a[k + 1];
  };
  const paket = sonra(["-p", "--package"]);
  const bin = sonra(["--bin"]);
  return (!!paket && paketler.some((p) => p.ad === paket)) || (!!bin && binler.has(bin));
}

/** Kökü geçici dizine kopyalar; .git ve target atlanır, ref/ ve node_modules bağlantı (junction) olur. */
function geciciKopya(kok: string, hedef: string) {
  fs.cpSync(kok, hedef, {
    recursive: true,
    filter: (kaynak, varis) => {
      const ad = path.basename(kaynak);
      if (ad === ".git" || ad === "target") return false;
      if ((ad === "ref" || ad === "node_modules") && fs.statSync(kaynak).isDirectory()) {
        // ponytail: salt-okunur kabul edilir; ref/ mühürlü, yazılırsa muhur-kontrol yakalar.
        fs.mkdirSync(path.dirname(varis), { recursive: true });
        fs.symlinkSync(kaynak, varis, "junction");
        return false;
      }
      return true;
    },
  });
}

export function pariteBaglantisiniDenetle(
  pariteDizini = path.join("araclar", "parite"),
  secenekler: { kokDizin?: string; zamanAsimiMs?: number } = {}
): {
  uygun: boolean;
  hatalar: string[];
  /** Kusur ölçüldü: bir karşılaştırıcı Kervan'ı çağırmadı ya da Kervan'ın çıktısına/çıkışına bağlı değil. false + uygun=false = ölçülemedi (eksik girdi, koşturma hatası). */
  kendiniDogrulayan: boolean;
} {
  const kirmizi = (h: string) => ({ uygun: false, hatalar: [h], kendiniDogrulayan: false });
  let kendiniDogrulayan = false;
  const kok = path.resolve(secenekler.kokDizin ?? process.cwd());
  const dizin = path.resolve(kok, pariteDizini);
  if (!fs.existsSync(dizin)) return kirmizi(`Parite dizini bulunamadı: ${pariteDizini}`);
  const gore = path.relative(kok, dizin);
  if (gore.startsWith("..") || path.isAbsolute(gore)) return kirmizi(`Parite dizini kökün dışında: ${dizin} (kök ${kok})`);
  const dosyalar = fs.readdirSync(dizin, { withFileTypes: true }).filter((e) => e.isFile() && e.name.endsWith(".ts")).map((e) => e.name);
  if (dosyalar.length === 0) return kirmizi(`Parite dizininde karşılaştırıcı (.ts) yok: ${pariteDizini}`);

  let altinlar: ReturnType<typeof altinlariOku>;
  try {
    altinlar = altinlariOku(kok);
  } catch (e: any) {
    return kirmizi(e.message);
  }
  // Altın yoksa da bağlantı ölçülür (yalnız eşit kipi): Kervan'ı hiç çağırmayan karşılaştırıcı ölçülmüş kusurdur.
  const altinYok = altinlar.length === 0;

  const { paketler, hata } = cargoPaketleriniAl(kok);
  if (hata) return kirmizi(`Kervan paketleri belirlenemedi: ${hata}`);
  if (paketler.length === 0) return kirmizi("cargo metadata hiç workspace paketi listelemedi");

  const gecici = fs.mkdtempSync(path.join(os.tmpdir(), "kervan-mutasyon-parite-"));
  // Kayıt/yapılandırma/kayıtçı: adı rastgele, kopyanın atası olmayan ayrı dizin.
  const gizli = fs.mkdtempSync(path.join(os.tmpdir(), crypto.randomBytes(6).toString("hex")));
  const hatalar: string[] = [];
  try {
    const kopya = path.join(gecici, "kopya");
    geciciKopya(kok, kopya);
    const paketJson = paketler.map((p) => ({ ad: p.ad, bin: [...p.binHedefleri] }));
    for (const [j, ad] of dosyalar.entries()) {
      for (const kip of (altinYok ? ["esit"] : ["esit", "fark", "hata"]) as ("esit" | "fark" | "hata")[]) {
        // Kip ve nonce dosya adlarında da geçmez: hepsi rastgele.
        const rastgele = () => crypto.randomBytes(12).toString("hex");
        const nonce = `n${rastgele()}`;
        const kayit = path.join(gizli, rastgele());
        const yapilandirma = path.join(gizli, rastgele());
        const kayitci = path.join(gizli, `${rastgele()}.ts`);
        fs.writeFileSync(kayit, "");
        fs.writeFileSync(yapilandirma, JSON.stringify({ kayit, kip, nonce, paketler: paketJson, altinlar }), "utf-8");
        fs.writeFileSync(kayitci, kayitciKaynagi(yapilandirma), "utf-8");
        const r = spawnSync("bun", ["--preload", kayitci, path.join(kopya, gore, ad)], {
          cwd: kopya,
          encoding: "utf-8",
          env: process.env,
          timeout: secenekler.zamanAsimiMs ?? 2 * 60 * 1000,
          maxBuffer: 64 * 1024 * 1024,
        });
        const yer = `Parite karşılaştırıcısı '${ad}' (${kip} kipi)`;
        if ((r.error as any)?.code === "ETIMEDOUT") {
          hatalar.push(`${yer} zaman aşımına uğradı; bağlantı kanıtlanamadı`);
          break;
        }
        if (r.error || r.status === null) {
          hatalar.push(`${yer} koşturulamadı: ${r.error?.message ?? r.signal}`);
          break;
        }
        let kayitlar: { kervan?: string[]; diger?: string[]; altin?: string | null }[];
        try {
          kayitlar = fs.readFileSync(kayit, "utf-8").split("\n").filter(Boolean).map((s) => JSON.parse(s));
        } catch (e: any) {
          hatalar.push(`${yer}: çağrı kaydı okunamadı: ${e.message}`);
          break;
        } finally {
          for (const d of [kayit, yapilandirma, kayitci]) fs.rmSync(d, { force: true });
        }
        const kervan = kayitlar.filter((k) => k.kervan);
        if (kervan.length === 0) {
          const ozet = kayitlar.map((k) => (k.diger ?? []).join(" ")).slice(0, 5).join(" | ") || "hiç süreç/ağ çağrısı yok";
          kendiniDogrulayan = true;
          hatalar.push(`Kendini doğrulayan parite tespit edildi: ${yer} koşuda Kervan ikilisini ya da paketini çağırmadı (kayıtlı çağrılar: ${ozet})`);
          break;
        }
        if (altinYok) {
          hatalar.push(`${yer} Kervan'ı çağırıyor ama port/altin altında {girdi, cikti} biçimli altın yok; parite ölçülemez`);
          break;
        }
        const cikti = `${r.stdout ?? ""}\n${r.stderr ?? ""}`;
        const eslesen = kervan.filter((k) => k.altin);
        if (kip === "esit") {
          if (eslesen.length === 0) {
            hatalar.push(`${yer}: Kervan'a hiçbir altının girdisi (port/altin/*.json 'girdi') stdin'den gönderilmedi; çıktı altınla karşılaştırılamaz`);
          } else if (r.status !== 0) {
            kendiniDogrulayan = true;
          hatalar.push(`Kendini doğrulayan parite tespit edildi: ${yer} Kervan altını (${eslesen[0].altin}) birebir döndürdüğü hâlde çıkış ${r.status}; karşılaştırıcı farkı ölçmüyor`);
          }
        } else if (kip === "fark") {
          const bozulan = eslesen.map((k) => `${k.altin} altınının tek`).join(", ") || "eşleşen altın yok; uydurma";
          if (r.status === 0) {
            kendiniDogrulayan = true;
          hatalar.push(`Kendini doğrulayan parite tespit edildi: ${yer} Kervan altından farklı çıktı verdiği (${bozulan} alanı nonce) hâlde çıkış 0 ile bitti (Kervan çıktısı karşılaştırmaya bağlı değil)`);
          } else if (!cikti.includes(nonce)) {
            hatalar.push(`${yer} düştü (çıkış ${r.status}) ama fark raporunda Kervan'ın değiştirilmiş alanı (${bozulan}) görünmüyor; düşüşün Kervan çıktısından geldiği kanıtlanamadı`);
          }
        } else if (r.status === 0) {
          kendiniDogrulayan = true;
          hatalar.push(`Kendini doğrulayan parite tespit edildi: ${yer} Kervan çıkış 101 ve boş çıktı verdiği hâlde çıkış 0 ile bitti (Kervan çıkış kodu denetlenmiyor)`);
        }
      }
    }
  } catch (e: any) {
    hatalar.push(`Parite bağlantısı denetlenemedi: ${e.message}`);
  } finally {
    fs.rmSync(gecici, { recursive: true, force: true });
    fs.rmSync(gizli, { recursive: true, force: true });
  }
  return { uygun: hatalar.length === 0, hatalar, kendiniDogrulayan };
}

// ---------------------------------------------------------------- test girdileri

type TestGirdisi =
  | { tur: "ad"; ad: string }
  | { tur: "komut"; komut: string; cargo?: { paket?: string; testHedefi?: string; suzgec?: string }; dosyalar: string[] };

export function testGirdisiniCoz(t: string): TestGirdisi | null {
  if (/^[A-Za-z_][\w]*(::[A-Za-z_][\w]*)*$/.test(t)) return { tur: "ad", ad: t };
  if (!t.startsWith("cargo ") && !t.startsWith("bun ")) return null;
  const parcalar = t.split(/[\s"'`,\[\]{}();]+/).filter(Boolean);
  const dosyalar = parcalar.filter((p) => /\.(?:ts|js|mjs)$/.test(p));
  const i = parcalar.findIndex((p, k) => p === "cargo" && parcalar[k + 1] === "test");
  if (i === -1) return { tur: "komut", komut: t, dosyalar };
  const sonra = (bayrak: string[]) => {
    const k = parcalar.findIndex((p, j) => j > i && bayrak.includes(p));
    return k === -1 ? undefined : parcalar[k + 1];
  };
  const ayrac = parcalar.indexOf("--", i);
  const suzgec = ayrac === -1 ? undefined : parcalar.slice(ayrac + 1).find((p) => !p.startsWith("-"));
  return { tur: "komut", komut: t, dosyalar, cargo: { paket: sonra(["-p", "--package"]), testHedefi: sonra(["--test"]), suzgec } };
}

interface PaketBilgisi {
  ad: string;
  testHedefleri: Set<string>;
  binHedefleri: Set<string>;
}

function cargoPaketleriniAl(kokDizin: string): { paketler: PaketBilgisi[]; hata?: string } {
  const r = spawnSync("cargo", ["metadata", "--no-deps", "--format-version", "1"], {
    cwd: kokDizin,
    encoding: "utf-8",
    maxBuffer: 64 * 1024 * 1024,
    env: cargoOrtami(),
    timeout: 10 * 60 * 1000,
  });
  if (r.error || r.status !== 0) return { paketler: [], hata: `cargo metadata başarısız (çıkış ${r.status ?? r.error?.message})` };
  try {
    const meta = JSON.parse(r.stdout);
    return {
      paketler: (meta.packages ?? []).map((p: any) => ({
        ad: p.name,
        testHedefleri: new Set((p.targets ?? []).filter((t: any) => t.kind?.includes("test")).map((t: any) => t.name)),
        binHedefleri: new Set((p.targets ?? []).filter((t: any) => t.kind?.includes("bin")).map((t: any) => String(t.name).toLowerCase())),
      })),
    };
  } catch (e: any) {
    return { paketler: [], hata: `cargo metadata ayrıştırılamadı: ${e.message}` };
  }
}

/** Her test girdisinin gerçekten var olan bir hedefi gösterdiğini doğrular. */
function testHedefleriniDenetle(mutantlar: MutantTanimi[], kokDizin: string, verilenListe?: Set<string>): string[] {
  const hatalar: string[] = [];
  let liste: Set<string> | undefined = verilenListe;
  let listeHatasi: string | undefined;
  const listeAl = () => {
    if (!liste) {
      const l = cargoTestListesiniAl(kokDizin);
      liste = l.kimlikler;
      listeHatasi = l.hata;
    }
    return liste;
  };
  let paketler: PaketBilgisi[] | undefined;
  let paketHatasi: string | undefined;
  const paketleriAl = () => {
    if (!paketler) {
      const p = cargoPaketleriniAl(kokDizin);
      paketler = p.paketler;
      paketHatasi = p.hata;
    }
    return paketler;
  };

  for (const m of mutantlar) {
    for (const t of m.oldurmesi_gereken_test) {
      const g = testGirdisiniCoz(t);
      const yer = `Mutant '${m.ad}': test '${t}'`;
      if (!g) {
        hatalar.push(`${yer} tanınmıyor (cargo test adı, 'cargo ...' ya da 'bun ...' komutu olmalı)`);
        continue;
      }
      if (g.tur === "ad") {
        if (!listeAl().has(g.ad)) hatalar.push(`${yer} derlenmiş cargo test listesinde yok${listeHatasi ? ` (${listeHatasi})` : ""}`);
        continue;
      }
      for (const d of g.dosyalar) {
        if (!fs.existsSync(path.resolve(kokDizin, d))) hatalar.push(`${yer}: '${d}' dosyası yok`);
      }
      if (g.cargo) {
        const pk = paketleriAl();
        const aday = g.cargo.paket ? pk.filter((p) => p.ad === g.cargo!.paket) : pk;
        if (g.cargo.paket && aday.length === 0) hatalar.push(`${yer}: '${g.cargo.paket}' paketi workspace'te yok${paketHatasi ? ` (${paketHatasi})` : ""}`);
        else if (g.cargo.testHedefi && !aday.some((p) => p.testHedefleri.has(g.cargo!.testHedefi!))) {
          hatalar.push(`${yer}: '${g.cargo.testHedefi}' test hedefi${g.cargo.paket ? ` '${g.cargo.paket}' paketinde` : ""} yok${paketHatasi ? ` (${paketHatasi})` : ""}`);
        }
        const s = g.cargo.suzgec;
        if (s && ![...listeAl()].some((k) => k.includes(s))) {
          hatalar.push(`${yer}: süzgeç '${s}' derlenmiş cargo test listesinde hiçbir testle eşleşmiyor${listeHatasi ? ` (${listeHatasi})` : ""}`);
        }
      } else if (g.dosyalar.length === 0) {
        hatalar.push(`${yer}: komut ne cargo test hedefi ne de var olan bir betik dosyası gösteriyor`);
      }
    }
  }
  return hatalar;
}

// ---------------------------------------------------------------- koşu ve sınıflama

export interface KosuSonucu {
  komut: string;
  stdout: string;
  stderr: string;
  status: number | null;
  signal: string | null;
  zamanAsimi: boolean;
  hata?: string;
  sureMs: number;
}

export type Sinif = "gecti" | "oldu" | "belirsiz";

/** Ham çıktıdan koşunun sonucunu çıkarır. Tanınmayan her durum 'belirsiz'dir. */
export function kosuyuSiniflandir(k: KosuSonucu): { sinif: Sinif; neden: string } {
  if (k.zamanAsimi) return { sinif: "belirsiz", neden: `zaman aşımı (${k.sureMs} ms)` };
  if (k.hata || k.status === null) return { sinif: "belirsiz", neden: `süreç tamamlanamadı (${k.hata ?? k.signal})` };
  const metin = `${k.stdout}\n${k.stderr}`;
  const ok = k.status === 0;

  if (/could not compile|^error\[E\d+\]|no test target named|did not match any packages|^error: package(?:\(s\))? .* not found/m.test(metin)) {
    return { sinif: "belirsiz", neden: "derleme/hedef hatası (test koşmadı)" };
  }

  const cargo = [...metin.matchAll(/^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed/gm)];
  if (cargo.length > 0) {
    if (cargo.some((c) => c[1] === "FAILED")) {
      return ok ? { sinif: "belirsiz", neden: "'test result: FAILED' ama çıkış 0" } : { sinif: "oldu", neden: "cargo: test result: FAILED" };
    }
    const gecen = cargo.reduce((t, c) => t + Number(c[2]), 0);
    if (gecen === 0) return { sinif: "belirsiz", neden: "cargo: 0 test koştu" };
    return ok ? { sinif: "gecti", neden: `cargo: ${gecen} test geçti` } : { sinif: "belirsiz", neden: `testler ok ama çıkış ${k.status}` };
  }

  const bunSay = (re: RegExp) => Number(metin.match(re)?.[1] ?? "0");
  if (/^\s*\d+ pass$/m.test(metin) || /^\s*\d+ fail$/m.test(metin)) {
    const pass = bunSay(/^\s*(\d+) pass$/m), fail = bunSay(/^\s*(\d+) fail$/m), err = bunSay(/^\s*(\d+) errors?$/m);
    if (err > 0) return { sinif: "belirsiz", neden: `bun test: ${err} yakalanmamış hata (yükleme/derleme)` };
    if (fail > 0) return ok ? { sinif: "belirsiz", neden: "bun test düşen test var ama çıkış 0" } : { sinif: "oldu", neden: `bun test: ${fail} test düştü` };
    if (pass > 0) return ok ? { sinif: "gecti", neden: `bun test: ${pass} test geçti` } : { sinif: "belirsiz", neden: `bun test geçti ama çıkış ${k.status}` };
    return { sinif: "belirsiz", neden: "bun test: 0 test koştu" };
  }

  const hukumler = [...metin.matchAll(/^HUKUM: (yesil|kirmizi)\b/gm)];
  if (hukumler.length > 0) {
    const h = hukumler[hukumler.length - 1][1];
    if (h === "kirmizi" && !ok) return { sinif: "oldu", neden: "kapı hükmü: kirmizi" };
    if (h === "yesil" && ok) return { sinif: "gecti", neden: "kapı hükmü: yesil" };
    return { sinif: "belirsiz", neden: `kapı hükmü '${h}' ile çıkış ${k.status} çelişiyor` };
  }

  if (/Blocking waiting for file lock/.test(metin)) return { sinif: "belirsiz", neden: "cargo kilidi beklendi, test çıktısı yok" };
  return { sinif: "belirsiz", neden: `tanınabilir test çıktısı yok (çıkış ${k.status})` };
}

/** Zaman aşımında yalnız kabuk ölür; Windows'ta yetim kalan alt süreç ağacını da öldürür. */
function agaciOldur(pid: number | undefined) {
  // ponytail: yalnız Windows; Linux'ta `sh -c` tek komutu exec ettiği için gerek görülmedi.
  if (!pid || process.platform !== "win32") return;
  const q = spawnSync("powershell", ["-NoProfile", "-Command", `(Get-CimInstance Win32_Process -Filter 'ParentProcessId=${pid}').ProcessId`], { encoding: "utf-8", timeout: 30000 });
  for (const cocuk of (q.stdout ?? "").split(/\s+/).filter(Boolean)) {
    spawnSync("taskkill", ["/F", "/T", "/PID", cocuk], { encoding: "utf-8", timeout: 30000 });
  }
}

export function testiKos(t: string, kokDizin: string, zamanAsimiMs: number): KosuSonucu {
  const g = testGirdisiniCoz(t);
  const bas = Date.now();
  const ortak = { cwd: kokDizin, encoding: "utf-8" as const, env: cargoOrtami(), timeout: zamanAsimiMs, maxBuffer: 256 * 1024 * 1024 };
  let r: SpawnSyncReturns<string>;
  let komut: string;
  if (g?.tur === "ad") {
    const args = ["test", "--workspace", "--all-features", "--", "--exact", g.ad];
    komut = `cargo ${args.join(" ")}`;
    r = spawnSync("cargo", args, ortak);
  } else {
    komut = t; // 'cargo ...' ve 'bun ...' komutları olduğu gibi koşulur
    r = spawnSync(t, { ...ortak, shell: true });
  }
  const zamanAsimi = (r.error as any)?.code === "ETIMEDOUT";
  if (zamanAsimi) agaciOldur(r.pid);
  return {
    komut,
    stdout: r.stdout ?? "",
    stderr: r.stderr ?? "",
    status: r.status,
    signal: r.signal,
    zamanAsimi,
    hata: r.error && !zamanAsimi ? r.error.message : undefined,
    sureMs: Date.now() - bas,
  };
}

function gunlugeYaz(dizin: string, ad: string, k: KosuSonucu, s: { sinif: Sinif; neden: string }, ek: string) {
  const metin = [
    `KOMUT: ${k.komut}`,
    ek,
    `CIKIS: ${k.status} SINYAL: ${k.signal} ZAMAN_ASIMI: ${k.zamanAsimi} SURE_MS: ${k.sureMs}${k.hata ? ` HATA: ${k.hata}` : ""}`,
    `SINIF: ${s.sinif} (${s.neden})`,
    "--- STDOUT ---",
    k.stdout,
    "--- STDERR ---",
    k.stderr,
  ].join("\n");
  fs.writeFileSync(path.join(dizin, `${ad.replace(/[^\w.-]/g, "_")}.log`), metin, "utf-8");
}

export interface MutasyonSecenekleri {
  /** Her koşunun ham çıktısının yazılacağı dizin (zorunlu). */
  gunlukDizini: string;
  /** Komutların koşulacağı ve göreli mutant yollarının çözüleceği kök (varsayılan: cwd). */
  kokDizin?: string;
  /** Geri alma yedeklerinin dizini; içinde eski yedek varsa önceki koşu yarıda kalmıştır. */
  yedekDizini?: string;
  zamanAsimiMs?: number;
  /** Önceden alınmış `cargo test -- --list` kimlikleri (yoksa koşulur). */
  cargoTestleri?: Set<string>;
}

export function mutantlariDene(
  mutantlar: MutantTanimi[],
  secenekler?: MutasyonSecenekleri
): {
  uygun: boolean;
  hatalar: string[];
  oldurulenSayisi: number;
  sonuclar: { ad: string; sinif: Sinif | "yasadi"; neden: string }[];
} {
  const sonuclar: { ad: string; sinif: Sinif | "yasadi"; neden: string }[] = [];
  const kirmizi = (hatalar: string[]) => ({ uygun: false, hatalar, oldurulenSayisi: 0, sonuclar });

  if (!secenekler?.gunlukDizini) return kirmizi(["Günlük dizini verilmedi (--gunluk <dizin>): ham çıktısız mutasyon sonucu kabul edilmez"]);
  if (mutantlar.length === 0) return kirmizi(["Denenecek mutant yok"]);
  const kok = path.resolve(secenekler.kokDizin ?? process.cwd());
  const yedekDizini = secenekler.yedekDizini ?? path.join(os.tmpdir(), "kervan-mutasyon-yedek");
  const zamanAsimiMs = secenekler.zamanAsimiMs ?? VARSAYILAN_ZAMAN_ASIMI_MS;
  fs.mkdirSync(secenekler.gunlukDizini, { recursive: true });
  fs.mkdirSync(yedekDizini, { recursive: true });

  // 0. Önceki koşu yarıda kaldıysa depodaki dosya hâlâ mutasyonlu olabilir.
  const artiklar = fs.readdirSync(yedekDizini).filter((d) => d.endsWith(".yedek"));
  if (artiklar.length > 0) {
    return kirmizi(artiklar.map((d) => {
      const yolDosyasi = path.join(yedekDizini, d.replace(/\.yedek$/, ".yol"));
      const hedef = fs.existsSync(yolDosyasi) ? fs.readFileSync(yolDosyasi, "utf-8") : "?";
      return `Önceki mutasyon koşusu yarıda kalmış: '${hedef}' için yedek ${path.join(yedekDizini, d)} duruyor; dosyayı yedekle karşılaştırıp geri al, sonra yedeği sil`;
    }));
  }

  // 1. Statik denetim: dosya, bul metni, test hedefleri.
  const hatalar: string[] = [];
  const hazir: { m: MutantTanimi; yol: string; orijinal: Buffer; mutasyonlu: Buffer }[] = [];
  for (const m of mutantlar) {
    const yol = path.resolve(kok, m.dosya);
    if (!fs.existsSync(yol)) {
      hatalar.push(`Mutant '${m.ad}': Hedef dosya '${m.dosya}' bulunamadı`);
      continue;
    }
    const orijinal = fs.readFileSync(yol);
    const bul = Buffer.from(m.bul, "utf-8");
    const yer = orijinal.indexOf(bul);
    if (yer === -1) {
      hatalar.push(`Mutant '${m.ad}': Değiştirilecek metin bulunamadı: "${m.bul}"`);
      continue;
    }
    const mutasyonlu = Buffer.concat([orijinal.subarray(0, yer), Buffer.from(m.yerine, "utf-8"), orijinal.subarray(yer + bul.length)]);
    if (mutasyonlu.equals(orijinal)) {
      hatalar.push(`Mutant '${m.ad}': mutasyon dosyayı değiştirmiyor (bul = yerine)`);
      continue;
    }
    hazir.push({ m, yol, orijinal, mutasyonlu });
  }
  hatalar.push(...testHedefleriniDenetle(mutantlar, kok, secenekler.cargoTestleri));
  if (hatalar.length > 0) return kirmizi(hatalar);

  // 2. Taban: her test mutasyonsuz ağaçta yeşil olmalı (aynı komut bir kez koşulur).
  const tabanlar = [...new Set(mutantlar.flatMap((m) => m.oldurmesi_gereken_test))];
  for (const [j, t] of tabanlar.entries()) {
    const k = testiKos(t, kok, zamanAsimiMs);
    const s = kosuyuSiniflandir(k);
    gunlugeYaz(secenekler.gunlukDizini, `taban-${j + 1}`, k, s, `TABAN (mutasyonsuz) — kullanan mutantlar: ${mutantlar.filter((m) => m.oldurmesi_gereken_test.includes(t)).map((m) => m.ad).join(", ")}`);
    if (s.sinif !== "gecti") hatalar.push(`Test '${t}': taban kırmızı (mutasyonsuz koşu yeşil değil: ${s.sinif}, ${s.neden})`);
  }
  if (hatalar.length > 0) return kirmizi(hatalar);

  // 3. Mutasyon: yedekle, uygula, koş, bayt bayt geri al, doğrula.
  let oldurulenSayisi = 0;
  for (const { m, yol, orijinal, mutasyonlu } of hazir) {
    const anahtar = crypto.createHash("sha256").update(yol).digest("hex").slice(0, 16);
    const yedekYolu = path.join(yedekDizini, `${anahtar}.yedek`);
    fs.writeFileSync(yedekYolu, orijinal);
    fs.writeFileSync(path.join(yedekDizini, `${anahtar}.yol`), yol, "utf-8");

    const siniflar: { t: string; sinif: Sinif; neden: string }[] = [];
    try {
      fs.writeFileSync(yol, mutasyonlu);
      for (const [j, t] of m.oldurmesi_gereken_test.entries()) {
        const k = testiKos(t, kok, zamanAsimiMs);
        const s = kosuyuSiniflandir(k);
        gunlugeYaz(secenekler.gunlukDizini, `${m.ad}-mutant-${j + 1}`, k, s, `MUTANT ${m.ad} (${m.dosya}): "${m.bul}" -> "${m.yerine}"`);
        siniflar.push({ t, ...s });
      }
    } finally {
      fs.writeFileSync(yol, orijinal);
    }
    if (!fs.readFileSync(yol).equals(orijinal)) {
      hatalar.push(`Mutant '${m.ad}': '${yol}' GERİ ALINAMADI; yedek ${yedekYolu} korunuyor`);
      return { uygun: false, hatalar, oldurulenSayisi, sonuclar };
    }
    fs.rmSync(yedekYolu, { force: true });
    fs.rmSync(path.join(yedekDizini, `${anahtar}.yol`), { force: true });

    const belirsiz = siniflar.filter((s) => s.sinif === "belirsiz");
    if (belirsiz.length > 0) {
      const neden = belirsiz.map((s) => `'${s.t}': ${s.neden}`).join("; ");
      hatalar.push(`Mutant '${m.ad}' sonucu belirsiz (ölüm sayılmaz): ${neden}`);
      sonuclar.push({ ad: m.ad, sinif: "belirsiz", neden });
    } else if (siniflar.some((s) => s.sinif === "oldu")) {
      oldurulenSayisi++;
      sonuclar.push({ ad: m.ad, sinif: "oldu", neden: siniflar.filter((s) => s.sinif === "oldu").map((s) => `'${s.t}': ${s.neden}`).join("; ") });
    } else {
      hatalar.push(`Mutant '${m.ad}' YAŞADI! Eşleşen testler mutasyona rağmen yeşil kaldı.`);
      sonuclar.push({ ad: m.ad, sinif: "yasadi", neden: "tüm testler geçti" });
    }
  }

  return { uygun: hatalar.length === 0, hatalar, oldurulenSayisi, sonuclar };
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  const arg = (ad: string) => {
    const i = args.indexOf(ad);
    return i === -1 ? undefined : args[i + 1];
  };
  // --gunluk verilmezse ham çıktı yeni bir os.tmpdir() dizinine yazılır ve yolu basılır
  // (kapilar.toml yalnız {faz} yer tutucusu taşıyor); port/kayit'e varsayılan olarak yazılmaz.
  const gunluk = arg("--gunluk") ?? fs.mkdtempSync(path.join(os.tmpdir(), "kervan-mutasyon-gunluk-"));
  console.log(`[mutasyon-kontrol] Ham test çıktısı günlük dizini: ${gunluk}`);
  const mutantlarYolu = arg("--mutantlar") ?? path.join("port", "mutantlar.toml");
  const pariteDizini = arg("--parite") ?? path.join("araclar", "parite");
  console.log("[mutasyon-kontrol] Parite bağlantıları ve mutantlar denetleniyor...");

  const hatalar: string[] = [];

  let mutantlar: MutantTanimi[] = [];
  try {
    mutantlar = mutantlarTomlOku(mutantlarYolu);
    hatalar.push(...mutantKumesiniDenetle(mutantlar));
  } catch (e: any) {
    hatalar.push(e.message);
  }
  hatalar.push(...pariteBaglantisiniDenetle(pariteDizini).hatalar);

  let olduruldu = 0;
  if (hatalar.length === 0) {
    const s = mutantlariDene(mutantlar, { gunlukDizini: gunluk });
    hatalar.push(...s.hatalar);
    olduruldu = s.oldurulenSayisi;
    for (const r of s.sonuclar) console.log(`  [${r.sinif}] ${r.ad}: ${r.neden}`);
  } else {
    console.error("[mutasyon-kontrol] Ön denetim kırmızı; mutantlar koşulmadı.");
  }

  if (hatalar.length === 0) {
    console.log(`✅ [mutasyon-kontrol] Tüm parite betikleri bağlı, ${olduruldu}/${mutantlar.length} mutant öldürüldü. Ham çıktı: ${gunluk}`);
    console.log("HUKUM: yesil");
    process.exit(0);
  }
  console.error("❌ [mutasyon-kontrol] MUTASYON VEYA PARİTE İHLALLERİ BULUNDU:");
  for (const h of hatalar) console.error(`  - ${h}`);
  console.log(`HUKUM: kirmizi ${hatalar[0]}`);
  process.exit(1);
}
