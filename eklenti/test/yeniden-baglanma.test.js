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
    sandbox,
    // B3 (2026-11-15): periyodik tarama testte `setInterval` ile değil,
    // doğrudan çağrılarak koşulur (fonksiyon sandbox bağlamında global).
    tara: () => baglam.acikSekmeleriTara(),
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

// B5 (denetim, 2026-11-15): `onopen`'daki `bagliSekmeler.clear()` KANITI.
//
// NEDEN: eski test (yukarıdaki) `clear()` satırı silinse de geçiyordu; çünkü
// `onopen` her hâlükârda `acikSekmeleriTara()` çağırıyor ve o tur `kayit`ı
// koşulsuz gönderiyor. `clear()`ın gerçekten gerekli olduğu tek durum: bağlantı
// yeniden kurulduğunda eski oturumdan kalan eşleme ÖLÜ bir sekmeyi gösteriyorsa.
// `clear()` olmadan budama döngüsü o bayat kaydı görür ve sunucuya `ayril`
// gönderir; `clear()` ile `bagliSekmeler` boş olduğu için budanacak bir şey
// yoktur. (Sunucu tarafında A2 kimlik kapısı bu `ayril`ı zaten düşürmez, ama
// gereksiz trafik ve yanlış log üretmemek için istemcide de susmalı.)
test("yeniden bağlanmada bayat eşleme temizlenir, ölü sekme için ayril gönderilmez", async () => {
  const { zamanlayiciyiKos, acilanSoketler, gonderiler, sandbox } = kur({ sekmeVar: true });
  await bosalt();

  const ilk = acilanSoketler[0];
  ilk.ac();
  await bosalt();
  expect(gonderiler.some((g) => g.action === "kayit")).toBe(true);

  // Soket düşerken içerik betiği de ölmüş olsun (sekme askıda kaldı).
  sandbox.browser.tabs.sendMessage = async () => {
    throw new Error("Receiving end does not exist");
  };
  ilk.close();
  expect(await zamanlayiciyiKos()).toBe(true);
  gonderiler.length = 0;
  const ikinci = acilanSoketler[1];
  ikinci.ac();
  await bosalt();

  expect(gonderiler.some((g) => g.action === "kayit")).toBe(false);
  expect(
    gonderiler.some((g) => g.action === "ayril"),
    `bayat eşleme budanmamalı: ${JSON.stringify(gonderiler)}`
  ).toBe(false);
});

// B2 (denetim, 2026-11-15): GECİKMİŞ KAPANIŞ KİMLİK KAPISI.
//
// NEDEN: eski soketin `onclose`'u gecikmeyle çalıştığında o sırada ayakta olan
// YENİ soketi `null`'lıyor ve fazladan bağlantı açıyordu (ölçüldü: 3 soket,
// ikisi CONNECTING). Kapı olmadığında ilk `eski.close()` çağrısı ikinci kez
// çalıştırılınca `socket = null` + yeni yeniden bağlanma planı ortaya çıkar.
test("gecikmiş eski kapanışı yeni soketi öldürmez ve fazladan bağlantı açmaz", async () => {
  const { zamanlayiciyiKos, acilanSoketler, sayaclar } = kur();
  await bosalt();

  const eski = acilanSoketler[0];
  eski.ac();
  await bosalt();

  // 1) Gerçek kapanış: yeniden bağlanma planlanır.
  eski.close();
  expect(sayaclar().zamanlayici).toBe(1);
  expect(await zamanlayiciyiKos()).toBe(true);
  const yeni = acilanSoketler[1];
  yeni.ac();
  await bosalt();

  // 2) GECİKMİŞ kapanış: aynı eski soketin `onclose`'u bir kez daha koşar.
  eski.close();
  await bosalt();

  expect(sayaclar().zamanlayici, "fazladan yeniden bağlanma planlanmamalı").toBe(0);
  expect(acilanSoketler.length, "üçüncü soket açılmamalı").toBe(2);
  expect(await zamanlayiciyiKos()).toBe(false);
});

// B3 (denetim, 2026-11-15): TAZELEME TURU.
//
// NEDEN: kayıt eskiden yalnız eşleme DEĞİŞİNCE gönderiliyordu; eşleme sabit
// kaldığı sürece sunucu damgayı tazelemiyor ve canlı bir sekme 90 sn sonra
// "ölü" sayılıyordu (SEKME_TAZELIK_ESIGI). Artık her 10 sn'lik tarama turu
// eşleme aynı olsa da `kayit` gönderir. Bu test o kapıyı ölçer.
test("tarama turu eşleme değişmese de sekme kaydını tazeler", async () => {
  const { acilanSoketler, gonderiler, tara, gunluk } = kur({ sekmeVar: true });
  await bosalt();
  acilanSoketler[0].ac();
  await bosalt();
  // İlk tarama bağlantı kuruluşunda yapıldı: eşleme artık dolu.
  expect(gonderiler.some((g) => g.action === "kayit")).toBe(true);

  // TAZELEME TURU: aynı sekme, aynı eşleme. Log YAZILMAMALI (gürültü
  // sözleşmesi) ama `kayit` GİTMELİ.
  gonderiler.length = 0;
  const oncekiGunluk = gunluk.length;
  await tara();
  await bosalt();

  const kayitlar = gonderiler.filter((g) => g.action === "kayit");
  expect(kayitlar.length).toBe(1);
  expect(kayitlar[0].provider).toBe("gemini");
  expect(kayitlar[0].profile).toBe("hesap1@gmail.com");
  expect(kayitlar[0].tab_id).toBe(7);
  expect(gunluk.length).toBe(oncekiGunluk);
});

// Tazeleme turu ölü sekmeyi DÜŞÜRÜR: budama kapısı da aynı turda ölçülür.
test("tarama turu ulaşılamayan sekmeyi düşürür", async () => {
  const { acilanSoketler, gonderiler, tara, sandbox } = kur({ sekmeVar: true });
  await bosalt();
  acilanSoketler[0].ac();
  await bosalt();
  expect(gonderiler.some((g) => g.action === "kayit")).toBe(true);

  // İçerik betiği artık yanıt vermiyor (sekme kapandı / betik öldü).
  sandbox.browser.tabs.sendMessage = async () => {
    throw new Error("Receiving end does not exist");
  };
  gonderiler.length = 0;
  await tara();
  await bosalt();

  expect(gonderiler.filter((g) => g.action === "kayit").length).toBe(0);
  expect(
    gonderiler.some((g) => g.action === "ayril" && g.provider === "gemini")
  ).toBe(true);
});
