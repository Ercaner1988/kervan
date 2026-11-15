/**
 * Kervan durum panosu — yerel sunucu.
 *
 * Yalnız 127.0.0.1 / localhost bağlantı kabul eder; dışarıya açılmaz.
 * Veri: port/pano/veri.json (olc-topla.ts üretir)
 * Statik: port/pano/index.html, pano.css, pano.js
 *
 * Kullanım: bun araclar/pano/sunucu.ts [--port 8790]
 */

import fs from "node:fs";
import path from "node:path";

const KOK = path.resolve(import.meta.dir, "..", "..");
const PANO = path.join(KOK, "port", "pano");
const VERI = path.join(PANO, "veri.json");

const IZINLI_HOST = new Set(["127.0.0.1", "localhost", "[::1]", "::1"]);

function portOku(): number {
  const i = process.argv.indexOf("--port");
  if (i >= 0) {
    const n = Number(process.argv[i + 1]);
    if (Number.isInteger(n) && n > 0 && n < 65536) return n;
  }
  return 8790;
}

const MIME: Record<string, string> = {
  ".html": "text/html; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".svg": "image/svg+xml",
  ".png": "image/png",
};

function dosyaSun(yol: string): Response {
  try {
    const govde = fs.readFileSync(yol);
    const tur = MIME[path.extname(yol).toLowerCase()] ?? "application/octet-stream";
    return new Response(govde, { headers: { "content-type": tur, "cache-control": "no-store" } });
  } catch {
    return new Response("bulunamadi", { status: 404 });
  }
}

const port = portOku();

const sunucu = Bun.serve({
  port,
  hostname: "127.0.0.1",
  fetch(istek) {
    const host = (istek.headers.get("host") ?? "").split(":")[0];
    if (host && !IZINLI_HOST.has(host)) {
      return new Response("yalniz yerel", { status: 403 });
    }

    const url = new URL(istek.url);
    const yol = url.pathname;

    if (yol === "/" || yol === "/index.html") return dosyaSun(path.join(PANO, "index.html"));
    if (yol === "/veri.json") return dosyaSun(VERI);
    if (yol === "/health") {
      return new Response(JSON.stringify({ durum: "ayakta", veri: fs.existsSync(VERI) }), {
        headers: { "content-type": "application/json; charset=utf-8" },
      });
    }

    // panic: panonun kendi dizini dışına çıkma girişimini engelle
    const hedef = path.join(PANO, path.normalize(yol).replace(/^([/\\])+/, ""));
    if (!hedef.startsWith(PANO)) return new Response("yasak", { status: 403 });

    return dosyaSun(hedef);
  },
});

console.log(`Kervan panosu: http://127.0.0.1:${port}/`);
if (!fs.existsSync(VERI)) {
  console.log("UYARI: port/pano/veri.json yok — once `bun araclar/pano/olc-topla.ts` kosun.");
}
