/* Kervan durum panosu — veri.json'u /veri.json'dan okur, yedi bölümü doldurur. */
"use strict";

const DURUM_ETIKET = {
  calisiyor: "çalışıyor",
  kismi: "kısmi",
  iskelet: "iskelet",
  yok: "yok",
};

const BAG_ETIKET = {
  bagli: "bağlı",
  iskelet: "iskelet",
  yok: "bağ yok",
};

let VERI = null;
let SUZGEÇ = "hepsi";

const el = (id) => document.getElementById(id);

function sayi(n) {
  if (n === null || n === undefined || Number.isNaN(n)) return "—";
  return Number(n).toLocaleString("tr-TR");
}

function yuzde(n) {
  if (n === null || n === undefined || Number.isNaN(n)) return "—";
  return `%${Number(n).toFixed(1).replace(".", ",")}`;
}

function dk(iso) {
  if (!iso) return "—";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const p = (x) => String(x).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

function kacis(s) {
  return String(s ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/* ------------------------------------------------------------ 1. paydalar */

function paydalarCiz() {
  const kap = el("paydalar");
  kap.innerHTML = "";

  for (const p of VERI.degerlendirme.paydalar) {
    const dislanan = p.satirlar.filter((s) => s.disarida).map((s) => s.ad);

    const kutu = document.createElement("div");
    kutu.className = "payda";
    kutu.innerHTML = `
      <h3>${kacis(p.ad)}</h3>
      <p class="hesap">${kacis(p.aciklama)}</p>
      <div class="buyuk">${yuzde(p.yuzde)}</div>
      <div class="aralik">verilen aralık: ${yuzde(p.alt)} – ${yuzde(p.ust)}</div>
      ${
        dislanan.length
          ? `<p class="dislanan">bu paydada yok sayılan alan: ${dislanan.map(kacis).join(", ")}</p>`
          : ""
      }
      <p class="yorum">Ağırlık toplamı ${sayi(p.agirlik)} · ${p.satirlar.filter((s) => !s.disarida).length} alan.
      Bu sayı ölçüm değil; alan yüzdelerinin ağırlıklı ortalamasıdır.</p>
    `;
    kap.appendChild(kutu);
  }
}

/* ------------------------------------------------------------ 2. ölçüm */

function olcumCiz() {
  const o = VERI.olcum;
  const liste = el("olcum");
  const satirlar = [];

  satirlar.push(["Rust kaynağı", `${sayi(o.rust.dosya)} dosya · ${sayi(o.rust.satir)} satır`]);
  satirlar.push(["Crate", sayi(o.rust.crate)]);
  satirlar.push(["Test", `${sayi(o.test.birim)} birim · ${sayi(o.test.entegrasyon_dosyasi)} entegrasyon dosyası`]);
  satirlar.push(["Kapı betiği / testi", `${sayi(o.kapilar.betik)} / ${sayi(o.kapilar.test)}`]);
  satirlar.push(["Mühürlü dosya", sayi(o.muhur.dosya)]);
  satirlar.push(["Mutant kuralı", sayi(o.mutant.kural)]);

  if (o.durum.var) {
    satirlar.push([
      "Son kapı koşusu",
      `${sayi(o.durum.yesil)}/${sayi(o.durum.kapi_sayisi)} yeşil · ${dk(o.durum.uretim_zamani)}`,
    ]);
    if (o.durum.sayimlar) {
      const s = o.durum.sayimlar;
      satirlar.push([
        "Defter",
        `planlanan ${sayi(s.defter_planlandi)} · doğrulanan ${sayi(s.defter_dogrulandi)}`,
      ]);
    }
  } else {
    satirlar.push(["Son kapı koşusu", "port/durum.json yok"]);
  }

  if (o.uclar.var) {
    satirlar.push(["HTTP ucu", `${sayi(o.uclar.toplam)} kayıt · ${sayi(o.uclar.gercek)} parite ölçümlü`]);
  }
  if (o.sunucu.var) satirlar.push(["Sunucuda route", sayi(o.sunucu.route)]);
  if (o.saglayici.var) {
    satirlar.push(["Yerleşik sağlayıcı", `${sayi(o.saglayici.yerlesik.length)} (${o.saglayici.yerlesik.join(", ")})`]);
  }
  if (o.strateji.var) satirlar.push(["Yönlendirme stratejisi", sayi(o.strateji.sayi)]);

  satirlar.push(["Git commit", sayi(o.git.commit)]);
  satirlar.push(["Dal / kirli dosya", `${o.git.dal} · ${sayi(o.git.kirli)}`]);
  satirlar.push(["Son commit", (o.git.son || "—").split("|")[0]]);

  if (o.calisma.ikili.length) {
    satirlar.push([
      "Derlenmiş ikili",
      o.calisma.ikili.map((i) => `${i.yol.split("/").slice(-2).join("/")} ${(i.bayt / 1048576).toFixed(1)} MB (${i.tarih})`).join(" · "),
    ]);
  }

  if (o.canli_yapilandirma.var) {
    satirlar.push([
      "Canlı yapılandırma",
      `${sayi(o.canli_yapilandirma.satir)} satır · ${sayi(o.canli_yapilandirma.bolumler.length)} bölüm`,
    ]);
  }

  liste.innerHTML = satirlar.map(([a, b]) => `<dt>${kacis(a)}</dt><dd>${kacis(b)}</dd>`).join("");

  el("d-alan").textContent = `${VERI.degerlendirme.alanlar.length} alan`;
  const agirlik = VERI.degerlendirme.alanlar.reduce((t, a) => t + a.agirlik, 0);
  el("d-agirlik").textContent = sayi(agirlik);
}

/* ------------------------------------------------------------ 3. alanlar */

function alanCiz() {
  const kap = el("alanlar");
  kap.innerHTML = "";

  const gorunen = VERI.degerlendirme.alanlar.filter((a) => SUZGEÇ === "hepsi" || a.durum === SUZGEÇ);

  if (!gorunen.length) {
    kap.innerHTML = `<p class="aciklama">Bu durumda alan yok.</p>`;
    return;
  }

  for (const a of gorunen) {
    const kutu = document.createElement("article");
    kutu.className = "alan";
    kutu.dataset.durum = a.durum;

    const orta = ((a.alt + a.ust) / 2).toFixed(1);
    kutu.innerHTML = `
      <div class="alan-ust">
        <span class="alan-ad">${kacis(a.ad)}</span>
        <span class="alan-sag">
          <span class="alan-agirlik">ağırlık ${sayi(a.agirlik)}</span>
          <span class="etiket ${a.durum === "calisiyor" ? "yesil" : a.durum === "kismi" ? "hardal" : a.durum === "iskelet" ? "cini" : "mercan"}">${DURUM_ETIKET[a.durum] ?? a.durum}</span>
          <span class="yuzde">${yuzde(a.yuzde)}</span>
        </span>
      </div>
      <div class="çubuk">
        <div class="dolu" style="width:${Math.max(0, Math.min(100, a.yuzde))}%"></div>
        <div class="bant" style="left:${Math.max(0, Math.min(100, a.alt))}%; width:${Math.max(0, Math.min(100, a.ust) - Math.max(0, Math.min(100, a.alt)))}%"></div>
      </div>
      <p class="alan-gerekce">${kacis(a.gerekce)}</p>
      ${a.kanit && a.kanit.length ? `<p class="alan-kanit">${a.kanit.map(kacis).join(" · ")}</p>` : ""}
    `;
    kap.appendChild(kutu);
  }
}

/* ------------------------------------------------------------ 4. amaçlar */

function amaclarCiz() {
  const kap = el("amaclar");
  kap.innerHTML = "";
  for (const a of VERI.degerlendirme.amaclar) {
    const kutu = document.createElement("div");
    kutu.className = "amac";
    kutu.innerHTML = `
      <div class="kimlik">${kacis(a.id)}</div>
      <div>
        <div class="baslik">${kacis(a.ad)}</div>
        <p class="hedef">hedef: ${kacis(a.hedef)}</p>
        <p class="bugun-not">${kacis(a.not)}</p>
        <div class="çubuk"><div class="dolu" style="width:${Math.max(0, Math.min(100, a.bugun))}%; background:var(--cini)"></div></div>
      </div>
      <div class="puan">%${String(a.bugun).replace(".", ",")}</div>
    `;
    kap.appendChild(kutu);
  }
}

/* ------------------------------------------------------------ 5. ekosistem */

function ekosistemCiz() {
  const o = VERI.olcum;
  const kap = el("ekosistem");
  kap.innerHTML = "";

  const bagDurum = new Map();
  for (const b of VERI.ekosistem.baglar) bagDurum.set(b.hedef, b);
  for (const b of VERI.ekosistem.baglar) {
    if (b.kaynak !== "kervan") bagDurum.set(b.kaynak, bagDurum.get(b.kaynak) ?? b);
  }

  for (const d of VERI.ekosistem.dugumler) {
    const b = bagDurum.get(d.id);
    const durum = d.merkez ? "merkez" : (b?.durum ?? "yok");
    const kutu = document.createElement("div");
    kutu.className = `dugum ${d.merkez ? "merkez" : "d-" + durum}`;
    kutu.innerHTML = `
      <div class="ad">${kacis(d.ad)}</div>
      <div class="rol">${kacis(d.rol)}</div>
      <div class="ilgi">${d.merkez ? "bu pano" : BAG_ETIKET[durum] ?? durum}</div>
    `;
    kap.appendChild(kutu);
  }

  const tablo = el("baglar");
  const satirlar = VERI.ekosistem.baglar
    .map(
      (b) => `<tr>
        <td data-e="bağ">${kacis(b.kaynak)} → ${kacis(b.hedef)}</td>
        <td data-e="durum">${kacis(BAG_ETIKET[b.durum] ?? b.durum)}</td>
        <td class="kucuk" data-e="kanıt">${kacis(b.kanit)}</td>
        <td class="kucuk" data-e="not">${kacis(b.not)}</td>
      </tr>`
    )
    .join("");
  tablo.innerHTML = `<thead><tr><th>bağ</th><th>durum</th><th>kanıt</th><th>not</th></tr></thead><tbody>${satirlar}</tbody>`;

  el("baglar-kart").querySelector("h3").textContent = `Bağlar (${VERI.ekosistem.baglar.length})`;
}

/* ------------------------------------------------------------ 6. ölçmediklerim */

function olcmedikCiz() {
  el("olcmedik").innerHTML = VERI.olcmediklerim.map((s) => `<li>${kacis(s)}</li>`).join("");
}

/* ------------------------------------------------------------ 7. belge gerçekliği */

function belgeCiz() {
  const o = VERI.olcum;
  const kap = el("belge");

  const satirlar = [
    ["PR.md", `var · ${sayi(o.belge.pr_md_bayt)} bayt`, "İçindeki sayıların bir kısmı ölçümle çelişiyor: 20 kapı (gerçek " + o.kapilar.betik + "), 7 mutant (gerçek " + o.mutant.kural + "), sunucu ≈1240 satır (gerçek " + (o.sunucu.route ? "bakınız ölçüm" : "?") + "), 'main hiç commit almadı' (gerçek " + o.git.commit + ")."],
    ["port/durum.json", o.durum.var ? `var · ${dk(o.durum.uretim_zamani)}` : "yok", o.durum.var && o.durum.sayimlar ? `Defter: ${o.durum.sayimlar.defter_dogrulandi} doğrulanmış / ${o.durum.sayimlar.defter_planlandi} planlanmış → açık kalan var.` : ""],
    ["port/envanter.json", o.belge.envanter_var ? "var" : "yok", "Bu dosya Kervan'ın değil, referans alınan OmniRoute'un JS envanterini taşır; Kervan sayısı sanılmamalı."],
    ["kervan_gui.log", `${sayi(o.belge.kervan_gui_log_bayt)} bayt`, "Çalışma günlüğü depoda duruyor; kaynak dosya değil."],
  ];

  kap.innerHTML = `<table class="tablo"><thead><tr><th>belge</th><th>durum</th><th>not</th></tr></thead><tbody>${satirlar
    .map(
      ([a, b, c]) => `<tr><td data-e="belge">${kacis(a)}</td><td data-e="durum">${kacis(b)}</td><td class="kucuk" data-e="not">${kacis(c)}</td></tr>`
    )
    .join("")}</tbody></table>`;
}

/* ------------------------------------------------------------ tema + çipler */

function temaKur() {
  const kayitli = localStorage.getItem("kervan-pano-tema");
  if (kayitli) document.documentElement.dataset.tema = kayitli;
  el("tema").addEventListener("click", () => {
    const su = document.documentElement.dataset.tema === "koyu" ? "" : "koyu";
    if (su) document.documentElement.dataset.tema = su;
    else delete document.documentElement.dataset.tema;
    localStorage.setItem("kervan-pano-tema", su);
  });
}

function cipKur() {
  for (const c of el("filtre").querySelectorAll(".cip")) {
    c.addEventListener("click", () => {
      SUZGEÇ = c.dataset.durum;
      for (const x of el("filtre").querySelectorAll(".cip")) x.classList.toggle("etkin", x === c);
      alanCiz();
    });
  }
}

/* ------------------------------------------------------------ açılış */

async function basla() {
  temaKur();
  cipKur();

  let veri;
  try {
    const r = await fetch("/veri.json", { cache: "no-store" });
    if (!r.ok) throw new Error(`HTTP ${r.status}`);
    veri = await r.json();
  } catch (e) {
    el("uretim").textContent = "veri okunamadı";
    el("uyari-kusak").innerHTML =
      `<strong>veri.json okunamadı (${kacis(e.message)}).</strong> ` +
      `Önce <code>bun araclar/pano/olc-topla.ts</code> koşmalı.`;
    return;
  }

  VERI = veri;

  el("uretim").textContent = "üretim: " + dk(veri.uretim);
  el("uretim").classList.remove("soluk");
  if (veri.uyari) el("uyari-kusak").innerHTML = `<strong>Bu panodaki yüzdeler ölçüm değil, değerlendirmedir.</strong> ${kacis(veri.uyari)}`;
  el("kaynak").textContent = `${veri.kaynak.varsayim} (sürüm ${veri.kaynak.varsayim_surum})`;

  paydalarCiz();
  olcumCiz();
  alanCiz();
  amaclarCiz();
  ekosistemCiz();
  olcmedikCiz();
  belgeCiz();
}

basla();
