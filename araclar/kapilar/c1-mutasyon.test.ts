// c1-mutasyon: güven kökü kapılarının (mutasyon-kontrol, kabul-haritasi, uc-kaydi,
// sabit-yanit, ulasilabilirlik) kandırılamadığını sınar.
// Bütün geçici dosyalar os.tmpdir()/kervan-c1-mutasyon-* altında; depo ağacına yazılmaz.
// Gerçek cargo koşulmaz: PATH'in başına konan sahte cargo.exe (bun build --compile)
// senaryo dosyasına göre cargo çıktısı basar ve her çağrının argv'sini kaydeder.
import { afterAll, beforeAll, beforeEach, describe, expect, it } from "bun:test";
import { spawnSync } from "child_process";
import fs from "fs";
import os from "os";
import path from "path";

import { mutantlariDene, mutantlarTomlOku, pariteBaglantisiniDenetle } from "./mutasyon-kontrol.ts";
import { kabulHaritasiniDogrula } from "./kabul-haritasi.ts";
import { ucKaydiniDenetle } from "./uc-kaydi.ts";
import { dosyayiTara, sabitYanitlariTara } from "./sabit-yanit.ts";
import { ulasilabilirlikDenetle } from "./ulasilabilirlik.ts";

const KAPI_DIZINI = import.meta.dir;
const KOK = fs.mkdtempSync(path.join(os.tmpdir(), "kervan-c1-mutasyon-"));
const SAHTE_DIZIN = path.join(KOK, "sahte-cargo");
const ESKI_PATH = process.env.PATH;

const SAHTE_CARGO_KAYNAK = `
import fs from "fs";
import path from "path";
const dizin = process.env.C1_SAHTE_CARGO_DIZIN;
if (!dizin) { console.error("C1_SAHTE_CARGO_DIZIN yok"); process.exit(97); }
const argv = process.argv.slice(2);
fs.appendFileSync(path.join(dizin, "cagrilar.jsonl"),
  JSON.stringify({ argv, jobs: process.env.CARGO_BUILD_JOBS ?? null }) + "\\n");
const s = JSON.parse(fs.readFileSync(path.join(dizin, "senaryo.json"), "utf-8"));
const bas = (c) => {
  if (c.uykuMs) Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, c.uykuMs);
  process.stdout.write(c.stdout ?? ""); process.stderr.write(c.stderr ?? ""); process.exit(c.kod ?? 0);
};
if (argv[0] === "metadata") bas({ stdout: s.metadata ?? "{}", kod: s.metadataKod ?? 0 });
if (argv[0] === "test" && argv.includes("--list")) bas({ stdout: s.liste ?? "", kod: s.listeKod ?? 0 });
if (argv[0] === "test" && s.isaretler && s.hedef && fs.existsSync(s.hedef)) {
  const icerik = fs.readFileSync(s.hedef, "latin1");
  for (const [isaret, c] of Object.entries(s.isaretler)) if (icerik.includes(isaret)) bas(c);
}
if (argv[0] === "test") {
  const mutasyonlu = !!s.hedef && fs.existsSync(s.hedef) && fs.readFileSync(s.hedef, "latin1").includes(s.isaret);
  bas(mutasyonlu ? s.mutantCikti : s.temizCikti);
}
console.error("sahte cargo: bilinmeyen komut " + argv.join(" "));
process.exit(98);
`;

const OK = {
  stdout: "running 1 test\ntest c1::t ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n",
  kod: 0,
};
const DUSTU = {
  stdout: "running 1 test\ntest c1::t ... FAILED\n\nfailures:\n\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n",
  stderr: "error: test failed, to rerun pass `--lib`\n",
  kod: 101,
};
const DERLENMEDI = {
  stderr: "error[E0425]: cannot find value `x` in this scope\nerror: could not compile `kervan-sunucu` (lib) due to 1 previous error\n",
  kod: 101,
};

function metadata(paketler: { ad: string; testler?: string[]; bin?: boolean }[], kenarlar: [string, string, string | null][] = []) {
  const id = (ad: string) => `path+file:///c1/${ad}#${ad}@0.1.0`;
  return JSON.stringify({
    workspace_members: paketler.map((p) => id(p.ad)),
    packages: paketler.map((p) => ({
      id: id(p.ad),
      name: p.ad,
      manifest_path: path.join(KOK, p.ad, "Cargo.toml"),
      targets: [
        { kind: [p.bin ? "bin" : "lib"], name: p.ad.replace(/-/g, "_") },
        ...(p.testler ?? []).map((t) => ({ kind: ["test"], name: t })),
      ],
    })),
    resolve: {
      nodes: paketler.map((p) => {
        const benim = kenarlar.filter((k) => k[0] === p.ad);
        return {
          id: id(p.ad),
          dependencies: benim.map((k) => id(k[1])),
          deps: benim.map((k) => ({ name: k[1].replace(/-/g, "_"), pkg: id(k[1]), dep_kinds: [{ kind: k[2], target: null }] })),
        };
      }),
    },
  });
}

function senaryo(s: Record<string, unknown>) {
  fs.writeFileSync(path.join(SAHTE_DIZIN, "senaryo.json"), JSON.stringify(s));
  fs.writeFileSync(path.join(SAHTE_DIZIN, "cagrilar.jsonl"), "");
}
function cagrilar(): { argv: string[]; jobs: string | null }[] {
  return fs.readFileSync(path.join(SAHTE_DIZIN, "cagrilar.jsonl"), "utf-8")
    .split("\n").filter(Boolean).map((l) => JSON.parse(l));
}
function yeniDizin(ad: string): string {
  return fs.mkdtempSync(path.join(KOK, `${ad}-`));
}
function kapiKos(kapi: string, args: string[], cwd: string) {
  const r = spawnSync("bun", [path.join(KAPI_DIZINI, kapi), ...args], { cwd, encoding: "utf-8", timeout: 120000 });
  const cikti = `${r.stdout ?? ""}\n${r.stderr ?? ""}`;
  const satirlar = (r.stdout ?? "").split(/\r?\n/).filter((s) => s.trim());
  return { kod: r.status, cikti, sonSatir: satirlar[satirlar.length - 1] ?? "" };
}

// Mutasyon fikstürü: hedef dosyada `let x = 1;` -> `let x = 2; // C1_MUTANT`
function mutantFiksturu(ad: string, testler: string[], icerik?: Buffer) {
  const dizin = yeniDizin(ad);
  const hedef = path.join(dizin, "hedef.rs");
  fs.writeFileSync(hedef, icerik ?? Buffer.from("fn f() {\n    let x = 1;\n}\n"));
  return {
    dizin,
    hedef,
    gunluk: path.join(dizin, "gunluk"),
    yedek: path.join(dizin, "yedek"),
    mutant: { ad, dosya: hedef, bul: "let x = 1;", yerine: "let x = 2; // C1_MUTANT", oldurmesi_gereken_test: testler },
  };
}
const META_CEVIRI = metadata([{ ad: "kervan", bin: true }, { ad: "kervan-ceviri", testler: ["p6_parite_test"] }], [["kervan", "kervan-ceviri", null]]);

beforeAll(() => {
  fs.mkdirSync(SAHTE_DIZIN, { recursive: true });
  const kaynak = path.join(SAHTE_DIZIN, "sahte-cargo.ts");
  fs.writeFileSync(kaynak, SAHTE_CARGO_KAYNAK);
  const b = spawnSync("bun", ["build", "--compile", kaynak, "--outfile", path.join(SAHTE_DIZIN, "cargo.exe")], { encoding: "utf-8" });
  if (b.status !== 0) throw new Error(`sahte cargo derlenemedi: ${b.stderr}`);
  process.env.C1_SAHTE_CARGO_DIZIN = SAHTE_DIZIN;
  process.env.PATH = SAHTE_DIZIN + path.delimiter + ESKI_PATH;
}, 60000);
beforeEach(() => senaryo({}));
afterAll(() => {
  process.env.PATH = ESKI_PATH;
  fs.rmSync(KOK, { recursive: true, force: true });
});

describe("c1-mutasyon::mutasyon-kontrol", () => {
  it("c1::m1_taban_kirmizi_ise_mutant_olmus_sayilmaz", () => {
    const f = mutantFiksturu("m1", ["c1::t"]);
    senaryo({ liste: "c1::t: test\n", metadata: META_CEVIRI, hedef: f.hedef, isaret: "C1_MUTANT", temizCikti: DUSTU, mutantCikti: DUSTU });
    const s = mutantlariDene([f.mutant], { gunlukDizini: f.gunluk, kokDizin: f.dizin, yedekDizini: f.yedek } as any);
    expect(s.uygun).toBe(false);
    expect(s.hatalar.join("\n")).toContain("taban kırmızı");
  }, 60000);

  it("c1::m2_derleme_hatasi_olum_degil_belirsizdir", () => {
    const f = mutantFiksturu("m2", ["c1::t"]);
    senaryo({ liste: "c1::t: test\n", metadata: META_CEVIRI, hedef: f.hedef, isaret: "C1_MUTANT", temizCikti: OK, mutantCikti: DERLENMEDI });
    const s = mutantlariDene([f.mutant], { gunlukDizini: f.gunluk, kokDizin: f.dizin, yedekDizini: f.yedek } as any);
    expect(s.uygun).toBe(false);
    expect(s.oldurulenSayisi).toBe(0);
    expect(s.hatalar.join("\n")).toContain("belirsiz");
  }, 60000);

  it("c1::m3_zaman_asimi_olum_degil_belirsizdir", () => {
    const f = mutantFiksturu("m3", ["c1::t"]);
    senaryo({ liste: "c1::t: test\n", metadata: META_CEVIRI, hedef: f.hedef, isaret: "C1_MUTANT", temizCikti: OK, mutantCikti: { ...DUSTU, uykuMs: 6000 } });
    const s = mutantlariDene([f.mutant], { gunlukDizini: f.gunluk, kokDizin: f.dizin, yedekDizini: f.yedek, zamanAsimiMs: 2500 } as any);
    expect(s.uygun).toBe(false);
    expect(s.hatalar.join("\n")).toContain("belirsiz");
  }, 60000);

  it("c1::m4_gercek_test_hatasi_olumdur_ve_ham_cikti_gunluge_yazilir", () => {
    const f = mutantFiksturu("m4", ["c1::t"]);
    senaryo({ liste: "c1::t: test\n", metadata: META_CEVIRI, hedef: f.hedef, isaret: "C1_MUTANT", temizCikti: OK, mutantCikti: DUSTU });
    const s = mutantlariDene([f.mutant], { gunlukDizini: f.gunluk, kokDizin: f.dizin, yedekDizini: f.yedek } as any);
    expect(s.hatalar).toEqual([]);
    expect(s.uygun).toBe(true);
    expect(s.oldurulenSayisi).toBe(1);
    // Ham çıktı argümanla verilen günlük dizinine yazılmış olmalı (taban + mutant).
    expect(fs.existsSync(f.gunluk)).toBe(true);
    const hepsi = fs.readdirSync(f.gunluk).map((d) => fs.readFileSync(path.join(f.gunluk, d), "utf-8")).join("\n");
    expect(hepsi).toContain("test result: FAILED");
    expect(hepsi).toContain("test result: ok. 1 passed");
    // Bare test adı tam adla (--exact) ve CARGO_BUILD_JOBS=2 ile koşulmuş olmalı.
    const kosular = cagrilar().filter((c) => c.argv[0] === "test" && !c.argv.includes("--list"));
    expect(kosular.length).toBeGreaterThanOrEqual(2);
    for (const c of kosular) {
      expect(c.argv).toContain("--exact");
      expect(c.jobs).toBe("2");
    }
  }, 60000);

  it("c1::m5_cargo_ile_baslayan_test_komutu_oldugu_gibi_kosulur", () => {
    const f = mutantFiksturu("m5", ["cargo test -p kervan-ceviri --test p6_parite_test"]);
    senaryo({ liste: "", metadata: META_CEVIRI, hedef: f.hedef, isaret: "C1_MUTANT", temizCikti: OK, mutantCikti: DUSTU });
    mutantlariDene([f.mutant], { gunlukDizini: f.gunluk, kokDizin: f.dizin, yedekDizini: f.yedek } as any);
    const kosular = cagrilar().filter((c) => c.argv[0] === "test");
    expect(kosular.length).toBeGreaterThanOrEqual(1);
    for (const c of kosular) {
      expect(c.argv.slice(0, 5)).toEqual(["test", "-p", "kervan-ceviri", "--test", "p6_parite_test"]);
      expect(c.argv.some((a) => a.includes("cargo test"))).toBe(false);
    }
  }, 60000);

  it("c1::m6_var_olmayan_test_hedefi_kirmizidir", () => {
    const f = mutantFiksturu("m6", ["cargo test -p kervan-ceviri --test olmayan_hedef_test"]);
    const g = mutantFiksturu("m6b", ["yok::boyle_bir_test"]);
    senaryo({ liste: "c1::t: test\n", metadata: META_CEVIRI, hedef: f.hedef, isaret: "C1_MUTANT", temizCikti: OK, mutantCikti: DUSTU });
    const s1 = mutantlariDene([f.mutant], { gunlukDizini: f.gunluk, kokDizin: f.dizin, yedekDizini: f.yedek } as any);
    expect(s1.uygun).toBe(false);
    expect(s1.hatalar.join("\n")).toContain("olmayan_hedef_test");
    senaryo({ liste: "c1::t: test\n", metadata: META_CEVIRI, hedef: g.hedef, isaret: "C1_MUTANT", temizCikti: OK, mutantCikti: DUSTU });
    const s2 = mutantlariDene([g.mutant], { gunlukDizini: g.gunluk, kokDizin: g.dizin, yedekDizini: g.yedek } as any);
    expect(s2.uygun).toBe(false);
    expect(s2.hatalar.join("\n")).toContain("yok::boyle_bir_test");
  }, 60000);

  it("c1::m7_mutasyon_bayt_bayt_geri_alinir", () => {
    // BOM + CRLF + geçersiz UTF-8 baytı: utf-8 okuyup yazan geri alma bunu bozar.
    const icerik = Buffer.concat([
      Buffer.from([0xef, 0xbb, 0xbf]),
      Buffer.from("fn f() {\r\n    // "),
      Buffer.from([0xff, 0xfe]),
      Buffer.from("\r\n    let x = 1;\r\n}\r\n"),
    ]);
    const f = mutantFiksturu("m7", ["c1::t"], icerik);
    senaryo({ liste: "c1::t: test\n", metadata: META_CEVIRI, hedef: f.hedef, isaret: "C1_MUTANT", temizCikti: OK, mutantCikti: DUSTU });
    mutantlariDene([f.mutant], { gunlukDizini: f.gunluk, kokDizin: f.dizin, yedekDizini: f.yedek } as any);
    expect(fs.readFileSync(f.hedef).equals(icerik)).toBe(true);
  }, 60000);

  it("c1::m8_yarida_kalmis_onceki_kosunun_yedegi_kirmizidir", () => {
    const f = mutantFiksturu("m8", ["c1::t"]);
    fs.mkdirSync(f.yedek, { recursive: true });
    fs.writeFileSync(path.join(f.yedek, "artik.yedek"), "eski");
    fs.writeFileSync(path.join(f.yedek, "artik.yol"), f.hedef);
    senaryo({ liste: "c1::t: test\n", metadata: META_CEVIRI, hedef: f.hedef, isaret: "C1_MUTANT", temizCikti: OK, mutantCikti: DUSTU });
    const s = mutantlariDene([f.mutant], { gunlukDizini: f.gunluk, kokDizin: f.dizin, yedekDizini: f.yedek } as any);
    expect(s.uygun).toBe(false);
    expect(s.hatalar.join("\n")).toContain("yarıda");
  }, 60000);

  // Emeklilik fikstürü: iki mutant aynı dosyada; A (emekli) C1_BOZUK, B (yerine geçen) C1_MUTANT yazar.
  function emekliFiksturu(ad: string) {
    const f = mutantFiksturu(ad, ["c1::t"], Buffer.from("fn f() {\n    let x = 1;\n    let y = 1;\n}\n"));
    const a = { ...f.mutant, ad: `${ad}-a`, yerine: "let x = 2; // C1_BOZUK", emekli: { neden: "derlenmiyor", yerine_gecen: `${ad}-b` } };
    const b = { ...f.mutant, ad: `${ad}-b`, bul: "let y = 1;", yerine: "let y = 2; // C1_MUTANT" };
    return { f, a, b };
  }
  const emekliSenaryo = (f: { hedef: string }, bozuk: unknown, yeni: unknown) =>
    senaryo({ liste: "c1::t: test\n", metadata: META_CEVIRI, hedef: f.hedef, isaretler: { C1_BOZUK: bozuk, C1_MUTANT: yeni }, temizCikti: OK });

  it("c1::m21_derlenmeyen_mutant_emekli_ve_yerine_gecen_olunce_yesil", () => {
    const { f, a, b } = emekliFiksturu("m21");
    emekliSenaryo(f, DERLENMEDI, DUSTU);
    const s = mutantlariDene([a, b], { gunlukDizini: f.gunluk, kokDizin: f.dizin, yedekDizini: f.yedek } as any);
    expect(s.hatalar).toEqual([]);
    expect(s.sonuclar.map((r) => r.sinif)).toEqual(["emekli", "oldu"]);
  }, 60000);

  it("c1::m22_derlenen_mutant_emekli_edilemez", () => {
    const { f, a, b } = emekliFiksturu("m22");
    emekliSenaryo(f, OK, DUSTU);
    const s = mutantlariDene([a, b], { gunlukDizini: f.gunluk, kokDizin: f.dizin, yedekDizini: f.yedek } as any);
    expect(s.uygun).toBe(false);
    expect(s.hatalar.join("\n")).toContain("emeklilik geçersiz");
  }, 60000);

  it("c1::m23_yerine_gecen_yasarsa_emeklilik_kirmizi", () => {
    const { f, a, b } = emekliFiksturu("m23");
    emekliSenaryo(f, DERLENMEDI, OK);
    const s = mutantlariDene([a, b], { gunlukDizini: f.gunluk, kokDizin: f.dizin, yedekDizini: f.yedek } as any);
    expect(s.uygun).toBe(false);
    expect(s.hatalar.join("\n")).toContain("öldürülmedi");
  }, 60000);

  it("c1::m24_emekli_kaydi_dogrulanir", () => {
    const d = yeniDizin("m24");
    const toml = path.join(d, "mutantlar.toml");
    const m = (ad: string) => `[[mutant]]\nad = "${ad}"\ndosya = "b.rs"\nbul = "c"\nyerine = "d"\noldurmesi_gereken_test = ["t"]\n`;
    const dene = (ek: string) => { fs.writeFileSync(toml, m("a") + m("b") + ek); return () => mutantlarTomlOku(toml); };
    expect(dene(`[[emekli]]\nad = "a"\nneden = "n"\nyerine_gecen = "yok"\n`)).toThrow("listede yok");
    expect(dene(`[[emekli]]\nad = "a"\nneden = "n"\nyerine_gecen = "a"\n`)).toThrow("emekli olamaz");
    expect(dene(`[[emekli]]\nad = "a"\nneden = ""\nyerine_gecen = "b"\n`)).toThrow("'neden'");
    expect(dene(`[[emekli]]\nad = "a"\nneden = "n"\nyerine_gecen = "b"\n`)()[0].emekli).toEqual({ neden: "n", yerine_gecen: "b" });
  });

  it("c1::m9_toml_virgullu_ve_koseli_ayracli_test_komutunu_bozmadan_okur", () => {
    const d = yeniDizin("m9");
    const komut = `bun -e "import { spawnSync } from 'child_process'; const r = spawnSync('cargo', ['test', '-p', 'x', '--test', 'y']); process.exit(r.status ?? 1);"`;
    const toml = path.join(d, "mutantlar.toml");
    fs.writeFileSync(toml, `[[mutant]]\nad = "a"\ndosya = "b.rs"\nbul = "c"\nyerine = "d"\noldurmesi_gereken_test = [${JSON.stringify(komut)}]\n`);
    const m = mutantlarTomlOku(toml);
    expect(m.length).toBe(1);
    expect(m[0].oldurmesi_gereken_test).toEqual([komut]);
  });

  it("c1::m10_mutantlar_toml_yoksa_ya_da_8den_azsa_kapi_kirmizidir", () => {
    const d = yeniDizin("m10");
    senaryo({ liste: "c1::t: test\n", metadata: META_CEVIRI, temizCikti: OK, mutantCikti: DUSTU });
    const yok = kapiKos("mutasyon-kontrol.ts", ["--gunluk", path.join(d, "g1"), "--mutantlar", path.join(d, "port", "mutantlar.toml")], d);
    expect(yok.kod).not.toBe(0);
    expect(yok.sonSatir).toMatch(/^HUKUM: kirmizi \S/);

    fs.mkdirSync(path.join(d, "port"), { recursive: true });
    fs.writeFileSync(path.join(d, "hedef.rs"), "let x = 1;\n");
    fs.writeFileSync(path.join(d, "port", "mutantlar.toml"),
      `[[mutant]]\nad = "rest-kancasini-kaldir"\ndosya = "hedef.rs"\nbul = "let x = 1;"\nyerine = "let x = 2;"\noldurmesi_gereken_test = ["c1::t"]\n`);
    const az = kapiKos("mutasyon-kontrol.ts", ["--gunluk", path.join(d, "g2"), "--mutantlar", path.join(d, "port", "mutantlar.toml")], d);
    expect(az.kod).not.toBe(0);
    expect(az.cikti).toContain("8");
    expect(az.sonSatir).toMatch(/^HUKUM: kirmizi \S/);
  }, 120000);

  it("c1::m11_yorumdaki_fetch_parite_baglantisi_sayilmaz_dizin_yoksa_kirmizi", () => {
    const d = yeniDizin("m11");
    fs.writeFileSync(path.join(d, "kos.ts"), `// fetch( ile Kervan'a bağlanırdık ama bağlanmıyoruz\n/* Bun.spawn */\nconst kervanYaniti = "x";\nconsole.log(kervanYaniti === "x");\n`);
    expect(pariteBaglantisiniDenetle(d).uygun).toBe(false);
    expect(pariteBaglantisiniDenetle(path.join(d, "olmayan")).uygun).toBe(false);
  });

  // Yargıç probları: parite bağlantısı metinle değil çalışma zamanında ölçülmeli.
  function pariteFiksturu(ad: string, betik: string) {
    const d = yeniDizin(ad);
    const parite = path.join(d, "araclar", "parite");
    fs.mkdirSync(parite, { recursive: true });
    fs.writeFileSync(path.join(parite, "kos.ts"), betik);
    senaryo({ metadata: META_CEVIRI });
    return { d, parite };
  }

  it("c1::m12_dizge_sabitindeki_fetch_baglanti_sayilmaz", () => {
    const f = pariteFiksturu("m12", `const not = "fetch(";\nconsole.log(not);\n`);
    expect(pariteBaglantisiniDenetle(f.parite, { kokDizin: f.d } as any).uygun).toBe(false);
  }, 60000);

  it("c1::m13_hic_cagrilmayan_islevdeki_fetch_ve_spawn_sayilmaz", () => {
    const f = pariteFiksturu("m13", `function hic() { fetch("http://x"); Bun.spawnSync(["cargo", "run", "-p", "kervan-ceviri"]); }\nconsole.log("eşit");\n`);
    expect(pariteBaglantisiniDenetle(f.parite, { kokDizin: f.d } as any).uygun).toBe(false);
  }, 60000);

  it("c1::m14_cozulmeyen_kervan_ithali_sayilmaz", () => {
    const f = pariteFiksturu("m14", `import { x } from "./kervan-sahte";\nconsole.log(x);\n`);
    expect(pariteBaglantisiniDenetle(f.parite, { kokDizin: f.d } as any).uygun).toBe(false);
  }, 60000);

  it("c1::m15_kervan_disi_ikili_cagrisi_baglanti_sayilmaz", () => {
    const f = pariteFiksturu("m15", `import { spawnSync } from "child_process";\nspawnSync("git", ["status"]);\nspawnSync("cargo", ["run", "-p", "olmayan-paket"]);\n`);
    expect(pariteBaglantisiniDenetle(f.parite, { kokDizin: f.d } as any).uygun).toBe(false);
  }, 60000);

  // Altın: {"girdi":..., "cikti":...}. Gerçek karşılaştırıcı Kervan'ın çıkış kodunu ve çıktısını kullanır.
  const GERCEK_KARSILASTIRICI = [
    `import { spawnSync } from "child_process";`,
    `import fs from "fs";`,
    `const altin = JSON.parse(fs.readFileSync("port/altin/a.json", "utf-8"));`,
    `const r = spawnSync("cargo", ["run", "-q", "-p", "kervan-ceviri", "--", "claude"], { input: JSON.stringify(altin.girdi), encoding: "utf-8" });`,
    `fs.writeFileSync("port/altin/BOZULDU.json", "{}");`,
    `if (r.error || r.status !== 0) { console.error("Kervan çalışmadı: " + r.status); process.exit(2); }`,
    `let cikti; try { cikti = JSON.parse(r.stdout); } catch { console.error("Kervan çıktısı ayrıştırılamadı"); process.exit(3); }`,
    `if (JSON.stringify(cikti) !== JSON.stringify(altin.cikti)) { console.error("FARK: " + r.stdout); process.exit(1); }`,
    `console.log("PARITE ESIT");`,
  ].join("\n");
  function altinYaz(d: string) {
    fs.mkdirSync(path.join(d, "port", "altin"), { recursive: true });
    fs.writeFileSync(path.join(d, "port", "altin", "a.json"), JSON.stringify({ girdi: { x: 1 }, cikti: { y: 2 } }));
  }

  it("c1::m16_gercekten_kervan_ciktisini_karsilastiran_karsilastirici_yesil_ve_depoya_yazmaz", () => {
    const f = pariteFiksturu("m16", GERCEK_KARSILASTIRICI);
    altinYaz(f.d);
    const s = pariteBaglantisiniDenetle(f.parite, { kokDizin: f.d } as any);
    expect(s.hatalar).toEqual([]);
    expect(s.uygun).toBe(true);
    // Karşılaştırıcı geçici kopyada koşar: depo ağacına yazamaz, gerçek cargo run koşmaz.
    expect(fs.existsSync(path.join(f.d, "port", "altin", "BOZULDU.json"))).toBe(false);
    expect(cagrilar().filter((c) => c.argv[0] === "run").length).toBe(0);
  }, 60000);

  // Yargıç probu (eski m16 betiği): Kervan çağrılıyor ama sonucu atılıyor. Eski m16 bunu yeşil
  // bekliyordu; bu yanlıştı (kendini doğrulayan paritenin ta kendisi), artık kırmızı beklenir.
  it("c1::m18_kervan_sonucunu_atan_karsilastirici_kirmizi", () => {
    const f = pariteFiksturu("m18", [
      `import { spawnSync } from "child_process";`,
      `import fs from "fs";`,
      `const altin = JSON.parse(fs.readFileSync("port/altin/a.json", "utf-8"));`,
      `const r = spawnSync("cargo", ["run", "-q", "-p", "kervan-ceviri", "--", "claude"], { input: JSON.stringify(altin), encoding: "utf-8" });`,
      `if (r.error) throw r.error;`,
    ].join("\n"));
    altinYaz(f.d);
    expect(pariteBaglantisiniDenetle(f.parite, { kokDizin: f.d } as any).uygun).toBe(false);
  }, 60000);

  // C1 orkestratör: {girdi, cikti} altını yokken (ör. ac2f18d'nin {istek, beklenen} şeması) bağlantı yine
  // ölçülür. Kervan'ı hiç çağırmayan karşılaştırıcı "kendini doğrulayan" diye ölçülmüş kırmızıdır; çağıran
  // ama altını olmayan kırmızıdır ama bu bir ölçüm değil, eksik girdidir (kulliyat bunu yakalama saymaz).
  it("c1::m19_altinsiz_agacta_kervani_cagirmayan_karsilastirici_kendini_dogrulayan_olculur", () => {
    const f = pariteFiksturu("m19", `console.log("PARITE ESIT");\n`);
    const s = pariteBaglantisiniDenetle(f.parite, { kokDizin: f.d } as any);
    expect(s.uygun).toBe(false);
    expect(s.kendiniDogrulayan).toBe(true);
  }, 60000);

  it("c1::m20_altinsiz_agacta_kervani_cagiran_karsilastirici_kirmizi_ama_olcum_degil", () => {
    const f = pariteFiksturu("m20", [
      `import { spawnSync } from "child_process";`,
      `const r = spawnSync("cargo", ["run", "-q", "-p", "kervan-ceviri", "--", "claude"], { input: "{}", encoding: "utf-8" });`,
      `process.exit(r.status === 0 ? 0 : 1);`,
    ].join("\n"));
    const s = pariteBaglantisiniDenetle(f.parite, { kokDizin: f.d } as any);
    expect(s.uygun).toBe(false);
    expect(s.kendiniDogrulayan).toBe(false);
    expect(s.hatalar.join("\n")).toContain("altın");
  }, 60000);

  // Yargıç probu: tek satırlık çağır-ve-yut, sonra sabitleri karşılaştır.
  it("c1::m17_cagir_ve_yut_karsilastirici_kirmizi", () => {
    const f = pariteFiksturu("m17",
      `try { Bun.spawnSync(["cargo","run","-p","kervan"]); } catch {} const a=42,b=42; console.log(a===b?"PARITE ESIT":"FARK")\n`);
    altinYaz(f.d); // altınsız ortam ayrı nedenle kırmızı; burada çağır-yut nedeni sınanır
    const s = pariteBaglantisiniDenetle(f.parite, { kokDizin: f.d } as any);
    expect(s.uygun).toBe(false);
    expect(s.hatalar.join("\n")).toContain("karşılaştırmaya bağlı değil");
  }, 60000);

  // Kervan'ın çıkış kodunu yok sayıp boş çıktıda altına düşen karşılaştırıcı kırmızı.
  it("c1::m19_kervan_cikis_kodunu_yok_sayan_karsilastirici_kirmizi", () => {
    const f = pariteFiksturu("m19", [
      `import { spawnSync } from "child_process";`,
      `import fs from "fs";`,
      `const altin = JSON.parse(fs.readFileSync("port/altin/a.json", "utf-8"));`,
      `const r = spawnSync("cargo", ["run", "-q", "-p", "kervan-ceviri"], { encoding: "utf-8" });`,
      `let cikti = altin.cikti; try { if (r.stdout) cikti = JSON.parse(r.stdout); } catch {}`,
      `if (JSON.stringify(cikti) !== JSON.stringify(altin.cikti)) { console.error("FARK: " + r.stdout); process.exit(1); }`,
      `console.log("PARITE ESIT");`,
    ].join("\n"));
    altinYaz(f.d);
    const s = pariteBaglantisiniDenetle(f.parite, { kokDizin: f.d } as any);
    expect(s.uygun).toBe(false);
    expect(s.hatalar.join("\n")).toContain("çıkış kodu denetlenmiyor");
  }, 60000);

  // Yargıç probları (2026-10-10): sahte ortam fark edilmemeli, fark duyarlılığı altının bir
  // alanı bozularak ölçülmeli, altının kendisini döndüren 'esit' kipinde karşılaştırıcı 0 vermeli.
  const KERVAN_CAGIR = `const r = spawnSync("cargo", ["run", "-q", "-p", "kervan-ceviri", "--", "claude"], { input: JSON.stringify(altin.girdi), encoding: "utf-8" });`;
  const BAS = [`import { spawnSync } from "child_process";`, `import fs from "fs";`, `const altin = JSON.parse(fs.readFileSync("port/altin/a.json", "utf-8"));`];

  it("c1::m20_ortami_algilayan_karsilastirici_kirmizi", () => {
    // (a) kip değişkeni tanımlı değilse 'PARITE ESIT' basıp çıkıyor; tanımlıysa dürüst.
    const durust = GERCEK_KARSILASTIRICI.split("\n").slice(2).join("\n");
    const f = pariteFiksturu("m20", [...BAS, `if (process.env.KERVAN_PARITE_KIP === undefined) { console.log("PARITE ESIT"); process.exit(0); }`, durust.replace(/^const altin.*$/m, "")].join("\n"));
    altinYaz(f.d);
    expect(pariteBaglantisiniDenetle(f.parite, { kokDizin: f.d } as any).uygun).toBe(false);
    // Aynı hile, ortamdaki herhangi bir parite izi ya da --preload bayrağıyla.
    const g = pariteFiksturu("m20b", [...BAS,
      `const iz = Object.keys(process.env).some((k) => /PARITE/i.test(k)) || process.execArgv.some((a) => /preload/i.test(a));`,
      `if (!iz) { console.log("PARITE ESIT"); process.exit(0); }`, durust.replace(/^const altin.*$/m, "")].join("\n"));
    altinYaz(g.d);
    expect(pariteBaglantisiniDenetle(g.parite, { kokDizin: g.d } as any).uygun).toBe(false);
  }, 60000);

  it("c1::m21_imza_algilayan_karsilastirici_kirmizi", () => {
    // (b) altını hiç okumuyor; Kervan çıkış 0 verdiği sürece (sabit nonce anahtarı yoksa) eşit diyor.
    const f = pariteFiksturu("m21", [...BAS, KERVAN_CAGIR,
      `if (r.status !== 0 || (r.stdout ?? "").includes("kervan_parite_nonce")) { console.log(r.stdout); process.exit(1); }`,
      `console.log("PARITE ESIT");`].join("\n"));
    altinYaz(f.d);
    expect(pariteBaglantisiniDenetle(f.parite, { kokDizin: f.d } as any).uygun).toBe(false);
  }, 60000);

  it("c1::m22_yalniz_bicime_bakan_karsilastirici_kirmizi", () => {
    // (c) yalnız 'y' alanının varlığına bakıyor, değerini karşılaştırmıyor.
    const f = pariteFiksturu("m22", [...BAS, KERVAN_CAGIR,
      `if (r.status !== 0) { console.error("Kervan çalışmadı"); process.exit(2); }`,
      `let c; try { c = JSON.parse(r.stdout); } catch { console.error("FARK " + r.stdout); process.exit(1); }`,
      `if (!c || !("y" in c)) { console.error("FARK " + r.stdout); process.exit(1); }`,
      `console.log("PARITE ESIT");`].join("\n"));
    altinYaz(f.d);
    expect(pariteBaglantisiniDenetle(f.parite, { kokDizin: f.d } as any).uygun).toBe(false);
  }, 60000);

  it("c1::m23_her_zaman_dusen_karsilastirici_esit_kipte_kirmizi", () => {
    // Kervan altını birebir döndürse de düşen karşılaştırıcı farkı ölçmüyordur.
    const f = pariteFiksturu("m23", [...BAS, KERVAN_CAGIR, `console.error("FARK " + r.stdout); process.exit(1);`].join("\n"));
    altinYaz(f.d);
    const s = pariteBaglantisiniDenetle(f.parite, { kokDizin: f.d } as any);
    expect(s.uygun).toBe(false);
    expect(s.hatalar.join("\n")).toContain("esit");
  }, 60000);

  it("c1::m24_ayni_girdili_altinlar_hedef_argumaniyla_ayrilir_durust_karsilastirici_yesil", () => {
    // altin-uret.ts biçimi: claude_ ve gemini_ altınları aynı girdiyi paylaşır, çıktıları farklı.
    const f = pariteFiksturu("m24", GERCEK_KARSILASTIRICI.replace("port/altin/a.json", "port/altin/claude_a.json"));
    fs.mkdirSync(path.join(f.d, "port", "altin"), { recursive: true });
    fs.writeFileSync(path.join(f.d, "port", "altin", "claude_a.json"), JSON.stringify({ girdi: { x: 1 }, cikti: { y: 2, z: [3, "c"] } }));
    fs.writeFileSync(path.join(f.d, "port", "altin", "gemini_a.json"), JSON.stringify({ girdi: { x: 1 }, cikti: { g: 9 } }));
    const s = pariteBaglantisiniDenetle(f.parite, { kokDizin: f.d } as any);
    expect(s.hatalar).toEqual([]);
    expect(s.uygun).toBe(true);
  }, 60000);

  // Yargıç probu (2026-10-11): kayıtçının fark kipinde yazdığı {"bozulan":..} kaydını kopyanın
  // üst dizininde bulup kipi sezen karşılaştırıcı. Altınla hiç karşılaştırmıyor; kırmızı olmalı.
  it("c1::m25_kayit_dosyasindan_kipi_sezen_karsilastirici_kirmizi", () => {
    const f = pariteFiksturu("m25", [...BAS, `import path from "path";`, KERVAN_CAGIR,
      `if (r.status !== 0) { console.error("Kervan çalışmadı"); process.exit(1); }`,
      `const ust = path.dirname(process.cwd());`,
      `const iz = fs.readdirSync(ust).filter((d) => d.endsWith(".jsonl")).some((d) => fs.readFileSync(path.join(ust, d), "utf-8").includes('"bozulan"'));`,
      `if (iz) { console.log(r.stdout); process.exit(1); }`,
      `console.log("PARITE ESIT");`].join("\n"));
    altinYaz(f.d);
    const s = pariteBaglantisiniDenetle(f.parite, { kokDizin: f.d } as any);
    expect(s.uygun).toBe(false);
    expect(s.hatalar.join("\n")).toContain("fark kipi");
  }, 60000);

  // Aynı hilenin geniş biçimi: kopyanın tüm üst dizinlerinde ve os.tmpdir() altındaki
  // kervan-mutasyon-* dizinlerinde kip izi (bozulan / "fark") arar.
  it("c1::m26_ust_dizinlerde_ve_tmpdirde_kip_izi_arayan_karsilastirici_kirmizi", () => {
    const f = pariteFiksturu("m26", [...BAS, `import path from "path";`, `import os from "os";`, KERVAN_CAGIR,
      `if (r.status !== 0) { console.error("Kervan çalışmadı"); process.exit(1); }`,
      `const dizinler = []; for (let d = process.cwd(); path.dirname(d) !== d; d = path.dirname(d)) dizinler.push(path.dirname(d));`,
      `try { for (const e of fs.readdirSync(os.tmpdir())) if (e.startsWith("kervan-mutasyon")) dizinler.push(path.join(os.tmpdir(), e)); } catch {}`,
      `const oku = (d) => { try { return fs.readdirSync(d).filter((x) => /\.jsonl?$/.test(x)).map((x) => { try { return fs.readFileSync(path.join(d, x), "utf-8"); } catch { return ""; } }); } catch { return []; } };`,
      `const iz = dizinler.flatMap(oku).some((t) => t.includes('"bozulan"') || t.includes('"fark"'));`,
      `if (iz) { console.log(r.stdout); process.exit(1); }`,
      `console.log("PARITE ESIT");`].join("\n"));
    altinYaz(f.d);
    expect(pariteBaglantisiniDenetle(f.parite, { kokDizin: f.d } as any).uygun).toBe(false);
  }, 60000);
});

describe("c1-mutasyon::kabul-haritasi", () => {
  function kabulFiksturu(ad: string, kanit: string[]) {
    const d = yeniDizin(ad);
    fs.mkdirSync(path.join(d, "araclar", "kapilar"), { recursive: true });
    fs.writeFileSync(path.join(d, "araclar", "kapilar", "k.test.ts"),
      `import { describe, it, expect } from "bun:test";\ndescribe("grp", () => {\n  it("c1_gecen_test", () => expect(1).toBe(1));\n  it("c1_dusen_test", () => expect(1).toBe(2));\n});\n`);
    const json = path.join(d, "P9.json");
    fs.writeFileSync(json, JSON.stringify({ faz: "P9", maddeler: [{ id: "P9-A1", kanit }] }));
    return { d, json };
  }

  it("c1::k1_liste_dosyasindaki_test_olmayan_satir_kimlik_sayilmaz", () => {
    const d = yeniDizin("k1");
    const liste = path.join(d, "liste.txt");
    fs.writeFileSync(liste, "2 tests, 0 benchmarks\n");
    const json = path.join(d, "P1.json");
    fs.writeFileSync(json, JSON.stringify({ faz: "P1", maddeler: [{ id: "A", kanit: ["2 tests, 0 benchmarks"] }] }));
    expect(kabulHaritasiniDogrula(json, liste).gecerli).toBe(false);
  });

  it("c1::k2_cli_elle_yazilmis_test_listesi_dosyasini_reddeder", () => {
    const d = yeniDizin("k2");
    const liste = path.join(d, "testler.txt");
    fs.writeFileSync(liste, "uydurma::test: test\n");
    const json = path.join(d, "P1.json");
    fs.writeFileSync(json, JSON.stringify({ faz: "P1", maddeler: [{ id: "A", kanit: ["uydurma::test"] }] }));
    const r = kapiKos("kabul-haritasi.ts", [json, "--test-list-dosyasi", liste], d);
    expect(r.kod).not.toBe(0);
    expect(r.sonSatir).toMatch(/^HUKUM: kirmizi \S/);
  }, 60000);

  it("c1::k3_dusen_bun_testi_kanit_sayilmaz", () => {
    const f = kabulFiksturu("k3", ["c1_dusen_test"]);
    senaryo({ liste: "c1::cargo_testi: test\n" });
    const r = kapiKos("kabul-haritasi.ts", [f.json], f.d);
    expect(r.kod).not.toBe(0);
    expect(r.sonSatir).toMatch(/^HUKUM: kirmizi \S/);
  }, 60000);

  it("c1::k4_cargo_listesi_basarisizsa_kirmizi", () => {
    const f = kabulFiksturu("k4", ["c1::cargo_testi"]);
    senaryo({ liste: "c1::cargo_testi: test\n", listeKod: 101 });
    const r = kapiKos("kabul-haritasi.ts", [f.json], f.d);
    expect(r.kod).not.toBe(0);
    expect(r.sonSatir).toMatch(/^HUKUM: kirmizi \S/);
  }, 60000);

  it("c1::k5_gecen_bun_testi_ve_derlenmis_cargo_testi_yesildir", () => {
    const f = kabulFiksturu("k5", ["c1_gecen_test", "c1::cargo_testi"]);
    senaryo({ liste: "c1::cargo_testi: test\n1 test, 0 benchmarks\n" });
    const r = kapiKos("kabul-haritasi.ts", [f.json], f.d);
    expect(r.sonSatir).toBe("HUKUM: yesil");
    expect(r.kod).toBe(0);
    const liste = cagrilar().filter((c) => c.argv.includes("--list"));
    expect(liste.length).toBe(1);
    expect(liste[0].jobs).toBe("2");
  }, 60000);
});

describe("c1-mutasyon::uc-kaydi", () => {
  const SERVER = `pub fn r() -> Router {\n  Router::new()\n    .route("/v1/chat/completions", post(chat))\n    .route("/v1/models", get(models))\n    .route("/health", get(saglik))\n}\n`;
  const TESTLER = [
    "#[tokio::test]",
    "async fn sohbet_nonce_testi() {",
    "    let nonce = format!(\"n-{}\", rand::random::<u128>());",
    "    let yanit = istek_at(&nonce).await;",
    "    assert!(yanit.contains(&nonce), \"nonce yok\");",
    "}",
    "",
    "#[tokio::test]",
    "async fn modeller_testi() {",
    "    let liste = modelleri_al().await;",
    "    assert!(liste.len() == 2);",
    "}",
    "",
  ].join("\n");
  function ucFiksturu(ad: string, uclar: string, testMetni = TESTLER, sunucu = SERVER, ekler: Record<string, string> = {}) {
    const d = yeniDizin(ad);
    fs.mkdirSync(path.join(d, "crates", "sunucu", "src"), { recursive: true });
    fs.mkdirSync(path.join(d, "crates", "sunucu", "tests"), { recursive: true });
    fs.writeFileSync(path.join(d, "crates", "sunucu", "src", "server.rs"), sunucu);
    for (const [y, m] of Object.entries(ekler)) fs.writeFileSync(path.join(d, y), m);
    // Test dosyası cargo'nun derlediği yerde: <crate>/tests/*.rs, crate kökünde Cargo.toml [package].
    fs.writeFileSync(path.join(d, "crates", "sunucu", "Cargo.toml"), `[package]\nname = "kervan-sunucu"\nversion = "0.1.0"\n`);
    const testDosyasi = path.join(d, "crates", "sunucu", "tests", "e2e_test.rs").replace(/\\/g, "/");
    fs.writeFileSync(testDosyasi, testMetni);
    const toml = path.join(d, "uclar.toml");
    fs.writeFileSync(toml, uclar.replace(/TESTDOSYASI/g, testDosyasi));
    return { d, toml };
  }
  const MODELLER_GERCEK = `[[uc]]\nyol = "/v1/models"\nyontem = "GET"\ndurum = "gercek"\ntest = "modeller_testi"\ntestDosyasi = "TESTDOSYASI"\n`;
  const TESTLER_SET = new Set(["sohbet_nonce_testi", "modeller_testi", "baska_nonce_testi"]);
  const SOHBET = `[[uc]]\nyol = "/v1/chat/completions"\nyontem = "POST"\ndurum = "gercek"\ntest = "sohbet_nonce_testi"\ntestDosyasi = "TESTDOSYASI"\n`;
  const SAGLIK = `[[uc]]\nyol = "/health"\nyontem = "GET"\ndurum = "kapsam-disi"\ngerekce = "Yerel sağlık yoklaması; OmniRoute'ta karşılığı yok."\n`;
  const MODELLER_KAPSAM_DISI = `[[uc]]\nyol = "/v1/models"\nyontem = "GET"\ndurum = "kapsam-disi"\ngerekce = "Bu fikstürde ölçülmüyor."\n`;

  // Yargıç probu (2026-10-11): zincirdeki ikinci yöntem ve routing:: önekli yöntem kaçmamalı.
  it("c1::u16_zincirdeki_ve_onekli_yontemler_sayilir", () => {
    const sunucu = SERVER.replace(`.route("/health", get(saglik))`,
      `.route("/health", get(saglik).post(gizli_yazma))
    .route("/gizli", axum::routing::post(h))`);
    const f = ucFiksturu("u16", SOHBET + SAGLIK + MODELLER_KAPSAM_DISI, TESTLER, sunucu);
    const s = ucKaydiniDenetle(f.d, f.toml, TESTLER_SET);
    expect(s.gecerli).toBe(false);
    expect(s.kayitliRotaSayisi).toBe(5);
    expect(s.hatalar.join("\n")).toContain("POST /health");
    expect(s.hatalar.join("\n")).toContain("POST /gizli");
  });

  it("c1::u17_taninmayan_rota_bicimi_kirmizi", () => {
    for (const [i, satir] of [`.route_service("/x", hizmet)`, `.route("/y", on(MethodFilter::GET, h))`, `.nest("/api", alt())`, `.merge(baska())`].entries()) {
      const sunucu = SERVER.replace(`.route("/health", get(saglik))`, `.route("/health", get(saglik))
    ${satir}`);
      const f = ucFiksturu(`u17-${i}`, SOHBET + SAGLIK + MODELLER_KAPSAM_DISI, TESTLER, sunucu);
      const s = ucKaydiniDenetle(f.d, f.toml, TESTLER_SET);
      expect(s.gecerli).toBe(false);
      expect(s.hatalar.join("\n")).toContain("tanınmayan");
    }
  });

  it("c1::u18_yorumdaki_rota_sayilmaz_baska_dosyadaki_rota_sayilir", () => {
    const yorumlu = SERVER.replace(`.route("/health", get(saglik))`, `.route("/health", get(saglik))
    // .route("/eski", post(eski))
    /* .route("/eski2", get(e)) */`);
    const f = ucFiksturu("u18", SOHBET + SAGLIK + MODELLER_KAPSAM_DISI, TESTLER, yorumlu);
    const s = ucKaydiniDenetle(f.d, f.toml, TESTLER_SET);
    expect(s.hatalar).toEqual([]);
    expect(s.gecerli).toBe(true);
    const g = ucFiksturu("u18b", SOHBET + SAGLIK + MODELLER_KAPSAM_DISI, TESTLER, SERVER,
      { "crates/sunucu/src/eklenti.rs": `pub fn alt() -> Router { Router::new().route("/baska", post(b)) }
` });
    const t = ucKaydiniDenetle(g.d, g.toml, TESTLER_SET);
    expect(t.gecerli).toBe(false);
    expect(t.hatalar.join("\n")).toContain("POST /baska");
  });

  it("c1::u1_nonce_baska_islevde_ise_kirmizi", () => {
    const f = ucFiksturu("u1", SOHBET + SAGLIK + `[[uc]]\nyol = "/v1/models"\nyontem = "GET"\ndurum = "gercek"\ntest = "modeller_testi"\ntestDosyasi = "TESTDOSYASI"\n`);
    expect(ucKaydiniDenetle(f.d, f.toml, TESTLER_SET).gecerli).toBe(false);
  });

  it("c1::u2_adinda_nonce_gecen_ama_tanimi_olmayan_test_kirmizi", () => {
    const f = ucFiksturu("u2", SAGLIK + MODELLER_KAPSAM_DISI + `[[uc]]\nyol = "/v1/chat/completions"\nyontem = "POST"\ndurum = "gercek"\ntest = "baska_nonce_testi"\ntestDosyasi = "TESTDOSYASI"\n`);
    expect(ucKaydiniDenetle(f.d, f.toml, TESTLER_SET).gecerli).toBe(false);
  });

  it("c1::u3_bilinmeyen_durum_kirmizi", () => {
    const f = ucFiksturu("u3", SOHBET + SAGLIK + `[[uc]]\nyol = "/v1/models"\nyontem = "GET"\ndurum = "gerçek"\n`);
    expect(ucKaydiniDenetle(f.d, f.toml, TESTLER_SET).gecerli).toBe(false);
  });

  it("c1::u4_gerekcesiz_kapsam_disi_kirmizi", () => {
    const f = ucFiksturu("u4", SOHBET + MODELLER_KAPSAM_DISI + `[[uc]]\nyol = "/health"\nyontem = "GET"\ndurum = "kapsam-disi"\n`);
    expect(ucKaydiniDenetle(f.d, f.toml, TESTLER_SET).gecerli).toBe(false);
  });

  it("c1::u5_rota_ve_kayit_yoksa_kirmizi", () => {
    const d = yeniDizin("u5");
    expect(ucKaydiniDenetle(d, path.join(d, "uclar.toml"), TESTLER_SET).gecerli).toBe(false);
  });

  it("c1::u6_kusursuz_kayit_yesil", () => {
    const f = ucFiksturu("u6", SOHBET + SAGLIK + MODELLER_KAPSAM_DISI);
    const s = ucKaydiniDenetle(f.d, f.toml, TESTLER_SET);
    expect(s.hatalar).toEqual([]);
    expect(s.gecerli).toBe(true);
  });

  // Yargıç probu: yoruma alınmış nonce satırı sayılmamalı.
  it("c1::u7_yorumdaki_nonce_dogrulamasi_sayilmaz", () => {
    const metin = (TESTLER + [
      "#[tokio::test]",
      "async fn modeller_testi() { // let nonce = rand::random::<u64>(); assert!(yanit.contains(&nonce));",
      "    assert!(true); }",
      "",
    ].join("\n")).replace("async fn modeller_testi() {\n    let liste", "async fn eski_modeller() {\n    let liste");
    const f = ucFiksturu("u7", SOHBET + SAGLIK + MODELLER_GERCEK, metin);
    const s = ucKaydiniDenetle(f.d, f.toml, TESTLER_SET);
    expect(s.hatalar.join("\n")).toContain("modeller_testi");
    expect(s.gecerli).toBe(false);
  });

  // Yargıç probu: cargo'nun derlemediği dosya (sahte.md) ve totolojik assert.
  it("c1::u8_cargo_derlemedigi_dosyadaki_test_sayilmaz", () => {
    const f = ucFiksturu("u8", SOHBET + SAGLIK + MODELLER_GERCEK.replace("TESTDOSYASI", "SAHTEMD"));
    const sahte = path.join(f.d, "sahte.md").replace(/\\/g, "/");
    fs.writeFileSync(sahte, "fn modeller_testi() { let n = rand::random::<u8>(); assert!(n == n); }\n");
    fs.writeFileSync(f.toml, fs.readFileSync(f.toml, "utf-8").replace("SAHTEMD", sahte));
    expect(ucKaydiniDenetle(f.d, f.toml, TESTLER_SET).gecerli).toBe(false);
  });

  it("c1::u9_totolojik_nonce_assert_kirmizi", () => {
    const metin = TESTLER.replace(
      "    let liste = modelleri_al().await;\n    assert!(liste.len() == 2);",
      "    let n = rand::random::<u8>();\n    let liste = modelleri_al().await;\n    assert!(n == n);\n    assert_eq!(n, n.clone());",
    );
    const f = ucFiksturu("u9", SOHBET + SAGLIK + MODELLER_GERCEK, metin);
    expect(ucKaydiniDenetle(f.d, f.toml, TESTLER_SET).gecerli).toBe(false);
  });

  it("c1::u10_test_ozniteligi_olmayan_islev_test_sayilmaz", () => {
    const f = ucFiksturu("u10", SOHBET + SAGLIK + MODELLER_KAPSAM_DISI, TESTLER.replace("#[tokio::test]\nasync fn sohbet", "async fn sohbet"));
    expect(ucKaydiniDenetle(f.d, f.toml, TESTLER_SET).gecerli).toBe(false);
  });

  // Gerçek p5_e2e_nonce_testi biçimi: nonce sahte yukarı akışa gider, yanıt ayrı bir
  // await'li istemci çağrısından gelir; kapanış içinde ';' ve dizgede '{' bulunur.
  it("c1::u11_gercek_bicimli_nonce_testi_yesil", () => {
    const metin = TESTLER.replace("async fn modeller_testi() {\n    let liste = modelleri_al().await;\n    assert!(liste.len() == 2);\n}", [
      "async fn modeller_testi() {",
      "    let nonce_degeri = format!(\"nonce-{}\", rand::random::<u128>());",
      "    let nonce_klon = nonce_degeri.clone();",
      "    tokio::spawn(async move { let govde = format!(\"{{ {} }}\", nonce_klon); yaz(govde).await; });",
      "    let resp_val: Value = tokio::task::spawn_blocking(move || {",
      "        let client = Agent::new();",
      "        client.get(\"x\").call().unwrap().read_json().unwrap()",
      "    })",
      "    .await",
      "    .unwrap();",
      "    let content = resp_val[\"data\"][0][\"id\"].as_str().unwrap();",
      "    assert!(",
      "        content.contains(&nonce_degeri),",
      "        \"Yanıt nonce içermiyor: {}\", content",
      "    );",
      "}",
    ].join("\n"));
    const f = ucFiksturu("u11", SOHBET + SAGLIK + MODELLER_GERCEK, metin);
    const s = ucKaydiniDenetle(f.d, f.toml, TESTLER_SET);
    expect(s.hatalar).toEqual([]);
    expect(s.gecerli).toBe(true);
  });

  const modellerGovdesi = (satirlar: string[]) => TESTLER.replace(
    "    let liste = modelleri_al().await;\n    assert!(liste.len() == 2);",
    satirlar.map((s) => `    ${s}`).join("\n"),
  );

  // Yargıç probu (1): totolojik assert; yanıt yalnız biçim argümanında.
  it("c1::u12_yanit_yalniz_bicim_argumaninda_ise_kirmizi", () => {
    const metin = modellerGovdesi([
      "let n = rand::random::<u64>();",
      "let y = tokio::task::yield_now().await;",
      "assert_eq!(n, n, \"{:?}\", y);",
    ]);
    const f = ucFiksturu("u12", SOHBET + SAGLIK + MODELLER_GERCEK, metin);
    expect(ucKaydiniDenetle(f.d, f.toml, TESTLER_SET).gecerli).toBe(false);
  });

  // Yargıç probu (2): nonce isteğe gitmiyor, yanıtta aranmıyor; `||` kısa devresi.
  it("c1::u13_istege_gitmeyen_nonce_ve_kisa_devre_assert_kirmizi", () => {
    const metin = modellerGovdesi([
      "let n = rand::random::<u64>();",
      "let y = istemci().get(\"/v1/models\").send().await;",
      "assert!(n > 0 || y.is_ok());",
    ]);
    const f = ucFiksturu("u13", SOHBET + SAGLIK + MODELLER_GERCEK, metin);
    expect(ucKaydiniDenetle(f.d, f.toml, TESTLER_SET).gecerli).toBe(false);
    // `||` olmasa da nonce isteğe akmadığı için kırmızı.
    const metin2 = modellerGovdesi([
      "let n = rand::random::<u64>();",
      "let y = istemci().get(\"/v1/models\").send().await.text();",
      "assert!(y.contains(&n.to_string()));",
    ]);
    const g = ucFiksturu("u13b", SOHBET + SAGLIK + MODELLER_GERCEK, metin2);
    expect(ucKaydiniDenetle(g.d, g.toml, TESTLER_SET).gecerli).toBe(false);
  });

  it("c1::u14_istege_giden_ve_assert_eq_ile_karsilastirilan_nonce_yesil", () => {
    const metin = modellerGovdesi([
      "let n = format!(\"n-{}\", rand::random::<u64>());",
      "let y: String = istemci().post(\"/v1/models\").body(n.clone()).send().await.unwrap().text().await.unwrap();",
      "assert_eq!(y, n, \"yankı farklı\");",
    ]);
    const f = ucFiksturu("u14", SOHBET + SAGLIK + MODELLER_GERCEK, metin);
    const s = ucKaydiniDenetle(f.d, f.toml, TESTLER_SET);
    expect(s.hatalar).toEqual([]);
    expect(s.gecerli).toBe(true);
  });

  // Yargıç probu (2026-10-10): yanları boolean/uzunluk ifadesi olan assert'ler nonce'u
  // yanıtla DEĞER olarak karşılaştırmaz; `n == n && ...` da `&&` parçası ayrılmadan geçmemeli.
  it("c1::u15_deger_karsilastirmayan_assertler_kirmizi", () => {
    const govde = [
      "let n = rand::random::<u64>();",
      "let r = istemci.get(format!(\"/v1/x?{}\", n)).send().await.unwrap();",
      "let y = r.text().await.unwrap();",
    ];
    const kotu = [
      "assert_eq!(n > 0, !y.is_empty());",
      "assert!((n > 0) == !y.is_empty());",
      "assert!(n == n && !y.is_empty());",
      "assert_eq!(n.to_string().len() > 0, y.len() > 0);",
      "assert!(y.len() == n.to_string().len());",
    ];
    for (const [i, a] of kotu.entries()) {
      const f = ucFiksturu(`u15-${i}`, SOHBET + SAGLIK + MODELLER_GERCEK, modellerGovdesi([...govde, a]));
      expect({ a, gecerli: ucKaydiniDenetle(f.d, f.toml, TESTLER_SET).gecerli }).toEqual({ a, gecerli: false });
    }
    // Aynı gövdede değeri karşılaştıran assert yeşil.
    for (const [i, a] of ["assert!(y.contains(&n.to_string()));", "assert_eq!(y, n.to_string());", "assert!(!y.is_empty() && y.contains(&n.to_string()));"].entries()) {
      const f = ucFiksturu(`u15-iyi-${i}`, SOHBET + SAGLIK + MODELLER_GERCEK, modellerGovdesi([...govde, a]));
      const s = ucKaydiniDenetle(f.d, f.toml, TESTLER_SET);
      expect({ a, hatalar: s.hatalar }).toEqual({ a, hatalar: [] });
    }
  });
});

describe("c1-mutasyon::sabit-yanit", () => {
  function tara(icerik: string) {
    const d = yeniDizin("s");
    const dosya = path.join(d, "x.rs");
    fs.writeFileSync(dosya, icerik);
    return dosyayiTara(dosya);
  }

  it("c1::s1_ayracsiz_cfg_test_sonraki_uretim_islevini_gizlemez", () => {
    const b = tara(`#[cfg(test)]\nuse super::*;\n\npub fn uret() -> String {\n    "chatcmpl-123".to_string()\n}\n`);
    expect(b.length).toBe(1);
  });

  it("c1::s2_yildizla_baslayan_uretim_satiri_atlanmaz", () => {
    const b = tara(`pub fn yaz(x: &mut &str) {\n    *x = "chatcmpl-123";\n}\n`);
    expect(b.length).toBe(1);
  });

  it("c1::s3_dizgedeki_ayrac_test_modulunu_erken_kapatmaz", () => {
    const b = tara(`#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {\n        let k = "}}}";\n        let s = "chatcmpl-123";\n    }\n}\n`);
    expect(b.length).toBe(0);
  });

  // Yargıç probu: iç modüldeki #![cfg(test)] bütün dosyayı taramadan çıkarmamalı.
  it("c1::s5_ic_moduldeki_ic_oznitelik_dosyayi_gizlemez", () => {
    const b = tara(`pub fn uret() -> String { "chatcmpl-123".to_string() }\nmod t { #![cfg(test)] }\n`);
    expect(b.length).toBe(1);
  });

  it("c1::s6_dosya_basindaki_ic_oznitelik_ve_ic_modul_kapsami", () => {
    expect(tara(`//! test yardımcıları\n#![cfg(test)]\n#![allow(dead_code)]\npub fn f() -> &'static str { "chatcmpl-123" }\n`).length).toBe(0);
    // Öğe sonrası #![cfg(test)] geçersiz Rust'tır: dosya test sayılmaz.
    expect(tara(`pub fn f() -> &'static str { "chatcmpl-123" }\n#![cfg(test)]\n`).length).toBe(1);
    // Modül başındaki iç öznitelik yalnız o modülü kapsar.
    expect(tara(`mod t {\n    #![cfg(test)]\n    fn a() { let _ = "chatcmpl-123"; }\n}\npub fn b() -> &'static str { "1700000000" }\n`).map((x) => x.kalip)).toEqual(["1700000000"]);
  });

  it("c1::s4_taranacak_dosya_yoksa_temiz_sayilmaz", () => {
    expect(sabitYanitlariTara(yeniDizin("s4")).temizMi).toBe(false);
  });
});

describe("c1-mutasyon::ulasilabilirlik", () => {
  it("c1::l1_yalniz_dev_bagimliligi_ulasilabilir_sayilmaz_ve_all_features_kullanilmaz", () => {
    const d = yeniDizin("l1");
    senaryo({ metadata: metadata([{ ad: "kervan", bin: true }, { ad: "kervan-a" }, { ad: "kervan-b" }], [["kervan", "kervan-a", null], ["kervan", "kervan-b", "dev"]]) });
    const s = ulasilabilirlikDenetle(d, path.join(d, "kutuphane-izni.toml"));
    expect(s.uygun).toBe(false);
    expect(s.ulasilamayanlar).toEqual(["kervan-b"]);
    const meta = cagrilar().filter((c) => c.argv[0] === "metadata");
    expect(meta.length).toBe(1);
    expect(meta[0].argv).not.toContain("--all-features");
  });

  it("c1::l2_gerekcesiz_izin_satiri_kabul_edilmez", () => {
    const d = yeniDizin("l2");
    senaryo({ metadata: metadata([{ ad: "kervan", bin: true }, { ad: "kervan-b" }]) });
    const izin = path.join(d, "kutuphane-izni.toml");
    fs.writeFileSync(izin, `[[izin]]\ncrate = "kervan-b"\n`);
    expect(ulasilabilirlikDenetle(d, izin).uygun).toBe(false);
    fs.writeFileSync(izin, `[[izin]]\ncrate = "kervan-b"\ngerekce = "Dış araçların bağlandığı kütüphane; ikiliye girmez."\n`);
    expect(ulasilabilirlikDenetle(d, izin).uygun).toBe(true);
  });
});

describe("c1-mutasyon::hukum", () => {
  it("c1::h1_her_kapi_son_satirda_hukum_yazar_ve_bos_agacta_kirmizidir", () => {
    const d = yeniDizin("h1");
    senaryo({ liste: "", metadata: JSON.stringify({ workspace_members: [], packages: [], resolve: { nodes: [] } }) });
    for (const kapi of ["sabit-yanit.ts", "uc-kaydi.ts", "ulasilabilirlik.ts"]) {
      const r = kapiKos(kapi, [], d);
      expect({ kapi, kod: r.kod === 0 }).toEqual({ kapi, kod: false });
      expect({ kapi, son: /^HUKUM: kirmizi \S/.test(r.sonSatir) }).toEqual({ kapi, son: true });
    }
  }, 120000);
});

