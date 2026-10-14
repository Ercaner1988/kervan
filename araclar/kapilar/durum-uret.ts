import crypto from "crypto";
import fs from "fs";
import path from "path";

// PRD FR-2.7: Üretilmiş Durum Oluşturucu (durum-uret.ts)
// port/durum.json (§6 şeması) + port/uclar.toml → port/DURUM.md, ve README*.md içindeki
// <!-- DURUM:BASLA --> … <!-- DURUM:BITIS --> bloğu. Çıktı yalnız girdilere bağlıdır (saat, rastgelelik yok);
// durum-kontrol aynı işlevle yeniden üretip bayt bayt karşılaştırır.
// Fail-closed: durum.json yoksa, ayrıştırılamazsa ya da şemaya uymazsa hiçbir şey yazılmaz, hata atılır.

export const README_DOSYALARI = ["README.md", "README.en.md", "README.ar.md", "README.ja.md"];
export const BLOK_BASLA = "<!-- DURUM:BASLA -->";
export const BLOK_BITIS = "<!-- DURUM:BITIS -->";

export interface DurumJson {
  uretici_ozeti: string;
  uretim_zamani: string;
  faz: string;
  kip?: string;
  kapilar_toml_ozeti?: string;
  kapilar: { ad: string; cikis: number; sure_ms: number; log: string }[];
  fazlar: { id: string; commit: string; kanit: string[] }[];
  sayimlar: { route_ts: number; defter_planlandi: number; defter_dogrulandi: number };
  tum_kapilar_yesil_mi?: boolean;
}

const dizgeMi = (v: unknown): v is string => typeof v === "string" && v.trim().length > 0;
const sayiMi = (v: unknown): v is number => Number.isInteger(v) && (v as number) >= 0;

/** §6 şeması; boş dizi = uygun. */
export function durumSemasiniDenetle(d: any): string[] {
  const h: string[] = [];
  if (d === null || typeof d !== "object" || Array.isArray(d)) return ["durum.json bir JSON nesnesi değil"];
  if (typeof d.uretici_ozeti !== "string" || !/^sha256:[0-9a-f]{64}$/.test(d.uretici_ozeti)) {
    h.push(`uretici_ozeti 'sha256:<64 hex>' değil: ${JSON.stringify(d.uretici_ozeti)}`);
  }
  if (!dizgeMi(d.uretim_zamani) || Number.isNaN(Date.parse(d.uretim_zamani))) h.push(`uretim_zamani geçerli bir ISO zaman değil: ${JSON.stringify(d.uretim_zamani)}`);
  if (!dizgeMi(d.faz)) h.push(`faz eksik ya da dizge değil: ${JSON.stringify(d.faz)}`);
  if (!Array.isArray(d.kapilar) || d.kapilar.length === 0) h.push("kapilar boş ya da dizi değil (hiç kapı koşulmamış)");
  else
    d.kapilar.forEach((k: any, i: number) => {
      if (!k || !dizgeMi(k.ad) || !Number.isInteger(k.cikis) || !sayiMi(k.sure_ms) || !dizgeMi(k.log)) {
        h.push(`kapilar[${i}] {ad, cikis, sure_ms, log} biçiminde değil: ${JSON.stringify(k)}`);
      }
    });
  if (!Array.isArray(d.fazlar)) h.push("fazlar dizi değil");
  else
    d.fazlar.forEach((f: any, i: number) => {
      if (!f || !dizgeMi(f.id) || typeof f.commit !== "string" || !/^[0-9a-f]{7,40}$/.test(f.commit) || !Array.isArray(f.kanit) || f.kanit.length === 0 || !f.kanit.every(dizgeMi)) {
        h.push(`fazlar[${i}] {id, commit (hex sha), kanit[≥1]} biçiminde değil: ${JSON.stringify(f)}`);
      }
    });
  const s = d.sayimlar;
  if (!s || typeof s !== "object" || !sayiMi(s.route_ts) || !sayiMi(s.defter_planlandi) || !sayiMi(s.defter_dogrulandi)) {
    h.push(`sayimlar {route_ts, defter_planlandi, defter_dogrulandi} (≥0 tamsayı) değil: ${JSON.stringify(s)}`);
  }
  if (d.tum_kapilar_yesil_mi !== undefined && Array.isArray(d.kapilar)) {
    const gercek = d.kapilar.length > 0 && d.kapilar.every((k: any) => k?.cikis === 0);
    if (d.tum_kapilar_yesil_mi !== gercek) h.push(`tum_kapilar_yesil_mi=${d.tum_kapilar_yesil_mi} kapı çıkışlarıyla çelişiyor (${gercek})`);
  }
  return h;
}

export function durumJsonOku(durumJsonYolu: string): DurumJson {
  if (!fs.existsSync(durumJsonYolu)) throw new Error(`durum.json bulunamadı: ${durumJsonYolu}`);
  let d: any;
  try {
    d = JSON.parse(fs.readFileSync(durumJsonYolu, "utf-8"));
  } catch (e: any) {
    throw new Error(`durum.json ayrıştırılamadı: ${e.message}`);
  }
  const h = durumSemasiniDenetle(d);
  if (h.length) throw new Error(`durum.json §6 şemasına uymuyor: ${h.join("; ")}`);
  return d;
}

const yesilMi = (d: DurumJson) => d.kapilar.every((k) => k.cikis === 0);
const hucre = (v: unknown) => String(v).replace(/\|/g, "\\|").replace(/\r?\n/g, " ");

export function durumMarkdownUret(
  durumJsonYolu = path.join("port", "durum.json"),
  uclarTomlYolu = path.join("port", "uclar.toml")
): { markdown: string; hash: string } {
  const d = durumJsonOku(durumJsonYolu);
  if (!fs.existsSync(uclarTomlYolu)) throw new Error(`uclar.toml bulunamadı: ${uclarTomlYolu}`);
  const uclar = fs.readFileSync(uclarTomlYolu, "utf-8").replace(/\r\n/g, "\n").trim();

  let g = `# DURUM.md — Kervan Aktarım Durum Defteri (ÜRETİLMİŞTİR)

> ⚠️ BU DOSYA araclar/kapilar/durum-uret.ts ÇIKTISIDIR. ELLE DÜZENLEMEYİNİZ.
> durum-kontrol bu dosyayı port/durum.json'dan yeniden üretip bayt bayt karşılaştırır.

- **Mevcut Faz:** ${d.faz ?? "belirtilmemiş"}
- **Üretim Zamanı (durum.json):** ${d.uretim_zamani}
- **Üretici Özeti:** ${d.uretici_ozeti}
- **Tüm Kapılar Yeşil mi:** ${yesilMi(d) ? "✅ EVET" : "❌ HAYIR"}

---

## 1. Kapı Sonuçları

| Kapı Adı | Çıkış Kodu | Süre (ms) | Log Dosyası |
|---|:---:|:---:|---|
`;
  for (const k of d.kapilar) {
    g += `| ${hucre(k.ad)} | ${k.cikis === 0 ? "✅" : "❌"} ${k.cikis} | ${k.sure_ms} | ${hucre(k.log)} |\n`;
  }
  g += `
## 2. Fazlar

| Faz | Commit | Kanıt |
|---|---|---|
`;
  for (const f of d.fazlar) g += `| ${hucre(f.id)} | ${f.commit} | ${hucre(f.kanit.join(", "))} |\n`;
  if (d.fazlar.length === 0) g += `| (yok) | | |\n`;
  g += `
## 3. Sayımlar

- route_ts: ${d.sayimlar.route_ts}
- defter_planlandi: ${d.sayimlar.defter_planlandi}
- defter_dogrulandi: ${d.sayimlar.defter_dogrulandi}

## 4. HTTP Rota Durumu (port/uclar.toml)

\`\`\`toml
${uclar}
\`\`\`
`;
  const hash = crypto.createHash("sha256").update(g).digest("hex");
  return { markdown: `<!-- URETILMIS_DURUM_HASH: ${hash} -->\n${g}`, hash };
}

/** README bloğunun içi (işaretler hariç, başta ve sonda \n). */
export function readmeBlokUret(d: DurumJson): string {
  let b = `\n> Bu blok araclar/kapilar/durum-uret.ts çıktısıdır; elle düzenlemeyin. Kaynak: port/durum.json\n\n`;
  b += `- Faz: ${d.faz ?? "belirtilmemiş"} (${d.uretim_zamani})\n`;
  b += `- Tüm kapılar yeşil mi: ${yesilMi(d) ? "evet" : "hayır"} (${d.kapilar.filter((k) => k.cikis === 0).length}/${d.kapilar.length})\n`;
  b += `- Fazlar: ${d.fazlar.length ? d.fazlar.map((f) => `${f.id}@${f.commit}`).join(", ") : "yok"}\n`;
  b += `- Defter: planlandi ${d.sayimlar.defter_planlandi}, dogrulandi ${d.sayimlar.defter_dogrulandi}; route_ts ${d.sayimlar.route_ts}\n`;
  return b;
}

/** README içeriğindeki bloğu ayıklar: null = blok yok; throw = bozuk işaretler. */
export function readmeBlokBul(icerik: string): { bas: number; son: number; govde: string } | null {
  const n1 = icerik.split(BLOK_BASLA).length - 1;
  const n2 = icerik.split(BLOK_BITIS).length - 1;
  if (n1 === 0 && n2 === 0) return null;
  if (n1 !== 1 || n2 !== 1) throw new Error(`durum bloğu işaretleri bozuk (BASLA ${n1}, BITIS ${n2}; tam bir çift olmalı)`);
  const bas = icerik.indexOf(BLOK_BASLA) + BLOK_BASLA.length;
  const son = icerik.indexOf(BLOK_BITIS);
  if (son < bas) throw new Error("durum bloğu: BITIS, BASLA'dan önce");
  return { bas, son, govde: icerik.slice(bas, son) };
}

/** Yazar; README'lerden blok işareti taşımayanlar `eksik`te döner (durum-kontrol onları kırmızı sayar). */
export function durumUretVeYaz(kok = process.cwd()): { yazilan: string[]; eksik: string[] } {
  const durumJson = path.join(kok, "port", "durum.json");
  const { markdown } = durumMarkdownUret(durumJson, path.join(kok, "port", "uclar.toml"));
  const d = durumJsonOku(durumJson);
  const blok = readmeBlokUret(d);
  // Önce hepsini hesapla (bozuk README varsa hiçbir şey yazma), sonra yaz.
  const yazilacak: [string, string][] = [[path.join(kok, "port", "DURUM.md"), markdown]];
  const eksik: string[] = [];
  for (const ad of README_DOSYALARI) {
    const yol = path.join(kok, ad);
    if (!fs.existsSync(yol)) continue;
    const icerik = fs.readFileSync(yol, "utf-8");
    let b;
    try {
      b = readmeBlokBul(icerik);
    } catch (e: any) {
      throw new Error(`${ad}: ${e.message}`);
    }
    if (b) yazilacak.push([yol, icerik.slice(0, b.bas) + blok + icerik.slice(b.son)]);
    else eksik.push(ad);
  }
  for (const [yol, icerik] of yazilacak) fs.writeFileSync(yol, icerik, "utf-8");
  return { yazilan: yazilacak.map(([y]) => path.relative(kok, y)), eksik };
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  let kok = process.cwd();
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--kok" && args[i + 1]) kok = path.resolve(args[++i]);
    else {
      console.log(`HUKUM: kirmizi bilinmeyen argüman: ${args[i]}`);
      process.exit(1);
    }
  }
  try {
    const { yazilan, eksik } = durumUretVeYaz(kok);
    console.log(`[durum-uret] yazıldı: ${yazilan.join(", ")}`);
    if (eksik.length) {
      console.log(`[durum-uret] durum bloğu işareti (${BLOK_BASLA} … ${BLOK_BITIS}) olmayan README: ${eksik.join(", ")}`);
      console.log(`HUKUM: kirmizi README durum bloğu yok: ${eksik.join(", ")} (işaretleri ekleyip yeniden koşun)`);
      process.exit(1);
    }
    console.log("HUKUM: yesil");
  } catch (e: any) {
    console.log(`[durum-uret] üretilemedi, hiçbir dosya yazılmadı: ${e?.message ?? e}`);
    console.log(`HUKUM: kirmizi ${String(e?.message ?? e).replace(/\s+/g, " ")}`);
    process.exit(1);
  }
}
