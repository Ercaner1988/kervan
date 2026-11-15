/**
 * Kervan durum panosu — ölçüm toplayıcı.
 *
 * İki katmanı AYIRIR ve ayrı bloklarda tutar:
 *   1. OLCUM      : diskten/git'ten okunan, tekrar üretilebilir sayılar
 *   2. DEGERLENDIRME : varsayim.json'dan gelen ağırlık + yüzde + gerekçe
 *
 * Çıktı: port/pano/veri.json
 *
 * Kullanım: bun araclar/pano/olc-topla.ts
 */

import fs from "node:fs";
import path from "node:path";

const KOK = path.resolve(import.meta.dir, "..", "..");
const VARSAYIM = path.join(KOK, "araclar", "pano", "varsayim.json");
const CIKTI = path.join(KOK, "port", "pano", "veri.json");

type Alan = {
  id: string;
  ad: string;
  agirlik: number;
  yuzde: number;
  alt: number;
  ust: number;
  durum: "calisiyor" | "kismi" | "iskelet" | "yok";
  gerekce: string;
  kanit: string[];
};

type Varsayim = {
  _aciklama: string;
  _surum: string;
  alanlar: Alan[];
  paydalar: Array<{
    id: string;
    ad: string;
    aciklama: string;
    haric: string[];
    yeniden: Record<string, { yuzde: number; alt: number; ust: number; gerekce: string }>;
  }>;
  amaclar: Array<{ id: string; ad: string; hedef: string; bugun: number; not: string }>;
  ekosistem: {
    dugumler: Array<{ id: string; ad: string; rol: string; merkez?: boolean }>;
    baglar: Array<{ kaynak: string; hedef: string; durum: string; kanit: string; not: string }>;
  };
  olcmediklerim: string[];
};

function oku(yol: string): string | null {
  try {
    return fs.readFileSync(yol, "utf-8");
  } catch {
    return null;
  }
}

function varMi(goreli: string): boolean {
  return fs.existsSync(path.join(KOK, goreli));
}

/** Bir dizin altındaki dosyaları, verilen sonekle süzer. */
function dosyalar(goreli: string, uzanti: string): string[] {
  const tam = path.join(KOK, goreli);
  const sonuc: string[] = [];
  const gez = (d: string) => {
    let girisler: fs.Dirent[];
    try {
      girisler = fs.readdirSync(d, { withFileTypes: true });
    } catch {
      return;
    }
    for (const g of girisler) {
      const p = path.join(d, g.name);
      if (g.isDirectory()) gez(p);
      else if (g.name.endsWith(uzanti)) sonuc.push(path.relative(KOK, p));
    }
  };
  gez(tam);
  return sonuc;
}

function satirSayisi(dosya: string[]): number {
  let n = 0;
  for (const d of dosya) {
    const icerik = oku(path.join(KOK, d));
    if (icerik) n += icerik.split("\n").length;
  }
  return n;
}

// ---------------------------------------------------------------- ÖLÇÜM

const olcum: Record<string, unknown> = {};

// Rust hacmi
const rustDosya = dosyalar("crates", ".rs");
olcum.rust = {
  dosya: rustDosya.length,
  satir: satirSayisi(rustDosya),
  crate: (() => {
    try {
      return fs
        .readdirSync(path.join(KOK, "crates"), { withFileTypes: true })
        .filter((g) => g.isDirectory()).length;
    } catch {
      return 0;
    }
  })(),
};

// Test sayısı
let testSayisi = 0;
let entegrasyonTesti = 0;
for (const d of rustDosya) {
  const icerik = oku(path.join(KOK, d));
  if (!icerik) continue;
  testSayisi += (icerik.match(/^\s*#\[(tokio::)?test\]/gm) ?? []).length;
}
for (const d of dosyalar(path.join("crates"), ".rs")) {
  if (d.includes(`${path.sep}tests${path.sep}`)) entegrasyonTesti += 1;
}

olcum.test = {
  birim: testSayisi,
  entegrasyon_dosyasi: entegrasyonTesti,
};

// Kapı altyapısı
const kapiBetik = dosyalar("araclar/kapilar", ".ts").filter((d) => !d.includes(".test."));
const kapiTest = dosyalar("araclar/kapilar", ".ts").filter((d) => d.includes(".test."));
olcum.kapilar = {
  betik: kapiBetik.length,
  test: kapiTest.length,
  betik_adlari: kapiBetik.map((d) => path.basename(d, ".ts")).sort(),
};

// durum.json
const durumHam = oku(path.join(KOK, "port", "durum.json"));
olcum.durum = (() => {
  if (!durumHam) return { var: false };
  try {
    const d = JSON.parse(durumHam);
    const kapilar = d.kapilar ?? [];
    const yesil = kapilar.filter((k: any) => k.cikis === 0).length;
    return {
      var: true,
      uretim_zamani: d.uretim_zamani ?? null,
      faz: d.faz ?? null,
      kapi_sayisi: kapilar.length,
      yesil,
      sayimlar: d.sayimlar ?? null,
      tum_kapilar_yesil_mi: d.tum_kapilar_yesil_mi ?? null,
    };
  } catch {
    return { var: true, okunamadi: true };
  }
})();

// uclar.toml — [[uc]] sayımı
olcum.uclar = (() => {
  const t = oku(path.join(KOK, "port", "uclar.toml"));
  if (!t) return { var: false };
  const toplam = (t.match(/^\[\[uc\]\]/gm) ?? []).length;
  const gercek = (t.match(/durum\s*=\s*"gercek"/g) ?? []).length;
  const kapsamDisi = (t.match(/durum\s*=\s*"kapsam-disi"/g) ?? []).length;
  return { var: true, toplam, gercek, kapsam_disi: kapsamDisi };
})();

// mühür + mutant
olcum.muhur = (() => {
  const t = oku(path.join(KOK, "port", "muhur.toml"));
  if (!t) return { var: false };
  return { var: true, dosya: (t.match(/^\[\[dosya\]\]/gm) ?? []).length };
})();
olcum.mutant = (() => {
  const t = oku(path.join(KOK, "port", "mutantlar.toml"));
  if (!t) return { var: false };
  return { var: true, kural: (t.match(/^\[\[mutant\]\]/gm) ?? []).length };
})();

// Sunucu yüzeyi
olcum.sunucu = (() => {
  const t = oku(path.join(KOK, "crates", "sunucu", "src", "server.rs"));
  if (!t) return { var: false };
  return { var: true, route: (t.match(/\.route\(/g) ?? []).length };
})();

// Sağlayıcı kaydı — p( çağrıları
olcum.saglayici = (() => {
  const t = oku(path.join(KOK, "crates", "saglayici", "src", "saglayicilar.rs"));
  if (!t) return { var: false };
  const adlar = [...t.matchAll(/^\s*p\(\s*\n?\s*"([a-z0-9-]+)"/gm)].map((m) => m[1]);
  return { var: true, yerlesik: adlar };
})();

// Yönlendirme stratejileri
olcum.strateji = (() => {
  const t = oku(path.join(KOK, "crates", "havuz", "src", "router.rs"));
  if (!t) return { var: false };
  const blok = /pub enum RoutingStrategy\s*\{([\s\S]*?)\n\}/.exec(t);
  if (!blok) return { var: true, sayi: 0 };
  const govde = blok[1]
    .split("\n")
    .map((s) => s.trim())
    .filter((s) => /^[A-Z][A-Za-z0-9]*,?$/.test(s));
  return { var: true, sayi: govde.length, adlar: govde.map((s) => s.replace(",", "")) };
})();

// Git
olcum.git = (() => {
  const { spawnSync } = require("node:child_process");
  const calis = (...a: string[]) => {
    const r = spawnSync("git", a, { cwd: KOK, encoding: "utf-8" });
    return (r.stdout ?? "").trim();
  };
  return {
    commit: Number(calis("rev-list", "--count", "HEAD") || 0),
    ilk: calis("log", "--reverse", "--format=%ad|%h|%s", "--date=short").split("\n")[0] ?? "",
    son: calis("log", "-1", "--format=%ad|%H|%s", "--date=iso") ?? "",
    dal: calis("rev-parse", "--abbrev-ref", "HEAD") ?? "",
    kirli: calis("status", "--porcelain").split("\n").filter(Boolean).length,
  };
})();

// Canlı yapılandırma (varsa) — yalnız bölüm başlıkları, değer okunmaz
olcum.canli_yapilandirma = (() => {
  const yol = "C:\\Users\\buzbe\\Desktop\\mcp-tools\\kervan\\kervan.toml";
  const t = oku(yol);
  if (!t) return { var: false, yol };
  return {
    var: true,
    yol,
    satir: t.split("\n").length,
    bolumler: [...t.matchAll(/^\[+([^\]]+)\]+\s*$/gm)].map((m) => m[1]),
  };
})();

// Çalışma durumu
olcum.calisma = (() => {
  const ikili = ["target/debug/kervan.exe", "target/release/kervan.exe"].map((g) => {
    const tam = path.join(KOK, g);
    try {
      const s = fs.statSync(tam);
      return { yol: g, bayt: s.size, tarih: s.mtime.toISOString().slice(0, 10) };
    } catch {
      return null;
    }
  });
  return { ikili: ikili.filter(Boolean) };
})();

// Belge gerçekliği — bayat iddialar
olcum.belge = (() => {
  const pr = oku(path.join(KOK, "PR.md"));
  const log = path.join(KOK, "kervan_gui.log");
  let logBayt = 0;
  try {
    logBayt = fs.statSync(log).size;
  } catch {}
  return {
    pr_md_var: pr !== null,
    pr_md_bayt: pr ? Buffer.byteLength(pr) : 0,
    kervan_gui_log_bayt: logBayt,
    envanter_var: varMi("port/envanter.json"),
  };
})();

// ---------------------------------------------------------------- DEĞERLENDİRME

const varsayim: Varsayim = JSON.parse(fs.readFileSync(VARSAYIM, "utf-8"));

function paydaHesapla(p: Varsayim["paydalar"][number]) {
  let agirlik = 0;
  let toplam = 0;
  let altToplam = 0;
  let ustToplam = 0;
  const satirlar: Array<{ id: string; ad: string; agirlik: number; yuzde: number; disarida: boolean }> = [];

  for (const a of varsayim.alanlar) {
    const disarida = p.haric.includes(a.id);
    const yeniden = p.yeniden[a.id];
    const yuzde = yeniden?.yuzde ?? a.yuzde;
    const alt = yeniden?.alt ?? a.alt;
    const ust = yeniden?.ust ?? a.ust;
    if (disarida) {
      satirlar.push({ id: a.id, ad: a.ad, agirlik: 0, yuzde, disarida: true });
      continue;
    }
    agirlik += a.agirlik;
    toplam += a.agirlik * yuzde;
    altToplam += a.agirlik * alt;
    ustToplam += a.agirlik * ust;
    satirlar.push({ id: a.id, ad: a.ad, agirlik: a.agirlik, yuzde, disarida: false });
  }

  return {
    id: p.id,
    ad: p.ad,
    aciklama: p.aciklama,
    yuzde: agirlik ? toplam / agirlik : 0,
    alt: agirlik ? altToplam / agirlik : 0,
    ust: agirlik ? ustToplam / agirlik : 0,
    agirlik,
    satirlar,
  };
}

const paydalar = varsayim.paydalar.map(paydaHesapla);

const veri = {
  uretim: new Date().toISOString(),
  kaynak: {
    varsayim: path.relative(KOK, VARSAYIM),
    varsayim_surum: varsayim._surum,
  },
  uyari: varsayim._aciklama,
  olcum,
  degerlendirme: {
    alanlar: varsayim.alanlar,
    paydalar,
    amaclar: varsayim.amaclar,
  },
  ekosistem: varsayim.ekosistem,
  olcmediklerim: varsayim.olcmediklerim,
};

fs.mkdirSync(path.dirname(CIKTI), { recursive: true });
fs.writeFileSync(CIKTI, JSON.stringify(veri, null, 2), "utf-8");

// ---------------------------------------------------------------- ÖZET (stdout)

const b = (n: number) => n.toFixed(1);
console.log(`Kervan panosu verisi yazıldı: ${path.relative(KOK, CIKTI)}`);
console.log("");
console.log("ÖLÇÜM");
console.log(`  Rust           ${olcum.rust.dosya} dosya · ${(olcum.rust as any).satir} satır · ${(olcum.rust as any).crate} crate`);
console.log(`  Test           ${(olcum.test as any).birim} birim · ${(olcum.test as any).entegrasyon_dosyasi} entegrasyon dosyası`);
console.log(`  Kapı           ${(olcum.kapilar as any).betik} betik · ${(olcum.kapilar as any).test} test`);
console.log(`  Mühür/mutant   ${(olcum.muhur as any).dosya ?? "?"} / ${(olcum.mutant as any).kural ?? "?"}`);
console.log(`  Uçlar          ${(olcum.uclar as any).toplam} kayıt · ${(olcum.uclar as any).gercek} gerçek`);
console.log(`  Strateji       ${(olcum.strateji as any).sayi}`);
console.log(`  Git            ${(olcum.git as any).commit} commit · dal ${(olcum.git as any).dal} · kirli ${(olcum.git as any).kirli}`);
console.log("");
console.log("DEĞERLENDİRME (ağırlıklı, ölçüm değil)");
for (const p of paydalar) {
  console.log(`  ${p.ad.padEnd(28)} %${b(p.yuzde)}  (${b(p.alt)}–${b(p.ust)})  ağırlık ${p.agirlik}`);
}
