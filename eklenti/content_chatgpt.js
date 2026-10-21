// Kervan WebExtension — ChatGPT / OpenAI Codex Content Script
const api = typeof browser !== "undefined" ? browser : chrome;

console.log("[Kervan ChatGPT] Content script yüklendi:", window.location.href);

api.runtime.sendMessage({
  type: "TAB_REGISTER",
  provider: "chatgpt"
}).catch(() => {});

api.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (message.type === "KERVAN_PROMPT") {
    chatgptSoruSor(message.id, message.istem);
    sendResponse({ ok: true });
  }
});

async function chatgptSoruSor(id, istem) {
  try {
    const kutuSecici = "#prompt-textarea, textarea, div[contenteditable='true']";
    const kutu = await window.kervanOgeBekle(kutuSecici, 8000);
    if (!kutu) throw new Error("ChatGPT istem kutusu bulunamadı.");

    kutu.focus();
    if (kutu.isContentEditable) {
      document.execCommand("selectAll", false, null);
      document.execCommand("insertText", false, istem);
      kutu.dispatchEvent(new Event("input", { bubbles: true }));
    } else {
      window.kervanMetinYaz(kutu, istem);
    }

    await new Promise((r) => setTimeout(r, 400));
    let gonderBtn = document.querySelector("button[data-testid='send-button']") ||
                    document.querySelector("button[aria-label*='Send']") ||
                    document.querySelector("button[aria-label*='Gönder']");

    if (gonderBtn && !gonderBtn.disabled) {
      gonderBtn.click();
    } else {
      kutu.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", code: "Enter", keyCode: 13, which: 13, bubbles: true }));
    }

    await new Promise((r) => setTimeout(r, 1200));
    let sonMetin = "";
    let hareketsizSayac = 0;

    const observer = new MutationObserver(() => {
      const yanitlar = document.querySelectorAll("div[data-message-author-role='assistant'], .markdown");
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
      if (window.kervanDurdurulduMu && window.kervanDurdurulduMu(id)) {
        clearInterval(bitisKontrol);
        observer.disconnect();
        return;
      }
      hareketsizSayac++;
      const durdurBtn = document.querySelector("button[data-testid='stop-button'], [data-testid='stop-button']");
      if ((!durdurBtn && sonMetin.length > 0 && hareketsizSayac >= 2) || (hareketsizSayac >= 6 && sonMetin.length > 0)) {
        clearInterval(bitisKontrol);
        observer.disconnect();
        api.runtime.sendMessage({ type: "STREAM_CHUNK", id: id, delta: "", done: true }).catch(() => {});
      }
    }, 1000);

  } catch (err) {
    console.error("[Kervan ChatGPT] Hata:", err);
    api.runtime.sendMessage({ type: "TAB_ERROR", id: id, mesaj: err.message }).catch(() => {});
  }
}
