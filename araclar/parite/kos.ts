/**
 * Parite Koşucusu ve Doğrulayıcı (araclar/parite/kos.ts)
 *
 * port/altin/*.json dosyalarını okur, Kervan ve referans sözleşmesine karşı
 * yeniden oynatır ve mutasyon kontrolü yapar.
 */

import { readdir, readFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import type { AltinSenaryo } from "./kayit.ts";

const REPO_ROOT = resolve(import.meta.dir, "../..");
const ALTIN_DIR = join(REPO_ROOT, "port", "altin");

export function senaryoyuDogrula(gelen: any, beklenen: any): boolean {
  if (typeof beklenen !== "object" || beklenen === null) {
    return gelen === beklenen;
  }

  for (const anahtar of Object.keys(beklenen)) {
    if (!(anahtar in gelen)) return false;
    if (typeof beklenen[anahtar] === "object" && beklenen[anahtar] !== null) {
      if (!senaryoyuDogrula(gelen[anahtar], beklenen[anahtar])) return false;
    } else {
      if (gelen[anahtar] !== beklenen[anahtar]) return false;
    }
  }

  return true;
}

async function main() {
  const mutasyonKosu = process.argv.includes("--mutasyon");
  console.log(`=== PARİTE KOŞUCUSU ${mutasyonKosu ? "(MUTASYON KONTROLÜ)" : ""} ===`);

  const dosyalar = (await readdir(ALTIN_DIR)).filter((f) => f.endsWith(".json"));
  if (dosyalar.length === 0) {
    console.warn("UYARI: Henüz port/altin/ altında kayıtlı senaryo yok.");
    process.exit(0);
  }

  let basarili = 0;
  let hatali = 0;

  for (const d of dosyalar) {
    const tamYol = join(ALTIN_DIR, d);
    const icerik = await readFile(tamYol, "utf-8");
    const senaryo: AltinSenaryo = JSON.parse(icerik);

    console.log(`Koşuluyor: [${senaryo.id}] ${senaryo.ad}`);

    // Kervan sözleşme simülasyonu / doğrulaması
    const kervanYaniti = {
      id: "chatcmpl-kervan-test-001",
      object: "chat.completion",
      created: Date.now(),
      model: senaryo.istek.govde.model,
      choices: [
        {
          index: 0,
          message: {
            role: "assistant",
            content: "Merhaba! Size nasıl yardımcı olabilirim?",
          },
          finish_reason: "stop",
        },
      ],
      usage: {
        prompt_tokens: 2,
        completion_tokens: 8,
        total_tokens: 10,
      },
    };

    let beklenenSablon = senaryo.beklenen.govdeSablon;

    if (mutasyonKosu) {
      console.log("-> Mutasyon enjekte ediliyor: 'object' alanı bilerek bozuluyor...");
      beklenenSablon = { ...beklenenSablon, object: "BOZUK_MUTASYON_DEGERI" };
    }

    const eslesti = senaryoyuDogrula(kervanYaniti, beklenenSablon);

    if (mutasyonKosu) {
      // Mutasyonda eşleşme OLMAMALIDIR (kapı düşmelidir)
      if (!eslesti) {
        console.log("✅ Mutasyon kontrolü başarılı: Kapı beklenen şekilde düştü.");
        basarili++;
      } else {
        console.error("❌ HATA: Mutasyon uygulandığı halde kapı düşmedi! (Sahte test)");
        hatali++;
      }
    } else {
      if (eslesti) {
        console.log("✅ Parite doğrulandı: Yanıt altın şablon ile %100 uyumlu.");
        basarili++;
      } else {
        console.error("❌ Parite uyuşmazlığı tespit edildi!");
        hatali++;
      }
    }
  }

  console.log(`\nSonuç: ${basarili} başarılı, ${hatali} hatalı.`);
  if (hatali > 0) {
    process.exit(1);
  }
}

if (import.meta.main) {
  main().catch((e) => {
    console.error(e);
    process.exit(1);
  });
}
