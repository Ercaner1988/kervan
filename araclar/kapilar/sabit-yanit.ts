import fs from "fs";
import path from "path";

// PRD FR-2.2: Sabit Yanıt Taraması (sabit-yanit.ts)
// Üretim kaynağında (test dışı src/ dosyalarında) sahte / sabit yanıt kalıplarını arar.
// #[cfg(test)] blokları içindeki eşleşmeler yoksayılır.

export const VARSAYILAN_KALIPLAR = [
  "chatcmpl-123",
  "1700000000",
  "selamlar!",
  "Simüle edilmiş",
];

export interface KalipBulgusu {
  dosya: string;
  satirNo: number;
  kalip: string;
  satirMetni: string;
}

export function rustDosyalariniBul(dizin: string): string[] {
  const dosyalar: string[] = [];
  if (!fs.existsSync(dizin)) return dosyalar;

  const entries = fs.readdirSync(dizin, { withFileTypes: true });
  for (const entry of entries) {
    const tamYol = path.join(dizin, entry.name);
    if (entry.isDirectory()) {
      if (entry.name !== "target" && entry.name !== ".git" && entry.name !== "ref") {
        dosyalar.push(...rustDosyalariniBul(tamYol));
      }
    } else if (entry.isFile() && entry.name.endsWith(".rs")) {
      // Yalnızca src/ altındaki dosyalar üretim kodudur
      if (tamYol.includes(`${path.sep}src${path.sep}`) || tamYol.startsWith(`src${path.sep}`)) {
        dosyalar.push(tamYol);
      }
    }
  }
  return dosyalar;
}

export function dosyayiTara(
  dosyaYolu: string,
  kaliplar = VARSAYILAN_KALIPLAR
): KalipBulgusu[] {
  const bulgular: KalipBulgusu[] = [];
  const icerik = fs.readFileSync(dosyaYolu, "utf-8");
  const satirlar = icerik.split(/\r?\n/);

  let testModulundeMi = false;
  let parantezDerinligi = 0;
  let testParantezBaslangic = -1;

  for (let i = 0; i < satirlar.length; i++) {
    const satir = satirlar[i];
    const trimSatir = satir.trim();

    // #[cfg(test)] tespiti
    if (trimSatir.includes("#[cfg(test)]") || trimSatir.startsWith("#[test]")) {
      testModulundeMi = true;
      testParantezBaslangic = parantezDerinligi;
    }

    // Parantez derinliği takibi
    for (const char of satir) {
      if (char === "{") parantezDerinligi++;
      else if (char === "}") {
        parantezDerinligi--;
        if (testModulundeMi && parantezDerinligi <= testParantezBaslangic) {
          testModulundeMi = false;
          testParantezBaslangic = -1;
        }
      }
    }

    // Eğer test bloğu içindeyse atla
    if (testModulundeMi) continue;

    // Yorum satırlarını atla
    if (trimSatir.startsWith("//") || trimSatir.startsWith("/*") || trimSatir.startsWith("*")) {
      continue;
    }

    for (const kalip of kaliplar) {
      if (satir.includes(kalip)) {
        bulgular.push({
          dosya: dosyaYolu,
          satirNo: i + 1,
          kalip,
          satirMetni: trimSatir,
        });
      }
    }
  }

  return bulgular;
}

export function sabitYanitlariTara(
  kokDizin = process.cwd(),
  kaliplar = VARSAYILAN_KALIPLAR
): {
  temizMi: boolean;
  bulgular: KalipBulgusu[];
  tarananDosyaSayisi: number;
} {
  const dosyalar = rustDosyalariniBul(kokDizin);
  const bulgular: KalipBulgusu[] = [];

  for (const dosya of dosyalar) {
    const dBulgulari = dosyayiTara(dosya, kaliplar);
    bulgular.push(...dBulgulari);
  }

  return {
    temizMi: bulgular.length === 0,
    bulgular,
    tarananDosyaSayisi: dosyalar.length,
  };
}

if (import.meta.main) {
  const sonuc = sabitYanitlariTara();
  console.log(`[sabit-yanit] Taranan Rust kaynak dosyası: ${sonuc.tarananDosyaSayisi}`);

  if (sonuc.temizMi) {
    console.log("✅ [sabit-yanit] Üretim kodunda hiçbir sabit sahte kalıp bulunamadı.");
    process.exit(0);
  } else {
    console.error("❌ [sabit-yanit] ÜRETİM KODUNDA SAHTE/SABİT YANIT KALIPLARI BULUNDU:");
    for (const b of sonuc.bulgular) {
      console.error(`  - ${b.dosya}:${b.satirNo} [${b.kalip}] -> "${b.satirMetni}"`);
    }
    process.exit(1);
  }
}
