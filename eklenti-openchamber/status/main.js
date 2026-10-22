// Kervan — çalışma durumu şeridi.
//
// Panelin küçük kardeşi: yalnız özet gösterir (açık/kapalı, hesap sayısı,
// yönlendirme stratejisi, bağlı tarayıcı sekmeleri). Domain mantığı Kervan'da;
// burası host köprüsü üzerinden `/durum` okur, kapalıysa `Başlat` sunar.
//
// Host protokolü paneldeki ile aynı zarfı kullanır:
//   { channel: "openchamber.sdk", v: 1, type, id, payload }
// Ek olarak yükseklik bildirimi: type "resize", payload { height }.

(() => {
  "use strict";

  const KANAL = "openchamber.sdk";
  const SURUM = 1;
  const ISTEK_ZAMANI_MS = 20_000;
  const YOKLAMA_MS = 4_000;

  let sayac = 0;
  let hostHazir = false;
  const bekleyen = new Map();

  const $ = (secici) => document.querySelector(secici);
  const nokta = $("#nokta");
  const ozet = $("#ozet");
  const hataKutusu = $("#hata");
  const baslatDugmesi = $("#baslat");

  const hostMesaji = (olay) => {
    if (olay.source !== window.parent) return;
    const mesaj = olay.data;
    if (!mesaj || mesaj.channel !== KANAL || mesaj.v !== SURUM) return;
    if (mesaj.type === "ready") {
      hostHazir = true;
      return;
    }
    if (mesaj.type !== "result" || typeof mesaj.id !== "string") return;
    const kayit = bekleyen.get(mesaj.id);
    if (!kayit) return;
    bekleyen.delete(mesaj.id);
    clearTimeout(kayit.zaman);
    if (mesaj.ok === true) kayit.coz(mesaj.payload ?? {});
    else kayit.reddet(new Error(String(mesaj.error ?? "OpenChamber isteği reddetti")));
  };
  window.addEventListener("message", hostMesaji);

  const gonder = (tip, payload) =>
    new Promise((coz, reddet) => {
      const id = `kervan-durum-${Date.now()}-${++sayac}`;
      const zaman = setTimeout(() => {
        bekleyen.delete(id);
        reddet(new Error("OpenChamber yanıt vermedi (zaman aşımı)."));
      }, ISTEK_ZAMANI_MS);
      bekleyen.set(id, { coz, reddet, zaman });
      window.parent.postMessage({ channel: KANAL, v: SURUM, type: tip, id, payload }, "*");
    });

  const servis = async (method, path, body) => {
    const istek = { method, path };
    if (body !== undefined) istek.body = JSON.stringify(body);
    const sonuc = await gonder("service-request", istek);
    let veri = null;
    if (typeof sonuc.body === "string" && sonuc.body.trim()) {
      try {
        veri = JSON.parse(sonuc.body);
      } catch {
        veri = { hata: sonuc.body };
      }
    }
    if (typeof sonuc.status === "number" && sonuc.status >= 400) {
      throw new Error(veri?.hata ?? veri?.error?.message ?? `Kervan servisi HTTP ${sonuc.status}`);
    }
    return veri;
  };

  const hataGoster = (metin) => {
    if (!metin) {
      hataKutusu.textContent = "";
      hataKutusu.classList.add("gizli");
      return;
    }
    hataKutusu.textContent = metin;
    hataKutusu.classList.remove("gizli");
  };

  const yuksekligiBildir = () => {
    const yukseklik = Math.ceil(document.body.getBoundingClientRect().height);
    // `resize` bir istek değil bildirimdir; yanıt beklemeyiz.
    window.parent.postMessage(
      { channel: KANAL, v: SURUM, type: "resize", id: `kervan-durum-${Date.now()}-${++sayac}`, payload: { height: yukseklik } },
      "*",
    );
  };

  const ciz = (durum) => {
    const acik = durum?.kervan === true;
    nokta.classList.toggle("acik", acik);
    nokta.classList.toggle("kapali", !acik);

    if (!acik) {
      ozet.textContent = "kapalı";
      baslatDugmesi.classList.remove("gizli");
      yuksekligiBildir();
      return;
    }

    baslatDugmesi.classList.add("gizli");
    const hesap = Array.isArray(durum?.accounts?.accounts) ? durum.accounts.accounts.length : 0;
    const strateji = durum?.yonlendirme?.strateji ?? "—";
    const sekmeler = Array.isArray(durum?.eklenti?.bagli_sekmeler) ? durum.eklenti.bagli_sekmeler.length : 0;
    const parcalar = [`${hesap} hesap`, strateji];
    parcalar.push(sekmeler > 0 ? `${sekmeler} sekme` : "tarayıcı yok");
    ozet.textContent = parcalar.join(" · ");
    yuksekligiBildir();
  };

  let mesgul = false;
  const yenile = async () => {
    if (mesgul || document.hidden) return;
    mesgul = true;
    try {
      const durum = await servis("GET", "/durum");
      ciz(durum);
      hataGoster("");
    } catch (hata) {
      nokta.classList.remove("acik", "kapali");
      ozet.textContent = "ulaşılamadı";
      hataGoster(hata.message ?? String(hata));
      yuksekligiBildir();
    } finally {
      mesgul = false;
    }
  };

  baslatDugmesi.addEventListener("click", () => {
    baslatDugmesi.disabled = true;
    void (async () => {
      try {
        await servis("POST", "/baslat");
        await yenile();
      } catch (hata) {
        hataGoster(hata.message ?? String(hata));
      } finally {
        baslatDugmesi.disabled = false;
      }
    })();
  });

  document.addEventListener("visibilitychange", () => {
    if (!document.hidden) void yenile();
  });
  new ResizeObserver(() => yuksekligiBildir()).observe(document.body);

  // Host hazır olunca ilk okuma; hazır değilse de dene (hello sonrası gelir).
  window.parent.postMessage({ channel: KANAL, v: SURUM, type: "hello" }, "*");
  void yenile();
  setInterval(() => {
    if (hostHazir) void yenile();
  }, YOKLAMA_MS);
})();
