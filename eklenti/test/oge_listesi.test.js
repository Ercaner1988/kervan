// KURAL EŞİTLİĞİ KAPISI (2026-10-24 incelemesi).
//
// Aynı okuma kuralı iki yerde yaşıyor ve yaşamak ZORUNDA: WebDriver havuzu
// yolu `crates/secim/src/lib.rs` içindeki `OGE_LISTESI` betiğini sayfaya
// enjekte eder, eklenti yolu ise `content_common.js`teki `kervanOgeOkuHam`ı
// çalıştırır. Kopyalar ayrıştığında bir yol "seçili" derken öbürü "değil"
// diyor ve arıza yalnız bir yolda kapanıyordu: aynı menüde '3.6 Düşünen'
// havuz yolunda işaretli sanılıp tıklama atlanıyor, eklenti yolunda doğru
// davranılıyordu.
//
// Bu test iki uygulamayı AYNI DOM üzerinde koşturup çıktıyı birebir
// karşılaştırır. Kural değişecekse iki dosya BİRLİKTE değişmeli.
import { test, expect } from "bun:test";
import { readFileSync } from "node:fs";
import { createContext, runInContext } from "node:vm";
import { parseHTML } from "linkedom";
import { sayfaKur } from "./harness.js";

const KAYNAK = readFileSync(new URL("../../crates/secim/src/lib.rs", import.meta.url), "utf8");
const BAS = 'const OGE_LISTESI: &str = r#"';
const bas = KAYNAK.indexOf(BAS);
const son = KAYNAK.indexOf('"#;', bas);
if (bas < 0 || son < 0) throw new Error("OGE_LISTESI sabiti lib.rs içinde bulunamadı");
const BETIK = KAYNAK.slice(bas + BAS.length, son);

// Havuz yolunun betiğini ayrı bir vm bağlamında, kendi `document`i ile koştur.
// Betik bir fonksiyon GÖVDESİ (sonunda `return` var, `arguments[0]` = seçici),
// bu yüzden fonksiyon ifadesine sarılır.
function havuzOku(document, secici) {
  const ctx = createContext({ document });
  const fn = runInContext(`(function () {${BETIK}\n})`, ctx);
  return JSON.parse(JSON.stringify(fn(secici)));
}

const SAYFALAR = {
  "tek işaret, sınıftan":
    `<div role="menuitem" class="mode selected"><span class="label">3.6 Flash</span></div>
     <div role="menuitem"><span class="label">3.1 Pro</span></div>`,
  "canlı çok işaretli menü":
    `<div role="menuitem" class="selected"><span class="label">3.6 Flash</span></div>
     <div role="menuitem"><span class="label">3.6 Düşünen</span><span aria-checked="true"></span></div>
     <div role="menuitem"><span class="label">3.1 Pro</span></div>`,
  "öznitelik ve alt düğüm kanıtları":
    `<div role="menuitem" aria-checked="true"><span class="label">A</span></div>
     <div role="menuitem"><span class="label">B</span><span aria-label="Seçildi">✓</span></div>
     <div role="menuitem" data-selected="true"><span class="label">C</span></div>`,
  "ikon satırı atlanır":
    `<div role="menuitem"><mat-icon>spark</mat-icon><div>3.6 Flash</div><div>Hızlı</div></div>`
};

for (const [ad, html] of Object.entries(SAYFALAR)) {
  test(`havuz betiği ile eklenti okuması aynı sonucu verir: ${ad}`, () => {
    const { window } = sayfaKur(html);
    const eklenti = window.kervanOgeOkuHam("[role=menuitem]");
    // İki okuma ayrı DOM'larda koşuyor (aynı HTML), çıktı birebir aynı olmalı.
    const { document } = parseHTML(`<!doctype html><html><body>${html}</body></html>`);
    const havuz = havuzOku(document, "[role=menuitem]");
    expect(havuz).toEqual(JSON.parse(JSON.stringify(eklenti)));
  });
}
