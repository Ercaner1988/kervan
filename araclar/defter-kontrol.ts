/**
 * Kervan Defter Kontrol Aracı (Bun-first)
 *
 * port/defter/dosyalar.jsonl ve port/defter/davranislar.jsonl dosyalarını
 * §3.3 kural ve kriterlerine göre denetler.
 */

import { readFile } from "node:fs/promises";
import { join, resolve } from "node:path";

const REPO_ROOT = resolve(import.meta.dir, "..");
const DEFTER_DIR = join(REPO_ROOT, "port", "defter");

interface DosyaKaydi {
  id: string;
  yol: string;
  satir: number;
  durum: string;
  hedef: string | null;
  okuma: {
    ajan: string | null;
    araliklar: [number, number][];
    kart: string | null;
  };
  kanit: string[];
  gerekce: string | null;
}

interface DavranisKaydi {
  id: string;
  tur: string;
  ad: string;
  kaynak: string[];
  hedef: string | null;
  durum: string;
  kanit: string[];
}

async function main() {
  console.log("=== DEFTER KONTROL BAŞLADI ===");

  const dosyalarIcerik = await readFile(join(DEFTER_DIR, "dosyalar.jsonl"), "utf-8");
  const davranislarIcerik = await readFile(join(DEFTER_DIR, "davranislar.jsonl"), "utf-8");

  const dosyalar: DosyaKaydi[] = dosyalarIcerik
    .trim()
    .split("\n")
    .filter(Boolean)
    .map((l) => JSON.parse(l));

  const davranislar: DavranisKaydi[] = davranislarIcerik
    .trim()
    .split("\n")
    .filter(Boolean)
    .map((l) => JSON.parse(l));

  console.log(`Toplam Dosya Kaydı: ${dosyalar.length}`);
  console.log(`Toplam Davranış Kaydı: ${davranislar.length}`);

  // Durum dağılımı
  const dosyaDurumlari: Record<string, number> = {};
  for (const d of dosyalar) {
    dosyaDurumlari[d.durum] = (dosyaDurumlari[d.durum] || 0) + 1;
  }

  const davranisDurumlari: Record<string, number> = {};
  for (const b of davranislar) {
    davranisDurumlari[b.durum] = (davranisDurumlari[b.durum] || 0) + 1;
  }

  console.log("\nDosya Durum Dağılımı:", dosyaDurumlari);
  console.log("Davranış Durum Dağılımı:", davranisDurumlari);

  let hataVar = false;

  // 1. Kanıtsız "dogrulandi" kontrolü
  const kanitsizDosya = dosyalar.filter((d) => d.durum === "dogrulandi" && (!d.kanit || d.kanit.length === 0));
  const kanitsizDavranis = davranislar.filter((b) => b.durum === "dogrulandi" && (!b.kanit || b.kanit.length === 0));

  if (kanitsizDosya.length > 0) {
    console.error(`HATA: ${kanitsizDosya.length} dosya kanıtsız 'dogrulandi' durumunda!`);
    hataVar = true;
  }

  if (kanitsizDavranis.length > 0) {
    console.error(`HATA: ${kanitsizDavranis.length} davranış kanıtsız 'dogrulandi' durumunda!`);
    hataVar = true;
  }

  // 2. Gerekçesiz "kapsam-disi" kontrolü
  const gerekcesizKapsamDisi = dosyalar.filter((d) => d.durum === "kapsam-disi" && (!d.gerekce || d.gerekce.trim() === ""));
  if (gerekcesizKapsamDisi.length > 0) {
    console.error(`HATA: ${gerekcesizKapsamDisi.length} dosya gerekçesiz 'kapsam-disi' durumunda!`);
    hataVar = true;
  }

  // 3. Geçersiz durum var mı?
  const gecerliDurumlar = new Set(["planlandi", "okundu", "aktarildi", "dogrulandi", "kapsam-disi", "insan-karari"]);
  for (const d of dosyalar) {
    if (!gecerliDurumlar.has(d.durum)) {
      console.error(`HATA: Geçersiz dosya durumu: ${d.id} (${d.durum})`);
      hataVar = true;
    }
  }

  if (hataVar) {
    console.error("\n❌ DEFTER DENETİMİ BAŞARISIZ OLDU!");
    process.exit(1);
  } else {
    console.log("\n✅ DEFTER DENETİMİ BAŞARILI: Tüm kurallar geçerli.");
  }
}

main().catch((err) => {
  console.error("Beklenmeyen hata:", err);
  process.exit(1);
});
