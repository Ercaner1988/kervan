import { gitDurumunuAl, temizKopyadaKos } from "./temiz-kopya.ts";
import { spawnSync } from "child_process";
import fs from "fs";

async function main() {
  console.log("=== P1-A2 DOĞRULAMA TESTİ BAŞLADI ===");

  const baslangicDurumu = gitDurumunuAl();

  // İzlenmeyen geçici bir dosya oluşturuyoruz (git commit edilmemiş!)
  const izlenmeyenDosya = "temp_izlenmeyen_gizli_dosya.txt";
  fs.writeFileSync(izlenmeyenDosya, "bu dosya git commitinde YOKTUR!", "utf-8");

  // Çalışma dizininde bu dosyayı arayan komut:
  const komut = `bun -e "import fs from 'fs'; if (!fs.existsSync('${izlenmeyenDosya}')) process.exit(1);"`;

  // 1. Çalışma dizininde koş: yeşil olmalı (çünkü dosya burada var)
  const yerelProc = spawnSync(komut, { shell: true, encoding: "utf-8" });
  const yerelCikis = yerelProc.status;
  console.log("1. Çalışma dizininde test sonucu çıkış kodu:", yerelCikis, "(beklenen: 0)");
  if (yerelCikis !== 0) {
    fs.unlinkSync(izlenmeyenDosya);
    process.exit(1);
  }

  // 2. Temiz kopyada koş: kırmızı olmalı (çünkü worktree commit'ten açılır, izlenmeyen dosya worktree'ye girmez!)
  const temizSonuc = temizKopyadaKos("HEAD", komut);
  console.log("2. Temiz kopyada test sonucu çıkış kodu:", temizSonuc.cikis, "(beklenen: != 0, kırmızı)");
  if (temizSonuc.cikis === 0) {
    fs.unlinkSync(izlenmeyenDosya);
    console.error("HATA: Temiz kopyada izlenmeyen dosya bulundu, yalıtım başarısız!");
    process.exit(2);
  }

  // İzlenmeyen test dosyasını temizle
  fs.unlinkSync(izlenmeyenDosya);

  // 3. Betik öncesi ve sonrası çalışma dizininin git status --porcelain çıktısı özdeş olmalı
  const bitisDurumu = gitDurumunuAl();
  const ozdesMi = baslangicDurumu === bitisDurumu;
  console.log("3. Çalışma dizini git status öncesi ve sonrası özdeş mi:", ozdesMi, "(beklenen: true)");
  if (!ozdesMi) {
    console.error(`HATA: Çalışma dizini kirlendi!\nBaşlangıç: ${baslangicDurumu}\nBitiş: ${bitisDurumu}`);
    process.exit(3);
  }

  console.log("✅ P1-A2 TÜM KABUL KRİTERLERİ BAŞARIYLA DOĞRULANDI.");
}

await main();
