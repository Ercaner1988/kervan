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

/** URL'den sağlayıcı adı. Eşleme tek yerde dursun diye ayrıldı. */
function saglayiciBul(url) {
  if (!url) return null;
  if (url.includes("qwen.ai") || url.includes("qwenlm.ai")) return "qwen";
  if (url.includes("kimi.ai")) return "kimi";
  if (url.includes("gemini.google.com")) return "gemini";
  if (url.includes("deepseek.com")) return "deepseek";
  if (url.includes("chatgpt.com") || url.includes("chat.openai.com")) return "chatgpt";
  if (
    url.includes("copilot.com") ||
    url.includes("copilot.microsoft.com") ||
    url.includes("m365.cloud.microsoft")
  ) {
    return "copilot";
  }
  if (url.includes("chat.z.ai")) return "z.ai";
  if (url.includes("perplexity.ai")) return "perplexity";
  if (url.includes("chat.mistral.ai")) return "mistral";
  return null;
}

/**
 * Sekmenin içerik betiği gerçekten çalışıyor mu?
 *
 * URL'ye bakmak YETMİYOR: yeni yüklenen sekmede betik henüz enjekte olmamış,
 * boşaltılmış (discarded) sekmede hiç çalışmamış olur ve ilk mesaj "Receiving
 * end does not exist" ile düşer. Bu yüzden haritaya yazmadan önce ping atılır
 * (2026-10-24 incelemesi: URL tabanlı kayıt kaydı ölü sekme üzerinde tutuyordu).
 */
async function sekmeHazirMi(tabId) {
  try {
    const y = await api.tabs.sendMessage(tabId, { type: "KERVAN_PING" });
    return !!(y && y.ok);
  } catch (e) {
    return false;
  }
}

/** Bir hesabı hem yerel haritadan hem Kervan'dan düşürür. */
function kervandanDusur(k) {
  if (!k) return;
  bagliSekmeler.delete(k);
  const ayrac = k.indexOf(":");
  const provider = k.slice(0, ayrac);
  const profile = k.slice(ayrac + 1);
  console.log(`[Kervan Extension] Hesap düşürüldü: ${k}`);
  if (socket && socket.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify({ action: "ayril", provider: provider, profile: profile }));
  }
}

// Açık olan sekmeleri tara, ULAŞILABİLİR olanları bağla, ulaşılamayanları düşür.
//
// SEÇİM KURALI (2026-10-24 ölçümü): aynı sağlayıcının birden çok sekmesi
// açıkken `tabs.query` sırasında SON görülen sekmeye yazmak, isteği bayat bir
// sekmeye gönderiyordu; kullanıcı yeni sekmede çalışırken eski sekme yanıt
// veriyor, tanı ise "yanıt yok" diyordu. Artık sıralama sabit: ETKİN sekme,
// yoksa en son erişilen sekme kazanır. Böylece hangi sekmenin kullanılacağı
// tarayıcı sırasına değil, kullanıcının gerçekte baktığı sekmeye bağlıdır.
async function acikSekmeleriTara() {
  try {
    const tabs = await api.tabs.query({});
    const ulasilan = new Set();
    const adaylar = new Map();
    for (const tab of tabs) {
      if (!tab.url) continue;
      // Boşaltılmış (discarded) sekme: içerik betiği HİÇ çalışmamıştır.
      if (tab.discarded) continue;
      const provider = saglayiciBul(tab.url);
      if (!provider) continue;
      if (!(await sekmeHazirMi(tab.id))) continue;

      ulasilan.add(tab.id);
      const cId = tab.cookieStoreId || "varsayilan";
      const k = anahtar(provider, cId);
      const puan = (tab.active ? 1e12 : 0) + (tab.lastAccessed || 0);
      const onceki = adaylar.get(k);
      if (!onceki || puan > onceki.puan) {
        adaylar.set(k, { tabId: tab.id, url: tab.url, provider, cId, puan });
      }
    }
    for (const [k, sec] of adaylar) {
      if (bagliSekmeler.get(k) !== sec.tabId) {
        console.log(`[Kervan] Açık sekme seçildi: ${k} (Tab: ${sec.tabId})`);
        bagliSekmeler.set(k, sec.tabId);
        kervanaKaydet(sec.provider, sec.url, sec.cId, sec.tabId);
      }
    }
    // Budama: bu taramada ulaşılamayan kayıtlar ölü sekmedir. Silmezsek
    // 10 sn'lik tarama onları yeniden yazıp çıkmazı sürdürüyordu.
    for (const [k, id] of [...bagliSekmeler.entries()]) {
      if (!ulasilan.has(id)) kervandanDusur(k);
    }
  } catch (e) {
    console.warn("[Kervan] Sekmeleri tarama hatası:", e);
  }
}

// Sağlayıcı sekmesini bulur ve mesajı iletir.
//
// Yeniden deneme KÖR değil: her deneme öncesi ping atılır, yani yalnız içerik
// betiği gerçekten hazır olan sekmeye yazılır. Yalnız BAĞLANTI hatasında
// denenir; "port closed" gibi belirsiz hatalarda mesaj zaten sayfaya ulaşmış
// olabileceği için yeniden gönderilmez (istemin iki kez işlenmesini önler).
async function sekmeyeGonder(provider, profile, mesaj) {
  const bul = async () => {
    let k = profile ? anahtar(provider, profile) : saglayiciAnahtari(provider);
    let tabId = k ? bagliSekmeler.get(k) : null;
    if (!tabId) {
      await acikSekmeleriTara();
      k = profile ? anahtar(provider, profile) : saglayiciAnahtari(provider);
      tabId = k ? bagliSekmeler.get(k) : null;
    }
    return { k, tabId };
  };
  const baglantiHatasi = (e) =>
    /Receiving end does not exist|Could not establish connection/i.test(
      (e && e.message) || ""
    );

  let { k, tabId } = await bul();
  if (!tabId) return null;
  try {
    return await api.tabs.sendMessage(tabId, mesaj);
  } catch (e) {
    if (!baglantiHatasi(e)) throw e;
    // Ölü kayıt hemen düşürülür; ardından içerik betiğinin enjekte olması için
    // kısa bir bekleme, sonra taze (ping'li) tarama.
    if (k) kervandanDusur(k);
    await new Promise((r) => setTimeout(r, 600));
    ({ k, tabId } = await bul());
    if (!tabId) throw e;
    return await api.tabs.sendMessage(tabId, mesaj);
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
        try {
          const yanit = await sekmeyeGonder(msg.provider, msg.profile, {
            type: "KERVAN_PROMPT",
            id: msg.id,
            provider: msg.provider,
            profile: msg.profile,
            istem: msg.istem,
            dosyalar: msg.dosyalar || []
          });
          if (yanit === null) {
            socket.send(JSON.stringify({
              action: "hata",
              id: msg.id,
              mesaj: `'${msg.provider}' için açık bir sekme bulunamadı.`
            }));
          }
        } catch (e) {
          console.warn("[Kervan] Sekmeye mesaj gönderilemedi...", e);
          socket.send(JSON.stringify({
            action: "hata",
            id: msg.id,
            mesaj: `Sekmeye ulaşılamadı. Sayfayı bir kez yenileyin (F5): ${e.message}`
          }));
        }
      } else if (msg.action === "oge_oku" || msg.action === "oge_bas") {
        // Seçim turu: okuma/tıklama ilgili sekmeye iletilir, yanıt HAM öğe
        // listesi olarak Kervan'a döner. Katlama/eşleştirme Kervan tarafındadır;
        // burada hiçbir metin yorumu yapılmaz.
        try {
          const yanit = await sekmeyeGonder(msg.provider, msg.profile, {
            type: "KERVAN_SECIM",
            komut: msg.action,
            id: msg.id,
            secici: msg.secici,
            dugme: msg.dugme,
            indeks: msg.indeks
          });
          if (yanit === null) {
            socket.send(JSON.stringify({
              action: "hata",
              id: msg.id,
              mesaj: `'${msg.provider}' için açık bir sekme bulunamadı.`
            }));
            return;
          }
          if (yanit && yanit.ok) {
            socket.send(JSON.stringify({ action: "ogeler", id: msg.id, ogeler: yanit.ogeler }));
          } else {
            socket.send(JSON.stringify({
              action: "hata",
              id: msg.id,
              mesaj: (yanit && yanit.mesaj) || "seçim turu yanıtsız kaldı"
            }));
          }
        } catch (e) {
          socket.send(JSON.stringify({
            action: "hata",
            id: msg.id,
            mesaj: `seçim turu sekmeye ulaştırılamadı: ${e.message}`
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
