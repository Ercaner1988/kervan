// Gemini istem yolunun sahada görülen iki arızası:
//   1) `execCommand("insertText")` hiçbir şey yazmıyor, kutu boş kalıyor.
//   2) Gönder düğmesine sentetik tıklama işlemiyor, istek hiç başlamıyor.
// İkisinin de ortak bedeli şuydu: sunucu 120 sn bekleyip "zaman aşımı" diyordu.
// Bu testler, arızanın SESSİZ kalmadığını ve yedek yolların devreye girdiğini
// tarayıcıya/hesaba dokunmadan sabitler.
import { test, expect } from "bun:test";
import { sayfaKur } from "./harness.js";

function dinleyici(window) {
  const dinleyiciler = window.chrome.mesajlar.filter((x) => typeof x === "function");
  if (!dinleyiciler.length) throw new Error("içerik betiği mesaj dinleyicisi kaydetmedi");
  // Birden çok betik kendi dinleyicisini kaydediyor (ortak betik + sağlayıcı);
  // mesaj hepsine verilir, ilgili olan işler.
  return (mesaj) =>
    new Promise((cozum) => {
      let cevap = false;
      for (const f of dinleyiciler) {
        f(mesaj, { tab: { id: 1, url: "https://sahte.test/app" } }, () => {
          cevap = true;
        });
      }
      cozum(cevap);
    });
}

function gonder(api, mesaj) {
  const g = api.mesajlar.find((m) => m && m.type === mesaj.type && m.id === mesaj.id);
  return g;
}

const bekle = (ms) => new Promise((r) => setTimeout(r, ms));

test("yazma execCommand'da tutmazsa yapıştırma/metin yedeği devreye girer", async () => {
  const { window, document, api } = sayfaKur(
    `<div contenteditable="true" id="k"></div>`,
    { dosyalar: ["content_common.js", "content_gemini.js"] }
  );
  // Gerçek sayfadaki davranış: execCommand çağrılır, hiçbir şey yazılmaz.
  document.execCommand = () => false;
  const kutu = document.querySelector("#k");
  kutu.addEventListener("paste", (e) => {
    const veri = e.clipboardData;
    const metin = veri && veri.getData ? veri.getData("text/plain") : "";
    if (metin) kutu.textContent = metin;
  });

  const sor = dinleyici(window);
  await sor({ type: "KERVAN_PROMPT", id: "t1", istem: "Merhaba dünya", dosyalar: [] });

  // Gönder düğmesi yok, Enter de işlemiyor: akış AÇIK hata ile bitmeli,
  // 120 sn sessiz beklemeye düşmemeli.
  for (let i = 0; i < 60 && !gonder(api, { type: "TAB_ERROR", id: "t1" }); i++) {
    await bekle(250);
  }
  const hata = gonder(api, { type: "TAB_ERROR", id: "t1" });
  expect(hata).toBeDefined();
  expect(hata.mesaj).toContain("istem gönderilemedi");
  // Yedek yol kutuya yazdığı için hata "yazılamadı" DEĞİL, "gönderilemedi" olmalı.
  expect(hata.mesaj).not.toContain("yazılamadı");
}, 40000);

test("yazma yollarının hepsi tutmazsa hata hangi yolu denediğini söyler", async () => {
  const { window, document, api } = sayfaKur(
    `<div contenteditable="true" id="k"></div>`,
    { dosyalar: ["content_common.js", "content_gemini.js"] }
  );
  document.execCommand = () => false;
  const kutu = document.querySelector("#k");
  // Editör her yazma girişimini reddediyor: içerik hiç oluşmuyor.
  Object.defineProperty(kutu, "innerText", { configurable: true, get: () => "" });
  Object.defineProperty(kutu, "textContent", {
    configurable: true,
    get: () => "",
    set: () => {}
  });

  const sor = dinleyici(window);
  await sor({ type: "KERVAN_PROMPT", id: "t2", istem: "Merhaba", dosyalar: [] });

  for (let i = 0; i < 60 && !gonder(api, { type: "TAB_ERROR", id: "t2" }); i++) {
    await bekle(250);
  }
  const hata = gonder(api, { type: "TAB_ERROR", id: "t2" });
  expect(hata).toBeDefined();
  expect(hata.mesaj).toContain("istem kutusuna yazılamadı");
  expect(hata.mesaj).toContain("execCommand");
  expect(hata.mesaj).toContain("yapistirma");
}, 40000);

test("gönderim sayfada iz bırakmazsa 'gönderildi' sanılmaz", async () => {
  const { window, document, api } = sayfaKur(
    `<div contenteditable="true" id="k"></div>
     <button aria-label="Send message" id="g">Gönder</button>`,
    { dosyalar: ["content_common.js", "content_gemini.js"] }
  );
  // Yazma tutuyor, gönderim tutmuyor; sayfada durdurma düğmesi de yanıt da yok.
  const kutu = document.querySelector("#k");
  kutu.addEventListener("paste", (e) => {
    const metin = e.clipboardData && e.clipboardData.getData("text/plain");
    if (metin) kutu.textContent = metin;
  });
  document.querySelector("#g").addEventListener("click", () => {});

  const sor = dinleyici(window);
  await sor({ type: "KERVAN_PROMPT", id: "t3", istem: "Merhaba", dosyalar: [] });
  for (let i = 0; i < 80 && !gonder(api, { type: "TAB_ERROR", id: "t3" }); i++) {
    await bekle(250);
  }
  const hata = gonder(api, { type: "TAB_ERROR", id: "t3" });
  expect(hata).toBeDefined();
  expect(hata.mesaj).toMatch(/istem gönderilemedi|sayfada iz yok/);
}, 40000);

test("ESKİ yanıt varken yeni yanıt gelmezse 'sayfada iz yok' ateşlenir", async () => {
  // 2026-10-24 incelemesi: denetim MUTLAK yanıt sayısına bakıyordu. Sohbette
  // tek bir eski yanıt varsa `yanit > 0` her zaman doğru olur, denetim hiç
  // beklemeden geçer ve "kutu boşaldı ama istek gitmedi" senaryosu tam olarak
  // NORMAL kullanımda yakalanmazdı. Ölçüt artık FARK: önceki yanıt sayısı
  // kaydedilir, yalnız bu turda eklenen yanıt iz sayılır.
  //
  // Bu test hem dalın gerçekten ateşlendiğini hem de fark hesabını ölçer:
  // sayfada 1 eski yanıt var, gönderim kutuyu boşaltıyor, yeni yanıt yok.
  const { window, document, api } = sayfaKur(
    `<model-response>eski yanıt</model-response>
     <div contenteditable="true" id="k"></div>
     <button aria-label="Send message" id="g">Gönder</button>`,
    { dosyalar: ["content_common.js", "content_gemini.js"] }
  );
  const kutu = document.querySelector("#k");
  kutu.addEventListener("paste", (e) => {
    const metin = e.clipboardData && e.clipboardData.getData("text/plain");
    if (metin) kutu.textContent = metin;
  });
  // Gönderim "başarılı" görünüyor (kutu boşalıyor) ama sayfada iz bırakmıyor:
  // Angular yeniden çizimi senaryosunun birebir kopyası.
  document.querySelector("#g").addEventListener("click", () => {
    kutu.textContent = "";
  });

  const sor = dinleyici(window);
  await sor({ type: "KERVAN_PROMPT", id: "t4", istem: "Merhaba", dosyalar: [] });
  for (let i = 0; i < 120 && !gonder(api, { type: "TAB_ERROR", id: "t4" }); i++) {
    await bekle(250);
  }
  const hata = gonder(api, { type: "TAB_ERROR", id: "t4" });
  expect(hata).toBeDefined();
  expect(hata.mesaj).toContain("sayfada iz yok");
  // Gönderim kapısı geçildi, arıza 3b'de yakalandı — "gönderilemedi" değil.
  expect(hata.mesaj).not.toContain("istem gönderilemedi");
  // Fark hesabının kanıtı: önceki ve şimdiki sayı hata metninde.
  expect(hata.mesaj).toContain("önce=1");
}, 45000);

test("yeni yanıt düğümü iz sayılır (fark hesabı yanlış alarm vermez)", async () => {
  // Karşı yön: fark hesabı "her zaman iz yok" dememeli. Bu turda yeni bir
  // yanıt düğümü belirdiğinde akış devam eder ve yanıt tamamlanır.
  const { window, document, api } = sayfaKur(
    `<div contenteditable="true" id="k"></div>
     <button aria-label="Send message" id="g">Gönder</button>`,
    { dosyalar: ["content_common.js", "content_gemini.js"] }
  );
  const kutu = document.querySelector("#k");
  kutu.addEventListener("paste", (e) => {
    const metin = e.clipboardData && e.clipboardData.getData("text/plain");
    if (metin) kutu.textContent = metin;
  });
  document.querySelector("#g").addEventListener("click", () => {
    kutu.textContent = "";
    const yanit = document.createElement("model-response");
    yanit.textContent = "gerçek yanıt";
    document.body.appendChild(yanit);
  });

  const sor = dinleyici(window);
  await sor({ type: "KERVAN_PROMPT", id: "t5", istem: "Merhaba", dosyalar: [] });
  for (let i = 0; i < 60 && !gonder(api, { type: "TAB_ERROR", id: "t5" }); i++) {
    if (api.mesajlar.some((m) => m && m.type === "STREAM_CHUNK" && m.id === "t5" && m.done)) break;
    await bekle(250);
  }
  const hata = gonder(api, { type: "TAB_ERROR", id: "t5" });
  expect(hata).toBeUndefined();
  const parcalar = api.mesajlar.filter((m) => m && m.type === "STREAM_CHUNK" && m.id === "t5");
  expect(parcalar.length).toBeGreaterThan(0);
  expect(parcalar.some((m) => m.done === true)).toBe(true);
}, 45000);
