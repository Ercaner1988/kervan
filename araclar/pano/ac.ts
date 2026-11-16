/**
 * Kervan durum panosu — tek tık açıcı.
 *
 * Masaüstündeki kısayol bunu çağırır. Yapar:
 *   1. Pano sunucusu ayakta mı diye `/health` ile bakar.
 *   2. Ayakta değilse `sunucu.ts`'i arka planda (gizli pencere) başlatır.
 *   3. Sağlık kapısı yeşile dönene kadar bekler (en çok 10 sn).
 *   4. Varsayılan tarayıcıda panoyu açar.
 *
 * Sunucu zaten ayaktaysa ikinci kez başlatmaz.
 * Durdurmak için: `bun araclar/pano/durdur.ts` ya da görev yöneticisinden `bun`.
 *
 * Kullanım: bun araclar/pano/ac.ts [--port 8790]
 */

import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

const KOK = path.resolve(import.meta.dir, "..", "..");
const PID_DOSYASI = path.join(KOK, "port", "pano", ".sunucu.pid");
const PORT_VARSAYILAN = 8790;

function portOku(): number {
  const i = process.argv.indexOf("--port");
  if (i >= 0) {
    const n = Number(process.argv[i + 1]);
    if (Number.isInteger(n) && n > 0 && n < 65536) return n;
  }
  return PORT_VARSAYILAN;
}

const port = portOku();
const adres = `http://127.0.0.1:${port}/`;

async function ayaktaMi(): Promise<boolean> {
  try {
    const y = await fetch(`http://127.0.0.1:${port}/health`, {
      signal: AbortSignal.timeout(600),
    });
    if (!y.ok) return false;
    const govde = (await y.json()) as { durum?: string };
    return govde.durum === "ayakta";
  } catch {
    return false;
  }
}

function sunucuyuBaslat(): void {
  const cocuk = spawn(process.execPath, [path.join(KOK, "araclar", "pano", "sunucu.ts"), "--port", String(port)], {
    cwd: KOK,
    detached: true,
    stdio: "ignore",
    windowsHide: true,
  });
  if (cocuk.pid) {
    try {
      fs.writeFileSync(PID_DOSYASI, String(cocuk.pid), "utf-8");
    } catch {
      // pid dosyası yazılamazsa durdur.ts portu tarayarak bulur.
    }
  }
  cocuk.unref();
}

async function bekle(sinirMs = 10_000): Promise<boolean> {
  const bas = Date.now();
  while (Date.now() - bas < sinirMs) {
    if (await ayaktaMi()) return true;
    await Bun.sleep(200);
  }
  return false;
}

if (await ayaktaMi()) {
  console.log(`Pano zaten ayakta: ${adres}`);
} else {
  sunucuyuBaslat();
  if (await bekle()) {
    console.log(`Pano başladı: ${adres}`);
  } else {
    console.error(`Pano başlamadı. Elle deneyin: bun araclar/pano/sunucu.ts --port ${port}`);
    process.exit(1);
  }
}

// `explorer.exe <url>` varsayılan tarayıcıyı açar ve cmd.exe gerektirmez.
spawn("explorer.exe", [adres], { detached: true, stdio: "ignore" }).unref();
