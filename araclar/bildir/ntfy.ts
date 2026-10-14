import fs from "fs";
import os from "os";
import path from "path";
import { spawn } from "child_process";

// Telefona ntfy bildirimi. Konu adı depoda DEĞİL: ~/.config/kervan/ntfy-konu (gizli, tahmin edilemez).
// Yalnız kısa durum cümleleri gider; sır, yol ya da kod koyma. ONAY ntfy'den VERİLMEZ: mühür onayı
// SSH imzalı etikettir (konuyu bilen herkes mesaj atabilir); bildirim yalnız "onay gerekli" der.
//
//   bun araclar/bildir/ntfy.ts "<başlık>" "<mesaj>" [öncelik 1-5]
//   bun araclar/bildir/ntfy.ts --kos <faz>    # hepsi.ts --faz <faz> koşar, bitince özetini bildirir

const KONU_YOLU = path.join(os.homedir(), ".config", "kervan", "ntfy-konu");
const SUNUCU = process.env.NTFY_SUNUCU ?? "https://ntfy.sh";

export async function bildir(baslik: string, mesaj: string, oncelik = 3): Promise<void> {
  if (!fs.existsSync(KONU_YOLU)) throw new Error(`ntfy konusu yok: ${KONU_YOLU}`);
  const topic = fs.readFileSync(KONU_YOLU, "utf-8").trim();
  // JSON yayın: başlık/mesajda UTF-8 (Türkçe) sorunsuz; başlık başlığı (HTTP header) kullanılmaz.
  const r = await fetch(SUNUCU, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ topic, title: baslik, message: mesaj, priority: oncelik }),
  });
  if (!r.ok) throw new Error(`ntfy yanıtı ${r.status}`);
}

/** hepsi.ts çıktısından tek satırlık özet: "9/11 kapı yeşil; kırmızı: a, b". */
export function ozetle(cikti: string): string {
  const kapi = [...cikti.matchAll(/^([✅❌]) \[([^\]]+)\]/gmu)];
  if (kapi.length === 0) return cikti.trim().split("\n").slice(-1)[0] ?? "çıktı yok";
  const kirmizi = kapi.filter((k) => k[1] === "❌").map((k) => k[2]);
  return `${kapi.length - kirmizi.length}/${kapi.length} kapı yeşil` + (kirmizi.length ? `; kırmızı: ${kirmizi.join(", ")}` : "");
}

if (import.meta.main) {
  const [a, ...r] = process.argv.slice(2);
  try {
    if (a === "--kos") {
      const faz = r[0];
      if (!faz) throw new Error("--kos <faz> ister");
      const kok = path.resolve(import.meta.dir, "..", "..");
      let cikti = "";
      const p = spawn("bun", ["araclar/kapilar/hepsi.ts", "--faz", faz], { cwd: kok, shell: true });
      p.stdout.on("data", (d) => ((cikti += d), process.stdout.write(d)));
      p.stderr.on("data", (d) => process.stderr.write(d));
      const kod: number = await new Promise((res) => p.on("close", (c) => res(c ?? 1)));
      const yesil = /^HUKUM: yesil/m.test(cikti);
      await bildir(`Kervan ${faz}: ${yesil ? "yeşil" : "kırmızı"}`, ozetle(cikti), yesil ? 3 : 4);
      process.exit(kod);
    }
    if (!a || !r[0]) throw new Error('kullanım: ntfy.ts "<başlık>" "<mesaj>" [öncelik] | --kos <faz>');
    await bildir(a, r[0], r[1] ? Number(r[1]) : 3);
    console.log("[ntfy] gönderildi");
  } catch (e: any) {
    console.error(`[ntfy] ${e?.message ?? e}`);
    process.exit(1);
  }
}
