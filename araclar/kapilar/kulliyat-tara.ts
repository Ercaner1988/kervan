import crypto from "crypto";
import fs from "fs";
import os from "os";
import path from "path";
import { temizKopyadaKos, TemizKopyaSecenek } from "./temiz-kopya.ts";

// PRD FR-2.5: Kırmızı Takım Külliyatı (kulliyat-tara.ts).
// Dizge ezberi YOK: denetlenen commit temiz-kopya ile os.tmpdir altında bir worktree'ye çıkarılır, oraya ÇAĞIRANIN
// kapı paketi (varsayılan HEAD'in git nesneleri) konur ve paketin kapıları o ağaca karşı koşulur. Her kapının
// stdout'undaki son boş olmayan satır (HUKUM) toplanır. Bir sahte sınıfı, o sınıfın kusurunu ÖLÇEN kapı
// 'kirmizi' derse yakalanmış sayılır (SINIF_KAPILARI, gerekçeli).
//   Külliyat kipi (varsayılan, ref = ac2f18d): Ek C'deki beş sınıfın HER BİRİ en az bir kapıdan kırmızı almalı.
//   Temiz kip (--temiz): aynı paket o ağaçta HİÇ sahte sınıf bulmamalı (bütün kapılar yeşil).
// Fail-closed: çözülemeyen ref → 'ref-yok' (çıkış 3). Kapının çökmesi, zaman aşımı, HUKUM satırının olmaması / son
// satır olmaması / nedensiz 'kirmizi' / çıkış koduyla tutarsız olması 'hata'dır: ölçüm yok sayılır, yakalama sayılmaz,
// sonuç kırmızıdır (bozuk paket her şeyi "yakalamış" görünemez).
// Ölçülemedi: her kapı OKUDUĞU girdilerin tamamını (KapiTanimi.girdi) ve ön koşullarını (onkosul) bildirir. Bir girdi
// yoksa, ayrıştırılamıyorsa (kapının KENDİ okuyucusu paketten içe aktarılıp çağrılır; atar, hata yazar ya da boş
// dönerse) ya da ön koşul (cargo metadata, kervan paketi) sağlanmıyorsa kapı koşulmaz, 'olculemedi' sayılır: fail-closed
// 'girdi yok/bozuk' kırmızısı sınıfın kusurunu ölçmez, yakalama sayılmaz; temiz kipte de kırmızıdır (ölçülmemiş ağaç
// temiz sayılamaz). Karar kapı iletisinin metnine değil, girdinin varlığına ve kapının kendi ayrıştırıcısına bakar.
// Bir sınıfı yalnız o sınıfın kusurunu ÖLÇEN kapı yakalar (uc-kaydi pakette koşar ama hiçbir sınıfı yakalamaz: kırmızısı
// çoğunlukla kayıt uyuşmazlığıdır, sabit yanıtın ölçümü değildir).
// rapor-kontrol'ün fazları ölçülen ağaçtan değil, ÇAĞIRANIN paketindeki kapilar.toml'dan (rapor-kontrol'ü listeleyen
// her faz) gelir; dosya yok/bozuk ya da liste boşsa 'hata'. O fazın RAPOR.md'si yoksa o koşu 'olculemedi'dir.
// mutasyon-kontrol CLI'ı koşulmaz (cargo test koşar); onun parite ölçümü (pariteBaglantisiniDenetle) cargo testsiz
// çağrılır ve 'parite-baglantisi' kapısı olarak kaydedilir.

export const CIKIS = { YESIL: 0, KIRMIZI: 1, REF_YOK: 3 } as const;
export const KULLIYAT_REF = "ac2f18d3fa588dad886589ff901ebd1c7ad5afae";

/** Kapının okuduğu girdi; yol worktree köküne göre ({faz} yerine faz adı).
 *  Varsayılan: boş olmayan dosya. dizin: boş olmayan dizin. uzanti: dizin ve altında (target/node_modules/.git/ref
 *  hariç) o uzantılı dosya. bicim: dosya (uzanti varsa altındaki HER dosya) ayrıştırılabilmeli. alanlar: o dosyalardan
 *  biri bu anahtarları taşıyan JSON nesnesi. okuyucu: [paketteki modül, işlev]; işlev(yol, hatalar) atmamalı, hatalar'a
 *  yazmamalı, boş dizi ya da boş olmayan {hatalar} dönmemeli. istege: yoksa sorun değil, varsa geçerli olmalı. */
export interface Girdi {
  yol: string;
  dizin?: boolean;
  uzanti?: string;
  bicim?: "json" | "toml";
  alanlar?: string[];
  okuyucu?: [string, string];
  istege?: boolean;
  /** Kapının kendi tarama kuralı: "src" = yalnız src/ altındaki dosyalar (sabit-yanit/uc-kaydi rustDosyalariniBul), "ust" = alt dizine inmeden (mutasyon-kontrol pariteBaglantisiniDenetle). */
  kapsam?: "src" | "ust";
}

/** cargo-paketi: `cargo metadata` başarılı ve en az bir workspace paketi var. kervan-paketi: ayrıca 'kervan' paketi var. */
export type Onkosul = "cargo-paketi" | "kervan-paketi";

export interface KapiTanimi {
  ad: string;
  /** Worktree kökünde koşan bun argümanları; "parite-baglantisi" için yok (süreç içi çağrı). */
  argv?: string[];
  girdi: Girdi[];
  onkosul?: Onkosul[];
}

const RS: Girdi = { yol: ".", uzanti: ".rs", kapsam: "src" }; // sabit-yanit/uc-kaydi kökten, yalnız src/ altını tarar (rustDosyalariniBul)
const REF: Girdi = { yol: "ref/omniroute-dbe703a", dizin: true }; // temiz-kopya çağıranın ref'ini bağlar
const UCLAR: Girdi = { yol: "port/uclar.toml", okuyucu: ["uc-kaydi.ts", "uclarTomlOku"] };
const DEFTER: Girdi[] = ["dosyalar", "davranislar"].map((d) => ({ yol: `port/defter/${d}.jsonl`, okuyucu: ["defter-kontrol.ts", "defterSatirlariniOku"] as [string, string] }));
const KAPILAR_TOML: Girdi = { yol: "araclar/kapilar/kapilar.toml", bicim: "toml" };
/** rapor-kontrol her faz için ayrı koşar; fazlar çağıranın kapilar.toml'undan. Girdiler kapının okuduğu dosyalardır. */
export const KAPI_PAKETI: KapiTanimi[] = [
  { ad: "sabit-yanit", argv: ["araclar/kapilar/sabit-yanit.ts"], girdi: [RS] },
  {
    ad: "ulasilabilirlik", argv: ["araclar/kapilar/ulasilabilirlik.ts"], onkosul: ["kervan-paketi"],
    girdi: [{ yol: "Cargo.toml", bicim: "toml" }, { yol: "port/kutuphane-izni.toml", okuyucu: ["ulasilabilirlik.ts", "kutuphaneIzinleriniOku"], istege: true }],
  },
  { ad: "uc-kaydi", argv: ["araclar/kapilar/uc-kaydi.ts"], girdi: [RS, UCLAR] },
  { ad: "defter-kontrol-final", argv: ["araclar/kapilar/defter-kontrol.ts", "--final"], girdi: [...DEFTER, REF] },
  {
    ad: "durum-kontrol", argv: ["araclar/kapilar/durum-kontrol.ts"],
    // Yargılanan şey DURUM.md'dir; zorunlu girdi yalnız o. Üretim kaynakları (durum.json, uclar.toml, …)
    // yoksa kapı DURUM.md'yi yeniden üretemez ve "elle yazılmış" der: bu, sınıfın ölçümünün kendisidir.
    girdi: [
      { yol: "port/DURUM.md" },
      { yol: "port/durum.json", okuyucu: ["durum-uret.ts", "durumJsonOku"], istege: true }, { yol: "README.md", istege: true },
      { ...UCLAR, istege: true }, ...DEFTER.map((d) => ({ ...d, istege: true })), { ...REF, istege: true },
      { yol: "araclar/kapilar/hepsi.ts", istege: true }, { ...KAPILAR_TOML, istege: true },
    ],
  },
  {
    ad: "rapor-kontrol", argv: ["araclar/kapilar/rapor-kontrol.ts"],
    girdi: [{ yol: "port/kayit/{faz}/RAPOR.md" }, { yol: "port/kayit/{faz}/kosu.json", bicim: "json", istege: true }, KAPILAR_TOML],
  },
  {
    ad: "parite-baglantisi", onkosul: ["cargo-paketi"],
    // Altın şart değil: bağlantı (Kervan çağrılıyor mu) altınsız da ölçülür; altın yoksa kapının kendisi
    // "ölçülemez" der ve aşağıda kendiniDogrulayan=false olduğu için yakalama sayılmaz.
    girdi: [{ yol: "araclar/parite", uzanti: ".ts", kapsam: "ust" }],
  },
];

export type SinifAdi = "sabit-yanit" | "bagsiz-crate" | "kendini-dogrulayan-parite" | "kanitsiz-defter" | "elle-yazilmis-durum";

/** PRD Ek C sınıfı → kusuru ölçen kapılar ve neden. Eşleme kapının NE ölçtüğüne dayanır, çıktı metnine değil. */
export const SINIF_KAPILARI: Record<SinifAdi, { kapilar: string[]; gerekce: string }> = {
  "sabit-yanit": {
    kapilar: ["sabit-yanit"],
    gerekce: "sabit-yanit üretim Rust kaynağında (test blokları maskelenmiş) sabit yanıt kalıbını ölçer; uc-kaydi sayılmaz, kırmızısı çoğunlukla rota/kayıt uyuşmazlığıdır",
  },
  "bagsiz-crate": {
    kapilar: ["ulasilabilirlik"],
    gerekce: "ulasilabilirlik her workspace crate'inin kervan ikilisine normal bağımlılıkla ulaştığını cargo metadata grafından ölçer",
  },
  "kendini-dogrulayan-parite": {
    kapilar: ["parite-baglantisi"],
    gerekce: "mutasyon-kontrol.pariteBaglantisiniDenetle karşılaştırıcıyı sahte Kervan'la esit/fark/hata kiplerinde koşar; Kervan'ı çağırmayan ya da farkı görmeyen karşılaştırıcı düşer",
  },
  "kanitsiz-defter": {
    kapilar: ["defter-kontrol-final"],
    gerekce: "defter-kontrol --final her satırın var olan hedef/kanıta bağlı, pencerelerin boşluksuz ve ref envanterinin tam olduğunu ölçer",
  },
  "elle-yazilmis-durum": {
    kapilar: ["durum-kontrol"],
    gerekce: "durum-kontrol DURUM.md/README durum bloğunu geçerli durum.json'dan yeniden üretip bayt bayt karşılaştırır, günlükleri ve faz commit'lerini doğrular; rapor-kontrol sayılmaz, kırmızısı çoğunlukla Ek D biçim eksiğidir (sınıfın ölçümü değil)",
  },
};
const SINIFLAR = Object.keys(SINIF_KAPILARI) as SinifAdi[];

export interface KapiSonucu {
  ad: string;
  komut: string;
  cikis: number | null;
  /** yesil/kirmizi: geçerli HUKUM satırı; hata: kapı bozuk (çökme, zaman aşımı, satır yok/bozuk/tutarsız);
   *  olculemedi: kapının girdisi ağaçta yok, kapı koşulmadı. */
  durum: "yesil" | "kirmizi" | "hata" | "olculemedi";
  neden: string;
}

export interface KulliyatSecenek extends TemizKopyaSecenek {
  /** Temiz kip: ağaçta hiç sahte sınıf bulunmamalı. */
  temiz?: boolean;
  /** Tek kapı için süre sınırı (ms). Varsayılan 900 sn. */
  kapiZamanAsimiMs?: number;
  /** Denetlenecek paket (yalnız test için değiştirilir). */
  paket?: KapiTanimi[];
  geciciKok?: string;
}

export interface KulliyatRaporu {
  ref: string;
  hedefSha: string;
  kip: "kulliyat" | "temiz";
  cikis: number;
  hukum: "yesil" | "kirmizi";
  neden: string;
  kapilar: KapiSonucu[];
  /** sınıf → onu kırmızıya çeviren kapılar (boş = kaçtı). */
  yakalayanlar: Record<SinifAdi, string[]>;
  sinif1_sabitYanit: boolean;
  sinif2_bagisizCrate: boolean;
  sinif3_pariteBaglantisiz: boolean;
  sinif4_kanitsizDefter: boolean;
  sinif5_elleYazilmisDurum: boolean;
  tumSiniflarYakalandiMi: boolean;
  bulgular: string[];
}

/** Bir kapının stdout + çıkışından hüküm: hepsi.ts sözleşmesi (son boş olmayan satır, neden boş olamaz). */
export function hukumCoz(stdout: string, cikis: number | null): { durum: KapiSonucu["durum"]; neden: string } {
  const satirlar = stdout.split(/\r?\n/).map((s) => s.trimEnd()).filter((s) => s.trim() !== "");
  const son = satirlar[satirlar.length - 1] ?? "";
  if (cikis === null) return { durum: "hata", neden: "kapı sonlandırıldı (zaman aşımı ya da sinyal)" };
  if (son === "HUKUM: yesil") {
    return cikis === 0 ? { durum: "yesil", neden: "" } : { durum: "hata", neden: `HUKUM: yesil ama çıkış ${cikis}` };
  }
  const m = /^HUKUM: kirmizi (.*\S.*)$/.exec(son);
  if (m) {
    return cikis !== 0 ? { durum: "kirmizi", neden: m[1].trim() } : { durum: "hata", neden: "HUKUM: kirmizi ama çıkış 0 (tutarsız)" };
  }
  if (satirlar.some((s) => s.startsWith("HUKUM:"))) return { durum: "hata", neden: `HUKUM satırı son satır değil ya da bozuk (son satır: '${son.slice(0, 80)}')` };
  return { durum: "hata", neden: `HUKUM satırı yok (çıkış ${cikis})` };
}

// Worktree'de koşan ara betik (os.tmpdir'de; worktree'nin kapı dizinine dokunmaz). Kapıları sırayla, süre sınırıyla
// (süreç ağacı öldürülerek) koşar; sonuçları tek bir işaretli JSON satırı olarak basar. İşaret nonce'u kapılara
// geçirilmez: hedefin kodu sahte sonuç satırı basamaz (kapıların stdout'u zaten yakalanır, dışarı basılmaz).
const KOSUCU = `
import { spawn, spawnSync } from "child_process";
import fs from "fs";
import path from "path";
const nonce = process.env.KULLIYAT_NONCE; delete process.env.KULLIYAT_NONCE;
const paket = JSON.parse(process.env.KULLIYAT_PAKET); delete process.env.KULLIYAT_PAKET;
const ms = Number(process.env.KULLIYAT_KAPI_MS); delete process.env.KULLIYAT_KAPI_MS;
const bun = process.execPath;
const kos = (argv) => new Promise((coz) => {
  const p = spawn(bun, argv, { cwd: process.cwd(), env: process.env, stdio: ["ignore", "pipe", "pipe"], detached: process.platform !== "win32" });
  let out = "", err = "", asti = false;
  p.stdout.on("data", (d) => { if (out.length < 4e6) out += d; });
  p.stderr.on("data", (d) => { if (err.length < 1e5) err += d; });
  const t = setTimeout(() => {
    asti = true;
    if (process.platform === "win32") spawnSync("taskkill", ["/PID", String(p.pid), "/T", "/F"]);
    else { try { process.kill(-p.pid, "SIGKILL"); } catch {} }
  }, ms);
  p.on("error", (e) => { clearTimeout(t); coz({ cikis: 1, out, err: err + String(e) }); });
  p.on("close", (c) => { clearTimeout(t); coz({ cikis: asti ? null : c, out, err }); });
});
// Fazlar çağıranın paketindeki kapilar.toml'dan (temiz-kopya worktree'nin araclar/kapilar'ını çağıranınkiyle değiştirir).
const fazlar = () => {
  const t = Bun.TOML.parse(fs.readFileSync(path.join("araclar", "kapilar", "kapilar.toml"), "utf-8"));
  const f = Object.entries(t?.faz ?? {}).filter(([, l]) => Array.isArray(l) && l.some((k) => k?.ad === "rapor-kontrol")).map(([a]) => a);
  if (!f.length) throw new Error("kapilar.toml'da rapor-kontrol'ü listeleyen faz yok");
  return f.sort((a, b) => Number(a.slice(1)) - Number(b.slice(1)));
};
// Girdi denetimi: kapının okuduğu her dosya var ve (kapının kendi okuyucusuyla) ayrıştırılabilir olmalı.
const ATLA = new Set(["target", "node_modules", ".git", "ref"]);
const bul = (dz, uz, on = "") => {
  const o = [];
  for (const e of fs.readdirSync(dz, { withFileTypes: true })) {
    if (ATLA.has(e.name)) continue;
    const r = on ? on + "/" + e.name : e.name;
    if (e.isDirectory()) o.push(...bul(path.join(dz, e.name), uz, r));
    else if (e.isFile() && e.name.endsWith(uz)) o.push(r);
  }
  return o;
};
const ayristir = (y, bicim) => {
  const m = fs.readFileSync(y, "utf-8");
  return bicim === "toml" ? Bun.TOML.parse(m) : JSON.parse(m);
};
const okuyucuHatasi = async (y, [modul, islev]) => {
  try {
    const f = (await import(path.resolve("araclar", "kapilar", modul)))[islev];
    if (typeof f !== "function") return "paketteki " + modul + " '" + islev + "' işlevini dışa açmıyor";
    const h = [];
    const r = f(y, h);
    if (h.length) return String(h[0]);
    if (Array.isArray(r) && !r.length) return "boş (kayıt yok)";
    if (Array.isArray(r?.hatalar) && r.hatalar.length) return String(r.hatalar[0]);
    return "";
  } catch (e) { return String(e?.message ?? e); }
};
let meta;
const cargoMeta = () => {
  if (meta) return meta;
  const r = spawnSync("cargo", ["metadata", "--format-version", "1"], { encoding: "utf-8", maxBuffer: 256 * 1024 * 1024, timeout: 600000, env: { ...process.env, CARGO_BUILD_JOBS: "2" } });
  if (r.error || r.status !== 0) return (meta = { hata: "cargo metadata başarısız (çıkış " + (r.status ?? r.error?.message) + "): " + String(r.stderr ?? "").trim().slice(-200) });
  try {
    const v = JSON.parse(r.stdout);
    const uyeler = new Set(v.workspace_members ?? []);
    return (meta = { adlar: (v.packages ?? []).filter((p) => uyeler.has(p.id)).map((p) => p.name) });
  } catch (e) { return (meta = { hata: "cargo metadata ayrıştırılamadı: " + e.message }); }
};
const girdiEksik = async (k, faz) => {
  if (!Array.isArray(k.girdi) || !k.girdi.length) return "kapının girdisi tanımlı değil";
  for (const g of k.girdi) {
    const y = g.yol.replace("{faz}", faz ?? "");
    let st;
    try { st = fs.statSync(y); } catch { if (g.istege) continue; return y + " yok"; }
    if (g.dizin) { if (!st.isDirectory() || !fs.readdirSync(y).length) return y + " boş ya da dizin değil"; continue; }
    if (g.uzanti) {
      if (!st.isDirectory()) return y + " dizin değil";
      // Girdi, kapının gerçekten okuduğu kümeyle aynı olmalı; yoksa kapının görmediği dosya 'girdi var' sayılır.
      const dosyalar = bul(y, g.uzanti).filter((f) =>
        g.kapsam === "src" ? f.split("/").includes("src") : g.kapsam === "ust" ? !f.includes("/") : true);
      if (!dosyalar.length) return y + " altında " + g.uzanti + " dosyası yok";
      const nesneler = [];
      if (g.bicim) for (const f of dosyalar) {
        try { nesneler.push(ayristir(path.join(y, f), g.bicim)); } catch (e) { return y + "/" + f + " ayrıştırılamadı: " + e.message; }
      }
      if (g.alanlar && !nesneler.some((v) => v !== null && typeof v === "object" && g.alanlar.every((a) => a in v))) {
        return y + " altında {" + g.alanlar.join(", ") + "} alanlı " + g.uzanti + " yok";
      }
      continue;
    }
    if (!st.isFile() || st.size === 0) return y + " boş ya da dosya değil";
    if (g.bicim) { try { ayristir(y, g.bicim); } catch (e) { return y + " ayrıştırılamadı: " + e.message; } }
    if (g.okuyucu) { const h = await okuyucuHatasi(y, g.okuyucu); if (h) return y + " kapının okuyucusundan geçmedi: " + h; }
  }
  for (const o of k.onkosul ?? []) {
    const m = cargoMeta();
    if (m.hata) return m.hata;
    if (!m.adlar.length) return "cargo metadata hiç workspace paketi listelemedi";
    if (o === "kervan-paketi" && !m.adlar.includes("kervan")) return "workspace'te 'kervan' paketi yok (ulaşılabilirliğin kökü)";
  }
  return "";
};
const sonuclar = [];
for (const k of paket) {
  if (!k.argv) {
    const eksik = await girdiEksik(k);
    if (eksik) { sonuclar.push({ ad: k.ad, komut: "pariteBaglantisiniDenetle(araclar/parite)", cikis: null, out: "", eksik }); continue; }
    let out = "", cikis = 1;
    try {
      const m = await import(path.resolve("araclar/kapilar/mutasyon-kontrol.ts"));
      const s = m.pariteBaglantisiniDenetle("araclar/parite");
      const h = Array.isArray(s?.hatalar) ? s.hatalar.map(String) : null;
      if (typeof s?.uygun !== "boolean" || !h) out = "pariteBaglantisiniDenetle beklenen {uygun, hatalar} yapısını döndürmedi";
      else if (s.uygun && h.length === 0) { out = "HUKUM: yesil"; cikis = 0; }
      else if (s.kendiniDogrulayan !== true) {
        // Kırmızı ama kusur ölçülmedi (altın yok, koşturma hatası): yakalama değil, ölçülemedi.
        sonuclar.push({ ad: k.ad, komut: "pariteBaglantisiniDenetle(araclar/parite)", cikis: null, out: "", eksik: "parite ölçülemedi: " + (h[0] || "neden yazılmadı") });
        continue;
      }
      else out = "HUKUM: kirmizi " + (h[0] || "uygun=false, hata yazılmadı").replace(/\\s+/g, " ");
    } catch (e) { out = "parite ölçümü çöktü: " + (e?.message ?? e); }
    sonuclar.push({ ad: k.ad, komut: "pariteBaglantisiniDenetle(araclar/parite)", cikis, out });
    continue;
  }
  let fz = [undefined];
  if (k.ad === "rapor-kontrol") {
    try { fz = fazlar(); }
    catch (e) { sonuclar.push({ ad: k.ad, komut: "faz listesi", cikis: null, out: "", hata: "faz listesi okunamadı: " + (e?.message ?? e) }); continue; }
  }
  for (const f of fz) {
    const a = f ? [...k.argv, f] : k.argv;
    const ad = f ? k.ad + ":" + f : k.ad;
    const eksik = await girdiEksik(k, f);
    if (eksik) { sonuclar.push({ ad, komut: "bun " + a.join(" "), cikis: null, out: "", eksik }); continue; }
    const r = await kos(a);
    sonuclar.push({ ad, komut: "bun " + a.join(" "), cikis: r.cikis, out: r.out, err: r.err.slice(-2000) });
  }
}
console.log(nonce + JSON.stringify(sonuclar));
`;

export function kulliyatCommitiniTara(ref = KULLIYAT_REF, secenek: KulliyatSecenek = {}): KulliyatRaporu {
  const temiz = secenek.temiz === true;
  const yakalayanlar = Object.fromEntries(SINIFLAR.map((s) => [s, [] as string[]])) as Record<SinifAdi, string[]>;
  const rapor: KulliyatRaporu = {
    ref, hedefSha: "", kip: temiz ? "temiz" : "kulliyat", cikis: CIKIS.KIRMIZI, hukum: "kirmizi", neden: "",
    kapilar: [], yakalayanlar, sinif1_sabitYanit: false, sinif2_bagisizCrate: false, sinif3_pariteBaglantisiz: false,
    sinif4_kanitsizDefter: false, sinif5_elleYazilmisDurum: false, tumSiniflarYakalandiMi: false, bulgular: [],
  };
  const kirmizi = (neden: string, cikis: number = CIKIS.KIRMIZI) => Object.assign(rapor, { cikis, neden, hukum: "kirmizi" as const });

  const paket = secenek.paket ?? KAPI_PAKETI;
  if (paket.length === 0) return kirmizi("kapı paketi boş; ölçülecek bir şey yok");

  const geciciKok = path.resolve(secenek.geciciKok ?? path.join(os.tmpdir(), "kervan-kulliyat"));
  fs.mkdirSync(geciciKok, { recursive: true });
  const betik = path.join(geciciKok, `kosucu_${process.pid}_${crypto.randomBytes(4).toString("hex")}.mjs`);
  const nonce = `@@KULLIYAT_${crypto.randomBytes(12).toString("hex")}@@`;
  fs.writeFileSync(betik, KOSUCU);
  const eskiEnv = { n: process.env.KULLIYAT_NONCE, p: process.env.KULLIYAT_PAKET, m: process.env.KULLIYAT_KAPI_MS, t: process.env.CARGO_TARGET_DIR };
  // Temiz kopya her koşuda boş target/ ile derlerdi (~9 dk). Kaynak yine temiz kopyadan gelir; yalnız bağımlılıkların
  // derlenmiş hali depodaki target/kulliyat'ta yeniden kullanılır (cargo parmak izi yol+özet bazlıdır, kopya kodu eski derlemeyi taşıyamaz).
  process.env.CARGO_TARGET_DIR = path.resolve(secenek.depo ?? ".", "target", "kulliyat");
  process.env.KULLIYAT_NONCE = nonce;
  process.env.KULLIYAT_PAKET = JSON.stringify(paket);
  process.env.KULLIYAT_KAPI_MS = String(secenek.kapiZamanAsimiMs ?? 900_000);
  let tk;
  try {
    tk = temizKopyadaKos(ref, `"${process.execPath}" "${betik}"`, path.join(geciciKok, "wt"), {
      depo: secenek.depo, kapiKaynagi: secenek.kapiKaynagi, kapiRef: secenek.kapiRef, refDizini: secenek.refDizini,
      zamanAsimiMs: secenek.zamanAsimiMs ?? 1_700_000,
    });
  } finally {
    for (const [k, v] of [["KULLIYAT_NONCE", eskiEnv.n], ["KULLIYAT_PAKET", eskiEnv.p], ["KULLIYAT_KAPI_MS", eskiEnv.m], ["CARGO_TARGET_DIR", eskiEnv.t]] as const) {
      if (v === undefined) delete process.env[k];
      else process.env[k] = v;
    }
    fs.rmSync(betik, { force: true });
  }
  rapor.hedefSha = tk.hedefSha;
  if (tk.cikis === 3 || tk.neden.startsWith("ref-yok")) return kirmizi(tk.neden || `ref-yok: '${ref}'`, CIKIS.REF_YOK);
  if (tk.cikis !== 0) return kirmizi(`temiz kopya koşusu kırmızı (çıkış ${tk.cikis}): ${tk.neden}`);

  const satir = tk.stdout.split(/\r?\n/).filter((s) => s.startsWith(nonce)).pop();
  let ham: { ad: string; komut: string; cikis: number | null; out: string; err?: string; eksik?: string; hata?: string }[];
  try {
    ham = JSON.parse(satir!.slice(nonce.length));
    if (!Array.isArray(ham)) throw new Error("dizi değil");
  } catch {
    return kirmizi("kapı koşucusunun sonuç satırı yok ya da ayrıştırılamadı");
  }
  rapor.kapilar = ham.map((r): KapiSonucu => {
    if (r.hata) return { ad: r.ad, komut: r.komut, cikis: r.cikis, durum: "hata", neden: String(r.hata) };
    if (r.eksik) return { ad: r.ad, komut: r.komut, cikis: r.cikis, durum: "olculemedi", neden: `girdi eksik: ${r.eksik} (kapı koşulmadı)` };
    const h = hukumCoz(r.out ?? "", r.cikis);
    // Ölçemeyen kapının nedenine stderr'in son satırı eklenir (tanı için; hükmü değiştirmez).
    const sonErr = String(r.err ?? "").split(/\r?\n/).map((x) => x.trim()).filter(Boolean).pop();
    if (h.durum === "hata" && sonErr) h.neden += `; stderr: ${sonErr.slice(0, 200)}`;
    return { ad: r.ad, komut: r.komut, cikis: r.cikis, ...h };
  });
  for (const k of paket) {
    if (!rapor.kapilar.some((r) => r.ad === k.ad || r.ad.startsWith(k.ad + ":"))) return kirmizi(`kapı koşulmadı: ${k.ad}`);
  }

  for (const s of SINIFLAR) {
    yakalayanlar[s] = rapor.kapilar
      .filter((r) => r.durum === "kirmizi" && SINIF_KAPILARI[s].kapilar.includes(r.ad.split(":")[0]))
      .map((r) => r.ad);
  }
  rapor.sinif1_sabitYanit = yakalayanlar["sabit-yanit"].length > 0;
  rapor.sinif2_bagisizCrate = yakalayanlar["bagsiz-crate"].length > 0;
  rapor.sinif3_pariteBaglantisiz = yakalayanlar["kendini-dogrulayan-parite"].length > 0;
  rapor.sinif4_kanitsizDefter = yakalayanlar["kanitsiz-defter"].length > 0;
  rapor.sinif5_elleYazilmisDurum = yakalayanlar["elle-yazilmis-durum"].length > 0;
  rapor.tumSiniflarYakalandiMi = SINIFLAR.every((s) => yakalayanlar[s].length > 0);
  for (const s of SINIFLAR) {
    for (const ad of yakalayanlar[s]) rapor.bulgular.push(`${s}: ${ad} kırmızı: ${rapor.kapilar.find((r) => r.ad === ad)!.neden}`);
  }
  for (const r of rapor.kapilar) if (r.durum === "olculemedi") rapor.bulgular.push(`ölçülemedi: ${r.ad}: ${r.neden}`);

  const hatali = rapor.kapilar.filter((r) => r.durum === "hata");
  if (hatali.length) return kirmizi(`${hatali.length} kapı ölçemedi; ilki ${hatali[0].ad}: ${hatali[0].neden}`);
  if (temiz) {
    const olculmeyen = rapor.kapilar.filter((r) => r.durum === "olculemedi");
    if (olculmeyen.length) return kirmizi(`temiz ağaçta ${olculmeyen.length} kapı ölçülemedi; ilki ${olculmeyen[0].ad}: ${olculmeyen[0].neden}`);
    const kirmizilar = rapor.kapilar.filter((r) => r.durum === "kirmizi");
    if (kirmizilar.length) return kirmizi(`temiz ağaçta ${kirmizilar.length} kapı kırmızı; ilki ${kirmizilar[0].ad}: ${kirmizilar[0].neden}`);
  } else {
    const kacan = SINIFLAR.filter((s) => yakalayanlar[s].length === 0);
    if (kacan.length) return kirmizi(`${kacan.length} sahte sınıf hiçbir kapıdan kırmızı almadı: ${kacan.join(", ")}`);
  }
  return Object.assign(rapor, { cikis: CIKIS.YESIL, hukum: "yesil" as const, neden: "" });
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  let ref: string | undefined;
  let temiz = false;
  for (const a of args) {
    if (a === "--temiz") temiz = true;
    else if (!a.startsWith("--") && ref === undefined) ref = a;
    else {
      console.log(`HUKUM: kirmizi bilinmeyen ya da fazla argüman: ${a} (kullanım: kulliyat-tara.ts [--temiz] [ref])`);
      process.exit(2);
    }
  }
  ref ??= temiz ? "HEAD" : KULLIYAT_REF;
  console.log(`[kulliyat-tara] ${temiz ? "temiz ağaç" : "külliyat"} kipi, ref '${ref}': kapı paketi temiz kopyada koşuluyor...`);
  const r = kulliyatCommitiniTara(ref, { temiz });
  if (r.hedefSha) console.log(`[kulliyat-tara] hedef: ${r.hedefSha}`);
  for (const k of r.kapilar) console.log(`  [${k.durum}] ${k.ad} (${k.komut}, çıkış ${k.cikis})${k.neden ? `: ${k.neden}` : ""}`);
  if (r.kapilar.length) {
    console.log("[kulliyat-tara] sınıf → kapı:");
    for (const s of SINIFLAR) {
      const y = r.yakalayanlar[s];
      const olcmeyen = r.kapilar.filter((k) => SINIF_KAPILARI[s].kapilar.includes(k.ad.split(":")[0]) && k.durum === "olculemedi").map((k) => k.ad);
      const durum = y.length ? `YAKALANDI (${y.join(", ")})` : `yakalanmadı${olcmeyen.length ? ` (ölçülemedi: ${olcmeyen.join(", ")})` : ""}`;
      console.log(`  ${s}: ${durum} | ölçen:${SINIF_KAPILARI[s].kapilar.join(", ")} | ${SINIF_KAPILARI[s].gerekce}`);
    }
  }
  console.log(r.hukum === "yesil" ? "HUKUM: yesil" : `HUKUM: kirmizi ${r.neden.replace(/\s+/g, " ")}`);
  process.exit(r.cikis);
}
