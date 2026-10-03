import { spawnSync } from "child_process";
import fs from "fs";
import path from "path";

// PRD FR-2.3: Uç Kaydı ve Test Kuralı (uc-kaydi.ts)
// 1. Kayıtlı her HTTP rotası port/uclar.toml içinde listelenmelidir.
// 2. 'gercek' ya da '501' olan her ucun testi cargo test listesinde bulunmalıdır.
// 3. 'gercek' olan ucların test kaynak kodunda 'nonce' kelimesi/mantığı bulunmalıdır.

export interface UcTanimi {
  yol: string;
  yontem: string;
  durum: "gercek" | "501" | "kapsam-disi";
  test?: string;
  testDosyasi?: string;
}

export function kayitliRotalariBul(kokDizin = process.cwd()): { yol: string; yontem: string }[] {
  const rotalar: { yol: string; yontem: string }[] = [];
  const serverPath = path.join(kokDizin, "crates", "sunucu", "src", "server.rs");
  if (!fs.existsSync(serverPath)) return rotalar;

  const icerik = fs.readFileSync(serverPath, "utf-8");
  const regex = /\.route\(\s*["']([^"']+)["']\s*,\s*(get|post|put|delete|patch)\(/g;
  let match;
  while ((match = regex.exec(icerik)) !== null) {
    rotalar.push({
      yol: match[1],
      yontem: match[2].toUpperCase(),
    });
  }
  return rotalar;
}

export function uclarTomlOku(dosyaYolu = path.join("port", "uclar.toml")): UcTanimi[] {
  if (!fs.existsSync(dosyaYolu)) return [];
  const icerik = fs.readFileSync(dosyaYolu, "utf-8");
  const uclar: UcTanimi[] = [];

  const bloklar = icerik.split(/\[\[uc\]\]/);
  for (const blok of bloklar) {
    const trimBlok = blok.trim();
    if (!trimBlok) continue;

    const yolMatch = trimBlok.match(/yol\s*=\s*["']([^"']+)["']/);
    const yontemMatch = trimBlok.match(/yontem\s*=\s*["']([^"']+)["']/);
    const durumMatch = trimBlok.match(/durum\s*=\s*["']([^"']+)["']/);
    const testMatch = trimBlok.match(/test\s*=\s*["']([^"']+)["']/);
    const testDosyaMatch = trimBlok.match(/testDosyasi\s*=\s*["']([^"']+)["']/);

    if (yolMatch && yontemMatch && durumMatch) {
      uclar.push({
        yol: yolMatch[1],
        yontem: yontemMatch[1].toUpperCase(),
        durum: durumMatch[1] as any,
        test: testMatch ? testMatch[1] : undefined,
        testDosyasi: testDosyaMatch ? testDosyaMatch[1] : undefined,
      });
    }
  }

  return uclar;
}

export function gercekCargoTestleriniAl(): Set<string> {
  const kimlikler = new Set<string>();
  try {
    const proc = spawnSync("cargo", ["test", "--workspace", "--all-targets", "--all-features", "--", "--list"], {
      encoding: "utf-8",
      maxBuffer: 10 * 1024 * 1024,
    });
    if (proc.stdout) {
      for (const line of proc.stdout.split(/\r?\n/)) {
        const match = line.match(/^([a-zA-Z0-9_:]+):\s*(test|benchmark)/);
        if (match) {
          kimlikler.add(match[1]);
        }
      }
    }
  } catch {}
  return kimlikler;
}

export function testDosyasindaNonceVarMi(testKimligi: string, testDosyaIpuclari?: string[]): boolean {
  // Test adına göre veya ipucuna göre dosya içeriğinde 'nonce' kelimesini ara
  const testDosyalari: string[] = testDosyaIpuclari || [
    path.join("crates", "sunucu", "src", "server.rs"),
    path.join("crates", "v1", "src", "sohbet.rs"),
    path.join("crates", "v1", "src", "lib.rs"),
    path.join("crates", "yukari", "src", "lib.rs"),
  ];

  for (const d of testDosyalari) {
    if (fs.existsSync(d)) {
      const icerik = fs.readFileSync(d, "utf-8");
      if (icerik.includes(testKimligi) || testKimligi.includes("nonce")) {
        if (icerik.toLowerCase().includes("nonce")) {
          return true;
        }
      }
    }
  }
  return testKimligi.toLowerCase().includes("nonce");
}

export function ucKaydiniDenetle(
  kokDizin = process.cwd(),
  uclarTomlYolu = path.join("port", "uclar.toml"),
  onbellekliTestler?: Set<string>
): {
  gecerli: boolean;
  kayitliRotaSayisi: number;
  tomlUclariSayisi: number;
  hatalar: string[];
} {
  const hatalar: string[] = [];
  const kodRotalari = kayitliRotalariBul(kokDizin);
  const tomlUclari = uclarTomlOku(uclarTomlYolu);

  // 1. Kodda olan ama toml'da olmayan rota kontrolü
  for (const r of kodRotalari) {
    const tomlVar = tomlUclari.find((u) => u.yol === r.yol && u.yontem === r.yontem);
    if (!tomlVar) {
      hatalar.push(`Kayıtlı rota port/uclar.toml dosyasında tanımlı değil: ${r.yontem} ${r.yol}`);
    }
  }

  // 2. Toml'daki uçların test kontrolü
  const cargoTestleri = onbellekliTestler || gercekCargoTestleriniAl();

  for (const uc of tomlUclari) {
    if (uc.durum === "gercek" || uc.durum === "501") {
      if (!uc.test) {
        hatalar.push(`Uç ${uc.yontem} ${uc.yol} (${uc.durum}) için 'test' kimliği zorunludur ancak belirtilmemiş`);
        continue;
      }

      if (!cargoTestleri.has(uc.test)) {
        hatalar.push(`Uç ${uc.yontem} ${uc.yol} için belirtilen test '${uc.test}' cargo test --list çıktısında bulunamadı`);
        continue;
      }

      // 3. 'gercek' uçlar için nonce kontrolü
      if (uc.durum === "gercek") {
        const nonceVar = testDosyasindaNonceVarMi(uc.test, uc.testDosyasi ? [uc.testDosyasi] : undefined);
        if (!nonceVar) {
          hatalar.push(`Uç ${uc.yontem} ${uc.yol} ('gercek') testi '${uc.test}' içinde 'nonce' doğrulaması bulunamadı`);
        }
      }
    }
  }

  return {
    gecerli: hatalar.length === 0,
    kayitliRotaSayisi: kodRotalari.length,
    tomlUclariSayisi: tomlUclari.length,
    hatalar,
  };
}

if (import.meta.main) {
  const sonuc = ucKaydiniDenetle();
  console.log(`[uc-kaydi] Kayıtlı HTTP rotası: ${sonuc.kayitliRotaSayisi}`);
  console.log(`[uc-kaydi] port/uclar.toml tanımı: ${sonuc.tomlUclariSayisi}`);

  if (sonuc.gecerli) {
    console.log("✅ [uc-kaydi] Tüm uçlar kayıtlı ve test kurallarına uygun.");
    process.exit(0);
  } else {
    console.error("❌ [uc-kaydi] UÇ KAYDI VE TEST KURALI İHLALLERİ BULUNDU:");
    for (const h of sonuc.hatalar) {
      console.error(`  - ${h}`);
    }
    process.exit(1);
  }
}
