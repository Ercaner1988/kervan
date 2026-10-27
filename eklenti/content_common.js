// Kervan WebExtension Ortak Content Script Yardımcıları

// React / Vue kontrollü input ve textarea alanlarına insan gibi metin yazma
window.kervanMetinYaz = function (element, metin) {
  element.focus();
  
  // React / Framework state güncellemesini tetiklemek için yerel setter çağrılır
  const nativeInputValueSetter = Object.getOwnPropertyDescriptor(
    element.tagName === "TEXTAREA" ? window.HTMLTextAreaElement.prototype : window.HTMLInputElement.prototype,
    "value"
  ).set;
  nativeInputValueSetter.call(element, metin);

  // Gerekli standart tarayıcı olayları tetiklenir
  element.dispatchEvent(new Event("input", { bubbles: true }));
  element.dispatchEvent(new Event("change", { bubbles: true }));
};

// Sayfada Cloudflare Turnstile veya bot duvarı belirdi mi denetler
window.kervanGuvenlikKontrolu = function (provider) {
  const cloudflareTurnstile = document.querySelector("iframe[src*='challenges.cloudflare.com']") ||
                             document.querySelector("#cf-challenge-running") ||
                             document.querySelector("#challenge-stage");

  if (cloudflareTurnstile) {
    chrome.runtime.sendMessage({
      type: "STATUS_UPDATE",
      provider: provider,
      durum: "CHALLENGE_BEKLIYOR",
      detay: "Cloudflare Turnstile doğrulaması sayfada aktif."
    }).catch(() => {});
    return true;
  }
  return false;
};

// Durdurma kaydı: Kervan `durdur` gönderdiğinde ilgili isteğin üretimi
// durdurulur. Her sağlayıcı betiği kendi bitiş denetiminde bu kümeye bakar.
window.kervanDurdurulanlar = window.kervanDurdurulanlar || new Set();
window.kervanDurdurulduMu = function (id) {
  return window.kervanDurdurulanlar.has(id);
};
// Belirli bir seçiciyi sayfada görünene kadar bekler
window.kervanOgeBekle = function (secici, zamanAsimiMs = 15000) {
  return new Promise((resolve, reject) => {
    const el = document.querySelector(secici);
    if (el) return resolve(el);

    const baslangic = Date.now();
    const observer = new MutationObserver(() => {
      const el = document.querySelector(secici);
      if (el) {
        observer.disconnect();
        resolve(el);
      } else if (Date.now() - baslangic > zamanAsimiMs) {
        observer.disconnect();
        reject(new Error(`Zaman aşımı: '${secici}' bulunamadı`));
      }
    });

    observer.observe(document.body, { childList: true, subtree: true });
  });
};

// === Seçim turları (D1d) ====================================================
// Eklenti yolunda model menüsü ve aç/kapa düğmeleri KONFİGÜRDEN gelir: bu betik
// siteye özgü değildir. Yaptığı tek iş ÖLÇMEKTİR — ham ad, tam metin, işaret.
// Katlama ve eşleştirme Rust tarafındadır (kervan-secim::hedef_indeks); burada
// katlama YAPILMAZ (iki katlama ayrışınca var olan seçenek "menüde yok"
// sayılmıştı, 2026-10-22 ölçümü).
//
// Ad okuma kuralı Rust'taki OGE_LISTESI sabitiyle AYNI olmak zorunda: önce
// `.label`, yoksa ilk satır; ikon düğümlerinin metniyle birebir eşleşen satırlar
// atlanır (AI Studio kartları ikon adını ilk satıra koyuyor).
window.kervanOgeOkuHam = function (secici) {
  const d = (s) => (s || "").replace(/[ \t\r]+/g, " ").trim();
  // SVG öğelerinde `className` bir nesnedir (SVGAnimatedString); doğrudan
  // regex'e vermek sınıf işaretini sessizce kaçırıyordu (2026-10-24).
  const sinif = (e) =>
    typeof e.className === "string" ? e.className : (e.className && e.className.baseVal) || "";
  const isaret = (e) =>
    /(^|[\s-])(selected|checked|active|on)(\s|$)/.test(sinif(e)) ||
    ["data-selected", "data-state", "aria-checked", "aria-selected", "aria-pressed"].some((a) =>
      ["true", "checked"].includes(e.getAttribute(a))
    ) ||
    !!e.querySelector('[aria-label="Seçildi"],[aria-label="Selected"],[aria-checked="true"]');
  // İŞARET KANITI: hangi kural eşleşti? 2026-10-24 ölçümünde iki öğe birden
  // "seçili" okundu ve hangi kuralın yanlış eşleştiği bilinmiyordu; tahmin
  // etmek yerine kuralın adı hata metnine taşınır.
  const isaretKaniti = (e) => {
    const m = /(^|[\s-])(selected|checked|active|on)(\s|$)/.exec(sinif(e));
    if (m) return `sinif:${m[2]}`;
    for (const a of ["data-selected", "data-state", "aria-checked", "aria-selected", "aria-pressed"]) {
      const v = e.getAttribute(a);
      if (["true", "checked"].includes(v)) return `${a}=${v}`;
    }
    // İKİ AYRI KANIT: "Seçildi" etiketi sitenin AÇIK seçim beyanıdır (güçlü).
    // Çıplak aria-checked=true ise satırdaki herhangi bir alt denetimin
    // durumu olabilir; 2026-10-24'te seçili OLMAYAN satırda eşleşti (zayıf).
    if (e.querySelector('[aria-label="Seçildi"],[aria-label="Selected"]')) return "torun:secildi";
    if (e.querySelector('[aria-checked="true"]')) return "torun:aria-checked=true";
    return "";
  };
  const ad = (e) => {
    const l = d((e.querySelector(".label") || {}).innerText);
    if (l) return l;
    const ikonlar = new Set(
      [...e.querySelectorAll('mat-icon,svg,[class*="icon"],[class*="symbol"],[class*="ligature"]')]
        .map((x) => d(x.innerText || x.textContent))
        .filter(Boolean)
    );
    const satirlar = (e.innerText || "").split("\n").map(d).filter(Boolean);
    return (
      satirlar.find((s) => !ikonlar.has(s)) ||
      satirlar[0] ||
      // Yalnız ikon taşıyan düğmelerde görünen ad `aria-label`/`title`dadır;
      // araç düğmelerini ölçerken metin boş kalıp liste işe yaramazdı.
      d(e.getAttribute("aria-label") || e.getAttribute("title"))
    );
  };
  return [...document.querySelectorAll(secici)].map((e) => ({
    metin: ad(e),
    tam: d((e.innerText || e.textContent || "").replace(/\s+/g, " ")),
    isaretli: isaret(e),
    kanit: isaretKaniti(e)
  }));
};

// Hata metnine sayfa bağlamı ekler: "menü bulunamadı" tek başına teşhis
// ettirmez. Giriş yapılmamış bir sekme ile değişmiş bir seçici aynı hatayı verir;
// adres + başlık ikisini ayırır (2026-10-23 canlı ölçümü).
function kervanSayfaIpucu() {
  const yol = `${location.host}${location.pathname}`.slice(0, 80);
  const baslik = (document.title || "").replace(/\s+/g, " ").slice(0, 60);
  const giris = document.querySelector('input[type="password"]') ? " [giriş formu]" : "";
  const kutu = document.querySelector("rich-textarea, textarea, [contenteditable='true']")
    ? " [istem kutusu]"
    : "";
  const etiketler = [...document.querySelectorAll("button[aria-label]")]
    .slice(0, 6)
    .map((b) => b.getAttribute("aria-label").slice(0, 28))
    .join(" / ");
  return `${yol} | "${baslik}"${giris}${kutu}${etiketler ? ` | düğmeler: ${etiketler}` : ""}`;
}

// Düğmenin gerçek durumu: tıklama neden işlemedi? (örtü / devre dışı / açık)
function kervanDugmeIpucu(d) {
  const r = d.getBoundingClientRect();
  const ortada = document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2);
  const ustune = !ortada ? "yok" : ortada === d || d.contains(ortada) ? "kendisi" : (ortada.tagName + (ortada.className ? "." + String(ortada.className).split(" ")[0] : ""));
  return [
    `genişlik=${Math.round(r.width)}`,
    `görünür=${r.width > 0 && r.height > 0}`,
    `disabled=${!!d.disabled}`,
    `aria-expanded=${d.getAttribute("aria-expanded")}`,
    `aria-haspopup=${d.getAttribute("aria-haspopup")}`,
    `merkezde=${ustune}`
  ].join(" ");
}

// Belge içinde hangi ARIA rolleri var? Menü açılmadığında "seçici mi değişti,
// menü mü hiç çizilmedi" ayrımını tek bakışta verir (2026-10-24 ölçümü).
function kervanRolIpucu() {
  const sayac = {};
  for (const e of document.querySelectorAll("[role]")) {
    const r = e.getAttribute("role");
    sayac[r] = (sayac[r] || 0) + 1;
  }
  const liste = Object.entries(sayac)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 8)
    .map(([r, n]) => `${r}=${n}`)
    .join(" ");
  return liste || "rol yok";
}

// İnsan tıklamasını taklit eder: bazı çerçeveler (Angular CDK, React menüleri)
// yalnız `click` olayına değil `pointerdown`/`mousedown` sırasına da bağlanır;
// sadece `el.click()` bu durumda menüyü açmıyor (2026-10-24 ölçümü).
function kervanTikla(el) {
  const o = { bubbles: true, cancelable: true, view: window };
  try {
    el.dispatchEvent(new PointerEvent("pointerdown", { ...o, pointerId: 1, isPrimary: true }));
  } catch (e) {
    el.dispatchEvent(new MouseEvent("mousedown", o));
  }
  el.dispatchEvent(new MouseEvent("mousedown", o));
  el.dispatchEvent(new MouseEvent("mouseup", o));
  el.click();
}

// Menü kapalıysa düğmeye basıp öğelerin çizilmesini bekler. `dugme` yoksa
// (aç/kapa düğmeleri her zaman sayfada) hiçbir şey yapmaz.
//
// STRATEJİ SIRASI, tek tiklamaya guvenmenin bedeli: 2026-10-24 olcumunde dugme
// gorunur/etkin/ortusu yoktu, `aria-haspopup=true` idi ama tek sentetik tiklama
// sonrasi `aria-expanded=false` kaliyordu. Iki ayri ariza ayni belirtiyi veriyor:
// (a) tik hic islenmiyor, (b) menu acilip ANINDA kapaniyor. Ikisini ayirt etmek
// ve ikincisini asmak icin her strateji ayri denenir ve MutationObserver
// "menu bir an gorundu mu" sayaci tutar. Hata metni bu sayaci tasir.
function kervanTus(el, key) {
  const kod = key === "Enter" ? 13 : 32;
  for (const tur of ["keydown", "keyup"]) {
    el.dispatchEvent(
      new KeyboardEvent(tur, { key, code: key, keyCode: kod, which: kod, bubbles: true })
    );
  }
}

async function kervanMenuAc(dugme, secici) {
  if (!dugme) return;
  if (document.querySelector(secici)) return;
  const d = document.querySelector(dugme);
  if (!d) {
    throw new Error(
      `model menüsü düğmesi bulunamadı ('${dugme}') — ${kervanSayfaIpucu()}`
    );
  }
  const stratejiler = [
    ["isaretci", () => kervanTikla(d)],
    ["click", () => d.click()],
    ["odak+enter", () => {
      d.focus();
      kervanTus(d, "Enter");
    }]
  ];
  const gunluk = [];
  for (const [ad, uygula] of stratejiler) {
    let goruldu = 0;
    const gozcu = new MutationObserver(() => {
      if (document.querySelector(secici)) goruldu++;
    });
    gozcu.observe(document.body, { childList: true, subtree: true });
    try {
      uygula();
    } catch (e) {
      gunluk.push(`${ad}: hata=${e.message}`);
      gozcu.disconnect();
      continue;
    }
    const bas = Date.now();
    while (Date.now() - bas < 2500) {
      if (document.querySelector(secici)) {
        gozcu.disconnect();
        return;
      }
      await new Promise((r) => setTimeout(r, 100));
    }
    gozcu.disconnect();
    gunluk.push(`${ad}: goruldu=${goruldu} expanded=${d.getAttribute("aria-expanded")}`);
    // Menu bir an gorunduyse "acildi-kapandi" durumundayiz: bir sonraki strateji
    // onu yeniden acabilir. Hic gorunmediyse tik islenmiyor demektir.
  }
  throw new Error(
    `model menüsü açılmadı ('${secici}') — denemeler: ${gunluk.join(" | ")} — düğme: ${kervanDugmeIpucu(d)} — rollar: ${kervanRolIpucu()} — ${kervanSayfaIpucu()}`
  );
}

function kervanMenuKapat(dugme) {
  if (!dugme) return;
  document.body.dispatchEvent(
    new KeyboardEvent("keydown", {
      key: "Escape",
      code: "Escape",
      keyCode: 27,
      which: 27,
      bubbles: true
    })
  );
}

// Bir seçim turu: oku ya da tıkla, GÜNCEL ham listeyi döndür.
//
// BOŞ LİSTE = MENÜ KAPANDI: sayfa menüyü açtıktan hemen sonra kapatabiliyor
// (animasyon/odak kaybı). Boş listeyi "menüde model yok" diye Rust'a bildirmek
// yanlış teşhis üretiyordu; burada bir kez yeniden açılır.
window.kervanSecimTuru = async function (mesaj) {
  const { komut, secici, dugme, indeks } = mesaj;
  const oku = async () => {
    for (let deneme = 0; deneme < 3; deneme++) {
      await kervanMenuAc(dugme, secici);
      await new Promise((r) => setTimeout(r, 150));
      const liste = window.kervanOgeOkuHam(secici);
      if (liste.length) return liste;
      await new Promise((r) => setTimeout(r, 250));
    }
    return [];
  };

  if (komut === "oge_bas") {
    await kervanMenuAc(dugme, secici);
    const ogeler = document.querySelectorAll(secici);
    if (!ogeler[indeks]) {
      throw new Error(
        `menü yenilendi: ${indeks}. öğe artık yok — ${kervanSayfaIpucu()}`
      );
    }
    kervanTikla(ogeler[indeks]);
    await new Promise((r) => setTimeout(r, 700));
  }
  const liste = await oku();
  kervanMenuKapat(dugme);
  return liste;
};

(typeof browser !== "undefined" ? browser : chrome).runtime.onMessage.addListener(
  (message, sender, sendResponse) => {
    // Hazır mı denetimi: arka plan, URL'ye değil bu yanıta bakarak sekme
    // kaydeder. Yanıt gelmiyorsa içerik betiği yok demektir.
    if (message.type === "KERVAN_PING") {
      sendResponse({ ok: true });
      return;
    }
    if (message.type === "KERVAN_SECIM") {
      window
        .kervanSecimTuru(message)
        .then((ogeler) => sendResponse({ ok: true, ogeler }))
        .catch((e) => sendResponse({ ok: false, mesaj: e.message }));
      return true; // yanıt asenkron gönderilecek
    }
    if (message.type === "KERVAN_STOP" && message.id) {
      window.kervanDurdurulanlar.add(message.id);
    }
  }
);
