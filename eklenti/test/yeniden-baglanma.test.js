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

function kur({ hataSayisi = 0, sekmeVar = false, sekmeler: ozelSekmeler = null } = {}) {
  const zamanlayicilar = [];
  const acilanSoketler = [];
  const gonderiler = [];
  const gunluk = [];
  const enjeksiyonlar = [];
  const alarmlar = [];
  let alarmDinleyici = null;
  let guncellemeDinleyici = null;
  let pingBasarisiz = 0; // kaç ping "içerik betiği yok" ile düşsün
  let enjeksiyonBasarisiz = false; // CSP/gezinme ortası: executeScript reddeder
  let sonrakiId = 1;
  let fetchSayaci = 0;
  let kalanHata = hataSayisi;

  // B4 (2026-11-15): çok sekme senaryoları için fikstür dışarıdan verilebilir.
  // Dizi REFERANSLA tutulur: test turlar arasında `active`/`lastAccessed`
  // değiştirip puanın yeniden hesaplandığını ama seçimin kaymadığını ölçer.
  const sekmeler = ozelSekmeler
    ? ozelSekmeler
    : sekmeVar
      ? [
          {
            id: 7,
            url: "https://gemini.google.com/app",
            cookieStoreId: "firefox-default",
            active: true,
            lastAccessed: 1,
            discarded: false,
            status: "complete"
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
        // B3 (2026-11-15): içerik betiği ölmüşse `sendMessage` "alıcı yok"
        // hatasıyla düşer. `pingBasarisiz` bu senaryoyu kurar: ilk N ping
        // hata verir, enjeksiyon sonrası ping geçer.
        sendMessage: async (tabId, mesaj) => {
          if (mesaj && mesaj.type === "KERVAN_PING" && pingBasarisiz > 0) {
            pingBasarisiz--;
            throw new Error("Could not establish connection. Receiving end does not exist.");
          }
          return { ok: true };
        },
        onRemoved: { addListener: () => {} },
        onUpdated: { addListener: (fn) => { guncellemeDinleyici = fn; } }
      },
      scripting: {
        executeScript: async ({ target, files }) => {
          enjeksiyonlar.push({ tabId: target && target.tabId, files });
          // 2026-11-15 denetimi: CSP engeli ya da gezinme ortası — gerçek
          // tarayıcı burada reddeder. Başarısız enjeksiyon CANLI hesabı
          // düşürmemeli (madde 1).
          if (enjeksiyonBasarisiz) throw new Error("Cannot access contents of the page");
          return [{ result: null }];
        }
      },
      alarms: {
        create: (ad, ayar) => { alarmlar.push({ ad, ayar }); },
        onAlarm: { addListener: (fn) => { alarmDinleyici = fn; } }
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
    enjeksiyonlar,
    alarmlar,
    alarmDinleyici: () => alarmDinleyici,
    guncellemeDinleyici: () => guncellemeDinleyici,
    pingDusur: (adet = 1) => { pingBasarisiz = adet; },
    enjeksiyonDusur: () => { enjeksiyonBasarisiz = true; },
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
  // Sekme gerçekten GİTMİŞ olmalı (tarayıcı çöktü, `onRemoved` ateşlenmedi):
  // yalnız içerik betiğinin ölmesi artık "canlı" sayılıyor (madde 1).
  const sekmeler = [
    {
      id: 7,
      url: "https://gemini.google.com/app",
      cookieStoreId: "firefox-default",
      active: true,
      lastAccessed: 1,
      discarded: false,
      status: "complete"
    }
  ];
  const { zamanlayiciyiKos, acilanSoketler, gonderiler } = kur({ sekmeler });
  await bosalt();

  const ilk = acilanSoketler[0];
  ilk.ac();
  await bosalt();
  expect(gonderiler.some((g) => g.action === "kayit")).toBe(true);

  // Soket düşerken sekme de kapanmış olsun.
  sekmeler.length = 0;
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
  const sekmeler = [
    {
      id: 7,
      url: "https://gemini.google.com/app",
      cookieStoreId: "firefox-default",
      active: true,
      lastAccessed: 1,
      discarded: false,
      status: "complete"
    }
  ];
  const { acilanSoketler, gonderiler, tara } = kur({ sekmeler });
  await bosalt();
  acilanSoketler[0].ac();
  await bosalt();
  expect(gonderiler.some((g) => g.action === "kayit")).toBe(true);

  // Sekme kapandı (ve `onRemoved` ateşlenmedi): tarama artık onu hiç görmez.
  sekmeler.length = 0;
  gonderiler.length = 0;
  await tara();
  await bosalt();

  expect(gonderiler.filter((g) => g.action === "kayit").length).toBe(0);
  expect(
    gonderiler.some((g) => g.action === "ayril" && g.provider === "gemini")
  ).toBe(true);
});

// B3 (denetim, 2026-11-15): PING HATASINDA YENİDEN ENJEKSİYON.
//
// NEDEN: içerik betiği sayfada ölmüş ama sekme yenilenmemişse (askıya alınmış
// süreç, betik çökmesi) hesap her turda düşürülüyor ve kullanıcı sekmeyi elle
// yenilemedikçe geri gelmiyordu. Artık `status === "complete"` ise
// `scripting.executeScript` ile betikler BİR KEZ yeniden enjekte edilir ve
// ping tekrarlanır.
test("içerik betiği ölmüşse bir kez yeniden enjekte edilir ve sekme kurtarılır", async () => {
  const { acilanSoketler, gonderiler, tara, enjeksiyonlar, pingDusur } = kur({ sekmeVar: true });
  await bosalt();
  acilanSoketler[0].ac();
  await bosalt();
  expect(gonderiler.some((g) => g.action === "kayit")).toBe(true);

  // Sekme hâlâ açık ama içerik betiği yanıt vermiyor: ilk ping düşer,
  // enjeksiyondan sonraki ping geçer.
  pingDusur(1);
  gonderiler.length = 0;
  await tara();
  await bosalt();

  // Dosya sırası manifest ile aynı olmalı: önce ortak yardımcılar.
  expect(enjeksiyonlar.length).toBe(1);
  expect(enjeksiyonlar[0].tabId).toBe(7);
  expect(enjeksiyonlar[0].files).toEqual([
    "content_common.js",
    "content_gemini.js"
  ]);

  // Kurtarıldı: kayıt gitti, ayril GİTMEDİ.
  expect(gonderiler.filter((g) => g.action === "kayit").length).toBe(1);
  expect(gonderiler.some((g) => g.action === "ayril")).toBe(false);
});

// Enjeksiyon sayfa yüklemesi başına BİR kez: CSP blokajında her turda denemek
// konsolu doldurur ve taramayı yavaşlatır. Sekme yenilenince url değişir.
test("aynı sayfa yüklemesinde ikinci kez enjeksiyon denenmez", async () => {
  const { acilanSoketler, gonderiler, tara, enjeksiyonlar, pingDusur } = kur({ sekmeVar: true });
  await bosalt();
  acilanSoketler[0].ac();
  await bosalt();

  pingDusur(1);
  await tara();
  await bosalt();
  expect(enjeksiyonlar.length).toBe(1);

  // Betik yine öldü: ping düşüyor, ama bu url için hak kullanıldı.
  pingDusur(1);
  gonderiler.length = 0;
  await tara();
  await bosalt();

  expect(enjeksiyonlar.length, "ikinci enjeksiyon denenmemeli").toBe(1);
  // DENETİM DÜZELTMESİ (madde 1): eskiden burada `ayril` BEKLENİYORDU. O
  // iddia hatayı sözleşme yapıyordu: sekme açık, oturum ayakta, yalnız betik
  // çalışmıyor — hesabı düşürmek yanlıştı. Sekme canlı sayılır, kayıt tazelenir.
  expect(
    gonderiler.some((g) => g.action === "ayril"),
    "kurtarılamayan ama AÇIK sekme düşürülmemeli"
  ).toBe(false);
  expect(gonderiler.filter((g) => g.action === "kayit").length).toBe(1);
});

// madde 1: enjeksiyon REDDEDİLİRSE (CSP, gezinme ortası) hesap sessizce
// düşmemeli. Kronik hata buydu: `executeScript` fırlatınca `ulasilan` boş
// kalıyor, budama `kervandanDusur` çağırıyor ve deneme hakkı harcanmış
// olduğu için düşme her turda yineleniyordu.
test("enjeksiyon reddedilirse açık sekme düşürülmez", async () => {
  const { acilanSoketler, gonderiler, tara, enjeksiyonlar, pingDusur, enjeksiyonDusur } =
    kur({ sekmeVar: true });
  await bosalt();
  acilanSoketler[0].ac();
  await bosalt();

  enjeksiyonDusur();
  pingDusur(2); // tur boyunca hiçbir ping geçmez
  gonderiler.length = 0;
  await tara();
  await bosalt();

  expect(enjeksiyonlar.length).toBe(1);
  expect(gonderiler.some((g) => g.action === "ayril")).toBe(false);
  expect(gonderiler.filter((g) => g.action === "kayit").length).toBe(1);

  // Ve düşme KALICI değil: ikinci turda da ayril yok, tekrar deneme de yok.
  pingDusur(2);
  gonderiler.length = 0;
  await tara();
  await bosalt();
  expect(enjeksiyonlar.length).toBe(1);
  expect(gonderiler.some((g) => g.action === "ayril")).toBe(false);
});

// madde 2: aynı adrese F5. `changeInfo.url` gelmez, url aynı kalır; defter
// `status === "complete"` görüldüğünde silinmeli ki yeni bir hak doğsun.
test("sayfa yenilenince enjeksiyon hakkı yenilenir", async () => {
  const { acilanSoketler, gonderiler, tara, enjeksiyonlar, pingDusur, guncellemeDinleyici } =
    kur({ sekmeVar: true });
  await bosalt();
  acilanSoketler[0].ac();
  await bosalt();

  pingDusur(1);
  await tara();
  await bosalt();
  expect(enjeksiyonlar.length).toBe(1);

  // Aynı adrese yenileme: yalnız `status` gelir, `url` gelmez.
  pingDusur(1);
  guncellemeDinleyici()(7, { status: "complete" });
  gonderiler.length = 0;
  await tara();
  await bosalt();

  expect(enjeksiyonlar.length, "yenilemeden sonra yeni deneme hakkı").toBe(2);
  expect(gonderiler.some((g) => g.action === "ayril")).toBe(false);
});

// B1 (denetim, 2026-11-15): ALARM + onUpdated UYANDIRICILARI.
//
// NEDEN: Firefox olay sayfası 30 sn boşta kalınca yok edilir ve açık WebSocket
// bu sayacı sıfırlamaz (bug 1748533, WONTFIX). Önceden tek dayanak `setInterval`
// idi; 30 sn kaçırılırsa döngü kalıcı kırılıyordu. Bu test, alarmın kurulduğunu
// ve alarmın gerçekten taramayı koşturduğunu ölçer.
test("periyodik alarm kurulur ve alarm taramayı koşturur", async () => {
  const { gonderiler, alarmlar, alarmDinleyici, acilanSoketler } = kur({ sekmeVar: true });
  await bosalt();
  acilanSoketler[0].ac();
  await bosalt();

  expect(alarmlar.length).toBe(1);
  expect(alarmlar[0].ad).toBe("kervan-sekme-tarama");
  expect(alarmlar[0].ayar.periodInMinutes).toBe(0.5);

  const dinleyici = alarmDinleyici();
  expect(typeof dinleyici).toBe("function");

  gonderiler.length = 0;
  dinleyici({ name: "baska-alarm" }); // ad eşleşmiyor: tarama YOK
  await bosalt();
  expect(gonderiler.length).toBe(0);

  dinleyici({ name: "kervan-sekme-tarama" });
  await bosalt();
  expect(gonderiler.filter((g) => g.action === "kayit").length).toBe(1);
});

// onUpdated: sekme yüklemesi tamamlanınca tarama 1 sn gecikmeyle tetiklenir.
test("sekme yüklemesinin bitişi taramayı tetikler", async () => {
  const { gonderiler, guncellemeDinleyici, acilanSoketler, zamanlayiciyiKos } = kur({
    sekmeVar: true
  });
  await bosalt();
  acilanSoketler[0].ac();
  await bosalt();

  const dinleyici = guncellemeDinleyici();
  expect(typeof dinleyici).toBe("function");

  gonderiler.length = 0;
  dinleyici(7, { status: "loading" }); // henüz tamamlanmadı: tetiklemez
  await bosalt();
  expect(gonderiler.length).toBe(0);

  dinleyici(7, { status: "complete" });
  await bosalt();
  expect(gonderiler.length, "gecikmeli tarama henüz koşmamalı").toBe(0);

  expect(await zamanlayiciyiKos()).toBe(true);
  await bosalt();
  expect(gonderiler.filter((g) => g.action === "kayit").length).toBe(1);
});

// B4 (denetim, 2026-11-15): YAPIŞKAN SEÇİM.
//
// NEDEN: seçim her turda puana göre yeniden yapılıyordu ve puan
// `(aktif ? 1e12 : 0) + lastAccessed`. Aynı sağlayıcı+profilin İKİ sekmesi
// açıkken kullanıcı sekmeler arasında gezindiğinde `lastAccessed` değişiyor,
// eşleme A↔B gidip geliyordu. İstek bir turda A'ya, sonraki turda B'ye
// düşüyor; yanıt kimliği (`id`) eşleşse de istem BAŞKA bir sohbet penceresine
// yazılıyor ve bağlam kayboluyordu. Beklenen: seçilen sekme yaşadığı sürece
// eşleme DEĞİŞMEZ, puan kayması onu deviremez.

/** İki Gemini sekmesi: 7 ve 9 (aynı profil → aynı anahtar). */
function ikiSekme() {
  return [
    {
      id: 7,
      url: "https://gemini.google.com/app",
      cookieStoreId: "firefox-default",
      active: true,
      lastAccessed: 100,
      discarded: false,
      status: "complete"
    },
    {
      id: 9,
      url: "https://gemini.google.com/app/ikinci",
      cookieStoreId: "firefox-default",
      active: false,
      lastAccessed: 50,
      discarded: false,
      status: "complete"
    }
  ];
}

test("puan kayması yapışkan seçimi devirmez", async () => {
  const sekmeler = ikiSekme();
  const { acilanSoketler, gonderiler, tara } = kur({ sekmeler });
  await bosalt();
  acilanSoketler[0].ac();
  await bosalt();

  // İlk seçim: 7 etkin (puan 1e12 + 100).
  let kayitlar = gonderiler.filter((g) => g.action === "kayit");
  expect(kayitlar.length).toBe(1);
  expect(kayitlar[0].tab_id).toBe(7);

  // Kullanıcı öbür sekmeye geçti: puan artık 9'da. Seçim KAYMAMALI.
  sekmeler[0].active = false;
  sekmeler[1].active = true;
  sekmeler[1].lastAccessed = 999;

  gonderiler.length = 0;
  await tara();
  await bosalt();
  kayitlar = gonderiler.filter((g) => g.action === "kayit");
  expect(kayitlar.length, "tazeleme tek kayıt göndermeli").toBe(1);
  expect(kayitlar[0].tab_id, "seçim yapışkan kalmalı").toBe(7);

  // İkinci tur da aynı: kararlılık tek turla sınırlı değil.
  gonderiler.length = 0;
  await tara();
  await bosalt();
  expect(gonderiler.filter((g) => g.action === "kayit")[0].tab_id).toBe(7);
});

test("seçili sekme kapanınca öbür sekmeye geçilir ve ayril gönderilmez", async () => {
  const sekmeler = ikiSekme();
  const { acilanSoketler, gonderiler, tara } = kur({ sekmeler });
  await bosalt();
  acilanSoketler[0].ac();
  await bosalt();
  expect(gonderiler.filter((g) => g.action === "kayit")[0].tab_id).toBe(7);

  // Seçili sekme kapandı: sorgu artık onu döndürmüyor.
  sekmeler.shift(); // 7 gitti, kalan 9
  sekmeler[0].active = true;

  gonderiler.length = 0;
  await tara();
  await bosalt();

  const kayitlar = gonderiler.filter((g) => g.action === "kayit");
  expect(kayitlar.length, "kalan sekme yeniden seçilmeli").toBe(1);
  expect(kayitlar[0].tab_id).toBe(9);
  // Hesap hâlâ ayakta: aynı anahtar için `ayril` GİTMEMELİ.
  expect(gonderiler.some((g) => g.action === "ayril")).toBe(false);
});

test("puanlar eşitse küçük tabId kazanır", async () => {
  // İki sekme de pasif ve aynı `lastAccessed`: puan eşit. Seçim tarayıcının
  // `tabs.query` sırasına bırakılırsa kararsız olur; küçük tabId sabitler.
  // Bu test YAPIŞKANLIĞI değil BERABERLİK kuralını ölçer: her koşu SIFIRDAN
  // başlar (boş `bagliSekmeler`), yani ilk seçim doğrudan puan/beraberlik
  // kuralıyla yapılır.
  const esitPuan = (ters) => {
    const sekmeler = ikiSekme();
    for (const s of sekmeler) {
      s.active = false;
      s.lastAccessed = 50;
    }
    return ters ? sekmeler.reverse() : sekmeler;
  };

  for (const ters of [false, true]) {
    const { acilanSoketler, gonderiler } = kur({ sekmeler: esitPuan(ters) });
    await bosalt();
    acilanSoketler[0].ac();
    await bosalt();
    const kayitlar = gonderiler.filter((g) => g.action === "kayit");
    expect(kayitlar.length, "ilk tarama tek kayıt göndermeli").toBe(1);
    expect(kayitlar[0].tab_id, `ters=${ters}: küçük tabId seçilmeli`).toBe(7);
  }
});
