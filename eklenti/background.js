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
let enjeksiyonDenemeleri = new Map(); // tabId -> url (son enjeksiyon denemesi)
let yenidenBaglanmaZamanlayicisi = null;
let guncellemeZamanlayicisi = null;
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
    // Sessiz: bu çağrı artık 10 sn'de bir yapılan TAZELEME turunda da koşuyor;
    // her turda log basmak konsolu okunamaz hale getirir (seçim logu
    // `acikSekmeleriTara` içinde, yalnız eşleme değişince yazılır).
    socket.send(JSON.stringify({
      action: "kayit",
      provider: provider,
      profile: profile || "varsayilan",
      tab_id: tabId,
      url: url || ""
    }));
  }
  // SOKET KAPALIYSA KAYIT KAYBOLUR — kuyruk YOK, yeniden gönderim YOK.
  // Bu bilinçli: `onopen` zaten `bagliSekmeler.clear()` + `acikSekmeleriTara()`
  // ile tüm açık sekmeleri yeniden kaydeder, 10 sn'lik tur da aynı işi
  // yapar. Yani kayıp en fazla bir tur (≤10 sn) sürer. Buraya kuyruk eklemek
  // `clear()` sonrası eski eşlemeyi geri getirip budama kapısını bozar.
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
 *
 * B3 (2026-11-15 denetimi): ping başarısız olduğunda YENİDEN ENJEKSİYON.
 * Önceden içerik betiği ölmüş ama sekme yenilenmemişse hesap her turda
 * düşürülüyor ve kullanıcı sekmeyi elle yenilemedikçe geri gelmiyordu. Artık
 * sayfa yüklenmişse (`status === "complete"`) `scripting.executeScript` ile
 * içerik betikleri BİR KEZ yeniden enjekte edilir, sonra ping tekrarlanır.
 *
 * NEDEN "bir kez": aynı sayfa yüklemesinde her 10 sn'de bir enjeksiyon
 * denemek (ör. betiğin sayfada çalışmasını engelleyen bir CSP) konsolu
 * doldurur ve taramayı yavaşlatır. `enjeksiyonDenemeleri` tabId→url tutar;
 * sekme yenilenince url değişir ve yeni bir deneme hakkı doğar.
 */
async function sekmeHazirMi(tab, provider) {
  if (await pingAt(tab.id)) return true;
  if (!provider || tab.status !== "complete") return false;
  if (enjeksiyonDenemeleri.get(tab.id) === tab.url) return false;
  enjeksiyonDenemeleri.set(tab.id, tab.url);
  if (!(await icerikBetigiEnjekte(tab.id, provider))) return false;
  return await pingAt(tab.id);
}

/** Ping atar; içerik betiği yoksa `false` (istisna yutulur). */
async function pingAt(tabId) {
  try {
    const y = await api.tabs.sendMessage(tabId, { type: "KERVAN_PING" });
    return !!(y && y.ok);
  } catch (e) {
    return false;
  }
}

/**
 * Sağlayıcının içerik betiklerini sekmeye yeniden enjekte eder.
 *
 * Dosya listesi `manifest.json`'daki `content_scripts` sırasıyla AYNI olmalı:
 * önce `content_common.js` (ortak yardımcılar), sonra sağlayıcı betiği.
 * Sıra bozulursa sağlayıcı betiği `window.kervanMetinYaz` gibi yardımcıları
 * bulamaz. Çift dinleyici riski içerik betiklerinin kendi kapılarıyla
 * kapatılmıştır (bkz. `content_*.js` başındaki `__kervanDinleyici` kapısı).
 */
const ICERIK_BETIKLERI = {
  qwen: ["content_common.js", "content_qwen.js"],
  kimi: ["content_common.js", "content_kimi.js"],
  gemini: ["content_common.js", "content_gemini.js"],
  deepseek: ["content_common.js", "content_deepseek.js"],
  chatgpt: ["content_common.js", "content_chatgpt.js"],
  copilot: ["content_common.js", "content_copilot.js"],
  "z.ai": ["content_common.js", "content_zai.js"],
  perplexity: ["content_common.js", "content_perplexity.js"],
  mistral: ["content_common.js", "content_mistral.js"]
};

async function icerikBetigiEnjekte(tabId, provider) {
  const dosyalar = ICERIK_BETIKLERI[provider];
  if (!dosyalar) return false;
  if (!api.scripting || !api.scripting.executeScript) {
    console.warn("[Kervan] scripting izni yok: yeniden enjeksiyon atlandı");
    return false;
  }
  try {
    await api.scripting.executeScript({ target: { tabId }, files: dosyalar });
    console.log(`[Kervan] İçerik betiği yeniden enjekte edildi: ${provider} (Tab: ${tabId})`);
    return true;
  } catch (e) {
    console.warn(`[Kervan] ${provider} içerik betiği enjekte edilemedi:`, e);
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
//
// YAPIŞKAN SEÇİM (B4, 2026-11-15 denetimi): puan her turda yeniden hesaplanıyor
// ve `lastAccessed` kullanıcı sekmeler arasında gezdikçe değişiyordu. İki sekme
// aynı sağlayıcı+profili taşıyorsa eşleme turlar arasında gidip geliyor
// ("flapping"), istek bir turda A sekmesine, sonraki turda B sekmesine
// düşüyordu — yanıt kimliği (`id`) eşleşse de istem BAŞKA bir sohbet
// penceresine yazılıyor, bağlam kayboluyordu. Çözüm: eşleme YAPIŞKAN. Bir
// anahtar için seçilmiş sekme hâlâ erişilebilirse o kalır; puan yalnız
// (a) ilk seçimde ve (b) seçili sekme kapandığında devreye girer. Puan eşitse
// küçük `tabId` kazanır — seçim böylece tarayıcı sırasından da bağımsızdır.
async function acikSekmeleriTara() {
  try {
    const tabs = await api.tabs.query({});
    const ulasilan = new Set();
    // k -> { enIyi, sekmeler: Map(tabId -> aday) }
    const adaylar = new Map();
    for (const tab of tabs) {
      if (!tab.url) continue;
      // Boşaltılmış (discarded) sekme: içerik betiği HİÇ çalışmamıştır.
      if (tab.discarded) continue;
      const provider = saglayiciBul(tab.url);
      if (!provider) continue;
      if (!(await sekmeHazirMi(tab, provider))) continue;

      ulasilan.add(tab.id);
      const cId = await window.kervanProfilKimligi(tab.cookieStoreId);
      const k = anahtar(provider, cId);
      const aday = {
        tabId: tab.id,
        url: tab.url,
        provider,
        cId,
        puan: (tab.active ? 1e12 : 0) + (tab.lastAccessed || 0)
      };
      const depo = adaylar.get(k);
      if (!depo) {
        adaylar.set(k, { enIyi: aday, sekmeler: new Map([[tab.id, aday]]) });
        continue;
      }
      depo.sekmeler.set(tab.id, aday);
      if (
        aday.puan > depo.enIyi.puan ||
        (aday.puan === depo.enIyi.puan && aday.tabId < depo.enIyi.tabId)
      ) {
        depo.enIyi = aday;
      }
    }
    for (const [k, depo] of adaylar) {
      // Yapışkanlık: mevcut eşleme bu turda hâlâ erişilebilirse korunur.
      const mevcut = bagliSekmeler.get(k);
      const sec =
        mevcut !== undefined && depo.sekmeler.has(mevcut)
          ? depo.sekmeler.get(mevcut)
          : depo.enIyi;
      // TAZELEME: eşleme değişmese de kayıt YENİDEN gönderilir. Sunucu,
      // kaydın `son_gorulme` damgasını tazeler ve bayat kaydı düşürür; kenar
      // tetiklemeli kayıtla canlı bir sekme 90 sn sonra "ölü" sayılıyordu
      // (2026-11-14 ölçümü). Sunucu logu yalnız YENİ kayıtta yazar.
      if (bagliSekmeler.get(k) !== sec.tabId) {
        console.log(`[Kervan] Açık sekme seçildi: ${k} (Tab: ${sec.tabId})`);
        bagliSekmeler.set(k, sec.tabId);
      }
      kervanaKaydet(sec.provider, sec.url, sec.cId, sec.tabId);
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

/**
 * Yeniden bağlanmayı planlar.
 *
 * NEDEN AYRI: zamanlayıcı tutamacı, iş çalışırken sıfırlanmazsa tek bir başarısız
 * deneme tutamacı dolu bırakır ve `if (!yenidenBaglanmaZamanlayicisi)` kapısı bir
 * daha açılmaz. Ölçüldü 2026-10-24: sunucu yeniden başlatıldığında açık yedi
 * tarayıcı köprüye geri DÖNMEDİ; her pencere elle yeniden açılmak zorunda kaldı.
 */
function yenidenDene(ms = 3000) {
  if (yenidenBaglanmaZamanlayicisi) return;
  yenidenBaglanmaZamanlayicisi = setTimeout(() => {
    yenidenBaglanmaZamanlayicisi = null;
    kervanaBaglan();
  }, ms);
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
    yenidenDene();
    return;
  }

  console.log("[Kervan Extension] Kervan yerel ağ geçidine bağlanılıyor:", WS_URL);
  let yeniSoket;
  try {
    yeniSoket = new WebSocket(`${WS_URL}?token=${encodeURIComponent(t)}`);
  } catch (err) {
    console.error("[Kervan Extension] WebSocket başlatma hatası:", err);
    yenidenDene();
    return;
  }
  socket = yeniSoket;

  yeniSoket.onopen = () => {
    console.log("[Kervan Extension] Kervan ağ geçidine bağlandı!");
    if (yenidenBaglanmaZamanlayicisi) {
      clearTimeout(yenidenBaglanmaZamanlayicisi);
      yenidenBaglanmaZamanlayicisi = null;
    }

    // Sekme kaydı KENAR-tetiklemeliydi: eşleme değişmedikçe `kayit` yeniden
    // gönderilmiyordu. Sunucu yeniden başlatıldığında yeni oturum boş kalıyor,
    // WebSocket ayakta olduğu hâlde hesap panelden kayboluyordu (ölçüldü
    // 2026-10-24: bağlantı Established, /eklenti/sekmeler `sayi: 0`). Artık her
    // bağlantı kuruluşunda kayıt tazelenir. Ayrıca periyodik tarama artık her
    // turda kayıt gönderir (sunucu tarafı tazelik damgası, 2026-11-14).
    bagliSekmeler.clear();
    acikSekmeleriTara();
  };

  yeniSoket.onmessage = async (event) => {
    // Kimlik kapısı: eski soketin gecikmiş mesajı yeni oturumun akışına
    // karışmasın.
    if (yeniSoket !== socket) return;
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
            yeniSoket.send(JSON.stringify({
              action: "hata",
              id: msg.id,
              mesaj: `'${msg.provider}' için açık bir sekme bulunamadı.`
            }));
          }
        } catch (e) {
          console.warn("[Kervan] Sekmeye mesaj gönderilemedi...", e);
          yeniSoket.send(JSON.stringify({
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
            yeniSoket.send(JSON.stringify({
              action: "hata",
              id: msg.id,
              mesaj: `'${msg.provider}' için açık bir sekme bulunamadı.`
            }));
            return;
          }
          if (yanit && yanit.ok) {
            yeniSoket.send(JSON.stringify({ action: "ogeler", id: msg.id, ogeler: yanit.ogeler }));
          } else {
            yeniSoket.send(JSON.stringify({
              action: "hata",
              id: msg.id,
              mesaj: (yanit && yanit.mesaj) || "seçim turu yanıtsız kaldı"
            }));
          }
        } catch (e) {
          yeniSoket.send(JSON.stringify({
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

  yeniSoket.onclose = () => {
    // KİMLİK KAPISI: geç kalan bir kapanış bildirimi, o sırada ayakta olan
    // YENİ soketi `null`'layıp fazladan bağlantı açıyordu (ölçüldü
    // 2026-11-15: 3 soket, ikisi CONNECTING). Yalnız hâlâ güncel olan
    // soketin kapanışı yeniden denemeyi tetikler.
    if (yeniSoket !== socket) {
      console.log("[Kervan Extension] Eski soketin kapanışı yok sayıldı");
      return;
    }
    console.log("[Kervan Extension] Bağlantı kapandı, 3 sn sonra yeniden denenecek...");
    socket = null;
    // Token dosyası silinmiş/yenilenmiş olabilir: sonraki denemede yeniden al.
    tokenYenile = true;
    yenidenDene();
  };

  yeniSoket.onerror = (err) => {
    console.warn("[Kervan Extension] WebSocket hatası:", err);
    if (yeniSoket !== socket) return;
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
    // Kimlik dosyası (kervan-profil.json) okuması async olduğu için yanıt da
    // gecikmeli gönderilir: `return true` async yanıt kanalını açar.
    window
      .kervanProfilKimligi(sender.tab ? sender.tab.cookieStoreId : null)
      .then((profile) => {
        const k = anahtar(provider, profile);
        console.log(`[Kervan Extension] Sekme kayıt oldu: ${k} (Tab ID: ${tabId})`);
        if (tabId) {
          bagliSekmeler.set(k, tabId);
        }
        kervanaKaydet(provider, sender.tab ? sender.tab.url : "", profile, tabId || 0);
        sendResponse({ ok: true });
      });
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
  // Enjeksiyon defteri de temizlenir: tabId'ler yeniden kullanılabilir,
  // silinmezse yeni bir sekme eski url kaydı yüzünden enjeksiyon hakkını
  // harcanmış bulur.
  enjeksiyonDenemeleri.delete(tabId);
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

/**
 * Taramayı kısa bir gecikmeyle tetikler (olay uyandırıcısı).
 *
 * NEDEN GECİKME: `tabs.onUpdated` bir yüklemede birkaç kez ateşler; her
 * ateşlemede tam tarama yapmak (her sekme için `tabs.sendMessage` ping'i)
 * boşa iş olur. 1 sn'lik pencere aynı yüklemenin olaylarını tek taramaya
 * indirger.
 */
function taramayiTetikle() {
  if (guncellemeZamanlayicisi) return;
  guncellemeZamanlayicisi = setTimeout(() => {
    guncellemeZamanlayicisi = null;
    acikSekmeleriTara();
  }, 1000);
}

// B1 (2026-11-15 denetimi): tazelik döngüsünün İKİNCİ dayanağı.
//
// NEDEN GEREKLİ: `manifest.json`da `background.scripts` kullanıldığı için
// Firefox bunu kalıcı OLMAYAN bir olay sayfası (event page) olarak çalıştırır
// ve `extensions.background.idle.timeout` (varsayılan 30000 ms) dolduğunda
// sayfayı yok eder. Açık bir WebSocket bu sayacı SIFIRLAMAZ (Mozilla bug
// 1748533, WONTFIX). Önceden sayfayı ayakta tutan tek şey `setInterval`
// turundaki `tabs.query` çağrısıydı: bir kez 30 sn kaçırılırsa zamanlayıcı
// sayfayla birlikte ölüyor ve döngü KALICI olarak kırılıyordu (toparlanma
// yalnız `runtime.onMessage` ya da `tabs.onRemoved` ile).
//
// `alarms` izniyle periyodik bir alarm, sayfa uyusa bile tarayıcı tarafından
// çalıştırılır ve sayfayı uyandırır; ayrıca `tabs.onUpdated` her sekme
// yüklemesinde ek bir uyandırıcıdır. `setInterval` üçüncü dayanak olarak
// KALIR (alarm en erken 30 sn'de bir çalışabilir; sık tarama için gerekli).
if (api.alarms && api.alarms.create) {
  api.alarms.create("kervan-sekme-tarama", { periodInMinutes: 0.5 });
  api.alarms.onAlarm.addListener((alarm) => {
    if (alarm.name === "kervan-sekme-tarama") acikSekmeleriTara();
  });
}

if (api.tabs.onUpdated) {
  api.tabs.onUpdated.addListener((tabId, degisim) => {
    if (degisim.status === "complete" || degisim.url) taramayiTetikle();
  });
}

// Başlangıçta bağlan
kervanaBaglan();

// Periyodik olarak sekmeleri kontrol et (her 10 sn) — bkz. yukarıdaki B1 notu.
setInterval(acikSekmeleriTara, 10000);
