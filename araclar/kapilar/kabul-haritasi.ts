import { spawnSync } from "child_process";
import fs from "fs";
import path from "path";

// PRD FR-1.3: Kabul haritası denetleyicisi
// Her kabul maddesini >=1 gerçek test kimliğine bağlar; var olmayan veya boş kimlikte kırmızı üretir.

interface KabulMaddesi {
  id: string;
  metin?: string;
  clause?: string;
  kanit?: string[];
}

interface KabulDosyasi {
  faz: string;
  maddeler: KabulMaddesi[];
}

export function gercekTestKimlikleriniTopla(testListDosyasi?: string): Set<string> {
  const kimlikler = new Set<string>();

  if (testListDosyasi && fs.existsSync(testListDosyasi)) {
    const icerik = fs.readFileSync(testListDosyasi, "utf-8");
    for (const line of icerik.split(/\r?\n/)) {
      const match = line.match(/^([a-zA-Z0-9_:]+):\s*(test|benchmark)/);
      if (match) {
        kimlikler.add(match[1]);
      } else if (line.trim().length > 0 && !line.startsWith("Running") && !line.startsWith("Doc-tests")) {
        kimlikler.add(line.trim());
      }
    }
    return kimlikler;
  }

  // Cargo test listesini al
  try {
    const cargoProc = spawnSync("cargo", ["test", "--workspace", "--all-targets", "--all-features", "--", "--list"], {
      encoding: "utf-8",
      maxBuffer: 10 * 1024 * 1024,
    });
    if (cargoProc.stdout) {
      for (const line of cargoProc.stdout.split(/\r?\n/)) {
        const match = line.match(/^([a-zA-Z0-9_:]+):\s*(test|benchmark)/);
        if (match) {
          kimlikler.add(match[1]);
        }
      }
    }
  } catch (e) {
    console.warn("[kabul-haritasi] cargo test --list çalıştırılamadı:", e);
  }

  // Bun test listesini al (araclar/ dizininden)
  try {
    const bunProc = spawnSync("bun", ["test", "araclar/kapilar"], {
      encoding: "utf-8",
      maxBuffer: 5 * 1024 * 1024,
    });
    const cikti = (bunProc.stdout || "") + "\n" + (bunProc.stderr || "");
    for (const line of cikti.split(/\r?\n/)) {
      const match = line.match(/\((pass|fail)\)\s+(.+?)\s+\[/);
      if (match) {
        const fullName = match[2].trim();
        kimlikler.add(fullName);
        const parts = fullName.split(" > ");
        if (parts.length > 1) {
          kimlikler.add(parts[parts.length - 1].trim());
        }
      }
    }
  } catch {
    // Bun test olmayabilir
  }

  return kimlikler;
}

export function kabulHaritasiniDogrula(
  kabulDosyaYolu: string,
  testListDosyasi?: string
): {
  gecerli: boolean;
  faz: string;
  maddeSayisi: number;
  hatalar: string[];
} {
  const hatalar: string[] = [];

  if (!fs.existsSync(kabulDosyaYolu)) {
    return {
      gecerli: false,
      faz: "bilinmeyen",
      maddeSayisi: 0,
      hatalar: [`Kabul dosyası bulunamadı: ${kabulDosyaYolu}`],
    };
  }

  let veri: KabulDosyasi;
  try {
    veri = JSON.parse(fs.readFileSync(kabulDosyaYolu, "utf-8"));
  } catch (e: any) {
    return {
      gecerli: false,
      faz: "bilinmeyen",
      maddeSayisi: 0,
      hatalar: [`Kabul dosyası JSON olarak ayrıştırılamadı: ${e.message}`],
    };
  }

  if (!veri.maddeler || !Array.isArray(veri.maddeler) || veri.maddeler.length === 0) {
    return {
      gecerli: false,
      faz: veri.faz || "bilinmeyen",
      maddeSayisi: 0,
      hatalar: [`Kabul maddeleri listesi boş veya eksik`],
    };
  }

  const gercekTestler = gercekTestKimlikleriniTopla(testListDosyasi);

  for (const madde of veri.maddeler) {
    if (!madde.id) {
      hatalar.push(`Madde ID'si eksik`);
      continue;
    }
    if (!madde.kanit || !Array.isArray(madde.kanit) || madde.kanit.length === 0) {
      hatalar.push(`Madde ${madde.id} için test kimliği (kanit) belirtilmemiş`);
      continue;
    }

    for (const testId of madde.kanit) {
      if (!gercekTestler.has(testId)) {
        hatalar.push(`Madde ${madde.id}: '${testId}' kimlikli test gerçek testler arasında bulunamadı`);
      }
    }
  }

  return {
    gecerli: hatalar.length === 0,
    faz: veri.faz,
    maddeSayisi: veri.maddeler.length,
    hatalar,
  };
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  let dosyaYolu = "";
  let testListDosyasi: string | undefined;

  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--test-list-dosyasi" && i + 1 < args.length) {
      testListDosyasi = args[++i];
    } else if (!dosyaYolu) {
      dosyaYolu = args[i];
    }
  }

  if (!dosyaYolu) {
    console.error("Kullanım: bun araclar/kapilar/kabul-haritasi.ts <port/kabul/P1.json | P1> [--test-list-dosyasi <yol>]");
    process.exit(1);
  }

  if (!dosyaYolu.endsWith(".json")) {
    dosyaYolu = path.join("port", "kabul", `${dosyaYolu}.json`);
  }

  const sonuc = kabulHaritasiniDogrula(dosyaYolu, testListDosyasi);

  if (sonuc.gecerli) {
    console.log(`✅ [kabul-haritasi] Faz ${sonuc.faz}: ${sonuc.maddeSayisi} kabul maddesi gerçek testlerle doğrulandı.`);
    process.exit(0);
  } else {
    console.error(`❌ [kabul-haritasi] Faz ${sonuc.faz} kabul haritası doğrulanamadı:`);
    for (const h of sonuc.hatalar) {
      console.error(`  - ${h}`);
    }
    process.exit(1);
  }
}
