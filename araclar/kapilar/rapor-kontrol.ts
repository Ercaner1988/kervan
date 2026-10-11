import fs from "fs";
import path from "path";
import { KOSU_DOSYASI, kapilarTomlYukle, type KapilarYapilandirmasi } from "./hepsi.ts";

// PRD FR-3.1: Rapor Sözleşmesi ve Sahte-Kapı Kontrolü (rapor-kontrol.ts)
// 1. Her 'geçti / yeşil / tamamlandı / çalışıyor' iddiası (başlık dahil her satırda) yeşil biten bir
//    hepsi.ts kapı günlüğüne bağlanır; kaynak dosya ya da rastgele dosya başarıyı kanıtlamaz.
//    Bağlanamayan yerde iddia yazılmaz, 'bakmadım' yazılır (aynı satırdaki 'bakmadım' iddiayı aklamaz).
// 2. Ek D sahte-kapı listesinin 6 maddesine 'baktım: <kanıt>' (hepsi.ts günlüğü ya da boş olmayan
//    dosya:satır) ya da 'bakmadım' cevabı verilir.
// Satırdaki bağlantı / .log / dosya:satır atıflarından biri bile çözülemiyorsa (uydurma kanıt) satır kırmızıdır.

export const EK_D_MADDELERI = [
  "Sabit yanıt",
  "Kendini doğrulayan kontrol",
  "Ulaşılamayan teslim",
  "Kanıtı aşan rapor",
  "Gevşetme",
  "Başarısız olamayan test",
];

// Kiril/Yunan benzer harfler (yеşil, gеçti): görünüşte aynı, kod noktası farklı. Latin karşılığına çevrilir.
const BENZER: Record<string, string> = Object.fromEntries(
  [..."аеорсухіјѕԁһӏАВЕКМНОРСТХІЈЅαεικνορτυχΑΒΕΖΗΙΚΜΝΟΡΤΥΧ"].map((c, i) => [c, "aeopcyxijsdhlABEKMHOPCTXIJSaeikvoptuxABEZHIKMNOPTYX"[i]])
);

// Türkçe katlama: NFKC (tam genişlik/bağlı harf), biçim ve görünmez karakterler (ZWSP, yumuşak tire,
// sözcük birleştirici, VS16) silinir, benzer harfler Latin'e çevrilir, İ/I doğru küçültülür, aramada ı -> i
// (CLAUDE.md: çıplak toLowerCase yok).
export function katla(s: string): string {
  return s
    .normalize("NFKC")
    .replace(/[\p{Cf}\p{Default_Ignorable_Code_Point}]/gu, "")
    .replace(/./gsu, (c) => BENZER[c] ?? c)
    .replace(/İ/g, "i")
    .replace(/I/g, "ı")
    .toLowerCase()
    .normalize("NFD")
    .replace(/\p{M}/gu, "") // ş->s, ç->c, ğ->g, ö->o, ü->u, i̇->i: ASCII yazım ('yesil', 'gecti') da yakalanır
    .replace(/ı/g, "i");
}

// Yalnız iddia aramak için: sözcük ortasına konan Markdown vurgusu (*, _, ~) ve HTML etiketi/varlık başvurusu
// iddiayı gizlemesin ('ge*ç*ti', 'ye<b>şi</b>l', 'tamam&#108;andı'). Kanıt çıkarımı özgün satırla yapılır.
const kodNoktasi = (n: number) => (Number.isInteger(n) && n >= 0 && n <= 0x10ffff ? String.fromCodePoint(n) : " ");
export function bicimsiz(s: string): string {
  return s
    .replace(/&#x([0-9a-f]+);/gi, (_, h) => kodNoktasi(parseInt(h, 16)))
    .replace(/&#(\d+);/g, (_, d) => kodNoktasi(Number(d)))
    .replace(/&(?:amp|lt|gt|quot|apos|nbsp);/g, " ")
    .replace(/<[^>]*>/g, "")
    .replace(/[*_~]/g, "");
}

// Başarı iddiası: katlanmış metinde kök + çekim ekleri ve eşdeğerleri (PRD FR-3.1 'geçti / yeşil /
// tamamlandı / çalışıyor' ve aynı anlamı taşıyan biçimler). Olumsuz/nötr biçimler (geçersiz, geçici,
// başarısız, çalışma, çalıştır-) bilerek dışarıda. ponytail: sözlük tavanı var; 'ortada pürüz kalmadı'
// gibi yeniden kurulmuş cümleyi betik yakalamaz, PRD gereği doğruluğu hakem denetler. Kaçak bulunursa kök buraya eklenir.
const BASARI_IDDIASI = new RegExp(
  [
    String.raw`\bgec(?:ti|iyor|er\b|mistir|mekte|ecek|ebil|tik|tiler)`,
    String.raw`\byesil`,
    String.raw`\btamam(?:\b|la(?:ndi|nmis|niyor|nacak|nir|di|dik))`,
    String.raw`\bcalis(?:iyor|ir|ti(?!r)|mis|makta|abil)`,
    String.raw`\bbasar(?:i(?!siz)|di|ili)`,
    String.raw`\b(?:sorunsuz|hatasiz|kusursuz|eksiksiz)`,
    String.raw`\bdogrulan(?:di|mis)`,
    String.raw`\b(?:sorun|hata)(?:u|lar)? (?:yok|kalmadi)\b`,
    String.raw`\b(?:pass(?:ed|es|ing)?|succe(?:ss|ed)\w*|green|works?|working|done|complete[ds]?|ok(?:ay)?|fixed|verified|lgtm)\b`,
    String.raw`[✅✔✓☑🟢💚👍]`,
    String.raw`\[x\]`,
  ].join("|"),
  "u"
);
const UZANTI = "log|rs|ts|toml|json|jsonl|md|txt";

interface Atif {
  ham: string;
  yol: string;
  satir?: number;
  // Bağlantı hedefi, .log ya da dosya:satır = kanıt iddiası: çözülmek ZORUNDA.
  // Satırsız çıplak dosya adı (örn. `hepsi.ts`) yalnız anıştır: çözülmezse yok sayılır, çözülse de kanıt sayılmaz.
  zorunlu: boolean;
}

// Satırdaki kanıt adaylarını çıkarır: markdown bağlantı hedefi, ters tırnak içi, çıplak yol[:satır].
export function atiflariCikar(satir: string): Atif[] {
  const adaylar = new Set<string>();
  const baglantilar = new Set<string>();
  for (const m of satir.matchAll(/\]\(([^)\s]+)\)/g)) {
    adaylar.add(m[1]);
    baglantilar.add(m[1]);
  }
  for (const m of satir.matchAll(/`([^`\s]+)`/g)) adaylar.add(m[1]);
  const ciplak = new RegExp(`(?:file:\\/\\/\\/)?[A-Za-z0-9_\\-./\\\\:]*[A-Za-z0-9_\\-]\\.(?:${UZANTI})(?::\\d+|#L\\d+)?(?![A-Za-z0-9])`, "g");
  // Bağlantının görünen metni kanıt değildir (hedefi yukarıda alındı); ters tırnak içi de alındı.
  const bagSiz = satir.replace(/\[[^\]]*\]\([^)]*\)/g, "").replace(/`[^`]*`/g, "");
  for (const m of bagSiz.matchAll(ciplak)) adaylar.add(m[0]);

  const atiflar: Atif[] = [];
  for (const ham of adaylar) {
    if (/^https?:\/\//i.test(ham)) continue; // ağ bağlantısı yerel kanıt değildir; sayılmaz
    let y = ham.replace(/^file:\/\/\//i, "");
    try {
      y = decodeURIComponent(y);
    } catch {}
    let satirNo: number | undefined;
    const sm = /(?:#L|:)(\d+)$/.exec(y);
    if (sm && new RegExp(`\\.(?:${UZANTI})(?:#L|:)\\d+$`).test(y)) {
      satirNo = Number(sm[1]);
      y = y.slice(0, sm.index);
    }
    if (!new RegExp(`\\.(?:${UZANTI})$`, "i").test(y)) continue; // kanıt sayılmayan biçim
    const zorunlu = baglantilar.has(ham) || satirNo !== undefined || /\.log$/i.test(y);
    atiflar.push({ ham, yol: y, satir: satirNo, zorunlu });
  }
  return atiflar;
}

function coz(yol: string, raporDizini: string): string | null {
  const adaylar = path.isAbsolute(yol) ? [yol] : [path.resolve(raporDizini, yol), path.resolve(yol)];
  return adaylar.find((a) => fs.existsSync(a) && fs.statSync(a).isFile()) ?? null;
}

// hepsi.ts tekKapiKos günlüğü mü? Biçim: 1. "=== KAPI BAŞLADI: <ad> ===", 2. "Komut: ...", 3. "Zaman: <ISO>",
// 4. "Faz: <faz> (kip: faz|test)", tam bir "=== KAPI BİTTİ: Çıkış Kodu = N" satırı ve onu izleyen son satır
// "=== KOŞUCU HÜKMÜ: yesil|kirmizi". Biçim tek başına kanıt değildir: başarı kanıtında ad kapilar.toml
// listesinden, Zaman/çıkış/hüküm de o koşunun kosu.json kaydından denetlenir (atifDenetle).
export function kosucuLogu(
  icerik: string
): { ad: string; zaman: string; cikis: number; hukum: string; faz: string; kip: string; komut: string } | null {
  const s = icerik.split(/\r?\n/).filter((x) => x.trim() !== "");
  if (s.length < 6) return null;
  const a = /^=== KAPI BAŞLADI: ([a-z0-9-]+) ===$/.exec(s[0]);
  if (!a || !/^Komut: \S/.test(s[1])) return null;
  const z = /^Zaman: (\S+)$/.exec(s[2]);
  if (!z || Number.isNaN(Date.parse(z[1]))) return null;
  const f = /^Faz: (\S+) \(kip: (faz|test)\)$/.exec(s[3]);
  if (!f) return null;
  const bitti = s.filter((x) => x.startsWith("=== KAPI BİTTİ:"));
  const b = /^=== KAPI BİTTİ: Çıkış Kodu = (-?\d+) \(Süre: \d+ ms\) ===$/.exec(s[s.length - 2]);
  const h = /^=== KOŞUCU HÜKMÜ: (yesil|kirmizi)\b.*===$/.exec(s[s.length - 1]);
  if (bitti.length !== 1 || !b || !h) return null;
  return { ad: a[1], zaman: z[1], cikis: Number(b[1]), hukum: h[1], faz: f[1], kip: f[2], komut: s[1].slice("Komut: ".length) };
}

type Tur = "basari" | "baktim";

/** Başarı kanıtının bağlamı: raporun fazı, o fazın kapilar.toml listesi ve koşunun kosu.json kaydı. */
interface Baglam {
  raporDizini: string;
  faz: string | null; // rapor port/kayit/P<n>/ altında değilse null
  cfg: KapilarYapilandirmasi | null;
  cfgHata?: string;
  kosu: any | null;
  kosuHata?: string;
}

function baglamKur(raporDizini: string, kapilarToml: string): Baglam {
  const b: Baglam = { raporDizini, faz: null, cfg: null, kosu: null };
  const f = path.basename(raporDizini);
  if (/^P\d+$/.test(f) && path.basename(path.dirname(raporDizini)) === "kayit") b.faz = f;
  try {
    b.cfg = kapilarTomlYukle(kapilarToml);
  } catch (e: any) {
    b.cfgHata = e?.message ?? String(e);
  }
  try {
    b.kosu = JSON.parse(fs.readFileSync(path.join(raporDizini, KOSU_DOSYASI), "utf-8"));
  } catch (e: any) {
    b.kosuHata = `${KOSU_DOSYASI} okunamadı (${e?.code ?? e?.message ?? e}): hepsi.ts bu fazı koşmamış`;
  }
  return b;
}

/** Başarı kanıtı sayılabilecek kapılar: rapor-kontrol kendini kanıtlayamaz; kip=uyari makine-sakin hep yeşil biter. */
function kanitKapilari(b: Baglam): { ad: string; komut: string }[] {
  if (!b.cfg || !b.faz) return [];
  const uyari = b.cfg.makineSakin.kip === "uyari";
  return (b.cfg.fazlar[b.faz] ?? []).filter((k) => k.ad !== "rapor-kontrol" && !(k.ad === "makine-sakin" && uyari));
}

// Bir atfı doğrular. sorun: atıf çözülemedi/uydurma; kapi: bu atıf hangi kapının başarısını KANITLAR.
// Başarı iddiası bir koşu sonucudur: yalnız bu koşuda hepsi.ts'nin yazdığı, kapilar.toml'daki o fazın
// kapı adına ait <ad>.log kanıtlar (kosu.json'daki kayıtla tutmalı). baktım bir inceleme beyanıdır:
// hepsi.ts günlüğü (hükmü ne olursa) ya da boş olmayan dosya:satır kanıtlar. Çıplak dosya adı ve rastgele
// dosya hiçbir iddiayı kanıtlamaz (yalnız çözülmesi gerekir).
function atifDenetle(a: Atif, b: Baglam, tur: Tur): { sorun?: string; kanit: boolean; kapi?: string } {
  const gercek = coz(a.yol, b.raporDizini);
  if (!gercek) return { sorun: `kanıt dosyası yok: ${a.ham}`, kanit: false };
  const icerik = fs.readFileSync(gercek, "utf-8");
  const satirlar = icerik.replace(/\r?\n$/, "").split(/\r?\n/);
  if (a.satir !== undefined && (a.satir < 1 || a.satir > satirlar.length)) {
    return { sorun: `satır yok: ${a.ham} (dosya ${satirlar.length} satır)`, kanit: false };
  }
  if (gercek.toLowerCase().endsWith(".log")) {
    const k = kosucuLogu(icerik);
    if (!k) return { sorun: `hepsi.ts günlüğü değil (KAPI BAŞLADI/Komut/Zaman/KAPI BİTTİ/KOŞUCU HÜKMÜ biçimi yok): ${a.ham}`, kanit: false };
    if (tur === "baktim") return { kanit: true };
    const red = (s: string) => ({ sorun: `${s}: ${a.ham}`, kanit: false });
    if (k.cikis !== 0 || k.hukum !== "yesil") return red(`kırmızı biten kapı günlüğü başarı kanıtı olamaz (çıkış ${k.cikis}, hüküm ${k.hukum})`);
    // Başarı yalnız raporun kendi port/kayit/<faz>/ dizinindeki, o fazın faz-kipi koşusuyla kanıtlanır.
    if (!b.faz) return red("rapor port/kayit/<faz>/ altında değil; başarı günlükle kanıtlanamaz");
    if (path.resolve(path.dirname(gercek)).toLowerCase() !== path.resolve(b.raporDizini).toLowerCase()) {
      return red(`günlük raporun faz dizininde değil (${b.faz})`);
    }
    if (k.faz !== b.faz || k.kip !== "faz") return red(`günlük ${b.faz} faz koşusu değil (Faz: ${k.faz}, kip: ${k.kip})`);
    // Ad: yalnız kapilar.toml'daki bu fazın kapısı; dosya adı, başlıktaki ad ve komut toml'la tutmalı.
    if (!b.cfg) return red(`kapilar.toml okunamadı (${b.cfgHata}); kapı listesi olmadan günlük kanıt sayılmaz`);
    const ad = path.basename(gercek).replace(/\.log$/i, "");
    const tanim = (b.cfg.fazlar[b.faz] ?? []).find((x) => x.ad === ad);
    if (!tanim) return red(`'${ad}' kapilar.toml'da ${b.faz} kapısı değil; listede olmayan günlük kanıt sayılmaz`);
    if (k.ad !== ad || k.komut !== tanim.komut.replaceAll("{faz}", b.faz)) {
      return red(`günlük başlığı kapilar.toml'la tutmuyor (ad ${k.ad}, komut ${JSON.stringify(k.komut)})`);
    }
    if (!kanitKapilari(b).some((x) => x.ad === ad)) {
      return red(`'${ad}' günlüğü başarı kanıtı olamaz (rapor-kontrol kendini, kip=uyari makine-sakin hiçbir şeyi kanıtlamaz)`);
    }
    // Koşu kaydı: hepsi.ts koşu başında listedeki günlükleri siler, her kapıdan sonra kosu.json'a yazar.
    if (!b.kosu) return red(b.kosuHata ?? `${KOSU_DOSYASI} yok`);
    const kayit = Array.isArray(b.kosu.kapilar) ? b.kosu.kapilar.find((x: any) => x?.ad === ad) : undefined;
    if (b.kosu.faz !== b.faz || b.kosu.kip !== "faz" || !kayit) return red(`${KOSU_DOSYASI} bu kapının ${b.faz} faz koşusunu kaydetmemiş`);
    if (kayit.baslangic !== k.zaman || kayit.cikis !== 0 || kayit.hukum !== "yesil") {
      return red(
        `günlük ${KOSU_DOSYASI} kaydıyla tutmuyor (kayıt: başlangıç ${kayit.baslangic}, çıkış ${kayit.cikis}, hüküm ${kayit.hukum}; günlük: ${k.zaman})`
      );
    }
    return { kanit: true, kapi: ad };
  }
  if (tur === "baktim" && a.satir !== undefined && satirlar[a.satir - 1].trim() !== "") return { kanit: true };
  return { kanit: false };
}

const kacis = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/**
 * Başarı iddiası satırının kanıtlaması gereken kapılar (Ek D 'Kanıtı aşan rapor'). Atıflar dışındaki metinde:
 * kapı adı ya da kökü (mutasyon -> mutasyon-kontrol) geçen kapı; P<n>-A<k> -> kabul-haritasi; 'tüm/bütün/hepsi'
 * -> kanıt sayılabilen bütün kapılar. Hiçbiri yoksa iddia hiçbir ölçüme bağlanmamıştır.
 * ponytail: sözcük eşlemesi; satırda bağlı bir kapı varken kapı adı geçmeyen ek yan cümleyi ('sunucu
 * çalışıyor') ayırt etmez. Doğruluğunu PRD gereği hakem denetler.
 */
function gerekenKapilar(satir: string, b: Baglam): { kapilar: Set<string>; evrensel: boolean } {
  const metin = katla(
    satir
      .replace(/\]\([^)]*\)/g, "]")
      .replace(/`[^`]*`/g, " ")
      .replace(new RegExp(`\\S+\\.(?:${UZANTI})(?::\\d+|#L\\d+)?`, "gi"), " ")
  );
  const tum = b.cfg && b.faz ? b.cfg.fazlar[b.faz] ?? [] : [];
  const gereken = new Set<string>();
  const evrensel = /\b(?:tum|butun|hepsi|her sey|all|every\w*|entire)\b/.test(metin);
  if (evrensel) for (const k of kanitKapilari(b)) gereken.add(k.ad);
  if (/\bp\d+-a\d+\b/.test(metin)) gereken.add("kabul-haritasi");
  for (const k of tum) {
    const kok = k.ad.split("-")[0];
    const desenler = [k.ad, k.ad.replace(/-/g, " ")];
    if (kok.length >= 4 && kok !== "rapor") desenler.push(kok);
    if (desenler.some((d) => new RegExp(`(?<![a-z0-9])${kacis(d)}`).test(metin))) gereken.add(k.ad);
  }
  return { kapilar: gereken, evrensel };
}

// Satırın kanıtı. baktım: >=1 kanıtlayan atıf. başarı: iddianın bağlandığı HER kapı için bu koşudan yeşil
// günlük. Her iki türde zorunlu atıfların HEPSİ çözülmüş olmalı. Sorun listesi döner (boş = kanıtlı).
function kanitSorunlari(satir: string, b: Baglam, tur: Tur): string[] {
  const sorunlar: string[] = [];
  const kanitlanan = new Set<string>();
  let kanit = 0;
  for (const a of atiflariCikar(satir)) {
    const r = atifDenetle(a, b, tur);
    if (r.kanit) kanit++;
    if (r.kapi) kanitlanan.add(r.kapi);
    if (r.sorun && a.zorunlu) sorunlar.push(r.sorun);
  }
  if (tur === "baktim") {
    if (kanit === 0 && sorunlar.length === 0) {
      sorunlar.push("kanıt yok: 'baktım' bir hepsi.ts günlüğüne ya da boş olmayan dosya:satır'a bağlanmalı (çıplak dosya adı yetmez)");
    }
    return sorunlar;
  }
  if (sorunlar.length) return sorunlar;
  if (kanit === 0) {
    return ["kanıt yok: başarı iddiası bu koşuda yeşil biten bir kapı günlüğüne (port/kayit/<faz>/<kapı>.log) bağlanmalı"];
  }
  const { kapilar: gereken, evrensel } = gerekenKapilar(satir, b);
  if (gereken.size === 0) {
    return [
      evrensel
        ? "'tüm' iddiası kanıtlanamaz: bu fazda kanıt sayılabilen kapı yok"
        : "iddia hangi kapıya ait belli değil: satırda ölçen kapının adı (örn. 'kulliyat-tara') ya da kabul maddesi (P<n>-A<k>) yazılmalı",
    ];
  }
  const eksik = [...gereken].filter((k) => !kanitlanan.has(k));
  if (eksik.length) {
    sorunlar.push(
      `iddia kanıtını aşıyor: ${eksik.join(", ")} için bu koşudan yeşil günlük gösterilmedi${evrensel ? " ('tüm' iddiası her kapının günlüğünü ister)" : ""}`
    );
  }
  return sorunlar;
}

export function raporuDenetle(
  raporYolu: string,
  secenek: { kapilarToml?: string } = {}
): {
  uygun: boolean;
  hatalar: string[];
  kontrolEdilenIddiaSayisi: number;
} {
  const hatalar: string[] = [];

  if (!fs.existsSync(raporYolu)) {
    return { uygun: false, hatalar: [`Rapor dosyası bulunamadı: ${raporYolu}`], kontrolEdilenIddiaSayisi: 0 };
  }

  // Kapı listesi mühürlü kapilar.toml'dan (bu betiğin yanındaki), cwd'den bağımsız.
  const b = baglamKur(path.dirname(path.resolve(raporYolu)), secenek.kapilarToml ?? path.join(import.meta.dir, "kapilar.toml"));
  const satirlar = fs.readFileSync(raporYolu, "utf-8").split(/\r?\n/);
  const katliSatirlar = satirlar.map(katla);

  // 1. Ek D: her madde 'baktım: <var olan kanıt>' ya da 'bakmadım'
  for (const madde of EK_D_MADDELERI) {
    const km = katla(madde);
    let cevap: string | null = null;
    for (let i = 0; i < satirlar.length; i++) {
      const k = katliSatirlar[i];
      const yer = k.indexOf(km);
      if (yer < 0) continue;
      const m = /^[\s:*]*(baktim|bakmadim)\b/.exec(k.slice(yer + km.length));
      if (!m) continue;
      if (m[1] === "bakmadim") {
        cevap = "tamam";
        break;
      }
      const sorunlar = kanitSorunlari(satirlar[i], b, "baktim");
      cevap = sorunlar.length ? `Satır ${i + 1}: '${madde}: baktım' kanıtı geçersiz: ${sorunlar.join("; ")}` : "tamam";
      if (cevap === "tamam") break;
    }
    if (cevap === null) {
      hatalar.push(`Ek D sahte-kapı listesi maddesi eksik veya cevapsız: '${madde}' ('baktım: <kanıt>' veya 'bakmadım' zorunludur)`);
    } else if (cevap !== "tamam") {
      hatalar.push(`Ek D ${cevap}`);
    }
  }

  // 2. Başarı iddiaları
  let kontrolEdilenIddiaSayisi = 0;
  for (let i = 0; i < satirlar.length; i++) {
    const satir = satirlar[i].trim();
    // Başlık (#), yorum, liste ve tablo satırları da iddia taşır: hiçbiri muaf değildir.
    if (!satir) continue;
    const k = katliSatirlar[i];
    if (!BASARI_IDDIASI.test(k) && !BASARI_IDDIASI.test(katla(bicimsiz(satir)))) continue;
    kontrolEdilenIddiaSayisi++;
    // 'bakmadım' aynı satırdaki başarı iddiasını muaf TUTMAZ: bakılmayan yer iddia içermeden yazılır.
    const sorunlar = kanitSorunlari(satir, b, "basari");
    if (sorunlar.length) {
      hatalar.push(`Satır ${i + 1}: Bağsız başarı iddiası (${sorunlar.join("; ")}):\n   "${satir}"`);
    }
  }

  return { uygun: hatalar.length === 0, hatalar, kontrolEdilenIddiaSayisi };
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  if (args.length !== 1) {
    console.error("Kullanım: bun araclar/kapilar/rapor-kontrol.ts <port/kayit/P1/RAPOR.md | P1>");
    console.log("HUKUM: kirmizi tek argüman (rapor yolu ya da faz) bekleniyordu");
    process.exit(2);
  }
  let raporYolu = args[0];
  if (!raporYolu.endsWith(".md")) raporYolu = path.join("port", "kayit", raporYolu, "RAPOR.md");

  const sonuc = raporuDenetle(raporYolu);
  console.log(`[rapor-kontrol] İncelenen başarı iddiası sayısı: ${sonuc.kontrolEdilenIddiaSayisi}`);
  if (sonuc.uygun) {
    console.log(`[rapor-kontrol] Rapor '${raporYolu}' sözleşmeye uyuyor.`);
    console.log("HUKUM: yesil");
    process.exit(0);
  }
  console.error(`❌ [rapor-kontrol] RAPOR SÖZLEŞMESİ İHLALLERİ BULUNDU (${raporYolu}):`);
  for (const h of sonuc.hatalar) console.error(`  - ${h}`);
  console.log(`HUKUM: kirmizi ${sonuc.hatalar.length} rapor sözleşmesi ihlali (${raporYolu})`);
  process.exit(1);
}
