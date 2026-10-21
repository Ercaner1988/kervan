// Kervan paneli — OpenChamber host köprüsü.
//
// Sandbox içindeki iframe ES modülü yükleyemez; bu yüzden SDK'yı paketlemek
// yerine host protokolünün ihtiyacımız olan üç mesajı (hello / ready /
// result) doğrudan konuşulur. Protokol `@openchamber/sdk` içindeki
// `host.ts` ve `contract.js` ile aynı zarfı kullanır:
//   { channel: "openchamber.sdk", v: 1, type, id, payload }
// Sunucu tarafı `service-request` yalnız {method, path, body} taşır.
(() => {
  "use strict";

  const KANAL = "openchamber.sdk";
  const SURUM = 1;
  const ISTEK_ZAMANI_MS = 35_000; // /baslat Kervan'ı 20 sn'ye kadar bekler

  let sayac = 0;
  let hostHazir = false;
  const bekleyen = new Map();

  const $ = (secici) => document.querySelector(secici);

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
    if (mesaj.ok === true) {
      kayit.coz(mesaj.payload ?? {});
    } else {
      const hata = new Error(String(mesaj.error ?? "OpenChamber isteği reddetti"));
      hata.kod = mesaj.code;
      kayit.reddet(hata);
    }
  };
  window.addEventListener("message", hostMesaji);

  const gonder = (tip, payload) =>
    new Promise((coz, reddet) => {
      const id = `kervan-${Date.now()}-${++sayac}`;
      const zaman = setTimeout(() => {
        bekleyen.delete(id);
        reddet(new Error("OpenChamber yanıt vermedi (zaman aşımı)."));
      }, ISTEK_ZAMANI_MS);
      bekleyen.set(id, { coz, reddet, zaman });
      window.parent.postMessage({ channel: KANAL, v: SURUM, type: tip, id, payload }, "*");
    });

  /**
   * Yerel servise istek. Yanıt `{status, body}`; body JSON metni.
   * HTTP hatası da Error'a çevrilir ki panel tek yerde yakalayabilsin.
   */
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
      const mesaj = veri?.hata ?? veri?.error?.message ?? `Kervan servisi HTTP ${sonuc.status}`;
      throw new Error(mesaj);
    }
    return veri;
  };

  let sonDurum = null;
  let mesgul = false;

  const eleman = (etiket, sinif, metin) => {
    const d = document.createElement(etiket);
    if (sinif) d.className = sinif;
    if (metin !== undefined) d.textContent = metin;
    return d;
  };

  const hataGoster = (metin) => {
    const kutu = $("#hataKutusu");
    if (!metin) {
      kutu.textContent = "";
      kutu.classList.add("gizli");
      return;
    }
    kutu.textContent = metin;
    kutu.classList.remove("gizli");
  };

  const calistir = async (islev) => {
    if (mesgul) return;
    mesgul = true;
    try {
      await islev();
      hataGoster("");
    } catch (hata) {
      hataGoster(hata.message ?? String(hata));
    } finally {
      mesgul = false;
    }
  };

  const olcuKarti = (deger, etiket) => {
    const kart = eleman("div", "olcu");
    kart.append(eleman("b", null, String(deger)), eleman("span", null, etiket));
    return kart;
  };

  // /accounts gövdesi {accounts: [...], sabit_oturumlar: n}.
  const hesapListesi = (durum) => {
    if (Array.isArray(durum?.accounts)) return durum.accounts;
    if (Array.isArray(durum?.accounts?.accounts)) return durum.accounts.accounts;
    return [];
  };

  const cizOlcumler = (durum) => {
    const kap = $("#olcumler");
    kap.textContent = "";
    const accounts = hesapListesi(durum);
    const duraklatilan = accounts.filter((a) => a.duraklatildi || a.paused || a.paused_reason).length;
    const hesaplar = Array.isArray(durum.eklenti?.hesaplar) ? durum.eklenti.hesaplar : [];
    kap.append(
      olcuKarti(accounts.length, "havuz hesabı"),
      olcuKarti(duraklatilan, "duraklatılmış"),
      olcuKarti(hesaplar.length, "bağlı web hesabı"),
      olcuKarti(durum.telemetri?.token_tasarruf_orani_yuzde ?? "—", "token tasarrufu"),
    );
  };

  const cizStrateji = (durum) => {
    const y = durum.yonlendirme ?? {};
    const secim = $("#strateji");
    const liste = Array.isArray(y.kullanilabilir) ? y.kullanilabilir : [];
    const mevcut = y.strateji ?? "";

    secim.textContent = "";
    for (const ad of liste) {
      const secenek = eleman("option", null, ad);
      secenek.value = ad;
      secim.append(secenek);
    }
    if (mevcut && !liste.includes(mevcut)) {
      const secenek = eleman("option", null, mevcut);
      secenek.value = mevcut;
      secim.append(secenek);
    }
    if (mevcut) secim.value = mevcut;

    const not = $("#stratejiNot");
    if (y.desteklenir === false) {
      not.textContent = "Bu strateji desteklenmiyor; sunucu auto'ya düşer.";
      not.className = "uyari";
    } else {
      not.textContent = "";
      not.className = "alt";
    }
    $("#uygula").disabled = liste.length === 0;
  };

  const cizHesaplar = (durum) => {
    const kap = $("#hesaplar");
    kap.textContent = "";
    const accounts = hesapListesi(durum);
    if (accounts.length === 0) {
      kap.append(eleman("div", "bos", "Havuz hesabı yok (kervan.toml → [[accounts]])."));
      return;
    }

    accounts.forEach((hesap) => {
      const index = Number.isInteger(hesap.index) ? hesap.index : null;
      const duraklatildi = Boolean(hesap.duraklatildi || hesap.paused || hesap.paused_reason);
      const satir = eleman("div", "satir");

      const sol = eleman("div");
      sol.append(eleman("div", "ad", hesap.name ?? hesap.hesap_adi ?? `hesap ${index}`));
      const ayrinti = [];
      if (hesap.provider ?? hesap.saglayici) ayrinti.push(hesap.provider ?? hesap.saglayici);
      const istek = hesap.istek_sayisi ?? hesap.requests;
      if (Number.isFinite(istek)) ayrinti.push(`${istek} istek`);
      if (typeof hesap.duraklatildi === "string") ayrinti.push(`neden: ${hesap.duraklatildi}`);
      if (ayrinti.length) sol.append(eleman("div", "alt", ayrinti.join(" · ")));

      const sag = eleman("div", "sag");
      const rozet = eleman(
        "span",
        `rozet${duraklatildi ? " duraklatildi" : " acik"}`,
        duraklatildi ? "duraklatıldı" : "etkin",
      );
      const dugme = eleman("button", duraklatildi ? "birincil" : "tehlike", duraklatildi ? "Devam" : "Duraklat");
      dugme.addEventListener("click", () =>
        calistir(async () => {
          dugme.disabled = true;
          await servis("POST", duraklatildi ? "/accounts/devam" : "/accounts/duraklat", { index });
          await yenile();
        }),
      );
      sag.append(rozet, dugme);
      satir.append(sol, sag);
      kap.append(satir);
    });
  };

  const cizWebHesaplar = (durum) => {
    const kap = $("#webHesaplar");
    kap.textContent = "";
    const hesaplar = Array.isArray(durum.eklenti?.hesaplar) ? durum.eklenti.hesaplar : [];
    if (hesaplar.length === 0) {
      kap.append(
        eleman(
          "div",
          "bos",
          "Bağlı web sekmesi yok. Tarayıcı eklentisi açık ve kervan sayfası yüklü olmalı.",
        ),
      );
      return;
    }
    for (const hesap of hesaplar) {
      const satir = eleman("div", "satir");
      const parcalar = String(hesap).split(":");
      satir.append(
        eleman("span", "ad", parcalar[0] ?? String(hesap)),
        eleman("span", "alt", parcalar.slice(1).join(":") || "varsayilan"),
      );
      satir.append(eleman("span", "rozet acik", "bağlı"));
      kap.append(satir);
    }
  };

  const cizKota = (durum) => {
    const kap = $("#kota");
    kap.textContent = "";
    const kota = durum.kota;
    const satirlar = Array.isArray(kota) ? kota : Array.isArray(kota?.hesaplar) ? kota.hesaplar : null;
    if (!satirlar || satirlar.length === 0) {
      kap.append(eleman("div", "bos", "Kota verisi yok."));
      return;
    }
    for (const kayit of satirlar) {
      const satir = eleman("div", "satir");
      const sol = eleman("div");
      sol.append(eleman("div", "ad", kayit.hesap ?? kayit.name ?? "hesap"));
      if (kayit.saglayici ?? kayit.provider) {
        sol.append(eleman("div", "alt", kayit.saglayici ?? kayit.provider));
      }
      const detay = eleman("details");
      detay.append(
        eleman("summary", null, "kullanım"),
        eleman("pre", null, JSON.stringify(kayit.kullanim ?? kayit, null, 2)),
      );
      sol.append(detay);
      satir.append(sol);
      kap.append(satir);
    }
  };

  const ciz = (durum) => {
    sonDurum = durum;
    const acik = durum.kervan === true;
    $("#nokta").className = `nokta ${acik ? "acik" : "kapali"}`;
    $("#durumYazi").textContent = acik
      ? `açık · ${sonDurum?.telemetri?.aktif_yonlendirme_stratejisi ?? "—"}`
      : "kapalı";
    $("#baslat").classList.toggle("gizli", acik);
    $("#icerik").classList.toggle("gizli", !acik);
    if (!acik) return;

    cizOlcumler(durum);
    cizStrateji(durum);
    cizHesaplar(durum);
    cizWebHesaplar(durum);
    cizKota(durum);
  };

  const yenile = async () => {
    const durum = await servis("GET", "/durum");
    ciz(durum);
  };

  const basla = () => calistir(async () => {
    const dugme = $("#baslat");
    dugme.disabled = true;
    dugme.textContent = "Başlatılıyor…";
    try {
      const sonuc = await servis("POST", "/baslat");
      if (!sonuc?.ok) throw new Error(sonuc?.mesaj ?? "Kervan başlatılamadı.");
      await yenile();
    } finally {
      dugme.disabled = false;
      dugme.textContent = "Başlat";
    }
  });

  const uygula = () => calistir(async () => {
    const secim = $("#strateji");
    const strateji = secim.value;
    if (!strateji) throw new Error("Önce bir strateji seçin.");
    const sonuc = await servis("POST", "/yonlendirme", { strateji });
    if (sonuc?.ok === false) throw new Error(sonuc?.hata ?? "Strateji uygulanamadı.");
    await yenile();
    const onay = sonDurum?.yonlendirme?.strateji;
    if (onay && onay !== strateji) {
      throw new Error(`Sunucu '${strateji}' yerine '${onay}' bildirdi.`);
    }
  });

  $("#yenile").addEventListener("click", () => calistir(yenile));
  $("#baslat").addEventListener("click", basla);
  $("#uygula").addEventListener("click", uygula);

  // Host köprüsünü başlat: hello → host 'ready' ve ilk 'workspace' push'u yollar.
  window.parent.postMessage({ channel: KANAL, v: SURUM, type: "hello" }, "*");

  // İlk çizim: host hazır olmasa da istek kuyruğa girer, ready sonrası döner.
  calistir(yenile);

  // Panel açıkken hafif tazeleme; /durum zaten birkaç küçük GET.
  const zamanlayici = setInterval(() => {
    if (!mesgul && hostHazir) {
      calistir(async () => ciz(await servis("GET", "/durum"))).catch(() => {});
    }
  }, 5000);
  window.addEventListener("pagehide", () => clearInterval(zamanlayici));
})();
