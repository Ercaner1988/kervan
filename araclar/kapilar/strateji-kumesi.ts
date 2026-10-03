import fs from "fs";
import path from "path";

// PRD FR-7.1: Strateji Kümesi Eşitliği Kapısı (araclar/kapilar/strateji-kumesi.ts)
// OmniRoute v3.8.52 (dbe703a) kaynak dosyasından (src/shared/constants/routingStrategies.ts)
// küme değerlerini türetir ve Kervan'ın dışa açık listesiyle küme eşitliğini doğrular.

const OMNIROUTE_STRATEGY_FILE = path.join(
  "ref",
  "omniroute-dbe703a",
  "src",
  "shared",
  "constants",
  "routingStrategies.ts"
);

const KERVAN_ROUTER_FILE = path.join("crates", "havuz", "src", "router.rs");

export function omnirouteStratejileriniOku(dosyaYolu = OMNIROUTE_STRATEGY_FILE): {
  kamuyaAcik: string[];
  dahili: string[];
} {
  if (!fs.existsSync(dosyaYolu)) {
    throw new Error(`OmniRoute strateji dosyası bulunamadı: ${dosyaYolu}`);
  }

  const icerik = fs.readFileSync(dosyaYolu, "utf-8");

  // ROUTING_STRATEGY_VALUES = [...]
  const publicMatch = icerik.match(/ROUTING_STRATEGY_VALUES\s*=\s*\[([\s\S]*?)\]\s*as\s*const/);
  if (!publicMatch) {
    throw new Error("ROUTING_STRATEGY_VALUES kalıbı bulunamadı!");
  }
  const kamuyaAcik = Array.from(publicMatch[1].matchAll(/["']([^"']+)["']/g)).map((m) => m[1]);

  // INTERNAL_ROUTING_STRATEGY_VALUES = [...]
  const internalMatch = icerik.match(/INTERNAL_ROUTING_STRATEGY_VALUES\s*=\s*\[([\s\S]*?)\]\s*as\s*const/);
  if (!internalMatch) {
    throw new Error("INTERNAL_ROUTING_STRATEGY_VALUES kalıbı bulunamadı!");
  }
  const dahili = Array.from(internalMatch[1].matchAll(/["']([^"']+)["']/g)).map((m) => m[1]);

  return { kamuyaAcik, dahili };
}

export function kervanStratejileriniOku(dosyaYolu = KERVAN_ROUTER_FILE): {
  kamuyaAcik: string[];
  dahili: string[];
} {
  if (!fs.existsSync(dosyaYolu)) {
    throw new Error(`Kervan router dosyası bulunamadı: ${dosyaYolu}`);
  }

  const icerik = fs.readFileSync(dosyaYolu, "utf-8");

  // pub const KAMUYA_ACIK: ... = [...]
  const publicMatch = icerik.match(/pub\s+const\s+KAMUYA_ACIK\s*:[^=]+=\s*\[([\s\S]*?)\];/);
  if (!publicMatch) {
    throw new Error("Kervan KAMUYA_ACIK sabiti bulunamadı!");
  }
  const kamuyaAcik = Array.from(publicMatch[1].matchAll(/["']([^"']+)["']/g)).map((m) => m[1]);

  // pub const DAHILI: ... = [...]
  const internalMatch = icerik.match(/pub\s+const\s+DAHILI\s*:[^=]+=\s*\[([\s\S]*?)\];/);
  if (!internalMatch) {
    throw new Error("Kervan DAHILI sabiti bulunamadı!");
  }
  const dahili = Array.from(internalMatch[1].matchAll(/["']([^"']+)["']/g)).map((m) => m[1]);

  return { kamuyaAcik, dahili };
}

export function kumeEsitliginiDenetle(): {
  uygun: boolean;
  hatalar: string[];
  kamuyaAcikSayisi: number;
  dahiliSayisi: number;
} {
  const hatalar: string[] = [];
  const ref = omnirouteStratejileriniOku();
  const kervan = kervanStratejileriniOku();

  const refPublicSet = new Set(ref.kamuyaAcik);
  const kervanPublicSet = new Set(kervan.kamuyaAcik);

  // 1. Sayı ve küme eşitliği kontrolü (20 adet)
  if (kervanPublicSet.size !== 20) {
    hatalar.push(`Kervan herkese açık strateji sayısı 20 olmalıdır, bulunan: ${kervanPublicSet.size}`);
  }

  for (const s of refPublicSet) {
    if (!kervanPublicSet.has(s)) {
      hatalar.push(`OmniRoute'ta olan '${s}' stratejisi Kervan'da eksik!`);
    }
  }

  for (const s of kervanPublicSet) {
    if (!refPublicSet.has(s)) {
      hatalar.push(`Kervan'daki '${s}' stratejisi OmniRoute kamuya açık kümesinde yok!`);
    }
  }

  // 2. Özel Kısıt: fusion ve pipeline KESİNLİKLE bulunmalı
  if (!kervanPublicSet.has("fusion")) {
    hatalar.push("Kritik eksik: 'fusion' stratejisi bulunamadı!");
  }
  if (!kervanPublicSet.has("pipeline")) {
    hatalar.push("Kritik eksik: 'pipeline' stratejisi bulunamadı!");
  }

  // 3. Özel Kısıt: quota-share KESİNLİKLE kamuya açık listede olmamalı, dahili olmalı
  if (kervanPublicSet.has("quota-share")) {
    hatalar.push("İhlal: 'quota-share' dahili bir stratejidir, kamuya açık olamaz!");
  }

  const kervanDahiliSet = new Set(kervan.dahili);
  if (!kervanDahiliSet.has("quota-share")) {
    hatalar.push("İhlal: 'quota-share' dahili stratejiler listesinde bulunmalıdır!");
  }

  return {
    uygun: hatalar.length === 0,
    hatalar,
    kamuyaAcikSayisi: kervanPublicSet.size,
    dahiliSayisi: kervanDahiliSet.size,
  };
}

if (import.meta.main) {
  const sonuc = kumeEsitliginiDenetle();
  if (!sonuc.uygun) {
    console.error("❌ [strateji-kumesi] Strateji kümesi eşitlik denetimi BAŞARISIZ:");
    for (const h of sonuc.hatalar) {
      console.error(`  - ${h}`);
    }
    process.exit(1);
  }

  console.log(
    `✅ [strateji-kumesi] Kervan ve OmniRoute strateji kümeleri küme ve bayt eşittir (${sonuc.kamuyaAcikSayisi} kamuya açık, ${sonuc.dahiliSayisi} dahili).`
  );
  process.exit(0);
}
