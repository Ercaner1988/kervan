import { spawn } from "child_process";
import fs from "fs";
import path from "path";

// PRD FR-1.1: Kapı Çalıştırıcısı (hepsi.ts)
// Kayıtlı kapıları sırayla koşar, ham çıktıları port/kayit/<faz>/<kapi>.log'a yazar,
// sonucu port/durum.json'a kaydeder. Bir kapı çökerse, zaman aşarsa veya hata verirse çıkış != 0.

export interface KapiGorevi {
  ad: string;
  komut: string;
  argumanlar?: string[];
  timeoutMs?: number;
}

export interface KapiSonucu {
  ad: string;
  komut: string;
  cikis: number;
  sure_ms: number;
  log: string;
  zaman: string;
  hata?: string;
}

export interface DurumVerisi {
  uretici_ozeti: string;
  uretim_zamani: string;
  faz: string;
  kapilar: KapiSonucu[];
  tum_kapilar_yesil_mi: boolean;
}

export async function tekKapiKos(
  gorev: KapiGorevi,
  logYolu: string,
  varsayilanTimeoutMs = 60000
): Promise<KapiSonucu> {
  const baslangic = Date.now();
  const timeoutMs = gorev.timeoutMs || varsayilanTimeoutMs;

  const logDir = path.dirname(logYolu);
  if (!fs.existsSync(logDir)) {
    fs.mkdirSync(logDir, { recursive: true });
  }

  const logStream = fs.createWriteStream(logYolu, { encoding: "utf-8" });

  logStream.write(`=== KAPI BAŞLADI: ${gorev.ad} ===\n`);
  logStream.write(`Komut: ${gorev.komut} ${(gorev.argumanlar || []).join(" ")}\n`);
  logStream.write(`Zaman: ${new Date().toISOString()}\n\n`);

  return new Promise<KapiSonucu>((resolve) => {
    let zamanAsimiOldu = false;
    const proc = spawn(gorev.komut, gorev.argumanlar || [], {
      shell: true,
      stdio: ["ignore", "pipe", "pipe"],
    });

    const timer = setTimeout(() => {
      zamanAsimiOldu = true;
      logStream.write(`\n❌ [ZAMAN AŞIMI]: Kapı ${timeoutMs} ms sınırını aştı!\n`);
      try {
        proc.kill("SIGKILL");
      } catch {}
    }, timeoutMs);

    proc.stdout?.on("data", (data) => {
      logStream.write(data);
    });

    proc.stderr?.on("data", (data) => {
      logStream.write(data);
    });

    proc.on("error", (err) => {
      clearTimeout(timer);
      const sure = Date.now() - baslangic;
      logStream.write(`\n❌ [HATA]: Süreç başlatılamadı: ${err.message}\n`);
      logStream.end();
      resolve({
        ad: gorev.ad,
        komut: `${gorev.komut} ${(gorev.argumanlar || []).join(" ")}`,
        cikis: 127,
        sure_ms: sure,
        log: logYolu,
        zaman: new Date().toISOString(),
        hata: err.message,
      });
    });

    proc.on("close", (code) => {
      clearTimeout(timer);
      const sure = Date.now() - baslangic;
      const cikis = zamanAsimiOldu ? 124 : code ?? 1;

      logStream.write(`\n=== KAPI BİTTİ: Çıkış Kodu = ${cikis} (Süre: ${sure} ms) ===\n`);
      logStream.end();

      resolve({
        ad: gorev.ad,
        komut: `${gorev.komut} ${(gorev.argumanlar || []).join(" ")}`,
        cikis,
        sure_ms: sure,
        log: logYolu,
        zaman: new Date().toISOString(),
        hata: zamanAsimiOldu ? "Zaman aşımı" : cikis !== 0 ? `Çıkış kodu: ${cikis}` : undefined,
      });
    });
  });
}

export async function kapilariKos(
  kapilar: KapiGorevi[],
  faz = "P1",
  kayitDizini = path.join("port", "kayit", faz),
  durumDosyasi = path.join("port", "durum.json")
): Promise<{ sonuclar: KapiSonucu[]; hepsiYesil: boolean }> {
  const sonuclar: KapiSonucu[] = [];

  for (const kapi of kapilar) {
    const logDosyasi = path.join(kayitDizini, `${kapi.ad}.log`);
    const sonuc = await tekKapiKos(kapi, logDosyasi);
    sonuclar.push(sonuc);
  }

  const hepsiYesil = sonuclar.every((s) => s.cikis === 0);

  const durum: DurumVerisi = {
    uretici_ozeti: "araclar/kapilar/hepsi.ts",
    uretim_zamani: new Date().toISOString(),
    faz,
    kapilar: sonuclar,
    tum_kapilar_yesil_mi: hepsiYesil,
  };

  const durumDir = path.dirname(durumDosyasi);
  if (!fs.existsSync(durumDir)) {
    fs.mkdirSync(durumDir, { recursive: true });
  }
  fs.writeFileSync(durumDosyasi, JSON.stringify(durum, null, 2), "utf-8");

  return { sonuclar, hepsiYesil };
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  let faz = "P1";
  let gorevler: KapiGorevi[] = [];

  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--faz" && i + 1 < args.length) {
      faz = args[++i];
    } else if (args[i] === "--kapi" && i + 1 < args.length) {
      const ad = args[++i];
      const komut = args[++i] || `bun araclar/kapilar/${ad}.ts`;
      gorevler.push({ ad, komut });
    }
  }

  // Varsayılan faz kapıları
  if (gorevler.length === 0) {
    if (faz === "P1") {
      gorevler = [
        { ad: "makine-sakin", komut: "bun araclar/kapilar/makine-sakin.ts --min-ram-gb 0.1 --max-cpu-pct 100" },
        { ad: "kabul-haritasi", komut: "bun araclar/kapilar/kabul-haritasi.ts P1" },
      ];
    } else if (faz === "P2") {
      gorevler = [
        { ad: "makine-sakin", komut: "bun araclar/kapilar/makine-sakin.ts --min-ram-gb 0.1 --max-cpu-pct 100" },
        { ad: "kabul-haritasi", komut: "bun araclar/kapilar/kabul-haritasi.ts P2" },
        { ad: "ulasilabilirlik", komut: "bun araclar/kapilar/ulasilabilirlik.ts" },
        { ad: "sabit-yanit", komut: "bun araclar/kapilar/sabit-yanit.ts" },
        { ad: "uc-kaydi", komut: "bun araclar/kapilar/uc-kaydi.ts" },
        { ad: "muhur-kontrol", komut: "bun araclar/kapilar/muhur-kontrol.ts --faz P2" },
        { ad: "rapor-kontrol", komut: "bun araclar/kapilar/rapor-kontrol.ts P1" },
      ];
    } else if (faz === "P3") {
      gorevler = [
        { ad: "makine-sakin", komut: "bun araclar/kapilar/makine-sakin.ts --min-ram-gb 0.1 --max-cpu-pct 100" },
        { ad: "kabul-haritasi", komut: "bun araclar/kapilar/kabul-haritasi.ts P3" },
        { ad: "ulasilabilirlik", komut: "bun araclar/kapilar/ulasilabilirlik.ts" },
        { ad: "sabit-yanit", komut: "bun araclar/kapilar/sabit-yanit.ts" },
        { ad: "uc-kaydi", komut: "bun araclar/kapilar/uc-kaydi.ts" },
        { ad: "muhur-kontrol", komut: "bun araclar/kapilar/muhur-kontrol.ts --faz P3" },
        { ad: "rapor-kontrol", komut: "bun araclar/kapilar/rapor-kontrol.ts P2" },
        { ad: "kulliyat-tara", komut: "bun araclar/kapilar/kulliyat-tara.ts omniroute-aktarim" },
        { ad: "mutasyon-kontrol", komut: "bun araclar/kapilar/mutasyon-kontrol.ts" },
        { ad: "durum-kontrol", komut: "bun araclar/kapilar/durum-kontrol.ts" },
        { ad: "defter-kontrol", komut: "bun araclar/kapilar/defter-kontrol.ts" },
      ];
    }
  }

  console.log(`[hepsi.ts] Faz '${faz}' için ${gorevler.length} kapı çalıştırılıyor...`);
  const { sonuclar, hepsiYesil } = await kapilariKos(gorevler, faz);

  for (const s of sonuclar) {
    const icon = s.cikis === 0 ? "✅" : "❌";
    console.log(`${icon} [${s.ad}] Çıkış: ${s.cikis} | Süre: ${s.sure_ms} ms | Log: ${s.log}`);
    if (s.hata) {
      console.log(`   Hata detayı: ${s.hata}`);
    }
  }

  if (hepsiYesil) {
    console.log(`\n🎉 Tüm kapılar başarıyla geçti (yeşil).`);
    process.exit(0);
  } else {
    console.error(`\n❌ Bazı kapılar başarısız oldu (kırmızı).`);
    process.exit(1);
  }
}
