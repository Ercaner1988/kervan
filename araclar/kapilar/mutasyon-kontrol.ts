import { spawnSync } from "child_process";
import fs from "fs";
import path from "path";

// PRD FR-2.6: Mutasyon ve Parite Bağlantısı Kapısı (mutasyon-kontrol.ts)
// 1. port/mutantlar.toml içindeki mutantları uygular, mutantın testi KIRMIZIYA çevirdiğini (öldüğünü) doğrular.
// 2. araclar/parite/*.ts karşılaştırıcılarının gerçek Kervan koduna/ikilisine (Bun.spawn, fetch, crate) bağlandığını doğrular.

export interface MutantTanimi {
  ad: string;
  dosya: string;
  bul: string;
  yerine: string;
  oldurmesi_gereken_test: string[];
}

function tomlStringAl(blok: string, anahtar: string): string | null {
  const literalMatch = blok.match(new RegExp(`${anahtar}\\s*=\\s*'([^']*)'`));
  if (literalMatch) return literalMatch[1];

  const doubleMatch = blok.match(new RegExp(`${anahtar}\\s*=\\s*"((?:\\\\.|[^"\\\\])*)"`));
  if (doubleMatch) {
    try {
      return JSON.parse(`"${doubleMatch[1]}"`);
    } catch {
      return doubleMatch[1].replace(/\\"/g, '"');
    }
  }
  return null;
}

export function mutantlarTomlOku(dosyaYolu = path.join("port", "mutantlar.toml")): MutantTanimi[] {
  if (!fs.existsSync(dosyaYolu)) return [];
  const icerik = fs.readFileSync(dosyaYolu, "utf-8");
  const mutantlar: MutantTanimi[] = [];

  const bloklar = icerik.split(/\[\[mutant\]\]/);
  for (const b of bloklar) {
    const trimB = b.trim();
    if (!trimB) continue;

    const ad = tomlStringAl(trimB, "ad");
    const dosya = tomlStringAl(trimB, "dosya");
    const bul = tomlStringAl(trimB, "bul");
    const yerine = tomlStringAl(trimB, "yerine") ?? "";
    const testMatch = trimB.match(/oldurmesi_gereken_test\s*=\s*\[(.*?)\]/s);

    if (ad && dosya && bul) {
      let testler: string[] = [];
      if (testMatch) {
        testler = testMatch[1]
          .split(",")
          .map((t) => t.trim().replace(/^["']|["']$/g, ""))
          .filter((t) => t.length > 0);
      }

      mutantlar.push({
        ad,
        dosya,
        bul,
        yerine,
        oldurmesi_gereken_test: testler,
      });
    }
  }

  return mutantlar;
}

export function pariteBaglantisiniDenetle(pariteDizini = path.join("araclar", "parite")): {
  uygun: boolean;
  hatalar: string[];
} {
  const hatalar: string[] = [];
  if (!fs.existsSync(pariteDizini)) return { uygun: true, hatalar: [] };

  const entries = fs.readdirSync(pariteDizini, { withFileTypes: true });
  for (const entry of entries) {
    if (entry.isFile() && entry.name.endsWith(".ts")) {
      const tamYol = path.join(pariteDizini, entry.name);
      const icerik = fs.readFileSync(tamYol, "utf-8");

      // Gerçek Kervan kodunu veya sürecini çağırma belirtileri:
      const spawnVar = /Bun\.spawn|spawnSync|child_process/.test(icerik);
      const fetchVar = /fetch\(/.test(icerik);
      const importKervanVar = /from\s+["'].*kervan.*["']/.test(icerik);

      if (!spawnVar && !fetchVar && !importKervanVar) {
        hatalar.push(
          `Kendini doğrulayan parite tespit edildi: '${entry.name}' Kervan sürecine (spawn/fetch/import) bağlanmıyor!`
        );
      }
    }
  }

  return { uygun: hatalar.length === 0, hatalar };
}

export function mutantlariDene(mutantlar: MutantTanimi[]): {
  uygun: boolean;
  hatalar: string[];
  oldurulenSayisi: number;
} {
  const hatalar: string[] = [];
  let oldurulenSayisi = 0;

  for (const m of mutantlar) {
    if (!fs.existsSync(m.dosya)) {
      hatalar.push(`Mutant '${m.ad}': Hedef dosya '${m.dosya}' bulunamadı`);
      continue;
    }

    const orijinalIcerik = fs.readFileSync(m.dosya, "utf-8");
    if (!orijinalIcerik.includes(m.bul)) {
      hatalar.push(`Mutant '${m.ad}': Değiştirilecek metin bulunamadı: "${m.bul}"`);
      continue;
    }

    // Mutantı uygula
    const mutasyonluIcerik = orijinalIcerik.replace(m.bul, m.yerine);
    fs.writeFileSync(m.dosya, mutasyonluIcerik, "utf-8");

    try {
      // Testi çalıştır (testin başarısız olması BEKLENİR)
      let testBasariliMi = true;
      for (const t of m.oldurmesi_gereken_test) {
        let testProc;
        if (t.startsWith("bun ")) {
          testProc = spawnSync(t, { shell: true, encoding: "utf-8" });
        } else {
          testProc = spawnSync("cargo", ["test", "--workspace", "--all-features", "--", t], {
            encoding: "utf-8",
          });
        }

        if (testProc.status !== 0) {
          // Test başarısız oldu -> mutant öldürüldü!
          testBasariliMi = false;
          break;
        }
      }

      if (testBasariliMi) {
        // Test geçti -> mutant yaşadı -> HATA!
        hatalar.push(`Mutant '${m.ad}' YAŞADI! Eşleşen testler mutasyona rağmen yeşil kaldı.`);
      } else {
        oldurulenSayisi++;
      }
    } finally {
      // Orijinal içeriği mutlaka geri yükle
      fs.writeFileSync(m.dosya, orijinalIcerik, "utf-8");
    }
  }

  return { uygun: hatalar.length === 0, hatalar, oldurulenSayisi };
}

if (import.meta.main) {
  console.log("[mutasyon-kontrol] Parite bağlantıları ve mutantlar denetleniyor...");

  const pariteSonuc = pariteBaglantisiniDenetle();
  const mutantlar = mutantlarTomlOku();
  const mutasyonSonuc = mutantlariDene(mutantlar);

  const tumHatalar = [...pariteSonuc.hatalar, ...mutasyonSonuc.hatalar];

  if (tumHatalar.length === 0) {
    console.log(`✅ [mutasyon-kontrol] Tüm parite betikleri bağlı, ${mutantlar.length} mutant öldürüldü.`);
    process.exit(0);
  } else {
    console.error(`❌ [mutasyon-kontrol] MUTASYON VEYA PARİTE İHLALLERİ BULUNDU:`);
    for (const h of tumHatalar) {
      console.error(`  - ${h}`);
    }
    process.exit(1);
  }
}
