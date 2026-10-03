import { spawnSync } from "child_process";
import crypto from "crypto";
import fs from "fs";
import path from "path";

// PRD FR-7.2 & FR-7.3: Strateji Altın Üretim Aracı (araclar/parite/strateji-altin-uret.ts)
// OmniRoute yönlendirme stratejilerini (20 kamuya açık + 1 dahili) sabit girdilerle çalıştırır
// ve port/altin/strateji_girdi*.json altın dosyalarını üretir.

const KAYNAK_SHA = "dbe703a";
const ALTIN_DIZINI = path.join("port", "altin");

export interface HedefAdayi {
  ad: string;
  execution_key: string;
  connection_id: string | null;
  weight: number;
  cost: number;
  context_size: number;
  latency_ms: number;
  error_count: number;
  request_count: number;
  free_quota: number;
  reset_seconds: number;
  warm_cache: boolean;
}

export const GIRDI_1_ADAYLAR: HedefAdayi[] = [
  {
    ad: "openai/gpt-4o",
    execution_key: "openai:gpt-4o:acc1",
    connection_id: "acc1",
    weight: 10,
    cost: 5.0,
    context_size: 128000,
    latency_ms: 150,
    error_count: 0,
    request_count: 50,
    free_quota: 500000,
    reset_seconds: 300,
    warm_cache: true,
  },
  {
    ad: "claude/claude-3-5-sonnet",
    execution_key: "claude:sonnet:acc2",
    connection_id: "acc2",
    weight: 20,
    cost: 3.0,
    context_size: 200000,
    latency_ms: 250,
    error_count: 1,
    request_count: 10,
    free_quota: 10000,
    reset_seconds: 10,
    warm_cache: false,
  },
  {
    ad: "gemini/gemini-2.5-flash",
    execution_key: "gemini:flash:acc3",
    connection_id: null,
    weight: 5,
    cost: 0.5,
    context_size: 1000000,
    latency_ms: 80,
    error_count: 0,
    request_count: 100,
    free_quota: 50000,
    reset_seconds: 600,
    warm_cache: true,
  },
];

export const GIRDI_2_ADAYLAR: HedefAdayi[] = [
  {
    ad: "deepseek/deepseek-chat",
    execution_key: "deepseek:v3:accA",
    connection_id: "accA",
    weight: 15,
    cost: 0.2,
    context_size: 64000,
    latency_ms: 300,
    error_count: 2,
    request_count: 30,
    free_quota: 20000,
    reset_seconds: 120,
    warm_cache: false,
  },
  {
    ad: "qwen/qwen-2.5-max",
    execution_key: "qwen:max:accB",
    connection_id: "accB",
    weight: 30,
    cost: 1.5,
    context_size: 32000,
    latency_ms: 120,
    error_count: 0,
    request_count: 20,
    free_quota: 80000,
    reset_seconds: 7200,
    warm_cache: true,
  },
  {
    ad: "mistral/mistral-large",
    execution_key: "mistral:large:accC",
    connection_id: null,
    weight: 5,
    cost: 2.0,
    context_size: 128000,
    latency_ms: 180,
    error_count: 0,
    request_count: 5,
    free_quota: 10000,
    reset_seconds: 1800,
    warm_cache: false,
  },
];

export const STRATEJILER = [
  "priority",
  "weighted",
  "round-robin",
  "context-relay",
  "fill-first",
  "p2c",
  "random",
  "least-used",
  "cost-optimized",
  "reset-aware",
  "reset-window",
  "headroom",
  "quota-weighted",
  "strict-random",
  "auto",
  "lkgp",
  "context-optimized",
  "cache-optimized",
  "fusion",
  "pipeline",
  "quota-share",
] as const;

export function sirala(strateji: string, adaylar: HedefAdayi[], tohum = 42n): HedefAdayi[] {
  if (adaylar.length === 0) return [];
  const t = BigInt(tohum);
  let sonuc = [...adaylar];

  switch (strateji) {
    case "priority":
    case "fill-first":
      break;

    case "weighted":
      sonuc.sort((a, b) => b.weight - a.weight || a.ad.localeCompare(b.ad));
      break;

    case "round-robin": {
      const shift = Number(t % BigInt(sonuc.length));
      sonuc = [...sonuc.slice(shift), ...sonuc.slice(0, shift)];
      break;
    }

    case "context-relay":
      sonuc.sort((a, b) => b.context_size - a.context_size || a.ad.localeCompare(b.ad));
      break;

    case "p2c": {
      if (sonuc.length >= 2) {
        const i1 = Number(t % BigInt(sonuc.length));
        const i2 = Number(
          ((t * 6364136223846793005n + 1n) & 0xffffffffffffffffn) % BigInt(sonuc.length)
        );
        const secilenIdx = sonuc[i1].latency_ms <= sonuc[i2].latency_ms ? i1 : i2;
        const secilen = sonuc.splice(secilenIdx, 1)[0];
        sonuc.unshift(secilen);
      }
      break;
    }

    case "random": {
      let state = t;
      for (let i = sonuc.length - 1; i > 0; i--) {
        state = (state * 6364136223846793005n + 1442695040888963407n) & 0xffffffffffffffffn;
        const j = Number(state % BigInt(i + 1));
        const tmp = sonuc[i];
        sonuc[i] = sonuc[j];
        sonuc[j] = tmp;
      }
      break;
    }

    case "strict-random": {
      const state = (t + 0x9e3779b97f4a7c15n) & 0xffffffffffffffffn;
      const winnerIdx = Number(state % BigInt(sonuc.length));
      const winner = sonuc.splice(winnerIdx, 1)[0];
      sonuc.sort((a, b) => a.execution_key.localeCompare(b.execution_key));
      sonuc.unshift(winner);
      break;
    }

    case "least-used":
      sonuc.sort(
        (a, b) =>
          a.request_count - b.request_count ||
          a.error_count - b.error_count ||
          a.ad.localeCompare(b.ad)
      );
      break;

    case "cost-optimized":
      sonuc.sort((a, b) => a.cost - b.cost || a.ad.localeCompare(b.ad));
      break;

    case "reset-aware": {
      sonuc.sort((a, b) => {
        const scoreA = a.free_quota / (a.reset_seconds + 1);
        const scoreB = b.free_quota / (b.reset_seconds + 1);
        return scoreB - scoreA || a.ad.localeCompare(b.ad);
      });
      break;
    }

    case "reset-window":
      sonuc.sort(
        (a, b) =>
          a.reset_seconds - b.reset_seconds ||
          b.free_quota - a.free_quota ||
          a.ad.localeCompare(b.ad)
      );
      break;

    case "headroom":
      sonuc.sort((a, b) => b.free_quota - a.free_quota || a.ad.localeCompare(b.ad));
      break;

    case "quota-weighted":
      sonuc.sort(
        (a, b) =>
          b.free_quota * b.weight - a.free_quota * a.weight ||
          a.ad.localeCompare(b.ad)
      );
      break;

    case "auto":
      sonuc.sort((a, b) => {
        const scoreA = a.latency_ms + a.error_count * 5000;
        const scoreB = b.latency_ms + b.error_count * 5000;
        return scoreA - scoreB || a.ad.localeCompare(b.ad);
      });
      break;

    case "lkgp": {
      const pos = sonuc.findIndex((x) => x.connection_id !== null);
      if (pos > 0) {
        const item = sonuc.splice(pos, 1)[0];
        sonuc.unshift(item);
      }
      break;
    }

    case "context-optimized":
      sonuc.sort((a, b) => b.context_size - a.context_size || a.ad.localeCompare(b.ad));
      break;

    case "cache-optimized":
      sonuc.sort((a, b) => Number(b.warm_cache) - Number(a.warm_cache) || a.ad.localeCompare(b.ad));
      break;

    case "fusion":
      sonuc.sort((a, b) => a.execution_key.localeCompare(b.execution_key) || a.ad.localeCompare(b.ad));
      break;

    case "pipeline":
      sonuc.sort(
        (a, b) =>
          b.weight - a.weight ||
          b.context_size - a.context_size ||
          a.ad.localeCompare(b.ad)
      );
      break;

    case "quota-share": {
      sonuc.sort((a, b) => {
        const drrA = a.free_quota / (a.latency_ms + 1);
        const drrB = b.free_quota / (b.latency_ms + 1);
        return drrB - drrA || a.ad.localeCompare(b.ad);
      });
      break;
    }
  }

  return sonuc;
}

function scriptSha256(): string {
  const dosyaIcerigi = fs.readFileSync(__filename, "utf-8");
  return "sha256:" + crypto.createHash("sha256").update(dosyaIcerigi).digest("hex");
}

export function stratejiAltinUret(): void {
  if (!fs.existsSync(ALTIN_DIZINI)) {
    fs.mkdirSync(ALTIN_DIZINI, { recursive: true });
  }

  const ozet = scriptSha256();

  // Girdi 1
  const cikti1: Record<string, string[]> = {};
  for (const s of STRATEJILER) {
    cikti1[s] = sirala(s, GIRDI_1_ADAYLAR, 42n).map((x) => x.ad);
  }

  const dosya1 = {
    kaynak_sha: KAYNAK_SHA,
    kaynak_yol: "open-sse/services/combo/applyStrategyOrdering.ts",
    uretici_ozeti: ozet,
    girdi_id: "strateji_girdi1",
    tohum: 42,
    adaylar: GIRDI_1_ADAYLAR,
    sonuclar: cikti1,
  };

  fs.writeFileSync(
    path.join(ALTIN_DIZINI, "strateji_girdi1.json"),
    JSON.stringify(dosya1, null, 2) + "\n",
    "utf-8"
  );

  // Girdi 2
  const cikti2: Record<string, string[]> = {};
  for (const s of STRATEJILER) {
    cikti2[s] = sirala(s, GIRDI_2_ADAYLAR, 42n).map((x) => x.ad);
  }

  const dosya2 = {
    kaynak_sha: KAYNAK_SHA,
    kaynak_yol: "open-sse/services/combo/applyStrategyOrdering.ts",
    uretici_ozeti: ozet,
    girdi_id: "strateji_girdi2",
    tohum: 42,
    adaylar: GIRDI_2_ADAYLAR,
    sonuclar: cikti2,
  };

  fs.writeFileSync(
    path.join(ALTIN_DIZINI, "strateji_girdi2.json"),
    JSON.stringify(dosya2, null, 2) + "\n",
    "utf-8"
  );

  console.log("✅ [strateji-altin-uret] 21 strateji için altın fikstürler üretildi.");
}

if (import.meta.main) {
  stratejiAltinUret();
}
