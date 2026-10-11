import { gitDurumunuAl, temizKopyadaKos } from "./temiz-kopya.ts";
import { spawnSync } from "child_process";
import fs from "fs";
import os from "os";
import path from "path";

// P1-A2: temiz-kopya yalıtımını kendi kurduğu tek kullanımlık depoda ölçer (os.tmpdir altında,
// adında 'test-temiz-kopya' geçen dizin). Kervan çalışma ağacına ve port/kayit'e yazmaz; aynı ağaçta
// koşan başka işlerin git durumunu değiştirmesi sonucu etkilemez.

function git(dizin: string, ...a: string[]) {
  const p = spawnSync("git", ["-c", "user.name=tk", "-c", "user.email=tk@ornek.invalid", "-c", "commit.gpgsign=false", ...a], { cwd: dizin, encoding: "utf-8" });
  if (p.status !== 0) throw new Error(`git ${a.join(" ")} başarısız: ${p.stderr}`);
  return p.stdout.trim();
}

function main(kok: string) {
  console.log("=== P1-A2 DOĞRULAMA TESTİ BAŞLADI ===");
  const depo = path.join(kok, "depo");
  fs.mkdirSync(path.join(depo, "araclar", "kapilar"), { recursive: true });
  git(depo, "init", "-q");
  // Commit'teki (mühürsüz) kapı: koşarsa HEDEF-KAPISI basar.
  fs.writeFileSync(path.join(depo, "araclar", "kapilar", "kapi.ts"), 'console.log("HEDEF-KAPISI");\n');
  fs.writeFileSync(path.join(depo, "izlenen.txt"), "x\n");
  git(depo, "add", "-A");
  git(depo, "commit", "-q", "-m", "hedef");

  // Çağıranın (mühürlü) kapısı: izlenmeyen dosyayı görürse 1, görmezse 0 ile çıkar.
  const kapiKaynagi = path.join(kok, "cagiran-kapilar");
  fs.mkdirSync(kapiKaynagi, { recursive: true });
  fs.writeFileSync(
    path.join(kapiKaynagi, "kapi.ts"),
    'import fs from "fs"; console.log("CAGIRAN-KAPISI"); process.exit(fs.existsSync("izlenmeyen.txt") || !fs.existsSync("izlenen.txt") ? 1 : 0);\n'
  );
  // İzlenmeyen dosya: çalışma ağacında var, commit'te yok.
  fs.writeFileSync(path.join(depo, "izlenmeyen.txt"), "bu dosya git commitinde YOKTUR!\n");

  const baslangic = gitDurumunuAl(depo);
  const yerel = spawnSync("bun", [path.join(kapiKaynagi, "kapi.ts")], { cwd: depo, encoding: "utf-8" });
  console.log("1. Çalışma dizininde kapı çıkışı:", yerel.status, "(beklenen: 1, izlenmeyen dosya görünür)");
  if (yerel.status !== 1) return "çalışma dizininde izlenmeyen dosya görünmedi; düzenek ölçmüyor";

  const s = temizKopyadaKos("HEAD", "bun araclar/kapilar/kapi.ts", path.join(kok, "gecici"), { depo, kapiKaynagi, zamanAsimiMs: 60_000 });
  console.log("2. Temiz kopyada çıkış:", s.cikis, "(beklenen: 0, izlenmeyen dosya kopyada yok)");
  if (s.cikis !== 0) return `temiz kopyada kapı yeşil değil: ${s.neden}`;
  if (!s.stdout.includes("CAGIRAN-KAPISI") || s.stdout.includes("HEDEF-KAPISI")) {
    return "temiz kopyada çağıranın kapısı yerine hedefin kendi kapısı koştu";
  }
  console.log("3. Koşan kapı çağıranınki: evet");

  const bitis = gitDurumunuAl(depo);
  console.log("4. Depo git durumu özdeş mi:", baslangic === bitis);
  if (baslangic !== bitis) return `depo kirlendi: '${baslangic}' → '${bitis}'`;
  if (s.calismaDiziniDegistiMi) return "temiz-kopya çalışma dizini değişti diyor";

  // Koşu sırasında kapıyı değiştirip aslını geri yazan komut: koşu sonu bayt karşılaştırması bunu göremez, izleyici görmeli.
  const degistirGeriYaz =
    `bun -e "const fs=require('fs');const y='araclar/kapilar/kapi.ts';const a=fs.readFileSync(y);fs.writeFileSync(y,'process.exit(0)');fs.writeFileSync(y,a)"`;
  const d = temizKopyadaKos("HEAD", degistirGeriYaz, path.join(kok, "gecici"), { depo, kapiKaynagi, zamanAsimiMs: 60_000 });
  console.log("5. Koşu sırasında değiştirilip geri yazılan kapı:", d.hukum, d.neden, "(beklenen: kirmizi kapi-degisti)");
  if (d.hukum !== "kirmizi" || !d.neden.includes("kapi-degisti")) return "koşu sırasında değiştirilip geri yazılan kapı yakalanmadı";
  return "";
}

const kok = fs.mkdtempSync(path.join(os.tmpdir(), "test-temiz-kopya-"));
let neden: string;
try {
  neden = main(kok);
} catch (e: any) {
  neden = `test-temiz-kopya çöktü: ${e?.message ?? e}`;
} finally {
  try {
    fs.rmSync(kok, { recursive: true, force: true });
  } catch {}
}
if (neden) {
  console.log(`HUKUM: kirmizi ${neden.replace(/\s+/g, " ")}`);
  process.exit(1);
}
console.log("P1-A2 tüm kabul kriterleri doğrulandı.");
console.log("HUKUM: yesil");
