// Kervan WebExtension — Kimi Content Script
const api = typeof browser !== "undefined" ? browser : chrome;

console.log("[Kervan Kimi] Content script yüklendi:", window.location.href);

// Sekmeyi Kervan'a kaydet
api.runtime.sendMessage({
  type: "TAB_REGISTER",
  provider: "kimi"
}).catch(() => {});

// Periyodik güvenlik ve giriş durumu kontrolü
setInterval(() => {
  if (window.kervanGuvenlikKontrolu) {
    window.kervanGuvenlikKontrolu("kimi");
  }
}, 5000);

api.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (message.type === "KERVAN_PROMPT") {
    kimiSoruSor(message.id, message.istem);
    sendResponse({ ok: true });
  }
});

async function kimiSoruSor(id, istem) {
  try {
    // 1. İstem kutusunu bul (Kimi genellikle contenteditable div kullanır)
    const kutuSecici = "div[contenteditable='true'], textarea";
    const kutu = await window.kervanOgeBekle(kutuSecici, 8000);
    if (!kutu) {
      throw new Error("Kimi istem kutusu bulunamadı.");
    }

    // 2. Metni insan gibi yerleştir
    kutu.focus();
    if (kutu.isContentEditable) {
      document.execCommand("selectAll", false, null);
      document.execCommand("insertText", false, istem);
      kutu.dispatchEvent(new Event("input", { bubbles: true }));
    } else {
      window.kervanMetinYaz(kutu, istem);
    }

    // 3. Gönder düğmesine tıkla
    await new Promise((r) => setTimeout(r, 400));
    let gonderBtn = document.querySelector("[data-testid='send-button']") ||
                    document.querySelector("button[aria-label*='Send']") ||
                    document.querySelector("button[aria-label*='Gönder']") ||
                    document.querySelector(".send-button") ||
                    kutu.closest("div")?.querySelector("button:not([disabled])");

    if (gonderBtn && !gonderBtn.disabled) {
      gonderBtn.click();
    } else {
      kutu.dispatchEvent(new KeyboardEvent("keydown", {
        key: "Enter",
        code: "Enter",
        keyCode: 13,
        which: 13,
        bubbles: true,
        cancelable: true
      }));
    }

    // 4. Yanıtı canlı gözlemle
    await new Promise((r) => setTimeout(r, 1000));
    let sonMetin = "";
    let hareketsizSayac = 0;

    const observer = new MutationObserver(() => {
      // Kimi yanıt balonları
      const yanitlar = document.querySelectorAll(".markdown, .segment-content, [class*='chat-item-assistant']");
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

    // Üretim bitiş kontrolü
    const bitisKontrol = setInterval(() => {
      if (window.kervanDurdurulduMu && window.kervanDurdurulduMu(id)) {
        clearInterval(bitisKontrol);
        observer.disconnect();
        return;
      }
      hareketsizSayac++;

      // Kimi'deki durdur/stop butonu var mı
      const durdurBtn = document.querySelector("[data-testid='stop-button'], [class*='stop']");
      if ((!durdurBtn && sonMetin.length > 0 && hareketsizSayac >= 2) || (hareketsizSayac >= 6 && sonMetin.length > 0)) {
        clearInterval(bitisKontrol);
        observer.disconnect();

        console.log("[Kervan Kimi] Yanıt tamamlandı, toplam uzunluk:", sonMetin.length);
        api.runtime.sendMessage({
          type: "STREAM_CHUNK",
          id: id,
          delta: "",
          done: true
        }).catch(() => {});
      }
    }, 1000);

  } catch (err) {
    console.error("[Kervan Kimi] Hata:", err);
    api.runtime.sendMessage({
      type: "TAB_ERROR",
      id: id,
      mesaj: err.message
    }).catch(() => {});
  }
}
