import crypto from "crypto";
import { spawnSync } from "child_process";
import fs from "fs";
import path from "path";

// PRD FR-2.4: Mühür Kontrolü (muhur-kontrol.ts)
// Üç koruma sınıfını denetler:
// 1. Tam mühürlü dosyalar (port/muhur.toml içindeki SHA-256 listesi)
// 2. Yalnız-ekleme dosyası (port/mutantlar.toml: var olan girdi silinemez veya değiştirilemez)
// 3. Yeniden üretilebilir altın dosyalar (port/altin/*.json: üretici özeti ve başlık doğrulaması)

export interface MuhurDosyasi {
  tarih?: string;
  dosyalar: { yol: string; sha256: string }[];
}

export function dosyaSha256(dosyaYolu: string): string {
  if (!fs.existsSync(dosyaYolu)) return "";
  const buf = fs.readFileSync(dosyaYolu);
  return crypto.createHash("sha256").update(buf).digest("hex");
}

export function muhurTomlOku(dosyaYolu = path.join("port", "muhur.toml")): MuhurDosyasi | null {
  if (!fs.existsSync(dosyaYolu)) return null;
  const icerik = fs.readFileSync(dosyaYolu, "utf-8");

  const dosyalar: { yol: string; sha256: string }[] = [];
  const bloklar = icerik.split(/\[\[dosya\]\]/);

  for (const blok of bloklar) {
    const yolMatch = blok.match(/yol\s*=\s*["']([^"']+)["']/);
    const shaMatch = blok.match(/sha256\s*=\s*["']([^"']+)["']/);
    if (yolMatch && shaMatch) {
      dosyalar.push({ yol: yolMatch[1], sha256: shaMatch[1] });
    }
  }

  return { dosyalar };
}

export function mutantlarAppendOnlyKontrol(
  mutantlarYolu = path.join("port", "mutantlar.toml"),
  referansCommit?: string
): { uygun: boolean; hata?: string } {
  if (!fs.existsSync(mutantlarYolu)) return { uygun: true };

  // Git diff ile silinmiş / değiştirilmiş satırları kontrol et
  if (referansCommit) {
    const diffProc = spawnSync("git", ["diff", "-U0", referansCommit, "--", mutantlarYolu], {
      encoding: "utf-8",
    });
    if (diffProc.stdout) {
      const satirlar = diffProc.stdout.split(/\r?\n/);
      for (const s of satirlar) {
        if (s.startsWith("-") && !s.startsWith("---")) {
          // Var olan bir satır silinmiş veya değiştirilmiş!
          return {
            uygun: false,
            hata: `port/mutantlar.toml yalnız-ekleme kuralı ihlali: silinen/değişen satır: "${s.substring(1).trim()}"`,
          };
        }
      }
    }
  }

  return { uygun: true };
}

export function altinDosyalariKontrol(altinDizini = path.join("port", "altin")): { uygun: boolean; hatalar: string[] } {
  const hatalar: string[] = [];
  if (!fs.existsSync(altinDizini)) return { uygun: true, hatalar: [] };

  const entries = fs.readdirSync(altinDizini, { withFileTypes: true });
  for (const entry of entries) {
    if (entry.isFile() && entry.name.endsWith(".json")) {
      const tamYol = path.join(altinDizini, entry.name);
      try {
        const icerik = fs.readFileSync(tamYol, "utf-8");
        const json = JSON.parse(icerik);
        // Başlıkta kaynak_sha veya uretici_ozeti olmalıdır
        if (!json.kaynak_sha && !json.uretici_ozeti && !json._baslik) {
          hatalar.push(`Altın dosya '${entry.name}' üretici başlığı/özeti (kaynak_sha/uretici_ozeti) içermiyor`);
        }
      } catch (e: any) {
        hatalar.push(`Altın dosya '${entry.name}' geçerli bir JSON değil: ${e.message}`);
      }
    }
  }

  return { uygun: hatalar.length === 0, hatalar };
}

export function muhurDenetle(
  faz = "P2",
  muhurYolu = path.join("port", "muhur.toml"),
  mutantlarYolu = path.join("port", "mutantlar.toml"),
  referansCommit?: string
): { uygun: boolean; hatalar: string[] } {
  const hatalar: string[] = [];

  const muhur = muhurTomlOku(muhurYolu);
  if (!muhur) {
    // Faz 4 ve sonrası için muhur.toml ZORUNLUDUR
    const fazNum = parseInt(faz.replace(/[^0-9]/g, "") || "0", 10);
    if (fazNum >= 4) {
      hatalar.push(`P4 ve sonrası fazlar için ${muhurYolu} dosyası ZORUNLUDUR ancak bulunamadı!`);
    } else {
      // P1-P3 arası henüz mühürlenmediyse bilgilendirme
      return { uygun: true, hatalar: [] };
    }
  } else {
    // Mühürlü dosyaların SHA-256'larını kontrol et
    for (const d of muhur.dosyalar) {
      const gercekSha = dosyaSha256(d.yol);
      if (!gercekSha) {
        hatalar.push(`Mühürlü dosya bulunamadı: ${d.yol}`);
      } else if (gercekSha !== d.sha256) {
        hatalar.push(`MÜHÜR İHLALİ: '${d.yol}' değiştirilmiş!\n  Beklenen SHA: ${d.sha256}\n  Gerçek SHA:   ${gercekSha}`);
      }
    }
  }

  // 2. Mutantlar yalnız-ekleme kontrolü
  const mutantSonuc = mutantlarAppendOnlyKontrol(mutantlarYolu, referansCommit);
  if (!mutantSonuc.uygun && mutantSonuc.hata) {
    hatalar.push(mutantSonuc.hata);
  }

  // 3. Altın dosyalar kontrolü
  const altinSonuc = altinDosyalariKontrol();
  if (!altinSonuc.uygun) {
    hatalar.push(...altinSonuc.hatalar);
  }

  return { uygun: hatalar.length === 0, hatalar };
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  let faz = "P2";
  let refCommit: string | undefined;

  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--faz" && i + 1 < args.length) {
      faz = args[++i];
    } else if (args[i] === "--ref" && i + 1 < args.length) {
      refCommit = args[++i];
    }
  }

  const sonuc = muhurDenetle(faz, undefined, undefined, refCommit);

  if (sonuc.uygun) {
    console.log(`✅ [muhur-kontrol] Faz ${faz}: Mühür ve yalnız-ekleme kuralları geçerli.`);
    process.exit(0);
  } else {
    console.error(`❌ [muhur-kontrol] MÜHÜR VE BÜTÜNLÜK İHLALLERİ BULUNDU:`);
    for (const h of sonuc.hatalar) {
      console.error(`  - ${h}`);
    }
    process.exit(1);
  }
}
