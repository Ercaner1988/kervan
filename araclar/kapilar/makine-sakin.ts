import os from "os";
import { KAPILAR_TOML, makineSakinAyari, tomlOku } from "./hepsi.ts";

// PRD FR-1.4: Makine sakin denetleyicisi.
// Eşikler YALNIZ araclar/kapilar/kapilar.toml [makine_sakin]'den okunur (mühürlü); komut satırından
// gevşetilemez. Öneri (O6): boş RAM >= 6 GB, ornek_sn (60) boyunca ortalama CPU <= %70.
// kip="uyari": eşik dışı UYARI + "HUKUM: yesil" (Ercan onayına kadar kapı sayılmaz, PRD §5).
// kip="zorunlu": eşik dışı "HUKUM: kirmizi". Ölçülemeyen eşik/ölçüm her kipte kırmızı.

export interface Secenekler {
  minRamGb: number;
  maxCpuPct: number;
  ornekMs?: number;
}

// Bun 1.4.2 / Windows: aynı süreçte os.cpus() sayaçları uykuda ilerlemiyor (ölçüldü: 1 sn arayla
// birebir aynı değer). Eski kod bu yüzden CPU'yu hep %0 okuyordu. Her örnek taze bir alt süreçte alınır.
function cpuZamanlari(): os.CpuInfo[] {
  const p = Bun.spawnSync([process.execPath, "-e", "console.log(JSON.stringify(require('os').cpus()))"]);
  if (p.exitCode !== 0) return [];
  try {
    return JSON.parse(p.stdout.toString());
  } catch {
    return [];
  }
}

async function cpuYukuOlc(ornekMs: number): Promise<number> {
  const cpus1 = cpuZamanlari();
  await new Promise((r) => setTimeout(r, ornekMs));
  const cpus2 = cpuZamanlari();
  if (!cpus1.length || cpus1.length !== cpus2.length) return NaN;

  let idleDelta = 0;
  let totalDelta = 0;
  for (let i = 0; i < cpus1.length; i++) {
    const t1 = cpus1[i].times;
    const t2 = cpus2[i].times;
    idleDelta += t2.idle - t1.idle;
    totalDelta += t2.user + t2.nice + t2.sys + t2.idle + t2.irq - (t1.user + t1.nice + t1.sys + t1.idle + t1.irq);
  }
  if (totalDelta <= 0) return NaN; // sayaç ilerlemedi: ölçüm yok
  const load = 100 - (100 * idleDelta) / totalDelta;
  return Math.max(0, Math.min(100, Math.round(load * 10) / 10));
}

export async function makineDurumuKontrol(secenekler: Secenekler): Promise<{
  uygun: boolean;
  bosRamGb: number;
  toplamRamGb: number;
  cpuYuku: number;
  olculemedi: boolean;
  gerekce?: string;
}> {
  const { minRamGb, maxCpuPct } = secenekler;
  const ornekMs = secenekler.ornekMs ?? 400;

  const bosRamGb = Math.round((os.freemem() / 1073741824) * 100) / 100;
  const toplamRamGb = Math.round((os.totalmem() / 1073741824) * 100) / 100;
  const cpuYuku = await cpuYukuOlc(ornekMs);

  const gerekceler: string[] = [];
  const esikGecersiz = !Number.isFinite(minRamGb) || !Number.isFinite(maxCpuPct);
  const olcumYok = !Number.isFinite(bosRamGb) || !Number.isFinite(cpuYuku);
  if (esikGecersiz) gerekceler.push(`Eşik ölçülemez: RAM=${minRamGb}, CPU=${maxCpuPct}`);
  if (olcumYok) gerekceler.push(`Ölçüm başarısız: RAM=${bosRamGb}, CPU=${cpuYuku}`);
  if (!(bosRamGb >= minRamGb)) gerekceler.push(`Boş RAM yetersiz: ${bosRamGb} GB < ${minRamGb} GB`);
  if (!(cpuYuku <= maxCpuPct)) gerekceler.push(`CPU yükü yüksek: %${cpuYuku} > %${maxCpuPct}`);

  return {
    uygun: gerekceler.length === 0,
    bosRamGb,
    toplamRamGb,
    cpuYuku,
    olculemedi: esikGecersiz || olcumYok,
    gerekce: gerekceler.length > 0 ? gerekceler.join("; ") : undefined,
  };
}

if (import.meta.main) {
  try {
    const args = process.argv.slice(2);
    if (args.length) throw new Error(`argüman kabul edilmez (eşikler ${KAPILAR_TOML}'dan okunur): ${args.join(" ")}`);
    const ayar = makineSakinAyari(tomlOku(KAPILAR_TOML));
    console.log(`[makine-sakin] Kip: ${ayar.kip}${ayar.kip === "uyari" ? " (eşikler Ercan onayı bekliyor; eşik dışı kapı sayılmaz)" : ""}`);
    console.log(`[makine-sakin] ${ayar.ornekSn} sn CPU örneği alınıyor...`);
    const sonuc = await makineDurumuKontrol({ minRamGb: ayar.minBosRamGb, maxCpuPct: ayar.maxCpuPct, ornekMs: ayar.ornekSn * 1000 });
    console.log(`[makine-sakin] Boş RAM: ${sonuc.bosRamGb} GB / ${sonuc.toplamRamGb} GB (Eşik: >= ${ayar.minBosRamGb} GB)`);
    console.log(`[makine-sakin] CPU Yükü (${ayar.ornekSn} sn ort.): %${sonuc.cpuYuku} (Eşik: <= %${ayar.maxCpuPct})`);

    if (sonuc.uygun) {
      console.log("Makine sakin: ağır adımlar için kaynaklar uygun.");
      console.log("HUKUM: yesil");
      process.exit(0);
    }
    if (ayar.kip === "uyari" && !sonuc.olculemedi) {
      console.log(`UYARI (kip=uyari, eşikler Ercan onayı bekliyor): makine sakin değil: ${sonuc.gerekce}`);
      console.log("HUKUM: yesil");
      process.exit(0);
    }
    console.error(`❌ Makine sakin değil: ${sonuc.gerekce}`);
    console.log(`HUKUM: kirmizi makine sakin değil: ${sonuc.gerekce}`);
    process.exit(1);
  } catch (e: any) {
    console.error(`❌ [makine-sakin] ${e?.message ?? e}`);
    console.log(`HUKUM: kirmizi makine-sakin ölçemedi: ${e?.message ?? e}`);
    process.exit(2);
  }
}
