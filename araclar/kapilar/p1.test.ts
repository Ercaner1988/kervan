import { describe, expect, it } from "bun:test";
import { kapilariKos, tekKapiKos } from "./hepsi.ts";
import { gitDurumunuAl, temizKopyadaKos } from "./temiz-kopya.ts";
import { kabulHaritasiniDogrula } from "./kabul-haritasi.ts";
import { makineDurumuKontrol } from "./makine-sakin.ts";
import { spawnSync } from "child_process";
import fs from "fs";
import os from "os";
import path from "path";

describe("p1::kapilar", () => {
  it("p1::a1_hepsi_kapilari_calistirir_ve_hata_yakalar", async () => {
    // Geçici kayıtlar depo ağacına (port/kayit/_test_*) değil os.tmpdir() altına yazılır.
    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "kervan-c1-kosucu-p1-"));
    // 1. Boş kapı listesi YEŞİL DEĞİL (2026-10-10 düzeltme): eski assert 'boş kayıt yeşil olmalı' yanlıştı;
    //    hiçbir kapı koşmayan koşu hiçbir şey ölçmez, P4-P10'da sıfır kapının yeşil sayılması buradan geldi.
    const bos = await kapilariKos([], "TEST-BOS", path.join(tmp, "_test_bos"), path.join(tmp, "_test_bos", "durum.json"));
    expect(bos.hepsiYesil).toBe(false);

    // 2. Kasıtlı kırık kapıyla çıkış != 0
    const kirik = await kapilariKos(
      [{ ad: "kirik-ornek", komut: "bun", argumanlar: ["-e", "process.exit(42)"] }],
      "TEST-KIRIK",
      path.join(tmp, "_test_kirik"),
      path.join(tmp, "_test_kirik", "durum.json")
    );
    expect(kirik.hepsiYesil).toBe(false);
    expect(kirik.sonuclar[0].cikis).toBe(42);

    // 3. Bilerek zaman aşan kapıyla çıkış != 0 (124)
    const zamanAsan = await tekKapiKos(
      {
        ad: "zaman-asimi-ornek",
        komut: "bun",
        argumanlar: ["-e", "await new Promise(function(res){ setTimeout(res, 5000); })"],
        timeoutMs: 300,
      },
      path.join(tmp, "_test_timeout", "zaman-asimi.log"),
      300
    );
    expect(zamanAsan.cikis).not.toBe(0);

    // 4. Log dosyası varlığı
    const logVar = fs.existsSync(path.join(tmp, "_test_kirik", "kirik-ornek.log"));
    expect(logVar).toBe(true);

    fs.rmSync(tmp, { recursive: true, force: true });
  }, 30000);

  it("p1::a2_temiz_kopya_izlenmeyen_dosyalari_yitirir_ve_calisma_agacini_korur", () => {
    const baslangicDurumu = gitDurumunuAl();
    const izlenmeyenDosya = "temp_izlenmeyen_p1_test.txt";
    fs.writeFileSync(izlenmeyenDosya, "git commitinde yok", "utf-8");

    const komut = `bun -e "import fs from 'fs'; if (!fs.existsSync('${izlenmeyenDosya}')) process.exit(1);"`;
    const yerelProc = spawnSync(komut, { shell: true, encoding: "utf-8" });
    expect(yerelProc.status).toBe(0);

    const temizSonuc = temizKopyadaKos("HEAD", komut);
    expect(temizSonuc.cikis).not.toBe(0);

    if (fs.existsSync(izlenmeyenDosya)) {
      fs.unlinkSync(izlenmeyenDosya);
    }
    const bitisDurumu = gitDurumunuAl();
    expect(bitisDurumu).toBe(baslangicDurumu);
  }, 30000);

  it("p1::a3_kabul_haritasi_gecersiz_veya_eksik_test_kimliklerini_reddeder", () => {
    fs.writeFileSync("temp_gecersiz_p1.json", JSON.stringify({
      faz: "TEST-GECERSIZ",
      maddeler: [{ id: "T-1", kanit: ["olmayan::test::xyz_999"] }]
    }));
    fs.writeFileSync("temp_kanitsiz_p1.json", JSON.stringify({
      faz: "TEST-KANITSIZ",
      maddeler: [{ id: "T-2", kanit: [] }]
    }));
    fs.writeFileSync("temp_test_list_p1.txt", "p1::a3_ornek_gecerli_test: test\n");
    fs.writeFileSync("temp_gecerli_p1.json", JSON.stringify({
      faz: "TEST-GECERLI",
      maddeler: [{ id: "T-3", kanit: ["p1::a3_ornek_gecerli_test"] }]
    }));

    const s1 = kabulHaritasiniDogrula("temp_gecersiz_p1.json", "temp_test_list_p1.txt");
    expect(s1.gecerli).toBe(false);

    const s2 = kabulHaritasiniDogrula("temp_kanitsiz_p1.json", "temp_test_list_p1.txt");
    expect(s2.gecerli).toBe(false);

    const s3 = kabulHaritasiniDogrula("temp_gecerli_p1.json", "temp_test_list_p1.txt");
    expect(s3.gecerli).toBe(true);

    fs.unlinkSync("temp_gecersiz_p1.json");
    fs.unlinkSync("temp_kanitsiz_p1.json");
    fs.unlinkSync("temp_gecerli_p1.json");
    fs.unlinkSync("temp_test_list_p1.txt");
  }, 30000);

  it("p1::a4_makine_sakin_esik_ici_ve_disi_davranislari_ayirt_eder", async () => {
    const asiriYuksek = await makineDurumuKontrol({ minRamGb: 9999, maxCpuPct: 0 });
    expect(asiriYuksek.uygun).toBe(false);

    const dusuk = await makineDurumuKontrol({ minRamGb: 0.01, maxCpuPct: 100 });
    expect(dusuk.uygun).toBe(true);
  }, 30000);
});
