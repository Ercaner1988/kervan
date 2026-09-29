// Kervan WebExtension — z.ai Content Script
const api = typeof browser !== "undefined" ? browser : chrome;

console.log("[Kervan z.ai] Content script yüklendi:", window.location.href);

api.runtime.sendMessage({
  type: "TAB_REGISTER",
  provider: "z.ai"
}).catch(() => {});

api.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (message.type === "KERVAN_PROMPT") {
    zaiSoruSor(message.id, message.istem);
    sendResponse({ ok: true });
  }
});

async function zaiSoruSor(id, istem) {
  try {
    const kutuSecici = "textarea, div[contenteditable='true']";
    const kutu = await window.kervanOgeBekle(kutuSecici, 8000);
    if (!kutu) throw new Error("z.ai istem kutusu bulunamadı.");

    window.kervanMetinYaz(kutu, istem);

    await new Promise((r) => setTimeout(r, 400));
    let gonderBtn = document.querySelector("button[type='submit'], .send-btn, button:has(svg)");
    if (gonderBtn && !gonderBtn.disabled) {
      gonderBtn.click();
    } else {
      kutu.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", code: "Enter", keyCode: 13, which: 13, bubbles: true }));
    }

    await new Promise((r) => setTimeout(r, 1200));
    let sonMetin = "";
    let hareketsizSayac = 0;

    const observer = new MutationObserver(() => {
      const yanitlar = document.querySelectorAll(".chat-assistant, .markdown-prose, [class*='assistant']");
      if (yanitlar.length > 0) {
        const sonYanit = yanitlar[yanitlar.length - 1];
        const guncelMetin = sonYanit.innerText || sonYanit.textContent || "";
        if (guncelMetin.length > sonMetin.length) {
          const delta = guncelMetin.slice(sonMetin.length);
          sonMetin = guncelMetin;
          hareketsizSayac = 0;
          api.runtime.sendMessage({ type: "STREAM_CHUNK", id: id, delta: delta, done: false }).catch(() => {});
        }
      }
    });

    observer.observe(document.body, { childList: true, subtree: true, characterData: true });

    const bitisKontrol = setInterval(() => {
      hareketsizSayac++;
      const durdurBtn = document.querySelector("[class*='stop'], button[aria-label*='Stop']");
      if ((!durdurBtn && sonMetin.length > 0 && hareketsizSayac >= 2) || (hareketsizSayac >= 6 && sonMetin.length > 0)) {
        clearInterval(bitisKontrol);
        observer.disconnect();
        api.runtime.sendMessage({ type: "STREAM_CHUNK", id: id, delta: "", done: true }).catch(() => {});
      }
    }, 1000);

  } catch (err) {
    console.error("[Kervan z.ai] Hata:", err);
    api.runtime.sendMessage({ type: "TAB_ERROR", id: id, mesaj: err.message }).catch(() => {});
  }
}
