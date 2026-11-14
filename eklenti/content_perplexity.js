// Kervan WebExtension — Perplexity Content Script
//
// NEDEN YAZARAK GÖNDERMİYORUZ (2026-11-14, canlı ölçüm):
// Perplexity'nin istem kutusu bir Lexical editörüdür
// (`DIV[contenteditable=true][data-lexical-editor]`). Lexical metni kendi sanal
// durumunda tutar ve DOM'a yapılan her yazımı bir sonraki çizimde siler.
// Üç yol denendi, üçü de kutuyu boş bıraktı (canlı hata metni):
//   execCommand("insertText")   → kutu bos kaldi
//   ClipboardEvent + DataTransfer → kutu bos kaldi (isTrusted=false yok sayılır)
//   textContent + beforeinput/input → kutu bos kaldi
// Sentetik klavye olayları da (`isTrusted=false`) Lexical'de yok sayılıyor.
//
// ÇÖZÜM: Perplexity istemi `?q=` sorgu parametresiyle KENDİSİ gönderiyor.
// Canlı ölçüm (2026-11-14, `.prose` ile doğrulandı):
//   https://www.perplexity.ai/?q=<enc>   → yanıt geldi ("KERVAN-ROOT2")
//   https://www.perplexity.ai/search?q=<enc> → ÇALIŞMIYOR: `/search/new?q=`
//     adresine yönleniyor, sayfa istemi almıyor (`.prose` 0, kutu boş).
// Yani kutuya yazmaya hiç gerek yok; sayfa yüklenirken istemi kendisi işliyor.
//
// Navigasyon bu betiği öldürdüğü için istek `storage.local` üzerinden taşınır:
//   1. İstem gelir  → `{id, istem, zaman}` depoya yazılır, sayfa `?q=` adresine
//      yönlendirilir.
//   2. Yeni sayfada betik yeniden yüklenir → depoda bekleyen istek varsa alınır
//      ve yanıt `.prose` üzerinde gözlenir (deltalar `STREAM_CHUNK` ile akar).
// Bekleyen kayıt 10 dakikadan eskiyse sessizce atılır (yarım kalmış navigasyon
// artığı bir sonraki soruya karışmasın).

const api = typeof browser !== "undefined" ? browser : chrome;

console.log("[Kervan Perplexity] Content script yüklendi:", window.location.href);

api.runtime.sendMessage({
  type: "TAB_REGISTER",
  provider: "perplexity"
}).catch(() => {});

const YANIT_SECICI = ".prose";
const DURDUR_SECICI = "button[aria-label*='Stop'], button[aria-label*='Durdur']";
// UYARI (2026-11-14, canlı arıza): `/search?q=` ÇALIŞMIYOR. Perplexity onu
// `/search/new?q=...` adresine yönlendiriyor ve o sayfa boş bir "yeni sohbet"
// ekranı: istem URL'den alınmıyor, `.prose` hiç oluşmuyor, kutu boş kalıyor
// (canlı ölçüm: `.prose` öğe=0, `[contenteditable]` 1 ve boş).
// Kök yol çalışıyor: `/?q=<enc>` tam sayfa yüklemesinde istemi gönderiyor
// (canlı: `?q=KERVAN-ROOT2` → `.prose` 1, yanıt metni geldi).
// Bu yüzden adres KÖK yoldur; `/search` eklenirse arıza geri gelir.
const SOR_ADRESI = "https://www.perplexity.ai/?q=";
const BEKLEYEN_ANAHTAR = "kervanBekleyenPerplexity";
const BAYAT_MS = 10 * 60 * 1000;
// İstem gönderildikten sonra ilk yanıt metni için üst sınır.
const ILK_METIN_MS = 45_000;
// Hiç metin gelmezse tamamen vazgeçme süresi.
const EN_UZUN_BEKLEME_MS = 150_000;

// ---------------------------------------------------------------------------
// Depo: `storage.local` varsa onu kullanır, yoksa (test koşucusu) belleğe yazar.
// ---------------------------------------------------------------------------
const bellekDepo = new Map();

function depoVarMi() {
  return Boolean(api && api.storage && api.storage.local);
}

// UYARI (2026-11-14, canlı arıza): Firefox/Zen `browser.*` API'si PROMISE döner
// ve callback argümanını HİÇ ÇAĞIRMAZ. Yalnız callback'e güvenmek `await`i
// sonsuza kadar bekletiyordu: istem depoya yazılmış sayılıyor, `location.assign`
// hiç koşmuyor, sunucu 120 sn zaman aşımına düşüyordu. Bu yüzden: promise
// dönerse promise beklenir, dönmezse (Chrome `chrome.*`) callback beklenir.
function depoYaz(anahtar, deger) {
  if (!depoVarMi()) {
    bellekDepo.set(anahtar, deger);
    return Promise.resolve();
  }
  const nesne = { [anahtar]: deger };
  try {
    const donus = typeof api.storage.local.set === "function"
      ? api.storage.local.set(nesne)
      : undefined;
    if (donus && typeof donus.then === "function") {
      return donus.then(() => {}, () => {});
    }
  } catch {
    /* callback yoluna düş */
  }
  return new Promise((coz) => {
    try {
      api.storage.local.set(nesne, () => coz());
    } catch {
      coz();
    }
  });
}

function depoOku(anahtar) {
  if (!depoVarMi()) return Promise.resolve(bellekDepo.get(anahtar));
  const anahtarlar = [anahtar];
  try {
    const donus = typeof api.storage.local.get === "function"
      ? api.storage.local.get(anahtarlar)
      : undefined;
    if (donus && typeof donus.then === "function") {
      return donus.then((s) => (s ? s[anahtar] : undefined), () => undefined);
    }
  } catch {
    /* callback yoluna düş */
  }
  return new Promise((coz) => {
    try {
      api.storage.local.get(anahtarlar, (sonuc) => coz(sonuc ? sonuc[anahtar] : undefined));
    } catch {
      coz(undefined);
    }
  });
}

function depoSil(anahtar) {
  if (!depoVarMi()) {
    bellekDepo.delete(anahtar);
    return Promise.resolve();
  }
  const anahtarlar = [anahtar];
  try {
    const donus = typeof api.storage.local.remove === "function"
      ? api.storage.local.remove(anahtarlar)
      : undefined;
    if (donus && typeof donus.then === "function") {
      return donus.then(() => {}, () => {});
    }
  } catch {
    /* callback yoluna düş */
  }
  return new Promise((coz) => {
    try {
      api.storage.local.remove(anahtarlar, () => coz());
    } catch {
      coz();
    }
  });
}

// ---------------------------------------------------------------------------
// Yanıt gözlemi: `.prose` son öğesini izler, deltaları `STREAM_CHUNK` ile yollar.
// ---------------------------------------------------------------------------
function yanitGozle(id) {
  // `sonMetin`: son görülen sayfa metni (değişiklik sezme).
  // `gonderilen`: istemciye GİTMİŞ toplam metin. Delta tabanı BUDUR.
  let sonMetin = "";
  let gonderilen = "";
  let sonOge = null;
  let hareketsizSayac = 0;
  const baslangic = Date.now();

  // UYARI (2026-11-14, canlı arıza): üç tuzak burada.
  // (1) Perplexity her turda YENİ bir `.prose` bloğu ekliyor. Yalnız
  //     `length-1` öğesini izlemek, öğe değişince uzunluğu eski öğenin
  //     metniyle karşılaştırır → delta hiç gitmez, tur 2 sn sonra "bitti"
  //     sanılır (canlı: yanıt 89 baytta kesildi).
  // (2) Markdown yeniden biçimlenirken metin KISALABİLİR (ölçüm: 746→734).
  //     Kısalmada delta üretilmez, yalnız taban güncellenir.
  // (3) TEKRAR KUSURU (2026-11-14, Claude Code turu): delta tabanı olarak
  //     son görülen metni kullanmak YETMEZ. Kısalma tabanı aşağı çekince,
  //     sonraki büyümede zaten gönderilmiş metin baştan yeniden dilimleniyor
  //     ve istemcide çift metin çıkıyordu — canlı: `Birden çokBirden çok`,
  //     `Bu\n\nBu`. Delta tabanı artık `gonderilen`; o yalnız İLERİ gider.
  const ilerle = () => {
    const yanitlar = document.querySelectorAll(YANIT_SECICI);
    if (yanitlar.length === 0) return;
    const sonYanit = yanitlar[yanitlar.length - 1];
    const guncelMetin = sonYanit.innerText || sonYanit.textContent || "";
    if (sonYanit !== sonOge) {
      // Yeni yanıt bloğu: tabanı sıfırla, tamamını delta olarak yolla.
      sonOge = sonYanit;
      sonMetin = "";
      gonderilen = "";
    }
    if (guncelMetin.length > gonderilen.length && guncelMetin.startsWith(gonderilen)) {
      // Normal yol: gönderilen metnin üstüne eklenen kuyruk.
      const delta = guncelMetin.slice(gonderilen.length);
      gonderilen = guncelMetin;
      sonMetin = guncelMetin;
      hareketsizSayac = 0;
      api.runtime
        .sendMessage({ type: "STREAM_CHUNK", id: id, delta: delta, done: false })
        .catch(() => {});
    } else if (guncelMetin !== sonMetin) {
      // İki hal: (a) kısalma, (b) gönderilmiş önekin ORTASI değişti.
      // İkisinde de geri alma protokolü yok. TEKRAR üretmemek için `gonderilen`
      // yalnız büyür; (b)'de değişen orta kısım istemcide eski haliyle kalır —
      // kabul edilen kayıp, çift metinden yeğdir.
      sonMetin = guncelMetin;
      hareketsizSayac = 0;
    }
  };

  // `MutationObserver` olmayan ortamda (test DOM'u) yoklamaya düşülür.
  let observer = null;
  if (typeof MutationObserver === "function") {
    observer = new MutationObserver(ilerle);
    observer.observe(document.body, { childList: true, subtree: true, characterData: true });
  }

  const bitisKontrol = setInterval(() => {
    if (window.kervanDurdurulduMu && window.kervanDurdurulduMu(id)) {
      clearInterval(bitisKontrol);
      if (observer) observer.disconnect();
      return;
    }
    ilerle();
    hareketsizSayac++;

    // Sessiz çıkmaz: metin hiç gelmezse 120 sn beklemek yerine kanıtla dur.
    if (sonMetin.length === 0 && Date.now() - baslangic > ILK_METIN_MS) {
      clearInterval(bitisKontrol);
      if (observer) observer.disconnect();
      api.runtime
        .sendMessage({
          type: "TAB_ERROR",
          id: id,
          mesaj:
            `istem gönderildi ama ${ILK_METIN_MS / 1000} sn içinde yanıt metni okunamadı — ` +
            `adres=${window.location.href} — ` +
            `'${YANIT_SECICI}' öğe=${document.querySelectorAll(YANIT_SECICI).length}, ` +
            `durdurma düğmesi=${document.querySelector(DURDUR_SECICI) ? "var" : "yok"}`
        })
        .catch(() => {});
      return;
    }
    if (sonMetin.length === 0 && Date.now() - baslangic > EN_UZUN_BEKLEME_MS) {
      clearInterval(bitisKontrol);
      if (observer) observer.disconnect();
      return;
    }

    // UYARI (2026-11-14, canlı ölçüm): Perplexity akışı DÜZENSİZ — metin
    // 1377 karakterde ~10 sn bekleyip 1997'ye sıçradı. 2 sn hareketsizliği
    // bitiş saymak yanıttı ortasından kesiyordu. Durdurma düğmesi yokken
    // 6 sn, varken 12 sn sessizlik bitiş sayılır.
    const durdurBtn = document.querySelector(DURDUR_SECICI);
    if ((!durdurBtn && sonMetin.length > 0 && hareketsizSayac >= 6) ||
        (hareketsizSayac >= 12 && sonMetin.length > 0)) {
      clearInterval(bitisKontrol);
      if (observer) observer.disconnect();
      api.runtime
        .sendMessage({ type: "STREAM_CHUNK", id: id, delta: "", done: true })
        .catch(() => {});
    }
  }, 1000);
}

// ---------------------------------------------------------------------------
// Yeni istem: depoya yaz, sayfayı `?q=` adresine yönlendir.
// Zaten bir cevap sayfasındaysak ve aynı istem tekrar gelirse yönlendirme
// yapılmaz; doğrudan gözleme geçilir.
// ---------------------------------------------------------------------------
async function perplexitySoruSor(id, istem) {
  try {
    const adres = SOR_ADRESI + encodeURIComponent(istem);
    await depoYaz(BEKLEYEN_ANAHTAR, { id: id, istem: istem, zaman: Date.now() });
    console.log("[Kervan Perplexity] istem ?q= yoluna devrediliyor:", adres.length, "kr");

    if (typeof window.location.assign === "function") {
      window.location.assign(adres);
    } else {
      window.location.href = adres;
    }
  } catch (err) {
    console.error("[Kervan Perplexity] Hata:", err);
    api.runtime.sendMessage({ type: "TAB_ERROR", id: id, mesaj: err.message }).catch(() => {});
  }
}

// ---------------------------------------------------------------------------
// Açılışta bekleyen istek var mı? Varsa (bu sayfa `?q=` sonucudur) gözleme başla.
// ---------------------------------------------------------------------------
async function bekleyeniIsle() {
  const bekleyen = await depoOku(BEKLEYEN_ANAHTAR);
  if (!bekleyen || typeof bekleyen !== "object") return;
  if (!bekleyen.id || typeof bekleyen.zaman !== "number") {
    await depoSil(BEKLEYEN_ANAHTAR);
    return;
  }
  const yas = Date.now() - bekleyen.zaman;
  if (yas > BAYAT_MS) {
    await depoSil(BEKLEYEN_ANAHTAR);
    console.log("[Kervan Perplexity] bayat bekleyen istek atıldı:", bekleyen.id);
    return;
  }
  await depoSil(BEKLEYEN_ANAHTAR);
  console.log(
    "[Kervan Perplexity] bekleyen istek devralındı:",
    bekleyen.id,
    "yaş=" + Math.round(yas / 1000) + "sn"
  );
  // Sayfanın istemi işlemeye başlaması için kısa bir nefes; gözlem zaten
  // metin gelene kadar bekliyor.
  yanitGozle(bekleyen.id);
}

api.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (message && message.type === "KERVAN_PROMPT") {
    perplexitySoruSor(message.id, message.istem);
    sendResponse({ ok: true });
  }
});

bekleyeniIsle().catch((e) => {
  console.error("[Kervan Perplexity] bekleyen istek okunamadı:", e);
});
