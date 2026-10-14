// Küme 'defter' (c1): defter-kontrol, durum-kontrol/durum-uret, temiz-kopya güven kökü testleri.
// Her test geçici dosyasını os.tmpdir() altında adında 'c1-defter' geçen bir dizinde açar; depo ağacına yazmaz.
import { afterAll, describe, expect, it, setDefaultTimeout } from "bun:test";
import { spawnSync } from "child_process";
import crypto from "crypto";
import fs from "fs";
import os from "os";
import path from "path";
import { defteriDenetle } from "./defter-kontrol.ts";
import { durumDenetle } from "./durum-kontrol.ts";
import { temizKopyadaKos } from "./temiz-kopya.ts";

const KAPI_DIZINI = import.meta.dir;
const KOK_TMP = fs.mkdtempSync(path.join(os.tmpdir(), "c1-defter-"));
// defter-kontrol kanıtı ÖLÇER (bun test koşusu, cargo derlemesi): 5 sn varsayılanı yetmez.
setDefaultTimeout(180000);
// Fikstür crate'leri için ortak cargo hedef dizini (os.tmpdir altında, küme etiketli).
const CARGO_HEDEF = path.join(KOK_TMP, "cargo-hedef");
afterAll(() => {
  try {
    fs.rmSync(KOK_TMP, { recursive: true, force: true });
  } catch {}
});

let sayac = 0;
function yeniDizin(ad: string): string {
  const d = path.join(KOK_TMP, `${ad}-${++sayac}`);
  fs.mkdirSync(d, { recursive: true });
  return d;
}
function yaz(kok: string, yol: string, icerik: string) {
  const tam = path.join(kok, yol);
  fs.mkdirSync(path.dirname(tam), { recursive: true });
  fs.writeFileSync(tam, icerik, "utf-8");
}
function satirlar(n: number): string {
  return Array.from({ length: n }, (_, i) => `const s${i + 1} = ${i + 1};`).join("\n") + "\n";
}
function jsonl(kayitlar: object[]): string {
  return kayitlar.map((k) => JSON.stringify(k)).join("\n") + "\n";
}

// ---------------------------------------------------------------- defter

// Geçerli bir defter ağacı: 450 satırlık kaynak, iki pencere, gerçek test + gerçek kayıt dosyası.
function defterAgaci(degistir?: { dosyalar?: object[]; davranislar?: object[]; refYok?: boolean }) {
  const kok = yeniDizin("defter");
  if (!degistir?.refYok) yaz(kok, "ref/omniroute-dbe703a/open-sse/a.ts", satirlar(450));
  yaz(kok, "Cargo.toml", '[workspace]\nmembers = ["crates/ornek"]\nresolver = "3"\n');
  yaz(kok, "crates/ornek/Cargo.toml", '[package]\nname = "ornek"\nversion = "0.1.0"\nedition = "2024"\n');
  yaz(kok, "crates/ornek/src/lib.rs", "pub fn x() {}\n");
  yaz(kok, "crates/ornek/tests/t.rs", "#[test]\nfn gercek_test_adi() {\n    assert!(true);\n}\n");
  yaz(kok, "port/kayit/P6/parite.log", "ham çıktı\n");
  const dosyalar = degistir?.dosyalar ?? [
    { id: "D1", kaynak: "open-sse/a.ts", satir: [1, 300], durum: "dogrulandi", hedef: "crates/ornek/src/lib.rs", kanit: ["gercek_test_adi"] },
    { id: "D2", kaynak: "open-sse/a.ts", satir: [290, 450], durum: "dogrulandi", hedef: "crates/ornek/src/lib.rs", kanit: ["port/kayit/P6/parite.log"] },
  ];
  const davranislar = degistir?.davranislar ?? [
    { id: "B1", tanim: "örnek davranış", kaynak: "open-sse/a.ts", satir: [10, 120], durum: "dogrulandi", hedef: "crates/ornek/src/lib.rs", kanit: ["gercek_test_adi"] },
  ];
  yaz(kok, "port/defter/dosyalar.jsonl", jsonl(dosyalar));
  yaz(kok, "port/defter/davranislar.jsonl", jsonl(davranislar));
  return kok;
}
function defterKos(kok: string, final = false) {
  // Eski imzayla uyumlu çağrı: (dosyalar yolu, final, kök)
  return defteriDenetle(path.join(kok, "port", "defter", "dosyalar.jsonl"), final, kok, { cargoHedefDizini: CARGO_HEDEF });
}

describe("c1::defter", () => {
  it("c1::defter_gecerli_agac_yesil", () => {
    const s = defterKos(defterAgaci());
    expect(s.hatalar).toEqual([]);
    expect(s.uygun).toBe(true);
  });

  it("c1::defter_sahte_kanit_kimligi_kirmizi", () => {
    const kok = defterAgaci({
      dosyalar: [
        { id: "D1", kaynak: "open-sse/a.ts", satir: [1, 300], durum: "dogrulandi", hedef: "crates/ornek/src/lib.rs", kanit: ["uydurma_test_kimligi"] },
        { id: "D2", kaynak: "open-sse/a.ts", satir: [301, 450], durum: "kapsam-disi", gerekce: "örnek" },
      ],
    });
    const s = defterKos(kok);
    expect(s.uygun).toBe(false);
    expect(s.hatalar.some((h) => h.includes("uydurma_test_kimligi"))).toBe(true);
  });

  it("c1::defter_kanit_olarak_kayit_disi_dosya_kirmizi", () => {
    // Var olan ama test adı/port/kayit/port/altin olmayan dosya kanıt sayılmaz.
    const kok = defterAgaci({
      dosyalar: [
        { id: "D1", kaynak: "open-sse/a.ts", satir: [1, 300], durum: "dogrulandi", hedef: "crates/ornek/src/lib.rs", kanit: ["crates/ornek/tests/t.rs"] },
        { id: "D2", kaynak: "open-sse/a.ts", satir: [301, 450], durum: "dogrulandi", hedef: "crates/ornek/src/lib.rs", kanit: ["gercek_test_adi"] },
      ],
    });
    expect(defterKos(kok).uygun).toBe(false);
  });

  it("c1::defter_eksik_pencere_sonu_kirmizi", () => {
    const kok = defterAgaci({
      dosyalar: [{ id: "D1", kaynak: "open-sse/a.ts", satir: [1, 300], durum: "kapsam-disi", gerekce: "örnek" }],
    });
    const s = defterKos(kok);
    expect(s.uygun).toBe(false);
    expect(s.hatalar.some((h) => h.includes("[301, 450]"))).toBe(true);
  });

  it("c1::defter_300_satiri_asan_pencere_kirmizi", () => {
    const kok = defterAgaci({
      dosyalar: [{ id: "D1", kaynak: "open-sse/a.ts", satir: [1, 450], durum: "kapsam-disi", gerekce: "örnek" }],
    });
    expect(defterKos(kok).uygun).toBe(false);
  });

  it("c1::defter_refte_olmayan_kaynak_kirmizi", () => {
    const kok = defterAgaci({
      dosyalar: [
        { id: "D1", kaynak: "open-sse/a.ts", satir: [1, 300], durum: "kapsam-disi", gerekce: "örnek" },
        { id: "D2", kaynak: "open-sse/a.ts", satir: [301, 450], durum: "kapsam-disi", gerekce: "örnek" },
        { id: "D3", kaynak: "ref/omniroute-dbe703a/src/lib/stealth/detector.ts", satir: [1, 300], durum: "kapsam-disi", gerekce: "örnek" },
      ],
    });
    const s = defterKos(kok);
    expect(s.uygun).toBe(false);
    expect(s.hatalar.some((h) => h.includes("detector.ts"))).toBe(true);
  });

  it("c1::defter_ref_dizini_yoksa_kirmizi", () => {
    const s = defterKos(defterAgaci({ refYok: true }));
    expect(s.uygun).toBe(false);
    expect(s.hatalar.some((h) => h.includes("ref"))).toBe(true);
  });

  it("c1::defter_bozuk_jsonl_satiri_kirmizi", () => {
    const kok = defterAgaci();
    fs.appendFileSync(path.join(kok, "port/defter/dosyalar.jsonl"), '{"id":"D9", "durum": dogrulandi\n');
    const s = defterKos(kok);
    expect(s.uygun).toBe(false);
    expect(s.hatalar.some((h) => h.includes("ayrıştırılamadı"))).toBe(true);
  });

  it("c1::defter_bos_defter_kirmizi", () => {
    const kok = defterAgaci({ dosyalar: [] });
    fs.writeFileSync(path.join(kok, "port/defter/dosyalar.jsonl"), "");
    expect(defterKos(kok).uygun).toBe(false);
  });

  it("c1::defter_davranislar_jsonl_de_denetlenir", () => {
    const kok = defterAgaci({
      davranislar: [{ id: "B1", tanim: "x", kaynak: "open-sse/a.ts", satir: [10, 20], durum: "dogrulandi", hedef: "crates/ornek/src/lib.rs", kanit: ["uydurma_davranis_kaniti"] }],
    });
    const s = defterKos(kok);
    expect(s.uygun).toBe(false);
    expect(s.hatalar.some((h) => h.includes("uydurma_davranis_kaniti"))).toBe(true);
  });

  it("c1::defter_dogrulandi_dosya_tamami_okunmadan_kirmizi", () => {
    // DOS-1 sınıfı: [1,300] dogrulandi ama 301-450 hâlâ planlandi (okunmamış).
    const kok = defterAgaci({
      dosyalar: [
        { id: "D1", kaynak: "open-sse/a.ts", satir: [1, 300], durum: "dogrulandi", hedef: "crates/ornek/src/lib.rs", kanit: ["gercek_test_adi"] },
        { id: "D2", kaynak: "open-sse/a.ts", satir: [301, 450], durum: "planlandi" },
      ],
      davranislar: [{ id: "B1", tanim: "x", kaynak: "open-sse/a.ts", satir: [10, 20], durum: "planlandi" }],
    });
    const s = defterKos(kok);
    expect(s.uygun).toBe(false);
    expect(s.hatalar.some((h) => h.includes("okunmamış"))).toBe(true);
  });

  it("c1::defter_davranis_araligi_okunmamis_pencerede_kirmizi", () => {
    const kok = defterAgaci({
      dosyalar: [
        { id: "D1", kaynak: "open-sse/a.ts", satir: [1, 300], durum: "planlandi" },
        { id: "D2", kaynak: "open-sse/a.ts", satir: [301, 450], durum: "planlandi" },
      ],
      davranislar: [{ id: "B1", tanim: "x", kaynak: "open-sse/a.ts", satir: [10, 20], durum: "dogrulandi", hedef: "crates/ornek/src/lib.rs", kanit: ["gercek_test_adi"] }],
    });
    expect(defterKos(kok).uygun).toBe(false);
  });

  it("c1::defter_final_planlandi_kirmizi_ve_cli_sayim_yazar", () => {
    const kok = defterAgaci({
      dosyalar: [
        { id: "D1", kaynak: "open-sse/a.ts", satir: [1, 300], durum: "planlandi" },
        { id: "D2", kaynak: "open-sse/a.ts", satir: [301, 450], durum: "planlandi" },
      ],
      davranislar: [{ id: "B1", tanim: "x", kaynak: "open-sse/a.ts", satir: [10, 20], durum: "planlandi" }],
    });
    expect(defterKos(kok, false).uygun).toBe(true);
    expect(defterKos(kok, true).uygun).toBe(false);

    const betik = path.join(KAPI_DIZINI, "defter-kontrol.ts");
    const normal = spawnSync("bun", [betik, "--kok", kok], { encoding: "utf-8", cwd: kok });
    const nCikti = (normal.stdout || "") + (normal.stderr || "");
    expect(normal.status).toBe(0);
    expect(nCikti).toMatch(/toplam\s*[:=]\s*3/);
    expect(nCikti).toMatch(/kapanmayan\s*[:=]\s*3/);
    expect(nCikti.trim().split(/\r?\n/).pop()).toBe("HUKUM: yesil");

    const fin = spawnSync("bun", [betik, "--kok", kok, "--final"], { encoding: "utf-8", cwd: kok });
    expect(fin.status).not.toBe(0);
    expect(((fin.stdout || "") + (fin.stderr || "")).trim().split(/\r?\n/).pop()).toMatch(/^HUKUM: kirmizi \S/);
  });
});

// Yargıç engeli: test adı kanıtı derlenen ve koşan bir teste çözülmeli (yorum/dizge/#[ignore]/kopuk dosya değil).
function kanitAgaci(kanit: string, ekDosyalar: Record<string, string>, hedef = "crates/ornek/src/lib.rs") {
  const kok = defterAgaci({
    dosyalar: [
      { id: "D1", kaynak: "open-sse/a.ts", satir: [1, 300], durum: "dogrulandi", hedef, kanit: [kanit] },
      { id: "D2", kaynak: "open-sse/a.ts", satir: [301, 450], durum: "kapsam-disi", gerekce: "örnek" },
    ],
    davranislar: [{ id: "B1", tanim: "x", kaynak: "open-sse/a.ts", satir: [10, 20], durum: "kapsam-disi", gerekce: "örnek" }],
  });
  for (const [y, i] of Object.entries(ekDosyalar)) yaz(kok, y, i);
  return defterKos(kok, true);
}

describe("c1::defter_kanit_cozumu", () => {
  it("c1::defter_kanit_yorumdaki_test_kirmizi", () => {
    const s = kanitAgaci("yorumdaki_sahte_test", { "crates/ornek/tests/y.rs": "// #[test] fn yorumdaki_sahte_test() {}\n/* #[test]\nfn yorumdaki_sahte_test() {} */\n" });
    expect(s.uygun).toBe(false);
  });
  it("c1::defter_kanit_dizgedeki_test_kirmizi", () => {
    const s = kanitAgaci("dizgedeki_sahte", { "crates/ornek/tests/d.rs": 'const S: &str = "#[test] fn dizgedeki_sahte() {}";\nconst R: &str = r#"#[test] fn dizgedeki_sahte() {}"#;\n' });
    expect(s.uygun).toBe(false);
  });
  it("c1::defter_kanit_ignore_test_kirmizi", () => {
    const s = kanitAgaci("hic_kosmayan", { "crates/ornek/tests/i.rs": "#[test]\n#[ignore]\nfn hic_kosmayan() {}\n" });
    expect(s.uygun).toBe(false);
  });
  it("c1::defter_kanit_crate_e_bagli_olmayan_dosya_kirmizi", () => {
    // src/kopuk.rs hiçbir 'mod' ile bağlanmamış: derlenmez, testi koşmaz.
    const s = kanitAgaci("kopuk_test", { "crates/ornek/src/kopuk.rs": "#[test]\nfn kopuk_test() {}\n", "crates/baska/tests/b.rs": "#[test]\nfn kopuk_test() {}\n" });
    expect(s.uygun).toBe(false);
  });
  it("c1::defter_kanit_bun_yorumdaki_test_kirmizi", () => {
    const s = kanitAgaci("bun_yorum_sahte", { "araclar/x/s.test.ts": '// it("bun_yorum_sahte", () => {});\nconst a = \'it("bun_yorum_sahte", () => {})\';\nit.skip("bun_yorum_sahte", () => {});\n' });
    expect(s.uygun).toBe(false);
  });
  it("c1::defter_hedef_dizin_ise_kirmizi", () => {
    const s = kanitAgaci("gercek_test_adi", {}, "crates");
    expect(s.uygun).toBe(false);
    expect(s.hatalar.some((h) => h.includes("hedef"))).toBe(true);
  });
  it("c1::defter_kanit_mod_agacindaki_gercek_test_yesil", () => {
    // lib.rs → mod ic; → ic.rs içindeki #[cfg(test)] mod tests { #[test] fn ... }: modül yoluyla da çözülür.
    const s = kanitAgaci("ic::tests::ic_test", {
      "crates/ornek/src/lib.rs": "pub fn x() {}\nmod ic;\n",
      "crates/ornek/src/ic.rs": 'pub fn y() -> &\'static str { "}" }\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn ic_test() { let _c = \'{\'; }\n}\n',
    });
    expect(s.hatalar).toEqual([]);
  });
});

// ---------------------------------------------------------------- durum

function sha(s: string | Buffer) {
  return crypto.createHash("sha256").update(s).digest("hex");
}
function kapiLogu(ad: string, cikis: number, hukum: string) {
  return `=== KAPI BAŞLADI: ${ad} ===\nKomut: bun araclar/kapilar/${ad}.ts\nZaman: 2026-10-08T23:59:00.000Z\n\nHUKUM: ${hukum}\n\n=== KAPI BİTTİ: Çıkış Kodu = ${cikis} (Süre: 12 ms) ===\n=== KOŞUCU HÜKMÜ: ${hukum} ===\n`;
}
function durumAgaci(durumDegistir?: (d: any) => any) {
  const kok = yeniDizin("durum");
  const hepsi = "// sahte hepsi.ts\nconsole.log(1);\n";
  yaz(kok, "araclar/kapilar/hepsi.ts", hepsi);
  yaz(kok, "port/uclar.toml", '[[uc]]\nyol = "/v1/models"\nyontem = "GET"\ndurum = "501"\ntest = "t"\n');
  yaz(kok, "port/defter/dosyalar.jsonl", jsonl([
    { id: "D1", kaynak: "a.ts", satir: [1, 300], durum: "planlandi" },
    { id: "D2", kaynak: "a.ts", satir: [301, 400], durum: "dogrulandi" },
  ]));
  yaz(kok, "port/defter/davranislar.jsonl", jsonl([{ id: "B1", tanim: "x", durum: "kapsam-disi", gerekce: "örnek" }]));
  // Ölçülen envanter: ref içinde 3 route.ts (node_modules sayılmaz).
  for (const r of ["app/api/a", "app/api/b", "app/v1/c"]) yaz(kok, `ref/omniroute-dbe703a/${r}/route.ts`, "export {}\n");
  yaz(kok, "ref/omniroute-dbe703a/node_modules/x/route.ts", "export {}\n");
  // hepsi.ts biçiminde gerçek kapı günlüğü.
  yaz(kok, "port/kayit/P2/ulasilabilirlik.log", kapiLogu("ulasilabilirlik", 0, "yesil"));
  // Ölçülen kapı listesi (kapilar.toml) ve git'te çözülen faz commit'i.
  const toml = '[[faz.P4]]\nad = "ulasilabilirlik"\nkomut = "bun araclar/kapilar/ulasilabilirlik.ts"\n';
  yaz(kok, "araclar/kapilar/kapilar.toml", toml);
  git(kok, "init", "-q");
  git(kok, "add", "-A");
  git(kok, "commit", "-q", "-m", "P4");
  let durum: any = {
    uretici_ozeti: "sha256:" + sha(hepsi),
    kapilar_toml_ozeti: "sha256:" + sha(toml),
    uretim_zamani: "2026-10-09T00:00:00.000Z",
    faz: "P4",
    kip: "faz",
    kapilar: [{ ad: "ulasilabilirlik", cikis: 0, sure_ms: 12, log: "port/kayit/P2/ulasilabilirlik.log" }],
    fazlar: [{ id: "P4", commit: git(kok, "rev-parse", "HEAD"), kanit: ["ulasilabilirlik"] }],
    sayimlar: { route_ts: 3, defter_planlandi: 1, defter_dogrulandi: 1 },
  };
  if (durumDegistir) durum = durumDegistir(durum);
  yaz(kok, "port/durum.json", typeof durum === "string" ? durum : JSON.stringify(durum, null, 2));
  // Fikstür README'leri gerçek deponun (HEAD) başlıklarını taşır: mühürlü taban başlık listesiyle karşılaştırılır.
  for (const ad of Object.keys(README_TABANI)) yaz(kok, ad, readmeFiksturu(ad));
  return kok;
}
// HEAD'deki README*.md başlıkları (durum-kontrol'ün mühürlü tabanının bağımsız kopyası).
const README_TABANI: Record<string, string[]> = {
  "README.md": ["# Kervan", "## 📦 Gereksinimler", "## 🛠 İşlev ve Özellikler", "## ⚙ Kurulum", "### Kurulum Adımları", "### Kullanım", "## 🗺 Durum ve Yol Haritası", "### Teknik Detaylar", "### Sağlayıcılar", "#### Sağlayıcı Durumu", "## ⚠️ Bilinen Sorunlar / Deneysel Kısımlar", "## 🤝 Katkıda Bulunanlar", "## 📜 Lisans"],
  "README.en.md": ["# Kervan", "## 📦 Prerequisites", "## 🛠 Features & Functionality", "## ⚙ Installation", "### Installation Steps", "### Usage", "## 🗺 Status & Roadmap", "### Technical Details", "### Providers", "#### Provider Status", "## ⚠️ Known Issues / Experimental Features", "## 🤝 Contributors", "## 📜 License"],
  "README.ar.md": ["# كرفان", "## 📦 المتطلبات", "## 🛠 الوظائف والميزات", "## ⚙ التثبيت", "### خطوات التثبيت", "### الاستخدام", "## 🗺 الحالة وخارطة الطريق", "### التفاصيل التقنية", "### المزودون", "#### حالة المزودين", "## ⚠️ المشاكل المعروفة / الميزات التجريبية", "## 🤝 المساهمون", "## 📜 الترخيص"],
  "README.ja.md": ["# キャラバン (Kervan)", "## 📦 前提条件", "## 🛠 機能と特徴", "## ⚙ セットアップ", "### セットアップ手順", "### 使用方法", "## 🗺 ステータスとロードマップ", "### 技術詳細", "### プロバイダー", "#### プロバイダーの状態", "## ⚠️ 既知の問題 / 実験的機能", "## 🤝 貢献者", "## 📜 ライセンス"],
};
function readmeFiksturu(ad: string): string {
  // 10. başlık (index 9) sağlayıcı durumu bölümüdür: gövdesi yalnız durum bloğu.
  return README_TABANI[ad].map((b, i) => (i === 9 ? `${b}\n<!-- DURUM:BASLA -->\n(boş)\n<!-- DURUM:BITIS -->\n` : `${b}\nx\n`)).join("");
}
function durumUretKos(kok: string) {
  return spawnSync("bun", [path.join(KAPI_DIZINI, "durum-uret.ts"), "--kok", kok], { encoding: "utf-8", cwd: kok });
}
function durumKos(kok: string) {
  // Eski imzayla uyumlu: (DURUM.md, README, kök)
  return durumDenetle(path.join(kok, "port", "DURUM.md"), path.join(kok, "README.md"), kok);
}
// Gövdesine kendi özetini yazan, elle yazılmış DURUM.md (eski kapının kandırılma yolu).
function elleDurumMd(govde: string) {
  return `<!-- URETILMIS_DURUM_HASH: ${sha(govde)} -->\n${govde}`;
}

describe("c1::durum", () => {
  it("c1::durum_uretilmis_agac_yesil", () => {
    const kok = durumAgaci();
    const u = durumUretKos(kok);
    expect(u.status).toBe(0);
    const s = durumKos(kok);
    expect(s.hatalar).toEqual([]);
    expect(s.uygun).toBe(true);
  });

  it("c1::durum_ozetli_ama_elle_yazilmis_durum_md_kirmizi", () => {
    const kok = durumAgaci();
    yaz(kok, "port/DURUM.md", elleDurumMd("# DURUM\nFaz 0–9 TAMAMLANDI\n"));
    expect(durumKos(kok).uygun).toBe(false);
  });

  it("c1::durum_sema_disi_durum_json_kirmizi", () => {
    // uretici_ozeti dosya yolu, sayimlar/fazlar yok: P10'daki gerçek durum.json biçimi.
    const kok = durumAgaci(() => ({ uretici_ozeti: "araclar/kapilar/hepsi.ts", uretim_zamani: "x", faz: "P10", kapilar: [] }));
    yaz(kok, "port/DURUM.md", elleDurumMd("# DURUM\n"));
    const s = durumKos(kok);
    expect(s.uygun).toBe(false);
    expect(durumUretKos(kok).status).not.toBe(0);
  });

  it("c1::durum_uretici_ozeti_hepsi_ts_ile_eslesmezse_kirmizi", () => {
    const kok = durumAgaci((d) => ({ ...d, uretici_ozeti: "sha256:" + "0".repeat(64) }));
    yaz(kok, "port/DURUM.md", elleDurumMd("# DURUM\n"));
    expect(durumKos(kok).uygun).toBe(false);
  });

  it("c1::durum_sayimlar_defterle_celisirse_kirmizi", () => {
    const kok = durumAgaci((d) => ({ ...d, sayimlar: { route_ts: 721, defter_planlandi: 0, defter_dogrulandi: 23872 } }));
    yaz(kok, "port/DURUM.md", elleDurumMd("# DURUM\n"));
    expect(durumKos(kok).uygun).toBe(false);
  });

  it("c1::durum_ayristirilamayan_durum_json_kirmizi", () => {
    const kok = durumAgaci(() => "{ bozuk json");
    yaz(kok, "port/DURUM.md", elleDurumMd("# DURUM\n"));
    expect(durumKos(kok).uygun).toBe(false);
  });

  it("c1::durum_readme_blogu_elle_duzenlenirse_kirmizi", () => {
    const kok = durumAgaci();
    const readme = path.join(kok, "README.md");
    // Blok fikstürde zaten var (Sağlayıcı Durumu altında, "(boş)"): durum-uret doldurur.
    expect(durumUretKos(kok).status).toBe(0);
    expect(durumKos(kok).uygun).toBe(true);
    const icerik = fs.readFileSync(readme, "utf-8");
    expect(icerik).not.toContain("(boş)");
    fs.writeFileSync(readme, icerik.replace("<!-- DURUM:BITIS -->", "- Tüm OmniRoute özellikleri destekleniyor ✅\n<!-- DURUM:BITIS -->"));
    expect(durumKos(kok).uygun).toBe(false);
  });

  it("c1::durum_cevrilmis_readme_blogu_da_denetlenir", () => {
    const kok = durumAgaci();
    yaz(kok, "README.en.md", "# Kervan\n<!-- DURUM:BASLA -->\nAll phases complete\n<!-- DURUM:BITIS -->\n");
    expect(durumUretKos(kok).status).toBe(0);
    yaz(kok, "README.en.md", "# Kervan\n<!-- DURUM:BASLA -->\nAll phases complete\n<!-- DURUM:BITIS -->\n");
    expect(durumKos(kok).uygun).toBe(false);
  });

  it("c1::durum_cli_son_satir_hukum", () => {
    const kok = durumAgaci();
    durumUretKos(kok);
    const p = spawnSync("bun", [path.join(KAPI_DIZINI, "durum-kontrol.ts"), "--kok", kok], { encoding: "utf-8", cwd: kok });
    expect(p.status).toBe(0);
    expect(((p.stdout || "") + (p.stderr || "")).trim().split(/\r?\n/).pop()).toBe("HUKUM: yesil");
  });
});

// ---------------------------------------------------------------- temiz-kopya

function git(dizin: string, ...arg: string[]) {
  const p = spawnSync("git", ["-c", "user.name=c1", "-c", "user.email=c1@ornek.invalid", "-c", "commit.gpgsign=false", ...arg], { cwd: dizin, encoding: "utf-8" });
  if (p.status !== 0) throw new Error(`git ${arg.join(" ")}: ${p.stderr}`);
  return p.stdout.trim();
}
// Denetlenen depo: commit'teki kapı "HEDEF-KAPISI" basar (mühürsüz, builder'ın yazdığı kapı).
function hedefDepo() {
  const depo = yeniDizin("tk-depo");
  git(depo, "init", "-q");
  yaz(depo, "araclar/kapilar/kapi.ts", 'console.log("HEDEF-KAPISI");\n');
  yaz(depo, "veri.txt", "x\n");
  git(depo, "add", "-A");
  git(depo, "commit", "-q", "-m", "hedef");
  return { depo, sha: git(depo, "rev-parse", "HEAD") };
}

describe("c1::temiz_kopya", () => {
  it("c1::temiz_kopya_cagiranin_kapilarini_hedefe_karsi_kosar", () => {
    const { depo, sha: hedef } = hedefDepo();
    const kapiKaynagi = yeniDizin("tk-kapi");
    yaz(kapiKaynagi, "kapi.ts", 'import fs from "fs"; console.log("CAGIRAN-KAPISI", fs.existsSync("veri.txt"));\n');
    const s: any = (temizKopyadaKos as any)(hedef, "bun araclar/kapilar/kapi.ts", yeniDizin("tk-gecici"), { depo, kapiKaynagi });
    expect(s.stdout).toContain("CAGIRAN-KAPISI true");
    expect(s.stdout).not.toContain("HEDEF-KAPISI");
    expect(s.cikis).toBe(0);
  }, 30000);

  it("c1::temiz_kopya_cozulemeyen_ref_kirmizi", () => {
    const { depo } = hedefDepo();
    const s: any = (temizKopyadaKos as any)("olmayan-dal-c1", "bun -e 0", yeniDizin("tk-gecici"), { depo });
    expect(s.cikis).toBe(3);
    expect(String(s.neden)).toContain("ref-yok");
  }, 30000);

  it("c1::temiz_kopya_zaman_asimi_kirmizi", () => {
    const { depo, sha: hedef } = hedefDepo();
    const bas = Date.now();
    const s: any = (temizKopyadaKos as any)(hedef, 'bun -e "setTimeout(()=>{}, 15000)"', yeniDizin("tk-gecici"), { depo, kapiKaynagi: null, zamanAsimiMs: 1500 });
    expect(s.cikis).toBe(124);
    expect(Date.now() - bas).toBeLessThan(12000);
  }, 30000);

  it("c1::temiz_kopya_cli_son_satir_hukum", () => {
    const { depo, sha: hedef } = hedefDepo();
    const kapiKaynagi = yeniDizin("tk-kapi");
    yaz(kapiKaynagi, "kapi.ts", "process.exit(7);\n");
    const p = spawnSync(
      "bun",
      [path.join(KAPI_DIZINI, "temiz-kopya.ts"), "--depo", depo, "--hedef", hedef, "--kapi-kaynagi", kapiKaynagi, "--gecici-kok", yeniDizin("tk-gecici"), "--komut", "bun araclar/kapilar/kapi.ts"],
      { encoding: "utf-8", cwd: depo }
    );
    expect(p.status).toBe(7);
    expect(((p.stdout || "") + (p.stderr || "")).trim().split(/\r?\n/).pop()).toMatch(/^HUKUM: kirmizi \S/);
  }, 30000);
});

// ---------------------------------------------------------------- yargıç engelleri: durum ölçümü

describe("c1::durum_olcum", () => {
  it("c1::durum_olmayan_log_kirmizi", () => {
    const kok = durumAgaci((d) => ({ ...d, kapilar: [{ ad: "ulasilabilirlik", cikis: 0, sure_ms: 1, log: "port/kayit/YOK/olmayan.log" }], tum_kapilar_yesil_mi: true }));
    expect(durumUretKos(kok).status).toBe(0);
    const s = durumKos(kok);
    expect(s.uygun).toBe(false);
    expect(s.hatalar.some((h) => h.includes("olmayan.log"))).toBe(true);
  });
  it("c1::durum_log_cikisi_celisirse_kirmizi", () => {
    const kok = durumAgaci();
    yaz(kok, "port/kayit/P2/ulasilabilirlik.log", kapiLogu("ulasilabilirlik", 1, "kirmizi düştü"));
    expect(durumUretKos(kok).status).toBe(0);
    expect(durumKos(kok).uygun).toBe(false);
  });
  it("c1::durum_route_ts_envanterle_celisirse_kirmizi", () => {
    const kok = durumAgaci((d) => ({ ...d, sayimlar: { ...d.sayimlar, route_ts: 999999 } }));
    expect(durumUretKos(kok).status).toBe(0);
    const s = durumKos(kok);
    expect(s.uygun).toBe(false);
    expect(s.hatalar.some((h) => h.includes("route_ts"))).toBe(true);
  });
  it("c1::durum_readme_blogu_yoksa_kirmizi", () => {
    const kok = durumAgaci();
    yaz(kok, "README.md", "# Kervan\n## Gereksinimler\nx\n## Kurulum\nx\n### Kullanım\nx\n#### Sağlayıcı Durumu\n| REST | ✅ destekleniyor |\n## Bilinen Sorunlar\nx\n");
    durumUretKos(kok);
    expect(durumKos(kok).uygun).toBe(false);
  });
  it("c1::durum_ceviri_readme_blogu_yoksa_kirmizi", () => {
    const kok = durumAgaci();
    yaz(kok, "README.en.md", "# Kervan\n## Provider Status\n| REST | ✅ supported |\n");
    durumUretKos(kok);
    expect(durumKos(kok).uygun).toBe(false);
  });
  it("c1::durum_saglayici_bolumunde_elle_tablo_kirmizi", () => {
    // Blok üretilmiş, ama aynı bölümde elle yazılmış durum tablosu duruyor.
    const kok = durumAgaci();
    expect(durumUretKos(kok).status).toBe(0);
    const readme = path.join(kok, "README.md");
    fs.writeFileSync(readme, fs.readFileSync(readme, "utf-8").replace("<!-- DURUM:BITIS -->\n", "<!-- DURUM:BITIS -->\n| REST | ✅ destekleniyor |\n"));
    expect(durumKos(kok).uygun).toBe(false);
  });
});

// ---------------------------------------------------------------- yargıç engelleri: temiz-kopya

describe("c1::temiz_kopya_guven_koku", () => {
  it("c1::temiz_kopya_varsayilan_kapilar_HEAD_den_gelir_calisma_agacindan_degil", () => {
    // A: denetlenen commit (mühürsüz kapı). B = HEAD: mühürlü kapı. Çalışma ağacında commit'lenmemiş KIRLI kapı.
    const { depo, sha: hedef } = hedefDepo();
    yaz(depo, "araclar/kapilar/kapi.ts", 'console.log("HEAD-MUHURLU");\n');
    git(depo, "add", "-A");
    git(depo, "commit", "-q", "-m", "muhur");
    yaz(depo, "araclar/kapilar/kapi.ts", 'console.log("KIRLI-CALISMA-AGACI");\n');
    const s = temizKopyadaKos(hedef, "bun araclar/kapilar/kapi.ts", yeniDizin("tk-gecici"), { depo });
    expect(s.stdout).toContain("HEAD-MUHURLU");
    expect(s.stdout).not.toContain("KIRLI-CALISMA-AGACI");
    expect(s.stdout).not.toContain("HEDEF-KAPISI");
    expect(s.cikis).toBe(0);
  }, 60000);

  it("c1::temiz_kopya_hedefin_bunfig_preload_kancasi_kirmizi", () => {
    // Yargıcın denemesi: hedef commit bunfig.toml preload ile process.exit'i 0'a çeviriyor.
    const depo = yeniDizin("tk-kanca");
    git(depo, "init", "-q");
    yaz(depo, "bunfig.toml", 'preload = ["./kanca.ts"]\n');
    yaz(depo, "kanca.ts", 'const e = process.exit.bind(process); (process as any).exit = (c?: number) => { console.log("KANCA: cikis " + c + " -> 0"); e(0); };\n');
    yaz(depo, "araclar/kapilar/kapi.ts", 'console.log("HEDEF-KAPISI");\n');
    git(depo, "add", "-A");
    git(depo, "commit", "-q", "-m", "kanca");
    const hedef = git(depo, "rev-parse", "HEAD");
    const kapiKaynagi = yeniDizin("tk-kapi");
    yaz(kapiKaynagi, "kapi.ts", 'console.log("CAGIRAN-KAPISI: kirmizi olmali"); process.exit(1);\n');
    const s = temizKopyadaKos(hedef, "bun araclar/kapilar/kapi.ts", yeniDizin("tk-gecici"), { depo, kapiKaynagi });
    expect(s.stdout).not.toContain("KANCA");
    expect(s.cikis).not.toBe(0);
    expect(s.hukum).toBe("kirmizi");
  }, 60000);

  it("c1::temiz_kopya_kapiyi_kosu_sirasinda_degistiren_hedef_kirmizi", () => {
    const { depo, sha: hedef } = hedefDepo();
    const kapiKaynagi = yeniDizin("tk-kapi");
    yaz(kapiKaynagi, "kapi.ts", 'console.log("CAGIRAN-KAPISI");\n');
    const komut = `bun -e "require('fs').writeFileSync('araclar/kapilar/kapi.ts', 'process.exit(0)')"`;
    const s = temizKopyadaKos(hedef, komut, yeniDizin("tk-gecici"), { depo, kapiKaynagi });
    expect(s.hukum).toBe("kirmizi");
  }, 60000);

  it("c1::temiz_kopya_cozulemeyen_kapi_ref_kirmizi", () => {
    const { depo, sha: hedef } = hedefDepo();
    const s: any = (temizKopyadaKos as any)(hedef, "bun -e 0", yeniDizin("tk-gecici"), { depo, kapiRef: "olmayan-muhur-ref" });
    expect(s.hukum).toBe("kirmizi");
    expect(s.cikis).not.toBe(0);
  }, 60000);
});

// ---------------------------------------------------------------- yargıç engelleri (tur 3)

describe("c1::defter_tur3", () => {
  it("c1::defter_kanit_yol_gecisi_kayit_disina_kirmizi", () => {
    // 'port/kayit/' öneki dizge olarak tutuyor ama yol kökteki Cargo.toml'a çözülüyor.
    for (const kanit of ["port/kayit/../../Cargo.toml", "port/altin/../defter/dosyalar.jsonl", "port\\kayit\\..\\..\\Cargo.toml"]) {
      const s = kanitAgaci(kanit, {});
      expect(s.uygun).toBe(false);
      expect(s.hatalar.some((h) => h.includes(kanit))).toBe(true);
    }
  });

  it("c1::defter_kanit_describe_skip_icindeki_bun_testi_kirmizi", () => {
    for (const sar of ["describe.skip", "describe.todo", "describe.skipIf(true)", "describe.if(false)"]) {
      const s = kanitAgaci("sahte_kanit_adi", { "araclar/s.test.ts": `import { describe, it } from "bun:test";\n${sar}("x", () => {\n  it("sahte_kanit_adi", () => {});\n});\n` });
      expect(s.uygun).toBe(false);
    }
    // Karşı sınama: düz describe içindeki test kanıttır.
    const iyi = kanitAgaci("gercek_bun_adi", { "araclar/s.test.ts": 'import { describe, it } from "bun:test";\ndescribe("x", () => {\n  it("gercek_bun_adi", () => {});\n});\n' });
    expect(iyi.hatalar).toEqual([]);
  });

  it("c1::defter_env_dosyasi_ref_dizinini_degistiremez", () => {
    // Gerçek kaynak 450 satır, defterde yalnız [1,300]; kökteki .env sahte (300 satırlık) ref'e yönlendiriyor.
    const kok = defterAgaci({
      dosyalar: [{ id: "D1", kaynak: "open-sse/a.ts", satir: [1, 300], durum: "dogrulandi", hedef: "crates/ornek/src/lib.rs", kanit: ["gercek_test_adi"] }],
      davranislar: [{ id: "B1", tanim: "x", durum: "kapsam-disi", gerekce: "örnek" }],
    });
    const sahte = yeniDizin("sahte-ref");
    yaz(sahte, "open-sse/a.ts", satirlar(300));
    yaz(kok, ".env", `KERVAN_REF_DIZINI=${sahte.replace(/\\/g, "/")}\n`);
    const p = spawnSync("bun", [path.join(KAPI_DIZINI, "defter-kontrol.ts"), "--kok", kok], { encoding: "utf-8", cwd: kok });
    const c = (p.stdout || "") + (p.stderr || "");
    expect(p.status).not.toBe(0);
    expect(c).toContain("[301, 450]");
    expect(c.trim().split(/\r?\n/).pop()).toMatch(/^HUKUM: kirmizi \S/);
    // Süreç ortamı da yönlendiremez (in-process).
    const eski = process.env.KERVAN_REF_DIZINI;
    process.env.KERVAN_REF_DIZINI = sahte;
    try {
      expect(defterKos(kok).uygun).toBe(false);
    } finally {
      if (eski === undefined) delete process.env.KERVAN_REF_DIZINI;
      else process.env.KERVAN_REF_DIZINI = eski;
    }
  });

  it("c1::durum_env_dosyasi_route_ts_olcumunu_degistiremez", () => {
    // Gerçek ref'te 3 route.ts; .env sahte ref'e (1 route.ts) yönlendiriyor, durum.json 1 diyor.
    const kok = durumAgaci((d) => ({ ...d, sayimlar: { ...d.sayimlar, route_ts: 1 } }));
    const sahte = yeniDizin("sahte-ref");
    yaz(sahte, "app/x/route.ts", "export {}\n");
    yaz(kok, ".env", `KERVAN_REF_DIZINI=${sahte.replace(/\\/g, "/")}\n`);
    expect(durumUretKos(kok).status).toBe(0);
    const p = spawnSync("bun", [path.join(KAPI_DIZINI, "durum-kontrol.ts"), "--kok", kok], { encoding: "utf-8", cwd: kok });
    const c = (p.stdout || "") + (p.stderr || "");
    expect(p.status).not.toBe(0);
    expect(c).toContain("route_ts");
    expect(c.trim().split(/\r?\n/).pop()).toMatch(/^HUKUM: kirmizi \S/);
  });

  it("c1::temiz_kopya_hedefin_env_dosyasi_kirmizi", () => {
    const depo = yeniDizin("tk-env");
    git(depo, "init", "-q");
    yaz(depo, ".env", "KERVAN_REF_DIZINI=/sahte\n");
    yaz(depo, "araclar/kapilar/kapi.ts", 'console.log("HEDEF-KAPISI");\n');
    git(depo, "add", "-A", "-f");
    git(depo, "commit", "-q", "-m", "env");
    const hedef = git(depo, "rev-parse", "HEAD");
    const kapiKaynagi = yeniDizin("tk-kapi");
    yaz(kapiKaynagi, "kapi.ts", 'console.log("CAGIRAN");\n');
    const s = temizKopyadaKos(hedef, "bun araclar/kapilar/kapi.ts", yeniDizin("tk-gecici"), { depo, kapiKaynagi });
    expect(s.hukum).toBe("kirmizi");
    expect(s.neden).toContain(".env");
    expect(s.stdout).not.toContain("CAGIRAN");
  }, 60000);

  it("c1::temiz_kopya_cagiranin_refi_varsayilan_yolda_gorunur", () => {
    // Kapılar ref'i ortam değişkeniyle değil, <kök>/ref/omniroute-dbe703a varsayılan yolundan okur.
    const { depo, sha: hedef } = hedefDepo();
    const ref = yeniDizin("tk-ref");
    yaz(ref, "isaret.txt", "c1\n");
    const kapiKaynagi = yeniDizin("tk-kapi");
    yaz(kapiKaynagi, "kapi.ts", 'import fs from "fs"; process.exit(fs.existsSync("ref/omniroute-dbe703a/isaret.txt") ? 0 : 9);\n');
    const s = temizKopyadaKos(hedef, "bun araclar/kapilar/kapi.ts", yeniDizin("tk-gecici"), { depo, kapiKaynagi, refDizini: ref });
    expect(s.cikis).toBe(0);
    expect(fs.existsSync(path.join(ref, "isaret.txt"))).toBe(true); // bağlantı kaldırılırken çağıranın ref'i silinmez
  }, 60000);

  it("c1::temiz_kopya_hedefte_izlenen_ref_dizini_kirmizi", () => {
    const depo = yeniDizin("tk-refli");
    git(depo, "init", "-q");
    yaz(depo, "ref/omniroute-dbe703a/open-sse/a.ts", "x\n");
    yaz(depo, "araclar/kapilar/kapi.ts", 'console.log("HEDEF-KAPISI");\n');
    git(depo, "add", "-A", "-f");
    git(depo, "commit", "-q", "-m", "sahte ref");
    const hedef = git(depo, "rev-parse", "HEAD");
    const kapiKaynagi = yeniDizin("tk-kapi");
    yaz(kapiKaynagi, "kapi.ts", 'console.log("CAGIRAN");\n');
    const s = temizKopyadaKos(hedef, "bun araclar/kapilar/kapi.ts", yeniDizin("tk-gecici"), { depo, kapiKaynagi, refDizini: yeniDizin("tk-ref") });
    expect(s.hukum).toBe("kirmizi");
    expect(s.neden).toContain("ref/");
  }, 60000);
});

// ---------------------------------------------------------------- yargıç engelleri (tur 4)

describe("c1::tur4_bun_kaniti", () => {
  // Her biri: `bun test` bu dosyada adı geçen testi KOŞMAZ; kanıt sayılmamalı.
  const sahteler: Record<string, string> = {
    yardimcidan_it: 'import { it } from "./sahte-yardimci.ts";\nit("sahte_kanit_adi", () => {});\n',
    olu_kod_return: 'import { describe, it } from "bun:test";\ndescribe("x", () => {\n  return;\n  it("sahte_kanit_adi", () => {});\n});\n',
    olu_kod_if_return: 'import { describe, it } from "bun:test";\ndescribe("x", () => {\n  if (true) { return; }\n  it("sahte_kanit_adi", () => {});\n});\n',
    parametre_it: 'import { describe } from "bun:test";\ndescribe("x", (it: any) => {\n  it("sahte_kanit_adi", () => {});\n});\n',
    ustte_throw: 'import { it } from "bun:test";\nthrow new Error("dur");\nit("sahte_kanit_adi", () => {});\n',
    only_yaninda: 'import { it, test } from "bun:test";\ntest.only("baska", () => {});\nit("sahte_kanit_adi", () => {});\n',
    ad_alani_ithali: 'import * as t from "bun:test";\nit("sahte_kanit_adi", () => {});\n',
  };
  for (const [ad, icerik] of Object.entries(sahteler)) {
    it(`c1::tur4_bun_kaniti_${ad}_kirmizi`, () => {
      const s = kanitAgaci("sahte_kanit_adi", { "araclar/s.test.ts": icerik, "araclar/sahte-yardimci.ts": "export const it = (_a: string, _f: () => void) => {};\n" });
      expect(s.uygun).toBe(false);
      expect(s.hatalar.some((h) => h.includes("sahte_kanit_adi"))).toBe(true);
    });
  }
  it("c1::tur4_bun_kaniti_global_it_gercekten_kosar_yesil", () => {
    // Eski statik kural bunları kırmızı sayıyordu; bun test ikisini de koşturup GEÇİRİR (ölçüldü), kanıt sayılmaları doğru.
    for (const icerik of ['it("gercek_bun_adi", () => {});\n', 'import * as t from "bun:test";\nconst { it } = t as any;\nit("gercek_bun_adi", () => {});\n']) {
      expect(kanitAgaci("gercek_bun_adi", { "araclar/s.test.ts": icerik }).hatalar).toEqual([]);
    }
  });
  it("c1::tur4_bun_kaniti_gercek_ithal_yesil", () => {
    const s = kanitAgaci("gercek_bun_adi", { "araclar/s.test.ts": 'import { describe, expect, it } from "bun:test";\nfunction yardimci() { return 1; }\ndescribe("x", () => {\n  it("gercek_bun_adi", () => { if (yardimci()) return; throw new Error("y"); });\n});\n' });
    expect(s.hatalar).toEqual([]);
  });
});

describe("c1::tur4_defter_final_envanter", () => {
  it("c1::tur4_final_defterde_olmayan_ref_dosyasi_kirmizi", () => {
    const kok = defterAgaci();
    yaz(kok, "ref/omniroute-dbe703a/open-sse/defterde-yok.ts", satirlar(500));
    expect(defterKos(kok, false).uygun).toBe(true); // final dışı kipte envanter tamlığı aranmaz
    const s = defterKos(kok, true);
    expect(s.uygun).toBe(false);
    expect(s.hatalar.some((h) => h.includes("defterde-yok.ts"))).toBe(true);
    const p = spawnSync("bun", [path.join(KAPI_DIZINI, "defter-kontrol.ts"), "--kok", kok, "--final"], { encoding: "utf-8", cwd: kok });
    expect(p.status).not.toBe(0);
    expect(((p.stdout || "") + (p.stderr || "")).trim().split(/\r?\n/).pop()).toMatch(/^HUKUM: kirmizi \S/);
  });
});

describe("c1::tur4_durum_readme", () => {
  const readmeyeEkle = (kok: string, ad: string, sonra: string, satir: string) => {
    const y = path.join(kok, ad);
    const ic = fs.readFileSync(y, "utf-8");
    expect(ic).toContain(sonra);
    fs.writeFileSync(y, ic.replace(sonra, `${sonra}${satir}\n`));
  };
  it("c1::tur4_uclarda_gercek_olmayan_uc_destekleniyor_denirse_kirmizi", () => {
    const kok = durumAgaci(); // uclar.toml: /v1/models durum "501"
    expect(durumUretKos(kok).status).toBe(0);
    expect(durumKos(kok).uygun).toBe(true);
    readmeyeEkle(kok, "README.md", "# Kervan\n", "REST /v1/models destekleniyor.");
    readmeyeEkle(kok, "README.md", "### Kullanım\n", "- `GET /v1/models` destekleniyor");
    const s = durumKos(kok);
    expect(s.uygun).toBe(false);
    expect(s.hatalar.some((h) => h.includes("/v1/models"))).toBe(true);
  });
  it("c1::tur4_uclarda_hic_olmayan_uc_supported_denirse_kirmizi", () => {
    const kok = durumAgaci();
    expect(durumUretKos(kok).status).toBe(0);
    readmeyeEkle(kok, "README.en.md", "### Usage\n", "The http://127.0.0.1:20128/v1/embeddings endpoint is supported ✅");
    const s = durumKos(kok);
    expect(s.uygun).toBe(false);
    expect(s.hatalar.some((h) => h.includes("/v1/embeddings"))).toBe(true);
  });
  it("c1::tur4_uclarda_gercek_uc_destekleniyor_denebilir", () => {
    const kok = durumAgaci();
    yaz(kok, "port/uclar.toml", '[[uc]]\nyol = "/v1/models"\nyontem = "GET"\ndurum = "gercek"\ntest = "t"\n');
    expect(durumUretKos(kok).status).toBe(0);
    readmeyeEkle(kok, "README.md", "### Kullanım\n", "- `GET /v1/models` destekleniyor");
    const s = durumKos(kok);
    expect(s.hatalar).toEqual([]);
  });
  it("c1::tur4_readme_bolumu_silinirse_kirmizi", () => {
    for (const [ad, baslik] of [["README.md", "## 🤝 Katkıda Bulunanlar\n"], ["README.en.md", "## 📜 License\n"], ["README.ja.md", "### 技術詳細\n"]]) {
      const kok = durumAgaci();
      expect(durumUretKos(kok).status).toBe(0);
      expect(durumKos(kok).uygun).toBe(true);
      const y = path.join(kok, ad);
      fs.writeFileSync(y, fs.readFileSync(y, "utf-8").replace(baslik, ""));
      const s = durumKos(kok);
      expect(s.uygun).toBe(false);
      expect(s.hatalar.some((h) => h.includes(ad) && h.includes(baslik.trim()))).toBe(true);
    }
  });
  it("c1::tur4_ceviri_readme_dosyasi_silinirse_kirmizi", () => {
    const kok = durumAgaci();
    expect(durumUretKos(kok).status).toBe(0);
    fs.rmSync(path.join(kok, "README.ar.md"));
    const s = durumKos(kok);
    expect(s.uygun).toBe(false);
    expect(s.hatalar.some((h) => h.includes("README.ar.md"))).toBe(true);
  });
});

describe("c1::tur4_temiz_kopya", () => {
  it("c1::tur4_hedefte_buyuk_harfli_Ref_dizini_kirmizi", () => {
    // Çağıranın ref'i yok (temiz klon/CI); hedef 'Ref/omniroute-dbe703a/...' izliyor. Windows'ta (core.ignorecase) kapı bunu ref/ diye okur.
    const depo = yeniDizin("tk-Refli");
    git(depo, "init", "-q");
    yaz(depo, "Ref/omniroute-dbe703a/isaret.txt", "sahte\n");
    yaz(depo, "araclar/kapilar/kapi.ts", 'console.log("HEDEF-KAPISI");\n');
    git(depo, "add", "-A", "-f");
    git(depo, "commit", "-q", "-m", "sahte Ref");
    const hedef = git(depo, "rev-parse", "HEAD");
    const kapiKaynagi = yeniDizin("tk-kapi");
    yaz(kapiKaynagi, "kapi.ts", 'import fs from "fs"; const g = fs.existsSync("ref/omniroute-dbe703a/isaret.txt"); console.log("REF GORUNEN: " + g); process.exit(g ? 0 : 9);\n');
    const olmayanRef = path.join(yeniDizin("tk-ref-yok"), "yok");
    const s = temizKopyadaKos(hedef, "bun araclar/kapilar/kapi.ts", yeniDizin("tk-gecici"), { depo, kapiKaynagi, refDizini: olmayanRef });
    expect(s.stdout).not.toContain("REF GORUNEN: true");
    expect(s.hukum).toBe("kirmizi");
    expect(s.neden).toMatch(/ref/i);
  }, 60000);
});

describe("c1::tur4_bun_kaniti_ek", () => {
  it("c1::tur4_bun_kaniti_islevden_sonra_if_return_kirmizi", () => {
    const s = kanitAgaci("sahte_kanit_adi", { "araclar/s.test.ts": 'import { describe, it } from "bun:test";\ndescribe("x", () => {\n  function f() { return { a: 1 }; }\n  if (f()) { return; }\n  it("sahte_kanit_adi", () => {});\n});\n' });
    expect(s.uygun).toBe(false);
  });
  it("c1::tur4_bun_kaniti_nesne_iceren_yardimcidan_sonra_yesil", () => {
    const s = kanitAgaci("gercek_bun_adi", { "araclar/s.test.ts": 'import { describe, it } from "bun:test";\nfunction git(d: string, ...a: string[]) { const p = { cwd: d }; if (!p) throw new Error("x"); return p; }\ndescribe("x", () => {\n  it("gercek_bun_adi", () => { git("."); });\n});\n' });
    expect(s.hatalar).toEqual([]);
  });
});

describe("c1::tur4_bun_kaniti_regex", () => {
  it("c1::tur4_bun_kaniti_regex_icindeki_tirnak_kodu_acmaz_kirmizi", () => {
    // Regex değişmezindeki tırnak dizge sanılırsa sonraki dizgenin içi kod gibi görünür.
    const s = kanitAgaci("sahte_kanit_adi", { "araclar/s.test.ts": 'import { it } from "bun:test";\nconst r = /"/;\nconst s = " ; it(\'sahte_kanit_adi\', () => {}); ";\n' });
    expect(s.uygun).toBe(false);
  });
});

// ---------------------------------------------------------------- tur 5: koşmayan testin kanıt sayılması
describe("c1::tur5_kosmayan_test_kaniti", () => {
  const b = 'import { describe, it } from "bun:test";\n';
  const bunSahteler: Record<string, string> = {
    if_icinde_describe: b + 'if (false) describe("x", () => {\n  it("sahte_kanit_adi", () => {});\n});\n',
    ve_isleci_describe: b + 'const c = false;\nc && describe("x", () => {\n  it("sahte_kanit_adi", () => {});\n});\n',
    uclu_isleci_describe: b + 'const a = false ? describe("x", () => { it("sahte_kanit_adi", () => {}); }) : 0;\n',
    cagrilmayan_ok_islevi: b + 'const f = () => describe("x", () => { it("sahte_kanit_adi", () => {}); });\n',
    describe_icinde_if_describe: b + 'describe("a", () => {\n  if (false) describe("x", () => { it("sahte_kanit_adi", () => {}); });\n});\n',
  };
  for (const [ad, icerik] of Object.entries(bunSahteler)) {
    it(`c1::tur5_bun_${ad}_kirmizi`, () => {
      const s = kanitAgaci("sahte_kanit_adi", { "araclar/s.test.ts": icerik });
      expect(s.uygun).toBe(false);
      expect(s.hatalar.some((h) => h.includes("sahte_kanit_adi"))).toBe(true);
    });
  }
  const rsSahteler: Record<string, string> = {
    fn_govdesindeki_test: "pub fn dis() {\n    #[test]\n    fn sahte_rs_kaniti() {}\n}\n",
    makro_govdesindeki_test: "macro_rules! m {\n    () => {\n        #[test]\n        fn sahte_rs_kaniti() {}\n    };\n}\n",
    cfg_attr_ignore: "#[test]\n#[cfg_attr(all(), ignore)]\nfn sahte_rs_kaniti() {}\n",
  };
  for (const [ad, icerik] of Object.entries(rsSahteler)) {
    it(`c1::tur5_rs_${ad}_kirmizi`, () => {
      const s = kanitAgaci("sahte_rs_kaniti", { "crates/ornek/src/lib.rs": "pub fn x() {}\n" + icerik });
      expect(s.uygun).toBe(false);
      expect(s.hatalar.some((h) => h.includes("sahte_rs_kaniti"))).toBe(true);
    });
  }
  it("c1::tur5_modul_duzeyindeki_gercek_testler_yesil", () => {
    const s = kanitAgaci("ic::gercek_rs", { "crates/ornek/src/lib.rs": "pub fn x() { let _ = 1; }\nmod ic {\n    #[test]\n    fn gercek_rs() {}\n}\n" });
    expect(s.hatalar).toEqual([]);
    const t = kanitAgaci("gercek_bun_adi", { "araclar/s.test.ts": b + 'describe("x", () => {});\ndescribe("y", () => {\n  it("gercek_bun_adi", () => {});\n});\n' });
    expect(t.hatalar).toEqual([]);
  });
});

describe("c1::tur5_aralik_dosya_sonu", () => {
  it("c1::tur5_davranis_araligi_dosya_sonunu_asarsa_kirmizi", () => {
    // a.ts 450 satır; [10, 9999] beyanı dosyada olmayan satırları "doğrulandı" sayar.
    const kok = defterAgaci({
      davranislar: [{ id: "B1", tanim: "x", kaynak: "open-sse/a.ts", satir: [10, 9999], durum: "dogrulandi", hedef: "crates/ornek/src/lib.rs", kanit: ["gercek_test_adi"] }],
    });
    const s = defterKos(kok);
    expect(s.uygun).toBe(false);
    expect(s.hatalar.some((h) => h.includes("9999"))).toBe(true);
  });
  it("c1::tur5_pencere_dosya_sonunu_asarsa_kirmizi", () => {
    const kok = defterAgaci({
      dosyalar: [
        { id: "D1", kaynak: "open-sse/a.ts", satir: [1, 300], durum: "kapsam-disi", gerekce: "örnek" },
        { id: "D2", kaynak: "open-sse/a.ts", satir: [301, 600], durum: "kapsam-disi", gerekce: "örnek" },
      ],
    });
    const s = defterKos(kok);
    expect(s.uygun).toBe(false);
    expect(s.hatalar.some((h) => h.includes("600"))).toBe(true);
  });
});

// ---------------------------------------------------------------- tur 6: yargıç engelleri (ölçüm, statik ayrıştırma değil)

describe("c1::tur6_bun_kaniti_olculur", () => {
  // Her biri: `bun test` adı geçen testi koşturmaz ya da başka adla koşturur; kanıt sayılmamalı.
  const b = 'import { describe, it } from "bun:test";\n';
  const sahteler: Record<string, [string, string]> = {
    sonra_process_exit: ["S_exit", b + 'it("S_exit", () => { throw new Error("x"); });\nprocess.exit(0);\n'],
    sonra_throw: ["S_throw", b + 'it("S_throw", () => {});\nthrow new Error("ust");\n'],
    sonra_sozdizimi_hatasi: ["S_syntax", b + 'it("S_syntax", () => {});\nconst x = ;\n'],
    async_describe_asilir: ["S_async", b + 'describe("d", async () => {\n  await new Promise(() => {});\n  it("S_async", () => {});\n});\n'],
    birlestirilmis_ad: ["S_", b + 'it("S_" + "gercek", () => {});\n'],
    govdesi_dusen: ["S_duser", b + 'it("S_duser", () => { throw new Error("düştü"); });\n'],
  };
  for (const [ad, [kanit, icerik]] of Object.entries(sahteler)) {
    it(`c1::tur6_bun_${ad}_kirmizi`, () => {
      const kok = defterAgaci({
        dosyalar: [
          { id: "D1", kaynak: "open-sse/a.ts", satir: [1, 300], durum: "dogrulandi", hedef: "crates/ornek/src/lib.rs", kanit: [kanit] },
          { id: "D2", kaynak: "open-sse/a.ts", satir: [301, 450], durum: "kapsam-disi", gerekce: "örnek" },
        ],
        davranislar: [{ id: "B1", tanim: "x", kaynak: "open-sse/a.ts", satir: [10, 20], durum: "kapsam-disi", gerekce: "örnek" }],
      });
      yaz(kok, "araclar/s.test.ts", icerik);
      const s = defteriDenetle(path.join(kok, "port", "defter", "dosyalar.jsonl"), true, kok, { zamanAsimiMs: 8000, cargoHedefDizini: CARGO_HEDEF });
      expect(s.uygun).toBe(false);
      expect(s.hatalar.some((h) => h.includes(`'${kanit}'`))).toBe(true);
    }, 60000);
  }
  it("c1::tur6_bun_birlestirilmis_adin_gercek_adi_yesil", () => {
    const s = kanitAgaci("S_gercek", { "araclar/s.test.ts": b + 'it("S_gercek", () => {});\n' });
    expect(s.hatalar).toEqual([]);
  }, 60000);
});

describe("c1::tur6_cargo_kaniti_olculur", () => {
  it("c1::tur6_autotests_false_ve_lib_test_false_kirmizi", () => {
    for (const kanit of ["entegrasyon_kosmaz", "lib_testi_kosmaz", "t::lib_testi_kosmaz"]) {
      const s = kanitAgaci(kanit, {
        "crates/ornek/Cargo.toml": '[package]\nname = "ornek"\nversion = "0.1.0"\nedition = "2021"\nautotests = false\n\n[lib]\ntest = false\n',
        "crates/ornek/src/lib.rs": "pub fn x() {}\n#[cfg(test)]\nmod t {\n    #[test]\n    fn lib_testi_kosmaz() {}\n}\n",
        "crates/ornek/tests/e.rs": "#[test]\nfn entegrasyon_kosmaz() {}\n",
      });
      expect(s.uygun).toBe(false);
      expect(s.hatalar.some((h) => h.includes(`'${kanit}'`))).toBe(true);
    }
  }, 180000);
  it("c1::tur6_workspace_exclude_paketi_kirmizi", () => {
    const s = kanitAgaci("disari_testi", {
      "Cargo.toml": '[workspace]\nmembers = ["crates/*"]\nexclude = ["crates/disari"]\nresolver = "2"\n',
      "crates/disari/Cargo.toml": '[package]\nname = "disari"\nversion = "0.1.0"\nedition = "2021"\n',
      "crates/disari/src/lib.rs": "pub fn y() {}\n",
      "crates/disari/tests/x.rs": "#[test]\nfn disari_testi() {}\n",
    });
    expect(s.uygun).toBe(false);
    expect(s.hatalar.some((h) => h.includes("'disari_testi'"))).toBe(true);
  }, 180000);
  it("c1::tur6_harness_false_hedefin_listesi_kanit_degil", () => {
    // harness=false: `--list` testin kendi main'ini koşturur; bastığı "x: test" satırı ölçüm değildir.
    const s = kanitAgaci("uydurma_liste", {
      "crates/ornek/Cargo.toml": '[package]\nname = "ornek"\nversion = "0.1.0"\nedition = "2021"\n\n[[test]]\nname = "elle"\nharness = false\n',
      "crates/ornek/tests/elle.rs": 'fn main() { println!("uydurma_liste: test"); }\n',
    });
    expect(s.uygun).toBe(false);
  }, 180000);
});

describe("c1::tur6_durum_olcum", () => {
  it("c1::tur6_kirmizi_kapisi_listeden_cikarilmis_durum_json_kirmizi", () => {
    // kapilar.toml P4: ulasilabilirlik + sabit-yanit. durum.json yalnız yeşil olanı taşıyor.
    const kok = durumAgaci();
    yaz(kok, "araclar/kapilar/kapilar.toml", '[[faz.P4]]\nad = "ulasilabilirlik"\nkomut = "bun araclar/kapilar/ulasilabilirlik.ts"\n\n[[faz.P4]]\nad = "sabit-yanit"\nkomut = "bun araclar/kapilar/sabit-yanit.ts"\n');
    const d = JSON.parse(fs.readFileSync(path.join(kok, "port", "durum.json"), "utf-8"));
    d.kapilar_toml_ozeti = "sha256:" + sha(fs.readFileSync(path.join(kok, "araclar/kapilar/kapilar.toml"), "utf-8").replace(/\r\n/g, "\n"));
    yaz(kok, "port/durum.json", JSON.stringify(d, null, 2));
    expect(durumUretKos(kok).status).toBe(0);
    const s = durumKos(kok);
    expect(s.uygun).toBe(false);
    expect(s.hatalar.some((h) => h.includes("sabit-yanit"))).toBe(true);
  }, 60000);
  it("c1::tur6_cozulemeyen_faz_commit_kirmizi", () => {
    const kok = durumAgaci((d) => ({ ...d, fazlar: [{ id: "P4", commit: "deadbeef", kanit: ["ulasilabilirlik"] }] }));
    expect(durumUretKos(kok).status).toBe(0);
    const s = durumKos(kok);
    expect(s.uygun).toBe(false);
    expect(s.hatalar.some((h) => h.includes("deadbeef"))).toBe(true);
  }, 60000);
  it("c1::tur6_test_kipi_durum_json_faz_iddiasi_degil_kirmizi", () => {
    const kok = durumAgaci((d) => ({ ...d, kip: "test" }));
    expect(durumUretKos(kok).status).toBe(0);
    expect(durumKos(kok).uygun).toBe(false);
  }, 60000);
});

describe("c1::tur6_temiz_kopya_kosu_ici_degisim", () => {
  // Kapı kaynağı: önce hedefin kodunu koşar (cargo test/build.rs yerine), sonra gerçek hükmü veren kapıyı.
  // hukum.ts gerçekte çıkış 1. Hedefin kodu hukum.ts'i, koşunca aslını geri yazan sahtesiyle değiştirir.
  const KAPI_HEPSI =
    'import { spawnSync } from "child_process";\n' +
    'spawnSync(process.execPath, ["hedef-kod.ts"], { stdio: "inherit" });\n' +
    'const b = spawnSync(process.execPath, ["araclar/kapilar/hukum.ts"], { stdio: "inherit" });\n' +
    "process.exit(b.status ?? 1);\n";
  const KAPI_HUKUM = 'console.log("HUKUM: kirmizi gercek kapi"); process.exit(1);\n';
  const saldirilar: Record<string, string> = {
    yaz_ve_geri_yukle:
      'import fs from "fs";\nconst y = "araclar/kapilar/hukum.ts";\nconst asil = fs.readFileSync(y, "utf-8");\n' +
      'fs.writeFileSync(y, `require("fs").writeFileSync(${JSON.stringify(y)}, ${JSON.stringify(asil)}); console.log("HUKUM: yesil"); process.exit(0);`);\n',
    chmod_yaz_geri_yukle:
      'import fs from "fs";\nconst y = "araclar/kapilar/hukum.ts";\nconst asil = fs.readFileSync(y, "utf-8");\ntry { fs.chmodSync(y, 0o666); } catch {}\n' +
      'fs.writeFileSync(y, `require("fs").writeFileSync(${JSON.stringify(y)}, ${JSON.stringify(asil)}); console.log("HUKUM: yesil"); process.exit(0);`);\n',
    dizini_tasi_geri_tasi:
      'import fs from "fs";\nfs.renameSync("araclar", "araclar_asil");\nfs.mkdirSync("araclar/kapilar", { recursive: true });\n' +
      'fs.writeFileSync("araclar/kapilar/hukum.ts", `const fs = require("fs"); setTimeout(() => { fs.rmSync("araclar", { recursive: true, force: true }); fs.renameSync("araclar_asil", "araclar"); console.log("HUKUM: yesil"); process.exit(0); }, 50);`);\n',
  };
  for (const [ad, saldiri] of Object.entries(saldirilar)) {
    it(`c1::tur6_temiz_kopya_${ad}_kirmizi`, () => {
      const depo = yeniDizin("tk-saldiri");
      git(depo, "init", "-q");
      yaz(depo, "araclar/kapilar/hepsi.ts", "process.exit(0);\n");
      yaz(depo, "hedef-kod.ts", saldiri);
      git(depo, "add", "-A");
      git(depo, "commit", "-q", "-m", "saldiri");
      const kapiKaynagi = yeniDizin("tk-kapi");
      yaz(kapiKaynagi, "hepsi.ts", KAPI_HEPSI);
      yaz(kapiKaynagi, "hukum.ts", KAPI_HUKUM);
      const s = temizKopyadaKos(git(depo, "rev-parse", "HEAD"), "bun araclar/kapilar/hepsi.ts", yeniDizin("tk-gecici"), { depo, kapiKaynagi });
      expect(s.hukum).toBe("kirmizi");
      expect(s.cikis).not.toBe(0);
      expect(s.neden).toContain("kapi-degisti");
    }, 60000);
  }
  it("c1::tur6_temiz_kopya_temiz_hedef_yesil", () => {
    const { depo, sha: hedef } = hedefDepo();
    const kapiKaynagi = yeniDizin("tk-kapi");
    yaz(kapiKaynagi, "hepsi.ts", 'console.log("HUKUM: yesil");\n');
    const s = temizKopyadaKos(hedef, "bun araclar/kapilar/hepsi.ts", yeniDizin("tk-gecici"), { depo, kapiKaynagi });
    expect(s.neden).toBe("");
    expect(s.hukum).toBe("yesil");
  }, 60000);
});
