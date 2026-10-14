import { kapilariKos, tekKapiKos } from "./hepsi.ts";
import fs from "fs";
import os from "os";
import path from "path";

// P1-A1 elle doğrulama betiği. Geçici dosyalar yalnız os.tmpdir() altında ('c1-kosucu' etiketli).
async function main() {
  console.log("=== P1-A1 DOĞRULAMA TESTİ BAŞLADI ===");
  const kok = fs.mkdtempSync(path.join(os.tmpdir(), "kervan-c1-kosucu-test-hepsi-"));
  try {
    // 1. Boş kapı listesi YEŞİL SAYILMAZ (fail-closed, PRD FR-1.1).
    //    Eski assert 'boş kayıt yeşil olmalı' yanlıştı: hiçbir kapı koşmayan bir koşu hiçbir şey
    //    ölçmez; onu yeşil saymak P4-P10'da sıfır kapının 'tüm kapılar geçti' demesine yol açtı
    //    (audit-backlog: 'hepsi.ts P4-P10 için sıfır kapı koşar ve bunu YEŞİL sayar').
    const bos = await kapilariKos([], "TEST-BOS", path.join(kok, "bos"), path.join(kok, "bos", "durum.json"));
    console.log("1. Boş listeyle yeşil mi:", bos.hepsiYesil, "(beklenen: false)");
    if (bos.hepsiYesil) process.exit(1);

    // 2. Kasıtlı kırık kapıyla çıkış != 0
    const kirik = await kapilariKos(
      [{ ad: "kirik-ornek", komut: "bun", argumanlar: ["-e", "process.exit(42)"] }],
      "TEST-KIRIK",
      path.join(kok, "kirik"),
      path.join(kok, "kirik", "durum.json")
    );
    console.log("2. Kırık kapıyla yeşil mi:", kirik.hepsiYesil, "(beklenen: false)");
    console.log("   Kırık kapı çıkış kodu:", kirik.sonuclar[0].cikis, "(beklenen: 42)");
    if (kirik.hepsiYesil || kirik.sonuclar[0].cikis !== 42) process.exit(2);

    // 3. Bilerek zaman aşan kapıyla çıkış != 0
    const zamanAsan = await tekKapiKos(
      {
        ad: "zaman-asimi-ornek",
        komut: "bun",
        argumanlar: ["-e", "\"await new Promise(r => setTimeout(r, 5000))\""],
        timeoutMs: 400,
      },
      path.join(kok, "timeout", "zaman-asimi.log"),
      400
    );
    console.log("3. Zaman aşımı çıkış kodu:", zamanAsan.cikis, "(beklenen: 124)");
    if (zamanAsan.cikis === 0 || zamanAsan.hukum !== "kirmizi") process.exit(3);

    // 4. Ham çıktı dosyası oluştu mu
    const logVar = fs.existsSync(path.join(kok, "kirik", "kirik-ornek.log"));
    console.log("4. Ham log dosyası oluştu mu:", logVar, "(beklenen: true)");
    if (!logVar) process.exit(4);

    // 5. Çıkış 0 ama hüküm satırı yok -> kırmızı
    const hukumsuz = await kapilariKos(
      [{ ad: "hukumsuz", komut: "bun", argumanlar: ["-e", "\"console.log('tamam')\""] }],
      "TEST-HUKUMSUZ",
      path.join(kok, "hukumsuz"),
      path.join(kok, "hukumsuz", "durum.json")
    );
    console.log("5. Hükümsüz kapıyla yeşil mi:", hukumsuz.hepsiYesil, "(beklenen: false)");
    if (hukumsuz.sonuclar[0].cikis !== 0 || hukumsuz.hepsiYesil) process.exit(5);

    console.log("P1-A1 kabul kriterleri doğrulandı.");
  } finally {
    fs.rmSync(kok, { recursive: true, force: true });
  }
}

await main();
