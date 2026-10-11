import { spawnSync } from "child_process";
import crypto from "crypto";
import fs from "fs";
import os from "os";
import path from "path";

// PRD FR-1.2: Temiz Kopya Doğrulayıcısı (temiz-kopya.ts)
// Denetlenen commit'i (--hedef) tek kullanımlık bir git worktree'ye çıkarır, oradaki araclar/kapilar/ dizinini
// ÇAĞIRANIN kapılarıyla DEĞİŞTİRİR ve komutu worktree içinde koşar. Kapı kaynağı:
//   - varsayılan: çağıran deponun --kapi-ref'indeki (varsayılan HEAD) araclar/kapilar'ı, git nesnelerinden
//     (git ls-tree + cat-file) çıkarılır; commit'lenmemiş/mühürsüz çalışma ağacı değişiklikleri KULLANILMAZ.
//   - --kapi-kaynagi <dizin>: açıkça verilen dizin (çağıranın sorumluluğu).
// Denetlenen commit'in kodu kapının hükmünü değiştiremesin diye:
//   - Hedef bunfig.toml/.bunfig.toml taşıyorsa (preload kancası kapıdan önce koşar) ya da Cargo yapılandırmasını
//     (.cargo/config runner, rust-toolchain) çağıranın ref'indekinden farklı taşıyorsa komut HİÇ koşmaz → kırmızı
//     (hedef-yapilandirma).
//   - Koşu BOYUNCA araclar/ (özyineli), .cargo/ ve kökteki korunan adlar izlenir: herhangi bir yazma/taşıma olayı
//     (sonra aslı geri yazılsa bile) süreç ağacını öldürür → kırmızı (kapi-degisti). Ek olarak koşudan sonra kapı
//     betikleri ve yapılandırma yerleştirilenle bayt-özdeş olmalı. Temiz bitiş nonce'lu bildirimle kanıtlanır.
//   - Hedef kökte .env* taşıyorsa (Bun cwd'deki .env, .env.local, .env.<NODE_ENV>'i kendiliğinden yükler) ya da
//     ref/ dizinini (harf duyarsız: Ref/, REF/) izliyorsa (sahte OmniRoute kaynağı) komut koşmaz → kırmızı (hedef-yapilandirma).
//     Çağıranın ref'i yokken worktree'de herhangi bir biçimde ref/ görünürse de komut koşmaz.
// ref/ git'e girmediği için çağıranın ref'i (--ref-dizini, varsayılan <depo>/ref/omniroute-dbe703a) worktree'ye
// <wt>/ref/omniroute-dbe703a bağlantısı (Windows'ta junction) olarak konur: kapılar ref'i ortam değişkeninden değil
// varsayılan yoldan okur (KERVAN_REF_DIZINI komuta geçirilmez). Bağlantı koşu sonunda aynı hedefi göstermiyorsa → kırmızı.
// Çağıranın çalışma dizinine dokunmaz; yalnız kendi geçici worktree'sini siler. Fail-closed: çözülemeyen ref, eksik kapı kaynağı,
// çökme, zaman aşımı → kırmızı.

export const CIKIS = { YESIL: 0, CALISMA_DIZINI: 2, REF_YOK: 3, HAZIRLIK: 4, YAPILANDIRMA: 5, KAPI_DEGISTI: 6, ZAMAN_ASIMI: 124 } as const;
export const VARSAYILAN_ZAMAN_ASIMI_MS = 30 * 60 * 1000;
export const KAPI_DIZINI = "araclar/kapilar";
/** Hedef commit'in değiştirmesine izin verilmeyen çalışma zamanı yapılandırması (komutun nasıl koştuğunu değiştirir). */
export const KORUNAN_YAPILANDIRMA = ["bunfig.toml", ".bunfig.toml", ".cargo/config.toml", ".cargo/config", "rust-toolchain", "rust-toolchain.toml"];
const BUN_YAPILANDIRMASI = ["bunfig.toml", ".bunfig.toml"];

export function gitDurumunuAl(dizin = process.cwd()): string {
  const proc = spawnSync("git", ["status", "--porcelain"], { cwd: dizin, encoding: "utf-8" });
  if (proc.status !== 0) throw new Error(`git status başarısız (${dizin}): ${proc.stderr}`);
  return proc.stdout.trim();
}

export interface TemizKopyaSecenek {
  /** Denetimi yapan depo (git komutları burada koşar). Varsayılan: process.cwd(). */
  depo?: string;
  /** Kapıların alınacağı dizin. Verilmezse kapılar `kapiRef`ten çıkarılır. null = hedefin kendi kapıları (yalnız teşhis). */
  kapiKaynagi?: string | null;
  /** Kapıların (ve korunan yapılandırmanın referansının) çıkarılacağı çağıran commit'i. Varsayılan: HEAD. */
  kapiRef?: string;
  zamanAsimiMs?: number;
  /** OmniRoute ref dizini; worktree'de ref/omniroute-dbe703a olarak bağlanır. Varsayılan: <depo>/ref/omniroute-dbe703a (varsa). */
  refDizini?: string;
}

export interface TemizKopyaSonucu {
  cikis: number;
  stdout: string;
  stderr: string;
  calismaDiziniDegistiMi: boolean;
  geciciWorktreeYolu: string;
  hedefSha: string;
  /** Kapıların geldiği yer: dizin yolu, "git:<sha>" ya da null (hedefin kendi kapıları). */
  kapiKaynagi: string | null;
  hukum: "yesil" | "kirmizi";
  neden: string;
}

// Komutu süreç ağacıyla birlikte öldürebilmek için ara koşucu (spawnSync tek başına torunları öldüremez).
// Koşu SIRASINDA kapı dizinini ve korunan kök adlarını izler: hedefin kodu (cargo test, build.rs, bun test) sonra
// koşacak bir kapıyı sahtesiyle değiştirip koşu bitmeden aslını geri yazsa da yazma/taşıma olayı görülür; ilk olayda
// süreç ağacı öldürülür ve çıkış KAPI_DEGISTI olur. Koşu sonundaki bayt karşılaştırması tek başına bunu göremez.
// Temiz bitişte, çocuklara geçirilmeyen nonce ile "izleyici-temiz" bildirilir; bildirim yoksa (sarmalayıcı dışarıdan
// sonlandırıldı) hüküm kırmızıdır.
const SARMALAYICI = `
const { spawn, spawnSync } = require("child_process");
const fs = require("fs");
const komut = process.env.TK_KOMUT, ms = Number(process.env.TK_ZAMAN_ASIMI_MS), nonce = process.env.TK_NONCE;
let izle = null;
try { izle = JSON.parse(process.env.TK_IZLE || "null"); } catch {}
const env = { ...process.env };
for (const k of ["TK_KOMUT", "TK_ZAMAN_ASIMI_MS", "TK_NONCE", "TK_IZLE"]) delete env[k];
if (!nonce || !Array.isArray(izle) || izle.length === 0) { console.error("[temiz-kopya] izleme yapılandırması yok"); process.exit(1); }
let p = null, asti = false, ihlal = "";
const oldur = () => {
  if (!p || !p.pid) return;
  if (process.platform === "win32") spawnSync("taskkill", ["/PID", String(p.pid), "/T", "/F"]);
  else { try { process.kill(-p.pid, "SIGKILL"); } catch {} }
};
const ihlalEt = (n) => {
  if (ihlal) return;
  ihlal = n;
  console.error("[temiz-kopya] kapi-degisti: koşu sırasında değiştirildi: " + n);
  oldur();
};
const izleyiciler = [];
for (const w of izle) {
  try {
    const desen = (w.adlar || []).map((r) => new RegExp(r, "i"));
    const iz = fs.watch(w.yol, { recursive: !!w.ozyineli }, (tur, ad) => {
      const a = ad == null ? "" : String(ad).replace(/\\\\/g, "/");
      if (desen.length && !desen.some((r) => r.test(a.split("/")[0]))) return;
      // Dizinin kendi zaman damgası (okuma/listeleme) "change" üretir; içerik değişimi dosya olayı olarak ayrıca gelir.
      if (tur === "change" && a) { try { if (fs.statSync(w.yol + "/" + a).isDirectory()) return; } catch {} }
      ihlalEt(w.etiket + (a ? "/" + a : "") + " (" + tur + ")");
    });
    iz.on("error", (e) => ihlalEt(w.etiket + ": izleyici hatası: " + e.message));
    izleyiciler.push(iz);
  } catch (e) { console.error("[temiz-kopya] izleyici kurulamadı (" + w.yol + "): " + e.message); process.exit(1); }
}
p = spawn(komut, { shell: true, stdio: "inherit", env, detached: process.platform !== "win32" });
const t = setTimeout(() => {
  asti = true;
  console.error("[temiz-kopya] zaman aşımı (" + ms + " ms): süreç ağacı öldürülüyor");
  oldur();
}, ms);
p.on("error", (e) => { console.error("[temiz-kopya] komut başlatılamadı: " + e.message); process.exit(1); });
p.on("exit", (c) => {
  clearTimeout(t);
  // Geç teslim edilen izleme olayları için kısa bekleme.
  setTimeout(() => {
    for (const iz of izleyiciler) { try { iz.close(); } catch {} }
    if (asti) process.exit(124);
    if (ihlal) process.exit(6);
    if (c === 0) console.error("[temiz-kopya] izleyici-temiz " + nonce);
    process.exit(c === null ? 1 : c);
  }, 300);
});
`;
/** Kök dizinde izlenen adlar (ilk yol parçası): kapılar, çalışma zamanı yapılandırması, ref bağlantısı, .env. */
const IZLENEN_KOK_ADLARI = ["^araclar$", "^\\.cargo$", "^\\.?bunfig\\.toml$", "^rust-toolchain(\\.toml)?$", "^ref$", "^\\.env"];

/** Dizindeki dosyalar: göreli yol (/) → içerik. */
function dizinOku(dizin: string, on = "", cikti = new Map<string, Buffer>()): Map<string, Buffer> {
  if (!fs.existsSync(dizin)) return cikti;
  for (const g of fs.readdirSync(dizin, { withFileTypes: true })) {
    const rel = on ? `${on}/${g.name}` : g.name;
    if (g.isDirectory()) dizinOku(path.join(dizin, g.name), rel, cikti);
    else if (g.isFile()) cikti.set(rel, fs.readFileSync(path.join(dizin, g.name)));
  }
  return cikti;
}

export function temizKopyadaKos(
  commit = "HEAD",
  komut = "bun araclar/kapilar/hepsi.ts",
  geciciKokDizin?: string,
  secenek: TemizKopyaSecenek = {}
): TemizKopyaSonucu {
  const depo = path.resolve(secenek.depo ?? process.cwd());
  const zamanAsimiMs = secenek.zamanAsimiMs ?? VARSAYILAN_ZAMAN_ASIMI_MS;
  // Worktree depo ağacının DIŞINDA (os.tmpdir) açılır: depoya ve port/kayit'e yazılmaz.
  const geciciKok = path.resolve(geciciKokDizin ?? path.join(os.tmpdir(), "kervan-temiz-kopya"));
  const geciciYol = path.join(geciciKok, `wt_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`);
  const sonuc: TemizKopyaSonucu = {
    cikis: 1, stdout: "", stderr: "", calismaDiziniDegistiMi: false, geciciWorktreeYolu: geciciYol,
    hedefSha: "", kapiKaynagi: null, hukum: "kirmizi", neden: "",
  };
  const kirmizi = (cikis: number, neden: string) => Object.assign(sonuc, { cikis, neden, hukum: "kirmizi" as const });
  const git = (...a: string[]) => spawnSync("git", a, { cwd: depo, encoding: "utf-8" });
  const gitBlob = (sha: string, yol: string): Buffer | null => {
    const p = spawnSync("git", ["cat-file", "blob", `${sha}:${yol}`], { cwd: depo, maxBuffer: 64 * 1024 * 1024 });
    return p.status === 0 && p.stdout ? p.stdout : null;
  };
  const coz = (ref: string) => {
    const rp = git("rev-parse", "--verify", "-q", `${ref}^{commit}`);
    return rp.status === 0 ? rp.stdout.trim() : "";
  };

  // 0. Hedef ve çağıran ref'lerini çöz.
  sonuc.hedefSha = coz(commit);
  if (!sonuc.hedefSha) return kirmizi(CIKIS.REF_YOK, `ref-yok: '${commit}' bir commit'e çözülemedi`);
  const kapiRef = secenek.kapiRef ?? "HEAD";
  const kapiSha = coz(kapiRef);
  if (!kapiSha) return kirmizi(CIKIS.REF_YOK, `ref-yok: kapı ref'i '${kapiRef}' bir commit'e çözülemedi`);

  // 1. Yerleştirilecek kapılar (bellekte; çalışma ağacından değil).
  let kapilar: Map<string, Buffer> | null = null;
  if (secenek.kapiKaynagi === null) sonuc.kapiKaynagi = null;
  else if (secenek.kapiKaynagi !== undefined) {
    const dz = path.resolve(secenek.kapiKaynagi);
    kapilar = fs.existsSync(dz) && fs.statSync(dz).isDirectory() ? dizinOku(dz) : new Map();
    sonuc.kapiKaynagi = dz;
    if (![...kapilar.keys()].some((a) => a.endsWith(".ts"))) return kirmizi(CIKIS.HAZIRLIK, `kapı kaynağı geçersiz (dizin yok ya da .ts yok): ${dz}`);
  } else {
    sonuc.kapiKaynagi = `git:${kapiSha}`;
    const ls = spawnSync("git", ["ls-tree", "-r", "-z", "--name-only", kapiSha, "--", KAPI_DIZINI], { cwd: depo, encoding: "utf-8" });
    const adlar = ls.status === 0 ? ls.stdout.split("\0").filter(Boolean) : [];
    if (!adlar.some((a) => a.endsWith(".ts"))) return kirmizi(CIKIS.HAZIRLIK, `kapı kaynağı geçersiz: ${kapiRef} (${kapiSha}) içinde ${KAPI_DIZINI}/*.ts yok`);
    kapilar = new Map();
    for (const ad of adlar) {
      const b = gitBlob(kapiSha, ad);
      if (!b) return kirmizi(CIKIS.HAZIRLIK, `kapı dosyası git'ten okunamadı: ${kapiSha}:${ad}`);
      kapilar.set(ad.slice(KAPI_DIZINI.length + 1), b);
    }
  }

  // 2. Hedef, komutun nasıl koştuğunu değiştiren yapılandırmayı değiştirmiş mi? (bunfig preload kancası vb.)
  const beklenenYapilandirma = new Map<string, Buffer | null>();
  for (const f of KORUNAN_YAPILANDIRMA) {
    // Bun yapılandırması (preload, koşucu ayarları) kapıların hiçbirine gerekmez: hedefte HİÇ olmamalı.
    const cagiran = BUN_YAPILANDIRMASI.includes(f) ? null : gitBlob(kapiSha, f);
    const hedef = gitBlob(sonuc.hedefSha, f);
    if ((cagiran === null) !== (hedef === null) || (cagiran && hedef && !cagiran.equals(hedef))) {
      return kirmizi(
        CIKIS.YAPILANDIRMA,
        BUN_YAPILANDIRMASI.includes(f)
          ? `hedef-yapilandirma: denetlenen commit '${f}' taşıyor; bun bunu (preload vb.) kapıdan önce yükler, hedefin kodu hükmü değiştirebilir; komut koşturulmadı`
          : `hedef-yapilandirma: denetlenen commit '${f}' dosyasını çağıranın ${kapiRef} sürümünden farklı taşıyor (hedefte ${hedef ? "var" : "yok"} / çağıranda ${cagiran ? "var" : "yok"}); hedefin kodu kapıdan önce koşabilir, komut koşturulmadı`
      );
    }
    beklenenYapilandirma.set(f, cagiran);
  }

  // Kök dizindeki .env* ve izlenen ref/: komuttan önce kodu/ortamı değiştirir.
  const kokAdlari = git("ls-tree", "--name-only", "-z", sonuc.hedefSha);
  if (kokAdlari.status !== 0) return kirmizi(CIKIS.HAZIRLIK, `hedef ağacı listelenemedi: ${kokAdlari.stderr.trim()}`);
  for (const ad of kokAdlari.stdout.split("\0").filter(Boolean)) {
    if (/^.env/i.test(ad)) return kirmizi(CIKIS.YAPILANDIRMA, `hedef-yapilandirma: denetlenen commit kökte '${ad}' taşıyor; bun bunu kendiliğinden yükler (ör. KERVAN_REF_DIZINI), komut koşturulmadı`);
    // Büyük/küçük harf duyarsız: Windows/macOS'ta (core.ignorecase) `Ref/` diskte `ref/` olarak okunur.
    if (/^ref$/i.test(ad)) return kirmizi(CIKIS.YAPILANDIRMA, `hedef-yapilandirma: denetlenen commit ref/ dizinini izliyor (.gitignore'da); sahte OmniRoute kaynağı kapılara ölçtürülemez, komut koşturulmadı`);
  }

  let baslangicDurumu: string;
  try {
    baslangicDurumu = gitDurumunuAl(depo);
  } catch (e: any) {
    return kirmizi(CIKIS.HAZIRLIK, e.message);
  }

  fs.mkdirSync(geciciKok, { recursive: true });
  const add = git("worktree", "add", "--detach", geciciYol, sonuc.hedefSha);
  if (add.status !== 0) return kirmizi(CIKIS.HAZIRLIK, `git worktree add başarısız: ${add.stderr.trim()}`);

  const refBag = path.join(geciciYol, "ref", "omniroute-dbe703a");
  let refHedefi: string | null = null;
  try {
    // 3. Hedefin kapılarını çağıranınkiyle değiştir.
    const hedefKapi = path.join(geciciYol, ...KAPI_DIZINI.split("/"));
    if (kapilar !== null) {
      fs.rmSync(hedefKapi, { recursive: true, force: true });
      for (const [rel, b] of kapilar) {
        const y = path.join(hedefKapi, ...rel.split("/"));
        fs.mkdirSync(path.dirname(y), { recursive: true });
        fs.writeFileSync(y, b);
      }
    }

    // 4. Komutu worktree'de, süre sınırıyla koş.
    const refDizini = path.resolve(secenek.refDizini ?? path.join(depo, "ref", "omniroute-dbe703a"));
    if (fs.existsSync(refDizini)) {
      fs.mkdirSync(path.dirname(refBag), { recursive: true });
      fs.symlinkSync(refDizini, refBag, "junction");
      refHedefi = fs.realpathSync(refDizini);
    } else {
      // Çağıranın ref'i yok: kapılar ref'i göremez olmalı. Worktree'de herhangi bir harf biçimiyle ref/ varsa (hedefin
      // getirdiği), kapı sahte kaynağı ölçer → koşturma.
      const sahte = fs.readdirSync(geciciYol).find((a) => /^ref$/i.test(a));
      if (sahte) {
        kirmizi(CIKIS.YAPILANDIRMA, `hedef-yapilandirma: çağıranın ref dizini yok (${refDizini}) ama denetlenen ağaçta '${sahte}/' var; sahte OmniRoute kaynağı kapılara ölçtürülemez, komut koşturulmadı`);
        return sonuc;
      }
    }
    // İzleme: kapı dizini (özyineli), kök dizindeki korunan adlar, .cargo (varsa). Kapılar bu dizine yazmaz.
    const izle: { yol: string; ozyineli: boolean; etiket: string; adlar?: string[] }[] = [{ yol: geciciYol, ozyineli: false, etiket: ".", adlar: IZLENEN_KOK_ADLARI }];
    const araclarDizini = path.join(geciciYol, "araclar");
    if (fs.existsSync(araclarDizini)) izle.push({ yol: araclarDizini, ozyineli: true, etiket: "araclar" });
    const cargoDizini = path.join(geciciYol, ".cargo");
    if (fs.existsSync(cargoDizini)) izle.push({ yol: cargoDizini, ozyineli: true, etiket: ".cargo" });
    const nonce = crypto.randomBytes(16).toString("hex");
    const env: Record<string, string | undefined> = {
      ...process.env, TK_KOMUT: komut, TK_ZAMAN_ASIMI_MS: String(zamanAsimiMs), TK_NONCE: nonce, TK_IZLE: JSON.stringify(izle),
      // Bun'ın içerik özetiyle anahtarlanan genel aktarım önbelleği, hedefin kodunun kapı betiğine zehirli çıktı koymasına açıktır.
      BUN_RUNTIME_TRANSPILER_CACHE_PATH: "0",
    };
    delete env.KERVAN_REF_DIZINI;
    const p = spawnSync(process.execPath, ["--no-env-file", "-e", SARMALAYICI], {
      cwd: geciciYol, env, encoding: "utf-8", maxBuffer: 64 * 1024 * 1024, timeout: zamanAsimiMs + 30_000,
    });
    sonuc.stdout = p.stdout || "";
    const temizSatiri = `[temiz-kopya] izleyici-temiz ${nonce}`;
    const hamStderr = p.stderr || "";
    const stderrSatirlari = hamStderr.split(/\r?\n/);
    const izleyiciTemiz = stderrSatirlari.includes(temizSatiri);
    sonuc.stderr = stderrSatirlari.filter((x) => x !== temizSatiri).join("\n");
    const izlemeIhlali = /\[temiz-kopya\] kapi-degisti: (.*)/.exec(hamStderr)?.[1] ?? (p.status === CIKIS.KAPI_DEGISTI ? "sarmalayıcı kapi-degisti bildirdi" : "");

    // 5. Koşu sırasında kapılar ya da korunan yapılandırma değişti mi? (hedefin kodu kapıyı yeniden yazmış olabilir)
    const degisen: string[] = [];
    if (kapilar !== null) {
      const simdi = dizinOku(hedefKapi);
      for (const [rel, b] of kapilar) if (!simdi.get(rel)?.equals(b)) degisen.push(`${KAPI_DIZINI}/${rel}`);
      for (const rel of simdi.keys()) if (!kapilar.has(rel)) degisen.push(`${KAPI_DIZINI}/${rel} (eklendi)`);
    }
    if (refHedefi !== null) {
      let simdi = "";
      try {
        simdi = fs.lstatSync(refBag).isSymbolicLink() ? fs.realpathSync(refBag) : "";
      } catch {}
      if (simdi !== refHedefi) degisen.push("ref/omniroute-dbe703a (bağlantı değişti)");
    }
    for (const [f, b] of beklenenYapilandirma) {
      const y = path.join(geciciYol, ...f.split("/"));
      const simdi = fs.existsSync(y) ? fs.readFileSync(y) : null;
      if ((simdi === null) !== (b === null) || (simdi && b && !simdi.equals(b))) degisen.push(f);
    }

    if (p.error) kirmizi(CIKIS.ZAMAN_ASIMI, `komut koşturulamadı ya da yedek süre aşıldı: ${p.error.message}`);
    else if (p.status === CIKIS.ZAMAN_ASIMI) kirmizi(CIKIS.ZAMAN_ASIMI, `zaman aşımı: komut ${zamanAsimiMs} ms içinde bitmedi`);
    else if (izlemeIhlali) kirmizi(CIKIS.KAPI_DEGISTI, `kapi-degisti: ${izlemeIhlali}`);
    else if (degisen.length) kirmizi(CIKIS.KAPI_DEGISTI, `kapi-degisti: koşu sırasında değiştirildi: ${degisen.slice(0, 5).join(", ")}${degisen.length > 5 ? ` (+${degisen.length - 5})` : ""}`);
    else if (p.status === null) kirmizi(1, `komut sinyalle sonlandı (${p.signal})`);
    else if (p.status !== 0) kirmizi(p.status, `komut çıkış kodu ${p.status}`);
    else if (!izleyiciTemiz) kirmizi(1, "izleyici temiz bitiş bildirmedi: sarmalayıcı dışarıdan sonlandırılmış olabilir");
    else Object.assign(sonuc, { cikis: 0, hukum: "yesil", neden: "" });
  } catch (e: any) {
    kirmizi(CIKIS.HAZIRLIK, `temiz kopya hazırlanamadı: ${e?.message ?? e}`);
  } finally {
    // 6. Yalnız kendi worktree'mizi kaldır. Önce fs.rmSync: bağlantıları (ref junction'ı, hedefin koyduğu bağlar)
    // izlemeden siler; çağıranın ref'i ve bağların gösterdiği başka yerler silinmez. Sonra git kaydı temizlenir.
    try {
      fs.rmSync(geciciYol, { recursive: true, force: true });
    } catch {}
    git("worktree", "remove", "--force", geciciYol);
    git("worktree", "prune");
  }

  // 7. Çalışma dizini dokunulmamış olmalı.
  try {
    sonuc.calismaDiziniDegistiMi = gitDurumunuAl(depo) !== baslangicDurumu;
  } catch (e: any) {
    sonuc.calismaDiziniDegistiMi = true;
  }
  if (sonuc.calismaDiziniDegistiMi && sonuc.cikis === 0) {
    kirmizi(CIKIS.CALISMA_DIZINI, "çalışma dizininin git durumu koşu sırasında değişti");
  }
  return sonuc;
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  let commit = "HEAD";
  let komut = "bun araclar/kapilar/hepsi.ts";
  let geciciKok: string | undefined;
  const secenek: TemizKopyaSecenek = {};
  const deger = (i: number) => {
    if (i >= args.length) {
      console.log(`HUKUM: kirmizi ${args[i - 1]} değer bekliyor`);
      process.exit(CIKIS.HAZIRLIK);
    }
    return args[i];
  };
  for (let i = 0; i < args.length; i++) {
    const a = args[i];
    if (a === "--komut") komut = deger(++i);
    else if (a === "--hedef") commit = deger(++i);
    else if (a === "--kapi-kaynagi") secenek.kapiKaynagi = path.resolve(deger(++i));
    else if (a === "--kapi-ref") secenek.kapiRef = deger(++i);
    else if (a === "--depo") secenek.depo = path.resolve(deger(++i));
    else if (a === "--ref-dizini") secenek.refDizini = path.resolve(deger(++i));
    else if (a === "--gecici-kok") geciciKok = path.resolve(deger(++i));
    else if (a === "--zaman-asimi-ms") {
      const n = Number(deger(++i));
      if (!Number.isInteger(n) || n <= 0) {
        console.log(`HUKUM: kirmizi --zaman-asimi-ms pozitif tamsayı olmalı`);
        process.exit(CIKIS.HAZIRLIK);
      }
      secenek.zamanAsimiMs = n;
    } else if (!a.startsWith("--")) commit = a;
    else {
      console.log(`HUKUM: kirmizi bilinmeyen argüman: ${a}`);
      process.exit(CIKIS.HAZIRLIK);
    }
  }

  const s = temizKopyadaKos(commit, komut, geciciKok, secenek);
  console.log(`[temiz-kopya] hedef: ${commit} → ${s.hedefSha || "?"}`);
  console.log(`[temiz-kopya] kapı kaynağı: ${s.kapiKaynagi ?? "(hedefin kendi kapıları)"}`);
  console.log(`[temiz-kopya] komut: ${komut}`);
  if (s.stdout) console.log(s.stdout.trimEnd());
  if (s.stderr) console.log(s.stderr.trimEnd());
  console.log(`[temiz-kopya] çıkış: ${s.cikis}`);
  console.log(s.hukum === "yesil" ? "HUKUM: yesil" : `HUKUM: kirmizi ${s.neden.replace(/\s+/g, " ")}`);
  process.exit(s.cikis);
}
