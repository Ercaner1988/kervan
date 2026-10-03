import os from "os";

// PRD FR-1.4: Makine sakin denetleyicisi
// Eşikler: boş RAM >= 6 GB, CPU <= %70 (yapılandırma / CLI bayrakları ile esnetilebilir)

interface Secenekler {
  minRamGb: number;
  maxCpuPct: number;
  sessiz: boolean;
}

function argumanlariAyristir(): Secenekler {
  const args = process.argv.slice(2);
  let minRamGb = 6.0;
  let maxCpuPct = 70.0;
  let sessiz = false;

  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--min-ram-gb" && i + 1 < args.length) {
      minRamGb = parseFloat(args[++i]);
    } else if (args[i] === "--max-cpu-pct" && i + 1 < args.length) {
      maxCpuPct = parseFloat(args[++i]);
    } else if (args[i] === "--sessiz") {
      sessiz = true;
    }
  }

  return { minRamGb, maxCpuPct, sessiz };
}

async function cpuYukuOlc(): Promise<number> {
  const cpus1 = os.cpus();
  await new Promise((r) => setTimeout(r, 400));
  const cpus2 = os.cpus();

  let idleDelta = 0;
  let totalDelta = 0;

  for (let i = 0; i < cpus1.length; i++) {
    const t1 = cpus1[i].times;
    const t2 = cpus2[i].times;

    const idle1 = t1.idle;
    const idle2 = t2.idle;

    const tot1 = t1.user + t1.nice + t1.sys + t1.idle + t1.irq;
    const tot2 = t2.user + t2.nice + t2.sys + t2.idle + t2.irq;

    idleDelta += idle2 - idle1;
    totalDelta += tot2 - tot1;
  }

  if (totalDelta <= 0) return 0;
  const load = 100 - (100 * idleDelta) / totalDelta;
  return Math.max(0, Math.min(100, Math.round(load * 10) / 10));
}

export async function makineDurumuKontrol(secenekler?: Partial<Secenekler>): Promise<{
  uygun: boolean;
  bosRamGb: number;
  toplamRamGb: number;
  cpuYuku: number;
  gerekce?: string;
}> {
  const opts = { ...argumanlariAyristir(), ...secenekler };

  const bosRamBytes = os.freemem();
  const toplamRamBytes = os.totalmem();
  const bosRamGb = Math.round((bosRamBytes / 1073741824) * 100) / 100;
  const toplamRamGb = Math.round((toplamRamBytes / 1073741824) * 100) / 100;

  const cpuYuku = await cpuYukuOlc();

  const gerekceler: string[] = [];
  if (bosRamGb < opts.minRamGb) {
    gerekceler.push(`Boş RAM yetersiz: ${bosRamGb} GB < ${opts.minRamGb} GB`);
  }
  if (cpuYuku > opts.maxCpuPct) {
    gerekceler.push(`CPU yükü yüksek: %${cpuYuku} > %${opts.maxCpuPct}`);
  }

  const uygun = gerekceler.length === 0;

  return {
    uygun,
    bosRamGb,
    toplamRamGb,
    cpuYuku,
    gerekce: gerekceler.length > 0 ? gerekceler.join("; ") : undefined,
  };
}

if (import.meta.main) {
  const opts = argumanlariAyristir();
  const sonuc = await makineDurumuKontrol(opts);

  if (!opts.sessiz) {
    console.log(`[makine-sakin] Boş RAM: ${sonuc.bosRamGb} GB / ${sonuc.toplamRamGb} GB (Eşik: >= ${opts.minRamGb} GB)`);
    console.log(`[makine-sakin] CPU Yükü: %${sonuc.cpuYuku} (Eşik: <= %${opts.maxCpuPct})`);
  }

  if (sonuc.uygun) {
    if (!opts.sessiz) console.log("✅ Makine sakin: Ağır adımlar için kaynaklar uygun.");
    process.exit(0);
  } else {
    console.error(`❌ Makine sakin değil: ${sonuc.gerekce}`);
    process.exit(1);
  }
}
