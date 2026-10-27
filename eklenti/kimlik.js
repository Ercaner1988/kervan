// Kervan eklenti kimliği — bir tarayıcı profilinin hesap adı.
//
// NEDEN: her hesap kendi Zen profilinde (`-profile <dizin>`) çalışır. Ayrı
// profiller aynı `cookieStoreId` değerini ("firefox-default") bildirir; Kervan'da
// hesap anahtarı `provider:profile` olduğundan yedi Gemini hesabı TEK kayda
// çöküyor, hesap seçimi de tek sekmeye düşüyordu (2026-10-24 ölçümü).
//
// ÇÖZÜM: profil dizinindeki eklenti kopyasına bırakılan `kervan-profil.json`
// ({"ad": "ercaner1988@gmail.com"}) o profilin kimliğidir. Dosya yoksa davranış
// eskisi gibi: `cookieStoreId`. Dosya kimliği önbelleklenir; yedek değer
// önbelleklenmez (aynı profildeki farklı konteynerler farklı yedek taşır).
(function () {
  const api = typeof browser !== "undefined" ? browser : chrome;
  let kimlik = null;
  let bekleyen = null;

  async function oku() {
    try {
      const url = api.runtime.getURL("kervan-profil.json");
      const yanit = await fetch(url, { cache: "no-store" });
      if (!yanit || !yanit.ok) return null;
      const govde = await yanit.json();
      const ad = govde && typeof govde.ad === "string" ? govde.ad.trim() : "";
      return ad || null;
    } catch (e) {
      // Dosya yok (çoğu profil) ya da paketlenmemiş: bu bir hata değil, yedeğe
      // düşülür. Yalnız geliştirici görsün diye konsola tek satır yazılır.
      console.log("[Kervan] kervan-profil.json okunamadı, cookieStoreId kullanılacak:", e.message);
      return null;
    }
  }

  window.kervanProfilKimligi = function (yedek) {
    if (kimlik) return Promise.resolve(kimlik);
    if (!bekleyen) bekleyen = oku();
    return bekleyen.then((ad) => {
      if (ad) {
        kimlik = ad;
        return ad;
      }
      return (yedek && String(yedek).trim()) || "varsayilan";
    });
  };
})();
