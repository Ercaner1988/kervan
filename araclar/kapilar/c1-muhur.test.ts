import { describe, expect, it, afterAll } from "bun:test";
import { spawnSync } from "child_process";
import fs from "fs";
import os from "os";
import path from "path";

// Küme 'muhur': muhur-kontrol / muhur-uret kandırma yolları.
// Her senaryo os.tmpdir() altında adında 'c1-muhur' geçen geçici bir git deposu kurar;
// kapılar CLI olarak (cwd = geçici depo) koşulur, depo ağacına yazılmaz.

const KONTROL = path.resolve(import.meta.dir, "muhur-kontrol.ts");
const URET = path.resolve(import.meta.dir, "muhur-uret.ts");
const KOK_TMP = fs.mkdtempSync(path.join(os.tmpdir(), "c1-muhur-"));
afterAll(() => fs.rmSync(KOK_TMP, { recursive: true, force: true }));

const GIT_ENV = {
  ...process.env,
  GIT_AUTHOR_NAME: "c1-muhur",
  GIT_AUTHOR_EMAIL: "c1-muhur@ornek.invalid",
  GIT_COMMITTER_NAME: "c1-muhur",
  GIT_COMMITTER_EMAIL: "c1-muhur@ornek.invalid",
};

function git(dir: string, ...args: string[]): string {
  const r = spawnSync("git", ["-c", "core.autocrlf=false", "-c", "commit.gpgsign=false", ...args], {
    cwd: dir,
    encoding: "utf-8",
    env: GIT_ENV,
  });
  if (r.status !== 0) throw new Error(`git ${args.join(" ")}: ${r.stderr}`);
  return r.stdout.trim();
}

function yaz(dir: string, yol: string, icerik: string) {
  const tam = path.join(dir, yol);
  fs.mkdirSync(path.dirname(tam), { recursive: true });
  fs.writeFileSync(tam, icerik, "utf-8");
}

const MUTANTLAR = `# yalniz-ekleme
[[mutant]]
ad = "m1"
dosya = "a.rs"
bul = "x"
yerine = "y"
oldurmesi_gereken_test = ["t1"]

[[mutant]]
ad = "m2"
dosya = "b.rs"
bul = "p"
yerine = "q"
oldurmesi_gereken_test = ["t2"]
`;

// Güven kökü: Ercan'ın genel anahtarı depo DIŞINDA, ev dizinindeki imzacılar dosyasında durur.
// Testte ev dizini geçici bir 'c1-muhur' dizinine çevrilir (HOME/USERPROFILE).
const EV = path.join(KOK_TMP, "ev");
const IMZACILAR = path.join(EV, ".config", "kervan", "muhur-imzacilar");
const KOS_ENV = { ...GIT_ENV, HOME: EV, USERPROFILE: EV };

function anahtarUret(ad: string): string {
  const k = path.join(KOK_TMP, `anahtar-${ad}`);
  if (!fs.existsSync(k)) {
    const r = spawnSync("ssh-keygen", ["-q", "-t", "ed25519", "-N", "", "-C", ad, "-f", k], { encoding: "utf-8" });
    if (r.status !== 0) throw new Error(`ssh-keygen: ${r.stderr}`);
  }
  return k;
}
const ERCAN_ANAHTARI = anahtarUret("ercan");
const SAHTE_ANAHTAR = anahtarUret("builder");
const imzaciSatiri = (ad: string, k: string) => `${ad} namespaces="git" ${fs.readFileSync(k + ".pub", "utf-8").trim()}
`;
fs.mkdirSync(path.dirname(IMZACILAR), { recursive: true });
fs.writeFileSync(IMZACILAR, imzaciSatiri("ercan", ERCAN_ANAHTARI));

/** muhur-onay etiketini verilen anahtarla imzalar (anahtar yoksa imzasız açıklamalı etiket). */
function etiketle(dir: string, anahtar: string | null, hedef = "HEAD") {
  const imza = anahtar ? ["-c", "gpg.format=ssh", "-c", `user.signingkey=${anahtar}`, "tag", "-f", "-s"] : ["tag", "-f", "-a"];
  git(dir, ...imza, "-m", "muhur onayi", "muhur-onay", hedef);
}

function kos(dir: string, betik = KONTROL, ...args: string[]) {
  const r = spawnSync("bun", [betik, ...args], { cwd: dir, encoding: "utf-8", timeout: 60000, env: KOS_ENV });
  const satirlar = `${r.stdout ?? ""}${r.stderr ?? ""}`.trim().split(/\r?\n/);
  return { kod: r.status, son: satirlar[satirlar.length - 1] ?? "", cikti: `${r.stdout}${r.stderr}` };
}

function onayla(dir: string) {
  const yol = path.join(dir, "port", "muhur.toml");
  const temiz = fs
    .readFileSync(yol, "utf-8")
    .split("\n")
    .filter((s) => !/^\s*onay(_bekliyor)?\s*=/.test(s))
    .join("\n");
  fs.writeFileSync(yol, `onay_bekliyor = false\nonay = "Ercan (c1-muhur deneme)"\n${temiz}`, "utf-8");
}

let sayac = 0;
/** Mühürlü, onaylı, commit'lenmiş temiz bir fixture depo kurar. */
function depoKur(): string {
  const dir = path.join(KOK_TMP, `depo-${++sayac}`);
  fs.mkdirSync(dir, { recursive: true });
  git(dir, "init", "-q");
  yaz(dir, ".gitignore", "/ref\n");
  yaz(dir, "araclar/kapilar/a.ts", "export const a = 1;\n");
  yaz(dir, "araclar/kapilar/alt/b.ts", "export const b = 2;\n");
  yaz(dir, "araclar/parite/altin-uret.ts", "// altin uretici\n");
  yaz(dir, "araclar/parite/strateji-altin-uret.ts", "// strateji altin uretici\n");
  yaz(dir, "port/altin/x.json", JSON.stringify({ uretici_ozeti: "abc", cikti: 1 }) + "\n");
  yaz(dir, "port/mutantlar.toml", MUTANTLAR);
  yaz(dir, "ref/omni/f.txt", "ref icerigi\n");
  yaz(dir, "src/serbest.rs", "fn main() {}\n");
  git(dir, "add", "-A");
  git(dir, "commit", "-q", "-m", "ilk");
  const u = kos(dir, URET);
  if (u.kod !== 0) throw new Error(`muhur-uret basarisiz: ${u.cikti}`);
  onayla(dir);
  git(dir, "add", "-A");
  git(dir, "commit", "-q", "-m", "muhur");
  etiketle(dir, ERCAN_ANAHTARI);
  return dir;
}

function kirmizi(dir: string, nedenParcasi?: RegExp) {
  const r = kos(dir);
  expect(r.kod).not.toBe(0);
  expect(r.son).toMatch(/^HUKUM: kirmizi \S/);
  if (nedenParcasi) expect(r.cikti).toMatch(nedenParcasi);
}

function yesil(dir: string) {
  const r = kos(dir);
  if (r.kod !== 0) console.error(r.cikti);
  expect(r.kod).toBe(0);
  expect(r.son).toBe("HUKUM: yesil");
}

describe("c1-muhur", () => {
  it("c1_muhur::temiz_muhurlu_depo_yesildir", () => {
    yesil(depoKur());
  }, 60000);

  it("c1_muhur::uretici_onay_cumlesi_yazmaz_onay_bekliyor_yazar", () => {
    const dir = path.join(KOK_TMP, "uretici");
    fs.mkdirSync(dir, { recursive: true });
    git(dir, "init", "-q");
    yaz(dir, "araclar/kapilar/a.ts", "1\n");
    yaz(dir, "araclar/parite/altin-uret.ts", "1\n");
    yaz(dir, "araclar/parite/strateji-altin-uret.ts", "1\n");
    yaz(dir, "ref/omni/f.txt", "r\n");
    const u = kos(dir, URET);
    expect(u.kod).toBe(0);
    const m = fs.readFileSync(path.join(dir, "port", "muhur.toml"), "utf-8");
    expect(m).not.toContain("Ercan onayı ile mühürlenmiştir");
    const t = Bun.TOML.parse(m) as any;
    expect(t.onay_bekliyor).toBe(true);
    const yollar = t.dosya.map((d: any) => d.yol);
    expect(yollar).toContain("araclar/parite/altin-uret.ts");
    expect(yollar).toContain("araclar/parite/strateji-altin-uret.ts");
    expect(typeof t.ref?.ozet).toBe("string");
  }, 60000);

  it("c1_muhur::onaysiz_muhur_kirmizidir", () => {
    const dir = depoKur();
    const yol = path.join(dir, "port", "muhur.toml");
    fs.writeFileSync(yol, fs.readFileSync(yol, "utf-8").replace("onay_bekliyor = false", "onay_bekliyor = true"));
    git(dir, "commit", "-qam", "onay geri");
    kirmizi(dir, /onay/i);
  }, 60000);

  it("c1_muhur::muhurlu_dizine_listede_olmayan_dosya_kirmizidir", () => {
    const dir = depoKur();
    yaz(dir, "araclar/kapilar/yeni-kapi.ts", "export const sahte = true;\n");
    kirmizi(dir, /yeni-kapi\.ts/);
  }, 60000);

  it("c1_muhur::altin_dizinine_listede_olmayan_dosya_kirmizidir", () => {
    const dir = depoKur();
    yaz(dir, "port/altin/eklenen.json", "{}\n");
    kirmizi(dir, /eklenen\.json/);
  }, 60000);

  it("c1_muhur::bos_muhur_toml_kirmizidir", () => {
    const dir = depoKur();
    fs.writeFileSync(path.join(dir, "port", "muhur.toml"), "# bos\n");
    git(dir, "commit", "-qam", "muhuru bosalt");
    kirmizi(dir);
  }, 60000);

  it("c1_muhur::ayristirilamayan_muhur_toml_kirmizidir", () => {
    const dir = depoKur();
    fs.writeFileSync(path.join(dir, "port", "muhur.toml"), "[[dosya]]\nyol = = \"x\"\n");
    git(dir, "commit", "-qam", "bozuk muhur");
    kirmizi(dir);
  }, 60000);

  it("c1_muhur::eksik_muhur_toml_kirmizidir_faz_ne_olursa_olsun", () => {
    const dir = depoKur();
    fs.rmSync(path.join(dir, "port", "muhur.toml"));
    const r = kos(dir, KONTROL, "--faz", "P2");
    expect(r.kod).not.toBe(0);
    expect(r.son).toMatch(/^HUKUM: kirmizi \S/);
  }, 60000);

  it("c1_muhur::mutant_silen_commit_kirmizidir", () => {
    const dir = depoKur();
    fs.writeFileSync(path.join(dir, "port", "mutantlar.toml"), MUTANTLAR.split("\n[[mutant]]\nad = \"m2\"")[0] + "\n");
    git(dir, "commit", "-qam", "mutant sil");
    kirmizi(dir, /mutant/i);
  }, 60000);

  it("c1_muhur::mutant_degistiren_commitsiz_degisiklik_kirmizidir", () => {
    const dir = depoKur();
    fs.writeFileSync(path.join(dir, "port", "mutantlar.toml"), MUTANTLAR.replace('yerine = "y"', 'yerine = "x"'));
    kirmizi(dir, /mutant/i);
  }, 60000);

  it("c1_muhur::mutantlar_dosyasi_silinince_kirmizidir", () => {
    const dir = depoKur();
    fs.rmSync(path.join(dir, "port", "mutantlar.toml"));
    git(dir, "commit", "-qam", "mutantlari sil");
    kirmizi(dir, /mutantlar\.toml/);
  }, 60000);

  // Yargıç saldırısı: dosyayı silmeden tüm mutantları düşür (tablo hiç yok -> mutant alanı undefined).
  it("c1_muhur::mutantlar_bosaltilinca_kirmizidir", () => {
    const dir = depoKur();
    fs.writeFileSync(path.join(dir, "port", "mutantlar.toml"), "# bos\n");
    git(dir, "commit", "-qam", "mutantlari bosalt");
    kirmizi(dir, /mutant/i);
  }, 60000);

  it("c1_muhur::mutant_tablosu_yeniden_adlandirilinca_kirmizidir", () => {
    const dir = depoKur();
    fs.writeFileSync(path.join(dir, "port", "mutantlar.toml"), MUTANTLAR.replaceAll("[[mutant]]", "[[mutantx]]"));
    git(dir, "commit", "-qam", "mutant tablosunu gizle");
    kirmizi(dir, /mutant/i);
  }, 60000);

  it("c1_muhur::mutant_alani_tablo_dizisi_degilse_kirmizidir", () => {
    const dir = depoKur();
    fs.writeFileSync(path.join(dir, "port", "mutantlar.toml"), 'mutant = "yok"\n');
    git(dir, "commit", "-qam", "mutant alanini dizgeye cevir");
    kirmizi(dir, /mutant/i);
  }, 60000);

  it("c1_muhur::mutant_ekleme_yesildir", () => {
    const dir = depoKur();
    fs.appendFileSync(
      path.join(dir, "port", "mutantlar.toml"),
      '\n[[mutant]]\nad = "m3"\ndosya = "c.rs"\nbul = "a"\nyerine = "b"\noldurmesi_gereken_test = ["t3"]\n',
    );
    git(dir, "commit", "-qam", "mutant ekle");
    yesil(dir);
  }, 60000);

  it("c1_muhur::muhurlu_yolu_degistirip_geri_alan_commitler_kirmizidir", () => {
    const dir = depoKur();
    const yol = path.join(dir, "araclar", "kapilar", "a.ts");
    const asil = fs.readFileSync(yol, "utf-8");
    fs.writeFileSync(yol, "export const a = 999; // kapi gevsetildi\n");
    git(dir, "commit", "-qam", "kapiyi gevset");
    fs.writeFileSync(yol, asil);
    git(dir, "commit", "-qam", "izleri sil");
    kirmizi(dir, /a\.ts/);
  }, 60000);

  it("c1_muhur::muhursuz_yola_dokunan_commit_yesildir", () => {
    const dir = depoKur();
    yaz(dir, "src/serbest.rs", "fn main() { println!(\"x\"); }\n");
    git(dir, "commit", "-qam", "serbest is");
    yesil(dir);
  }, 60000);

  it("c1_muhur::kapiyi_degistirip_muhuru_commitsiz_yeniden_ureten_kirmizidir", () => {
    const dir = depoKur();
    yaz(dir, "araclar/kapilar/a.ts", "export const a = 0; // sahte\n");
    expect(kos(dir, URET).kod).toBe(0);
    onayla(dir);
    kirmizi(dir, /muhur\.toml/);
  }, 60000);

  /** Yargıç saldırısı: kapıyı gevşet, mutant sil, mühürü yeniden üret ve onayı kendin doldur; hepsi TEK commit'te. */
  function kendiniMuhurle(dir: string, ekBaslik = "") {
    yaz(dir, "araclar/kapilar/a.ts", "export const a = 0; // gevsetildi\n");
    fs.writeFileSync(path.join(dir, "port", "mutantlar.toml"), "# yalniz-ekleme\n[[mutant]]" + MUTANTLAR.split("[[mutant]]")[2]);
    expect(kos(dir, URET).kod).toBe(0);
    onayla(dir);
    const yol = path.join(dir, "port", "muhur.toml");
    fs.writeFileSync(yol, ekBaslik + fs.readFileSync(yol, "utf-8"));
    git(dir, "add", "-A");
    git(dir, "commit", "-q", "-m", "kapi + muhur + onay tek commit");
  }

  it("c1_muhur::kapiyi_degistirip_muhuru_ayni_committe_yeniden_uretip_onaylayan_kirmizidir", () => {
    const dir = depoKur();
    kendiniMuhurle(dir);
    kirmizi(dir, /a\.ts|muhur\.toml/);
  }, 60000);

  it("c1_muhur::muhur_commit_HEAD_yazarak_kendini_muhurleyen_kirmizidir", () => {
    const dir = depoKur();
    kendiniMuhurle(dir, 'muhur_commit = "HEAD"\n');
    kirmizi(dir);
  }, 60000);

  it("c1_muhur::builder_kendi_anahtariyla_etiketi_yeniden_imzalarsa_kirmizidir", () => {
    const dir = depoKur();
    kendiniMuhurle(dir);
    etiketle(dir, SAHTE_ANAHTAR);
    kirmizi(dir, /imza/i);
  }, 60000);

  it("c1_muhur::repo_configte_kendi_imzaci_dosyasini_gosteren_kirmizidir", () => {
    const dir = depoKur();
    kendiniMuhurle(dir);
    etiketle(dir, SAHTE_ANAHTAR);
    const sahte = path.join(KOK_TMP, `sahte-imzacilar-${sayac}`);
    fs.writeFileSync(sahte, imzaciSatiri("ercan", SAHTE_ANAHTAR));
    git(dir, "config", "gpg.ssh.allowedSignersFile", sahte);
    kirmizi(dir, /imza/i);
  }, 60000);

  // Yargıç saldırısı (karışık vekil): Ercan builder'ın commit'ine başka amaçla imzalı etiket atar
  // (ör. faz onayı 'p5-onay'); builder o tag nesnesini refs/tags/muhur-onay'a bağlar.
  it("c1_muhur::baska_adli_imzali_etiketi_muhur_onayina_baglayan_kirmizidir", () => {
    const dir = depoKur();
    kendiniMuhurle(dir);
    git(dir, "-c", "gpg.format=ssh", "-c", `user.signingkey=${ERCAN_ANAHTARI}`, "tag", "-s", "-m", "P5 faz onayi", "p5-onay", "HEAD");
    git(dir, "update-ref", "refs/tags/muhur-onay", git(dir, "rev-parse", "refs/tags/p5-onay"));
    kirmizi(dir, /tag muhur-onay/);
  }, 60000);

  // Etiket bir commit'i değil, commit'i gösteren başka bir etiketi (iç içe) gösteriyorsa: 'type commit' değil.
  it("c1_muhur::etiketi_gosteren_imzali_etiket_kirmizidir", () => {
    const dir = depoKur();
    kendiniMuhurle(dir);
    git(dir, "tag", "-a", "-m", "ic", "ic-etiket", "HEAD");
    git(dir, "-c", "gpg.format=ssh", "-c", `user.signingkey=${ERCAN_ANAHTARI}`, "tag", "-f", "-s", "-m", "x", "muhur-onay", "ic-etiket");
    kirmizi(dir, /type commit/);
  }, 60000);

  it("c1_muhur::imzasiz_onay_etiketi_kirmizidir", () => {
    const dir = depoKur();
    kendiniMuhurle(dir);
    etiketle(dir, null);
    kirmizi(dir, /imza/i);
  }, 60000);

  it("c1_muhur::onay_etiketi_yoksa_kirmizidir", () => {
    const dir = depoKur();
    git(dir, "tag", "-d", "muhur-onay");
    kirmizi(dir, /muhur-onay/);
  }, 60000);

  it("c1_muhur::imzacilar_dosyasi_yoksa_kirmizidir", () => {
    const dir = depoKur();
    const yedek = fs.readFileSync(IMZACILAR, "utf-8");
    fs.rmSync(IMZACILAR);
    try {
      kirmizi(dir, /imzac/i);
    } finally {
      fs.writeFileSync(IMZACILAR, yedek);
    }
  }, 60000);

  it("c1_muhur::muhur_tomlu_etiketten_sonra_degistiren_commit_kirmizidir", () => {
    const dir = depoKur();
    const yol = path.join(dir, "port", "muhur.toml");
    fs.writeFileSync(yol, "# yorum eklendi\n" + fs.readFileSync(yol, "utf-8"));
    git(dir, "commit", "-qam", "muhur.toml oynandi");
    kirmizi(dir, /muhur\.toml/);
  }, 60000);

  it("c1_muhur::elle_duzenlenmis_altin_kirmizidir", () => {
    const dir = depoKur();
    yaz(dir, "port/altin/x.json", JSON.stringify({ uretici_ozeti: "abc", cikti: 2 }) + "\n");
    kirmizi(dir, /x\.json/);
  }, 60000);

  it("c1_muhur::ref_yoksa_kirmizidir", () => {
    const dir = depoKur();
    fs.rmSync(path.join(dir, "ref"), { recursive: true, force: true });
    kirmizi(dir, /ref/);
  }, 60000);

  it("c1_muhur::ref_icerigi_degisince_kirmizidir", () => {
    const dir = depoKur();
    yaz(dir, "ref/omni/f.txt", "ref icerigi degisti\n");
    kirmizi(dir, /ref/);
  }, 60000);

  it("c1_muhur::cozulemeyen_muhur_commit_kirmizidir", () => {
    const dir = depoKur();
    const yol = path.join(dir, "port", "muhur.toml");
    fs.writeFileSync(yol, `muhur_commit = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef"\n${fs.readFileSync(yol, "utf-8")}`);
    git(dir, "commit", "-qam", "sahte muhur commit");
    kirmizi(dir, /muhur_commit/);
  }, 60000);

  it("c1_muhur::git_deposu_olmayan_dizinde_kirmizidir", () => {
    const dir = depoKur();
    fs.rmSync(path.join(dir, ".git"), { recursive: true, force: true });
    kirmizi(dir);
  }, 60000);

  it("c1_muhur::bilinmeyen_argumanla_kirmizidir", () => {
    const dir = depoKur();
    const r = kos(dir, KONTROL, "--ref", "HEAD");
    expect(r.kod).not.toBe(0);
    expect(r.son).toMatch(/^HUKUM: kirmizi \S/);
  }, 60000);

  // Yargıç saldırısı: imzalı etiketin gösterdiği M commit'ini 'git replace M C' ile builder'ın commit'ine çevir.
  it("c1_muhur::replace_nesnesiyle_muhur_commitini_degistiren_kirmizidir", () => {
    const dir = depoKur();
    const m = git(dir, "rev-parse", "muhur-onay^{commit}");
    kendiniMuhurle(dir);
    git(dir, "replace", "-f", m, git(dir, "rev-parse", "HEAD"));
    kirmizi(dir);
  }, 60000);

  // Yargıç saldırısı: rename algılaması mühürlü dosyayı dışarı taşıyan commit'te yalnız yeni adı gösterir.
  it("c1_muhur::muhurlu_dosyayi_disari_tasiyan_commit_yeniden_adlandirmayla_kacamaz", () => {
    const dir = depoKur();
    const asil = fs.readFileSync(path.join(dir, "araclar", "kapilar", "a.ts"), "utf-8");
    fs.mkdirSync(path.join(dir, "x"), { recursive: true });
    git(dir, "mv", "araclar/kapilar/a.ts", "x/a.ts");
    git(dir, "commit", "-q", "-m", "tasi");
    yaz(dir, "araclar/kapilar/a.ts", asil);
    kirmizi(dir, /araclar\/kapilar\/a\.ts/);
  }, 60000);

  it("c1_muhur::grafts_dosyasi_kirmizidir", () => {
    const dir = depoKur();
    fs.writeFileSync(path.join(dir, ".git", "info", "grafts"), `${git(dir, "rev-parse", "HEAD")} ${git(dir, "rev-parse", "HEAD^1")}\n`);
    kirmizi(dir, /graft/i);
  }, 60000);

  it("c1_muhur::sig_depo_kirmizidir", () => {
    const dir = depoKur();
    fs.writeFileSync(path.join(dir, ".git", "shallow"), git(dir, "rev-parse", "HEAD^1") + "\n");
    kirmizi(dir, /shallow/i);
  }, 60000);
});
