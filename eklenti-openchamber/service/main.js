// Kervan OpenChamber uzantısı — yerel servis (Node shim).
//
// Görev: panelin ihtiyaç duyduğu Kervan uçlarını 127.0.0.1:20129 üzerinden
// proxy'lemek ve Kervan kapalıyken "Başlat" düğmesini karşılamak. Domain
// mantığı burada DEĞİL, Kervan'da kalır.
//
// OpenChamber bu süreci kendi Node'u ile başlatır ve iki ortam değişkeni verir:
//   OPENCHAMBER_SERVICE_PORT  — dinlenecek loopback portu
//   OPENCHAMBER_SERVICE_TOKEN — her istekte zorunlu Bearer token
// Sağlık denetimi GET /health + Authorization başlığıyla yapılır.

import http from "node:http";
import path from "node:path";
import fs from "node:fs";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";

const port = Number(process.env.OPENCHAMBER_SERVICE_PORT);
const token = process.env.OPENCHAMBER_SERVICE_TOKEN ?? "";
if (!port || !token) {
  console.error("OPENCHAMBER_SERVICE_PORT ve OPENCHAMBER_SERVICE_TOKEN zorunlu");
  process.exit(1);
}

const PACKAGE_ROOT = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const REPO_ROOT = path.resolve(PACKAGE_ROOT, "..");
const KERVAN_URL = (process.env.KERVAN_URL ?? "http://127.0.0.1:20129").replace(/\/+$/, "");
const GET_TIMEOUT_MS = 2500;
const START_TIMEOUT_MS = 20_000;

const json = (res, status, body) => {
  res.writeHead(status, {
    "Content-Type": "application/json; charset=utf-8",
    "Cache-Control": "no-store",
  });
  res.end(JSON.stringify(body));
};

const okuGovde = (req, sinir = 64 * 1024) =>
  new Promise((resolve, reject) => {
    let toplam = 0;
    const parcalar = [];
    req.on("data", (parca) => {
      toplam += parca.length;
      if (toplam > sinir) {
        reject(new Error("gövde çok büyük"));
        req.destroy();
        return;
      }
      parcalar.push(parca);
    });
    req.on("end", () => {
      const metin = Buffer.concat(parcalar).toString("utf8").trim();
      if (!metin) return resolve({});
      try {
        resolve(JSON.parse(metin));
      } catch {
        reject(new Error("geçersiz JSON"));
      }
    });
    req.on("error", reject);
  });

/** Kervan'a istek atar; Kervan kapalıysa `null` döner (panel bunu durum olarak gösterir). */
const kervan = async (yol, { method = "GET", body } = {}) => {
  try {
    const yanit = await fetch(`${KERVAN_URL}${yol}`, {
      method,
      headers: body === undefined ? {} : { "Content-Type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
      signal: AbortSignal.timeout(GET_TIMEOUT_MS),
    });
    const metin = await yanit.text();
    let veri = null;
    try {
      veri = metin ? JSON.parse(metin) : null;
    } catch {
      veri = metin;
    }
    return { durum: yanit.status, veri };
  } catch {
    return null;
  }
};

/**
 * Uzun süren Kervan çağrıları için aynı yardımcı, geniş zaman aşımıyla.
 *
 * NEDEN AYRI: `/hesap/dogrula` geckodriver'ı ayağa kaldırır, tarayıcıyı açar
 * ve sayfanın yerleşmesini bekler — 2,5 sn'lik genel sınır bunu keser ve panel
 * "Kervan kapalı" sanır. Süre açıkça verilir ki hangi ucun ne kadar beklediği
 * kodda görünsün.
 */
const kervanUzun = async (yol, { method = "POST", body, ms = 180_000 } = {}) => {
  try {
    const yanit = await fetch(`${KERVAN_URL}${yol}`, {
      method,
      headers: body === undefined ? {} : { "Content-Type": "application/json" },
      body: body === undefined ? undefined : JSON.stringify(body),
      signal: AbortSignal.timeout(ms),
    });
    const metin = await yanit.text();
    let veri = null;
    try {
      veri = metin ? JSON.parse(metin) : null;
    } catch {
      veri = metin;
    }
    return { durum: yanit.status, veri };
  } catch {
    return null;
  }
};

const kervanAcikMi = async () => {
  try {
    const yanit = await fetch(`${KERVAN_URL}/health`, {
      signal: AbortSignal.timeout(1200),
    });
    return yanit.ok;
  } catch {
    return false;
  }
};

/** Kervan ikilisini bulur: ortam değişkeni → depodaki en yeni hedef → PATH. */
const ikiliAdaylari = () => {
  const derlenmis = [
    path.join(REPO_ROOT, "target", "release", "kervan.exe"),
    path.join(REPO_ROOT, "target", "debug", "kervan.exe"),
    path.join(REPO_ROOT, "target", "release", "kervan"),
    path.join(REPO_ROOT, "target", "debug", "kervan"),
  ]
    .filter((aday) => fs.existsSync(aday))
    .map((aday) => ({ aday, zaman: fs.statSync(aday).mtimeMs }))
    // Bayat release, taze debug'ın önüne geçmesin: en son derlenen kazanır.
    .sort((a, b) => b.zaman - a.zaman)
    .map((kayit) => kayit.aday);

  const adaylar = [process.env.KERVAN_BIN, ...derlenmis, "kervan.exe", "kervan"].filter(Boolean);
  const gorulen = new Set();
  return adaylar.filter((aday) => {
    if (gorulen.has(aday)) return false;
    gorulen.add(aday);
    // Yol içeren aday gerçekten var olmalı; çıplak komut PATH'te aranır.
    return !aday.includes(path.sep) || fs.existsSync(aday);
  });
};

const baslat = async () => {
  if (await kervanAcikMi()) return { ok: true, zaten: true, mesaj: "Kervan zaten açık." };

  const cfg = path.join(REPO_ROOT, "kervan.toml");
  const ortam = { ...process.env };
  if (fs.existsSync(cfg)) ortam.KERVAN_CONFIG = cfg;

  const hatalar = [];
  for (const ikili of ikiliAdaylari()) {
    try {
      await new Promise((resolve, reject) => {
        const cocuk = spawn(ikili, ["--serve"], {
          cwd: REPO_ROOT,
          env: ortam,
          detached: true,
          stdio: "ignore",
          windowsHide: true,
        });
        cocuk.once("error", reject);
        cocuk.once("spawn", () => {
          cocuk.unref();
          resolve();
        });
      });
    } catch (hata) {
      hatalar.push(`${ikili}: ${hata.message}`);
      continue;
    }

    const bitis = Date.now() + START_TIMEOUT_MS;
    while (Date.now() < bitis) {
      await new Promise((r) => setTimeout(r, 400));
      if (await kervanAcikMi()) return { ok: true, mesaj: `Kervan başlatıldı (${ikili}).` };
    }
    return {
      ok: false,
      mesaj: `${ikili} başlatıldı ama /health ${START_TIMEOUT_MS / 1000} sn içinde yanıt vermedi. Kervan'ın günlüğüne bakın.`,
    };
  }

  return {
    ok: false,
    mesaj:
      "Kervan ikilisi bulunamadı. KERVAN_BIN ortam değişkeniyle tam yolu verin " +
      "(ör. C:\\...\\kervan\\target\\debug\\kervan.exe).",
    denenenler: hatalar,
  };
};

/**
 * Yapılandırma değiştikten sonra havuzu tazelemenin tek yolu yeniden başlatma:
 * havuz açılışta kurulur ve çalışırken yuva eklemez. Kervan zarifçe kapanır
 * (`/kapat`), port boşalınca aynı `baslat` yolu devreye girer.
 */
const yenidenBaslat = async () => {
  if (!(await kervanAcikMi())) {
    return { ...(await baslat()), zaten: undefined };
  }
  const kapatma = await kervanUzun("/kapat", { ms: 15_000 });
  if (!kapatma) {
    return { ok: false, mesaj: "Kervan'a kapanma isteği ulaşmadı." };
  }
  const bitis = Date.now() + 15_000;
  while (Date.now() < bitis) {
    await new Promise((r) => setTimeout(r, 300));
    if (!(await kervanAcikMi())) break;
  }
  if (await kervanAcikMi()) {
    // Sessizce "oldu" demek yerine açık hata: kilitli süreç elle kapatılmalı.
    return {
      ok: false,
      mesaj:
        "Kervan 15 sn içinde kapanmadı (açık istek ya da ikili kilidi olabilir). " +
        "Kervan'ı elle kapatıp yeniden başlatın.",
    };
  }
  return await baslat();
};

const durum = async () => {
  const acik = await kervanAcikMi();
  if (!acik) {
    return { kervan: false, mesaj: "Kervan kapalı." };
  }
  const [accounts, yonlendirme, eklenti, telemetri, kota, bakiye] = await Promise.all([
    kervan("/accounts"),
    kervan("/yonlendirme"),
    kervan("/eklenti/sekmeler"),
    kervan("/telemetri"),
    kervan("/kota"),
    kervan("/bakiye"),
  ]);
  return {
    kervan: true,
    accounts: accounts?.veri ?? null,
    // Panelin hesap ekleme secicisi icin tanimli saglayici adlari.
    saglayicilar: accounts?.veri?.saglayicilar ?? null,
    yonlendirme: yonlendirme?.veri ?? null,
    eklenti: eklenti?.veri ?? null,
    telemetri: telemetri?.veri ?? null,
    kota: kota?.veri ?? null,
    bakiye: bakiye?.veri ?? null,
  };
};

const sunucu = http.createServer(async (req, res) => {
  if (req.headers.authorization !== `Bearer ${token}`) {
    json(res, 401, { hata: "yetkisiz" });
    return;
  }

  const url = new URL(req.url ?? "/", "http://127.0.0.1");

  if (req.method === "GET" && url.pathname === "/health") {
    json(res, 200, { ok: true });
    return;
  }

  if (req.method === "GET" && url.pathname === "/durum") {
    json(res, 200, await durum());
    return;
  }

  if (req.method === "POST" && url.pathname === "/baslat") {
    json(res, 200, await baslat());
    return;
  }

  if (req.method === "POST" && url.pathname === "/yonlendirme") {
    try {
      const govde = await okuGovde(req);
      const sonuc = await kervan("/yonlendirme", { method: "POST", body: govde });
      if (!sonuc) {
        json(res, 503, { hata: "Kervan kapalı." });
        return;
      }
      json(res, sonuc.durum, sonuc.veri);
    } catch (hata) {
      json(res, 400, { hata: hata.message });
    }
    return;
  }

  if (
    req.method === "POST" &&
    (url.pathname === "/accounts/duraklat" || url.pathname === "/accounts/devam")
  ) {
    try {
      const govde = await okuGovde(req);
      const index = Number(govde.index);
      if (!Number.isInteger(index) || index < 0) {
        json(res, 400, { hata: "index negatif olmayan bir tamsayı olmalı" });
        return;
      }
      const eylem = url.pathname.endsWith("duraklat") ? "duraklat" : "devam";
      const sonuc = await kervan(`/accounts/${index}/${eylem}`, { method: "POST" });
      if (!sonuc) {
        json(res, 503, { hata: "Kervan kapalı." });
        return;
      }
      json(res, sonuc.durum, sonuc.veri);
    } catch (hata) {
      json(res, 400, { hata: hata.message });
    }
    return;
  }

  // Panelden hesap ekleme: otomasyonsuz giriş penceresi + giriş doğrulama.
  // Domain mantığı Kervan'da; burada yalnız iki uç proxy'lenir.
  if (
    req.method === "POST" &&
    (url.pathname === "/hesap/ac" || url.pathname === "/hesap/dogrula")
  ) {
    try {
      const govde = await okuGovde(req);
      // Doğrulama geckodriver'ı ayağa kaldırıp sayfa yerleşmesini bekler;
      // 2,5 sn'lik genel zaman aşımı burada yetmez.
      const sonuc = await kervanUzun(url.pathname, { method: "POST", body: govde });
      if (!sonuc) {
        json(res, 503, { hata: "Kervan kapalı." });
        return;
      }
      json(res, sonuc.durum, sonuc.veri);
    } catch (hata) {
      json(res, 400, { hata: hata.message });
    }
    return;
  }

  // Yapılandırma değiştikten sonra havuzu tazeleme: kapat → yeniden başlat.
  if (req.method === "POST" && url.pathname === "/yeniden-baslat") {
    try {
      json(res, 200, await yenidenBaslat());
    } catch (hata) {
      json(res, 500, { hata: hata.message });
    }
    return;
  }

  json(res, 404, { hata: "bilinmeyen yol" });
});

const kapat = () => {
  sunucu.close();
  process.exit(0);
};
process.on("SIGTERM", kapat);
process.on("SIGINT", kapat);

sunucu.listen(port, "127.0.0.1", () => {
  console.log(`kervan-panel servisi 127.0.0.1:${port} üzerinde hazır (Kervan: ${KERVAN_URL})`);
});
