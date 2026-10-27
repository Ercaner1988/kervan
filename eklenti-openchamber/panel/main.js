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

  const gonder = (tip, payload, zamanAsimi = ISTEK_ZAMANI_MS) =>
    new Promise((coz, reddet) => {
      const id = `kervan-${Date.now()}-${++sayac}`;
      const zaman = setTimeout(() => {
        bekleyen.delete(id);
        reddet(new Error("OpenChamber yanıt vermedi (zaman aşımı)."));
      }, zamanAsimi);
      bekleyen.set(id, { coz, reddet, zaman });
      window.parent.postMessage({ channel: KANAL, v: SURUM, type: tip, id, payload }, "*");
    });

  /**
   * Yerel servise istek. Yanıt `{status, body}`; body JSON metni.
   * HTTP hatası da Error'a çevrilir ki panel tek yerde yakalayabilsin.
   */
  const servis = async (method, path, body, zamanAsimi) => {
    const istek = { method, path };
    if (body !== undefined) istek.body = JSON.stringify(body);
    const sonuc = await gonder("service-request", istek, zamanAsimi);
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

  const hesapNotu = (metin, sinif = "alt") => {
    const not = $("#hesapNot");
    not.textContent = metin;
    not.className = sinif;
  };

  /**
   * Sağlayıcı seçicisini tanımlı defterden doldurur. Liste değişmediyse
   * dokunmaz: 5 sn'lik tazeleme kullanıcının seçimini sıfırlamamalı.
   */
  const cizHesapEkle = (durum) => {
    const secim = $("#hesapSaglayici");
    const tanimli = Array.isArray(durum.saglayicilar) ? durum.saglayicilar : [];
    const mevcut = [...secim.options].map((o) => o.value);
    if (mevcut.join("|") === tanimli.join("|")) return;
    secim.textContent = "";
    for (const ad of tanimli) {
      const secenek = eleman("option", null, ad);
      secenek.value = ad;
      secim.append(secenek);
    }
    if (tanimli.length === 0) {
      hesapNotu("Tanımlı sağlayıcı yok; kervan.toml → [providers] gerekli.", "uyari");
    }
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

  // ── Model ve özellik seçimi ────────────────────────────────────────────────
  // Seçenekler SUNUCUDAN gelir (/kervan/secenekler): yalnız kervan.toml'da
  // ÖLÇÜLMÜŞ model kodları ve aç/kapa anahtarları. Panelde serbest metin model
  // adı yoktur; olmayan bir ad yazmak 400'e düşer ve kullanıcı bunu ancak
  // yanıt gelmeyince anlardı.
  let secenekOnbellek = null;

  const secenekGetir = async (yenileZorla = false) => {
    if (!secenekOnbellek || yenileZorla) {
      const veri = await servis("GET", "/secenekler");
      secenekOnbellek = veri?.saglayicilar ?? {};
    }
    return secenekOnbellek;
  };

  const secModeliDoldur = (ad) => {
    const secim = $("#secModel");
    const kayit = secenekOnbellek?.[ad];
    const modeller = Array.isArray(kayit?.modeller) ? kayit.modeller : [];
    secim.textContent = "";

    // Bos deger = site varsayilani: istekte `kervan/<ad>` gonderilir. Bu, sitede
    // hicbir menuye dokunmamak demektir; "secilmis gibi" davranilmaz.
    const varsayilan = eleman("option", null, "site varsayılanı");
    varsayilan.value = "";
    varsayilan.dataset.kimlik = `kervan/${ad}`;
    secim.append(varsayilan);

    // `kervan.toml` listesi saglayici GENELI bir varsayimdir. Olcum defteri
    // varsa gercek menuler hesap basina ayri gosterilir: ayni "3.1 Pro" bir
    // hesapta secilebilirken digerinde kilitli. Ikisini tek listeye karistirmak
    // "secilir sandim, 402 aldim" durumunu uretirdi.
    const olculen = kayit?.olcum?.hesaplar ?? {};
    const olcumVar = Object.keys(olculen).length > 0;
    if (olcumVar) {
      const grup = eleman("optgroup", null);
      grup.label = "yapılandırma (varsayım)";
      for (const m of modeller) {
        const etiket = m.kilitli
          ? `${m.etiket} · ${m.kilit_nedeni ?? "plan gerekir"}`
          : m.etiket;
        const secenek = eleman("option", null, etiket ?? m.kod);
        secenek.value = m.kod;
        if (m.kilitli) secenek.disabled = true;
        secenek.dataset.kimlik = `kervan/${ad}/${m.kod}`;
        grup.append(secenek);
      }
      if (grup.childElementCount) secim.append(grup);
      for (const hesap of Object.keys(olculen).sort()) {
        const kayitHesap = olculen[hesap] ?? {};
        const hesapModelleri = Array.isArray(kayitHesap.modeller)
          ? kayitHesap.modeller
          : [];
        if (!hesapModelleri.length) continue;
        const hesapGrubu = eleman("optgroup", null);
        hesapGrubu.label = `ölçüldü · ${hesap}`;
        for (const m of hesapModelleri) {
          const etiket = m.kilitli
            ? `${m.etiket} · ${m.kilit_nedeni ?? "plan gerekir"}`
            : m.etiket;
          const secenek = eleman("option", null, etiket ?? m.kod);
          secenek.value = `@${hesap}:${m.kod}`;
          if (m.kilitli) secenek.disabled = true;
          secenek.dataset.kimlik = `kervan/${ad}/${m.kod}@${hesap}`;
          hesapGrubu.append(secenek);
        }
        secim.append(hesapGrubu);
      }
    } else {
      for (const m of modeller) {
        // Kilitli secenek GORUNUR ama secilemez: hesabin potansiyeli gorunur,
        // yanlislikla secilip 402 almak yerine neden yazili olur.
        const etiket = m.kilitli
          ? `${m.etiket} · ${m.kilit_nedeni ?? "plan gerekir"}`
          : m.etiket;
        const secenek = eleman("option", null, etiket ?? m.kod);
        secenek.value = m.kod;
        if (m.kilitli) secenek.disabled = true;
        secenek.dataset.kimlik = `kervan/${ad}/${m.kod}`;
        secim.append(secenek);
      }
    }
    secim.disabled = secim.options.length <= 1;
    kimlikYaz(ad);
  };

  const kimlikYaz = (ad) => {
    const secili = $("#secModel").selectedOptions?.[0];
    $("#secKimlik").textContent = secili?.dataset?.kimlik || `kervan/${ad}`;
  };

  const secAnahtarlariDoldur = (ad) => {
    const kap = $("#secAnahtarlar");
    kap.textContent = "";
    const anahtarlar = Array.isArray(secenekOnbellek?.[ad]?.anahtarlar)
      ? secenekOnbellek[ad].anahtarlar
      : [];
    if (anahtarlar.length === 0) return;
    for (const a of anahtarlar) {
      const satir = eleman("div", "satir");
      const secim = eleman("select");
      secim.dataset.anahtar = a.kod;
      secim.setAttribute("aria-label", a.etiket ?? a.kod);
      for (const [deger, metin] of [
        ["", "değiştirme"],
        ["acik", "açık"],
        ["kapali", "kapalı"],
      ]) {
        const secenek = eleman("option", null, metin);
        secenek.value = deger;
        secim.append(secenek);
      }
      satir.append(eleman("span", "ad", a.etiket ?? a.kod), secim);
      kap.append(satir);
    }
  };

  let secImza = "";

  const cizSecenekler = async () => {
    const veri = await secenekGetir();
    const adlar = Object.keys(veri).sort();
    const secim = $("#secSaglayici");
    const mevcut = [...secim.options].map((o) => o.value);
    if (mevcut.join("|") !== adlar.join("|")) {
      secim.textContent = "";
      for (const ad of adlar) {
        const secenek = eleman("option", null, ad);
        secenek.value = ad;
        secim.append(secenek);
      }
    }
    if (adlar.length === 0) {
      secModeliDoldur("");
      $("#secAnahtarlar").textContent = "";
      $("#secKimlik").textContent = "";
      secImza = "";
      return;
    }
    if (!adlar.includes(secim.value)) secim.value = adlar[0];

    // 5 sn'lik tazeleme kullanıcının seçimini SIFIRLAMAMALI: aynı sağlayıcı için
    // model/anahtar listesi değişmediyse DOM'a dokunulmaz.
    const kayit = veri[secim.value] ?? {};
    const imza = [
      secim.value,
      (kayit.modeller ?? [])
        .map((m) => `${m.kod}:${m.kilitli ? 1 : 0}`)
        .join(","),
      // Ölçüm defteri de imzaya girer: yeni bir ölçüm (ya da hesap listesi
      // değişimi) panelde görünmeli, yoksa 5 sn'lik tazeleme eski listeyi tutar.
      Object.entries(kayit.olcum?.hesaplar ?? {})
        .map(
          ([hesap, o]) =>
            `${hesap}=${(o.modeller ?? [])
              .map((m) => `${m.kod}:${m.kilitli ? 1 : 0}`)
              .join("+")}`,
        )
        .join(","),
      (kayit.anahtarlar ?? []).map((a) => a.kod).join(","),
    ].join("|");
    if (imza === secImza) return;
    secImza = imza;
    secModeliDoldur(secim.value);
    secAnahtarlariDoldur(secim.value);
  };

  const secimGovdesi = () => {
    const ad = $("#secSaglayici").value;
    if (!ad) throw new Error("Önce bir sağlayıcı seçin.");
    const secili = $("#secModel").selectedOptions?.[0];
    // Kimlik `data-kimlik`ten gelir: ölçülmüş seçeneklerde hesap eki taşır
    // (`kervan/gemini/4-argon@hesap`). Kodu yeniden kurmak hesabı düşürürdü.
    const kimlik = secili?.dataset?.kimlik || `kervan/${ad}`;
    const anahtarlar = {};
    for (const secim of document.querySelectorAll("#secAnahtarlar select")) {
      if (!secim.value) continue; // "değiştirme" → istekte yok
      anahtarlar[secim.dataset.anahtar] = secim.value === "acik";
    }
    const govde = {
      model: kimlik,
      messages: [{ role: "user", content: $("#secIstem").value.trim() || "merhaba" }],
    };
    if (Object.keys(anahtarlar).length) govde.kervan = { anahtarlar };
    return govde;
  };

  const secDene = () => calistir(async () => {
    const govde = secimGovdesi();
    $("#secNot").textContent = "İstek gönderiliyor: model uygulanmazsa sunucu açık hata döner…";
    $("#secNot").className = "alt";
    $("#secYanit").classList.add("gizli");
    try {
      // Tarayıcı turu 120 sn'yi bulabiliyor; genel 35 sn'lik sınır yetmez.
      const yanit = await servis("POST", "/dene", govde, 180_000);
      const metin = yanit?.choices?.[0]?.message?.content ?? "(boş yanıt)";
      $("#secNot").textContent = `${govde.model} · yanıt geldi`;
      $("#secYanit").textContent = metin;
      $("#secYanit").classList.remove("gizli");
      await yenile();
    } catch (hata) {
      // Sunucunun 400/501 mesajı burada görünür: "seçtim ama uygulanmadı" sessizliği yok.
      $("#secNot").textContent = hata.message ?? String(hata);
      $("#secNot").className = "hata";
    }
  });

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
    cizHesapEkle(durum);
    cizWebHesaplar(durum);
    cizKota(durum);
    // Secenekler ayri bir uctan gelir; hata olsa bile panelin geri kalani cizili kalir.
    cizSecenekler().catch((hata) => {
      $("#secNot").textContent = hata.message ?? String(hata);
      $("#secNot").className = "hata";
    });
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

  const hesapAc = () => calistir(async () => {
    const saglayici = $("#hesapSaglayici").value;
    const eposta = $("#hesapEposta").value.trim();
    if (!saglayici) throw new Error("Önce sağlayıcı seçin.");
    if (!eposta) throw new Error("E-posta girin.");
    hesapNotu("Giriş penceresi açılıyor…");
    const sonuc = await servis("POST", "/hesap/ac", { eposta, saglayici }, 60_000);
    hesapNotu(sonuc?.mesaj ?? "Giriş penceresi açıldı.");
  });

  const hesapDogrula = () => calistir(async () => {
    const saglayici = $("#hesapSaglayici").value;
    const eposta = $("#hesapEposta").value.trim();
    if (!saglayici) throw new Error("Önce sağlayıcı seçin.");
    if (!eposta) throw new Error("E-posta girin.");
    hesapNotu("Giriş ölçülüyor: tarayıcı açılıyor, sayfa bekleniyor…");
    // geckodriver + tarayıcı + yerleşme: 35 sn'lik genel sınır yetmez.
    const sonuc = await servis("POST", "/hesap/dogrula", { eposta, saglayici }, 150_000);
    $("#hesapYenidenBaslat").classList.toggle("gizli", !sonuc?.yeniden_baslat);
    hesapNotu(sonuc?.mesaj ?? "Giriş durumu okunamadı.", sonuc?.giris ? "alt" : "uyari");
  });

  const hesabiDevreyeAl = () => calistir(async () => {
    hesapNotu("Kervan yeniden başlatılıyor…");
    const sonuc = await servis("POST", "/yeniden-baslat", undefined, 90_000);
    if (!sonuc?.ok) throw new Error(sonuc?.mesaj ?? "Kervan yeniden başlatılamadı.");
    $("#hesapYenidenBaslat").classList.add("gizli");
    hesapNotu("Kervan yeniden başladı; yeni hesap havuzda görünmeli.");
    await yenile();
  });

  $("#yenile").addEventListener("click", () => calistir(yenile));
  $("#baslat").addEventListener("click", basla);
  $("#uygula").addEventListener("click", uygula);
  // Saglayici/model degisince kimlik ve secenekler tazelenir. Model listesi ayni
  // saglayici icin onbellekten gelir; secim kullaniciya kalir (imza degismedikce
  // DOM'a dokunulmaz).
  $("#secSaglayici").addEventListener("change", () => {
    secImza = "";
    cizSecenekler().catch((hata) => {
      $("#secNot").textContent = hata.message ?? String(hata);
      $("#secNot").className = "hata";
    });
  });
  $("#secModel").addEventListener("change", () => kimlikYaz($("#secSaglayici").value));
  $("#secDene").addEventListener("click", secDene);
  $("#hesapAc").addEventListener("click", hesapAc);
  $("#hesapDogrula").addEventListener("click", hesapDogrula);
  $("#hesapYenidenBaslat").addEventListener("click", hesabiDevreyeAl);

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
