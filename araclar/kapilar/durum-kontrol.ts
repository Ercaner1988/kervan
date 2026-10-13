import { spawnSync } from "child_process";
import crypto from "crypto";
import fs from "fs";
import path from "path";
import { defterSatirlariniOku } from "./defter-kontrol.ts";
import { BLOK_BASLA, BLOK_BITIS, DurumJson, README_DOSYALARI, durumJsonOku, durumMarkdownUret, readmeBlokBul, readmeBlokUret } from "./durum-uret.ts";

// PRD FR-2.7: Üretilmiş Durum Kontrolü (durum-kontrol.ts). Kendi içindeki özete güvenmez; ölçer:
// 1. port/durum.json §6 şemasında; uretici_ozeti = "sha256:" + sha256(araclar/kapilar/hepsi.ts, CRLF→LF).
//    İddialar ÖLÇÜLÜR: her kapının log'u port/kayit/ altında VAR, hepsi.ts biçiminde başlar/biter, log'daki çıkış
//    kodu ve koşucu hükmü durum.json'la aynı; sayimlar.route_ts = ref'teki route.ts sayısı; sayimlar.defter_* =
//    port/defter/{dosyalar,davranislar}.jsonl'de sayılan. kip = "faz"; kapilar[].ad, kapilar.toml'da d.faz
//    listesiyle sıra dahil aynı (kapilar_toml_ozeti de tutmalı); fazlar[].commit git'te çözülür ve HEAD'in atasıdır. Not: durum dosyaları elle de yazılabilir; "yeşil"
//    iddiasının asıl kanıtı temiz-kopya ile yeniden koşudur (PRD §8). Bu kapı yalnız tutarlılığı ölçer.
// 2. port/DURUM.md, durum-uret ile durum.json + uclar.toml'dan YENİDEN üretilen çıktıyla bayt-özdeş
//    (yalnız CRLF→LF denkleştirilir; .gitattributes text=auto).
// 3. Var olan her README*.md durum bloğunu ZORUNLU taşır ve blok üretilenle bayt-özdeş; README.md'de
//    "Sağlayıcı Durumu" bölümü yalnız bu bloktan oluşur (elle durum tablosu kırmızı). Bozuk işaret kırmızı.
// 4. README*.md mühürlü taban başlık listesini taşır (silinmiş bölüm ya da dil sürümü kırmızı).
// 5. Durum bloğu dışında uclar.toml'da "gercek" olmayan uca "destekleniyor/supported/✅" diyen satır kırmızı.

const lf = (s: string) => s.replace(/\r\n/g, "\n");
const sha256 = (b: string | Buffer) => crypto.createHash("sha256").update(b).digest("hex");
const dosyaMi = (y: string) => fs.existsSync(y) && fs.statSync(y).isFile();
const SAGLAYICI_BOLUMU = "Sağlayıcı Durumu";

/** OmniRoute ref ağacındaki route.ts dosyaları (node_modules ve gizli dizinler hariç). */
export function routeTsSay(dizin: string): number {
  let n = 0;
  for (const g of fs.readdirSync(dizin, { withFileTypes: true })) {
    if (g.name === "node_modules") continue; // hepsi.ts sayimlariOlc ile aynı: nokta dizinleri de sayılır
    if (g.isDirectory()) n += routeTsSay(path.join(dizin, g.name));
    else if (g.isFile() && g.name === "route.ts") n++;
  }
  return n;
}

/** Kapı log'u hepsi.ts biçiminde mi ve durum.json satırıyla aynı sonucu mu söylüyor? null = uygun. */
export function kapiLoguDenetle(k: { ad: string; cikis: number; log: string; hukum?: string }, kok: string): string | null {
  const y = String(k.log).replace(/\\/g, "/");
  const tam = path.resolve(kok, y);
  const rel = path.relative(kok, tam).replace(/\\/g, "/");
  if (path.isAbsolute(y) || !rel.startsWith("port/kayit/")) return `kapı '${k.ad}': log '${k.log}' port/kayit/ altında göreli bir yol değil`;
  if (!dosyaMi(tam)) return `kapı '${k.ad}': log dosyası yok: ${k.log} (durum.json bir koşunun çıktısı değil)`;
  const satirlar = lf(fs.readFileSync(tam, "utf-8")).split("\n").filter((x) => x.trim());
  if (satirlar[0] !== `=== KAPI BAŞLADI: ${k.ad} ===`) return `kapı '${k.ad}': log başlığı bu kapının değil (${JSON.stringify(satirlar[0] ?? "<boş>")}): ${k.log}`;
  const bitti = /^=== KAPI BİTTİ: Çıkış Kodu = (-?\d+) \(Süre: \d+ ms\) ===$/.exec(satirlar[satirlar.length - 2] ?? "");
  const hukum = /^=== KOŞUCU HÜKMÜ: (yesil|kirmizi)(?: .*)? ===$/.exec(satirlar[satirlar.length - 1] ?? "");
  if (!bitti || !hukum) return `kapı '${k.ad}': log tamamlanmamış ya da ayrıştırılamıyor (son iki satır KAPI BİTTİ / KOŞUCU HÜKMÜ değil): ${k.log}`;
  if (Number(bitti[1]) !== k.cikis) return `kapı '${k.ad}': durum.json çıkış ${k.cikis} diyor, log'da ${bitti[1]}: ${k.log}`;
  if ((hukum[1] === "yesil") !== (k.cikis === 0)) return `kapı '${k.ad}': çıkış ${k.cikis} ama log'daki koşucu hükmü '${hukum[1]}': ${k.log}`;
  if (k.hukum !== undefined && k.hukum !== hukum[1]) return `kapı '${k.ad}': durum.json hükmü '${k.hukum}', log'da '${hukum[1]}'`;
  return null;
}

export function durumMdKontrol(
  durumMdYolu = path.join("port", "DURUM.md"),
  kokDizin = process.cwd()
): { uygun: boolean; hata?: string } {
  if (!fs.existsSync(durumMdYolu)) return { uygun: false, hata: `port/DURUM.md bulunamadı: ${durumMdYolu}` };
  const mevcut = lf(fs.readFileSync(durumMdYolu, "utf-8"));

  // Hızlı ön denetim (eski kural korunur): gömülü özet gövdeyle tutmalı.
  const m = mevcut.match(/^<!-- URETILMIS_DURUM_HASH: ([a-f0-9]+) -->\n/);
  if (!m) return { uygun: false, hata: `port/DURUM.md üretim başlığı (URETILMIS_DURUM_HASH) içermiyor; elle yazılmış!` };
  if (sha256(mevcut.slice(m[0].length)) !== m[1]) {
    return { uygun: false, hata: `port/DURUM.md elle düzenlenmiş (hash uyuşmazlığı)!` };
  }

  // Asıl denetim: kaynaktan yeniden üret, karşılaştır.
  let beklenen: string;
  try {
    beklenen = durumMarkdownUret(path.join(kokDizin, "port", "durum.json"), path.join(kokDizin, "port", "uclar.toml")).markdown;
  } catch (e: any) {
    return { uygun: false, hata: `port/DURUM.md yeniden üretilemedi: ${e.message}` };
  }
  if (mevcut !== beklenen) {
    const a = mevcut.split("\n"), b = beklenen.split("\n");
    let i = 0;
    while (i < a.length && a[i] === b[i]) i++;
    return {
      uygun: false,
      hata: `port/DURUM.md elle düzenlenmiş: durum-uret çıktısıyla bayt-özdeş değil (ilk fark satır ${i + 1}: dosya ${JSON.stringify(a[i] ?? "<son>")} / üretilen ${JSON.stringify(b[i] ?? "<son>")})`,
    };
  }
  return { uygun: true };
}

/**
 * durum.json bir FAZ koşusunun TAM çıktısı mı? kip "faz"; kapilar_toml_ozeti kökteki kapilar.toml'un özeti;
 * kapilar[].ad, kapilar.toml'da d.faz için tanımlı listeyle sıra dahil AYNI (kırmızı kapıyı listeden çıkarmak geçmez).
 */
export function kapiListesiKontrol(d: DurumJson, kok: string): string[] {
  const h: string[] = [];
  if (d.kip !== "faz") h.push(`durum.json kip='${d.kip}': faz koşusu değil (test kipi koşusu faz durumu iddia edemez)`);
  const tomlYolu = path.join(kok, "araclar", "kapilar", "kapilar.toml");
  if (!dosyaMi(tomlYolu)) return [...h, `kapı listesi doğrulanamadı: ${tomlYolu} yok`];
  const icerik = lf(fs.readFileSync(tomlYolu, "utf-8"));
  const ozet = "sha256:" + sha256(icerik);
  if (d.kapilar_toml_ozeti !== ozet) h.push(`durum.json kapilar_toml_ozeti (${d.kapilar_toml_ozeti}) araclar/kapilar/kapilar.toml özetiyle (${ozet}) eşleşmiyor`);
  let liste: unknown;
  try {
    liste = (globalThis as any).Bun.TOML.parse(icerik)?.faz?.[d.faz];
  } catch (e: any) {
    return [...h, `kapilar.toml ayrıştırılamadı: ${e.message}`];
  }
  if (!Array.isArray(liste) || liste.length === 0) return [...h, `kapilar.toml'da faz '${d.faz}' için kapı listesi yok`];
  const beklenen = liste.map((k: any) => String(k?.ad));
  const gercek = d.kapilar.map((k) => k.ad);
  if (beklenen.join("\u0000") !== gercek.join("\u0000")) {
    const eksik = beklenen.filter((a) => !gercek.includes(a));
    const fazla = gercek.filter((a) => !beklenen.includes(a));
    h.push(
      `durum.json kapilar listesi kapilar.toml faz ${d.faz} listesiyle aynı değil (sıra dahil): beklenen [${beklenen.join(", ")}], durum.json [${gercek.join(", ")}]` +
        (eksik.length ? `; eksik: ${eksik.join(", ")}` : "") + (fazla.length ? `; fazla: ${fazla.join(", ")}` : "")
    );
  }
  return h;
}

/** fazlar[].commit git'te bir commit'e çözülmeli ve HEAD'in atası olmalı; çözülemeyen ref kırmızı. */
export function fazCommitKontrol(d: DurumJson, kok: string): string[] {
  const h: string[] = [];
  const git = (...a: string[]) => spawnSync("git", a, { cwd: kok, encoding: "utf-8" });
  for (const f of d.fazlar) {
    const rp = git("rev-parse", "--verify", "-q", `${f.commit}^{commit}`);
    if (rp.status !== 0) {
      h.push(`fazlar ${f.id}: commit '${f.commit}' git'te bir commit'e çözülemedi`);
      continue;
    }
    if (git("merge-base", "--is-ancestor", rp.stdout.trim(), "HEAD").status !== 0) h.push(`fazlar ${f.id}: commit '${f.commit}' HEAD'in atası değil`);
  }
  return h;
}

/** durum.json'un ölçtüğü şeylerle tutarlılığı (şema + üretici özeti + kapı listesi + faz commit'leri + defter sayımı). */
export function durumJsonKontrol(kokDizin = process.cwd(), refDizinArg?: string): string[] {
  let d;
  try {
    d = durumJsonOku(path.join(kokDizin, "port", "durum.json"));
  } catch (e: any) {
    return [e.message];
  }
  const h: string[] = [];
  const hepsi = path.join(kokDizin, "araclar", "kapilar", "hepsi.ts");
  if (!fs.existsSync(hepsi)) h.push(`uretici_ozeti doğrulanamadı: ${hepsi} yok`);
  else {
    const gercek = "sha256:" + sha256(lf(fs.readFileSync(hepsi, "utf-8"))); // hepsi.ts dosyaOzeti ile aynı: CRLF→LF
    if (d.uretici_ozeti !== gercek) h.push(`durum.json uretici_ozeti (${d.uretici_ozeti}) araclar/kapilar/hepsi.ts özetiyle (${gercek}) eşleşmiyor; durum.json bu kapı çalıştırıcısının çıktısı değil`);
  }
  // Koşu içinde (hepsi.ts KERVAN_KOSU_KAPI verir) durum.json bir önceki koşunundur; koşan kapının ve sonrakilerin günlüğü
  // yazılıyor ya da silinmiş olur. Yalnız ondan ÖNCEKİ kapıların günlüğü doğrulanır; tek başına koşuda (ortam boş) hepsi doğrulanır.
  const kosan = process.env.KERVAN_KOSU_KAPI;
  const kosanSira = kosan ? d.kapilar.findIndex((k) => k.ad === kosan) : -1;
  d.kapilar.forEach((k, i) => {
    if (kosanSira >= 0 && i >= kosanSira) return;
    const hata = kapiLoguDenetle(k, kokDizin);
    if (hata) h.push(hata);
  });
  h.push(...kapiListesiKontrol(d, kokDizin), ...fazCommitKontrol(d, kokDizin));
  // Ref yalnız açık argümanla (--ref) değişir; ortam değişkeni okunmaz (Bun cwd'deki .env'i kendiliğinden yükler).
  const refDizin = path.resolve(refDizinArg ?? path.join(kokDizin, "ref", "omniroute-dbe703a"));
  if (!fs.existsSync(refDizin) || !fs.statSync(refDizin).isDirectory()) h.push(`sayimlar.route_ts ölçülemedi: OmniRoute ref dizini yok: ${refDizin}`);
  else {
    const olculen = routeTsSay(refDizin);
    if (olculen === 0) h.push(`sayimlar.route_ts ölçülemedi: ${refDizin} içinde hiç route.ts yok`);
    else if (d.sayimlar.route_ts !== olculen) h.push(`sayimlar.route_ts=${d.sayimlar.route_ts}, ref envanterinde ${olculen} route.ts var`);
  }
  const okumaHatalari: string[] = [];
  const defterDizini = path.join(kokDizin, "port", "defter");
  // hepsi.ts sayimlariOlc ile aynı kaynak: yalnız dosyalar.jsonl (davranislar.jsonl dosya pencereleri değildir).
  const kayitlar = defterSatirlariniOku(path.join(defterDizini, "dosyalar.jsonl"), okumaHatalari);
  if (okumaHatalari.length) h.push(`sayimlar doğrulanamadı: ${okumaHatalari.join("; ")}`);
  else {
    const say = (x: string) => kayitlar.filter((k) => k.durum === x).length;
    if (d.sayimlar.defter_planlandi !== say("planlandi")) h.push(`sayimlar.defter_planlandi=${d.sayimlar.defter_planlandi}, defterde ${say("planlandi")}`);
    if (d.sayimlar.defter_dogrulandi !== say("dogrulandi")) h.push(`sayimlar.defter_dogrulandi=${d.sayimlar.defter_dogrulandi}, defterde ${say("dogrulandi")}`);
  }
  return h;
}

/** README dosyalarının mühürlü taban başlıkları (HEAD 10f8598). Silinen başlık kırmızı; yeni başlık eklenebilir.
 *  Taban, kapıyla birlikte mühürlenir: bölüm bilerek kaldırılacaksa bu liste değişir ve Ercan yeniden mühürler. */
export const TABAN_README_BASLIKLARI: Record<string, string[]> = {
  "README.md": ["# Kervan", "## 📦 Gereksinimler", "## 🛠 İşlev ve Özellikler", "## ⚙ Kurulum", "### Kurulum Adımları", "### Kullanım", "## 🗺 Durum ve Yol Haritası", "### Teknik Detaylar", "### Sağlayıcılar", "#### Sağlayıcı Durumu", "## ⚠️ Bilinen Sorunlar / Deneysel Kısımlar", "## 🤝 Katkıda Bulunanlar", "## 📜 Lisans"],
  "README.en.md": ["# Kervan", "## 📦 Prerequisites", "## 🛠 Features & Functionality", "## ⚙ Installation", "### Installation Steps", "### Usage", "## 🗺 Status & Roadmap", "### Technical Details", "### Providers", "#### Provider Status", "## ⚠️ Known Issues / Experimental Features", "## 🤝 Contributors", "## 📜 License"],
  "README.ar.md": ["# كرفان", "## 📦 المتطلبات", "## 🛠 الوظائف والميزات", "## ⚙ التثبيت", "### خطوات التثبيت", "### الاستخدام", "## 🗺 الحالة وخارطة الطريق", "### التفاصيل التقنية", "### المزودون", "#### حالة المزودين", "## ⚠️ المشاكل المعروفة / الميزات التجريبية", "## 🤝 المساهمون", "## 📜 الترخيص"],
  "README.ja.md": ["# キャラバン (Kervan)", "## 📦 前提条件", "## 🛠 機能と特徴", "## ⚙ セットアップ", "### セットアップ手順", "### 使用方法", "## 🗺 ステータスとロードマップ", "### 技術詳細", "### プロバイダー", "#### プロバイダーの状態", "## ⚠️ 既知の問題 / 実験的機能", "## 🤝 貢献者", "## 📜 ライセンス"],
};

/** Destek iddiası sözcükleri (TR/EN/AR/JA). ponytail: sözcük listesi; olumsuz cümle ("not supported") da iddia sayılır (fail-closed yön). */
const DESTEK_IDDIASI = /destekleni(?:yor|r)|çalışıyor|kullanılabilir|\bsupported\b|\bworks\b|\bavailable\b|✅|مدعوم|متاح|対応|サポート|利用可能/i;

/** Satırdaki HTTP uç yolları (/v1/models; http://host:port/v1/x → /v1/x). */
export function satirdakiUcYollari(satir: string): string[] {
  const s = satir.replace(/https?:\/\/[^\s/`'")\]]+/g, " ");
  return [...s.matchAll(/(?<![\p{L}\p{N}_/.~-])\/[A-Za-z0-9_][\w\-./{}:]*/gu)].map((x) => x[0].replace(/[.,:;]+$/, "").replace(/(.)\/$/, "$1"));
}

/** uclar.toml'da durumu "gercek" olan uç yolları; dosya yok/ayrıştırılamaz/boşsa hata atar. */
export function gercekUclar(uclarTomlYolu: string): Set<string> {
  if (!dosyaMi(uclarTomlYolu)) throw new Error(`uclar.toml bulunamadı: ${uclarTomlYolu}`);
  let t: any;
  try {
    t = (globalThis as any).Bun.TOML.parse(fs.readFileSync(uclarTomlYolu, "utf-8"));
  } catch (e: any) {
    throw new Error(`uclar.toml ayrıştırılamadı: ${e.message}`);
  }
  if (!Array.isArray(t.uc) || t.uc.length === 0) throw new Error(`uclar.toml'da [[uc]] kaydı yok`);
  return new Set(t.uc.filter((u: any) => u?.durum === "gercek" && typeof u.yol === "string").map((u: any) => u.yol));
}

export function readmeKontrol(
  readmeYolu = "README.md",
  uclarTomlYolu?: string,
  kokDizin = process.cwd(),
  taban: Record<string, string[]> = TABAN_README_BASLIKLARI
): { uygun: boolean; hatalar: string[] } {
  const hatalar: string[] = [];
  if (!fs.existsSync(readmeYolu)) return { uygun: false, hatalar: [`${readmeYolu} bulunamadı`] };
  const dizin = path.dirname(readmeYolu);
  const yolu = (ad: string) => (ad === "README.md" ? readmeYolu : path.join(dizin, ad));

  // 1. Mühürlü taban başlık listesiyle fark: silinen bölüm ya da silinen dil sürümü kırmızı.
  for (const [ad, basliklar] of Object.entries(taban)) {
    if (!dosyaMi(yolu(ad))) {
      hatalar.push(`${ad} bulunamadı: mühürlü tabanda var, dil sürümü silinmiş`);
      continue;
    }
    const mevcut = new Set(lf(fs.readFileSync(yolu(ad), "utf-8")).split("\n").filter((s) => /^#{1,6}\s/.test(s)).map((s) => s.trimEnd()));
    for (const b of basliklar) if (!mevcut.has(b)) hatalar.push(`${ad}: README içinde '${b}' bölümü silinmiş veya bulunamadı (mühürlü taban başlık listesi)`);
  }

  // 2. Durum bloğu dışında, uclar.toml'da "gercek" olmayan uca destek iddiası kırmızı (FR-2.7).
  let gercek: Set<string> | null = null;
  try {
    gercek = gercekUclar(uclarTomlYolu ?? path.join(kokDizin, "port", "uclar.toml"));
  } catch (e: any) {
    hatalar.push(`README destek iddiaları denetlenemedi: ${e.message}`);
  }
  for (const ad of gercek ? README_DOSYALARI : []) {
    if (!dosyaMi(yolu(ad))) continue;
    let ic = lf(fs.readFileSync(yolu(ad), "utf-8"));
    try {
      const b = readmeBlokBul(ic);
      if (b) ic = ic.slice(0, b.bas) + b.govde.replace(/[^\n]/g, "") + ic.slice(b.son); // blok aşağıda üretilenle karşılaştırılır
    } catch {} // bozuk işaret aşağıda kırmızı; burada bütün metin taranır
    ic.split("\n").forEach((satir, i) => {
      if (!DESTEK_IDDIASI.test(satir)) return;
      for (const y of satirdakiUcYollari(satir)) {
        if (!gercek!.has(y)) hatalar.push(`${ad}:${i + 1}: '${y}' destekleniyor deniyor ama uclar.toml'da durumu "gercek" değil: ${JSON.stringify(satir.trim())}`);
      }
    });
  }

  // 3. Durum blokları: README.md'nin dizinindeki tüm dil sürümleri.
  let blok: string | null = null;
  for (const ad of README_DOSYALARI) {
    const yol = ad === "README.md" ? readmeYolu : path.join(dizin, ad);
    if (!fs.existsSync(yol)) continue;
    let b;
    try {
      b = readmeBlokBul(lf(fs.readFileSync(yol, "utf-8")));
    } catch (e: any) {
      hatalar.push(`${ad}: ${e.message}`);
      continue;
    }
    if (!b) {
      hatalar.push(`${ad}: durum bloğu (${BLOK_BASLA} … ${BLOK_BITIS}) yok; durum/özellik iddiası yalnız durum-uret çıktısı olabilir (FR-2.7)`);
      continue;
    }
    if (blok === null) {
      try {
        blok = readmeBlokUret(durumJsonOku(path.join(kokDizin, "port", "durum.json")));
      } catch (e: any) {
        hatalar.push(`${ad}: durum bloğu var ama durum.json'dan üretilemiyor: ${e.message}`);
        break;
      }
    }
    if (b.govde !== blok) hatalar.push(`${ad}: durum bloğu elle düzenlenmiş (durum-uret çıktısıyla bayt-özdeş değil)`);
  }

  // README.md'de "Sağlayıcı Durumu" bölümünün gövdesi yalnız üretilmiş blok olmalı (elle durum tablosu yok).
  const satirlar = lf(fs.readFileSync(readmeYolu, "utf-8")).split("\n");
  const i = satirlar.findIndex((s) => /^#{1,6}\s/.test(s) && s.includes(SAGLAYICI_BOLUMU));
  if (i >= 0) {
    let j = i + 1;
    while (j < satirlar.length && !/^#{1,6}\s/.test(satirlar[j])) j++;
    const govde = satirlar.slice(i + 1, j).join("\n").trim();
    if (!govde.startsWith(BLOK_BASLA) || !govde.endsWith(BLOK_BITIS)) {
      hatalar.push(`README.md: '${SAGLAYICI_BOLUMU}' bölümünde üretilmiş durum bloğu dışında elle yazılmış içerik var (yalnız ${BLOK_BASLA} … ${BLOK_BITIS} olmalı)`);
    }
  }
  return { uygun: hatalar.length === 0, hatalar };
}

export function durumDenetle(
  durumMdYolu?: string,
  readmeYolu?: string,
  kokDizin = process.cwd(),
  ek: { refDizin?: string } = {}
): { uygun: boolean; hatalar: string[] } {
  const kok = path.resolve(kokDizin);
  const hatalar: string[] = [...durumJsonKontrol(kok, ek.refDizin)];
  const ds = durumMdKontrol(durumMdYolu ?? path.join(kok, "port", "DURUM.md"), kok);
  if (!ds.uygun) hatalar.push(ds.hata!);
  hatalar.push(...readmeKontrol(readmeYolu ?? path.join(kok, "README.md"), path.join(kok, "port", "uclar.toml"), kok).hatalar);
  return { uygun: hatalar.length === 0, hatalar };
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  let kok = process.cwd();
  let refDizin: string | undefined;
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--kok" && args[i + 1]) kok = path.resolve(args[++i]);
    else if (args[i] === "--ref" && args[i + 1]) refDizin = path.resolve(args[++i]);
    else {
      console.log(`HUKUM: kirmizi bilinmeyen argüman: ${args[i]}`);
      process.exit(1);
    }
  }
  let sonuc;
  try {
    sonuc = durumDenetle(undefined, undefined, kok, { refDizin });
  } catch (e: any) {
    console.log(`HUKUM: kirmizi durum-kontrol çöktü: ${e?.message ?? e}`);
    process.exit(1);
  }
  if (sonuc.uygun) {
    console.log("[durum-kontrol] port/durum.json, port/DURUM.md ve README durum blokları üretilmiş çıktıyla özdeş.");
    console.log("HUKUM: yesil");
    process.exit(0);
  }
  console.log(`[durum-kontrol] ${sonuc.hatalar.length} ihlal:`);
  for (const h of sonuc.hatalar) console.log(`  - ${h}`);
  console.log(`HUKUM: kirmizi ${sonuc.hatalar.length} ihlal; ilki: ${sonuc.hatalar[0].replace(/\s+/g, " ")}`);
  process.exit(1);
}
