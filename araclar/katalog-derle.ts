import { REGISTRY } from "../ref/omniroute-dbe703a/open-sse/config/providers/index.ts";
import * as fs from "node:fs";
import * as path from "node:path";

interface ModelOzeti {
  id: string;
  ad: string;
  baglam: number;
  dusunme: boolean;
  gorsel: boolean;
}

interface SaglayiciOzeti {
  id: string;
  ad: string;
  format: string;
  taban_url: string;
  url_son_ek: string;
  auth_tipi: string;
  auth_basligi: string;
  varsayilan_baglam: number;
  basliklar: Record<string, string>;
  modeller: ModelOzeti[];
}

const ciktiDizini = path.resolve(import.meta.dir, "../port/katalog");
if (!fs.existsSync(ciktiDizini)) {
  fs.mkdirSync(ciktiDizini, { recursive: true });
}

const saglayicilar: SaglayiciOzeti[] = [];

for (const [key, p] of Object.entries(REGISTRY)) {
  const modeller: ModelOzeti[] = (p.models || []).map((m: any) => ({
    id: m.id || "",
    ad: m.name || m.id || "",
    baglam: m.contextLength || p.defaultContextLength || 8192,
    dusunme: Boolean(m.supportsReasoning),
    gorsel: Boolean(m.supportsVision),
  }));

  saglayicilar.push({
    id: p.id || key,
    ad: p.alias || p.id || key,
    format: p.format || "openai",
    taban_url: p.baseUrl || "",
    url_son_ek: p.urlSuffix || "",
    auth_tipi: p.authType || "apikey",
    auth_basligi: p.authHeader || "Authorization",
    varsayilan_baglam: p.defaultContextLength || 8192,
    basliklar: (p.headers as Record<string, string>) || {},
    modeller,
  });
}

// Deterministic sort by ID
saglayicilar.sort((a, b) => a.id.localeCompare(b.id));

const hedefDosya = path.join(ciktiDizini, "saglayicilar.json");
fs.writeFileSync(hedefDosya, JSON.stringify(saglayicilar, null, 2), "utf-8");

console.log(`✅ Katalog başarıyla derlendi: ${saglayicilar.length} sağlayıcı kaydedildi.`);
console.log(`Dosya: ${hedefDosya}`);
