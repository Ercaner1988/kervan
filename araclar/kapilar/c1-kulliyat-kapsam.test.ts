// Külliyat girdi denetimi, kapının gerçekten okuduğu dosya kümesine bakmalı (yargıç engeli, C1 3. tur):
// sabit-yanit yalnız src/ altını, parite yalnız araclar/parite'nin üst düzeyini okur. Kapının görmediği
// dosya "girdi var" sayılırsa, girdisiz bir çöp ağacın kırmızısı "yakalandı" diye yazılır.
import { afterAll, describe, expect, it } from "bun:test";
import { spawnSync } from "child_process";
import fs from "fs";
import os from "os";
import path from "path";

const KOK = fs.mkdtempSync(path.join(os.tmpdir(), "kervan-c1-kulliyat-kapsam-"));
afterAll(() => fs.rmSync(KOK, { recursive: true, force: true }));

describe("c1::kulliyat_girdi_kapsami", () => {
  it("c1::kapinin_okumadigi_dosya_girdi_sayilmaz", async () => {
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
    // Tek Rust dosyası tests/ altında: sabit-yanit'in taradığı src/ boş.
    y("tests/a.rs", "#[test]\nfn t() {}\n");
    y("Cargo.toml", `[package]\nname = "kervan"\nversion = "0.1.0"\nedition = "2021"\n`);
    // Tek karşılaştırıcı alt dizinde: pariteBaglantisiniDenetle üst düzeyi okur.
    y("araclar/parite/alt/kos.ts", "export {};\n");
    y("port/altin/a.json", `{"girdi":{},"cikti":{}}`);
    g("add", "-A");
    g("commit", "-q", "-m", "cop");
    const r = kulliyatCommitiniTara("HEAD", {
      depo: cop, kapiKaynagi: import.meta.dir, kapiZamanAsimiMs: 180_000, geciciKok: path.join(KOK, "gk"),
    });
    const ozet = r.kapilar.map((k) => `[${k.durum}] ${k.ad}: ${k.neden.slice(0, 140)}`).join("\n");
    const bul = (ad: string) => r.kapilar.find((k) => k.ad === ad);
    expect(`${bul("sabit-yanit")?.neden}\n${ozet}`).toContain(".rs dosyası yok");
    expect(`${bul("parite-baglantisi")?.neden}\n${ozet}`).toContain(".ts dosyası yok");
    expect(r.yakalayanlar["sabit-yanit"]).toEqual([]);
    expect(r.yakalayanlar["kendini-dogrulayan-parite"]).toEqual([]);
  }, 600_000);
});
