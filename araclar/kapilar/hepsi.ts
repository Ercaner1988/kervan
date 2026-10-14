import { spawn, spawnSync } from "child_process";
import crypto from "crypto";
import fs from "fs";
import os from "os";
import path from "path";
import { defterSatirlariniOku } from "./defter-kontrol.ts";
import { kosucuLogu } from "./rapor-kontrol.ts";

// PRD FR-1.1: Kapı Çalıştırıcısı (hepsi.ts)
// Faz -> kapı listesini YALNIZ araclar/kapilar/kapilar.toml'dan (mühürlü) okur, kapıları sırayla
// koşar, ham çıktıları port/kayit/<faz>/<kapi>.log'a yazar, sonucu port/durum.json'a kaydeder.
// Fail-closed: eksik/bozuk yapılandırma, bilinmeyen faz, boş liste, çökme, zaman aşımı,
// çıkış != 0 ya da stdout'un son satırında geçerli "HUKUM: yesil" olmaması -> KIRMIZI.
// durum.json §6: sayimlar (route.ts + defter) ve fazlar (git geçmişindeki doğrulanmış faz koşuları)
// kapılardan önce ölçülür; ölçülemezse kırmızı. --test-kipi --kapi <ad>: ayrı geçici dizine yazar,
// gerçek port/durum.json ve port/kayit'e dokunmaz, her zaman "HUKUM: kirmizi" ve çıkış 3.

export const KAPILAR_TOML = path.join("araclar", "kapilar", "kapilar.toml");
export const HUKUM_YESIL = "HUKUM: yesil";
/** Koşu kaydı: kapilariKos her koşunun başında kayıt dizinine yazar, her kapıdan sonra günceller. */
export const KOSU_DOSYASI = "kosu.json";
const HUKUM_KIRMIZI = /^HUKUM: kirmizi \S.*$/;

export interface KapiGorevi {
  ad: string;
  komut: string;
  argumanlar?: string[];
  timeoutMs?: number;
}

export interface KapiSonucu {
  ad: string;
  komut: string;
  cikis: number;
  hukum: "yesil" | "kirmizi";
  neden?: string;
  sure_ms: number;
  log: string;
  /** Günlüğün "Zaman:" satırı (rapor-kontrol kosu.json ile çapraz denetler). */
  baslangic: string;
  zaman: string;
  hata?: string;
}

export interface DurumVerisi {
  uretici_ozeti: string;
  kapilar_toml_ozeti?: string;
  uretim_zamani: string;
  faz: string;
  kip: "faz" | "test";
  kapilar: KapiSonucu[];
  fazlar: FazKaydi[];
  sayimlar: Sayimlar | null;
  tum_kapilar_yesil_mi: boolean;
}

export interface FazKaydi {
  id: string;
  commit: string;
  kanit: string[];
}

export interface Sayimlar {
  route_ts: number;
  defter_planlandi: number;
  defter_dogrulandi: number;
}

export interface MakineSakinAyari {
  minBosRamGb: number;
  maxCpuPct: number;
  ornekSn: number;
  kip: "uyari" | "zorunlu";
}

export interface KapilarYapilandirmasi {
  varsayilanZamanAsimiMs: number;
  makineSakin: MakineSakinAyari;
  sayim: { kaynakDizini: string };
  fazlar: Record<string, { ad: string; komut: string; timeoutMs?: number }[]>;
}

// ---------------- Yapılandırma ----------------

export function tomlOku(yol: string): any {
  if (!fs.existsSync(yol)) throw new Error(`yapılandırma dosyası yok: ${yol}`);
  try {
    return Bun.TOML.parse(fs.readFileSync(yol, "utf-8"));
  } catch (e: any) {
    throw new Error(`yapılandırma ayrıştırılamadı (${yol}): ${e?.message ?? e}`);
  }
}

function pozitifSayi(v: unknown, ad: string): number {
  if (typeof v !== "number" || !Number.isFinite(v) || v <= 0) throw new Error(`${ad} pozitif sonlu bir sayı olmalı (değer: ${String(v)})`);
  return v;
}

export function makineSakinAyari(ham: any): MakineSakinAyari {
  const m = ham?.makine_sakin;
  if (!m || typeof m !== "object") throw new Error("[makine_sakin] bölümü yok");
  const minBosRamGb = pozitifSayi(m.min_bos_ram_gb, "makine_sakin.min_bos_ram_gb");
  const maxCpuPct = pozitifSayi(m.max_cpu_pct, "makine_sakin.max_cpu_pct");
  // ponytail: ölçülen CPU %0-100'e kısılır; eşik >= 100 hiçbir zaman aşılamaz -> ölçmeyen kapı.
  if (maxCpuPct >= 100) throw new Error(`makine_sakin.max_cpu_pct < 100 olmalı (değer: ${maxCpuPct}); >= 100 hiç kırmızı veremez`);
  const ornekSn = pozitifSayi(m.ornek_sn, "makine_sakin.ornek_sn");
  if (m.kip !== "uyari" && m.kip !== "zorunlu") throw new Error(`makine_sakin.kip "uyari" ya da "zorunlu" olmalı (değer: ${String(m.kip)})`);
  return { minBosRamGb, maxCpuPct, ornekSn, kip: m.kip };
}

function fazNo(faz: string): number | null {
  const m = /^P(\d+)$/.exec(faz);
  return m ? Number(m[1]) : null;
}

export function kapilarTomlYukle(yol = KAPILAR_TOML): KapilarYapilandirmasi {
  return kapilarTomlCoz(tomlOku(yol));
}

/** Ayrıştırılmış kapilar.toml nesnesini doğrular (fazlariOlc geçmiş commit'lerdeki dosyayı da böyle okur). */
export function kapilarTomlCoz(ham: any): KapilarYapilandirmasi {
  const varsayilanSn = pozitifSayi(ham?.kosucu?.varsayilan_zaman_asimi_sn, "kosucu.varsayilan_zaman_asimi_sn");
  const makineSakin = makineSakinAyari(ham);
  const kaynakDizini = ham?.sayim?.kaynak_dizini;
  if (typeof kaynakDizini !== "string" || !kaynakDizini.trim()) throw new Error("[sayim] kaynak_dizini (route.ts sayılacak anlık görüntü) tanımlı değil");

  const fazHam = ham?.faz;
  if (!fazHam || typeof fazHam !== "object") throw new Error("[faz] bölümü yok");

  const fazlar: KapilarYapilandirmasi["fazlar"] = {};
  for (const [faz, liste] of Object.entries(fazHam)) {
    if (fazNo(faz) === null) throw new Error(`geçersiz faz adı: ${faz} (P<sayı> bekleniyor)`);
    if (!Array.isArray(liste) || liste.length === 0) throw new Error(`faz ${faz}: kapı listesi boş ya da dizi değil`);
    const adlar = new Set<string>();
    fazlar[faz] = liste.map((k: any, i: number) => {
      if (typeof k?.ad !== "string" || !/^[a-z0-9-]+$/.test(k.ad)) throw new Error(`faz ${faz} #${i + 1}: 'ad' [a-z0-9-]+ olmalı`);
      if (typeof k.komut !== "string" || !k.komut.trim()) throw new Error(`faz ${faz} '${k.ad}': 'komut' boş olamaz`);
      if (adlar.has(k.ad)) throw new Error(`faz ${faz}: '${k.ad}' iki kez tanımlı`);
      adlar.add(k.ad);
      const g: { ad: string; komut: string; timeoutMs?: number } = { ad: k.ad, komut: k.komut };
      if (k.zaman_asimi_sn !== undefined) g.timeoutMs = pozitifSayi(k.zaman_asimi_sn, `faz ${faz} '${k.ad}'.zaman_asimi_sn`) * 1000;
      return g;
    });
  }

  // P4 ve sonrası en az P3 listesini (ad + komut şablonu, harfi harfine) kapsamalı.
  const ileri = Object.keys(fazlar).filter((f) => (fazNo(f) as number) >= 4);
  if (ileri.length > 0) {
    if (!fazlar.P3) throw new Error("P4+ tanımlı ama P3 listesi yok; üst küme denetlenemez");
    for (const f of ileri) {
      const var_ = new Set(fazlar[f].map((k) => `${k.ad}\u0000${k.komut}`));
      const eksik = fazlar.P3.filter((k) => !var_.has(`${k.ad}\u0000${k.komut}`)).map((k) => k.ad);
      if (eksik.length) throw new Error(`faz ${f} P3 listesinin üst kümesi değil; eksik/değişmiş: ${eksik.join(", ")}`);
    }
  }

  return { varsayilanZamanAsimiMs: varsayilanSn * 1000, makineSakin, sayim: { kaynakDizini }, fazlar };
}

// ---------------- Koşu ----------------

export function hukumAyristir(stdoutKuyrugu: string): { hukum: "yesil" | "kirmizi"; neden?: string } {
  const satirlar = stdoutKuyrugu.split(/\r?\n/).map((s) => s.trim()).filter(Boolean);
  const son = satirlar[satirlar.length - 1] ?? "";
  if (son === HUKUM_YESIL) return { hukum: "yesil" };
  if (HUKUM_KIRMIZI.test(son)) return { hukum: "kirmizi", neden: son.slice("HUKUM: kirmizi ".length) };
  return { hukum: "kirmizi", neden: `hüküm satırı yok ya da geçersiz (stdout son satırı: ${JSON.stringify(son.slice(0, 200))})` };
}

function surecAgaciniOldur(pid: number | undefined) {
  if (!pid) return;
  try {
    if (process.platform === "win32") spawnSync("taskkill", ["/pid", String(pid), "/T", "/F"], { stdio: "ignore" });
    else process.kill(-pid, "SIGKILL");
  } catch {}
}

export async function tekKapiKos(
  gorev: KapiGorevi,
  logYolu: string,
  varsayilanTimeoutMs = 900000,
  // Günlüğe yazılır; rapor-kontrol yalnız raporun fazındaki "kip: faz" günlüğünü kanıt sayar.
  // Bağlam verilmeyen (elle) koşu test kipidir: hiçbir faz raporunu kanıtlayamaz.
  baglam: { faz: string; kip: "faz" | "test" } = { faz: "?", kip: "test" }
): Promise<KapiSonucu> {
  const baslangic = Date.now();
  const timeoutMs = gorev.timeoutMs || varsayilanTimeoutMs;
  const komutMetni = `${gorev.komut} ${(gorev.argumanlar || []).join(" ")}`.trim();
  const baslangicIso = new Date(baslangic).toISOString();

  fs.mkdirSync(path.dirname(logYolu), { recursive: true });
  const logStream = fs.createWriteStream(logYolu, { encoding: "utf-8" });
  logStream.write(`=== KAPI BAŞLADI: ${gorev.ad} ===\n`);
  logStream.write(`Komut: ${komutMetni}\n`);
  logStream.write(`Zaman: ${baslangicIso}\n`);
  logStream.write(`Faz: ${baglam.faz} (kip: ${baglam.kip})\n\n`);

  return new Promise<KapiSonucu>((resolve) => {
    let bitti = false;
    let zamanAsimiOldu = false;
    let kuyruk = ""; // stdout'un son 16 KB'ı: hüküm satırı için
    const proc = spawn(gorev.komut, gorev.argumanlar || [], {
      shell: true,
      // Kapıya hangi kapının koştuğunu bildirir: durum-kontrol, bu kapı ve sonrakilerin günlüğünü (henüz yok/yazılıyor) aramaz.
      env: { ...process.env, KERVAN_KOSU_KAPI: gorev.ad },
      stdio: ["ignore", "pipe", "pipe"],
      detached: process.platform !== "win32",
    });

    const bitir = (cikis: number, hata?: string) => {
      if (bitti) return;
      bitti = true;
      clearTimeout(timer);
      const sure = Date.now() - baslangic;
      let { hukum, neden } = hukumAyristir(kuyruk);
      if (cikis !== 0) {
        hukum = "kirmizi";
        neden = hata ?? `çıkış kodu ${cikis}`;
      }
      logStream.write(`\n=== KAPI BİTTİ: Çıkış Kodu = ${cikis} (Süre: ${sure} ms) ===\n`);
      logStream.write(`=== KOŞUCU HÜKMÜ: ${hukum}${neden ? " " + neden : ""} ===\n`);
      // Günlük diske inmeden dönme ('finish'): çağıran hemen process.exit yaparsa hüküm satırı kaybolur.
      logStream.end(() => resolve({
        ad: gorev.ad,
        komut: komutMetni,
        cikis,
        hukum,
        neden,
        sure_ms: sure,
        log: logYolu,
        baslangic: baslangicIso,
        zaman: new Date().toISOString(),
        hata: hukum === "kirmizi" ? neden : undefined,
      }));
    };

    const timer = setTimeout(() => {
      zamanAsimiOldu = true;
      logStream.write(`\n❌ [ZAMAN AŞIMI]: Kapı ${timeoutMs} ms sınırını aştı!\n`);
      surecAgaciniOldur(proc.pid);
      // Torun süreç boruyu açık tutarsa 'close' gelmeyebilir; beklemeden kırmızı bitir.
      setTimeout(() => bitir(124, "Zaman aşımı"), 5000).unref?.();
    }, timeoutMs);

    proc.stdout?.on("data", (d: Buffer) => {
      logStream.write(d);
      kuyruk = (kuyruk + d.toString("utf-8")).slice(-16384);
    });
    proc.stderr?.on("data", (d: Buffer) => logStream.write(d));

    proc.on("error", (err) => {
      logStream.write(`\n❌ [HATA]: Süreç başlatılamadı: ${err.message}\n`);
      bitir(127, `süreç başlatılamadı: ${err.message}`);
    });

    proc.on("close", (code, sinyal) => {
      if (zamanAsimiOldu) return bitir(124, "Zaman aşımı");
      bitir(code ?? 1, code === null ? `sinyalle sonlandı: ${sinyal}` : undefined);
    });
  });
}

function dosyaOzeti(yol: string): string {
  try {
    const lf = fs.readFileSync(yol, "utf-8").replace(/\r\n/g, "\n");
    return "sha256:" + crypto.createHash("sha256").update(lf).digest("hex");
  } catch {
    return "okunamadi";
  }
}

const lfOzet = (metin: string) => "sha256:" + crypto.createHash("sha256").update(metin.replace(/\r\n/g, "\n")).digest("hex");

// ---------------- §6 ölçümleri (durum.json.sayimlar / fazlar) ----------------

/** route.ts: anlık görüntüde gerçekten sayılır; defter: port/defter/dosyalar.jsonl (durum-kontrol ile aynı kaynak). */
export function sayimlariOlc(kok: string, kaynakDizini: string): Sayimlar {
  const kaynak = path.resolve(kok, kaynakDizini);
  if (!fs.existsSync(kaynak) || !fs.statSync(kaynak).isDirectory()) throw new Error(`sayım kaynağı yok: ${kaynakDizini}`);
  // Bun.Glob (~0,8 sn); fs.readdirSync({recursive}) Windows'ta aynı ağaçta ~48 sn sürüyor (ölçüldü).
  // dot: true -> nokta dizinleri de sayılır (find ile aynı: 730).
  let route_ts = 0;
  for (const y of new Bun.Glob("**/route.ts").scanSync({ cwd: kaynak, dot: true })) {
    if (!y.split(/[\\/]/).includes("node_modules")) route_ts++;
  }
  if (route_ts === 0) throw new Error(`sayım kaynağında hiç route.ts yok: ${kaynakDizini}`);
  const hatalar: string[] = [];
  const kayitlar = defterSatirlariniOku(path.join(kok, "port", "defter", "dosyalar.jsonl"), hatalar);
  if (hatalar.length) throw new Error(`defter sayılamadı: ${hatalar.join("; ")}`);
  const say = (d: string) => kayitlar.filter((k: any) => k.durum === d).length;
  return { route_ts, defter_planlandi: say("planlandi"), defter_dogrulandi: say("dogrulandi") };
}

function gitCikti(kok: string, args: string[]): string | null {
  const p = spawnSync("git", args, { cwd: kok, encoding: "utf-8", maxBuffer: 64 * 1024 * 1024 });
  return p.status === 0 ? p.stdout : null;
}

/**
 * Tamamlanmış fazlar: HEAD atalarında konusu "P<n>" ile başlayan en yeni commit. O commit'te:
 * port/durum.json o fazın faz-kipi, tüm kapıları yeşil koşusu; özetleri o commit'teki hepsi.ts +
 * kapilar.toml'la tutar; kapı adları (sırasıyla) o commit'teki kapilar.toml'un faz listesidir; her kapının
 * port/kayit/<faz>/<ad>.log günlüğü vardır, koşucu biçimindedir, aynı fazın faz kipinde, toml'daki komutla,
 * çıkış 0 ve yeşil hükümle biter. Ayrıca P1..P(n-1) da aynı biçimde tamamlanmıştır (aradaki boşluktan sonrası sayılmaz).
 * ponytail: günlük ve durum.json düz metindir; biçimi birebir taklit eden, tüm günlükleri de elle yazan sahteci
 * yakalanmaz (imzalı kayıt gerekir). Kazanılan: alan + iki özetle yetinilmez; sahteci toml listesini, her kapının
 * günlüğünü ve önceki fazları da üretmek zorundadır, ve bunların hepsi commit'te denetlenebilir iz bırakır.
 */
export function fazlariOlc(kok: string): FazKaydi[] {
  const log = gitCikti(kok, ["log", "--format=%H%x09%s", "HEAD"]);
  if (log === null) throw new Error("git geçmişi okunamadı (HEAD çözülemedi)");
  const gorulen = new Set<string>();
  const dogrulanan: FazKaydi[] = [];
  for (const satir of log.split("\n")) {
    const [sha, konu] = satir.split("\t");
    const id = /^(P\d+)\b/.exec(konu ?? "")?.[1];
    if (!id || gorulen.has(id)) continue;
    gorulen.add(id);
    const kayit = fazCommitiniDogrula(kok, sha, id);
    if (kayit) dogrulanan.push(kayit);
  }
  dogrulanan.sort((a, b) => Number(a.id.slice(1)) - Number(b.id.slice(1)));
  const fazlar: FazKaydi[] = [];
  for (const f of dogrulanan) {
    if (f.id !== `P${fazlar.length + 1}`) break;
    fazlar.push(f);
  }
  return fazlar;
}

function fazCommitiniDogrula(kok: string, sha: string, id: string): FazKaydi | null {
  const goster = (y: string) => gitCikti(kok, ["show", `${sha}:${y}`]);
  let d: any;
  let liste: { ad: string; komut: string }[] | undefined;
  const kosucu = goster("araclar/kapilar/hepsi.ts");
  const toml = goster(KAPILAR_TOML.replaceAll("\\", "/"));
  if (kosucu === null || toml === null) return null;
  try {
    d = JSON.parse(goster("port/durum.json") ?? "");
    liste = kapilarTomlCoz(Bun.TOML.parse(toml)).fazlar[id];
  } catch {
    return null;
  }
  if (
    !liste || d?.faz !== id || d?.kip !== "faz" || d?.tum_kapilar_yesil_mi !== true ||
    d.uretici_ozeti !== lfOzet(kosucu) || d.kapilar_toml_ozeti !== lfOzet(toml) ||
    !Array.isArray(d.kapilar) || d.kapilar.length !== liste.length
  ) return null;
  for (let i = 0; i < liste.length; i++) {
    const k = d.kapilar[i];
    if (k?.ad !== liste[i].ad || k.cikis !== 0 || k.hukum !== "yesil") return null;
    const metin = goster(`port/kayit/${id}/${liste[i].ad}.log`);
    const g = metin === null ? null : kosucuLogu(metin);
    if (!g || g.cikis !== 0 || g.hukum !== "yesil" || g.faz !== id || g.kip !== "faz" || g.komut !== liste[i].komut.replaceAll("{faz}", id)) return null;
  }
  return { id, commit: sha, kanit: liste.map((k) => k.ad) };
}

export async function kapilariKos(
  kapilar: KapiGorevi[],
  faz = "P1",
  kayitDizini = path.join("port", "kayit", faz),
  durumDosyasi = path.join("port", "durum.json"),
  secenek: {
    kip?: "faz" | "test";
    varsayilanTimeoutMs?: number;
    kapilarToml?: string;
    fazlar?: FazKaydi[];
    sayimlar?: Sayimlar | null;
  } = {}
): Promise<{ sonuclar: KapiSonucu[]; hepsiYesil: boolean }> {
  const sonuclar: KapiSonucu[] = [];
  const kip = secenek.kip ?? "faz";
  // Koşu başında listedeki kapıların eski günlükleri ve eski koşu kaydı silinir: koşu sırasında (örn.
  // rapor-kontrol) görülen <ad>.log yalnız BU koşuda yazılmış olabilir; elle yazılan günlük burada ölür.
  fs.mkdirSync(kayitDizini, { recursive: true });
  const kosuYolu = path.join(kayitDizini, KOSU_DOSYASI);
  for (const kapi of kapilar) fs.rmSync(path.join(kayitDizini, `${kapi.ad}.log`), { force: true });
  const kosu = {
    faz,
    kip,
    baslangic: new Date().toISOString(),
    kapilar: [] as Pick<KapiSonucu, "ad" | "cikis" | "hukum" | "baslangic" | "log">[],
  };
  fs.writeFileSync(kosuYolu, JSON.stringify(kosu, null, 2), "utf-8");
  for (const kapi of kapilar) {
    const s = await tekKapiKos(kapi, path.join(kayitDizini, `${kapi.ad}.log`), secenek.varsayilanTimeoutMs, { faz, kip });
    sonuclar.push(s);
    kosu.kapilar.push({ ad: s.ad, cikis: s.cikis, hukum: s.hukum, baslangic: s.baslangic, log: s.log });
    fs.writeFileSync(kosuYolu, JSON.stringify(kosu, null, 2), "utf-8");
  }

  // Boş liste hiçbir şey ölçmez: yeşil sayılmaz (fail-closed). Test kipi hiçbir zaman faz yeşili değildir.
  const hepsiYesil = kip === "faz" && sonuclar.length > 0 && sonuclar.every((s) => s.cikis === 0 && s.hukum === "yesil");

  const durum: DurumVerisi = {
    uretici_ozeti: dosyaOzeti(import.meta.path),
    kapilar_toml_ozeti: secenek.kapilarToml ? dosyaOzeti(secenek.kapilarToml) : undefined,
    uretim_zamani: new Date().toISOString(),
    faz,
    kip,
    kapilar: sonuclar,
    fazlar: secenek.fazlar ?? [],
    sayimlar: secenek.sayimlar ?? null,
    tum_kapilar_yesil_mi: hepsiYesil,
  };
  fs.mkdirSync(path.dirname(durumDosyasi), { recursive: true });
  fs.writeFileSync(durumDosyasi, JSON.stringify(durum, null, 2), "utf-8");

  return { sonuclar, hepsiYesil };
}

// ---------------- Komut satırı ----------------

export function argumanlariAyristir(args: string[]): { faz: string; testKipi: boolean; secilenler: string[] } {
  let faz: string | undefined;
  let testKipi = false;
  const secilenler: string[] = [];
  const deger = (i: number, bayrak: string) => {
    const v = args[i];
    if (v === undefined || v.startsWith("--")) throw new Error(`${bayrak} bir değer ister (gelen: ${v ?? "hiçbiri"})`);
    return v;
  };
  for (let i = 0; i < args.length; i++) {
    const a = args[i];
    if (a === "--faz") {
      if (faz !== undefined) throw new Error("--faz iki kez verildi");
      faz = deger(++i, "--faz");
    } else if (a === "--kapi") secilenler.push(deger(++i, "--kapi"));
    else if (a === "--test-kipi") testKipi = true;
    else throw new Error(`bilinmeyen argüman: ${a}`);
  }
  if (!faz) throw new Error("--faz zorunlu (örn. --faz P3)");
  if (secilenler.length && !testKipi) throw new Error("--kapi ile liste daraltmak yasak; yalnız --test-kipi ile (gerçek kayda yazılmaz, sonuç hep kırmızı)");
  return { faz, testKipi, secilenler };
}

if (import.meta.main) {
  let faz = "?";
  try {
    const a = argumanlariAyristir(process.argv.slice(2));
    faz = a.faz;
    const cfg = kapilarTomlYukle(KAPILAR_TOML);
    const liste = cfg.fazlar[faz];
    if (!liste) throw new Error(`bilinmeyen faz: ${faz} (${KAPILAR_TOML} içinde tanımlı: ${Object.keys(cfg.fazlar).join(", ")})`);

    let gorevler: KapiGorevi[] = liste.map((k) => ({ ad: k.ad, komut: k.komut.replaceAll("{faz}", faz), timeoutMs: k.timeoutMs }));
    if (a.secilenler.length) {
      const bilinmeyen = a.secilenler.filter((s) => !gorevler.some((g) => g.ad === s));
      if (bilinmeyen.length) throw new Error(`faz ${faz} listesinde olmayan kapı: ${bilinmeyen.join(", ")}`);
      gorevler = gorevler.filter((g) => a.secilenler.includes(g.ad));
    }

    // Test kipi: gerçek port/durum.json ve port/kayit/<faz>/'a DOKUNMAZ (ayrı geçici dizin), sonuç hep kırmızı.
    // Faz kipi: §6 ölçümleri kapılardan ÖNCE alınır; ölçülemezse kapı koşmadan kırmızı.
    let kayit: string | undefined;
    let durumYolu: string | undefined;
    let olcum: { fazlar?: FazKaydi[]; sayimlar?: Sayimlar } = {};
    if (a.testKipi) {
      kayit = fs.mkdtempSync(path.join(os.tmpdir(), `kervan-test-kipi-${faz}-`));
      durumYolu = path.join(kayit, "durum.json");
    } else {
      olcum = { sayimlar: sayimlariOlc(process.cwd(), cfg.sayim.kaynakDizini), fazlar: fazlariOlc(process.cwd()) };
    }

    console.log(`[hepsi.ts] Faz '${faz}' için ${gorevler.length} kapı çalıştırılıyor${a.testKipi ? ` (TEST KİPİ: kayıt ${kayit}; faz sonucu sayılmaz)` : ""}...`);
    const { sonuclar, hepsiYesil } = await kapilariKos(gorevler, faz, kayit, durumYolu, {
      kip: a.testKipi ? "test" : "faz",
      varsayilanTimeoutMs: cfg.varsayilanZamanAsimiMs,
      kapilarToml: KAPILAR_TOML,
      ...olcum,
    });

    for (const s of sonuclar) {
      console.log(`${s.hukum === "yesil" ? "✅" : "❌"} [${s.ad}] Çıkış: ${s.cikis} | Hüküm: ${s.hukum} | Süre: ${s.sure_ms} ms | Log: ${s.log}`);
      if (s.neden) console.log(`   Neden: ${s.neden}`);
    }
    if (a.testKipi) {
      const y = sonuclar.filter((s) => s.hukum === "yesil").length;
      console.log(`HUKUM: kirmizi test kipi: daraltılmış koşu faz sonucu değildir (${y}/${sonuclar.length} kapı yeşil, kayıt ${kayit})`);
      process.exit(3);
    }
    if (hepsiYesil) {
      console.log(`\nTüm kapılar yeşil (${sonuclar.length} kapı).`);
      console.log(HUKUM_YESIL);
      process.exit(0);
    }
    const kirmizilar = sonuclar.filter((s) => s.hukum !== "yesil").map((s) => s.ad);
    console.log(`HUKUM: kirmizi kırmızı kapılar: ${kirmizilar.join(", ")}`);
    process.exit(1);
  } catch (e: any) {
    console.error(`❌ [hepsi.ts] ${e?.message ?? e}`);
    console.log(`HUKUM: kirmizi koşucu başlayamadı (faz ${faz}): ${e?.message ?? e}`);
    process.exit(2);
  }
}
