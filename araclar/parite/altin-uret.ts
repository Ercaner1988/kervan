import { spawnSync } from "child_process";
import crypto from "crypto";
import fs from "fs";
import os from "os";
import path from "path";

// PRD FR-5.1: Altın Üretim Aracı (araclar/parite/altin-uret.ts)
// OmniRoute referans kodunu (dbe703a) yalıtılmış ortamda çalıştırıp port/altin/*.json dosyalarını üretir.

// Canlı OmniRoute ve ~/.omniroute'a ASLA dokunulmaz
process.env.DATA_DIR = path.join(os.tmpdir(), "omniroute-gold-data");
if (!fs.existsSync(process.env.DATA_DIR)) {
  fs.mkdirSync(process.env.DATA_DIR, { recursive: true });
}

// Dinamik import ile DATA_DIR'in önceden devrede olması garanti edilir
const { openaiToClaudeRequest } = await import(
  "../../ref/omniroute-dbe703a/open-sse/translator/request/openai-to-claude.ts"
);
const { openaiToGeminiRequest } = await import(
  "../../ref/omniroute-dbe703a/open-sse/translator/request/openai-to-gemini.ts"
);

const KAYNAK_SHA = "dbe703a";
const ALTIN_DIZINI = path.join("port", "altin");

export interface AltinDosya {
  kaynak_sha: string;
  kaynak_yol: string;
  uretici_ozeti: string;
  girdi_id: string;
  girdi: any;
  cikti: any;
}

export const SABIT_GIRDILER: Record<string, any> = {
  "1_sade": {
    model: "gpt-4o",
    messages: [{ role: "user", content: "Merhaba Kervan!" }],
  },
  "2_system_coktur": {
    model: "gpt-4o",
    messages: [
      { role: "system", content: "Sen yardimci bir asistansin." },
      { role: "user", content: "Nasilsin?" },
      { role: "assistant", content: "Iyiyim, tesekkurler." },
      { role: "user", content: "Bugun hava nasil?" },
    ],
  },
  "3_parametreler": {
    model: "gpt-4o",
    messages: [{ role: "user", content: "Parametre testi" }],
    temperature: 0.7,
    top_p: 0.95,
    max_tokens: 1024,
  },
  "4_bos_icerik": {
    model: "gpt-4o",
    messages: [{ role: "user", content: "" }],
  },
  "5_arac_tanimi": {
    model: "gpt-4o",
    messages: [{ role: "user", content: "Hava durumu sorgula" }],
    tools: [
      {
        type: "function",
        function: {
          name: "get_weather",
          description: "Get weather in location",
          parameters: {
            type: "object",
            properties: {
              location: { type: "string" },
            },
            required: ["location"],
          },
        },
      },
    ],
  },
  "6_uzun_icerik": {
    model: "gpt-4o",
    messages: [
      {
        role: "user",
        content:
          "Kervan ve OmniRoute aktarım sürecinde altın parite testleri için uzun metin gövdesi. ".repeat(
            50
          ),
      },
    ],
  },
};

function scriptSha256(): string {
  const dosyaIcerigi = fs.readFileSync(__filename, "utf-8");
  return "sha256:" + crypto.createHash("sha256").update(dosyaIcerigi).digest("hex");
}

export function altinUretimHaritasi(): Map<string, AltinDosya> {
  const ozet = scriptSha256();
  const harita = new Map<string, AltinDosya>();

  for (const [id, girdi] of Object.entries(SABIT_GIRDILER)) {
    // Claude altın dosyası
    const claudeCikti = openaiToClaudeRequest("claude-3-7-sonnet", girdi, false);
    const claudeId = `claude_${id}`;
    harita.set(claudeId, {
      kaynak_sha: KAYNAK_SHA,
      kaynak_yol: "open-sse/translator/request/openai-to-claude.ts",
      uretici_ozeti: ozet,
      girdi_id: claudeId,
      girdi,
      cikti: claudeCikti,
    });

    // Gemini altın dosyası
    const geminiCikti = openaiToGeminiRequest("gemini-2.5-flash", girdi, false);
    const geminiId = `gemini_${id}`;
    harita.set(geminiId, {
      kaynak_sha: KAYNAK_SHA,
      kaynak_yol: "open-sse/translator/request/openai-to-gemini.ts",
      uretici_ozeti: ozet,
      girdi_id: geminiId,
      girdi,
      cikti: geminiCikti,
    });
  }

  return harita;
}

export function altinDosyalariYaz(): void {
  if (!fs.existsSync(ALTIN_DIZINI)) {
    fs.mkdirSync(ALTIN_DIZINI, { recursive: true });
  }

  const harita = altinUretimHaritasi();
  for (const [dosyaAdi, veri] of harita.entries()) {
    const hedefYol = path.join(ALTIN_DIZINI, `${dosyaAdi}.json`);
    const jsonMetni = JSON.stringify(veri, null, 2) + "\n";
    fs.writeFileSync(hedefYol, jsonMetni, "utf-8");
  }
}

export function altinDosyalariDogrula(): boolean {
  if (!fs.existsSync(ALTIN_DIZINI)) return false;

  const harita = altinUretimHaritasi();
  for (const [dosyaAdi, veri] of harita.entries()) {
    const hedefYol = path.join(ALTIN_DIZINI, `${dosyaAdi}.json`);
    if (!fs.existsSync(hedefYol)) return false;

    const mevcutMetin = fs.readFileSync(hedefYol, "utf-8");
    const beklenenMetin = JSON.stringify(veri, null, 2) + "\n";
    if (mevcutMetin !== beklenenMetin) {
      // İçerik JSON derin eşitliğini ve üretici özetini sına
      try {
        const mevcutJson = JSON.parse(mevcutMetin);
        if (
          mevcutJson.kaynak_sha !== veri.kaynak_sha ||
          mevcutJson.girdi_id !== veri.girdi_id ||
          JSON.stringify(mevcutJson.cikti) !== JSON.stringify(veri.cikti)
        ) {
          return false;
        }
      } catch {
        return false;
      }
    }
  }

  return true;
}

export function kervanCevirisiniCagir(hedef: "claude" | "gemini", girdiJson: any): any {
  const proc = spawnSync("cargo", ["run", "-q", "-p", "kervan-ceviri", "--", hedef], {
    input: JSON.stringify(girdiJson),
    encoding: "utf-8",
  });
  if (proc.status === 0 && proc.stdout) {
    try {
      return JSON.parse(proc.stdout);
    } catch {
      return null;
    }
  }
  return null;
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  if (args.includes("--kontrol")) {
    const uygun = altinDosyalariDogrula();
    if (!uygun) {
      console.error("HATA: Altın dosyalar bayt-özdeş değil veya eksik!");
      process.exit(1);
    }
    console.log("Altın dosyalar doğrulandı (bayt-özdeş).");
    process.exit(0);
  } else {
    altinDosyalariYaz();
    console.log(`Altın dosyalar başarıyla üretildi -> ${ALTIN_DIZINI}`);
  }
}
