// Yeniden bağlanma testleri (2026-10-24 saha arızası).
//
// NEDEN: sunucu yeniden başlatıldığında AÇIK tarayıcı köprüye dönmüyordu ve
// hesap panelden kayboluyordu. İki ayrı kök neden ölçüldü:
//
// 1. Yeniden bağlanma zamanlayıcısının tutamacı hiç sıfırlanmıyordu; sunucu
//    kapalıyken yapılan tek deneme tutamacı dolu bırakıyor ve
//    `if (!yenidenBaglanmaZamanlayicisi)` kapısı bir daha açılmıyordu.
// 2. Sekme kaydı KENAR-tetiklemeliydi: `bagliSekmeler` eşlemesi bağlantı
//    koptuğunda da yaşadığı için yeni oturumda `kayit` bir daha
//    gönderilmiyordu; WebSocket ayakta ama sunucu tarafında hesap yok
//    (`sayi: 0`) kalıyordu.
//
// Aşağıdaki testler sahte zamanlayıcı, sahte `fetch` ve sahte `WebSocket` ile
// iki kapıyı da ölçer.
import { test, expect } from "bun:test";
import { readFileSync } from "node:fs";
import { createContext, runInContext } from "node:vm";

const KIMLIK = readFileSync(new URL("../../eklenti/kimlik.js", import.meta.url), "utf8");
const KAYNAK = readFileSync(new URL("../../eklenti/background.js", import.meta.url), "utf8");

const bosalt = async () => {
  for (let i = 0; i < 25; i++) await Promise.resolve();
};

function kur({ hataSayisi = 0, sekmeVar = false } = {}) {
  const zamanlayicilar = [];
  const acilanSoketler = [];
  const gonderiler = [];
  const gunluk = [];
  let sonrakiId = 1;
  let fetchSayaci = 0;
  let kalanHata = hataSayisi;

  const sekmeler = sekmeVar
    ? [
        {
          id: 7,
          url: "https://gemini.google.com/app",
          cookieStoreId: "firefox-default",
          active: true,
          lastAccessed: 1,
          discarded: false
        }
      ]
    : [];

  const sandbox = {
    console: {
      log: (...a) => gunluk.push(a.join(" ")),
      warn: (...a) => gunluk.push(a.join(" ")),
      error: (...a) => gunluk.push(a.join(" "))
    },
    setTimeout: (fn, ms) => {
      const id = sonrakiId++;
      zamanlayicilar.push({ id, fn, ms });
      return id;
    },
    clearTimeout: (id) => {
      const i = zamanlayicilar.findIndex((z) => z.id === id);
      if (i >= 0) zamanlayicilar.splice(i, 1);
    },
    setInterval: () => 0, // 10 sn'lik tarama testte koşmaz
    fetch: async (url) => {
      if (String(url).includes("kervan-profil.json")) {
        return { ok: true, json: async () => ({ ad: "hesap1@gmail.com" }) };
      }
      fetchSayaci++;
      if (kalanHata > 0) {
        kalanHata--;
        return { ok: false, status: 503 };
      }
      return { ok: true, json: async () => ({ token: "sahte-jeton" }) };
    },
    WebSocket: class {
      static OPEN = 1;
      static CONNECTING = 0;
      static CLOSED = 3;
      constructor(url) {
        this.url = url;
        this.readyState = 0;
        acilanSoketler.push(this);
      }
      send(veri) {
        gonderiler.push(JSON.parse(veri));
      }
      // Testte tarayıcı yerine biz açıyoruz: gerçekte tarayıcı yapar.
      ac() {
        this.readyState = 1;
        if (this.onopen) this.onopen();
      }
      close() {
        this.readyState = 3;
        if (this.onclose) this.onclose();
      }
    },
    browser: {
      runtime: {
        onMessage: { addListener: () => {} },
        getURL: (yol) => `moz-extension://sahte/${yol}`
      },
      storage: { local: { get: async () => ({}), set: async () => {} } },
      tabs: {
        query: async () => sekmeler,
        sendMessage: async () => ({ ok: true }),
        onRemoved: { addListener: () => {} }
      }
    }
  };
  sandbox.window = sandbox;
  const baglam = createContext(sandbox);
  runInContext(KIMLIK, baglam);
  runInContext(KAYNAK, baglam);

  // Sıradaki zamanlayıcıyı elle çalıştırır (testin saati bizde).
  const zamanlayiciyiKos = async () => {
    const z = zamanlayicilar.shift();
    if (!z) return false;
    z.fn();
    await bosalt();
    return true;
  };

  return {
    zamanlayiciyiKos,
    acilanSoketler,
    gonderiler,
    gunluk,
    sayaclar: () => ({ fetch: fetchSayaci, zamanlayici: zamanlayicilar.length })
  };
}

test("sunucu kapalıyken yapılan deneme yeniden planlanır (tek denemede kalmaz)", async () => {
  const { zamanlayiciyiKos, acilanSoketler, sayaclar } = kur({ hataSayisi: 3 });
  await bosalt();
  expect(acilanSoketler.length).toBe(0);

  // Üç başarısız deneme (biri açılışta, ikisi zamanlayıcıdan) + dördüncüde
  // başarı. Her başarısızlık YENİ bir zamanlayıcı bırakmalı; bırakmazsa döngü
  // burada durur.
  for (let i = 0; i < 3; i++) {
    expect(await zamanlayiciyiKos()).toBe(true);
  }

  expect(sayaclar().fetch).toBe(4);
  expect(acilanSoketler.length).toBe(1);
  expect(acilanSoketler[0].url).toContain("token=sahte-jeton");
});

test("kopan bağlantı kendiliğinden yeniden kurulur", async () => {
  const { zamanlayiciyiKos, acilanSoketler } = kur();
  await bosalt();
  expect(acilanSoketler.length).toBe(1);

  acilanSoketler[0].close(); // sunucu düştü
  expect(await zamanlayiciyiKos()).toBe(true);

  expect(acilanSoketler.length).toBe(2);
});

test("her bağlantı kuruluşunda sekme kaydı yeniden gönderilir", async () => {
  const { zamanlayiciyiKos, acilanSoketler, gonderiler } = kur({ sekmeVar: true });
  await bosalt();

  const ilk = acilanSoketler[0];
  ilk.ac();
  await bosalt();
  expect(gonderiler.some((g) => g.action === "kayit" && g.profile === "hesap1@gmail.com")).toBe(true);

  // Sunucu yeniden başladı: soket düştü, yenisi kuruldu. Eşleme (bagliSekmeler)
  // hâlâ aynı tabId'yi taşıdığı için kaydın yeniden gönderilmesi TEK başına bu
  // testin konusudur.
  gonderiler.length = 0;
  ilk.close();
  expect(await zamanlayiciyiKos()).toBe(true);
  const ikinci = acilanSoketler[1];
  ikinci.ac();
  await bosalt();

  expect(gonderiler.some((g) => g.action === "kayit" && g.profile === "hesap1@gmail.com")).toBe(true);
});
