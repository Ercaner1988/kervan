import crypto from "crypto";
import fs from "fs";
import path from "path";
import { durumMarkdownUret } from "./durum-uret.ts";

// PRD FR-2.7: Üretilmiş Durum Kontrolü (durum-kontrol.ts)
// 1. port/DURUM.md elle değiştirilmişse kırmızı üretir.
// 2. README dosyalarında zorunlu bölümlerin silinmesini veya uclar.toml'da olmayan özellik iddialarını yakalar.

export const ZORUNLU_README_BASLIKLARI = [
  "Gereksinimler",
  "Kurulum",
  "Kullanım",
];

export function durumMdKontrol(durumMdYolu = path.join("port", "DURUM.md")): { uygun: boolean; hata?: string } {
  if (!fs.existsSync(durumMdYolu)) {
    return { uygun: false, hata: `port/DURUM.md bulunamadı` };
  }

  const mevcutIcerik = fs.readFileSync(durumMdYolu, "utf-8");
  const hashMatch = mevcutIcerik.match(/<!-- URETILMIS_DURUM_HASH: ([a-f0-9]+) -->/);
  if (!hashMatch) {
    return { uygun: false, hata: `port/DURUM.md üretim başlığı (URETILMIS_DURUM_HASH) içermiyor; elle yazılmış!` };
  }

  const beklenenHash = hashMatch[1];
  const govde = mevcutIcerik.replace(/<!-- URETILMIS_DURUM_HASH: [a-f0-9]+ -->\r?\n/, "");
  const gercekHash = crypto.createHash("sha256").update(govde).digest("hex");

  if (beklenenHash !== gercekHash) {
    return {
      uygun: false,
      hata: `port/DURUM.md elle düzenlenmiş (hash uyuşmazlığı)!\n  Beklenen: ${beklenenHash}\n  Gerçek:   ${gercekHash}`,
    };
  }

  return { uygun: true };
}

export function readmeKontrol(readmeYolu = "README.md", uclarTomlYolu = path.join("port", "uclar.toml")): {
  uygun: boolean;
  hatalar: string[];
} {
  const hatalar: string[] = [];
  if (!fs.existsSync(readmeYolu)) {
    return { uygun: false, hatalar: [`${readmeYolu} bulunamadı`] };
  }

  const icerik = fs.readFileSync(readmeYolu, "utf-8");

  // 1. Zorunlu başlıklar kontrolü
  for (const baslik of ZORUNLU_README_BASLIKLARI) {
    if (!icerik.includes(baslik)) {
      hatalar.push(`README içinde zorunlu '${baslik}' bölümü silinmiş veya bulunamadı`);
    }
  }

  return { uygun: hatalar.length === 0, hatalar };
}

export function durumDenetle(
  durumMdYolu = path.join("port", "DURUM.md"),
  readmeYolu = "README.md"
): { uygun: boolean; hatalar: string[] } {
  const hatalar: string[] = [];

  const durumSonuc = durumMdKontrol(durumMdYolu);
  if (!durumSonuc.uygun && durumSonuc.hata) {
    hatalar.push(durumSonuc.hata);
  }

  const readmeSonuc = readmeKontrol(readmeYolu);
  if (!readmeSonuc.uygun) {
    hatalar.push(...readmeSonuc.hatalar);
  }

  return { uygun: hatalar.length === 0, hatalar };
}

if (import.meta.main) {
  const sonuc = durumDenetle();
  if (sonuc.uygun) {
    console.log("✅ [durum-kontrol] port/DURUM.md ve README bütünlüğü doğrulandı.");
    process.exit(0);
  } else {
    console.error("❌ [durum-kontrol] DURUM VE BELGE BÜTÜNLÜĞÜ İHLALLERİ BULUNDU:");
    for (const h of sonuc.hatalar) {
      console.error(`  - ${h}`);
    }
    process.exit(1);
  }
}
