import crypto from "crypto";
import fs from "fs";
import os from "os";
import path from "path";

// PRD FR-6.3: akış (SSE) yanıt çevirisi altınları GERÇEK OmniRoute kodundan (ref dbe703a) üretilir.
// Her senaryo, yukarı akış olay dizisini OmniRoute'un kendi giriş noktasından
// (translator/index.ts translateResponse + initState) geçirir; saat sabitlenir (Date.now).
// Kapsam: metin akışları. Düşünme blokları, araç çağrıları ve önbellek ayrıntıları kapsam dışı
// (araçlar Kervan'da açık hatayla reddedilir; sapmalar.toml).
// Kullanım: bun araclar/uretec/akis-altin-uret.ts [--kontrol]

const KAYNAK_SHA = "dbe703a";
const REF = path.resolve(import.meta.dir, "../../ref/omniroute-dbe703a");
const ALTIN_DIZINI = path.resolve(import.meta.dir, "../../port/altin");
const SABIT_SAAT_MS = 1700000000000;

process.env.DATA_DIR = fs.mkdtempSync(path.join(os.tmpdir(), "kervan-akis-altin-"));
process.env.DISABLE_SQLITE_AUTO_BACKUP = "true";
process.env.APP_LOG_TO_FILE = "false";
Date.now = () => SABIT_SAAT_MS;

await import(path.join(REF, "open-sse/translator/bootstrap.ts"));
const { translateResponse, initState } = await import(path.join(REF, "open-sse/translator/index.ts"));

export const KAYNAK_YOLLAR = [
  "open-sse/translator/index.ts",
  "open-sse/translator/response/claude-to-openai.ts",
  "open-sse/translator/response/gemini-to-openai.ts",
  "open-sse/utils/finishReason.ts",
];

const baslangic = (id: string, model: string, girdi: number) => ({
  type: "message_start",
  message: { id, type: "message", role: "assistant", content: [], model, usage: { input_tokens: girdi, output_tokens: 0 } },
});
const blokBas = { type: "content_block_start", index: 0, content_block: { type: "text", text: "" } };
const metin = (t: string) => ({ type: "content_block_delta", index: 0, delta: { type: "text_delta", text: t } });
const blokSon = { type: "content_block_stop", index: 0 };
const bitis = (neden: string, cikti: number) => ({ type: "message_delta", delta: { stop_reason: neden }, usage: { output_tokens: cikti } });
const dur = { type: "message_stop" };

const gAday = (t: string | null, neden?: string, kullanim?: [number, number]) => ({
  candidates: [{ content: { role: "model", parts: t === null ? [] : [{ text: t }] }, ...(neden ? { finishReason: neden } : {}) }],
  ...(kullanim ? { usageMetadata: { promptTokenCount: kullanim[0], candidatesTokenCount: kullanim[1], totalTokenCount: kullanim[0] + kullanim[1] } } : {}),
});

export const SENARYOLAR: { id: string; bicim: "claude" | "gemini"; olaylar: unknown[] }[] = [
  { id: "akis_claude_1_sade", bicim: "claude", olaylar: [baslangic("msg_1", "claude-x", 5), blokBas, metin("Merhaba"), metin(" dünya"), blokSon, bitis("end_turn", 3), dur] },
  { id: "akis_claude_2_uzunluk", bicim: "claude", olaylar: [baslangic("msg_2", "claude-y", 12), blokBas, metin("Kesil"), blokSon, bitis("max_tokens", 1), dur] },
  { id: "akis_claude_3_ret", bicim: "claude", olaylar: [baslangic("msg_3", "claude-z", 4), bitis("refusal", 0), dur] },
  { id: "akis_claude_4_durdurma_dizisi", bicim: "claude", olaylar: [baslangic("msg_4", "claude-x", 7), blokBas, metin("Bitti:"), blokSon, bitis("stop_sequence", 2), dur] },
  { id: "akis_gemini_1_sade", bicim: "gemini", olaylar: [gAday("Merhaba"), gAday(" dünya", "STOP", [5, 3])] },
  { id: "akis_gemini_2_uzunluk", bicim: "gemini", olaylar: [gAday("Kesil"), gAday(null, "MAX_TOKENS", [9, 1])] },
  { id: "akis_gemini_3_guvenlik", bicim: "gemini", olaylar: [gAday("Bu"), gAday(null, "SAFETY", [4, 1])] },
  { id: "akis_gemini_4_tek_olay", bicim: "gemini", olaylar: [gAday("Tek parça", "STOP", [2, 2])] },
  { id: "akis_gemini_5_kimlikli", bicim: "gemini", olaylar: [{ ...gAday("Kimlik"), responseId: "yanit-9", modelVersion: "gemini-2.5-flash" }, gAday(null, "STOP", [3, 1])] },
];

function ureticiOzeti(): string {
  const icerik = fs.readFileSync(import.meta.path, "utf-8").replace(/\r\n/g, "\n");
  return "sha256:" + crypto.createHash("sha256").update(icerik).digest("hex");
}

/** Her olayın OmniRoute çıktısı (parça listesi) ve akış sonu boşaltması (null olay). */
export function senaryoyuUret(s: (typeof SENARYOLAR)[number]) {
  const durum = initState("openai");
  const parcalar = s.olaylar.map((o) => translateResponse(s.bicim, "openai", o, durum) ?? []);
  const bosaltma = translateResponse(s.bicim, "openai", null, durum) ?? [];
  return { parcalar, bosaltma };
}

export function akisAltinlariniUret(): Map<string, string> {
  const ozet = ureticiOzeti();
  const dosyalar = new Map<string, string>();
  for (const s of SENARYOLAR) {
    const { parcalar, bosaltma } = senaryoyuUret(s);
    const dosya = {
      kaynak_sha: KAYNAK_SHA,
      kaynak_yollar: KAYNAK_YOLLAR,
      uretici_ozeti: ozet,
      girdi_id: s.id,
      bicim: s.bicim,
      sabit_saat_ms: SABIT_SAAT_MS,
      olaylar: s.olaylar,
      parcalar,
      bosaltma,
    };
    dosyalar.set(path.join(ALTIN_DIZINI, `${s.id}.json`), JSON.stringify(dosya, null, 2) + "\n");
  }
  return dosyalar;
}

if (import.meta.main) {
  const dosyalar = akisAltinlariniUret();
  if (process.argv.includes("--kontrol")) {
    const farkli = [...dosyalar].filter(([y, icerik]) => !fs.existsSync(y) || fs.readFileSync(y, "utf-8").replace(/\r\n/g, "\n") !== icerik);
    for (const [y] of farkli) console.error(`farklı ya da eksik: ${path.relative(process.cwd(), y)}`);
    console.log(farkli.length ? `HUKUM: kirmizi ${farkli.length} akış altını üreticiyle bayt-özdeş değil` : "HUKUM: yesil");
    process.exit(farkli.length ? 1 : 0);
  }
  for (const [y, icerik] of dosyalar) fs.writeFileSync(y, icerik, "utf-8");
  console.log(`akış altınları üretildi: ${dosyalar.size} dosya`);
  process.exit(0);
}
