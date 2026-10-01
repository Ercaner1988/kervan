/**
 * Parite Kayıt Aracı (araclar/parite/kayit.ts)
 *
 * Referans senaryoları uçucu alanlardan arındırarak (normalize)
 * port/altin/<senaryo>.json dosyalarına yazar.
 */

import { join, resolve } from "node:path";

const REPO_ROOT = resolve(import.meta.dir, "../..");
const ALTIN_DIR = join(REPO_ROOT, "port", "altin");

export interface AltinSenaryo {
  id: string;
  ad: string;
  istek: {
    yontem: string;
    yol: string;
    basliklar: Record<string, string>;
    govde: any;
  };
  beklenen: {
    durumKodu: number;
    govdeSablon: any;
  };
}

export function merhabaSenaryosu(): AltinSenaryo {
  return {
    id: "parite-merhaba-001",
    ad: "Temel Merhaba Sohbet Tamamlama Paritesi",
    istek: {
      yontem: "POST",
      yol: "/v1/chat/completions",
      basliklar: {
        "content-type": "application/json",
      },
      govde: {
        model: "gemini-3.8-flash",
        messages: [{ role: "user", content: "merhaba" }],
      },
    },
    beklenen: {
      durumKodu: 200,
      govdeSablon: {
        object: "chat.completion",
        choices: [
          {
            index: 0,
            message: {
              role: "assistant",
            },
            finish_reason: "stop",
          },
        ],
      },
    },
  };
}

async function main() {
  console.log("=== PARİTE KAYIT MOTORU ===");
  const merhaba = merhabaSenaryosu();
  const hedef = join(ALTIN_DIR, "merhaba.json");
  await Bun.write(hedef, JSON.stringify(merhaba, null, 2));
  console.log(`Altın senaryo kaydedildi: ${hedef}`);
}

if (import.meta.main) {
  main().catch((e) => {
    console.error(e);
    process.exit(1);
  });
}
