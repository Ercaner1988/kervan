import fs from "fs";
import path from "path";
import { cargoTestListesiniAl } from "./kabul-haritasi.ts";
import { islevBul, rustDosyalariniBul, rustYapisiniMaskele, testAraliklariniBul } from "./sabit-yanit.ts";

// PRD FR-2.3: Uç Kaydı ve Test Kuralı (uc-kaydi.ts)
// 1. Kayıtlı her HTTP rotası port/uclar.toml içinde listelenmelidir (ve tersi).
// 2. durum yalnız `gercek` | `501` | `kapsam-disi`; `kapsam-disi` boş olmayan `gerekce` ister.
// 3. `gercek`/`501` ucun testi derlenmiş cargo test listesinde bulunmalıdır.
// 4. `gercek` ucun testi cargo'nun derlediği bir .rs dosyasında (crate src/ ya da tests/), #[test]
//    öznitelikli olmalı ve KENDİ (yorumu/dizgesi maskelenmiş) gövdesinde rastgele bir değeri (nonce)
//    yanıttan gelen bir değerle aynı assert içinde doğrulamalıdır (totolojik assert sayılmaz).
// Fail-closed: rota bulunamazsa, kayıt yoksa/ayrıştırılamazsa, test listesi alınamazsa kırmızı.

const DURUMLAR = ["gercek", "501", "kapsam-disi"] as const;

export interface UcTanimi {
  yol: string;
  yontem: string;
  durum: string;
  test?: string;
  testDosyasi?: string;
  gerekce?: string;
}

const YONTEM = "get|post|put|delete|patch|head|options|trace";
/**
 * Rota kaydeden Router çağrıları; `.route` dışındakiler bu kapıca çözülemez -> kırmızı. UFCS biçimi
 * (`Router::route(r, ..)`, `Router::<S>::route(..)`) de yakalanır: argüman sayısı tutmadığı için hata olur.
 * UFCS yalnız `Router` önekiyle aranır; başka türün `::merge(` gibi çağrıları rota değildir.
 */
const ROUTER_CAGRISI = /(?:\.|\bRouter\s*(?:::\s*<[^>]*>\s*)?::)\s*(route|route_service|nest|nest_service|merge|fallback|fallback_service)\s*\(/g;

/**
 * Üretim Rust kaynağındaki (src/, test öğeleri hariç) HTTP rotaları. Tarama yorum ve dizge içeriği
 * maskelenmiş metinde yapılır; yol dizgesi asıl metinden okunur. `.route("/y", get(a).post(b))`
 * zincirindeki HER yöntem (`routing::`/`axum::routing::` önekiyle de) ayrı rota sayılır. Çözülemeyen
 * biçim (route_service, nest, merge, fallback, `on(..)`, dizge olmayan yol, bilinmeyen zincir) hatadır.
 */
export function rotaTaramasi(kokDizin = process.cwd()): { rotalar: { yol: string; yontem: string; yer: string }[]; hatalar: string[] } {
  const rotalar: { yol: string; yontem: string; yer: string }[] = [];
  const hatalar: string[] = [];
  for (const dosya of rustDosyalariniBul(kokDizin)) {
    const asil = fs.readFileSync(dosya, "utf-8");
    const yapi = rustYapisiniMaskele(asil);
    const testAraliklari = testAraliklariniBul(yapi);
    const gore = path.relative(kokDizin, dosya).replace(/\\/g, "/");
    for (const m of yapi.matchAll(ROUTER_CAGRISI)) {
      if (testAraliklari.some(([a, b]) => m.index! >= a && m.index! < b)) continue;
      const satir = yapi.slice(0, m.index!).split("\n").length;
      const yer = `${gore}:${satir}`;
      const bas = m.index! + m[0].length;
      const son = dengeliBul(yapi, bas, (ch) => ch === ")");
      const argumanlar = son === -1 ? [] : ustDuzeyBol(yapi.slice(bas, son), ",");
      if (m[1] !== "route" || argumanlar.length !== 2) {
        hatalar.push(`${yer}: tanınmayan rota kaydı '.${m[1]}(..)'; uç kaydı bu biçimi çözemez (yalnız .route("<yol>", <yöntem>(..)[.<yöntem>(..)]))`);
        continue;
      }
      const yolMetni = asil.slice(bas, bas + argumanlar[0].length).trim();
      const yol = yolMetni.match(/^"([^"\\]*)"$/)?.[1];
      const yontemler = yontemZinciri(argumanlar[1]);
      if (yol === undefined || !yontemler) {
        hatalar.push(`${yer}: tanınmayan rota biçimi '.route(${asil.slice(bas, son).replace(/\s+/g, " ").trim()})'`);
        continue;
      }
      for (const y of yontemler) rotalar.push({ yol, yontem: y, yer });
    }
  }
  return { rotalar, hatalar };
}

/** `get(a).post(b)`, `routing::post(h)` -> ["GET", "POST"]; başka her biçim null. */
function yontemZinciri(ifade: string): string[] | null {
  let t = ifade.replace(/\s+/g, "");
  const ilk = t.match(new RegExp(`^(?:(?:::)?axum::)?(?:routing::)?(${YONTEM})\\(`));
  if (!ilk) return null;
  const yontemler: string[] = [];
  let ad = ilk[1];
  let k = ilk[0].length;
  for (;;) {
    const son = dengeliBul(t, k, (ch) => ch === ")");
    if (son === -1) return null;
    yontemler.push(ad.toUpperCase());
    t = t.slice(son + 1);
    if (t === "") return yontemler;
    const sonraki = t.match(new RegExp(`^\\.(${YONTEM})\\(`));
    if (!sonraki) return null;
    ad = sonraki[1];
    k = sonraki[0].length;
  }
}

export function kayitliRotalariBul(kokDizin = process.cwd()): { yol: string; yontem: string }[] {
  return rotaTaramasi(kokDizin).rotalar;
}

/** uclar.toml'u Bun.TOML ile okur. Dosya yoksa ya da ayrıştırılamazsa hata atar. */
export function uclarTomlOku(dosyaYolu = path.join("port", "uclar.toml")): UcTanimi[] {
  const veri: any = Bun.TOML.parse(fs.readFileSync(dosyaYolu, "utf-8"));
  const uclar = veri?.uc;
  if (!Array.isArray(uclar)) throw new Error(`${dosyaYolu} içinde [[uc]] listesi yok`);
  return uclar.map((u: any) => ({
    yol: String(u.yol ?? ""),
    yontem: String(u.yontem ?? "").toUpperCase(),
    durum: String(u.durum ?? ""),
    test: typeof u.test === "string" ? u.test : undefined,
    testDosyasi: typeof u.testDosyasi === "string" ? u.testDosyasi : undefined,
    gerekce: typeof u.gerekce === "string" ? u.gerekce : undefined,
  }));
}

const RASTGELE = /\brand::|\brandom\b|\bUuid::new_v4\b|\bgetrandom\b|\bthread_rng\b/;

/** `bas`tan itibaren ()[]{} derinliği 0'da `dur` karakterine kadar; bulunmazsa -1. */
function dengeliBul(metin: string, bas: number, dur: (ch: string, k: number) => boolean): number {
  let d = 0;
  for (let k = bas; k < metin.length; k++) {
    const ch = metin[k];
    if (d === 0 && dur(ch, k)) return k;
    if (ch === "(" || ch === "[" || ch === "{") d++;
    else if (ch === ")" || ch === "]" || ch === "}") {
      if (--d < 0) return -1;
    }
  }
  return -1;
}

const anar = (ad: string) => new RegExp(`\\b${ad}\\b`);
const geciyorMu = (metin: string, adlar: Set<string>) => [...adlar].some((a) => anar(a).test(metin));

/**
 * Test işlevinin MASKELENMİŞ gövdesinde (yorum ve dizge içeriği yok) nonce doğrulaması var mı:
 *  - N: rastgele kaynaktan bağlanan değişken ve ondan await'siz türetilenler (clone, format!..),
 *  - Y: `.await` içeren bir ifadeden (istek/yanıt) bağlanan değişken ve ondan türetilenler,
 *  - N, isteği kuran (await/send/call içeren) bir deyime akmalı,
 *  - bir assert N ile Y'yi doğrudan karşılaştırmalı (assert_eq!(a, b), a == b, y.contains(n));
 *    biçim argümanı, `||` kısa devresi ve iki yanı aynı totolojik assert sayılmaz.
 * ponytail: sözdizimsel akış izi; `let y = async { n }.await` gibi kasıtlı dolanmayı ayırt
 * edemez. Üst sınır: nonce testine bağlı bir mutant (port/mutantlar.toml) kesin kanıt olur.
 */
export function govdedeNonceDogrulamasiVarMi(govde: string): boolean {
  const baglar: { ad: string; sag: string }[] = [];
  for (const m of govde.matchAll(/\blet\s+(?:mut\s+)?([A-Za-z_]\w*)\b/g)) {
    if (/^[A-Z]/.test(m[1])) continue; // `if let Some(x) = ..` gibi kalıplar bağ değildir
    const bas = m.index! + m[0].length;
    const esit = dengeliBul(govde, bas, (ch, k) => ch === "=" && govde[k + 1] !== "=" && govde[k + 1] !== ">" && !"=!<>".includes(govde[k - 1]));
    const noktaliVirgul = dengeliBul(govde, bas, (ch) => ch === ";");
    if (esit === -1 || noktaliVirgul === -1 || esit > noktaliVirgul) continue;
    baglar.push({ ad: m[1], sag: govde.slice(esit + 1, noktaliVirgul) });
  }
  const N = new Set<string>();
  const Y = new Set<string>();
  for (let degisti = true; degisti; ) {
    degisti = false;
    for (const b of baglar) {
      const yanit = /\.\s*await\b/.test(b.sag) || geciyorMu(b.sag, Y);
      if (yanit && !Y.has(b.ad)) {
        Y.add(b.ad);
        N.delete(b.ad);
        degisti = true;
      } else if (!yanit && !Y.has(b.ad) && !N.has(b.ad) && (RASTGELE.test(b.sag) || geciyorMu(b.sag, N))) {
        N.add(b.ad);
        degisti = true;
      }
    }
  }
  if (N.size === 0 || Y.size === 0) return false;

  // Nonce isteğe gitmeli: N'yi kullanan, assert olmayan ve `.await`/`.send(`/`.call(` içeren bir deyim.
  const gonderildi = govde.split(";").some((d) => !/\bassert/.test(d) && /\.\s*await\b|\.send\s*\(|\.call\s*\(/.test(d) && geciyorMu(d, N));
  if (!gonderildi) return false;

  // Assert N ile Y'yi DEĞER olarak karşılaştırmalı: assert_eq!(a, b) ya da assert!(y.contains(n) / a == b).
  // Her yan yalnız bir N/Y adı ve değeri koruyan zincir (&, clone, to_string, as_str, unwrap, [..])
  // olabilir; boolean/uzunluk ifadesi (n > 0, y.len(), !y.is_empty()) yan sayılmaz. Koşul '&&'
  // parçalarına ayrılır ve parçalardan biri karşılaştırma olmalı; '||' kısa devresi kabul edilmez.
  const karsilastirir = (a: string, b: string) =>
    (degerIfadesi(a, N) && degerIfadesi(b, Y)) || (degerIfadesi(a, Y) && degerIfadesi(b, N));
  for (const m of govde.matchAll(/\bassert(_eq)?!\s*\(/g)) {
    const bas = m.index! + m[0].length;
    const son = dengeliBul(govde, bas, (ch) => ch === ")");
    if (son === -1) continue;
    const argumanlar = ustDuzeyBol(govde.slice(bas, son), ",");
    if (m[1]) {
      if (argumanlar.length >= 2 && karsilastirir(argumanlar[0], argumanlar[1])) return true;
      continue;
    }
    if (argumanlar[0].includes("||")) continue;
    for (const parca of ustDuzeyBol(argumanlar[0], "&&").map((p) => p.trim())) {
      const esitlik = ustDuzeyBol(parca, "==");
      if (esitlik.length === 2 && karsilastirir(esitlik[0], esitlik[1])) return true;
      const c = [...parca.matchAll(/\.\s*contains\s*\(/g)].pop();
      if (!c) continue;
      const ic = c.index! + c[0].length;
      // contains(..) parçanın son öğesi olmalı: `y.contains(&n)` evet, `y.contains(&n).then(..)` hayır.
      if (dengeliBul(parca, ic, (ch) => ch === ")") !== parca.length - 1) continue;
      if (karsilastirir(parca.slice(0, c.index!), parca.slice(ic, parca.length - 1))) return true;
    }
  }
  return false;
}

const DEGER_ZINCIRI = /^(?:\.(?:clone|to_string|to_owned|as_str|as_ref|as_bytes|borrow|unwrap|trim|into|to_vec)\(\)|\.expect\("[^"]*"\)|\[[^\[\]]*\])*$/;

/** İfade yalnız kümedeki bir adın değerini mi taşıyor (`&n`, `n.to_string()`, `y["a"][0].as_str().unwrap()`)? */
function degerIfadesi(ifade: string, kume: Set<string>): boolean {
  let t = ifade.replace(/\s+/g, "");
  while (t.startsWith("(") && dengeliBul(t, 1, (ch) => ch === ")") === t.length - 1) t = t.slice(1, -1);
  t = t.replace(/^(?:&(?:mut)?|\*)+/, "");
  const ad = t.match(/^[A-Za-z_]\w*/)?.[0];
  return !!ad && kume.has(ad) && DEGER_ZINCIRI.test(t.slice(ad.length));
}

/** ()[]{} derinliği 0'daki `ayrac`lardan böler. */
function ustDuzeyBol(metin: string, ayrac: string): string[] {
  const parcalar: string[] = [];
  let d = 0;
  let bas = 0;
  for (let k = 0; k < metin.length; k++) {
    const ch = metin[k];
    if (ch === "(" || ch === "[" || ch === "{") d++;
    else if (ch === ")" || ch === "]" || ch === "}") d--;
    else if (d === 0 && metin.startsWith(ayrac, k)) {
      parcalar.push(metin.slice(bas, k));
      bas = k + ayrac.length;
      k = bas - 1;
    }
  }
  parcalar.push(metin.slice(bas));
  return parcalar;
}

/**
 * Dosya cargo'nun derlediği bir test kaynağı mı: `.rs`, kök içinde, en yakın `Cargo.toml`'u
 * `[package]` olan crate'in `src/` ya da `tests/` altında. Hata metni ya da null döner.
 * ponytail: `src/` altındaki dosyanın `mod` ağacına bağlı olduğu ayrıca izlenmez; kimliğin
 * derlenmiş `cargo test --list`'te bulunması bu açığı daraltır.
 */
export function cargoTestKaynagiHatasi(kokDizin: string, dosya: string): string | null {
  const kok = path.resolve(kokDizin);
  const tam = path.resolve(kok, dosya);
  const gore = path.relative(kok, tam);
  if (!tam.endsWith(".rs")) return `'${dosya}' Rust kaynağı değil (.rs değil)`;
  if (gore.startsWith("..") || path.isAbsolute(gore)) return `'${dosya}' depo kökünün dışında`;
  if (!fs.existsSync(tam)) return `'${dosya}' yok`;
  for (let d = path.dirname(tam); d.length >= kok.length; d = path.dirname(d)) {
    const manifest = path.join(d, "Cargo.toml");
    if (fs.existsSync(manifest)) {
      if (!/^\s*\[package\]/m.test(fs.readFileSync(manifest, "utf-8"))) return `'${dosya}' için en yakın Cargo.toml bir paket değil (${manifest})`;
      const ic = path.relative(d, tam).split(path.sep)[0];
      return ic === "src" || ic === "tests" ? null : `'${dosya}' crate'in src/ ya da tests/ dizininde değil; cargo derlemez`;
    }
    if (d === kok) break;
  }
  return `'${dosya}' hiçbir Cargo paketine ait değil`;
}

/** Testin nonce doğrulamasını denetler; sorun yoksa null, varsa nedenini döndürür. */
export function testNonceHatasi(testKimligi: string, testDosyaIpuclari?: string[], kokDizin = process.cwd()): string | null {
  const ad = testKimligi.split("::").pop()!;
  const dosyalar = testDosyaIpuclari ?? [
    path.join("crates", "sunucu", "src", "server.rs"),
    path.join("crates", "yukari", "src", "lib.rs"),
  ];
  const nedenler: string[] = [];
  for (const d of dosyalar) {
    const kaynakHatasi = cargoTestKaynagiHatasi(kokDizin, d);
    if (kaynakHatasi) {
      nedenler.push(kaynakHatasi);
      continue;
    }
    const islev = islevBul(fs.readFileSync(path.resolve(kokDizin, d), "utf-8"), ad);
    if (!islev) nedenler.push(`'${d}' içinde 'fn ${ad}' yok`);
    else if (!islev.testOzniteligi) nedenler.push(`'${d}' içindeki 'fn ${ad}' #[test] öznitelikli değil`);
    else if (!govdedeNonceDogrulamasiVarMi(islev.govde)) nedenler.push(`'${d}' içindeki 'fn ${ad}' gövdesinde rastgele nonce'u yanıtla birlikte doğrulayan assert yok`);
    else return null;
  }
  return nedenler.join("; ") || "test dosyası yok";
}

/** Geriye uyumluluk: nonce doğrulaması geçerli mi. */
export function testDosyasindaNonceVarMi(testKimligi: string, testDosyaIpuclari?: string[], kokDizin = process.cwd()): boolean {
  return testNonceHatasi(testKimligi, testDosyaIpuclari, kokDizin) === null;
}

export function ucKaydiniDenetle(
  kokDizin = process.cwd(),
  uclarTomlYolu = path.join(kokDizin, "port", "uclar.toml"),
  onbellekliTestler?: Set<string>
): {
  gecerli: boolean;
  kayitliRotaSayisi: number;
  tomlUclariSayisi: number;
  hatalar: string[];
} {
  const hatalar: string[] = [];
  const tarama = rotaTaramasi(kokDizin);
  const kodRotalari = tarama.rotalar;
  hatalar.push(...tarama.hatalar);
  if (kodRotalari.length === 0) hatalar.push(`Kodda kayıtlı HTTP rotası bulunamadı (üretim src/*.rs, kök: ${kokDizin})`);

  let tomlUclari: UcTanimi[] = [];
  try {
    tomlUclari = uclarTomlOku(uclarTomlYolu);
  } catch (e: any) {
    hatalar.push(`Uç kaydı okunamadı (${uclarTomlYolu}): ${e.message}`);
  }
  if (tomlUclari.length === 0) hatalar.push("port/uclar.toml boş: hiç uç kaydı yok");

  for (const r of kodRotalari) {
    if (!tomlUclari.some((u) => u.yol === r.yol && u.yontem === r.yontem)) {
      hatalar.push(`Kayıtlı rota port/uclar.toml dosyasında tanımlı değil: ${r.yontem} ${r.yol}`);
    }
  }
  for (const u of tomlUclari) {
    if (!kodRotalari.some((r) => u.yol === r.yol && u.yontem === r.yontem)) {
      hatalar.push(`port/uclar.toml'daki uç kodda kayıtlı değil: ${u.yontem} ${u.yol}`);
    }
  }

  let cargoTestleri = onbellekliTestler;
  const testGereken = tomlUclari.some((u) => u.durum === "gercek" || u.durum === "501");
  if (!cargoTestleri && testGereken) {
    const liste = cargoTestListesiniAl(kokDizin);
    if (liste.hata) hatalar.push(liste.hata);
    cargoTestleri = liste.kimlikler;
  }

  for (const uc of tomlUclari) {
    const ad = `${uc.yontem} ${uc.yol}`;
    if (!(DURUMLAR as readonly string[]).includes(uc.durum)) {
      hatalar.push(`Uç ${ad}: bilinmeyen durum '${uc.durum}' (geçerli: ${DURUMLAR.join(" | ")})`);
      continue;
    }
    if (uc.durum === "kapsam-disi") {
      if (!uc.gerekce?.trim()) hatalar.push(`Uç ${ad} 'kapsam-disi' ama 'gerekce' yazılmamış`);
      continue;
    }
    if (!uc.test) {
      hatalar.push(`Uç ${ad} (${uc.durum}) için 'test' kimliği zorunludur ancak belirtilmemiş`);
      continue;
    }
    if (!cargoTestleri?.has(uc.test)) {
      hatalar.push(`Uç ${ad} için belirtilen test '${uc.test}' cargo test --list çıktısında bulunamadı`);
      continue;
    }
    const nonceHatasi = uc.durum === "gercek" ? testNonceHatasi(uc.test, uc.testDosyasi ? [uc.testDosyasi] : undefined, kokDizin) : null;
    if (nonceHatasi) hatalar.push(`Uç ${ad} ('gercek') testi '${uc.test}': ${nonceHatasi}`);
  }

  return {
    gecerli: hatalar.length === 0,
    kayitliRotaSayisi: kodRotalari.length,
    tomlUclariSayisi: tomlUclari.length,
    hatalar,
  };
}

if (import.meta.main) {
  const sonuc = ucKaydiniDenetle();
  console.log(`[uc-kaydi] Kayıtlı HTTP rotası: ${sonuc.kayitliRotaSayisi}`);
  console.log(`[uc-kaydi] port/uclar.toml tanımı: ${sonuc.tomlUclariSayisi}`);
  if (sonuc.gecerli) {
    console.log("✅ [uc-kaydi] Tüm uçlar kayıtlı ve test kurallarına uygun.");
    console.log("HUKUM: yesil");
    process.exit(0);
  }
  console.error("❌ [uc-kaydi] UÇ KAYDI VE TEST KURALI İHLALLERİ BULUNDU:");
  for (const h of sonuc.hatalar) console.error(`  - ${h}`);
  console.log(`HUKUM: kirmizi ${sonuc.hatalar[0]}`);
  process.exit(1);
}
