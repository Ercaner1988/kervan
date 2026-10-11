import crypto from "crypto";
import { spawnSync } from "child_process";
import fs from "fs";
import os from "os";
import path from "path";

// PRD FR-2.4: Mühür kontrolü. Fail-closed: her belirsizlik (eksik/boş/ayrıştırılamayan dosya,
// git hatası, çözülemeyen commit, boş liste) kırmızıdır ve nedeni yazılır.
//  1. Tam mühürlü küme: araclar/kapilar/**, port/altin/**, iki altın üretici; ref/** için tek ağaç özeti.
//     Diskteki küme ile port/muhur.toml listesi birebir eşleşmeli (listede olmayan dosya = kırmızı).
//  2. Mühür commit'i = Ercan'ın SSH anahtarıyla imzaladığı `muhur-onay` etiketinin gösterdiği commit.
//     İmzalı tag nesnesinin başlıkları birebir doğrulanır: 'tag muhur-onay', 'type commit', 'object <commit>'
//     (başka adla imzalanmış etiketi ref'e bağlamak = karışık vekil, kırmızı).
//     Onay muhur.toml'un içinde DEĞİL: builder muhur.toml'u yazabilir ama Ercan'ın özel anahtarıyla
//     imza atamaz. Güven kökü (Ercan'ın genel anahtarı) depo dışında: ~/.config/kervan/muhur-imzacilar
//     (git allowed-signers biçimi). İmza, depo .git/config'i ne derse desin bu dosyaya göre doğrulanır.
//     Mühür commit'i → HEAD arasında mühürlü yola YA DA muhur.toml'a dokunan her commit kırmızıdır
//     (istisna yok: yeniden mühür = Ercan'ın yeni imzalı etiketi). Çalışma ağacındaki muhur.toml,
//     mühür commit'indekiyle aynı olmalı. Aralık rename algılaması olmadan (--no-renames) okunur;
//     grafts/shallow/refs/replace varsa geçmiş güvenilmez (kırmızı), replace nesneleri hiç okunmaz.
//     Listedeki her dosya HEAD ağacında da izlenmeli (izlenmeyen kopya yetmez).
//  3. port/mutantlar.toml yalnız-ekleme: mühür commit'indeki girdiler yerinde ve aynen durmalı.
//  4. muhur.toml'daki onay_bekliyor = false / onay alanı insan içindir; tek başına onay sayılmaz.

export const MUHURLU_DIZINLER = ["araclar/kapilar", "port/altin"];
export const MUHURLU_DOSYALAR = ["araclar/parite/altin-uret.ts", "araclar/parite/strateji-altin-uret.ts"];
export const REF_DIZINI = "ref";
export const MUHUR_YOLU = "port/muhur.toml";
export const MUTANTLAR_YOLU = "port/mutantlar.toml";
export const ONAY_ETIKETI = "refs/tags/muhur-onay";
export const imzacilarYolu = () => path.join(os.homedir(), ".config", "kervan", "muhur-imzacilar");

const sha = (b: Buffer | string) => crypto.createHash("sha256").update(b).digest("hex");
const lf = (s: string) => s.replace(/\r\n/g, "\n");
const OID = /^(?:[0-9a-f]{40}|[0-9a-f]{64})$/;

export function dosyaSha256(dosyaYolu: string): string {
  if (!fs.existsSync(dosyaYolu)) return "";
  // `* text=auto` Windows çıkışında LF'yi CRLF'ye çevirir; özet git'teki (LF) içerik üzerinden alınır.
  const buf = fs.readFileSync(dosyaYolu);
  return sha(Buffer.from(buf.toString("latin1").replace(/\r\n/g, "\n"), "latin1"));
}

export function muhurluMu(yol: string): boolean {
  return MUHURLU_DOSYALAR.includes(yol) || MUHURLU_DIZINLER.some((d) => yol.startsWith(d + "/"));
}

function yuru(kok: string, goreli: string, atla: (ad: string) => boolean, cikti: { dosyalar: string[]; baglar: string[] }) {
  for (const e of fs.readdirSync(path.join(kok, goreli), { withFileTypes: true })) {
    if (atla(e.name)) continue;
    const y = `${goreli}/${e.name}`;
    if (e.isSymbolicLink()) cikti.baglar.push(y);
    else if (e.isDirectory()) yuru(kok, y, atla, cikti);
    else if (e.isFile()) cikti.dosyalar.push(y);
    else cikti.baglar.push(y); // ponytail: soket/aygıt vb. de "dosya değil" sayılır
  }
}

/** Diskteki tam mühürlü küme (ref hariç). Hata listesi boş değilse küme güvenilmezdir. */
export function muhurKumesi(kok: string): { dosyalar: Map<string, string>; hatalar: string[] } {
  const hatalar: string[] = [];
  const dosyalar = new Map<string, string>();
  for (const d of MUHURLU_DIZINLER) {
    if (!fs.existsSync(path.join(kok, d))) continue; // port/altin henüz yoksa boş küme; dizin içeriği listeyle eşlenir
    const c = { dosyalar: [] as string[], baglar: [] as string[] };
    yuru(kok, d, () => false, c);
    for (const b of c.baglar) hatalar.push(`Mühürlü dizinde düz dosya olmayan girdi (bağ vb.): ${b}`);
    for (const y of c.dosyalar) dosyalar.set(y, dosyaSha256(path.join(kok, y)));
  }
  for (const f of MUHURLU_DOSYALAR) {
    const tam = path.join(kok, f);
    if (!fs.existsSync(tam) || !fs.lstatSync(tam).isFile()) hatalar.push(`Zorunlu mühürlü dosya yok: ${f}`);
    else dosyalar.set(f, dosyaSha256(tam));
  }
  if (dosyalar.size === 0) hatalar.push("Mühürlü küme boş: araclar/kapilar altında dosya yok");
  return { dosyalar, hatalar };
}

/** ref/** ağaç özeti: sıralı "yol<TAB>sha" satırlarının sha'sı. ref git'te olmadığı için tek özet. */
export function refOzeti(kok: string): { ozet: string; dosya_sayisi: number } {
  const refKok = path.join(kok, REF_DIZINI);
  if (!fs.existsSync(refKok) || !fs.statSync(refKok).isDirectory()) throw new Error(`ref dizini yok: ${refKok}`);
  const c = { dosyalar: [] as string[], baglar: [] as string[] };
  // ponytail: iç içe .git dizinleri (fetch/gc ile değişen git iç durumu) atlanır; çalışma ağacı özetlenir.
  yuru(kok, REF_DIZINI, (ad) => ad === ".git", c);
  const satirlar: string[] = [];
  for (const y of c.dosyalar) satirlar.push(`${y}\t${sha(fs.readFileSync(path.join(kok, y)))}`);
  for (const y of c.baglar) {
    let hedef: string;
    try {
      hedef = fs.readlinkSync(path.join(kok, y));
    } catch {
      hedef = "?dosya-degil";
    }
    satirlar.push(`${y}\tbag:${hedef}`);
  }
  if (satirlar.length === 0) throw new Error("ref dizini boş");
  satirlar.sort();
  return { ozet: sha(satirlar.join("\n") + "\n"), dosya_sayisi: satirlar.length };
}

// replace nesneleri ve commit-graph önbelleği (sahte ebeveyn taşıyabilir) okumada devre dışı
// (grafts/shallow ayrıca gecmisGuvenilirMi ile kırmızı); çağıranın
// GIT_DIR/GIT_WORK_TREE vb. ortamı denetlenen depodan başka yere bakmasın diye silinir.
const GIT_ENV: Record<string, string> = Object.fromEntries(
  Object.entries(process.env).filter(([k, v]) => v !== undefined && !k.toUpperCase().startsWith("GIT_")) as [string, string][],
);
GIT_ENV.GIT_NO_REPLACE_OBJECTS = "1";
GIT_ENV.GIT_CONFIG_NOSYSTEM = "1";

function git(kok: string, ...args: string[]): { ok: boolean; out: string; err: string } {
  const r = spawnSync("git", ["--no-replace-objects", "-c", "core.quotepath=false", "-c", "core.fsmonitor=false", "-c", "core.commitGraph=false", ...args], {
    cwd: kok,
    encoding: "utf-8",
    timeout: 60000,
    maxBuffer: 64 * 1024 * 1024,
    env: GIT_ENV,
  });
  return { ok: r.status === 0 && !r.error, out: r.stdout ?? "", err: (r.stderr || r.error?.message || "").trim() };
}

/** Commit grafını budayan/yeniden bağlayan düzenekler (grafts, shallow, replace ref'leri) varsa aralık güvenilmez. */
export function gecmisGuvenilirMi(kok: string, hatalar: string[]): boolean {
  const once = hatalar.length;
  const sig = git(kok, "rev-parse", "--is-shallow-repository");
  if (!sig.ok || sig.out.trim() !== "false") hatalar.push(`Sığ (shallow) depo ya da denetlenemedi: commit aralığı budanmış olabilir (${sig.out.trim() || sig.err})`);
  for (const ic of ["info/grafts", "shallow"]) {
    const p = git(kok, "rev-parse", "--git-path", ic);
    if (!p.ok) hatalar.push(`git yolu çözülemedi (${ic}): ${p.err}`);
    else if (fs.existsSync(path.resolve(kok, p.out.trim()))) {
      hatalar.push(`Geçmişi yeniden bağlayan .git/${ic} dosyası var (graft/shallow): commit aralığı güvenilmez`);
    }
  }
  const rep = git(kok, "for-each-ref", "--format=%(refname)", "refs/replace/");
  if (!rep.ok) hatalar.push(`replace ref'leri listelenemedi: ${rep.err}`);
  else if (rep.out.trim()) hatalar.push(`git replace nesneleri var (${rep.out.trim().split("\n").join(", ")}): commit geçmişi sahtelenmiş olabilir`);
  return hatalar.length === once;
}

/** `muhur-onay` etiketini depo dışındaki güven köküyle doğrular; geçerliyse gösterdiği commit'i döndürür. */
export function imzaliMuhurCommiti(kok: string, hatalar: string[]): string {
  const imzacilar = path.resolve(imzacilarYolu());
  const goreli = path.relative(path.resolve(kok), imzacilar);
  if (!goreli.startsWith("..") && !path.isAbsolute(goreli)) {
    hatalar.push(`İmzacılar dosyası depo içinde (${imzacilar}): güven kökü builder'ın yazabildiği yerde olamaz`);
    return "";
  }
  if (!fs.existsSync(imzacilar) || fs.readFileSync(imzacilar, "utf-8").trim() === "") {
    hatalar.push(`İmzacılar dosyası yok ya da boş: ${imzacilar} (Ercan'ın SSH genel anahtarı, allowed-signers biçimi)`);
    return "";
  }
  // Ref bir kez nesne kimliğine çözülür; tüm denetimler aynı nesne üzerinde yapılır.
  const ref = git(kok, "rev-parse", "--verify", "--quiet", ONAY_ETIKETI);
  const nesne = ref.out.trim();
  if (!ref.ok || !OID.test(nesne)) {
    hatalar.push(`Onay etiketi muhur-onay yok: Ercan 'git tag -s muhur-onay' ile imzalamalı`);
    return "";
  }
  const tur = git(kok, "cat-file", "-t", nesne);
  if (!tur.ok || tur.out.trim() !== "tag") {
    hatalar.push(`Onay etiketi muhur-onay açıklamalı değil: Ercan 'git tag -s muhur-onay' ile imzalamalı`);
    return "";
  }
  const govde = git(kok, "cat-file", "tag", nesne);
  if (!govde.ok || !govde.out.includes("-----BEGIN SSH SIGNATURE-----")) {
    hatalar.push("muhur-onay etiketinde SSH imzası yok (imzasız ya da GPG imzalı etiket kabul edilmez)");
    return "";
  }
  // İmza başlıkları da kapsar: 'tag muhur-onay' başlığı, Ercan'ın bu etiketi MÜHÜR ONAYI olarak imzaladığını
  // gösterir. Başka adlı imzalı etiket (ör. faz onayı p5-onay) ref'e bağlanırsa karışık vekil -> kırmızı.
  const bas = govde.out.indexOf("\n\n");
  const basliklar = (bas < 0 ? "" : govde.out.slice(0, bas)).split("\n");
  const deger = (ad: string) => basliklar.filter((b) => b.startsWith(ad + " ")).map((b) => b.slice(ad.length + 1));
  const [obj, typ, ad] = [deger("object"), deger("type"), deger("tag")];
  if (ad.length !== 1 || ad[0] !== "muhur-onay") {
    hatalar.push(`muhur-onay ref'inin gösterdiği imzalı etiketin başlığı 'tag muhur-onay' değil (${JSON.stringify(ad)}): başka amaçla imzalanmış etiket mühür onayı sayılmaz`);
    return "";
  }
  if (typ.length !== 1 || typ[0] !== "commit") {
    hatalar.push(`muhur-onay etiketinin başlığı 'type commit' değil (${JSON.stringify(typ)})`);
    return "";
  }
  if (obj.length !== 1 || !OID.test(obj[0]) || git(kok, "cat-file", "-t", obj[0]).out.trim() !== "commit") {
    hatalar.push(`muhur-onay etiketinin 'object' başlığı geçerli bir commit değil (${JSON.stringify(obj)})`);
    return "";
  }
  // Depo .git/config'i (allowedSignersFile, ssh.program) komut satırı -c ile ezilir.
  // ponytail: ssh-keygen PATH'ten çözülür; PATH de güvenilmezse mutlak yol sabitlenir.
  const v = git(kok, "-c", "gpg.ssh.program=ssh-keygen", "-c", `gpg.ssh.allowedSignersFile=${imzacilar}`, "verify-tag", nesne);
  if (!v.ok) {
    hatalar.push(`muhur-onay etiketinin imzası güven köküyle doğrulanamadı: ${v.err.replace(/\s+/g, " ")}`);
    return "";
  }
  return obj[0];
}

function tomlOku(dosya: string): any {
  return Bun.TOML.parse(fs.readFileSync(dosya, "utf-8"));
}

export function muhurDenetle(kok: string = process.cwd()): { uygun: boolean; hatalar: string[] } {
  const hatalar: string[] = [];
  const bitir = () => ({ uygun: hatalar.length === 0, hatalar });

  // --- muhur.toml ---
  const muhurTam = path.join(kok, MUHUR_YOLU);
  if (!fs.existsSync(muhurTam)) {
    hatalar.push(`${MUHUR_YOLU} yok: mühürsüz depo kırmızıdır`);
    return bitir();
  }
  let muhur: any;
  try {
    muhur = tomlOku(muhurTam);
  } catch (e: any) {
    hatalar.push(`${MUHUR_YOLU} ayrıştırılamadı: ${e.message}`);
    return bitir();
  }

  if (muhur.onay_bekliyor !== false || typeof muhur.onay !== "string" || muhur.onay.trim() === "") {
    hatalar.push(`Mühür Ercan onayı bekliyor: ${MUHUR_YOLU} içinde onay_bekliyor = false ve dolu onay alanı yok`);
  }

  // --- dosya listesi ↔ disk ---
  const liste = new Map<string, string>();
  if (!Array.isArray(muhur.dosya) || muhur.dosya.length === 0) {
    hatalar.push(`${MUHUR_YOLU} içinde [[dosya]] girdisi yok (boş mühür)`);
  } else {
    for (const d of muhur.dosya) {
      const yol = typeof d?.yol === "string" ? d.yol.replace(/\\/g, "/") : "";
      if (!yol || typeof d.sha256 !== "string" || !/^[0-9a-f]{64}$/.test(d.sha256)) {
        hatalar.push(`Geçersiz [[dosya]] girdisi: ${JSON.stringify(d)}`);
      } else if (!muhurluMu(yol)) {
        hatalar.push(`Mühür kapsamı dışında girdi: ${yol}`);
      } else if (liste.has(yol)) {
        hatalar.push(`Yinelenen [[dosya]] girdisi: ${yol}`);
      } else liste.set(yol, d.sha256);
    }
  }
  const disk = muhurKumesi(kok);
  hatalar.push(...disk.hatalar);
  for (const [yol, beklenen] of liste) {
    const gercek = disk.dosyalar.get(yol);
    if (gercek === undefined) hatalar.push(`Mühürlü dosya bulunamadı: ${yol}`);
    else if (gercek !== beklenen) hatalar.push(`MÜHÜR İHLALİ: '${yol}' değiştirilmiş (beklenen ${beklenen}, gerçek ${gercek})`);
  }
  for (const yol of disk.dosyalar.keys()) {
    if (!liste.has(yol)) hatalar.push(`Mühürlü yolda listede olmayan dosya: ${yol}`);
  }

  // --- ref ağaç özeti ---
  if (typeof muhur.ref?.ozet !== "string" || !/^[0-9a-f]{64}$/.test(muhur.ref.ozet)) {
    hatalar.push(`${MUHUR_YOLU} içinde [ref] ozet yok ya da geçersiz`);
  } else {
    try {
      const r = refOzeti(kok);
      if (r.ozet !== muhur.ref.ozet) {
        hatalar.push(`MÜHÜR İHLALİ: ref ağacı değişmiş (beklenen ${muhur.ref.ozet}, gerçek ${r.ozet}, ${r.dosya_sayisi} dosya)`);
      }
    } catch (e: any) {
      hatalar.push(`ref özeti alınamadı: ${e.message}`);
    }
  }

  // --- git: mühür commit'i ve aralık ---
  if (!git(kok, "rev-parse", "--verify", "--quiet", "HEAD^{commit}").ok) {
    hatalar.push("git deposu ya da HEAD yok: commit aralığı ve yalnız-ekleme denetlenemedi");
    return bitir();
  }
  if (!gecmisGuvenilirMi(kok, hatalar)) return bitir();
  // Disk denetimi çalışma ağacına bakar; izlenmeyen kopya HEAD'den silinmiş mühürlü dosyayı örtmesin.
  const agac = git(kok, "ls-tree", "-r", "-z", "--name-only", "HEAD");
  if (!agac.ok) hatalar.push(`HEAD ağacı okunamadı: ${agac.err}`);
  else {
    const headde = new Set(agac.out.split("\0").filter(Boolean));
    for (const yol of liste.keys()) if (!headde.has(yol)) hatalar.push(`Mühürlü dosya HEAD'de izlenmiyor: ${yol}`);
    for (const yol of headde) if (muhurluMu(yol) && !liste.has(yol)) hatalar.push(`HEAD'de listede olmayan mühürlü dosya: ${yol}`);
  }
  const muhurCommit = imzaliMuhurCommiti(kok, hatalar);
  if (!muhurCommit) return bitir();
  if (muhur.muhur_commit !== undefined) {
    // Açık alan yalnız daraltır: imzalı etiketin commit'iyle aynı değilse kırmızı (ör. "HEAD" hilesi).
    const r = typeof muhur.muhur_commit === "string" ? git(kok, "rev-parse", "--verify", "--quiet", `${muhur.muhur_commit}^{commit}`) : null;
    if (!r?.ok) hatalar.push(`muhur_commit çözülemedi: ${JSON.stringify(muhur.muhur_commit)}`);
    else if (r.out.trim() !== muhurCommit) hatalar.push(`muhur_commit (${r.out.trim()}) imzalı onay etiketinin commit'i (${muhurCommit}) değil`);
  }
  const muhurdeki = git(kok, "show", `${muhurCommit}:${MUHUR_YOLU}`);
  if (!muhurdeki.ok) hatalar.push(`İmzalı mühür commit'inde ${MUHUR_YOLU} yok: ${muhurdeki.err}`);
  else if (lf(muhurdeki.out) !== lf(fs.readFileSync(muhurTam, "utf-8"))) {
    hatalar.push(`${MUHUR_YOLU} imzalı mühür commit'indeki sürümden farklı (Ercan imzası olmadan yeniden mühür)`);
  }
  if (!git(kok, "merge-base", "--is-ancestor", muhurCommit, "HEAD").ok) {
    hatalar.push(`Mühür commit'i ${muhurCommit} HEAD'in atası değil`);
    return bitir();
  }

  const commitler = git(kok, "rev-list", `${muhurCommit}..HEAD`);
  if (!commitler.ok) hatalar.push(`git rev-list başarısız: ${commitler.err}`);
  // ponytail: commit başına bir git çağrısı; aralık yüzlerce commit olursa tek `git log --name-only -m` ile toplanır.
  for (const c of commitler.out.split("\n").filter(Boolean)) {
    const d = git(kok, "diff", "--no-renames", "--no-ext-diff", "--name-only", "-z", `${c}^1`, c);
    if (!d.ok) {
      hatalar.push(`git diff başarısız (${c}): ${d.err}`);
      continue;
    }
    // muhur.toml'a dokunmak da ihlaldir: yeniden mühür yalnız Ercan'ın yeni imzalı etiketiyle olur.
    const dokunan = d.out.split("\0").filter((y) => y && (y === MUHUR_YOLU || muhurluMu(y)));
    if (dokunan.length) hatalar.push(`Mühürden sonra mühürlü yola dokunan commit ${c.slice(0, 12)}: ${dokunan.join(", ")}`);
  }

  // --- mutantlar.toml yalnız-ekleme (mühür commit'indeki sürüme göre) ---
  const mutTam = path.join(kok, MUTANTLAR_YOLU);
  if (!fs.existsSync(mutTam)) hatalar.push(`${MUTANTLAR_YOLU} yok: yalnız-ekleme dosyası silinemez`);
  else {
    const eski = git(kok, "show", `${muhurCommit}:${MUTANTLAR_YOLU}`);
    // Fail-closed: ayrıştırma hatası, [[mutant]] tablosunun hiç olmaması (undefined), dizi olmayan ya da
    // boş dizi -> kırmızı. Yalnız iki taraf da dolu diziyse girdi girdi karşılaştırılır.
    const mutantDizisi = (metin: () => string, ad: string): any[] | null => {
      let m: unknown;
      try {
        m = Bun.TOML.parse(metin()).mutant;
      } catch (e: any) {
        hatalar.push(`${ad} ayrıştırılamadı: ${e.message}`);
        return null;
      }
      if (!Array.isArray(m) || m.length === 0) {
        hatalar.push(`${ad} içinde [[mutant]] tablosu yok ya da boş (yalnız-ekleme dosyası boşaltılamaz)`);
        return null;
      }
      return m;
    };
    const simdiki = mutantDizisi(() => fs.readFileSync(mutTam, "utf-8"), MUTANTLAR_YOLU);
    let onceki: any[] | null = null;
    if (!eski.ok) hatalar.push(`Mühür commit'inde ${MUTANTLAR_YOLU} okunamadı: ${eski.err}`);
    else onceki = mutantDizisi(() => eski.out, `Mühür commit'indeki ${MUTANTLAR_YOLU}`);
    if (simdiki && onceki) {
      onceki.forEach((m, i) => {
        if (JSON.stringify(simdiki![i]) !== JSON.stringify(m)) {
          hatalar.push(`${MUTANTLAR_YOLU} yalnız-ekleme ihlali: mutant #${i + 1} (${m?.ad ?? "?"}) silinmiş ya da değişmiş`);
        }
      });
    }
  }

  return bitir();
}

if (import.meta.main) {
  let sonuc: { uygun: boolean; hatalar: string[] };
  try {
    const args = process.argv.slice(2);
    const bilinmeyen: string[] = [];
    for (let i = 0; i < args.length; i++) {
      // --faz yalnız uyumluluk için kabul edilir; mühür her fazda zorunludur.
      if (args[i] === "--faz" && i + 1 < args.length) i++;
      else bilinmeyen.push(args[i]);
    }
    sonuc = bilinmeyen.length
      ? { uygun: false, hatalar: [`Bilinmeyen argüman: ${bilinmeyen.join(" ")} (referans commit argümanla verilemez)`] }
      : muhurDenetle();
  } catch (e: any) {
    sonuc = { uygun: false, hatalar: [`muhur-kontrol çöktü: ${e?.message ?? e}`] };
  }
  for (const h of sonuc.hatalar) console.log(`  - ${h}`);
  if (sonuc.uygun) {
    console.log("HUKUM: yesil");
    process.exit(0);
  }
  console.log(`HUKUM: kirmizi ${sonuc.hatalar.length} ihlal; ilki: ${sonuc.hatalar[0].replace(/\s+/g, " ")}`);
  process.exit(1);
}
