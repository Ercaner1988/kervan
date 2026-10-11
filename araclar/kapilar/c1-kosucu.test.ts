// Küme 'kosucu' (C1): hepsi.ts / makine-sakin.ts / rapor-kontrol.ts / kapilar.toml'un
// kandırılamadığını gösteren testler. Geçici dosyalar yalnız os.tmpdir() altında,
// adında 'c1-kosucu' geçen dizinde açılır; depo ağacına ve port/kayit'e yazılmaz.
import { afterAll, describe, expect, it, setDefaultTimeout } from "bun:test";
import { spawnSync } from "child_process";
import fs from "fs";
import os from "os";
import path from "path";
import * as hepsi from "./hepsi.ts";
import * as makine from "./makine-sakin.ts";
import * as rapor from "./rapor-kontrol.ts";
import { durumSemasiniDenetle } from "./durum-uret.ts";

const KOK = fs.mkdtempSync(path.join(os.tmpdir(), "kervan-c1-kosucu-"));
const HEPSI = path.join(import.meta.dir, "hepsi.ts");
const MAKINE = path.join(import.meta.dir, "makine-sakin.ts");
const RAPOR = path.join(import.meta.dir, "rapor-kontrol.ts");
const DEPO_TOML = path.join(import.meta.dir, "kapilar.toml");

// Alt süreç (bun, git) açan testler Windows'ta 5 sn varsayılanını aşabiliyor.
setDefaultTimeout(120000);
afterAll(() => fs.rmSync(KOK, { recursive: true, force: true }));

let sayac = 0;
function yeniDizin(etiket: string): string {
  const d = path.join(KOK, `${etiket}-${sayac++}`);
  fs.mkdirSync(path.join(d, "araclar", "kapilar"), { recursive: true });
  // Örnek kapı betikleri (cwd'ye göre çalışır)
  fs.writeFileSync(path.join(d, "yesil.ts"), `console.log("ölçtüm"); console.log("HUKUM: yesil");\n`);
  fs.writeFileSync(path.join(d, "hukumsuz.ts"), `console.log("her şey yolunda");\n`);
  fs.writeFileSync(path.join(d, "kirmizi-sifir.ts"), `console.log("HUKUM: kirmizi bir şey bozuk");\n`);
  fs.writeFileSync(path.join(d, "hukum-ortada.ts"), `console.log("HUKUM: yesil"); console.log("sonra başka satır");\n`);
  fs.writeFileSync(path.join(d, "yesil-ama-cikis1.ts"), `console.log("HUKUM: yesil"); process.exit(1);\n`);
  return d;
}

function tomlYaz(dizin: string, icerik: string) {
  fs.writeFileSync(path.join(dizin, "araclar", "kapilar", "kapilar.toml"), icerik, "utf-8");
}

function kos(betik: string, args: string[], cwd: string) {
  const p = spawnSync("bun", [betik, ...args], { cwd, encoding: "utf-8", timeout: 120000 });
  return { cikis: p.status, out: (p.stdout || "") + (p.stderr || ""), stdout: p.stdout || "" };
}

function sonSatir(s: string): string {
  const satirlar = s.split(/\r?\n/).map((x) => x.trim()).filter(Boolean);
  return satirlar[satirlar.length - 1] || "";
}

const MAKINE_UYARI = `[kosucu]
varsayilan_zaman_asimi_sn = 120
[makine_sakin]\nmin_bos_ram_gb = 6\nmax_cpu_pct = 70\nornek_sn = 1\nkip = "uyari"\n[sayim]\nkaynak_dizini = "ref/snap"\n`;

// hepsi.ts'nin §6 sayımlarını ve fazlarını ölçebileceği tam düzenek: geçici git deposu (yalnız
// os.tmpdir() altında; kervan ağacına commit atılmaz), 2 route.ts, 3 satırlık defter.
function git(d: string, ...a: string[]) {
  const p = spawnSync("git", ["-c", "user.name=c1", "-c", "user.email=c1@yerel", "-c", "commit.gpgsign=false", ...a], { cwd: d, encoding: "utf-8" });
  if (p.status !== 0) throw new Error(`git ${a.join(" ")}: ${p.stderr}`);
  return p.stdout.trim();
}
function tamDuzenek(d: string) {
  for (const r of ["ref/snap/a/route.ts", "ref/snap/b/c/route.ts", "ref/snap/b/page.ts"]) {
    fs.mkdirSync(path.dirname(path.join(d, r)), { recursive: true });
    fs.writeFileSync(path.join(d, r), "x\n");
  }
  fs.mkdirSync(path.join(d, "port", "defter"), { recursive: true });
  fs.writeFileSync(
    path.join(d, "port", "defter", "dosyalar.jsonl"),
    ['{"id":"D1","durum":"planlandi"}', '{"id":"D2","durum":"dogrulandi"}', '{"id":"D3","durum":"okundu"}'].join("\n") + "\n"
  );
  git(d, "init", "-q");
  git(d, "add", "-A");
  git(d, "commit", "-q", "-m", "baslangic");
}

describe("c1::hepsi_fail_closed", () => {
  it("c1::bos_kapi_listesi_yesil_sayilmaz", async () => {
    const d = yeniDizin("bos");
    const r = await hepsi.kapilariKos([], "TEST", path.join(d, "kayit"), path.join(d, "durum.json"));
    expect(r.hepsiYesil).toBe(false);
  });

  it("c1::cikis_0_ama_hukum_satiri_yoksa_kirmizi", async () => {
    const d = yeniDizin("hukumsuz");
    const r = await hepsi.kapilariKos(
      [{ ad: "hukumsuz", komut: `bun "${path.join(d, "hukumsuz.ts")}"` }],
      "TEST",
      path.join(d, "kayit"),
      path.join(d, "durum.json")
    );
    expect(r.sonuclar[0].cikis).toBe(0);
    expect(r.hepsiYesil).toBe(false);
  });

  it("c1::cikis_0_ve_hukum_kirmizi_ise_kirmizi", async () => {
    const d = yeniDizin("kirmizi0");
    const r = await hepsi.kapilariKos(
      [{ ad: "k", komut: `bun "${path.join(d, "kirmizi-sifir.ts")}"` }],
      "TEST",
      path.join(d, "kayit"),
      path.join(d, "durum.json")
    );
    expect(r.hepsiYesil).toBe(false);
  });

  it("c1::hukum_son_satir_degilse_kirmizi", async () => {
    const d = yeniDizin("ortada");
    const r = await hepsi.kapilariKos(
      [{ ad: "o", komut: `bun "${path.join(d, "hukum-ortada.ts")}"` }],
      "TEST",
      path.join(d, "kayit"),
      path.join(d, "durum.json")
    );
    expect(r.hepsiYesil).toBe(false);
  });

  it("c1::hukum_yesil_ama_cikis_1_ise_kirmizi", async () => {
    const d = yeniDizin("cikis1");
    const r = await hepsi.kapilariKos(
      [{ ad: "c", komut: `bun "${path.join(d, "yesil-ama-cikis1.ts")}"` }],
      "TEST",
      path.join(d, "kayit"),
      path.join(d, "durum.json")
    );
    expect(r.hepsiYesil).toBe(false);
  });

  it("c1::cikis_0_ve_hukum_yesil_ise_yesil", async () => {
    const d = yeniDizin("yesil");
    const r = await hepsi.kapilariKos(
      [{ ad: "y", komut: `bun "${path.join(d, "yesil.ts")}"` }],
      "TEST",
      path.join(d, "kayit"),
      path.join(d, "durum.json")
    );
    expect(r.hepsiYesil).toBe(true);
    expect(fs.existsSync(path.join(d, "kayit", "y.log"))).toBe(true);
  });

  it("c1::kapilar_toml_yoksa_cikis_sifir_degil", () => {
    const d = yeniDizin("tomlyok");
    const r = kos(HEPSI, ["--faz", "P4"], d);
    expect(r.cikis).not.toBe(0);
  });

  it("c1::bozuk_kapilar_toml_cikis_sifir_degil", () => {
    const d = yeniDizin("tomlbozuk");
    tomlYaz(d, `[[faz.P1]\nad = "y"\n`);
    const r = kos(HEPSI, ["--faz", "P1"], d);
    expect(r.cikis).not.toBe(0);
  });

  it("c1::bilinmeyen_faz_cikis_sifir_degil", () => {
    const d = yeniDizin("bilinmeyen");
    tomlYaz(d, `${MAKINE_UYARI}[[faz.P1]]\nad = "y"\nkomut = "bun yesil.ts"\n`);
    const r = kos(HEPSI, ["--faz", "P7"], d);
    expect(r.cikis).not.toBe(0);
    expect(fs.existsSync(path.join(d, "port", "durum.json")) && JSON.parse(fs.readFileSync(path.join(d, "port", "durum.json"), "utf-8")).tum_kapilar_yesil_mi === true).toBe(false);
  });

  it("c1::bos_faz_listesi_cikis_sifir_degil", () => {
    const d = yeniDizin("bosfaz");
    tomlYaz(d, `${MAKINE_UYARI}[faz]\nP1 = []\n`);
    const r = kos(HEPSI, ["--faz", "P1"], d);
    expect(r.cikis).not.toBe(0);
  });

  it("c1::faz_verilmezse_cikis_sifir_degil", () => {
    const d = yeniDizin("fazsiz");
    tomlYaz(d, `${MAKINE_UYARI}[[faz.P1]]\nad = "y"\nkomut = "bun yesil.ts"\n`);
    const r = kos(HEPSI, [], d);
    expect(r.cikis).not.toBe(0);
  });

  it("c1::p4_p3_ust_kumesi_degilse_cikis_sifir_degil", () => {
    const d = yeniDizin("ustkume");
    tomlYaz(
      d,
      `${MAKINE_UYARI}[[faz.P3]]\nad = "y"\nkomut = "bun yesil.ts"\n[[faz.P3]]\nad = "z"\nkomut = "bun yesil.ts {faz}"\n` +
        `[[faz.P4]]\nad = "y"\nkomut = "bun yesil.ts"\n`
    );
    const r = kos(HEPSI, ["--faz", "P4"], d);
    expect(r.cikis).not.toBe(0);
  });

  it("c1::toml_listesi_kosulur_ve_yesil_hukumle_gecer", () => {
    const d = yeniDizin("tamyol");
    tamDuzenek(d);
    tomlYaz(d, `${MAKINE_UYARI}[[faz.P1]]\nad = "y"\nkomut = "bun yesil.ts {faz}"\n`);
    const r = kos(HEPSI, ["--faz", "P1"], d);
    expect(r.cikis).toBe(0);
    const durum = JSON.parse(fs.readFileSync(path.join(d, "port", "durum.json"), "utf-8"));
    expect(durum.kip).toBe("faz");
    expect(durum.kapilar.map((k: any) => k.ad)).toEqual(["y"]);
    expect(durum.kapilar[0].komut).toContain("yesil.ts P1");
  });

  it("c1::toml_listesinde_hukumsuz_kapi_paketi_kirmizi_yapar", () => {
    const d = yeniDizin("pakethukumsuz");
    tomlYaz(d, `${MAKINE_UYARI}[[faz.P1]]\nad = "y"\nkomut = "bun yesil.ts"\n[[faz.P1]]\nad = "h"\nkomut = "bun hukumsuz.ts"\n`);
    const r = kos(HEPSI, ["--faz", "P1"], d);
    expect(r.cikis).not.toBe(0);
  });

  it("c1::kapi_ile_daraltma_test_kipi_olmadan_yasak", () => {
    const d = yeniDizin("daraltma");
    tomlYaz(d, `${MAKINE_UYARI}[[faz.P1]]\nad = "y"\nkomut = "bun yesil.ts"\n[[faz.P1]]\nad = "h"\nkomut = "bun hukumsuz.ts"\n`);
    const r = kos(HEPSI, ["--faz", "P1", "--kapi", "y"], d);
    expect(r.cikis).not.toBe(0);
  });

  it("c1::kapi_ile_keyfi_komut_verilemez", () => {
    const d = yeniDizin("keyfi");
    tomlYaz(d, `${MAKINE_UYARI}[[faz.P1]]\nad = "h"\nkomut = "bun hukumsuz.ts"\n`);
    const r = kos(HEPSI, ["--faz", "P1", "--kapi", "sahte", "bun yesil.ts"], d);
    expect(r.cikis).not.toBe(0);
  });

  // ESKİ assert (bu kümenin önceki denemesi) yanlıştı: test kipinde 'cikis 0' ve port/durum.json'a
  // yazılmasını bekliyordu. Daraltılmış koşu gerçek port/durum.json + port/kayit/<faz>'ı ezip
  // 'HUKUM: yesil' ile 0 dönüyordu; kipi okuyan başka kapı yok, yani liste daraltma açığı yalnız
  // etiket almıştı (yargıç engeli). Doğrusu: gerçek kayda dokunmaz, hiçbir zaman yeşil çıkmaz.
  it("c1::test_kipi_gercek_kayda_yazmaz_ve_yesil_cikmaz", () => {
    const d = yeniDizin("testkipi");
    tamDuzenek(d);
    tomlYaz(d, `${MAKINE_UYARI}[[faz.P1]]\nad = "y"\nkomut = "bun yesil.ts"\n[[faz.P1]]\nad = "h"\nkomut = "bun hukumsuz.ts"\n`);
    const r = kos(HEPSI, ["--faz", "P1", "--test-kipi", "--kapi", "y"], d);
    expect(r.cikis).not.toBe(0);
    expect(sonSatir(r.stdout)).toMatch(/^HUKUM: kirmizi \S/);
    expect(fs.existsSync(path.join(d, "port", "durum.json"))).toBe(false);
    expect(fs.existsSync(path.join(d, "port", "kayit"))).toBe(false);
    const ayri = /kayıt (\S+)\)$/.exec(sonSatir(r.stdout))?.[1];
    expect(ayri && path.resolve(ayri).startsWith(path.resolve(os.tmpdir()))).toBe(true);
    expect(fs.existsSync(path.join(ayri!, "y.log"))).toBe(true);
    fs.rmSync(ayri!, { recursive: true, force: true });
  });

  it("c1::kapi_bayragi_sonraki_bayragi_yutmaz", () => {
    const d = yeniDizin("yutma");
    tomlYaz(d, `${MAKINE_UYARI}[[faz.P1]]\nad = "y"\nkomut = "bun yesil.ts"\n`);
    // '--kapi --kapi y': ilk --kapi'nin değeri bayrak olamaz -> hata
    const r = kos(HEPSI, ["--faz", "P1", "--test-kipi", "--kapi", "--kapi", "y"], d);
    expect(r.cikis).not.toBe(0);
  });

  it("c1::bilinmeyen_arguman_cikis_sifir_degil", () => {
    const d = yeniDizin("bilinmeyenarg");
    tomlYaz(d, `${MAKINE_UYARI}[[faz.P1]]\nad = "y"\nkomut = "bun yesil.ts"\n`);
    const r = kos(HEPSI, ["--faz", "P1", "--gevset"], d);
    expect(r.cikis).not.toBe(0);
  });
});

describe("c1::durum_json_sema", () => {
  it("c1::durum_json_fazlar_ve_sayimlar_olculur_ve_sema_gecer", () => {
    const d = yeniDizin("sema");
    tamDuzenek(d);
    // Geçmiş: P1 o commit'teki koşucunun yeşil çıktısı; P2 kırmızı; P4 yeşil iddialı ama koşucu özeti
    // tutmuyor (elle yazılmış / eski koşucu, Flash'ın durum.json'u gibi). fazlar'a yalnız P1 girmeli.
    const ozet = (m: string) => "sha256:" + require("crypto").createHash("sha256").update(m).digest("hex");
    const kosucu = "// eski koşucu\n";
    // fazlariOlc (2026-10-10) o commit'teki toml listesini ve günlükleri de ister: düzenekte P1/P2 listesi ve
    // P1 günlüğü var (beklenti aynı: fazlar yalnız P1).
    tomlYaz(d, MAKINE_UYARI + ["P1", "P2", "P3"].map((f) => `[[faz.${f}]]\nad = "y"\nkomut = "bun yesil.ts"\n`).join(""));
    fs.writeFileSync(path.join(d, "araclar", "kapilar", "hepsi.ts"), kosucu);
    fs.mkdirSync(path.join(d, "port", "kayit", "P1"), { recursive: true });
    fs.writeFileSync(path.join(d, "port", "kayit", "P1", "y.log"), fazLogu("y", "bun yesil.ts", "P1"));
    const toml = fs.readFileSync(path.join(d, "araclar", "kapilar", "kapilar.toml"), "utf-8");
    const durumYaz = (faz: string, yesil: boolean, uretici = ozet(kosucu)) =>
      fs.writeFileSync(
        path.join(d, "port", "durum.json"),
        JSON.stringify({
          uretici_ozeti: uretici,
          kapilar_toml_ozeti: ozet(toml),
          faz,
          kip: "faz",
          tum_kapilar_yesil_mi: yesil,
          kapilar: [{ ad: "y", cikis: yesil ? 0 : 1, hukum: yesil ? "yesil" : "kirmizi" }],
        })
      );
    const kaydet = (m: string) => {
      git(d, "add", "-A");
      git(d, "commit", "-q", "-m", m);
      return git(d, "rev-parse", "HEAD");
    };
    durumYaz("P1", true);
    const p1sha = kaydet("P1: ilk faz");
    durumYaz("P2", false);
    kaydet("P2: ikinci faz");
    durumYaz("P4", true, "araclar/kapilar/hepsi.ts");
    kaydet("P4: sahte yeşil");

    const r = kos(HEPSI, ["--faz", "P3"], d);
    expect(r.out).not.toContain("koşucu başlayamadı");
    expect(r.cikis).toBe(0);
    const durum = JSON.parse(fs.readFileSync(path.join(d, "port", "durum.json"), "utf-8"));
    expect(durumSemasiniDenetle(durum)).toEqual([]);
    expect(durum.sayimlar).toEqual({ route_ts: 2, defter_planlandi: 1, defter_dogrulandi: 1 });
    expect(durum.fazlar).toEqual([{ id: "P1", commit: p1sha, kanit: ["y"] }]);
  });

  it("c1::sayim_kaynagi_yoksa_kirmizi", () => {
    const d = yeniDizin("sayimyok");
    tamDuzenek(d);
    fs.rmSync(path.join(d, "ref"), { recursive: true, force: true });
    tomlYaz(d, `${MAKINE_UYARI}[[faz.P1]]\nad = "y"\nkomut = "bun yesil.ts"\n`);
    const r = kos(HEPSI, ["--faz", "P1"], d);
    expect(r.cikis).not.toBe(0);
    expect(sonSatir(r.stdout)).toMatch(/^HUKUM: kirmizi \S/);
  });

  it("c1::bozuk_defter_satiri_kirmizi", () => {
    const d = yeniDizin("defterbozuk");
    tamDuzenek(d);
    fs.appendFileSync(path.join(d, "port", "defter", "dosyalar.jsonl"), "{bozuk\n");
    tomlYaz(d, `${MAKINE_UYARI}[[faz.P1]]\nad = "y"\nkomut = "bun yesil.ts"\n`);
    const r = kos(HEPSI, ["--faz", "P1"], d);
    expect(r.cikis).not.toBe(0);
  });

  it("c1::git_deposu_degilse_kirmizi", () => {
    const d = yeniDizin("gityok");
    tamDuzenek(d);
    fs.rmSync(path.join(d, ".git"), { recursive: true, force: true });
    tomlYaz(d, `${MAKINE_UYARI}[[faz.P1]]\nad = "y"\nkomut = "bun yesil.ts"\n`);
    const r = kos(HEPSI, ["--faz", "P1"], d);
    expect(r.cikis).not.toBe(0);
  });
});

describe("c1::kapilar_toml_icerigi", () => {
  it("c1::depo_kapilar_toml_p1_p10_tanimli_ve_p4_ustu_p3_ust_kumesi", () => {
    expect(fs.existsSync(DEPO_TOML)).toBe(true);
    expect(typeof (hepsi as any).kapilarTomlYukle).toBe("function");
    const cfg = (hepsi as any).kapilarTomlYukle(DEPO_TOML);
    const fazlar = ["P1", "P2", "P3", "P4", "P5", "P6", "P7", "P8", "P9", "P10"];
    for (const f of fazlar) expect(cfg.fazlar[f]?.length ?? 0).toBeGreaterThan(0);
    const p3 = new Set(cfg.fazlar.P3.map((k: any) => `${k.ad}\u0000${k.komut}`));
    for (const f of fazlar.slice(3)) {
      const fk = new Set(cfg.fazlar[f].map((k: any) => `${k.ad}\u0000${k.komut}`));
      for (const k of p3) expect(fk.has(k)).toBe(true);
    }
    const kulliyat = cfg.fazlar.P3.find((k: any) => k.ad === "kulliyat-tara");
    expect(kulliyat.komut).toContain("ac2f18d");
    expect(kulliyat.komut).not.toContain("omniroute-aktarim");
    const muhur = cfg.fazlar.P3.find((k: any) => k.ad === "muhur-kontrol");
    expect(muhur.komut.trim()).toBe("bun araclar/kapilar/muhur-kontrol.ts");
    for (const f of fazlar) {
      for (const k of cfg.fazlar[f]) expect(k.komut).not.toMatch(/--min-ram-gb|--max-cpu-pct/);
    }
  });

  it("c1::depo_makine_sakin_esikleri_o6_onerisi", () => {
    const cfg = (hepsi as any).kapilarTomlYukle(DEPO_TOML);
    expect(cfg.makineSakin.minBosRamGb).toBe(6);
    expect(cfg.makineSakin.maxCpuPct).toBe(70);
    expect(cfg.makineSakin.ornekSn).toBe(60);
    expect(cfg.makineSakin.kip).toBe("uyari");
    expect(cfg.sayim?.kaynakDizini).toBe("ref/omniroute-dbe703a");
  });
});

describe("c1::makine_sakin", () => {
  it("c1::nan_esik_yesil_vermez", async () => {
    const r = await makine.makineDurumuKontrol({ minRamGb: NaN, maxCpuPct: NaN, ornekMs: 100 } as any);
    expect(r.uygun).toBe(false);
  });

  it("c1::cli_gevsetme_bayragi_reddedilir", () => {
    const d = yeniDizin("mgevset");
    tomlYaz(d, MAKINE_UYARI);
    const r = kos(MAKINE, ["--min-ram-gb", "0.1", "--max-cpu-pct", "100"], d);
    expect(r.cikis).not.toBe(0);
    expect(sonSatir(r.stdout)).toMatch(/^HUKUM: kirmizi \S/);
  });

  it("c1::zorunlu_kipte_esik_disi_kirmizi_ve_hukum_basar", () => {
    const d = yeniDizin("mzorunlu");
    tomlYaz(d, `[makine_sakin]\nmin_bos_ram_gb = 99999\nmax_cpu_pct = 70\nornek_sn = 1\nkip = "zorunlu"\n`);
    const r = kos(MAKINE, [], d);
    expect(r.cikis).not.toBe(0);
    expect(sonSatir(r.stdout)).toMatch(/^HUKUM: kirmizi \S/);
  });

  it("c1::uyari_kipinde_esik_disi_uyari_yazar_ve_hukum_basar", () => {
    const d = yeniDizin("muyari");
    tomlYaz(d, `[makine_sakin]\nmin_bos_ram_gb = 99999\nmax_cpu_pct = 70\nornek_sn = 1\nkip = "uyari"\n`);
    const r = kos(MAKINE, [], d);
    expect(r.out).toContain("UYARI");
    expect(r.out).toContain("Ercan onayı");
    expect(sonSatir(r.stdout)).toBe("HUKUM: yesil");
    expect(r.cikis).toBe(0);
  });

  it("c1::esik_yapilandirmasi_yoksa_kirmizi", () => {
    const d = yeniDizin("myok");
    const r = kos(MAKINE, [], d);
    expect(r.cikis).not.toBe(0);
    expect(sonSatir(r.stdout)).toMatch(/^HUKUM: kirmizi \S/);
  });

  it("c1::bilinmeyen_kip_kirmizi", () => {
    const d = yeniDizin("mkip");
    tomlYaz(d, `[makine_sakin]\nmin_bos_ram_gb = 6\nmax_cpu_pct = 70\nornek_sn = 1\nkip = "gevsek"\n`);
    const r = kos(MAKINE, [], d);
    expect(r.cikis).not.toBe(0);
  });

  it("c1::cpu_esigi_100_ve_ustu_olculemez_kirmizi", () => {
    const d = yeniDizin("mcpu");
    tomlYaz(d, `[makine_sakin]\nmin_bos_ram_gb = 6\nmax_cpu_pct = 100\nornek_sn = 1\nkip = "zorunlu"\n`);
    const r = kos(MAKINE, [], d);
    expect(r.cikis).not.toBe(0);
  });
});

const EK_D_TAM = (kanit: string) =>
  [
    `- Sabit yanıt: baktım: ${kanit}`,
    `- Kendini doğrulayan kontrol: baktım: ${kanit}`,
    `- Ulaşılamayan teslim: bakmadım`,
    `- Kanıtı aşan rapor: baktım: ${kanit}`,
    `- Gevşetme: bakmadım`,
    `- Başarısız olamayan test: baktım: ${kanit}`,
  ].join("\n");

function raporDizini(etiket: string) {
  // Rapor gerçek yerleşimde durur: port/kayit/P1/RAPOR.md (başarı günlüğü aynı faz dizininden gelmeli).
  const d = path.join(KOK, `${etiket}-${sayac++}`, "port", "kayit", "P1");
  fs.mkdirSync(d, { recursive: true });
  // hepsi.ts'nin yazdığı biçim (tekKapiKos): başlık, Komut, Zaman, Faz, ham çıktı, BİTTİ, KOŞUCU HÜKMÜ.
  const log = (cikis: number, hukum: string) =>
    `=== KAPI BAŞLADI: x ===\nKomut: bun x.ts\nZaman: 2026-10-09T10:00:00.000Z\nFaz: P1 (kip: faz)\n\nölçtüm\nHUKUM: ${hukum}\n\n` +
    `=== KAPI BİTTİ: Çıkış Kodu = ${cikis} (Süre: 5 ms) ===\n=== KOŞUCU HÜKMÜ: ${hukum} ===\n`;
  fs.writeFileSync(path.join(d, "iyi.log"), log(0, "yesil"));
  fs.writeFileSync(path.join(d, "kotu.log"), log(1, "kirmizi bozuk"));
  fs.writeFileSync(path.join(d, "uydurma.log"), "P5-A1 testleri koştu, hepsi geçti.\nok\n");
  fs.writeFileSync(path.join(d, "bos.txt"), "x\n");
  fs.writeFileSync(path.join(d, "kod.ts"), "satir1\nsatir2\nsatir3\n");
  return d;
}

function raporYaz(d: string, govde: string) {
  const y = path.join(d, "RAPOR.md");
  fs.writeFileSync(y, govde, "utf-8");
  return y;
}

describe("c1::rapor_kontrol", () => {
  // Düzeltildi (2026-10-11): eski hâli elle yazılmış, kapilar.toml'da olmayan 'iyi.log'u başarı kanıtı
  // sayıyordu; bu, yargıcın gösterdiği sahteciliğin ta kendisi. Artık gerçek koşu + kapı adı gerekir.
  it("c1::temiz_rapor_yesil", async () => {
    const { d, tomlYolu } = await gercekKosu("rtemiz", "P1", { iyi: YESIL });
    fs.writeFileSync(path.join(d, "kod.ts"), "satir1\nsatir2\nsatir3\n");
    const y = raporYaz(d, `# R\n\niyi kapısı geçti: [iyi.log](iyi.log)\niyi çalışıyor: \`iyi.log\` (değişen yer \`kod.ts:2\`)\n\n${EK_D_TAM("`iyi.log`")}\n`);
    const s = rapor.raporuDenetle(y, { kapilarToml: tomlYolu });
    expect(s.hatalar).toEqual([]);
    expect(s.uygun).toBe(true);
  });

  it("c1::kanitsiz_yesil_iddiasi_kirmizi", () => {
    const d = raporDizini("ryesil");
    const y = raporYaz(d, `# R\n\nTüm kapılar YEŞİL.\n\n${EK_D_TAM("`iyi.log`")}\n`);
    expect(rapor.raporuDenetle(y).uygun).toBe(false);
  });

  it("c1::olmayan_log_kanit_sayilmaz", () => {
    const d = raporDizini("rlogyok");
    const y = raporYaz(d, `# R\n\nHer şey geçti, bkz. olmayan-dosya.log\n\n${EK_D_TAM("`iyi.log`")}\n`);
    expect(rapor.raporuDenetle(y).uygun).toBe(false);
  });

  it("c1::kirmizi_kapinin_logu_basari_kaniti_olamaz", () => {
    const d = raporDizini("rkotulog");
    const y = raporYaz(d, `# R\n\nKapı geçti: [kotu.log](kotu.log)\n\n${EK_D_TAM("`iyi.log`")}\n`);
    expect(rapor.raporuDenetle(y).uygun).toBe(false);
  });

  it("c1::satir_sayisini_asan_dosya_satir_kanit_sayilmaz", () => {
    const d = raporDizini("rsatir");
    const y = raporYaz(d, `# R\n\nTest tamamlandı: \`kod.ts:999\`\n\n${EK_D_TAM("`iyi.log`")}\n`);
    expect(rapor.raporuDenetle(y).uygun).toBe(false);
  });

  it("c1::kanitsiz_baktim_kirmizi", () => {
    const d = raporDizini("rbaktim");
    const y = raporYaz(
      d,
      `# R\n\n- Sabit yanıt: baktım\n- Kendini doğrulayan kontrol: baktım\n- Ulaşılamayan teslim: baktım\n- Kanıtı aşan rapor: baktım\n- Gevşetme: baktım\n- Başarısız olamayan test: baktım\n`
    );
    expect(rapor.raporuDenetle(y).uygun).toBe(false);
  });

  it("c1::baktim_sahte_kanitla_kirmizi", () => {
    const d = raporDizini("rbaktimsahte");
    const y = raporYaz(d, `# R\n\n${EK_D_TAM("`uydurma/yok.ts:5`")}\n`);
    expect(rapor.raporuDenetle(y).uygun).toBe(false);
  });

  it("c1::baslik_satirindaki_basari_iddiasi_denetlenir", () => {
    const d = raporDizini("rbaslik");
    const y = raporYaz(d, `# P5: tüm kapılar YEŞİL, her şey tamamlandı\n\n${EK_D_TAM("`iyi.log`")}\n`);
    expect(rapor.raporuDenetle(y).uygun).toBe(false);
  });

  it("c1::bakmadim_kelimesi_basari_iddiasini_muaf_tutmaz", () => {
    const d = raporDizini("rbakmadim");
    const y = raporYaz(d, `# R\n\nTüm kapılar YEŞİL (sabit yanıta bakmadım)\n\n${EK_D_TAM("`iyi.log`")}\n`);
    expect(rapor.raporuDenetle(y).uygun).toBe(false);
  });

  it("c1::kosucu_basligi_olmayan_uydurma_log_kanit_sayilmaz", () => {
    const d = raporDizini("ruydurma");
    const y = raporYaz(d, `# R\n\nP5-A1 GEÇTİ: uydurma.log\n\n${EK_D_TAM("`iyi.log`")}\n`);
    expect(rapor.raporuDenetle(y).uygun).toBe(false);
  });

  it("c1::rastgele_dosya_satiri_basari_kaniti_degil", () => {
    const d = raporDizini("rbos1");
    const y = raporYaz(d, `# R\n\nTüm kapılar YEŞİL: bos.txt:1\n\n${EK_D_TAM("`iyi.log`")}\n`);
    expect(rapor.raporuDenetle(y).uygun).toBe(false);
  });

  it("c1::rastgele_dosya_adi_basari_kaniti_degil", () => {
    const d = raporDizini("rbos2");
    const y = raporYaz(d, `# R\n\nTüm kapılar YEŞİL: \`bos.txt\`\n\n${EK_D_TAM("`iyi.log`")}\n`);
    expect(rapor.raporuDenetle(y).uygun).toBe(false);
  });

  it("c1::baktim_ciplak_dosya_adiyla_kanitlanmaz", () => {
    const d = raporDizini("rbaktimciplak");
    fs.writeFileSync(path.join(d, "Cargo.toml"), "[package]\n");
    const y = raporYaz(d, `# R\n\n${EK_D_TAM("`Cargo.toml`")}\n`);
    expect(rapor.raporuDenetle(y).uygun).toBe(false);
  });

  it("c1::baktim_dosya_satiri_ile_kanitlanir", () => {
    const d = raporDizini("rbaktimiyi");
    const y = raporYaz(d, `# R\n\n${EK_D_TAM("`kod.ts:2`")}\n`);
    expect(rapor.raporuDenetle(y).hatalar).toEqual([]);
  });

  it("c1::cli_hukum_satiri_basar", () => {
    const d = raporDizini("rcli");
    const y = raporYaz(d, `# R\n\nTüm kapılar YEŞİL.\n\n${EK_D_TAM("`iyi.log`")}\n`);
    const r = kos(RAPOR, [y], d);
    expect(r.cikis).not.toBe(0);
    expect(sonSatir(r.stdout)).toMatch(/^HUKUM: kirmizi \S/);
  });
});

// ---------------- Yargıç engelleri (2026-10-09 yeniden deneme) ----------------
// 1. ASCII yazım ('yesil', 'gecti', 'calisiyor') iddia sayılmalı. 2. Başarı kanıtı günlüğü yalnız
// raporun kendi port/kayit/<faz>/ dizininde, o fazın faz kipi koşusundan gelmeli. 3. tekKapiKos günlüğü
// diske tam yazılmadan dönmemeli (hemen process.exit hüküm satırını kaybettirmesin).

const EK_D_BAKMADIM = EK_D_MADDELERI_BAKMADIM();
function EK_D_MADDELERI_BAKMADIM() {
  return rapor.EK_D_MADDELERI.map((m) => `- ${m}: bakmadım`).join("\n");
}

function fazKayitDizini(etiket: string, faz: string) {
  const kok = path.join(KOK, `${etiket}-${sayac++}`);
  const d = path.join(kok, "port", "kayit", faz);
  fs.mkdirSync(d, { recursive: true });
  const yesil = path.join(kok, "yesil.ts");
  fs.writeFileSync(yesil, `console.log("ölçtüm"); console.log("HUKUM: yesil");\n`);
  return { kok, d, yesil };
}

async function gercekLog(kayit: string, faz: string, yesil: string, kip: "faz" | "test" = "faz") {
  await hepsi.kapilariKos([{ ad: "y", komut: `bun "${yesil}"` }], faz, kayit, path.join(kayit, "..", `durum-${faz}-${kip}.json`), { kip });
  // finish düzeltmesinden bağımsız olsun: kanıt testleri tam günlükle koşsun (finish ayrı testte).
  await Bun.sleep(300);
  return path.join(kayit, "y.log");
}

describe("c1::yargic_engelleri", () => {
  for (const cumle of ["Tum kapilar yesil.", "Butun testler gecti.", "Sunucu calisiyor.", "Is tamamlandi."]) {
    it(`c1::ascii_iddia_sayilir: ${cumle}`, () => {
      const d = raporDizini("rascii");
      const y = raporYaz(d, `# R\n\n${cumle}\n\n${EK_D_BAKMADIM}\n`);
      const s = rapor.raporuDenetle(y);
      expect(s.kontrolEdilenIddiaSayisi).toBeGreaterThan(0);
      expect(s.uygun).toBe(false);
    });
  }

  // Düzeltildi (2026-10-11): 'y' kapilar.toml'da yoktu; listede olmayan günlük artık kanıt değildir.
  it("c1::kendi_faz_dizinindeki_faz_kipi_logu_kanittir", async () => {
    const { d, tomlYolu } = await gercekKosu("rkendi", "P10", { y: YESIL });
    const y = raporYaz(d, `# R\n\nP10 tüm kapılar yeşil: [y.log](y.log)\n\n${EK_D_BAKMADIM}\n`);
    const s = rapor.raporuDenetle(y, { kapilarToml: tomlYolu });
    expect(s.hatalar).toEqual([]);
    expect(s.uygun).toBe(true);
  });

  it("c1::baska_fazin_logu_mutlak_yolla_kanit_olamaz", async () => {
    const p5 = fazKayitDizini("rp5", "P5");
    const log = await gercekLog(p5.d, "P5", p5.yesil);
    const p10 = path.join(p5.kok, "port", "kayit", "P10");
    fs.mkdirSync(p10, { recursive: true });
    const y = raporYaz(p10, `# R\n\nP10 tüm kapılar yeşil: ${log}\n\n${EK_D_BAKMADIM}\n`);
    expect(rapor.raporuDenetle(y).uygun).toBe(false);
  });

  it("c1::baska_fazin_logu_goreli_yolla_kanit_olamaz", async () => {
    const p5 = fazKayitDizini("rp5g", "P5");
    await gercekLog(p5.d, "P5", p5.yesil);
    const p10 = path.join(p5.kok, "port", "kayit", "P10");
    fs.mkdirSync(p10, { recursive: true });
    const y = raporYaz(p10, `# R\n\nP10 tüm kapılar yeşil: [y.log](../P5/y.log)\n\n${EK_D_BAKMADIM}\n`);
    expect(rapor.raporuDenetle(y).uygun).toBe(false);
  });

  it("c1::baska_fazin_logu_kopyalansa_da_kanit_olamaz", async () => {
    const p5 = fazKayitDizini("rp5k", "P5");
    const log = await gercekLog(p5.d, "P5", p5.yesil);
    const p10 = path.join(p5.kok, "port", "kayit", "P10");
    fs.mkdirSync(p10, { recursive: true });
    fs.copyFileSync(log, path.join(p10, "y.log"));
    const y = raporYaz(p10, `# R\n\nP10 tüm kapılar yeşil: [y.log](y.log)\n\n${EK_D_BAKMADIM}\n`);
    expect(rapor.raporuDenetle(y).uygun).toBe(false);
  });

  it("c1::test_kipi_tmp_logu_kanit_olamaz", async () => {
    const { kok, d, yesil } = fazKayitDizini("rtest", "P10");
    const tmpKayit = fs.mkdtempSync(path.join(kok, "kervan-test-kipi-P10-"));
    const log = await gercekLog(tmpKayit, "P10", yesil, "test");
    const y1 = raporYaz(d, `# R\n\nP10 tüm kapılar yeşil: ${log}\n\n${EK_D_BAKMADIM}\n`);
    expect(rapor.raporuDenetle(y1).uygun).toBe(false);
    // Aynı dizine kopyalansa da test kipi koşusu faz kanıtı değildir.
    fs.copyFileSync(log, path.join(d, "y.log"));
    const y2 = raporYaz(d, `# R\n\nP10 tüm kapılar yeşil: [y.log](y.log)\n\n${EK_D_BAKMADIM}\n`);
    expect(rapor.raporuDenetle(y2).uygun).toBe(false);
  });

  it("c1::faz_dizini_disindaki_rapor_log_kaniti_kullanamaz", async () => {
    const d = path.join(KOK, `rdisari-${sayac++}`);
    fs.mkdirSync(d, { recursive: true });
    fs.writeFileSync(path.join(d, "yesil.ts"), `console.log("HUKUM: yesil");\n`);
    await gercekLog(d, "P10", path.join(d, "yesil.ts"));
    const y = raporYaz(d, `# R\n\nTüm kapılar yeşil: [y.log](y.log)\n\n${EK_D_BAKMADIM}\n`);
    expect(rapor.raporuDenetle(y).uygun).toBe(false);
  });

  it("c1::tekKapiKos_sonrasi_hemen_exit_logu_tam_birakir", () => {
    const { d, yesil } = fazKayitDizini("rfinish", "P1");
    for (let i = 0; i < 3; i++) {
      const log = path.join(d, `k${i}.log`);
      const betik = path.join(d, `kos${i}.ts`);
      fs.writeFileSync(
        betik,
        `import { tekKapiKos } from ${JSON.stringify(HEPSI)};\n` +
          `await tekKapiKos({ ad: "k", komut: ${JSON.stringify(`bun "${yesil}"`)} }, ${JSON.stringify(log)}, 60000, { faz: "P1", kip: "faz" });\n` +
          `process.exit(0);\n`
      );
      const r = kos(betik, [], d);
      expect(r.cikis).toBe(0);
      const icerik = fs.readFileSync(log, "utf-8");
      expect(sonSatir(icerik)).toMatch(/^=== KOŞUCU HÜKMÜ: yesil/);
      expect(rapor.kosucuLogu(icerik)).not.toBeNull();
    }
  });

  it("c1::kapilariKos_donunce_log_okunabilir_tam", async () => {
    const { d, yesil } = fazKayitDizini("rhemen", "P1");
    await hepsi.kapilariKos(
      [{ ad: "a", komut: `bun "${yesil}"` }, { ad: "b", komut: `bun "${yesil}"` }],
      "P1",
      d,
      path.join(d, "..", "durum.json")
    );
    for (const ad of ["a", "b"]) expect(rapor.kosucuLogu(fs.readFileSync(path.join(d, `${ad}.log`), "utf-8"))).not.toBeNull();
  });
});

// ---------------- Yargıç engelleri (2026-10-10 yeniden deneme) ----------------
// 1. rapor-kontrol başarı iddiasını tam dizge ezberiyle arıyordu: biçim karakteri (ZWSP, yumuşak tire),
//    Kiril/Yunan benzer harf, çekimli biçim (geçiyor, tamamlanmış, çalışır) ve eşdeğerler (başarılı, ✅,
//    passed) iddia sayılmıyordu. 2. fazlariOlc elle yazılmış durum.json'u faz sayıyordu: kapı listesi
//    o commit'teki kapilar.toml'la, günlükler port/kayit/<faz>/'la eşleşmiyordu; önceki fazlar aranmıyordu.

describe("c1::yargic_engelleri_2_rapor", () => {
  const SAHTE = [
    "Tüm testler geç​ti.",
    "geç­ti",
    "Tüm testler geçiyor.",
    "Faz başarıyla tamamlanmıştır.",
    "Sunucu çalışır durumda, uçtan uca testler başarılı.",
    "| P3-A1 | ✅ |",
    "All tests passed.",
    "yеşil",
    "Ｇｅçti",
    "Kapı yeşi⁠l.",
    "Tests succeeded, everything works.",
    "- [x] P3-A1",
    // 3. tur yargıç engeli: sözcük ortasındaki Markdown/HTML biçimi ve HTML varlık başvurusu iddiayı gizliyordu.
    "Kapı ge*ç*ti.",
    "Kapı ye<b>şi</b>l.",
    "Faz tamam&#108;andı.",
    "Kapı ge__ç__ti.",
    "Kapı ye~~ş~~il.",
  ];
  for (const cumle of SAHTE) {
    it(`c1::kanitsiz_iddia_yakalanir: ${JSON.stringify(cumle)}`, () => {
      const d = path.join(KOK, `rsahte-${sayac++}`, "port", "kayit", "P3");
      fs.mkdirSync(d, { recursive: true });
      const y = raporYaz(d, `# P3 raporu\n\n${cumle}\n\n${EK_D_BAKMADIM}\n`);
      const s = rapor.raporuDenetle(y);
      expect(s.kontrolEdilenIddiaSayisi).toBeGreaterThan(0);
      expect(s.uygun).toBe(false);
    });
  }

  it("c1::olumsuz_ve_notr_cumleler_iddia_sayilmaz", () => {
    const d = path.join(KOK, `rnotr-${sayac++}`, "port", "kayit", "P3");
    fs.mkdirSync(d, { recursive: true });
    const y = raporYaz(
      d,
      `# P3 raporu\n\nKapı kırmızı: kanıt geçersiz, geçici dosya silindi.\nBaşarısız olan iki test var.\nÇalışma dizini değişmedi.\n\n${EK_D_BAKMADIM}\n`
    );
    const s = rapor.raporuDenetle(y);
    expect(s.hatalar).toEqual([]);
    expect(s.uygun).toBe(true);
  });
});

const TOML_FAZLI =
  MAKINE_UYARI + ["P1", "P2", "P3", "P7"].map((f) => `[[faz.${f}]]\nad = "y"\nkomut = "bun yesil.ts {faz}"\n`).join("");

function fazLogu(ad: string, komut: string, faz: string, cikis = 0, hukum = "yesil") {
  return (
    `=== KAPI BAŞLADI: ${ad} ===\nKomut: ${komut}\nZaman: 2026-10-09T10:00:00.000Z\nFaz: ${faz} (kip: faz)\n\nölçtüm\nHUKUM: ${hukum}\n\n` +
    `=== KAPI BİTTİ: Çıkış Kodu = ${cikis} (Süre: 5 ms) ===\n=== KOŞUCU HÜKMÜ: ${hukum} ===\n`
  );
}

const lfOzetTest = (y: string) =>
  "sha256:" + require("crypto").createHash("sha256").update(fs.readFileSync(y, "utf-8").replace(/\r\n/g, "\n")).digest("hex");

// Bir faz commit'i atar: durum.json özetleri o anki hepsi.ts + kapilar.toml'dan elle hesaplanır (sahteci de
// bunu yapabilir). log: her kapı için port/kayit/<faz>/<ad>.log içeriği (null = günlük yazma).
function fazCommit(d: string, faz: string, kapilar: { ad: string; log: string | null }[]) {
  const kayit = path.join(d, "port", "kayit", faz);
  fs.mkdirSync(kayit, { recursive: true });
  for (const k of kapilar) if (k.log !== null) fs.writeFileSync(path.join(kayit, `${k.ad}.log`), k.log);
  fs.writeFileSync(
    path.join(d, "port", "durum.json"),
    JSON.stringify({
      uretici_ozeti: lfOzetTest(path.join(d, "araclar", "kapilar", "hepsi.ts")),
      kapilar_toml_ozeti: lfOzetTest(path.join(d, "araclar", "kapilar", "kapilar.toml")),
      uretim_zamani: "2026-10-09T10:00:01.000Z",
      faz,
      kip: "faz",
      tum_kapilar_yesil_mi: true,
      kapilar: kapilar.map((k) => ({ ad: k.ad, cikis: 0, hukum: "yesil", log: `port/kayit/${faz}/${k.ad}.log` })),
    })
  );
  git(d, "add", "-A");
  git(d, "commit", "-q", "-m", `${faz} bitti`);
  return git(d, "rev-parse", "HEAD");
}

function fazDeposu(etiket: string) {
  const d = yeniDizin(etiket);
  tamDuzenek(d);
  tomlYaz(d, TOML_FAZLI);
  fs.writeFileSync(path.join(d, "araclar", "kapilar", "hepsi.ts"), "// koşucu\n");
  git(d, "add", "-A");
  git(d, "commit", "-q", "-m", "kapilar");
  return d;
}

const iyiLog = (faz: string) => fazLogu("y", `bun yesil.ts ${faz}`, faz);

describe("c1::yargic_engelleri_2_fazlar", () => {
  it("c1::elle_yazilmis_durum_faz_sayilmaz_yargic_denemesi", () => {
    const d = fazDeposu("felle");
    fazCommit(d, "P1", [{ ad: "y", log: iyiLog("P1") }]);
    fazCommit(d, "P2", [{ ad: "y", log: iyiLog("P2") }]);
    fazCommit(d, "P3", [{ ad: "y", log: iyiLog("P3") }]);
    fazCommit(d, "P7", [{ ad: "elle", log: null }]);
    expect(hepsi.fazlariOlc(d).map((f) => f.id)).not.toContain("P7");
  });

  it("c1::liste_tutsa_da_gunluk_yoksa_faz_sayilmaz", () => {
    const d = fazDeposu("fgunluksuz");
    fazCommit(d, "P1", [{ ad: "y", log: null }]);
    expect(hepsi.fazlariOlc(d)).toEqual([]);
  });

  it("c1::kirmizi_gunluk_faz_sayilmaz", () => {
    const d = fazDeposu("fkirmizi");
    fazCommit(d, "P1", [{ ad: "y", log: fazLogu("y", "bun yesil.ts P1", "P1", 1, "kirmizi bozuk") }]);
    expect(hepsi.fazlariOlc(d)).toEqual([]);
  });

  it("c1::gunluk_komutu_toml_komutu_degilse_faz_sayilmaz", () => {
    const d = fazDeposu("fkomut");
    fazCommit(d, "P1", [{ ad: "y", log: fazLogu("y", "bun baska.ts", "P1") }]);
    expect(hepsi.fazlariOlc(d)).toEqual([]);
  });

  it("c1::gunluk_baska_fazin_ise_faz_sayilmaz", () => {
    const d = fazDeposu("fbaskafaz");
    fazCommit(d, "P1", [{ ad: "y", log: fazLogu("y", "bun yesil.ts P1", "P9") }]);
    expect(hepsi.fazlariOlc(d)).toEqual([]);
  });

  it("c1::onceki_faz_yoksa_sonraki_faz_sayilmaz", () => {
    const d = fazDeposu("fonceki");
    fazCommit(d, "P2", [{ ad: "y", log: iyiLog("P2") }]);
    expect(hepsi.fazlariOlc(d)).toEqual([]);
  });

  it("c1::tam_ve_sirali_fazlar_sayilir", () => {
    const d = fazDeposu("ftam");
    const s1 = fazCommit(d, "P1", [{ ad: "y", log: iyiLog("P1") }]);
    const s2 = fazCommit(d, "P2", [{ ad: "y", log: iyiLog("P2") }]);
    expect(hepsi.fazlariOlc(d)).toEqual([
      { id: "P1", commit: s1, kanit: ["y"] },
      { id: "P2", commit: s2, kanit: ["y"] },
    ]);
  });
});

// ---------------- Yargıç engelleri (2026-10-11 yeniden deneme) ----------------
// 1. Koşucu biçiminde elle yazılmış, adı ne olursa olsun bir günlük (uydurma.log) başarı kanıtı sayılıyordu.
//    Kanıt yalnız kapilar.toml'daki o fazın kapı adına ait <ad>.log olabilir ve hepsi.ts'nin o koşuda
//    yazdığı port/kayit/<faz>/kosu.json kaydıyla (başlangıç zamanı, çıkış, hüküm) tutmalıdır.
// 2. İddia onu ölçen kapıya bağlanmıyordu: makine-sakin.log (kip=uyari, hep yeşil) her iddiayı kanıtlıyordu.

function tomlLiteral(s: string) {
  return `'${s}'`;
}

/** Geçici kapilar.toml + gerçek kapilariKos koşusu (faz kipi). kapilar: ad -> betik içeriği. */
async function gercekKosu(etiket: string, faz: string, kapilar: Record<string, string>) {
  const kok = path.join(KOK, `${etiket}-${sayac++}`);
  const d = path.join(kok, "port", "kayit", faz);
  fs.mkdirSync(d, { recursive: true });
  const gorevler: hepsi.KapiGorevi[] = [];
  let toml = MAKINE_UYARI;
  for (const [ad, govde] of Object.entries(kapilar)) {
    const betik = path.join(kok, `${ad}.ts`);
    fs.writeFileSync(betik, govde);
    const komut = `bun "${betik}"`;
    // P4+ için kapilar.toml P3 üst kümesi ister: aynı liste P3'e de yazılır.
    for (const f of new Set([faz, Number(faz.slice(1)) >= 4 ? "P3" : faz])) {
      toml += `[[faz.${f}]]\nad = "${ad}"\nkomut = ${tomlLiteral(komut)}\n`;
    }
    gorevler.push({ ad, komut });
  }
  const tomlYolu = path.join(kok, "kapilar.toml");
  fs.writeFileSync(tomlYolu, toml);
  await hepsi.kapilariKos(gorevler, faz, d, path.join(kok, "port", "durum.json"), { kip: "faz" });
  return { kok, d, tomlYolu };
}

const YESIL = `console.log("ölçtüm"); console.log("HUKUM: yesil");\n`;
const KIRMIZI = `console.log("HUKUM: kirmizi bozuk"); process.exit(1);\n`;

describe("c1::yargic_engelleri_3_kanit", () => {
  const uydurmaLog = (ad: string, komut: string) =>
    `=== KAPI BAŞLADI: ${ad} ===\nKomut: ${komut}\nZaman: 2026-10-11T10:00:00.000Z\nFaz: P3 (kip: faz)\n` +
    `=== KAPI BİTTİ: Çıkış Kodu = 0 (Süre: 5 ms) ===\n=== KOŞUCU HÜKMÜ: yesil ===\n`;

  it("c1::yargic_denemesi_elle_uydurma_log_kanit_olamaz", () => {
    const d = path.join(KOK, `ruyd-${sayac++}`, "port", "kayit", "P3");
    fs.mkdirSync(d, { recursive: true });
    fs.writeFileSync(path.join(d, "uydurma.log"), uydurmaLog("uydurma", "elle"));
    const y = raporYaz(d, `# P3\n\nTüm kabul kriterleri geçti, mutasyon testleri yeşil, sunucu çalışıyor: [kanıt](uydurma.log)\n\n${EK_D_BAKMADIM}\n`);
    expect(rapor.raporuDenetle(y).uygun).toBe(false);
    const r = kos(RAPOR, ["P3"], path.join(d, "..", "..", ".."));
    expect(r.cikis).not.toBe(0);
    expect(sonSatir(r.stdout)).toMatch(/^HUKUM: kirmizi \S/);
  });

  it("c1::listedeki_ada_elle_yazilan_log_kosu_kaydi_olmadan_kanit_olamaz", () => {
    const cfg = hepsi.kapilarTomlYukle(DEPO_TOML);
    const k = cfg.fazlar.P3.find((x) => x.ad === "kulliyat-tara")!;
    const d = path.join(KOK, `rlisteli-${sayac++}`, "port", "kayit", "P3");
    fs.mkdirSync(d, { recursive: true });
    fs.writeFileSync(path.join(d, "kulliyat-tara.log"), uydurmaLog("kulliyat-tara", k.komut.replaceAll("{faz}", "P3")));
    const y = raporYaz(d, `# P3\n\nkulliyat-tara yeşil: [k](kulliyat-tara.log)\n\n${EK_D_BAKMADIM}\n`);
    expect(rapor.raporuDenetle(y).uygun).toBe(false);
  });

  it("c1::uyari_kipindeki_makine_sakin_logu_kanit_olamaz", async () => {
    const { d, tomlYolu } = await gercekKosu("rmakine", "P3", { "makine-sakin": YESIL });
    const y = raporYaz(d, `# P3\n\nP3-A1 geçti, mutasyon yeşil, makine-sakin yeşil [k](makine-sakin.log)\n\n${EK_D_BAKMADIM}\n`);
    expect(rapor.raporuDenetle(y, { kapilarToml: tomlYolu }).uygun).toBe(false);
  });

  it("c1::iddia_kanit_kapisinin_adini_tasimali", async () => {
    const { d, tomlYolu } = await gercekKosu("rbag", "P3", { ornek: YESIL, "mutasyon-kontrol": KIRMIZI });
    const y = raporYaz(d, `# P3\n\nmutasyon-kontrol yeşil: [k](ornek.log)\n\n${EK_D_BAKMADIM}\n`);
    expect(rapor.raporuDenetle(y, { kapilarToml: tomlYolu }).uygun).toBe(false);
    const y2 = raporYaz(d, `# P3\n\nHer şey geçti: [k](ornek.log)\n\n${EK_D_BAKMADIM}\n`);
    expect(rapor.raporuDenetle(y2, { kapilarToml: tomlYolu }).uygun).toBe(false);
  });

  it("c1::tum_iddiasi_tek_kapi_logunu_yetmez", async () => {
    const { d, tomlYolu } = await gercekKosu("rtum", "P3", { ornek: YESIL, ikinci: YESIL });
    const y = raporYaz(d, `# P3\n\nTüm kapılar yeşil: [k](ornek.log)\n\n${EK_D_BAKMADIM}\n`);
    expect(rapor.raporuDenetle(y, { kapilarToml: tomlYolu }).uygun).toBe(false);
  });

  it("c1::kosudan_sonra_yesile_cevrilen_log_kanit_olamaz", async () => {
    const { d, tomlYolu } = await gercekKosu("rcevir", "P3", { ornek: KIRMIZI });
    const yol = path.join(d, "ornek.log");
    const eski = fs.readFileSync(yol, "utf-8");
    fs.writeFileSync(
      yol,
      eski
        .replace(/Çıkış Kodu = 1 /, "Çıkış Kodu = 0 ")
        .replace(/KOŞUCU HÜKMÜ: kirmizi.*===/, "KOŞUCU HÜKMÜ: yesil ===")
    );
    expect(rapor.kosucuLogu(fs.readFileSync(yol, "utf-8"))?.hukum).toBe("yesil");
    const y = raporYaz(d, `# P3\n\nornek yeşil: [k](ornek.log)\n\n${EK_D_BAKMADIM}\n`);
    expect(rapor.raporuDenetle(y, { kapilarToml: tomlYolu }).uygun).toBe(false);
  });

  it("c1::kosu_basinda_listedeki_eski_loglar_silinir", async () => {
    const kok = path.join(KOK, `rsil-${sayac++}`);
    const d = path.join(kok, "port", "kayit", "P3");
    fs.mkdirSync(d, { recursive: true });
    fs.writeFileSync(path.join(d, "b.log"), uydurmaLog("b", "elle"));
    const a = path.join(kok, "a.ts");
    fs.writeFileSync(a, `console.log(require("fs").existsSync(${JSON.stringify(path.join(d, "b.log"))}) ? "HUKUM: kirmizi eski b.log duruyor" : "HUKUM: yesil");\n`);
    const b = path.join(kok, "b.ts");
    fs.writeFileSync(b, YESIL);
    const s = await hepsi.kapilariKos([{ ad: "a", komut: `bun "${a}"` }, { ad: "b", komut: `bun "${b}"` }], "P3", d, path.join(kok, "port", "durum.json"));
    expect(s.sonuclar[0].hukum).toBe("yesil");
  });

  it("c1::gercek_kosu_logu_ve_kapi_adi_ile_iddia_kanitlanir", async () => {
    const { d, tomlYolu } = await gercekKosu("rgercek", "P3", { ornek: YESIL, ikinci: YESIL });
    const y = raporYaz(
      d,
      `# P3\n\nornek yeşil: [k](ornek.log)\nTüm kapılar yeşil: [a](ornek.log) [b](ikinci.log)\n\n${EK_D_BAKMADIM}\n`
    );
    const s = rapor.raporuDenetle(y, { kapilarToml: tomlYolu });
    expect(s.hatalar).toEqual([]);
    expect(s.uygun).toBe(true);
  });
});
