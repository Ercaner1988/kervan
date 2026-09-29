// Kervan WebExtension Arka Plan Köprüsü (Background Script)

const api = typeof browser !== "undefined" ? browser : chrome;
const WS_URL = "ws://127.0.0.1:20129/ws/eklenti";

let socket = null;
let bagliSekmeler = new Map(); // hesapAnahtari (provider:cookieStoreId) -> tabId
let yenidenBaglanmaZamanlayicisi = null;

function kervanaKaydet(provider, url, cookieStoreId = "varsayilan", tabId = 0) {
  if (socket && socket.readyState === WebSocket.OPEN) {
    const hesapKimligi = `${provider}:${cookieStoreId}`;
    console.log(`[Kervan] '${hesapKimligi}' sekmesi Kervan'a bildiriliyor (Tab: ${tabId})...`);
    socket.send(JSON.stringify({
      action: "kayit",
      provider: provider,
      profile: cookieStoreId,
      tab_id: tabId,
      url: url || ""
    }));
  }
}

// Açık olan sekmeleri tara ve eşleşenleri otomatik bağla
async function acikSekmeleriTara() {
  try {
    const tabs = await api.tabs.query({});
    for (const tab of tabs) {
      if (!tab.url) continue;
      const cId = tab.cookieStoreId || "varsayilan";
      let provider = null;

      if (tab.url.includes("qwen.ai") || tab.url.includes("qwenlm.ai")) {
        provider = "qwen";
      } else if (tab.url.includes("kimi.ai")) {
        provider = "kimi";
      } else if (tab.url.includes("gemini.google.com")) {
        provider = "gemini";
      } else if (tab.url.includes("deepseek.com")) {
        provider = "deepseek";
      } else if (tab.url.includes("chatgpt.com") || tab.url.includes("chat.openai.com")) {
        provider = "chatgpt";
      } else if (tab.url.includes("copilot.com") || tab.url.includes("copilot.microsoft.com") || tab.url.includes("m365.cloud.microsoft")) {
        provider = "copilot";
      } else if (tab.url.includes("chat.z.ai")) {
        provider = "z.ai";
      } else if (tab.url.includes("perplexity.ai")) {
        provider = "perplexity";
      } else if (tab.url.includes("chat.mistral.ai")) {
        provider = "mistral";
      }

      if (provider) {
        const anahtar = `${provider}:${cId}`;
        console.log(`[Kervan] Açık sekme bulundu: ${anahtar} (Tab: ${tab.id})`);
        bagliSekmeler.set(anahtar, tab.id);
        kervanaKaydet(provider, tab.url, cId, tab.id);
      }
    }
  } catch (e) {
    console.warn("[Kervan] Sekmeleri tarama hatası:", e);
  }
}

function kervanaBaglan() {
  if (socket && (socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING)) {
    return;
  }

  console.log("[Kervan Extension] Kervan yerel ağ geçidine bağlanılıyor:", WS_URL);
  try {
    socket = new WebSocket(WS_URL);
  } catch (err) {
    console.error("[Kervan Extension] WebSocket başlatma hatası:", err);
    yenidenBaglanmaZamanlayicisi = setTimeout(kervanaBaglan, 3000);
    return;
  }

  socket.onopen = () => {
    console.log("[Kervan Extension] Kervan ağ geçidine bağlandı!");
    if (yenidenBaglanmaZamanlayicisi) {
      clearTimeout(yenidenBaglanmaZamanlayicisi);
      yenidenBaglanmaZamanlayicisi = null;
    }

    // Açık sekmeleri tara ve Kervan'a bildir
    acikSekmeleriTara();
  };

  socket.onmessage = async (event) => {
    try {
      const msg = JSON.parse(event.data);
      console.log("[Kervan Extension] Kervan'dan emir geldi:", msg);

      if (msg.action === "sor") {
        let tabId = bagliSekmeler.get(msg.provider);

        // Eğer haritada yoksa yeniden tara
        if (!tabId) {
          await acikSekmeleriTara();
          tabId = bagliSekmeler.get(msg.provider);
        }

        if (!tabId) {
          socket.send(JSON.stringify({
            action: "hata",
            id: msg.id,
            mesaj: `'${msg.provider}' için açık bir sekme bulunamadı.`
          }));
          return;
        }

        // İlgili sekmeye soruyu ilet
        try {
          await api.tabs.sendMessage(tabId, {
            type: "KERVAN_PROMPT",
            id: msg.id,
            provider: msg.provider,
            istem: msg.istem,
            dosyalar: msg.dosyalar || []
          });
        } catch (e) {
          console.warn("[Kervan] Sekmeye mesaj gönderilemedi, content script enjekte ediliyor...", e);
          // Gerekirse content script'i dinamik çalıştır
          socket.send(JSON.stringify({
            action: "hata",
            id: msg.id,
            mesaj: `Sekmeye ulaşılamadı. Sayfayı bir kez yenileyin (F5): ${e.message}`
          }));
        }
      } else if (msg.action === "durdur") {
        const tabId = bagliSekmeler.get(msg.provider);
        if (tabId) {
          api.tabs.sendMessage(tabId, { type: "KERVAN_STOP", id: msg.id }).catch(() => {});
        }
      }
    } catch (e) {
      console.error("[Kervan Extension] Mesaj işleme hatası:", e);
    }
  };

  socket.onclose = () => {
    console.log("[Kervan Extension] Bağlantı kapandı, 3 sn sonra yeniden denenecek...");
    socket = null;
    if (!yenidenBaglanmaZamanlayicisi) {
      yenidenBaglanmaZamanlayicisi = setTimeout(kervanaBaglan, 3000);
    }
  };

  socket.onerror = (err) => {
    console.warn("[Kervan Extension] WebSocket hatası:", err);
    if (socket) {
      socket.close();
    }
  };
}

// Content script'lerden gelen mesajları dinle
api.runtime.onMessage.addListener((message, sender, sendResponse) => {
  const tabId = sender.tab ? sender.tab.id : null;

  if (message.type === "TAB_REGISTER") {
    const provider = message.provider;
    console.log(`[Kervan Extension] Sekme kayıt oldu: ${provider} (Tab ID: ${tabId})`);
    if (tabId) {
      bagliSekmeler.set(provider, tabId);
    }
    kervanaKaydet(provider, sender.tab ? sender.tab.url : "");
    sendResponse({ ok: true });
    return true;
  } else if (message.type === "STREAM_CHUNK") {
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({
        action: "parca",
        id: message.id,
        delta: message.delta,
        done: message.done
      }));
    }
  } else if (message.type === "STATUS_UPDATE") {
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({
        action: "durum",
        provider: message.provider,
        durum: message.durum,
        detay: message.detay
      }));
    }
  } else if (message.type === "TAB_ERROR") {
    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({
        action: "hata",
        id: message.id,
        mesaj: message.mesaj
      }));
    }
  }
});

// Sekme kapandığında havuzdan düşür
api.tabs.onRemoved.addListener((tabId) => {
  for (const [provider, id] of bagliSekmeler.entries()) {
    if (id === tabId) {
      bagliSekmeler.delete(provider);
      console.log(`[Kervan Extension] Sekme kapandı: ${provider}`);
      break;
    }
  }
});

// Başlangıçta bağlan
kervanaBaglan();

// Periyodik olarak sekmeleri kontrol et (her 10 sn)
setInterval(acikSekmeleriTara, 10000);
