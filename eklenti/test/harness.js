// Sahte sayfa koşucusu: content script'lerini gerçek dosyadan yükler ve
// linkedom DOM'u üzerinde çalıştırır. Tarayıcı/hesap gerekmez.
import { parseHTML } from "linkedom";
import { readFileSync } from "node:fs";
import { createContext, runInContext } from "node:vm";

// MutationObserver linkedom'da YOK. Sadece "menü belirdi mi" sayacı için
// kullanıldığından, gerçek gözlemci yerine kısa aralıklı yoklama yapan bir
// vekil veriyoruz; test edilen davranış (bekleme/strateji sırası) aynı kalır.
class YoklayanGozcu {
  constructor(geri) {
    this.geri = geri;
    this.zaman = null;
  }
  observe() {
    this.zaman = setInterval(() => this.geri([]), 25);
    if (this.zaman.unref) this.zaman.unref();
  }
  disconnect() {
    if (this.zaman) clearInterval(this.zaman);
    this.zaman = null;
  }
}

function sohbet(paylasilanDepo) {
  const mesajlar = [];
  // `storage.local` gerçek tarayıcıda sekmeler ve yeniden yüklemeler arasında
  // yaşar; content script navigasyonda öldüğü için istem buradan taşınır.
  // Testte de aynı sözleşme kurulur: `secenekler.depo` verilirse o harita
  // paylaşılır, böylece "yeni sayfa" aynı depoyu görür.
  const depo = paylasilanDepo || new Map();
  return {
    mesajlar,
    depo,
    runtime: {
      onMessage: { addListener: (f) => mesajlar.push(f) },
      sendMessage: async (m) => {
        mesajlar.push(m);
        return { ok: true };
      }
    },
    storage: {
      local: {
        set: (nesne, geri) => {
          for (const [k, v] of Object.entries(nesne)) depo.set(k, v);
          if (geri) geri();
        },
        get: (anahtarlar, geri) => {
          const sonuc = {};
          for (const k of anahtarlar) if (depo.has(k)) sonuc[k] = depo.get(k);
          if (geri) geri(sonuc);
        },
        remove: (anahtarlar, geri) => {
          for (const k of anahtarlar) depo.delete(k);
          if (geri) geri();
        }
      }
    }
  };
}

/**
 * Sahte sayfa kurar.
 * @param {string} html  body içeriği
 * @param {object} secenekler { dosyalar: string[] } yüklenecek content script'leri
 */
export function sayfaKur(html, secenekler = {}) {
  const { window, document } = parseHTML(
    `<!doctype html><html><body>${html}</body></html>`
  );

  const api = sohbet(secenekler.depo);
  // linkedom, tarayıcının olay sınıflarının ve elementFromPoint'in tamamını
  // sağlamıyor. Testin ölçtüğü şey DOM mantığı olduğu için eksikler sade
  // vekillerle kapatılır; kapatılamayan yetenek testte AÇIKÇA belirtilir.
  const EventSinifi = window.Event;
  const olay = (ad) =>
    window[ad] ||
    class extends EventSinifi {
      constructor(tur, secenekler = {}) {
        super(tur, secenekler);
        Object.assign(this, secenekler);
      }
    };
  if (typeof window.document.elementFromPoint !== "function") {
    window.document.elementFromPoint = () => null;
  }

  // `location` atanabilir olmalı: Perplexity yolu istemi `?q=` ile taşıyor.
  // Gerçek tarayıcıda `assign` sayfayı değiştirir; testte hedef adres
  // `window.gidilen` altında birikir, böylece "hangi adrese gidildi" ölçülür.
  const konum = {
    href: "https://www.perplexity.ai/",
    host: "www.perplexity.ai",
    hostname: "www.perplexity.ai",
    assign: (adres) => {
      konum.href = adres;
      ctx.gidilen.push(adres);
    },
    replace: (adres) => {
      konum.href = adres;
      ctx.gidilen.push(adres);
    }
  };

  const ctx = {
    window,
    document,
    location: konum,
    gidilen: [],
    console,
    setTimeout,
    clearTimeout,
    setInterval,
    clearInterval,
    Promise,
    Event: window.Event,
    KeyboardEvent: olay("KeyboardEvent"),
    MouseEvent: olay("MouseEvent"),
    PointerEvent: olay("PointerEvent"),
    ClipboardEvent: olay("ClipboardEvent"),
    DataTransfer: window.DataTransfer || class {},
    MutationObserver: YoklayanGozcu,
    chrome: api,
    browser: api,
    fetch: async () => ({ json: async () => ({ token: "test" }) })
  };
  ctx.globalThis = ctx;
  ctx.window = ctx; // content script'ler window.X üzerinden yazıyor
  ctx.self = ctx;

  const dosyalar = secenekler.dosyalar || ["content_common.js"];
  for (const ad of dosyalar) {
    const kaynak = readFileSync(new URL(`../${ad}`, import.meta.url), "utf8");
    runInContext(kaynak, createContext(ctx), { filename: ad });
  }

  return { window: ctx, document, api };
}

/**
 * Bir öğenin `innerText`ini elle kurar. linkedom innerText'i düz metin
 * verir (satır sonu yok); gerçek tarayıcı satır satır verir. Satır mantığını
 * sınayan testler bu yardımcıyı kullanmalı.
 */
export function satirla(el, metin) {
  Object.defineProperty(el, "innerText", {
    configurable: true,
    get: () => metin
  });
  return el;
}
