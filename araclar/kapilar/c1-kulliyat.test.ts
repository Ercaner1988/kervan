import { afterAll, beforeAll, describe, expect, it } from "bun:test";
import { spawnSync } from "child_process";
import fs from "fs";
import os from "os";
import path from "path";

// C1 / kulliyat kümesi: kulliyat-tara dizge ezberiyle kandırılamamalı; kapı paketini temiz kopyada koşup
// HUKUM satırlarından karar vermeli. Bütün denemeler CLI üzerinden, os.tmpdir altındaki sahte depoda.
// Sahte depo kendi araclar/kapilar paketini taşır (HEAD'den alınır): her kapı, denetlenen ağaçtaki
// isaret/<kapi> dosyasını okur (ilk satır çıkış kodu, kalanı stdout); dosya yoksa 'HUKUM: yesil'.
// Kapıların okuduğu girdiler (*.rs, kervan paketli Cargo.toml, port/..., ref/) girdileriYaz() ile kurulur: girdisi
// olmayan ya da ayrıştırılamayan kapının kırmızısı 'ölçülemedi' sayılmalı, yakalama sayılmamalı. rapor-kontrol fazları
// paketteki kapilar.toml'dan. Son describe GERÇEK paketi yargıcın çöp ağacına karşı koşar.

const BETIK = path.join(import.meta.dir, "kulliyat-tara.ts");
const KOK = fs.mkdtempSync(path.join(os.tmpdir(), "kervan-c1-kulliyat-"));
const DEPO = path.join(KOK, "depo");
const sha: Record<string, string> = {};

const git = (...a: string[]) => {
  const p = spawnSync("git", ["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false", "-c", "core.hooksPath=NUL", ...a], { cwd: DEPO, encoding: "utf-8" });
  if (p.status !== 0) throw new Error(`git ${a.join(" ")}: ${p.stderr}`);
  return p.stdout.trim();
};
const yaz = (rel: string, icerik: string) => {
  const y = path.join(DEPO, ...rel.split("/"));
  fs.mkdirSync(path.dirname(y), { recursive: true });
  fs.writeFileSync(y, icerik);
};
const commitle = (ad: string) => {
  git("add", "-A");
  git("commit", "-q", "--allow-empty", "-m", ad);
  sha[ad] = git("rev-parse", "HEAD");
};
const isaretleriYaz = (m: Record<string, string>) => {
  fs.rmSync(path.join(DEPO, "isaret"), { recursive: true, force: true });
  for (const [k, v] of Object.entries(m)) yaz(`isaret/${k}`, v);
};
const KAPILAR = ["sabit-yanit", "ulasilabilirlik", "uc-kaydi", "defter-kontrol", "durum-kontrol", "rapor-kontrol"];
const hepsiKirmizi = () => Object.fromEntries([...KAPILAR, "parite-baglantisi"].map((k) => [k, `1\nHUKUM: kirmizi ${k} sahte kusur ölçtü`]));

// Paketin her kapısının ölçtüğü girdi (kulliyat-tara KAPI_PAKETI.girdi ile aynı yollar).
const CARGO = `[package]\nname = "kervan"\nversion = "0.1.0"\nedition = "2021"\n\n[workspace]\n`;
const girdileriYaz = () => {
  yaz("crates/v1/src/lib.rs", "pub fn f() {}\n");
  yaz("src/main.rs", "fn main() {}\n");
  yaz("Cargo.toml", CARGO);
  yaz("port/uclar.toml", `[[uc]]\nyol = "/x"\nyontem = "GET"\ndurum = "kapsam-disi"\ngerekce = "t"\n`);
  yaz("port/durum.json", `{"faz": "P1"}`);
  yaz("port/defter/davranislar.jsonl", `{"id": "D1", "durum": "planlandi"}\n`);
  yaz("araclar/parite/kos.ts", "export {};\n");
  yaz("port/altin/a.json", `{"girdi": {}, "cikti": {}}`);
  yaz("port/defter/dosyalar.jsonl", `{"id": "F1", "durum": "planlandi"}\n`);
  yaz("port/DURUM.md", "durum\n");
  for (const f of ["P1", "P2"]) yaz(`port/kayit/${f}/RAPOR.md`, "rapor\n");
};

const kos = (...a: string[]) => {
  const p = spawnSync(process.execPath, [BETIK, ...a], { cwd: DEPO, encoding: "utf-8", timeout: 120_000 });
  const satirlar = (p.stdout || "").split(/\r?\n/).filter((s) => s.trim());
  return { cikis: p.status, stdout: p.stdout || "", stderr: p.stderr || "", son: satirlar[satirlar.length - 1] ?? "" };
};

// Kapıların okuyucuları (kulliyat-tara girdileri bunlarla ayrıştırır): gerçeklerinin yalın karşılıkları.
const OKUYUCU: Record<string, string> = {
  "uc-kaydi": `export function uclarTomlOku(y) { const u = Bun.TOML.parse(fs.readFileSync(y, "utf-8")).uc; if (!Array.isArray(u)) throw new Error("[[uc]] yok"); return u; }`,
  "defter-kontrol": `export function defterSatirlariniOku(y, h = []) { return fs.readFileSync(y, "utf-8").split("\\n").filter((s) => s.trim()).flatMap((s, i) => { try { return [JSON.parse(s)]; } catch { h.push("satır " + (i + 1) + " bozuk"); return []; } }); }`,
};

beforeAll(() => {
  fs.mkdirSync(DEPO, { recursive: true });
  git("init", "-q");
  // ref/ izlenmez (temiz-kopya izlenen ref/'i reddeder); çağıranın ref'i worktree'ye bağlanır.
  fs.appendFileSync(path.join(DEPO, ".git", "info", "exclude"), "ref/\n");
  yaz("ref/omniroute-dbe703a/route.ts", "x\n");
  for (const k of KAPILAR) {
    yaz(`araclar/kapilar/${k}.ts`, `import fs from "fs";
${OKUYUCU[k] ?? ""}
if (import.meta.main) {
const faz = process.argv[2];
if ("${k}" === "rapor-kontrol" && !fs.existsSync("port/kayit/" + faz + "/RAPOR.md")) { console.log("HUKUM: kirmizi rapor yok: " + faz); process.exit(1); }
const y = "isaret/${k}";
if (!fs.existsSync(y)) { console.log("HUKUM: yesil"); process.exit(0); }
const [kod, ...satir] = fs.readFileSync(y, "utf-8").split("\\n");
console.log(satir.join("\\n"));
process.exit(Number(kod));
}
`);
  }
  yaz("araclar/kapilar/durum-uret.ts", `import fs from "fs";\nexport function durumJsonOku(y: string) { return JSON.parse(fs.readFileSync(y, "utf-8")); }\n`);
  yaz("araclar/kapilar/hepsi.ts", "export {};\n");
  yaz("araclar/kapilar/mutasyon-kontrol.ts", `import fs from "fs";
export function pariteBaglantisiniDenetle(d: string) {
  const y = "isaret/parite-baglantisi";
  if (!fs.existsSync(y)) return { uygun: true, hatalar: [], kendiniDogrulayan: false };
  // Gerçek kapının sözleşmesi: kusur ölçüldüyse kendiniDogrulayan=true; "ölçülemez" (ör. altın yok) kırmızısı ölçüm değil.
  const h = fs.readFileSync(y, "utf-8").split("\\n").slice(1).join(" ");
  return { uygun: false, hatalar: [h], kendiniDogrulayan: !h.includes("ölçülemez") };
}
`);
  // Çağıranın mühürlü faz listesi: rapor-kontrol P1 ve P2'de.
  yaz("araclar/kapilar/kapilar.toml", `[[faz.P1]]\nad = "rapor-kontrol"\nkomut = "x"\n\n[[faz.P2]]\nad = "sabit-yanit"\nkomut = "x"\n\n[[faz.P2]]\nad = "rapor-kontrol"\nkomut = "x"\n`);
  yaz("README.md", "# sahte\n");
  girdileriYaz();
  commitle("kapilar");

  // Eski kâhinin aradığı dizgelerin hepsi var; ama paketin kapıları ağacı yeşil ölçüyor.
  yaz("crates/v1/src/sohbet.rs", `fn f() -> &'static str { "chatcmpl-123 1700000000 selamlar!" }\n`);
  yaz("Cargo.toml", CARGO + "# kervan-v1 bağımlılığı yok\n");
  yaz("araclar/parite/kos.ts", `console.log(1 === 1);\n`);
  yaz("port/defter/dosyalar.jsonl", `{"durum":"planlandi","hedef":null,"kanit":[]}\n`);
  yaz("port/DURUM.md", "TAMAMLANDI\n");
  commitle("ezber");

  girdileriYaz();
  isaretleriYaz(hepsiKirmizi());
  commitle("hepsi-kirmizi");

  const kacan = hepsiKirmizi();
  delete kacan.ulasilabilirlik;
  isaretleriYaz(kacan);
  commitle("bagsiz-kacti");

  // sabit-yanit çöküyor (HUKUM yok), uc-kaydi nedensiz kırmızı: sabit-yanit sınıfı ÖLÇÜLEMEDİ, yakalanmış sayılmaz.
  isaretleriYaz({ ...hepsiKirmizi(), "sabit-yanit": "1\nTypeError: patladi", "uc-kaydi": "1\nHUKUM: kirmizi " });
  commitle("cokme");

  // HUKUM son satır değil; çıkış 0 ile kırmızı (tutarsız).
  isaretleriYaz({ ...hepsiKirmizi(), "sabit-yanit": "1\nHUKUM: kirmizi x\nsonra bir satır", "uc-kaydi": "0\nHUKUM: kirmizi x" });
  commitle("bozuk-hukum");

  // Boş ağaç: hiçbir sınıfın girdisi yok, kapılar gerçekçi biçimde fail-closed kırmızı ('girdi yok').
  for (const f of ["crates", "src", "Cargo.toml", "araclar/parite", "port"]) fs.rmSync(path.join(DEPO, f), { recursive: true, force: true });
  isaretleriYaz(Object.fromEntries([...KAPILAR, "parite-baglantisi"].map((k) => [k, `1\nHUKUM: kirmizi ${k}: girdi dosyası yok`])));
  commitle("bos-agac");

  // Raporsuz faz: P2'nin kayıt dizini var ama RAPOR.md yok (adı değişmiş); bütün kapılar yeşil.
  fs.rmSync(path.join(DEPO, "isaret"), { recursive: true, force: true });
  girdileriYaz();
  fs.renameSync(path.join(DEPO, "port/kayit/P2/RAPOR.md"), path.join(DEPO, "port/kayit/P2/RAPOR-eski.md"));
  yaz("port/kayit/P2/gunluk.txt", "eski\n");
  commitle("raporsuz-faz");

  // Biçimsiz altın: port/altin/*.json var ama {girdi, cikti} yok. Kapı bağlantıyı yine ölçer; karşılaştırıcı
  // Kervan'ı çağırıyorsa kırmızısı "altın yok, ölçülemez"dir ve yakalama sayılmaz (mutasyon-kontrol m20).
  girdileriYaz();
  yaz("port/altin/a.json", `{"istek": {}, "beklenen": {}}`);
  isaretleriYaz({ ...hepsiKirmizi(), "parite-baglantisi": "1\nHUKUM: kirmizi Kervan çağrılıyor ama {girdi, cikti} biçimli altın yok; parite ölçülemez" });
  commitle("bicimsiz-altin");
}, 60_000);

afterAll(() => {
  spawnSync("git", ["worktree", "prune"], { cwd: DEPO });
  fs.rmSync(KOK, { recursive: true, force: true });
});

describe("c1::kulliyat", () => {
  it("c1::kulliyat_ezber_dizgeleri_kapi_yesilken_yakalama_sayilmaz", () => {
    const r = kos(sha["ezber"]);
    expect(r.cikis).toBe(1);
    expect(r.son).toStartWith("HUKUM: kirmizi ");
    expect(r.son).toContain("5 sahte sınıf hiçbir kapıdan kırmızı almadı");
  }, 120_000);

  it("c1::kulliyat_her_sinif_olcen_kapidan_kirmizi_alinca_yesil_ve_eslemeyi_yazar", () => {
    const r = kos(sha["hepsi-kirmizi"]);
    expect(r.son).toBe("HUKUM: yesil");
    expect(r.cikis).toBe(0);
    expect(r.stdout).toContain("sabit-yanit: YAKALANDI (sabit-yanit) |");
    expect(r.stdout).toContain("bagsiz-crate: YAKALANDI (ulasilabilirlik)");
    expect(r.stdout).toContain("kendini-dogrulayan-parite: YAKALANDI (parite-baglantisi)");
    expect(r.stdout).toContain("kanitsiz-defter: YAKALANDI (defter-kontrol-final)");
    expect(r.stdout).toContain("elle-yazilmis-durum: YAKALANDI (durum-kontrol) |");
  }, 120_000);

  it("c1::kulliyat_tek_sinif_kacinca_kirmizi_ve_sinif_adi_yazilir", () => {
    const r = kos(sha["bagsiz-kacti"]);
    expect(r.cikis).toBe(1);
    expect(r.son).toBe("HUKUM: kirmizi 1 sahte sınıf hiçbir kapıdan kırmızı almadı: bagsiz-crate");
  }, 120_000);

  it("c1::kulliyat_coken_ya_da_nedensiz_kapi_yakalama_sayilmaz", () => {
    const r = kos(sha["cokme"]);
    expect(r.cikis).toBe(1);
    expect(r.son).toContain("kapı ölçemedi");
    expect(r.stdout).toContain("sabit-yanit: yakalanmadı");
  }, 120_000);

  it("c1::kulliyat_son_satir_olmayan_ya_da_cikisla_tutarsiz_hukum_hata", () => {
    const r = kos(sha["bozuk-hukum"]);
    expect(r.cikis).toBe(1);
    expect(r.son).toContain("2 kapı ölçemedi");
  }, 120_000);

  it("c1::kulliyat_cozulemeyen_ref_ref_yok_ve_cikis_3", () => {
    const r = kos("yok-boyle-bir-ref");
    expect(r.cikis).toBe(3);
    expect(r.son).toStartWith("HUKUM: kirmizi ref-yok");
  }, 120_000);

  it("c1::kulliyat_varsayilan_ref_dal_adi_degil_ac2f18d_sha", () => {
    const r = kos();
    expect(r.cikis).toBe(3);
    expect(r.son).toContain("ac2f18d3fa588dad886589ff901ebd1c7ad5afae");
  }, 120_000);

  it("c1::kulliyat_temiz_kip_beklentisi_ters", () => {
    const temiz = kos("--temiz", sha["ezber"]);
    expect(temiz.son).toBe("HUKUM: yesil");
    expect(temiz.cikis).toBe(0);
    const kirli = kos("--temiz", sha["hepsi-kirmizi"]);
    expect(kirli.cikis).toBe(1);
    expect(kirli.son).toStartWith("HUKUM: kirmizi temiz ağaçta 8 kapı kırmızı");
  }, 240_000);

  it("c1::kulliyat_bos_agacta_eksik_girdi_kirmizisi_yakalama_sayilmaz", () => {
    const r = kos(sha["bos-agac"]);
    expect(r.cikis).toBe(1);
    expect(r.son).toBe("HUKUM: kirmizi 5 sahte sınıf hiçbir kapıdan kırmızı almadı: sabit-yanit, bagsiz-crate, kendini-dogrulayan-parite, kanitsiz-defter, elle-yazilmis-durum");
    expect(r.stdout).toContain("[olculemedi] sabit-yanit");
    expect(r.stdout).not.toContain("YAKALANDI");
  }, 120_000);

  it("c1::kulliyat_bicimsiz_altinda_parite_kirmizisi_olcum_sayilmaz", () => {
    const r = kos(sha["bicimsiz-altin"]);
    expect(r.cikis).toBe(1);
    expect(r.son).toBe("HUKUM: kirmizi 1 sahte sınıf hiçbir kapıdan kırmızı almadı: kendini-dogrulayan-parite");
    expect(r.stdout).toContain("[olculemedi] parite-baglantisi");
  }, 120_000);

  it("c1::kulliyat_temiz_kip_faz_listesi_cagiranin_kapilar_tomlundan_raporsuz_faz_kirmizi", () => {
    const r = kos("--temiz", sha["raporsuz-faz"]);
    expect(r.cikis).toBe(1);
    expect(r.stdout).toContain("rapor-kontrol:P1");
    expect(r.stdout).toContain("[olculemedi] rapor-kontrol:P2");
    expect(r.son).toStartWith("HUKUM: kirmizi temiz ağaçta 1 kapı ölçülemedi; ilki rapor-kontrol:P2");
  }, 120_000);
});

// Yargıç engeli: hiçbir sahte sınıf taşımayan, yalnız ayrıştırılamaz/eksik ikincil girdili bir çöp ağaç. GERÇEK kapı
// paketi (bu çalışma ağacının araclar/kapilar'ı) koşulur. Fail-closed 'girdi bozuk/eksik' kırmızıları sınıf yakalama
// sayılmamalı: sabit-yanit, bagsiz-crate, kanitsiz-defter, elle-yazilmis-durum yakalanmış görünemez, külliyat yeşil olamaz.
describe("c1::kulliyat_gercek_paket", () => {
  it("c1::kulliyat_cop_agacta_bozuk_ya_da_eksik_girdi_kirmizisi_yakalama_sayilmaz", async () => {
    const { kulliyatCommitiniTara } = await import("./kulliyat-tara.ts");
    const cop = path.join(KOK, "cop");
    fs.mkdirSync(cop, { recursive: true });
    const g = (...a: string[]) => {
      const p = spawnSync("git", ["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false", "-c", "core.hooksPath=NUL", ...a], { cwd: cop, encoding: "utf-8" });
      if (p.status !== 0) throw new Error(`git ${a.join(" ")}: ${p.stderr}`);
    };
    const y = (rel: string, s: string) => {
      const t = path.join(cop, ...rel.split("/"));
      fs.mkdirSync(path.dirname(t), { recursive: true });
      fs.writeFileSync(t, s);
    };
    g("init", "-q");
    y("crates/a/src/lib.rs", "pub fn f() -> u32 { 2 }\n");
    y("Cargo.toml", `[workspace]\nmembers = ["crates/a"]\nresolver = "2"\n`);
    y("crates/a/Cargo.toml", `[package]\nname = "a"\nversion = "0.1.0"\nedition = "2021"\n`);
    y("port/uclar.toml", "bu [[ toml degil\n");
    y("port/defter/dosyalar.jsonl", "bozuk-satir\n");
    y("port/DURUM.md", "x\n");
    for (let i = 2; i <= 10; i++) y(`port/kayit/P${i}/RAPOR.md`, "x\n");
    y("araclar/parite/a.ts", "export {};\n");
    y("port/altin/a.json", `{"girdi":{},"cikti":{}}`);
    g("add", "-A");
    g("commit", "-q", "-m", "cop");
    const r = kulliyatCommitiniTara("HEAD", {
      depo: cop, kapiKaynagi: import.meta.dir, kapiZamanAsimiMs: 180_000, geciciKok: path.join(KOK, "cop-gk"),
    });
    const ozet = r.kapilar.map((k) => `[${k.durum}] ${k.ad}: ${k.neden.slice(0, 120)}`).join("\n");
    for (const s of ["sabit-yanit", "bagsiz-crate", "kanitsiz-defter", "elle-yazilmis-durum"] as const) {
      expect(`${s} <- ${r.yakalayanlar[s].join(",")}\n${ozet}`).toBe(`${s} <- \n${ozet}`);
    }
    expect(r.hukum).toBe("kirmizi");
    expect(r.cikis).toBe(1);
  }, 600_000);
});
