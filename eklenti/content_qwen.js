// Kervan WebExtension — Qwen Content Script
const api = typeof browser !== "undefined" ? browser : chrome;

console.log("[Kervan Qwen] Content script yüklendi:", window.location.href);

// Sekmeyi Kervan'a kaydet
api.runtime.sendMessage({
  type: "TAB_REGISTER",
  provider: "qwen"
}).catch(() => {});

// Periyodik Cloudflare / Turnstile kontrolü
setInterval(() => {
  if (window.kervanGuvenlikKontrolu) {
    window.kervanGuvenlikKontrolu("qwen");
  }
}, 5000);

api.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (message.type === "KERVAN_PROMPT") {
    qwenSoruSor(message.id, message.istem);
    sendResponse({ ok: true });
  }
});

async function qwenSoruSor(id, istem) {
  try {
    // 1. İstem kutusunu bul
    const kutuSecici = "textarea, div[contenteditable='true'], [contenteditable]";
    const kutu = await window.kervanOgeBekle(kutuSecici, 8000);
    if (!kutu) {
      throw new Error("Qwen istem kutusu bulunamadı.");
    }

    // 2. Metni insan gibi yaz
    window.kervanMetinYaz(kutu, istem);

    // 3. Gönder butonunu bul ve tıkla
    await new Promise((r) => setTimeout(r, 400));
    let gonderBtn = document.querySelector("button[type='submit']") ||
                    document.querySelector("button.send-btn") ||
                    document.querySelector("[data-testid='send-button']") ||
                    kutu.closest("form")?.querySelector("button");

    if (gonderBtn && !gonderBtn.disabled) {
      gonderBtn.click();
    } else {
      // Buton bulunamazsa Enter simüle et
      kutu.dispatchEvent(new KeyboardEvent("keydown", {
        key: "Enter",
        code: "Enter",
        keyCode: 13,
        which: 13,
        bubbles: true,
        cancelable: true
      }));
    }

    // 4. Yanıtı canlı gözlemle (MutationObserver)
    await new Promise((r) => setTimeout(r, 1000));
    let sonMetin = "";
    let hareketsizSayac = 0;

    const observer = new MutationObserver(() => {
      // Qwen yanıt balonları
      const yanitlar = document.querySelectorAll(".markdown-content, .qwen-markdown, [class*='chat-message-assistant'], .chat-content");
      if (yanitlar.length > 0) {
        const sonYanit = yanitlar[yanitlar.length - 1];
        const guncelMetin = sonYanit.innerText || sonYanit.textContent || "";
        
        if (guncelMetin.length > sonMetin.length) {
          const delta = guncelMetin.slice(sonMetin.length);
          sonMetin = guncelMetin;
          hareketsizSayac = 0;

          api.runtime.sendMessage({
            type: "STREAM_CHUNK",
            id: id,
            delta: delta,
            done: false
          }).catch(() => {});
        }
      }
    });

    observer.observe(document.body, { childList: true, subtree: true, characterData: true });

    // Üretimin bitişini denetleyen sayaç
    const bitisKontrol = setInterval(() => {
      if (window.kervanDurdurulduMu && window.kervanDurdurulduMu(id)) {
        clearInterval(bitisKontrol);
        observer.disconnect();
        return;
      }
      hareketsizSayac++;

      // Stop/Durdur butonu kaybolduysa ve metin varsa veya 4 saniyedir yeni token gelmediyse
      const durdurBtn = document.querySelector("[class*='stop'], button[aria-label*='Stop'], button[aria-label*='Durdur']");
      if ((!durdurBtn && sonMetin.length > 0 && hareketsizSayac >= 2) || (hareketsizSayac >= 6 && sonMetin.length > 0)) {
        clearInterval(bitisKontrol);
        observer.disconnect();

        console.log("[Kervan Qwen] Yanıt tamamlandı, toplam uzunluk:", sonMetin.length);
        api.runtime.sendMessage({
          type: "STREAM_CHUNK",
          id: id,
          delta: "",
          done: true
        }).catch(() => {});
      }
    }, 1000);

  } catch (err) {
    console.error("[Kervan Qwen] Hata:", err);
    api.runtime.sendMessage({
      type: "TAB_ERROR",
      id: id,
      mesaj: err.message
    }).catch(() => {});
  }
}
