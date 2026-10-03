import crypto from "crypto";
import fs from "fs";
import path from "path";

// PRD FR-2.4: Mühür Üretici (muhur-uret.ts)
// Tam mühürlü dosyaların (araclar/kapilar/**, ref/**) SHA-256 özetlerini port/muhur.toml dosyasına yazar.

export function tumDosyalariListele(dizin: string): string[] {
  const sonuclar: string[] = [];
  if (!fs.existsSync(dizin)) return sonuclar;

  const entries = fs.readdirSync(dizin, { withFileTypes: true });
  for (const entry of entries) {
    const tamYol = path.join(dizin, entry.name);
    if (entry.isDirectory()) {
      sonuclar.push(...tumDosyalariListele(tamYol));
    } else if (entry.isFile()) {
      sonuclar.push(tamYol.replace(/\\/g, "/"));
    }
  }
  return sonuclar.sort();
}

export function muhurDosyasiUret(cikisYolu = path.join("port", "muhur.toml")): string {
  const dosyalar = tumDosyalariListele("araclar/kapilar");

  let cikti = `# port/muhur.toml - SHA-256 Mühür Listesi
# PRD FR-2.4: P3'ün insan kapısında Ercan onayı ile mühürlenmiştir.
# Bu dosyada listelenen dosyalar P4 ve sonrasında DEĞİŞTİRİLEMEZ.

`;

  for (const dosya of dosyalar) {
    const buf = fs.readFileSync(dosya);
    const sha = crypto.createHash("sha256").update(buf).digest("hex");
    cikti += `[[dosya]]\nyol = "${dosya}"\nsha256 = "${sha}"\n\n`;
  }

  const cikisDir = path.dirname(cikisYolu);
  if (!fs.existsSync(cikisDir)) {
    fs.mkdirSync(cikisDir, { recursive: true });
  }

  fs.writeFileSync(cikisYolu, cikti, "utf-8");
  return cikti;
}

if (import.meta.main) {
  muhurDosyasiUret();
  console.log("✅ [muhur-uret] port/muhur.toml başarıyla üretildi.");
}
