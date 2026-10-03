import { spawnSync } from "child_process";
import fs from "fs";
import path from "path";

// PRD FR-2.1: Ulaşılabilirlik Kapısı (ulasilabilirlik.ts)
// Workspace içindeki her crate ya `kervan` ikilisinin geçişli bağımlısı olmalı,
// ya kendi [[bin]] hedefine sahip olmalı, ya da port/kutuphane-izni.toml'da izinli olmalıdır.

interface IzinKaydi {
  crate: string;
  gerekce: string;
}

export function kutuphaneIzinleriniOku(izinDosyasi = path.join("port", "kutuphane-izni.toml")): Set<string> {
  const izinler = new Set<string>();
  if (!fs.existsSync(izinDosyasi)) return izinler;

  const icerik = fs.readFileSync(izinDosyasi, "utf-8");
  // Basit toml parser (bağımlılık olmadan [[izin]] bloklarını okur)
  const regex = /crate\s*=\s*["']([^"']+)["']/g;
  let match;
  while ((match = regex.exec(icerik)) !== null) {
    izinler.add(match[1]);
  }
  return izinler;
}

export function ulasilabilirlikDenetle(dizin = process.cwd(), izinDosyasi?: string): {
  uygun: boolean;
  ulasilamayanlar: string[];
  toplamWorkspaceCrateSayisi: number;
  ulasilanCrateSayisi: number;
} {
  const metaProc = spawnSync("cargo", ["metadata", "--all-features", "--format-version", "1"], {
    cwd: dizin,
    encoding: "utf-8",
    maxBuffer: 20 * 1024 * 1024,
  });

  if (metaProc.status !== 0) {
    throw new Error(`cargo metadata başarısız oldu: ${metaProc.stderr}`);
  }

  const meta = JSON.parse(metaProc.stdout);
  const izinliKutuphaneler = kutuphaneIzinleriniOku(izinDosyasi);

  // 1. Workspace paketlerini topla
  const workspacePkgIds = new Set<string>(meta.workspace_members);
  const workspacePaketleri = meta.packages.filter((p: any) => workspacePkgIds.has(p.id));

  // 2. Kendi ikilisi (bin) olan paketleri bul
  const kendiIkiliPaketleri = new Set<string>();
  for (const pkg of workspacePaketleri) {
    const hasBin = pkg.targets && pkg.targets.some((t: any) => t.kind && t.kind.includes("bin"));
    if (hasBin) {
      kendiIkiliPaketleri.add(pkg.id);
    }
  }

  // 3. Resolve grafında geçişli bağımlılıkları hesapla
  const resolveNodes = new Map<string, string[]>();
  for (const node of meta.resolve.nodes) {
    resolveNodes.set(node.id, node.dependencies || []);
  }

  // Root paket `kervan` ve kendi bin'i olan paketlerden başla
  const ulasilabilenler = new Set<string>();
  const kuyruk: string[] = [];

  for (const pkg of workspacePaketleri) {
    if (pkg.name === "kervan" || kendiIkiliPaketleri.has(pkg.id)) {
      ulasilabilenler.add(pkg.id);
      kuyruk.push(pkg.id);
    }
  }

  while (kuyruk.length > 0) {
    const simdiki = kuyruk.shift()!;
    const deps = resolveNodes.get(simdiki) || [];
    for (const depId of deps) {
      if (!ulasilabilenler.has(depId)) {
        ulasilabilenler.add(depId);
        kuyruk.push(depId);
      }
    }
  }

  // 4. Ulaşılamayan workspace crate'lerini belirle
  const ulasilamayanlar: string[] = [];
  for (const pkg of workspacePaketleri) {
    if (!ulasilabilenler.has(pkg.id) && !izinliKutuphaneler.has(pkg.name)) {
      ulasilamayanlar.push(pkg.name);
    }
  }

  return {
    uygun: ulasilamayanlar.length === 0,
    ulasilamayanlar,
    toplamWorkspaceCrateSayisi: workspacePaketleri.length,
    ulasilanCrateSayisi: workspacePaketleri.length - ulasilamayanlar.length,
  };
}

if (import.meta.main) {
  try {
    const sonuc = ulasilabilirlikDenetle();
    console.log(`[ulasilabilirlik] Toplam Workspace Crate: ${sonuc.toplamWorkspaceCrateSayisi}`);
    console.log(`[ulasilabilirlik] Ulaşılan / İzinli Crate: ${sonuc.ulasilanCrateSayisi}`);

    if (sonuc.uygun) {
      console.log("✅ [ulasilabilirlik] Tüm workspace crate'leri bir ikiliye bağlı veya izinlidir.");
      process.exit(0);
    } else {
      console.error("❌ [ulasilabilirlik] ULAŞILAMAZ / BAĞSIZ CRATE(LER) TESPİT EDİLDİ:");
      for (const ad of sonuc.ulasilamayanlar) {
        console.error(`  - ${ad}`);
      }
      process.exit(1);
    }
  } catch (err: any) {
    console.error(`❌ [ulasilabilirlik] Hata: ${err.message}`);
    process.exit(2);
  }
}
