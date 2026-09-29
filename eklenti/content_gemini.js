// Kervan WebExtension — Gemini Content Script (Metin, Resim ve Video Destekli)
const api = typeof browser !== "undefined" ? browser : chrome;

console.log("[Kervan Gemini] Content script yüklendi:", window.location.href);

// Sekmeyi Kervan'a kaydet
api.runtime.sendMessage({
  type: "TAB_REGISTER",
  provider: "gemini"
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
    geminiSoruSor(message.id, message.istem, message.dosyalar || []);
    sendResponse({ ok: true });
  }
});

async function geminiSoruSor(id, istem, dosyalar = []) {
  try {
    // 1. İstem kutusunu bul (Gemini rich-textarea içindeki contenteditable div'i kullanır)
    const kutuSecici = "div[contenteditable='true'], rich-textarea div[contenteditable='true']";
    const kutu = await window.kervanOgeBekle(kutuSecici, 8000);
    if (!kutu) {
      throw new Error("Gemini istem kutusu bulunamadı.");
    }

    // 1.5. Ek dosyalar varsa yükle (Görsel, Video, Belge)
    if (dosyalar && dosyalar.length > 0) {
      console.log(`[Kervan Gemini] ${dosyalar.length} adet dosya yükleniyor...`);
      const dataTransfer = new DataTransfer();

      for (const d of dosyalar) {
        try {
          const fileObj = base64ToFile(d.veri_base64, d.ad, d.mime_tipi);
          dataTransfer.items.add(fileObj);
        } catch (e) {
          console.warn("[Kervan Gemini] Dosya dönüştürme hatası:", d.ad, e);
        }
      }

      if (dataTransfer.files.length > 0) {
        // A) Gemini dosya yükleme input[type='file'] elemanını bul ve ata
        const fileInput = document.querySelector("input[type='file']");
        if (fileInput) {
          try {
            fileInput.files = dataTransfer.files;
            fileInput.dispatchEvent(new Event("change", { bubbles: true }));
            fileInput.dispatchEvent(new Event("input", { bubbles: true }));
          } catch (e) {
            console.warn("[Kervan Gemini] fileInput atama hatası:", e);
          }
        }

        // B) Pano yapıştırma (paste) simülasyonu sağla
        try {
          kutu.focus();
          const pasteEvent = new ClipboardEvent("paste", {
            clipboardData: dataTransfer,
            bubbles: true,
            cancelable: true
          });
          kutu.dispatchEvent(pasteEvent);
        } catch (e) {
          console.warn("[Kervan Gemini] Yapıştırma olayı fırlatılırken uyarı:", e);
        }

        // C) Dosyaların yüklenmesi için 1500-2000 ms bekle
        await new Promise((r) => setTimeout(r, 1800));

        // Varsa mat-progress-bar veya yükleme spinner'ı bitene kadar bekle
        const baslamaZamani = Date.now();
        while (Date.now() - baslamaZamani < 15000) {
          const yukleniyor = document.querySelector(
            "mat-progress-bar, .upload-progress, .file-loading, [aria-label*='Yükleniyor'], [aria-label*='Uploading'], .uploader-progress"
          );
          if (!yukleniyor) {
            break;
          }
          await new Promise((r) => setTimeout(r, 500));
        }
      }
    }

    // 2. Metni insan gibi yaz (Gemini contenteditable paragraflarını temizleyip yerleştirir)
    kutu.focus();
    document.execCommand("selectAll", false, null);
    document.execCommand("insertText", false, istem);
    kutu.dispatchEvent(new Event("input", { bubbles: true }));

    // 3. Gönder butonuna tıkla
    await new Promise((r) => setTimeout(r, 500));
    let gonderBtn = document.querySelector("button[aria-label*='Send']") ||
                    document.querySelector("button[aria-label*='Gönder']") ||
                    document.querySelector(".send-button") ||
                    document.querySelector("button.send-button-container");

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
    await new Promise((r) => setTimeout(r, 1200));
    let sonMetin = "";
    let hareketsizSayac = 0;
    const gonderilenMedyalar = new Set();

    const medyaTarayici = (sonYanit) => {
      const bulunanlar = [];
      // Görseller (Imagen 3)
      const gorseller = sonYanit.querySelectorAll(
        "img[src*='googleusercontent'], img[src*='blob:'], img[src*='data:image'], img.image-result, .image-container img, [data-test-id='image-response'] img, div[data-image-index] img, img.generated-image"
      );
      for (const img of gorseller) {
        const src = img.src || img.getAttribute("src");
        if (!src) continue;
        if (src.startsWith("data:image/svg")) continue;
        // Avatar ve profil fotoğraflarını filtrele
        if (src.includes("/a/") || img.closest(".user-avatar, .avatar, profile-badge, button.user-avatar")) continue;
        bulunanlar.push(`![Üretilen Görsel](${src})`);
      }

      // Videolar (Veo 2)
      const videolar = sonYanit.querySelectorAll(
        "video, video source, a[href*='video-download'], a[href*='googlevideo'], a[href*='.mp4'], [data-test-id='video-response'] video, .video-container video, a[download*='video']"
      );
      for (const vid of videolar) {
        const src = vid.src || vid.currentSrc || (vid.querySelector && vid.querySelector("source") ? vid.querySelector("source").src : null) || vid.href;
        if (!src) continue;
        bulunanlar.push(`[Üretilen Video](${src})`);
      }

      return Array.from(new Set(bulunanlar));
    };

    const observer = new MutationObserver(() => {
      // Gemini yanıt balonları (model-response veya message-content)
      const yanitlar = document.querySelectorAll("model-response, message-content, .response-container");
      if (yanitlar.length > 0) {
        const sonYanit = yanitlar[yanitlar.length - 1];
        
        // 1. Metin içeriği canlı akış
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

        // 2. Medya içerikleri (Imagen 3 / Veo 2)
        const medyalar = medyaTarayici(sonYanit);
        for (const m of medyalar) {
          if (!gonderilenMedyalar.has(m)) {
            gonderilenMedyalar.add(m);
            hareketsizSayac = 0;
            api.runtime.sendMessage({
              type: "STREAM_CHUNK",
              id: id,
              delta: "\n\n" + m,
              done: false
            }).catch(() => {});
          }
        }
      }
    });

    observer.observe(document.body, { childList: true, subtree: true, characterData: true });

    // Üretim bitiş kontrolü
    const bitisKontrol = setInterval(() => {
      hareketsizSayac++;

      // Görsel veya video üretimi devam ederken ilerleme çubuğu varsa bitirme
      const yukleniyor = document.querySelector(
        "mat-progress-bar, .loading-indicator, .sparkle-animation, [aria-label*='Generating'], [aria-label*='Üretiliyor'], .generating, [data-test-id='generating-indicator']"
      );
      if (yukleniyor) {
        hareketsizSayac = 0; // Üretim devam ediyor, zaman aşımını engelle
        return;
      }

      // Gemini'deki durdur butonu kaybolduysa ve metin/görsel geldiyse
      const durdurBtn = document.querySelector("button[aria-label*='Stop'], button[aria-label*='Durdur']");
      const birSeyUretildi = sonMetin.length > 0 || gonderilenMedyalar.size > 0;
      if ((!durdurBtn && birSeyUretildi && hareketsizSayac >= 2) || (hareketsizSayac >= 10 && birSeyUretildi)) {
        clearInterval(bitisKontrol);
        observer.disconnect();

        // Son bir kez daha DOM'daki medyaları kontrol et
        const yanitlar = document.querySelectorAll("model-response, message-content, .response-container");
        if (yanitlar.length > 0) {
          const sonYanit = yanitlar[yanitlar.length - 1];
          const sonMedyalar = medyaTarayici(sonYanit);
          for (const m of sonMedyalar) {
            if (!gonderilenMedyalar.has(m)) {
              gonderilenMedyalar.add(m);
              api.runtime.sendMessage({
                type: "STREAM_CHUNK",
                id: id,
                delta: "\n\n" + m,
                done: false
              }).catch(() => {});
            }
          }
        }

        console.log("[Kervan Gemini] Yanıt tamamlandı, metin uzunluğu:", sonMetin.length, "medya sayısı:", gonderilenMedyalar.size);
        api.runtime.sendMessage({
          type: "STREAM_CHUNK",
          id: id,
          delta: "",
          done: true
        }).catch(() => {});
      }
    }, 1000);

  } catch (err) {
    console.error("[Kervan Gemini] Hata:", err);
    api.runtime.sendMessage({
      type: "TAB_ERROR",
      id: id,
      mesaj: err.message
    }).catch(() => {});
  }
}
