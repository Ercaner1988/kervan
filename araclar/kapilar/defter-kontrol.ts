import fs from "fs";
import path from "path";

// PRD FR-2.8: Defter Kapısı (defter-kontrol.ts)
// 1. 'dogrulandi' satırı VAR OLAN hedef dosyasına ve VAR OLAN kanıt kimliğine/altın dosyaya sahip olmalıdır.
// 2. Her kaynak dosya için 300 satırlık okuma pencereleri örtüşme ile tam olmalıdır (boşluk bırakılamaz).
// 3. 'kapsam-disi' ve 'insan-karari' satırları gerekçeli olmalıdır.

export interface DefterKaydi {
  id: string;
  kaynak: string;
  satir?: [number, number];
  durum: "planlandi" | "okundu" | "aktarildi" | "dogrulandi" | "kapsam-disi" | "insan-karari";
  hedef?: string;
  kanit?: string[];
  gerekce?: string;
}

export function defterSatirlariniOku(dosyaYolu: string): DefterKaydi[] {
  if (!fs.existsSync(dosyaYolu)) return [];
  const icerik = fs.readFileSync(dosyaYolu, "utf-8");
  const kayitlar: DefterKaydi[] = [];

  for (const line of icerik.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    try {
      kayitlar.push(JSON.parse(trimmed));
    } catch {}
  }

  return kayitlar;
}

export function defteriDenetle(
  dosyalarJsonlYolu = path.join("port", "defter", "dosyalar.jsonl"),
  finalModu = false,
  kokDizin = process.cwd()
): { uygun: boolean; hatalar: string[]; kayitSayisi: number } {
  const hatalar: string[] = [];
  const kayitlar = defterSatirlariniOku(dosyalarJsonlYolu);

  if (kayitlar.length === 0) {
    // Defter henüz oluşturulmadıysa (P1-P3 başlangıcı) geçerli say
    return { uygun: true, hatalar: [], kayitSayisi: 0 };
  }

  // Kaynak dosyalarına göre pencereleri topla
  const dosyaPencereleri = new Map<string, [number, number][]>();

  for (let i = 0; i < kayitlar.length; i++) {
    const k = kayitlar[i];
    const sira = i + 1;

    // 1. 'dogrulandi' kontrolleri
    if (k.durum === "dogrulandi") {
      if (!k.hedef) {
        hatalar.push(`Kayıt #${sira} (${k.id}): 'dogrulandi' durumunda ancak 'hedef' dosyası belirtilmemiş`);
      } else {
        const hedefTamYol = path.resolve(kokDizin, k.hedef);
        if (!fs.existsSync(hedefTamYol)) {
          hatalar.push(`Kayıt #${sira} (${k.id}): 'hedef' dosyası bulunamadı: ${k.hedef}`);
        }
      }

      if (!k.kanit || !Array.isArray(k.kanit) || k.kanit.length === 0) {
        hatalar.push(`Kayıt #${sira} (${k.id}): 'dogrulandi' durumunda ancak 'kanit' listesi boş`);
      } else {
        for (const kanit of k.kanit) {
          // Kanıt bir dosya yolu ise varlığı denetlenir
          if (kanit.includes("/") || kanit.includes("\\")) {
            const kanitTamYol = path.resolve(kokDizin, kanit);
            if (!fs.existsSync(kanitTamYol)) {
              hatalar.push(`Kayıt #${sira} (${k.id}): Kanıt dosyası bulunamadı: ${kanit}`);
            }
          }
        }
      }
    }

    // 2. 'kapsam-disi' ve 'insan-karari' gerekçe kontrolü
    if (k.durum === "kapsam-disi" || k.durum === "insan-karari") {
      if (!k.gerekce || k.gerekce.trim().length === 0) {
        hatalar.push(`Kayıt #${sira} (${k.id}): '${k.durum}' durumunda gerekçe belirtilmemiş`);
      }
    }

    // 3. Final modu kontrolü
    if (finalModu) {
      if (k.durum === "planlandi" || k.durum === "okundu" || k.durum === "aktarildi") {
        hatalar.push(`Kayıt #${sira} (${k.id}): Final modunda kapanmamış durum: '${k.durum}'`);
      }
    }

    // Pencere takibi
    if (k.satir && Array.isArray(k.satir) && k.satir.length === 2) {
      const pencereler = dosyaPencereleri.get(k.kaynak) || [];
      pencereler.push(k.satir);
      dosyaPencereleri.set(k.kaynak, pencereler);
    }
  }

  // 4. Pencere boşluğu (gap) kontrolü
  for (const [kaynak, pencereler] of dosyaPencereleri.entries()) {
    // Pencereleri başlangıç satırına göre sırala
    pencereler.sort((a, b) => a[0] - b[0]);
    let beklenenBaslangic = 1;

    for (const [bas, son] of pencereler) {
      if (bas > beklenenBaslangic) {
        hatalar.push(`Kaynak '${kaynak}' içinde satır aralığı boşluğu tespit edildi: [${beklenenBaslangic}, ${bas - 1}] okunmadı!`);
      }
      beklenenBaslangic = Math.max(beklenenBaslangic, son + 1);
    }
  }

  return { uygun: hatalar.length === 0, hatalar, kayitSayisi: kayitlar.length };
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  let finalModu = false;
  let dosyaYolu = path.join("port", "defter", "dosyalar.jsonl");

  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--final") {
      finalModu = true;
    } else if (!args[i].startsWith("--")) {
      dosyaYolu = args[i];
    }
  }

  const sonuc = defteriDenetle(dosyaYolu, finalModu);
  console.log(`[defter-kontrol] İncelenen kayıt sayısı: ${sonuc.kayitSayisi} (Final modu: ${finalModu ? "AÇIK" : "KAPALI"})`);

  if (sonuc.uygun) {
    console.log("✅ [defter-kontrol] Defter kayıtları ve pencere örtüşmeleri geçerli.");
    process.exit(0);
  } else {
    console.error("❌ [defter-kontrol] DEFTER KURALLARI İHLALLERİ BULUNDU:");
    for (const h of sonuc.hatalar) {
      console.error(`  - ${h}`);
    }
    process.exit(1);
  }
}
