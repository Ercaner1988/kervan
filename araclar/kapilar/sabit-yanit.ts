import fs from "fs";
import path from "path";

// PRD FR-2.2: Sabit Yanıt Taraması (sabit-yanit.ts)
// Üretim kaynağında (test dışı src/ dosyalarında) sahte / sabit yanıt kalıplarını arar.
// Yalnız `#[cfg(test)]` / `#[test]` ile işaretli ÖĞE (ayraçlı blok ya da `;` ile biten
// ayraçsız öğe) ve başına `#![cfg(test)]` konmuş dosya/blok yoksayılır. Yorumlar ATLANMAZ: üretim
// dosyasındaki yorumda geçen sabit kalıp da bulgudur (fail-closed).
// Yapı (ayraç derinliği, öğe sonu) dizge ve yorum içeriği maskelenmiş metinden çıkarılır.

export const VARSAYILAN_KALIPLAR = [
  "chatcmpl-123",
  "1700000000",
  "selamlar!",
  "Simüle edilmiş",
];

export interface KalipBulgusu {
  dosya: string;
  satirNo: number;
  kalip: string;
  satirMetni: string;
}

export function rustDosyalariniBul(dizin: string): string[] {
  const dosyalar: string[] = [];
  if (!fs.existsSync(dizin)) return dosyalar;

  const entries = fs.readdirSync(dizin, { withFileTypes: true });
  for (const entry of entries) {
    const tamYol = path.join(dizin, entry.name);
    if (entry.isDirectory()) {
      if (entry.name !== "target" && entry.name !== ".git" && entry.name !== "ref" && entry.name !== "node_modules") {
        dosyalar.push(...rustDosyalariniBul(tamYol));
      }
    } else if (entry.isFile() && entry.name.endsWith(".rs")) {
      // Yalnızca src/ altındaki dosyalar üretim kodudur
      if (tamYol.includes(`${path.sep}src${path.sep}`) || tamYol.startsWith(`src${path.sep}`)) {
        dosyalar.push(tamYol);
      }
    }
  }
  return dosyalar;
}

const KIMLIK = /[A-Za-z0-9_]/;

/**
 * Rust kaynağında yorumları ve dizge/karakter sabitlerinin içeriğini boşlukla değiştirir
 * (satır sonları korunur, uzunluk aynı kalır). Yapısal tarama bu metin üzerinde yapılır.
 */
export function rustYapisiniMaskele(kaynak: string): string {
  const c = kaynak.split("");
  const n = c.length;
  const bosalt = (a: number, b: number) => {
    for (let k = a; k < b && k < n; k++) if (c[k] !== "\n" && c[k] !== "\r") c[k] = " ";
  };
  let i = 0;
  while (i < n) {
    const ch = kaynak[i];
    const sonraki = kaynak[i + 1];
    if (ch === "/" && sonraki === "/") {
      const son = kaynak.indexOf("\n", i);
      const bitis = son === -1 ? n : son;
      bosalt(i, bitis);
      i = bitis;
    } else if (ch === "/" && sonraki === "*") {
      let derinlik = 1;
      let j = i + 2;
      while (j < n && derinlik > 0) {
        if (kaynak[j] === "/" && kaynak[j + 1] === "*") { derinlik++; j += 2; }
        else if (kaynak[j] === "*" && kaynak[j + 1] === "/") { derinlik--; j += 2; }
        else j++;
      }
      bosalt(i, j);
      i = j;
    } else if ((ch === "r" || (ch === "b" && sonraki === "r")) && !KIMLIK.test(kaynak[i - 1] ?? " ")) {
      // Ham dizge: r"..", r#".."#, br#".."#
      let j = ch === "b" ? i + 2 : i + 1;
      let diyez = 0;
      while (kaynak[j] === "#") { diyez++; j++; }
      if (kaynak[j] === '"') {
        const kapanis = '"' + "#".repeat(diyez);
        const son = kaynak.indexOf(kapanis, j + 1);
        const bitis = son === -1 ? n : son + kapanis.length;
        bosalt(j + 1, bitis - kapanis.length);
        i = bitis;
      } else {
        i++;
      }
    } else if (ch === '"') {
      let j = i + 1;
      while (j < n && kaynak[j] !== '"') j += kaynak[j] === "\\" ? 2 : 1;
      bosalt(i + 1, j);
      i = j + 1;
    } else if (ch === "'") {
      // Karakter sabiti mi, ömür ('a) mi?
      if (sonraki === "\\") {
        let j = i + 2;
        while (j < n && kaynak[j] !== "'" && kaynak[j] !== "\n") j++;
        bosalt(i + 1, j);
        i = j + 1;
      } else {
        const cp = kaynak.codePointAt(i + 1);
        const genislik = cp !== undefined && cp > 0xffff ? 2 : 1;
        if (kaynak[i + 1 + genislik] === "'") {
          bosalt(i + 1, i + 1 + genislik);
          i = i + 2 + genislik;
        } else {
          i++;
        }
      }
    } else {
      i++;
    }
  }
  return c.join("");
}

/** `acilis` konumundaki `{` ile eşleşen `}`'in konumu (maskelenmiş metinde); yoksa -1. */
function ayracSonu(yapi: string, acilis: number): number {
  let derinlik = 0;
  for (let k = acilis; k < yapi.length; k++) {
    if (yapi[k] === "{") derinlik++;
    else if (yapi[k] === "}" && --derinlik === 0) return k;
  }
  return -1;
}

/**
 * Öğe başlangıcından (öznitelik sonrasından) öğenin bittiği konuma kadar ilerler. `alanKipi`: öznitelik bir
 * öğeye değil parametreye, yapı alanına ya da enum varyantına konmuştur; kapsam ilk üst düzey `,`'de biter.
 * Her iki kipte de saran parantez/ayraç kapanınca (`)`, `]`, `}`) kapsam biter: öznitelik dışına taşmaz.
 */
function ogeSonu(yapi: string, bas: number, alanKipi = false): number {
  let derinlik = 0; // ( ve [
  for (let k = bas; k < yapi.length; k++) {
    const ch = yapi[k];
    if (ch === "(" || ch === "[") derinlik++;
    else if (ch === ")" || ch === "]") { if (--derinlik < 0) return k; }
    else if (ch === "}" && derinlik === 0) return k;
    else if (ch === "," && derinlik === 0 && alanKipi) return k + 1;
    else if (ch === ";" && derinlik === 0) return k + 1;
    else if (ch === "{" && derinlik === 0) {
      const son = ayracSonu(yapi, k);
      if (son === -1) return yapi.length;
      const kalan = yapi.slice(son + 1).match(/^\s*;/);
      return son + 1 + (kalan ? kalan[0].length : 0);
    }
  }
  return yapi.length;
}

/** `konum`u saran `{`'nin konumu (maskelenmiş metinde); dosya düzeyindeyse -1. */
function saranAyrac(yapi: string, konum: number): number {
  let derinlik = 0;
  for (let k = konum - 1; k >= 0; k--) {
    if (yapi[k] === "}") derinlik++;
    else if (yapi[k] === "{" && derinlik-- === 0) return k;
  }
  return -1;
}

/** Öznitelikten sonra (başka öznitelikler ve görünürlük atlanınca) bir öğe anahtar sözcüğü geliyor mu. */
const OGE_ONU = /^(?:\s*#\s*\[(?:[^\[\]]|\[[^\]]*\])*\])*\s*(?:pub\s*(?:\([^)]*\))?\s+)?(?:fn|mod|struct|enum|union|impl|trait|const|static|use|type|extern|async|unsafe|macro_rules\s*!)\b/;

/** Bir bloğun başında yalnız boşluk ve başka iç öznitelikler olabilir. */
const IC_OZNITELIK_ONCESI = /^(?:\s*#!\s*\[(?:[^\[\]]|\[[^\]]*\])*\])*\s*$/;

/**
 * `#[cfg(test)]` / `#[test]` öğelerinin ve `#![cfg(test)]` kapsamlarının [baş, son) aralıkları.
 * `#![cfg(test)]` yalnız bulunduğu bloğun (ya da dosyanın) BAŞINDAYSA geçerlidir ve yalnız o
 * bloğu kapsar; öğelerden sonra gelen iç öznitelik geçersiz Rust'tır, hiçbir şeyi gizlemez.
 */
export function testAraliklariniBul(yapi: string): [number, number][] {
  const araliklar: [number, number][] = [];
  const ic = /#!\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]/g;
  let im;
  while ((im = ic.exec(yapi)) !== null) {
    const ac = saranAyrac(yapi, im.index);
    if (!IC_OZNITELIK_ONCESI.test(yapi.slice(ac + 1, im.index))) continue;
    if (ac === -1) return [[0, yapi.length]];
    const son = ayracSonu(yapi, ac);
    araliklar.push([ac, son === -1 ? yapi.length : son + 1]);
  }
  const re = /#\s*\[\s*(?:cfg\s*\(\s*test\s*\)|test)\s*\]/g;
  let m;
  while ((m = re.exec(yapi)) !== null) {
    const sonra = yapi.slice(m.index + m[0].length, m.index + m[0].length + 400);
    const son = ogeSonu(yapi, m.index + m[0].length, !OGE_ONU.test(sonra));
    araliklar.push([m.index, son]);
  }
  return araliklar;
}

/** `fn`'den hemen önceki öznitelik bloğu (niteleyiciler hariç). */
const OZNITELIK_BLOGU = /((?:#\s*\[(?:[^\[\]]|\[[^\]]*\])*\]\s*)+)(?:pub(?:\s*\([^)]*\))?\s+)?(?:const\s+)?(?:async\s+)?(?:unsafe\s+)?$/;

/**
 * `fn <ad>` işlevini bulur. Gövde MASKELENMİŞ metinden döner: yorumlar ve dizge içerikleri
 * boşluktur, yani yoruma alınmış kod ya da dizgedeki metin hiçbir denetimde sayılmaz.
 * `testOzniteligi`: işlevin hemen önündeki özniteliklerde `#[test]` / `#[<yol>::test]` var mı.
 */
export function islevBul(kaynak: string, ad: string): { govde: string; testOzniteligi: boolean } | null {
  const yapi = rustYapisiniMaskele(kaynak);
  const re = new RegExp(`\\bfn\\s+${ad.replace(/[^\w]/g, "")}\\b`, "g");
  let m;
  while ((m = re.exec(yapi)) !== null) {
    let derinlik = 0;
    for (let k = m.index + m[0].length; k < yapi.length; k++) {
      const ch = yapi[k];
      if (ch === "(" || ch === "[" || ch === "<") derinlik++;
      else if (ch === ")" || ch === "]" || (ch === ">" && yapi[k - 1] !== "-")) derinlik--;
      else if (ch === ";" && derinlik <= 0) break; // gövdesiz bildirim
      else if (ch === "{" && derinlik <= 0) {
        const son = ayracSonu(yapi, k);
        if (son === -1) return null;
        const oznitelikler = yapi.slice(0, m.index).match(OZNITELIK_BLOGU)?.[1] ?? "";
        return { govde: yapi.slice(k, son + 1), testOzniteligi: /#\s*\[\s*(?:\w+\s*::\s*)*test\b/.test(oznitelikler) };
      }
    }
  }
  return null;
}

/** `fn <ad>` işlevinin maskelenmiş `{ ... }` gövdesi; yoksa null. */
export function islevGovdesiniBul(kaynak: string, ad: string): string | null {
  return islevBul(kaynak, ad)?.govde ?? null;
}

export function dosyayiTara(dosyaYolu: string, kaliplar = VARSAYILAN_KALIPLAR): KalipBulgusu[] {
  const icerik = fs.readFileSync(dosyaYolu, "utf-8");
  const araliklar = testAraliklariniBul(rustYapisiniMaskele(icerik));
  const testteMi = (k: number) => araliklar.some(([a, b]) => k >= a && k < b);
  const satirBaslari = [0];
  for (let k = 0; k < icerik.length; k++) if (icerik[k] === "\n") satirBaslari.push(k + 1);
  const satirNo = (k: number) => {
    let lo = 0, hi = satirBaslari.length - 1;
    while (lo < hi) {
      const orta = (lo + hi + 1) >> 1;
      if (satirBaslari[orta] <= k) lo = orta; else hi = orta - 1;
    }
    return lo + 1;
  };

  const bulgular: KalipBulgusu[] = [];
  for (const kalip of kaliplar) {
    for (let k = icerik.indexOf(kalip); k !== -1; k = icerik.indexOf(kalip, k + 1)) {
      if (testteMi(k)) continue;
      const no = satirNo(k);
      const bas = satirBaslari[no - 1];
      const son = icerik.indexOf("\n", bas);
      bulgular.push({ dosya: dosyaYolu, satirNo: no, kalip, satirMetni: icerik.slice(bas, son === -1 ? undefined : son).trim() });
    }
  }
  return bulgular.sort((a, b) => a.satirNo - b.satirNo);
}

export function sabitYanitlariTara(
  kokDizin = process.cwd(),
  kaliplar = VARSAYILAN_KALIPLAR
): {
  temizMi: boolean;
  bulgular: KalipBulgusu[];
  tarananDosyaSayisi: number;
  hatalar: string[];
} {
  const dosyalar = rustDosyalariniBul(kokDizin);
  const bulgular: KalipBulgusu[] = [];
  const hatalar: string[] = [];
  if (dosyalar.length === 0) hatalar.push(`Taranacak üretim Rust dosyası bulunamadı (kök: ${kokDizin})`);

  for (const dosya of dosyalar) {
    try {
      bulgular.push(...dosyayiTara(dosya, kaliplar));
    } catch (e: any) {
      hatalar.push(`${dosya} okunamadı: ${e.message}`);
    }
  }

  return { temizMi: bulgular.length === 0 && hatalar.length === 0, bulgular, tarananDosyaSayisi: dosyalar.length, hatalar };
}

if (import.meta.main) {
  const sonuc = sabitYanitlariTara();
  console.log(`[sabit-yanit] Taranan Rust kaynak dosyası: ${sonuc.tarananDosyaSayisi}`);

  if (sonuc.temizMi) {
    console.log("✅ [sabit-yanit] Üretim kodunda hiçbir sabit sahte kalıp bulunamadı.");
    console.log("HUKUM: yesil");
    process.exit(0);
  }
  console.error("❌ [sabit-yanit] ÜRETİM KODUNDA SAHTE/SABİT YANIT KALIPLARI BULUNDU ya da TARAMA YAPILAMADI:");
  for (const h of sonuc.hatalar) console.error(`  - ${h}`);
  for (const b of sonuc.bulgular) console.error(`  - ${b.dosya}:${b.satirNo} [${b.kalip}] -> "${b.satirMetni}"`);
  const ilk = sonuc.hatalar[0] ?? `${sonuc.bulgular.length} sabit kalıp bulgusu, ilki ${sonuc.bulgular[0].dosya}:${sonuc.bulgular[0].satirNo} [${sonuc.bulgular[0].kalip}]`;
  console.log(`HUKUM: kirmizi ${ilk}`);
  process.exit(1);
}
