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
