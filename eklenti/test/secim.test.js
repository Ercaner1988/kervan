// Seçim turunun sahada ölçülmüş arızalarını sabitleyen testler.
//
// Her test, 2026-10-24'te GERÇEK tarayıcıda görülmüş bir belirtiyi sahte
// sayfada yeniden üretir. Amaç: aynı hesaba tekrar tekrar istek atarak
// "belki bu sefer olur" ölçümü yerine, arızanın kuralını kalıcı kılmak.
import { test, expect } from "bun:test";
import { sayfaKur, satirla } from "./harness.js";

test("işaret kanıtı kuralın adını verir", () => {
  const { window } = sayfaKur(
    `<div role="menuitem" class="mode selected"><span class="label">A</span></div>
     <div role="menuitem" aria-checked="true"><span class="label">B</span></div>
     <div role="menuitem"><span class="label">C</span><span aria-checked="true">x</span></div>
     <div role="menuitem"><span class="label">E</span><span aria-label="Seçildi">✓</span></div>
     <div role="menuitem"><span class="label">D</span></div>`
  );
  const l = window.kervanOgeOkuHam("[role=menuitem]");
  expect(l[0].kanit).toBe("sinif:selected");
  expect(l[1].kanit).toBe("aria-checked=true");
  expect(l[2].kanit).toBe("torun:aria-checked=true");
  expect(l[3].kanit).toBe("torun:secildi");
  expect(l[4].kanit).toBe("");
  expect(l[4].isaretli).toBe(false);
});

test("iki öğe birden işaretliyse kanıt hangi kuralın eşleştiğini gösterir", () => {
  // Canlı ölçümün birebir kopyası: "3.6 Flash* | 3.6 Düşünen* | 3.1 Pro"
  // Birinci öğe sınıftan, ikincisi alt düğümden işaret aldı.
  const { window } = sayfaKur(
    `<div role="menuitem" class="selected"><span class="label">3.6 Flash</span></div>
     <div role="menuitem"><span class="label">3.6 Düşünen</span><span aria-checked="true"></span></div>
     <div role="menuitem"><span class="label">3.1 Pro</span></div>`
  );
  const l = window.kervanOgeOkuHam("[role=menuitem]");
  const isaretliler = l.filter((o) => o.isaretli);
  expect(isaretliler.length).toBe(2);
  expect(isaretliler.map((o) => o.kanit)).toEqual(["sinif:selected", "torun:aria-checked=true"]);
});

test("menü yalnız düz click ile açılıyorsa isabetli strateji bulunur", async () => {
  // Sahada görülen durum: işaretçi olayları (pointerdown/mousedown) menüyü
  // açmıyor, hatta açılmasını engelliyor. Kod tek tıklamaya güvenseydi
  // "menü açılmadı" derdi.
  const { window, document } = sayfaKur(
    `<button id="m">Model</button><div id="kutu"></div>`
  );
  const b = document.querySelector("#m");
  const kutu = document.querySelector("#kutu");
  let isaretciGeldi = false;
  b.addEventListener("mousedown", () => {
    isaretciGeldi = true;
  });
  b.addEventListener("click", () => {
    if (isaretciGeldi) {
      isaretciGeldi = false; // işaretçi dizisi bu tıklamayı yuttu
      return;
    }
    kutu.innerHTML = `<div role="menuitem"><span class="label">3.6 Flash</span></div>`;
  });

  const liste = await window.kervanSecimTuru({
    komut: "oge_oku",
    secici: "[role=menuitem]",
    dugme: "#m"
  });
  expect(liste.length).toBe(1);
  expect(liste[0].metin).toBe("3.6 Flash");
}, 20000);

test("menü hiç açılmazsa hata, denenen stratejileri adlarıyla taşır", async () => {
  const { window } = sayfaKur(
    `<button id="m">Model</button><div id="kutu"></div>`
  );
  const hata = await window
    .kervanSecimTuru({ komut: "oge_oku", secici: "[role=menuitem]", dugme: "#m" })
    .then(() => null)
    .catch((e) => e);
  expect(hata).not.toBeNull();
  expect(hata.message).toContain("model menüsü açılmadı");
  expect(hata.message).toContain("isaretci:");
  expect(hata.message).toContain("click:");
  expect(hata.message).toContain("odak+enter:");
}, 25000);

test("ilk okuma boş gelirse menü yeniden açılır, 'model yok' denmez", async () => {
  // Canlı belirti: menü açılıp hemen kapandığında boş liste Rust'a "menüde
  // model yok" diye gidiyordu.
  const { window, document } = sayfaKur(
    `<button id="m">Model</button><div id="kutu"></div>`
  );
  const b = document.querySelector("#m");
  const kutu = document.querySelector("#kutu");
  let acilis = 0;
  b.addEventListener("click", () => {
    acilis++;
    if (acilis === 1) {
      // ilk açılış: anında kapanıyor
      kutu.innerHTML = "";
      return;
    }
    kutu.innerHTML = `<div role="menuitem"><span class="label">3.1 Pro</span></div>`;
  });

  const liste = await window.kervanSecimTuru({
    komut: "oge_oku",
    secici: "[role=menuitem]",
    dugme: "#m"
  });
  expect(liste.length).toBe(1);
  expect(liste[0].metin).toBe("3.1 Pro");
});

test("oge_bas: menü yeniden açılır, indeksteki öğe tıklanır, GÜNCEL liste döner", async () => {
  // Bu dal Rust testlerinde elle üretilmiş listelerle değil, gerçek DOM
  // üzerinde yalnız burada sınanıyor: menüyü aç → indekse tıkla → seçim menüyü
  // kapatır → yeniden aç → oku. Zincirin her halkası ayrı arızalanabilir.
  const { window, document } = sayfaKur(
    `<button id="m" aria-haspopup="true">Model</button><div id="kutu"></div>`
  );
  const b = document.querySelector("#m");
  const kutu = document.querySelector("#kutu");
  const ogeler = () => kutu.querySelectorAll("[role=menuitem]");
  let secili = 0;
  b.addEventListener("click", () => {
    // Gerçek menü gibi AÇ/KAPA: seçim menüyü kapatır, düğme yeniden açar.
    if (kutu.children.length) {
      kutu.innerHTML = "";
      return;
    }
    kutu.innerHTML = ["3.6 Flash", "3.6 Düşünen", "3.1 Pro"]
      .map(
        (ad, i) =>
          `<div role="menuitem"${i === secili ? ' class="selected"' : ""}><span class="label">${ad}</span></div>`
      )
      .join("");
    for (const [i, el] of [...ogeler()].entries()) {
      el.addEventListener("click", () => {
        secili = i; // sayfa seçimi uygular ve menüyü kapatır
        kutu.innerHTML = "";
      });
    }
  });

  // Menü AÇIK başlasın: ilk oku kapanışı görmeden tıklama yoluna girsin.
  b.click();
  expect(ogeler().length).toBe(3);

  const liste = await window.kervanSecimTuru({
    komut: "oge_bas",
    secici: "[role=menuitem]",
    dugme: "#m",
    indeks: 1
  });
  expect(liste.length).toBe(3);
  expect(liste[1].metin).toBe("3.6 Düşünen");
  expect(liste[1].isaretli).toBe(true);
  expect(liste[0].isaretli).toBe(false);
}, 25000);

test("aç/kapa yolunda düğme yoksa menü açılmaya çalışılmaz", async () => {
  // Aç/kapa anahtarları her zaman sayfada görünür; `dugme` tanımsızdır.
  // kervanMenuAc bu durumda hiçbir şey yapmamalı (yoksa 'model menüsü düğmesi
  // bulunamadı' hatası bütün aç/kapa seçimlerini düşürürdü).
  const { window } = sayfaKur(
    `<div role="switch" aria-checked="true" id="a">Araçlar</div>
     <div role="switch" aria-checked="false" id="b">Derin araştırma</div>`
  );
  const liste = await window.kervanSecimTuru({
    komut: "oge_oku",
    secici: "[role=switch]",
    dugme: undefined
  });
  expect(liste.map((o) => o.metin)).toEqual(["Araçlar", "Derin araştırma"]);
  expect(liste.map((o) => o.isaretli)).toEqual([true, false]);
}, 20000);

test("satır mantığı: .label yoksa ikon adı atlanır, model adı okunur", () => {
  const { window, document } = sayfaKur(
    `<div role="menuitem"><mat-icon>spark</mat-icon><div>3.6 Flash</div><div>Hızlı</div></div>`
  );
  const el = document.querySelector("[role=menuitem]");
  satirla(el, "spark\n3.6 Flash\nHızlı");
  const l = window.kervanOgeOkuHam("[role=menuitem]");
  expect(l[0].metin).toBe("3.6 Flash");
});
