// Kervan WebExtension — Microsoft Copilot & M365 Content Script
const api = typeof browser !== "undefined" ? browser : chrome;

console.log("[Kervan Copilot] Content script yüklendi:", window.location.href);

api.runtime.sendMessage({
  type: "TAB_REGISTER",
  provider: "copilot"
}).catch(() => {});

api.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (message.type === "KERVAN_PROMPT") {
    copilotSoruSor(message.id, message.istem, message.dosyalar || []);
    sendResponse({ ok: true });
  }
});

function base64ToFile(base64Str, fileName, mimeType) {
  const safBase64 = base64Str.includes(",") ? base64Str.split(",")[1] : base64Str;
  const byteKarakterler = atob(safBase64);
  const byteSayilari = new Uint8Array(byteKarakterler.length);
  for (let i = 0; i < byteKarakterler.length; i++) {
    byteSayilari[i] = byteKarakterler.charCodeAt(i);
  }
  const blob = new Blob([byteSayilari], { type: mimeType });
  return new File([blob], fileName, { type: mimeType });
}

async function copilotSoruSor(id, istem, dosyalar = []) {
  try {
    const kutuSecici = "textarea[placeholder*='Copilot'], textarea[placeholder*='ileti'], textarea[data-testid='composer-input'], textarea, div[contenteditable='true'], [role='textbox']";
    const kutu = await window.kervanOgeBekle(kutuSecici, 8000);
    if (!kutu) throw new Error("Copilot istem kutusu bulunamadı.");

    // Ek dosyalar varsa ata
    if (dosyalar && dosyalar.length > 0) {
      const dataTransfer = new DataTransfer();
      for (const d of dosyalar) {
        try {
          dataTransfer.items.add(base64ToFile(d.veri_base64, d.ad, d.mime_tipi));
        } catch (e) {
          console.warn("[Kervan Copilot] Dosya dönüştürme hatası:", e);
        }
      }
      const fileInput = document.querySelector("input[type='file']");
      if (fileInput && dataTransfer.files.length > 0) {
        try {
          fileInput.files = dataTransfer.files;
          fileInput.dispatchEvent(new Event("change", { bubbles: true }));
        } catch (e) {}
      }
      try {
        kutu.dispatchEvent(new ClipboardEvent("paste", { clipboardData: dataTransfer, bubbles: true }));
      } catch (e) {}
      await new Promise((r) => setTimeout(r, 1200));
    }

    kutu.focus();
    if (kutu.isContentEditable) {
      document.execCommand("selectAll", false, null);
      document.execCommand("insertText", false, istem);
      kutu.dispatchEvent(new Event("input", { bubbles: true }));
    } else {
      window.kervanMetinYaz(kutu, istem);
    }

    await new Promise((r) => setTimeout(r, 400));
    let gonderBtn = document.querySelector("button[aria-label*='Gönder'], button[aria-label*='Send'], button[aria-label*='Submit'], button[data-testid='send-button'], button[type='submit']");
    if (gonderBtn && !gonderBtn.disabled) {
      gonderBtn.click();
    } else {
      kutu.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", code: "Enter", keyCode: 13, which: 13, bubbles: true }));
    }

    await new Promise((r) => setTimeout(r, 1200));
    let sonMetin = "";
    let hareketsizSayac = 0;

    const observer = new MutationObserver(() => {
      const yanitlar = document.querySelectorAll("[data-testid='ai-message-body'], [data-testid='markdown-reply'], .ac-textBlock, [data-content='ai-message'], div[class*='message-content'], div.space-y-3, div[class*='speech-bubble']");
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
      const durdurBtn = document.querySelector("button[aria-label*='Stop']");
      if ((!durdurBtn && sonMetin.length > 0 && hareketsizSayac >= 2) || (hareketsizSayac >= 6 && sonMetin.length > 0)) {
        clearInterval(bitisKontrol);
        observer.disconnect();
        api.runtime.sendMessage({ type: "STREAM_CHUNK", id: id, delta: "", done: true }).catch(() => {});
      }
    }, 1000);

  } catch (err) {
    console.error("[Kervan Copilot] Hata:", err);
    api.runtime.sendMessage({ type: "TAB_ERROR", id: id, mesaj: err.message }).catch(() => {});
  }
}
