import fs from "fs";
import path from "path";
import { MUHUR_YOLU, muhurKumesi, refOzeti } from "./muhur-kontrol.ts";

// PRD FR-2.4: Mühür üretici. Tam mühürlü kümenin (araclar/kapilar/**, port/altin/**, altın üreticileri)
// SHA-256 listesini ve ref/** ağaç özetini port/muhur.toml'a yazar. Onay YAZMAZ: onay_bekliyor = true
// bırakır. Onay, muhur.toml'u içeren commit'e Ercan'ın SSH anahtarıyla attığı 'muhur-onay' etiketidir
// (muhur-kontrol.ts, imzaliMuhurCommiti); muhur.toml'daki alanlar tek başına onay sayılmaz.

export function muhurDosyasiUret(kok: string = process.cwd(), cikisYolu = path.join(kok, MUHUR_YOLU)): string {
  const kume = muhurKumesi(kok);
  if (kume.hatalar.length) throw new Error(`Mühür üretilemez:\n  - ${kume.hatalar.join("\n  - ")}`);
  const ref = refOzeti(kok); // ref yoksa/boşsa fırlatır

  let cikti = `# port/muhur.toml - SHA-256 mühür listesi (PRD FR-2.4), muhur-uret.ts çıktısı.
# Onay alanlarını yalnız Ercan doldurur: onay_bekliyor = false ve onay = "<ad, tarih>", commit,
# ardından imzalı etiket: git -c gpg.format=ssh tag -s -f muhur-onay -m "<ad, tarih>"
# (genel anahtar depo dışında: ~/.config/kervan/muhur-imzacilar, allowed-signers biçimi).
onay_bekliyor = true
onay = ""

[ref]
ozet = "${ref.ozet}"
dosya_sayisi = ${ref.dosya_sayisi}

`;
  for (const yol of [...kume.dosyalar.keys()].sort()) {
    cikti += `[[dosya]]\nyol = "${yol}"\nsha256 = "${kume.dosyalar.get(yol)}"\n\n`;
  }

  fs.mkdirSync(path.dirname(cikisYolu), { recursive: true });
  fs.writeFileSync(cikisYolu, cikti, "utf-8");
  return cikti;
}

if (import.meta.main) {
  try {
    muhurDosyasiUret();
    console.log(`[muhur-uret] ${MUHUR_YOLU} üretildi; Ercan onayı bekliyor (onay_bekliyor = true; imzalı muhur-onay etiketi gerekli).`);
  } catch (e: any) {
    console.error(`[muhur-uret] ${e?.message ?? e}`);
    process.exit(1);
  }
}
