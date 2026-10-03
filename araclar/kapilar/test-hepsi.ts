import { kapilariKos, tekKapiKos } from "./hepsi.ts";
import fs from "fs";

async function main() {
  console.log("=== P1-A1 DOĞRULAMA TESTİ BAŞLADI ===");

  // 1. Kayıt boşken çıkış 0 (hepsiYesil = true)
  const bos = await kapilariKos([], "TEST-BOS", "port/kayit/_test_bos", "port/kayit/_test_bos/durum.json");
  console.log("1. Kayıt boşken çıkış yeşil mi:", bos.hepsiYesil, "(beklenen: true)");
  if (!bos.hepsiYesil) process.exit(1);

  // 2. Kasıtlı kırık kapıyla çıkış != 0
  const kirik = await kapilariKos(
    [{ ad: "kirik-ornek", komut: "bun -e \"process.exit(42)\"" }],
    "TEST-KIRIK",
    "port/kayit/_test_kirik",
    "port/kayit/_test_kirik/durum.json"
  );
  console.log("2. Kırık kapıyla çıkış yeşil mi:", kirik.hepsiYesil, "(beklenen: false)");
  console.log("   Kırık kapı çıkış kodu:", kirik.sonuclar[0].cikis, "(beklenen: 42)");
  if (kirik.hepsiYesil || kirik.sonuclar[0].cikis !== 42) process.exit(2);

  // 3. Bilerek zaman aşan kapıyla çıkış != 0
  const zamanAsan = await tekKapiKos(
    {
      ad: "zaman-asimi-ornek",
      komut: "bun",
      argumanlar: ["-e", "await new Promise(r => setTimeout(r, 5000))"],
      timeoutMs: 400,
    },
    "port/kayit/_test_timeout/zaman-asimi.log",
    400
  );
  console.log("3. Zaman aşımı çıkış kodu:", zamanAsan.cikis, "(beklenen: 124)");
  console.log("   Zaman aşımı hatası:", zamanAsan.hata);
  if (zamanAsan.cikis === 0) process.exit(3);

  // 4. port/kayit/ altında ham çıktı dosyası oluştu mu
  const logVar = fs.existsSync("port/kayit/_test_kirik/kirik-ornek.log");
  console.log("4. Ham log dosyası oluştu mu:", logVar, "(beklenen: true)");
  if (!logVar) process.exit(4);

  // Temizlik
  fs.rmSync("port/kayit/_test_bos", { recursive: true, force: true });
  fs.rmSync("port/kayit/_test_kirik", { recursive: true, force: true });
  fs.rmSync("port/kayit/_test_timeout", { recursive: true, force: true });

  console.log("✅ P1-A1 TÜM KABUL KRİTERLERİ BAŞARIYLA DOĞRULANDI.");
}

await main();
