// Kervan WebExtension — Mistral (Le Chat) Content Script
const api = typeof browser !== "undefined" ? browser : chrome;

console.log("[Kervan Mistral] Content script yüklendi:", window.location.href);

api.runtime.sendMessage({
  type: "TAB_REGISTER",
  provider: "mistral"
}).catch(() => {});

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

api.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (message.type === "KERVAN_PROMPT") {
    mistralSoruSor(message.id, message.istem, message.dosyalar || []);
    sendResponse({ ok: true });
  }
});

async function mistralSoruSor(id, istem, dosyalar = []) {
  try {
    const kutuSecici = "textarea, div[contenteditable='true']";
    const kutu = await window.kervanOgeBekle(kutuSecici, 8000);
    if (!kutu) throw new Error("Mistral istem kutusu bulunamadı.");

    // Ek dosyalar varsa Mistral'e yükle (PDF, DOCX, TXT, Resim vb.)
    if (dosyalar && dosyalar.length > 0) {
      console.log(`[Kervan Mistral] ${dosyalar.length} adet dosya yükleniyor...`);
      const dataTransfer = new DataTransfer();

      for (const d of dosyalar) {
        try {
          const fileObj = base64ToFile(d.veri_base64, d.ad, d.mime_tipi);
          dataTransfer.items.add(fileObj);
        } catch (e) {
          console.warn("[Kervan Mistral] Dosya dönüştürme hatası:", d.ad, e);
        }
      }

      if (dataTransfer.files.length > 0) {
        // A) Mistral dosya input'unu bul ve ata
        const fileInput = document.querySelector("input[type='file']");
        if (fileInput) {
          try {
            fileInput.files = dataTransfer.files;
            fileInput.dispatchEvent(new Event("change", { bubbles: true }));
            fileInput.dispatchEvent(new Event("input", { bubbles: true }));
          } catch (e) {
            console.warn("[Kervan Mistral] fileInput ata hatası:", e);
          }
        }

        // B) Drop & Paste simülasyonu
        try {
          kutu.dispatchEvent(new DragEvent("drop", { dataTransfer, bubbles: true }));
          kutu.dispatchEvent(new ClipboardEvent("paste", { clipboardData: dataTransfer, bubbles: true }));
        } catch (e) {
          console.warn("[Kervan Mistral] drop/paste hatası:", e);
        }

        // Yükleme için kısa bekleme
        await new Promise((r) => setTimeout(r, 1500));
      }
    }

    window.kervanMetinYaz(kutu, istem);

    await new Promise((r) => setTimeout(r, 400));
    let gonderBtn = document.querySelector("button[type='submit'], button[aria-label*='Send'], button:has(svg)");
    if (gonderBtn && !gonderBtn.disabled) {
      gonderBtn.click();
    } else {
      kutu.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", code: "Enter", keyCode: 13, which: 13, bubbles: true }));
    }

    await new Promise((r) => setTimeout(r, 1200));
    let sonMetin = "";
    let hareketsizSayac = 0;

    const observer = new MutationObserver(() => {
      const yanitlar = document.querySelectorAll(".markdown, [class*='prose'], [data-testid*='chat-message']");
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
      const durdurBtn = document.querySelector("button[aria-label*='Stop'], [class*='stop']");
      if ((!durdurBtn && sonMetin.length > 0 && hareketsizSayac >= 2) || (hareketsizSayac >= 6 && sonMetin.length > 0)) {
        clearInterval(bitisKontrol);
        observer.disconnect();
        api.runtime.sendMessage({ type: "STREAM_CHUNK", id: id, delta: "", done: true }).catch(() => {});
      }
    }, 1000);

  } catch (err) {
    console.error("[Kervan Mistral] Hata:", err);
    api.runtime.sendMessage({ type: "TAB_ERROR", id: id, mesaj: err.message }).catch(() => {});
  }
}
