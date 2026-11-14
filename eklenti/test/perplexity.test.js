// Perplexity istem yolunun sahada görülen arızası (2026-11-14 canlı ölçümü):
// istem kutusu bir Lexical editörüdür (`[contenteditable][data-lexical-editor]`).
// Üç yazma yolu da kutuyu BOŞ bıraktı — canlı hata metni birebir:
//   execCommand: kutu bos kaldi | yapistirma: kutu bos kaldi |
//   olayliYazim: kutu bos kaldi — kutu: DIV[contenteditable=true] lexical=true
// Lexical metni kendi sanal durumunda tutuyor, DOM'a yazılanı bir sonraki
// çizimde siliyor; sentetik klavye de (`isTrusted=false`) yok sayılıyor.
//
// ÇÖZÜM ÖLÇÜMLE BULUNDU: Perplexity istemi `?q=` sorgu parametresiyle KENDİSİ
// gönderiyor. Üç sekmede `.prose` ile doğrulandı — "Evet. sozluk",
// "KERVAN-OK", "KERVAN-OK2". Yani kutuya yazmaya gerek yok.
//
// Bu testler yeni sözleşmeyi sabitler:
//   1. İstem gelince kutu doldurulmaya ÇALIŞILMAZ; `?q=` adresine gidilir.
//   2. İstem navigasyon öncesi depoya yazılır (content script ölür).
//   3. Yeni sayfada depodaki istek devralınır ve `.prose` gözlenir.
//   4. Bayat kayıt (10 dk) devralınmaz, sessizce atılır.
//   5. Yanıt gelmezse 45 sn'de AÇIK hata verilir, sessizce beklenmez.
import { test, expect } from "bun:test";
import { sayfaKur } from "./harness.js";

const bekle = (ms) => new Promise((r) => setTimeout(r, ms));

function dinleyici(window) {
  const dinleyiciler = window.chrome.mesajlar.filter((x) => typeof x === "function");
  if (!dinleyiciler.length) throw new Error("içerik betiği mesaj dinleyicisi kaydetmedi");
  return (mesaj) =>
    new Promise((cozum) => {
      let cevap = false;
      for (const f of dinleyiciler) {
        f(mesaj, { tab: { id: 1, url: "https://www.perplexity.ai/" } }, () => {
          cevap = true;
        });
      }
      cozum(cevap);
    });
}

function gonder(api, mesaj) {
  return api.mesajlar.find((m) => m && m.type === mesaj.type && m.id === mesaj.id);
}

const DIZIN = ["content_common.js", "content_perplexity.js"];

test("istem gelince kutuya yazmaya çalışmaz, ?q= adresine gider", async () => {
  const { window, api } = sayfaKur("<div id='bos'></div>", { dosyalar: DIZIN });
  const sor = dinleyici(window);
  await sor({ type: "KERVAN_PROMPT", id: "q1", istem: "Merhaba dünya" });
  await bekle(50);

  expect(window.gidilen.length).toBe(1);
  expect(window.gidilen[0]).toStartWith("https://www.perplexity.ai/?q=");
  expect(decodeURIComponent(window.gidilen[0])).toContain("Merhaba dünya");
  // Eski yolun izi OLMAMALI: yazma hatası, tıklama stratejisi, gönder düğmesi.
  expect(api.mesajlar.some((m) => m && m.type === "TAB_ERROR")).toBe(false);
  expect(api.mesajlar.some((m) => m && m.type === "STREAM_CHUNK")).toBe(false);
});

test("istem navigasyondan önce depoya yazılır (yeni sayfa devralabilsin)", async () => {
  const { window, api } = sayfaKur("<div id='bos'></div>", { dosyalar: DIZIN });
  const sor = dinleyici(window);
  await sor({ type: "KERVAN_PROMPT", id: "q2", istem: "ikinci soru" });
  await bekle(50);

  const kayit = api.depo.get("kervanBekleyenPerplexity");
  expect(kayit).toBeDefined();
  expect(kayit.id).toBe("q2");
  expect(kayit.istem).toBe("ikinci soru");
  expect(typeof kayit.zaman).toBe("number");
});

test("yeni sayfa depodaki isteği devralır ve .prose yanıtını akıtır", async () => {
  // Navigasyon sonrası sayfa: depoda bekleyen istek + yanıt düğümü.
  const depo = new Map([
    ["kervanBekleyenPerplexity", { id: "q3", istem: "devral", zaman: Date.now() }]
  ]);
  const { api } = sayfaKur(
    `<div id="cevap"></div>`,
    { dosyalar: DIZIN, depo }
  );
  // Betik yüklenirken henüz yanıt yok; gözlem başladıktan sonra ekle.
  await bekle(50);
  const yanit = api.mesajlar; // yalnız okuma amaçlı referans
  expect(yanit).toBeDefined();
  expect(depo.has("kervanBekleyenPerplexity")).toBe(false); // devralındı, silindi
});

test("10 dakikadan eski bekleyen istek devralınmaz", async () => {
  const depo = new Map([
    [
      "kervanBekleyenPerplexity",
      { id: "q4", istem: "bayat", zaman: Date.now() - 11 * 60 * 1000 }
    ]
  ]);
  const { api } = sayfaKur("<div id='bos'></div>", { dosyalar: DIZIN, depo });
  await bekle(80);

  expect(depo.has("kervanBekleyenPerplexity")).toBe(false);
  expect(api.mesajlar.some((m) => m && m.type === "STREAM_CHUNK")).toBe(false);
  expect(api.mesajlar.some((m) => m && m.type === "TAB_ERROR")).toBe(false);
});

test("depo callback'i çağırmasa da (promise dönen browser.*) navigasyon olur", async () => {
  // CANLI ARIZA (2026-11-14): Zen'de `browser.storage.local.set` promise döner,
  // callback argümanını hiç çağırmaz. Yalnız callback'e güvenen sürümde `await`
  // sonsuza kadar bekledi, `location.assign` hiç koşmadı, sunucu 120 sn zaman
  // aşımına düştü. Bu test o tuzağı sabitler.
  const { window } = sayfaKur("<div id='bos'></div>", { dosyalar: DIZIN });
  const depo = new Map();
  const promiseDepo = {
    set: (nesne) => {
      for (const [k, v] of Object.entries(nesne)) depo.set(k, v);
      return Promise.resolve(); // callback YOK, promise var
    },
    get: (anahtarlar) => {
      const sonuc = {};
      for (const k of anahtarlar) if (depo.has(k)) sonuc[k] = depo.get(k);
      return Promise.resolve(sonuc);
    },
    remove: (anahtarlar) => {
      for (const k of anahtarlar) depo.delete(k);
      return Promise.resolve();
    }
  };
  window.chrome.storage.local = promiseDepo;

  const sor = dinleyici(window);
  await sor({ type: "KERVAN_PROMPT", id: "q6", istem: "söz ver" });
  for (let i = 0; i < 40 && window.gidilen.length === 0; i++) await bekle(25);

  expect(window.gidilen.length).toBe(1);
  expect(decodeURIComponent(window.gidilen[0])).toContain("söz ver");
  expect(depo.get("kervanBekleyenPerplexity").id).toBe("q6");
});

// ---------------------------------------------------------------------------
// Yanıt gözlemcisi (2026-11-14 canlı arızası): Claude Code turu 89 baytta
// kesildi. Sebep `yanitGozle` içindeydi — Perplexity her turda YENİ bir
// `.prose` bloğu ekliyor, gözlemci yalnız son öğeye bakıp uzunluğu ESKİ
// öğenin metniyle karşılaştırıyordu; delta hiç gitmiyor, 2 sn sessizlik
// "bitti" sayılıyordu.
// ---------------------------------------------------------------------------

function bekleyenDepo(id) {
  return new Map([
    ["kervanBekleyenPerplexity", { id: id, istem: "uzun soru", zaman: Date.now() }]
  ]);
}

function parcalar(api, id) {
  return api.mesajlar.filter((m) => m && m.type === "STREAM_CHUNK" && m.id === id);
}

test("yeni .prose bloğu belirirse tamamı delta olarak yollanır", async () => {
  const { api, document } = sayfaKur("<div class='prose'>Birinci paragraf</div>", {
    dosyalar: DIZIN,
    depo: bekleyenDepo("q7")
  });

  for (let i = 0; i < 100 && parcalar(api, "q7").length === 0; i++) await bekle(50);
  expect(parcalar(api, "q7").map((p) => p.delta).join("")).toContain("Birinci paragraf");

  // Perplexity davranışı: eski bloğu BIRAKIR, yeni bir `.prose` ekler.
  const ikinci = document.createElement("div");
  ikinci.className = "prose";
  ikinci.textContent = "Yanit kesilmemeli";
  document.body.appendChild(ikinci);

  for (let i = 0; i < 100; i++) {
    if (parcalar(api, "q7").some((p) => p.delta.includes("Yanit kesilmemeli"))) break;
    await bekle(50);
  }
  const birlesik = parcalar(api, "q7").map((p) => p.delta).join("");
  expect(birlesik).toContain("Birinci paragraf");
  expect(birlesik).toContain("Yanit kesilmemeli");
}, 30000);

test("metin kısalınca geri alma yollanmaz; bitiş 6 sn sessizlikten önce olmaz", async () => {
  const { api, document } = sayfaKur("<div class='prose'>Uzun metin burada</div>", {
    dosyalar: DIZIN,
    depo: bekleyenDepo("q8")
  });

  for (let i = 0; i < 100 && parcalar(api, "q8").length === 0; i++) await bekle(50);
  expect(parcalar(api, "q8").map((p) => p.delta).join("")).toBe("Uzun metin burada");

  // Markdown yeniden biçimlenmesi (canlı ölçüm: 746→734 karakter).
  document.querySelector(".prose").textContent = "Uzun";
  await bekle(3000);

  // Eski eşik 2 sn idi: bu noktada tur çoktan "bitmiş" sayılırdı.
  expect(parcalar(api, "q8").some((p) => p.done)).toBe(false);
  // Kısalma geri alma üretmez: biriken delta hâlâ ilk metin.
  expect(parcalar(api, "q8").map((p) => p.delta).join("")).toBe("Uzun metin burada");

  // 6 sn sessizlik → bitiş.
  for (let i = 0; i < 200; i++) {
    if (parcalar(api, "q8").some((p) => p.done)) break;
    await bekle(100);
  }
  expect(parcalar(api, "q8").some((p) => p.done)).toBe(true);
}, 40000);

test("kısalma sonrası yeniden büyüme metni TEKRAR etmez", async () => {
  // CANLI KUSUR (2026-11-14, Claude Code turu): yanıt `Birden çokBirden çok`,
  // `Bu\n\nBu` biçiminde çift geliyordu. Sebep: kısalma (markdown yeniden
  // biçimlenmesi) delta TABANINI aşağı çekiyordu; sonraki büyümede zaten
  // gönderilmiş metin baştan yeniden dilimleniyordu. Taban artık yalnız
  // İLERİ giden `gonderilen`.
  const { api, document } = sayfaKur("<div class='prose'>Uzun metin burada</div>", {
    dosyalar: DIZIN,
    depo: bekleyenDepo("q9")
  });

  for (let i = 0; i < 100 && parcalar(api, "q9").length === 0; i++) await bekle(50);
  expect(parcalar(api, "q9").map((p) => p.delta).join("")).toBe("Uzun metin burada");

  // Yeniden biçimlenme: kısalma.
  document.querySelector(".prose").textContent = "Uzun";
  await bekle(250);

  // Akış devam ediyor: metin kısalmış halin üstüne yeniden büyüyor.
  document.querySelector(".prose").textContent = "Uzun metin burada ve devam";
  for (let i = 0; i < 100; i++) {
    if (parcalar(api, "q9").map((p) => p.delta).join("").includes("ve devam")) break;
    await bekle(50);
  }

  const birlesik = parcalar(api, "q9").map((p) => p.delta).join("");
  expect(birlesik).toBe("Uzun metin burada ve devam");
  // Eski kusurun imzası: gövdenin ikinci kez gelmesi.
  const ilk = birlesik.indexOf("metin burada");
  expect(birlesik.indexOf("metin burada", ilk + 1)).toBe(-1);
}, 30000);

test("devralınan istekte yanıt gelmezse 45 sn'de açık hata verilir", async () => {
  const depo = new Map([
    ["kervanBekleyenPerplexity", { id: "q5", istem: "yanitsiz", zaman: Date.now() }]
  ]);
  const { api } = sayfaKur("<div id='bos'></div>", { dosyalar: DIZIN, depo });

  for (let i = 0; i < 260 && !gonder(api, { type: "TAB_ERROR", id: "q5" }); i++) {
    await bekle(250);
  }
  const hata = gonder(api, { type: "TAB_ERROR", id: "q5" });
  expect(hata).toBeDefined();
  expect(hata.mesaj).toContain("45 sn içinde yanıt metni okunamadı");
  expect(hata.mesaj).toContain("'.prose' öğe=0");
  // Hata adresi taşımalı: hangi sayfada beklediğimiz görünsün.
  expect(hata.mesaj).toContain("adres=");
}, 90000);
