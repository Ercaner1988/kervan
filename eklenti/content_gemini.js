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

    // 2. Metni yaz — ve YAZILDIĞINI OKUYARAK doğrula.
    //
    // ÖLÇÜM (2026-10-24, gerçek profil): `execCommand("insertText")` bu
    // editörde sessizce hiçbir şey yazmadı; kutu boş kaldı. Kod bunu fark
    // etmediği için boş istem gönderiliyor, sunucu 120 sn sonra "zaman aşımı"
    // diyordu. Artık üç yol sırayla denenir ve her adımdan sonra kutunun
    // içeriği okunur; hiçbiri tutmazsa açık hata verilir.
    const kutuBosMu = () => (kutu.innerText || kutu.textContent || "").trim().length === 0;
    const yazmaYollari = [
      [
        "execCommand",
        () => {
          kutu.focus();
          document.execCommand("selectAll", false, null);
          document.execCommand("insertText", false, istem);
        }
      ],
      [
        "yapistirma",
        () => {
          kutu.focus();
          const veri = new DataTransfer();
          veri.setData("text/plain", istem);
          kutu.dispatchEvent(
            new ClipboardEvent("paste", {
              clipboardData: veri,
              bubbles: true,
              cancelable: true
            })
          );
        }
      ],
      [
        "metin",
        () => {
          kutu.focus();
          kutu.textContent = istem;
          kutu.dispatchEvent(new Event("input", { bubbles: true }));
        }
      ]
    ];

    const yazmaGunlugu = [];
    let yazildi = false;
    for (const [ad, uygula] of yazmaYollari) {
      try {
        uygula();
        kutu.dispatchEvent(new Event("input", { bubbles: true }));
        if (!kutuBosMu()) {
          yazildi = true;
          break;
        }
      } catch (e) {
        yazmaGunlugu.push(`${ad}: hata=${e.message}`);
        continue;
      }
      yazmaGunlugu.push(`${ad}: kutu bos kaldi`);
    }
    if (!yazildi) {
      throw new Error(
        `istem kutusuna yazılamadı — denemeler: ${yazmaGunlugu.join(" | ")} — kutu: ${
          kutu.tagName
        }[contenteditable=${kutu.getAttribute("contenteditable")}]`
      );
    }

    // 3. Gönder — ve GERÇEKTEN gönderildiğini doğrula.
    //
    // ÖLÇÜM (2026-10-24, gerçek profil): düğme görünür ve etkin, `click()`
    // işlenmiyor; istem kutuda kalıyor, yanıt hiç başlamıyor, sunucu 120 sn
    // sonra "zaman aşımı" diyor. Sessiz çıkmaz yerine üç yol denenir ve
    // başarı "kutu boşaldı" ya da "yeni kullanıcı mesajı düğümü belirdi" ile
    // ölçülür; olmadıysa açık hata verilir.
    await new Promise((r) => setTimeout(r, 500));

    const soruSayisi = () =>
      document.querySelectorAll(
        "user-query, .user-query-container, [data-test-id='user-query']"
      ).length;
    const yanitSayisi = () =>
      document.querySelectorAll("model-response, message-content, .model-response-text")
        .length;
    const bosaldi = () => (kutu.innerText || kutu.textContent || "").trim().length === 0;

    if (bosaldi()) {
      throw new Error(
        "istem kutusu yazımdan hemen sonra boşaldı — editör içeriği kabul etmedi"
      );
    }

    const oncekiSoru = soruSayisi();
    // ÖNCEKİ YANIT SAYISI: sohbette eski yanıtlar varsa mutlak sayı zaten
    // sıfırdan büyük olur ve "iz var" denetimi hiç beklemeden geçerdi
    // (2026-10-24 incelemesi). Ölçüt FARK olmalı: yalnız bu turda eklenen
    // yanıt izdir.
    const oncekiYanit = yanitSayisi();
    const gonderBtn =
      document.querySelector("button[aria-label*='Send']") ||
      document.querySelector("button[aria-label*='Gönder']") ||
      document.querySelector(".send-button") ||
      document.querySelector("button.send-button-container");

    const enter = (el) => {
      if (typeof window.kervanTus === "function") {
        window.kervanTus(el, "Enter");
        return;
      }
      for (const tur of ["keydown", "keyup"]) {
        el.dispatchEvent(
          new KeyboardEvent(tur, {
            key: "Enter",
            code: "Enter",
            keyCode: 13,
            which: 13,
            bubbles: true,
            cancelable: true
          })
        );
      }
    };

    const dene = (ad) => {
      if (!gonderBtn || gonderBtn.disabled) {
        kutu.focus();
        enter(kutu);
        return;
      }
      if (ad === "isaretci" && typeof window.kervanTikla === "function") {
        window.kervanTikla(gonderBtn);
      } else if (ad === "click") {
        gonderBtn.click();
      } else {
        gonderBtn.focus();
        enter(gonderBtn);
      }
    };

    const bekle = async () => {
      const bas = Date.now();
      while (Date.now() - bas < 3000) {
        if (soruSayisi() > oncekiSoru || bosaldi()) return true;
        await new Promise((r) => setTimeout(r, 150));
      }
      return false;
    };

    let gonderildi = false;
    const gunluk = [];
    for (const ad of ["isaretci", "click", "odak+enter"]) {
      if (ad !== "isaretci") await new Promise((r) => setTimeout(r, 300));
      dene(ad);
      if (await bekle()) {
        gonderildi = true;
        break;
      }
      gunluk.push(`${ad}: kutu=${(kutu.innerText || "").trim().length}kr`);
    }
    if (!gonderildi) {
      throw new Error(
        `istem gönderilemedi — düğme: ${
          gonderBtn
            ? `aria-label=${gonderBtn.getAttribute("aria-label")} disabled=${gonderBtn.disabled}`
            : "yok"
        } — denemeler: ${gunluk.join(" | ")}`
      );
    }

    // 3b. Gönderim doğrulaması tek başına yetmez: kutu, Angular yeniden
    // çizdiği için kendiliğinden de boşalabilir. Gerçek gönderimin sayfadaki
    // izi "durdurma düğmesi" ya da bir yanıt düğümüdür. Yoksa 120 sn bekleyip
    // "zaman aşımı" demek yerine burada, kanıtla durulur.
    {
      const bas = Date.now();
      let iz = "";
      while (Date.now() - bas < 10000) {
        const durdur = document.querySelector(
          "button[aria-label*='Stop'], button[aria-label*='Durdur'], button[aria-label*='Yanıtı durdur']"
        );
        const yanit = yanitSayisi();
        if (durdur || yanit > oncekiYanit) {
          iz = "";
          break;
        }
        iz = `durdurma düğmesi yok, yeni yanıt düğümü yok (önce=${oncekiYanit}, şimdi=${yanit})`;
        await new Promise((r) => setTimeout(r, 250));
      }
      if (iz) {
        throw new Error(
          `istem gönderildi sanıldı ama sayfada iz yok (${iz}) — düğme: ${
            gonderBtn ? gonderBtn.getAttribute("aria-label") : "yok"
          } — kutu=${(kutu.innerText || "").trim().length}kr — denemeler: ${gunluk.join(" | ")}`
        );
      }
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
      if (window.kervanDurdurulduMu && window.kervanDurdurulduMu(id)) {
        clearInterval(bitisKontrol);
        observer.disconnect();
        return;
      }
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
