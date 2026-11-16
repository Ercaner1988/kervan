/**
 * Kervan durum panosu — sunucuyu durdurur.
 *
 * `ac.ts` sunucuyu ayrı (detached) bir süreç olarak başlattığı için konsoldan
 * Ctrl+C ile durmaz. Bu betik portu dinleyen süreci bulup sonlandırır.
 *
 * Kullanım: bun araclar/pano/durdur.ts [--port 8790]
 */

import { spawnSync } from "node:child_process";

function portOku(): number {
  const i = process.argv.indexOf("--port");
  if (i >= 0) {
    const n = Number(process.argv[i + 1]);
    if (Number.isInteger(n) && n > 0 && n < 65536) return n;
  }
  return 8790;
}

const port = portOku();

// PowerShell: portu dinleyen süreç kimliğini ver.
const komut = `(Get-NetTCPConnection -LocalPort ${port} -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty OwningProcess)`;
const r = spawnSync("powershell", ["-NoProfile", "-NonInteractive", "-Command", komut], {
  encoding: "utf-8",
  windowsHide: true,
});

const pid = (r.stdout ?? "").trim();

if (!pid) {
  console.log(`Port ${port} dinlenmiyor — durdurulacak sunucu yok.`);
  process.exit(0);
}

const s = spawnSync("taskkill", ["/PID", pid, "/T", "/F"], { encoding: "utf-8", windowsHide: true });
if (s.status === 0) {
  console.log(`Pano sunucusu durduruldu (pid ${pid}, port ${port}).`);
} else {
  console.error(`Durdurulamadı (pid ${pid}): ${(s.stderr ?? s.stdout ?? "").trim()}`);
  process.exit(1);
}
