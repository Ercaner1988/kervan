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

function sohbet() {
  const mesajlar = [];
  return {
    mesajlar,
    runtime: {
      onMessage: { addListener: (f) => mesajlar.push(f) },
      sendMessage: async (m) => {
        mesajlar.push(m);
        return { ok: true };
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

  const api = sohbet();
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

  const ctx = {
    window,
    document,
    location: { href: "https://sahte.test/app", host: "sahte.test", hostname: "sahte.test" },
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
