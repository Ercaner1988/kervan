// Kervan WebExtension Arka Plan Köprüsü (Background Script)
//
// Çok hesap: aynı sağlayıcının farklı tarayıcı profillerindeki (cookieStoreId)
// sekmeleri ayrı hesaplardır. Anahtar `provider:profile`dir; Kervan hangi
// hesabı seçtiyse `sor`/`durdur` mesajında profile alanıyla bildirir.

const api = typeof browser !== "undefined" ? browser : chrome;
const WS_URL = "ws://127.0.0.1:20129/ws/eklenti";
const TOKEN_URL = "http://127.0.0.1:20129/eklenti/token";

let socket = null;
let bagliSekmeler = new Map(); // "provider:profile" -> tabId
let yenidenBaglanmaZamanlayicisi = null;
let token = null; // yalnız bellekte; kalıcı kopya storage.local'da
let tokenYenile = false;

/**
 * Kervan'ın paylaşılan token'ını alır. Uç yalnız uzantı kaynaklı isteklere
 * açıktır; düz bir web sayfası buradan token okuyamaz. Token storage.local'da
 * saklanır (yeniden başlatmada ilk istekte tekrar sorulmasın).
 */
async function tokenAl() {
  if (token && !tokenYenile) return token;
  if (!tokenYenile) {
    try {
      const depo = await api.storage.local.get("kervanToken");
      if (depo && depo.kervanToken) {
        token = depo.kervanToken;
        return token;
      }
    } catch (e) {
      console.warn("[Kervan] Token deposu okunamadı:", e);
    }
  }
  const yanit = await fetch(TOKEN_URL, { cache: "no-store" });
  if (!yanit.ok) {
    throw new Error(`token ucu ${yanit.status} döndü`);
  }
  const govde = await yanit.json();
  if (!govde || !govde.token) {
    throw new Error("token ucu boş yanıt verdi");
  }
  token = govde.token;
  tokenYenile = false;
  try {
    await api.storage.local.set({ kervanToken: token });
  } catch (e) {
    console.warn("[Kervan] Token saklanamadı:", e);
  }
  return token;
}

function anahtar(provider, profile) {
  return `${provider}:${profile || "varsayilan"}`;
}

/** Sağlayıcının bağlı ilk hesabının anahtarını döner (profilsiz eski istekler). */
function saglayiciAnahtari(provider) {
  const onek = `${provider}:`;
  for (const k of bagliSekmeler.keys()) {
    if (k.startsWith(onek)) return k;
  }
  return null;
}

function kervanaKaydet(provider, url, profile, tabId) {
  if (socket && socket.readyState === WebSocket.OPEN) {
    const hesapKimligi = anahtar(provider, profile);
    console.log(`[Kervan] '${hesapKimligi}' sekmesi Kervan'a bildiriliyor (Tab: ${tabId})...`);
    socket.send(JSON.stringify({
      action: "kayit",
      provider: provider,
      profile: profile || "varsayilan",
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
        const k = anahtar(provider, cId);
        console.log(`[Kervan] Açık sekme bulundu: ${k} (Tab: ${tab.id})`);
        bagliSekmeler.set(k, tab.id);
        kervanaKaydet(provider, tab.url, cId, tab.id);
      }
    }
  } catch (e) {
    console.warn("[Kervan] Sekmeleri tarama hatası:", e);
  }
}

async function kervanaBaglan() {
  if (socket && (socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING)) {
    return;
  }

  let t;
  try {
    t = await tokenAl();
  } catch (err) {
    console.warn("[Kervan Extension] Token alınamadı, 3 sn sonra yeniden denenecek:", err);
    if (!yenidenBaglanmaZamanlayicisi) {
      yenidenBaglanmaZamanlayicisi = setTimeout(kervanaBaglan, 3000);
    }
    return;
  }

  console.log("[Kervan Extension] Kervan yerel ağ geçidine bağlanılıyor:", WS_URL);
  try {
    socket = new WebSocket(`${WS_URL}?token=${encodeURIComponent(t)}`);
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
        // Yeni Kervan profile gönderir; eski Kervan göndermez.
        let k = msg.profile ? anahtar(msg.provider, msg.profile) : saglayiciAnahtari(msg.provider);
        let tabId = k ? bagliSekmeler.get(k) : null;

        // Haritada yoksa yeniden tara
        if (!tabId) {
          await acikSekmeleriTara();
          k = msg.profile ? anahtar(msg.provider, msg.profile) : saglayiciAnahtari(msg.provider);
          tabId = k ? bagliSekmeler.get(k) : null;
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
            profile: msg.profile,
            istem: msg.istem,
            dosyalar: msg.dosyalar || []
          });
        } catch (e) {
          console.warn("[Kervan] Sekmeye mesaj gönderilemedi...", e);
          socket.send(JSON.stringify({
            action: "hata",
            id: msg.id,
            mesaj: `Sekmeye ulaşılamadı. Sayfayı bir kez yenileyin (F5): ${e.message}`
          }));
        }
      } else if (msg.action === "durdur") {
        const k = msg.profile ? anahtar(msg.provider, msg.profile) : saglayiciAnahtari(msg.provider);
        const tabId = k ? bagliSekmeler.get(k) : null;
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
    // Token dosyası silinmiş/yenilenmiş olabilir: sonraki denemede yeniden al.
    tokenYenile = true;
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
  const profile = sender.tab && sender.tab.cookieStoreId ? sender.tab.cookieStoreId : "varsayilan";

  if (message.type === "TAB_REGISTER") {
    const provider = message.provider;
    const k = anahtar(provider, profile);
    console.log(`[Kervan Extension] Sekme kayıt oldu: ${k} (Tab ID: ${tabId})`);
    if (tabId) {
      bagliSekmeler.set(k, tabId);
    }
    kervanaKaydet(provider, sender.tab ? sender.tab.url : "", profile, tabId || 0);
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

// Sekme kapandığında ilgili hesabı Kervan'dan da düşür
api.tabs.onRemoved.addListener((tabId) => {
  for (const [k, id] of bagliSekmeler.entries()) {
    if (id === tabId) {
      bagliSekmeler.delete(k);
      const ayrac = k.indexOf(":");
      const provider = k.slice(0, ayrac);
      const profile = k.slice(ayrac + 1);
      console.log(`[Kervan Extension] Sekme kapandı: ${k}`);
      if (socket && socket.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify({
          action: "ayril",
          provider: provider,
          profile: profile
        }));
      }
    }
  }
});

// Başlangıçta bağlan
kervanaBaglan();

// Periyodik olarak sekmeleri kontrol et (her 10 sn)
setInterval(acikSekmeleriTara, 10000);
