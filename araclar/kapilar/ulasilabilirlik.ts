import { spawnSync } from "child_process";
import fs from "fs";
import path from "path";
import { cargoOrtami } from "./kabul-haritasi.ts";

// PRD FR-2.1: Ulaşılabilirlik Kapısı (ulasilabilirlik.ts)
// Workspace içindeki her crate ya `kervan` ikilisinin geçişli bağımlısı olmalı,
// ya kendi [[bin]] hedefine sahip olmalı, ya da port/kutuphane-izni.toml'da GEREKÇELİ izinli olmalıdır.
// Graf varsayılan özelliklerle (`--all-features` YOK) alınır ve yalnız normal
// (dep_kinds.kind = null) kenarlar izlenir: dev/build bağımlılığı ikiliye girmez.
// Fail-closed: cargo metadata koşamazsa / ayrıştırılamazsa, `kervan` yoksa, izin dosyası bozuksa kırmızı.

export function kutuphaneIzinleriniOku(izinDosyasi = path.join("port", "kutuphane-izni.toml")): {
  izinler: Set<string>;
  hatalar: string[];
} {
  const izinler = new Set<string>();
  const hatalar: string[] = [];
  if (!fs.existsSync(izinDosyasi)) return { izinler, hatalar };

  let veri: any;
  try {
    veri = Bun.TOML.parse(fs.readFileSync(izinDosyasi, "utf-8"));
  } catch (e: any) {
    return { izinler, hatalar: [`${izinDosyasi} ayrıştırılamadı: ${e.message}`] };
  }
  const satirlar = veri?.izin;
  if (!Array.isArray(satirlar)) return { izinler, hatalar: [`${izinDosyasi} içinde [[izin]] listesi yok`] };
  for (const s of satirlar) {
    const ad = typeof s?.crate === "string" ? s.crate.trim() : "";
    if (!ad) {
      hatalar.push(`${izinDosyasi}: 'crate' alanı olmayan izin satırı`);
    } else if (typeof s.gerekce !== "string" || !s.gerekce.trim()) {
      hatalar.push(`${izinDosyasi}: '${ad}' izni gerekçesiz (gerekce zorunlu)`);
    } else {
      izinler.add(ad);
    }
  }
  return { izinler, hatalar };
}

export function ulasilabilirlikDenetle(
  dizin = process.cwd(),
  izinDosyasi = path.join(dizin, "port", "kutuphane-izni.toml")
): {
  uygun: boolean;
  ulasilamayanlar: string[];
  toplamWorkspaceCrateSayisi: number;
  ulasilanCrateSayisi: number;
  hatalar: string[];
} {
  const kirmizi = (hata: string) => ({ uygun: false, ulasilamayanlar: [], toplamWorkspaceCrateSayisi: 0, ulasilanCrateSayisi: 0, hatalar: [hata] });

  const metaProc = spawnSync("cargo", ["metadata", "--format-version", "1"], {
    cwd: dizin,
    encoding: "utf-8",
    maxBuffer: 64 * 1024 * 1024,
    env: cargoOrtami(),
    timeout: 10 * 60 * 1000,
  });
  if (metaProc.error || metaProc.status !== 0) {
    return kirmizi(`cargo metadata başarısız (çıkış ${metaProc.status ?? metaProc.error?.message}): ${(metaProc.stderr ?? "").trim().slice(-400)}`);
  }

  let meta: any;
  try {
    meta = JSON.parse(metaProc.stdout);
  } catch (e: any) {
    return kirmizi(`cargo metadata çıktısı ayrıştırılamadı: ${e.message}`);
  }
  if (!Array.isArray(meta?.workspace_members) || !Array.isArray(meta?.packages) || !Array.isArray(meta?.resolve?.nodes)) {
    return kirmizi("cargo metadata çıktısında workspace_members/packages/resolve.nodes yok");
  }

  const izin = kutuphaneIzinleriniOku(izinDosyasi);
  const hatalar = [...izin.hatalar];

  const workspacePkgIds = new Set<string>(meta.workspace_members);
  const workspacePaketleri = meta.packages.filter((p: any) => workspacePkgIds.has(p.id));
  if (workspacePaketleri.length === 0) return kirmizi("Workspace'te hiç crate yok");
  if (!workspacePaketleri.some((p: any) => p.name === "kervan")) hatalar.push("Workspace'te `kervan` paketi yok");

  // Yalnız normal kenarlar (kind = null). dep_kinds yoksa kenar türü bilinemez -> izlenmez (fail-closed).
  const normalKenarlar = new Map<string, string[]>();
  for (const node of meta.resolve.nodes) {
    const hedefler: string[] = [];
    for (const d of node.deps ?? []) {
      if (Array.isArray(d.dep_kinds) && d.dep_kinds.some((k: any) => k?.kind === null)) {
        hedefler.push(d.pkg);
      }
    }
    normalKenarlar.set(node.id, hedefler);
  }

  const ulasilabilenler = new Set<string>();
  const kuyruk: string[] = [];
  for (const pkg of workspacePaketleri) {
    const binVar = (pkg.targets ?? []).some((t: any) => Array.isArray(t.kind) && t.kind.includes("bin"));
    if (pkg.name === "kervan" || binVar) {
      ulasilabilenler.add(pkg.id);
      kuyruk.push(pkg.id);
    }
  }
  while (kuyruk.length > 0) {
    for (const depId of normalKenarlar.get(kuyruk.shift()!) ?? []) {
      if (!ulasilabilenler.has(depId)) {
        ulasilabilenler.add(depId);
        kuyruk.push(depId);
      }
    }
  }

  const ulasilamayanlar: string[] = [];
  for (const pkg of workspacePaketleri) {
    if (!ulasilabilenler.has(pkg.id) && !izin.izinler.has(pkg.name)) ulasilamayanlar.push(pkg.name);
  }
  for (const ad of ulasilamayanlar) hatalar.push(`'${ad}' hiçbir ikiliye normal bağımlılıkla bağlı değil ve gerekçeli izni yok`);

  return {
    uygun: hatalar.length === 0,
    ulasilamayanlar,
    toplamWorkspaceCrateSayisi: workspacePaketleri.length,
    ulasilanCrateSayisi: workspacePaketleri.length - ulasilamayanlar.length,
    hatalar,
  };
}

if (import.meta.main) {
  const sonuc = ulasilabilirlikDenetle();
  console.log(`[ulasilabilirlik] Toplam Workspace Crate: ${sonuc.toplamWorkspaceCrateSayisi}`);
  console.log(`[ulasilabilirlik] Ulaşılan / İzinli Crate: ${sonuc.ulasilanCrateSayisi}`);
  if (sonuc.uygun) {
    console.log("✅ [ulasilabilirlik] Tüm workspace crate'leri bir ikiliye bağlı veya gerekçeli izinlidir.");
    console.log("HUKUM: yesil");
    process.exit(0);
  }
  console.error("❌ [ulasilabilirlik] ULAŞILAMAZ / BAĞSIZ CRATE(LER) ya da DENETİM HATASI:");
  for (const h of sonuc.hatalar) console.error(`  - ${h}`);
  console.log(`HUKUM: kirmizi ${sonuc.hatalar[0]}`);
  process.exit(1);
}
