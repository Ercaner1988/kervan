// Profil kimliği testleri (2026-10-24).
//
// NEDEN: yedi Gemini hesabı ayrı Zen profillerinde çalışıyor ama hepsi aynı
// `cookieStoreId`yi ("firefox-default") bildiriyor. Kervan'da anahtar
// `provider:profile` olduğu için hepsi TEK kayda çöküyordu. Kimlik dosyası
// (`kervan-profil.json`) bu çökmeyi kaldırır; burada kuralı sabitliyoruz:
// dosya varsa dosya kazanır, yoksa yedeğe (cookieStoreId) düşülür.
import { test, expect } from "bun:test";
import { readFileSync } from "node:fs";
import { createContext, runInContext } from "node:vm";

const KAYNAK = readFileSync(new URL("../../eklenti/kimlik.js", import.meta.url), "utf8");

// Sahte tarayıcı: `fetch` yalnız kimlik dosyası adresini tanır; sayaç, dosyanın
// kaç kez okunduğunu gösterir (önbellek iddiasının kanıtı).
function kur(dosya) {
  const sayac = { fetch: 0 };
  const sandbox = {
    console,
    fetch: async () => {
      sayac.fetch++;
      if (dosya === null) return { ok: false, status: 404 };
      return { ok: true, json: async () => dosya };
    },
    browser: { runtime: { getURL: (y) => `moz-extension://test/${y}` } }
  };
  sandbox.window = sandbox;
  const baglam = createContext(sandbox);
  runInContext(KAYNAK, baglam);
  return { window: sandbox, sayac };
}

test("kimlik dosyası varsa hesap adı döner ve bir kez okunur", async () => {
  const { window, sayac } = kur({ ad: "ercaner1988@gmail.com" });
  expect(await window.kervanProfilKimligi("firefox-default")).toBe("ercaner1988@gmail.com");
  expect(await window.kervanProfilKimligi("firefox-default")).toBe("ercaner1988@gmail.com");
  expect(sayac.fetch).toBe(1);
});

test("dosya yoksa cookieStoreId yedeğine düşer", async () => {
  const { window } = kur(null);
  expect(await window.kervanProfilKimligi("firefox-container-3")).toBe("firefox-container-3");
});

test("yedek değer önbelleklenmez: konteynerler kendi kimliğini korur", async () => {
  const { window, sayac } = kur(null);
  expect(await window.kervanProfilKimligi("firefox-container-1")).toBe("firefox-container-1");
  expect(await window.kervanProfilKimligi("firefox-container-2")).toBe("firefox-container-2");
  expect(sayac.fetch).toBe(1); // dosya yokluğu da önbelleklenir, yedek değil
});

test("boş ya da bozuk ad yedeğe düşer", async () => {
  const { window } = kur({ ad: "   " });
  expect(await window.kervanProfilKimligi("firefox-default")).toBe("firefox-default");
  const ikinci = kur({ ad: 42 });
  expect(await ikinci.window.kervanProfilKimligi("varsayilan")).toBe("varsayilan");
});

test("yedek de verilmezse 'varsayilan'", async () => {
  const { window } = kur(null);
  expect(await window.kervanProfilKimligi(null)).toBe("varsayilan");
});
