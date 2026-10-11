import crypto from "crypto";
import fs from "fs";
import os from "os";
import path from "path";

// PRD FR-7.2: strateji altınları GERÇEK OmniRoute kodundan (ref dbe703a) üretilir.
// Durum (fiyat, bağlam sınırı, istek kayıtları, LKGP) yalıtılmış geçici DATA_DIR'deki gerçek
// DB'ye OmniRoute'un kendi yazıcılarıyla yazılır, geri okunup doğrulanır; rastgelelik
// OmniRoute'un test kancası _setSecureRandomFloatSource ile altına yazılan akıştan gelir.
// Altın, "strateji çekirdeği" sırasıdır (applyStrategyOrdering + weighted/round-robin
// çekirdekleri); priority ön taraması, süreklilik filtreleri gibi çekirdek dışı adımlar kapsam dışı.
// Kullanım: bun araclar/parite/strateji-altin-uret.ts [--kontrol]

const KAYNAK_SHA = "dbe703a";
const REF = path.resolve(import.meta.dir, "../../ref/omniroute-dbe703a");
const ALTIN_DIZINI = path.resolve(import.meta.dir, "../../port/altin");

// Canlı OmniRoute ve ~/.omniroute'a ASLA dokunulmaz: her koşu kendi boş dizininde.
process.env.DATA_DIR = fs.mkdtempSync(path.join(os.tmpdir(), "kervan-strateji-altin-"));
process.env.DISABLE_SQLITE_AUTO_BACKUP = "true";
process.env.APP_LOG_TO_FILE = "false";

const yukle = (y: string) => import(path.join(REF, y));
const { applyStrategyOrdering } = await yukle("open-sse/services/combo/applyStrategyOrdering.ts");
const { selectWeightedTarget, orderTargetsForWeightedFallback } = await yukle("open-sse/services/combo/targetSorters.ts");
const { rrCounters, rrStickyTargets, getStickyRoundRobinStartIndex } = await yukle("open-sse/services/combo/rrState.ts");
const { getModelContextLimitForModelString } = await yukle("open-sse/services/combo/comboStructure.ts");
const { recordComboRequest, resetAllComboMetrics } = await yukle("open-sse/services/comboMetrics.ts");
const { parseModel } = await yukle("open-sse/services/model.ts");
const { _setSecureRandomFloatSource } = await yukle("src/shared/utils/secureRandom.ts");
const { _resetAllDecks } = await yukle("src/shared/utils/shuffleDeck.ts");
const { normalizeRoutingStrategy } = await yukle("src/shared/constants/routingStrategies.ts");
const { updatePricing, getPricingForModel, setLKGP } = await yukle("src/lib/db/settings.ts");
const { setModelContextOverride } = await yukle("src/lib/db/modelContextOverrides.ts");
const { getComboStepWeight } = await yukle("src/lib/combos/steps.ts");

export const KAYNAK_YOLLAR = [
  "open-sse/services/combo/applyStrategyOrdering.ts",
  "open-sse/services/combo/targetSorters.ts",
  "open-sse/services/combo/rrState.ts",
  "open-sse/services/combo/roundRobinCombo.ts",
  "src/shared/utils/shuffleDeck.ts",
  "src/shared/constants/routingStrategies.ts",
  "src/lib/combos/steps.ts",
  "open-sse/services/comboMetrics.ts",
  "src/lib/db/settings/pricing.ts",
];

/** applyStrategyOrdering'den geçen ve altını üretilen stratejiler (adım sayısı = 1). */
const CEKIRDEK = ["priority", "fill-first", "context-relay", "p2c", "random", "least-used", "cost-optimized", "lkgp", "context-optimized"];

export interface Hedef { model_str: string; provider: string; execution_key: string; connection_id: string | null; weight: number }
export interface IstekKaydi { model_str: string; execution_key: string; basarili: boolean; gecikme_ms: number }
export interface Girdi {
  girdi_id: string;
  hedefler: Hedef[];
  durum: {
    fiyat: Record<string, number>;
    baglam: Record<string, number>;
    kayitlar: IstekKaydi[];
    lkgp: { provider: string; connection_id: string | null } | null;
  };
}

const h = (model_str: string, execution_key: string, connection_id: string | null, weight: number): Hedef =>
  ({ model_str, provider: model_str.split("/")[0], execution_key, connection_id, weight });
const k = (model_str: string, execution_key: string, basarili: boolean, gecikme_ms: number, kez = 1): IstekKaydi[] =>
  Array.from({ length: kez }, () => ({ model_str, execution_key, basarili, gecikme_ms }));

export const GIRDILER: Girdi[] = [
  {
    // Ayrık değerler: her strateji farklı bir kazanan seçer.
    girdi_id: "strateji_girdi1",
    hedefler: [
      h("kva/model-hizli", "kva/model-hizli#h1", "h1", 10),
      h("kvb/model-orta", "kvb/model-orta#h2", "h2", 20),
      h("kvc/model-genis", "kvc/model-genis", null, 5),
    ],
    durum: {
      fiyat: { "kva/model-hizli": 5, "kvb/model-orta": 3, "kvc/model-genis": 0.5 },
      baglam: { "kva/model-hizli": 128000, "kvb/model-orta": 200000, "kvc/model-genis": 1000000 },
      kayitlar: [
        ...k("kva/model-hizli", "kva/model-hizli#h1", true, 150, 3),
        ...k("kvb/model-orta", "kvb/model-orta#h2", true, 250),
        ...k("kvb/model-orta", "kvb/model-orta#h2", false, 400),
        ...k("kvc/model-genis", "kvc/model-genis", true, 80, 5),
      ],
      lkgp: { provider: "kvb", connection_id: "h2" },
    },
  },
  {
    // Eşitlikler: aynı model iki hesapta, fiyatsız model (∞), eşit fiyat/bağlam/kullanım/ağırlık.
    girdi_id: "strateji_girdi2",
    hedefler: [
      h("kvd/model-ortak", "kvd/model-ortak#d1", "d1", 10),
      h("kve/model-fiyatsiz", "kve/model-fiyatsiz#e1", "e1", 10),
      h("kvd/model-ortak", "kvd/model-ortak#d2", "d2", 30),
      h("kvf/model-esit", "kvf/model-esit", null, 0),
      h("kvg/model-ucuz", "kvg/model-ucuz#g1", "g1", 30),
    ],
    durum: {
      fiyat: { "kvd/model-ortak": 1, "kvf/model-esit": 1, "kvg/model-ucuz": 0.25 },
      baglam: { "kvd/model-ortak": 64000, "kvf/model-esit": 64000, "kvg/model-ucuz": 32000 },
      kayitlar: [
        ...k("kvd/model-ortak", "kvd/model-ortak#d2", true, 300, 2),
        // p2c: başarı ile gecikme çelişir (f hızlı ama hep hatalı; g yarı başarılı, yavaş).
        ...k("kvf/model-esit", "kvf/model-esit", false, 50, 2),
        ...k("kvg/model-ucuz", "kvg/model-ucuz#g1", true, 100),
        ...k("kvg/model-ucuz", "kvg/model-ucuz#g1", false, 900),
      ],
      lkgp: { provider: "kvd", connection_id: "d2" },
    },
  },
  {
    // Boş durum: fiyat/bağlam/kayıt yok, ağırlıklar 0 (eşit olasılıklı seçim yolu);
    // LKGP bağlantısı hiçbir hedefle eşleşmez → sağlayıcı eşleşmesine düşülür.
    girdi_id: "strateji_girdi3",
    hedefler: [
      h("kvh/model-1", "kvh/model-1", null, 0),
      h("kvi/model-2", "kvi/model-2#i1", "i1", 0),
      h("kvj/model-3", "kvj/model-3", null, 0),
    ],
    durum: { fiyat: {}, baglam: {}, kayitlar: [], lkgp: { provider: "kvj", connection_id: "olmayan" } },
  },
  {
    // Kenarlar: ağırlık kırpma (150, -200), aynı execution_key iki hedefte, fiyat anahtarı
    // büyük harf + "-" (hedef "."), boş execution_key/model_str kayıtları.
    girdi_id: "strateji_girdi4",
    hedefler: [
      h("kvk/model.1", "kvk/model.1#k1", "k1", 150),
      h("kvl/model-2", "ortak", "l1", -200),
      h("kvm/model-3", "ortak", "m1", 20),
      h("kvn/model-4", "kvn/model-4", null, 20),
    ],
    durum: {
      fiyat: { "kvk/Model-1": 2, "kvn/model-4": 1, "kvl/model-2": 3 },
      baglam: {},
      kayitlar: [
        ...k("kvk/model.1", "kvk/model.1#k1", true, 10),
        ...k("kvn/model-4", "", true, 10, 3),
        ...k("", "kvk/model.1#k1", true, 10, 4),
      ],
      lkgp: null,
    },
  },
];

export const TAKMA_AD_GIRDILERI = [
  "usage", "context", "weekly-reset", "reset-window-order", "  Priority ", "ROUND-ROBIN",
  "roundrobin", "quota-share", "bilinmeyen", "", "fusion", "least-used",
  "LKGP", "round-robin\u0085", "﻿random", " p2c　",
];

/** Girdiye özgü sabit rastgele akış: 48 bitlik tamsayılar; kayan = n / 2^48 (iki tarafta kesin). */
function rastgeleAkis(girdiId: string, adet = 96): number[] {
  const akis: number[] = [];
  for (let i = 0; akis.length < adet; i++) {
    const b = crypto.createHash("sha256").update(`${girdiId}:${i}`).digest();
    for (let j = 0; j + 6 <= b.length && akis.length < adet; j += 6) akis.push(b.readUIntBE(j, 6));
  }
  return akis;
}

/** Akışı OmniRoute'a bağlar; tükenirse hata (sessiz döngü yok). Tüketilen adedi döner. */
function akisiBagla(akis: number[]): () => number {
  let i = 0;
  _setSecureRandomFloatSource(() => {
    if (i >= akis.length) throw new Error("rastgele akış tükendi");
    return akis[i++] / 2 ** 48;
  });
  return () => i;
}

const sessiz = { info() {}, warn() {}, debug() {}, error() {} };

async function durumuYaz(g: Girdi): Promise<void> {
  const fiyatlar: Record<string, Record<string, { input: number }>> = {};
  for (const [m, input] of Object.entries(g.durum.fiyat)) {
    const p = parseModel(m);
    (fiyatlar[p.provider || p.providerAlias] ??= {})[p.model] = { input };
  }
  if (Object.keys(fiyatlar).length) await updatePricing(fiyatlar);
  for (const [m, c] of Object.entries(g.durum.baglam)) {
    const p = parseModel(m);
    if (!setModelContextOverride(p.provider || p.providerAlias, p.model, c)) throw new Error(`bağlam yazılamadı: ${m}`);
  }
  for (const kayit of g.durum.kayitlar) {
    const hedef = g.hedefler.find((x) => x.execution_key === kayit.execution_key);
    recordComboRequest(g.girdi_id, kayit.model_str, {
      success: kayit.basarili,
      latencyMs: kayit.gecikme_ms,
      target: { executionKey: kayit.execution_key, provider: hedef?.provider ?? null, connectionId: hedef?.connection_id ?? null },
    });
  }
  if (g.durum.lkgp) await setLKGP(g.girdi_id, g.girdi_id, g.durum.lkgp.provider, g.durum.lkgp.connection_id ?? undefined);

  // Geri okuma: stratejilerin kullandığı okuyucular yazılanı görmeli (fail-closed).
  for (const x of g.hedefler) {
    const p = parseModel(x.model_str);
    const fiyat = Number((await getPricingForModel(p.provider || p.providerAlias, p.model))?.input);
    // Tam anahtar yoksa büyük/küçük harf ve "." → "-" eşdeğeri aranır (getPricingForModel kuralı).
    const kucuk = x.model_str.toLowerCase();
    const anahtar = Object.keys(g.durum.fiyat).find((a) => a === x.model_str)
      ?? Object.keys(g.durum.fiyat).find((a) => a.toLowerCase() === kucuk || a.toLowerCase() === kucuk.replace(/\./g, "-"));
    const beklenenFiyat = anahtar === undefined ? undefined : g.durum.fiyat[anahtar];
    if (beklenenFiyat === undefined ? Number.isFinite(fiyat) : fiyat !== beklenenFiyat)
      throw new Error(`fiyat geri okunamadı: ${x.model_str} → ${fiyat}`);
    const baglam = getModelContextLimitForModelString(x.model_str);
    if ((baglam ?? undefined) !== g.durum.baglam[x.model_str]) throw new Error(`bağlam geri okunamadı: ${x.model_str} → ${baglam}`);
  }
}

// Ağırlık, combo adımı ayrıştırılırken OmniRoute'un kendi toWeight'inden geçer (kırpma).
const hedefNesnesi = (x: Hedef) => ({ kind: "model", modelStr: x.model_str, provider: x.provider, executionKey: x.execution_key, connectionId: x.connection_id, weight: getComboStepWeight({ weight: x.weight }) });
// Kimlik = execution_key@model_str: aynı anahtarlı hedefler de ayırt edilir.
const anahtarlar = (l: Array<{ executionKey: string; modelStr: string }>) => l.map((t) => `${t.executionKey}@${t.modelStr}`);

export async function girdiyiUret(g: Girdi) {
  await durumuYaz(g);
  const akis = rastgeleAkis(g.girdi_id);
  const combo = { name: g.girdi_id };
  const sonuclar: Record<string, string[][]> = {};
  const tuketim: Record<string, number> = {};
  const n = g.hedefler.length;

  const calistir = async (strateji: string, adim: number) => {
    _resetAllDecks();
    const sayac = akisiBagla(akis);
    const adimlar: string[][] = [];
    for (let i = 0; i < adim; i++) {
      const r = await applyStrategyOrdering(strateji, g.hedefler.map(hedefNesnesi), { combo, config: {}, body: {}, log: sessiz, apiKeyAllowedConnections: null });
      adimlar.push(anahtarlar(r.orderedTargets));
    }
    sonuclar[strateji] = adimlar;
    tuketim[strateji] = sayac();
  };
  // p2c ve random her adımda akıştan çeker: farklı çiftler/karışımlar için çok adım.
  for (const s of CEKIRDEK) await calistir(s, s === "p2c" ? 8 : s === "random" ? 3 : 1);
  // strict-random: deste döngü sınırını (son ≠ sonraki ilk) aşan ardışık çağrılar.
  await calistir("strict-random", 2 * n + 1);

  // weighted çekirdeği: targetResolution → comboStructure.resolveWeightedTargets ile aynı iki çağrı.
  {
    const sayac = akisiBagla(akis);
    const adimlar: string[][] = [];
    for (let i = 0; i < 8; i++) {
      const hedefler = g.hedefler.map(hedefNesnesi);
      const secilen = selectWeightedTarget(hedefler);
      adimlar.push(anahtarlar(orderTargetsForWeightedFallback(hedefler, secilen.executionKey, false)));
    }
    sonuclar["weighted"] = adimlar;
    tuketim["weighted"] = sayac();
  }

  // round-robin çekirdeği: başlangıç getStickyRoundRobinStartIndex (yapışkanlık sınırı 1),
  // sıra roundRobinCombo.ts döngüsündeki (başlangıç + ofset) % n.
  {
    const adimlar: string[][] = [];
    for (let sayac = 0; sayac <= n; sayac++) {
      rrCounters.set(g.girdi_id, sayac);
      rrStickyTargets.delete(g.girdi_id);
      const hedefler = g.hedefler.map(hedefNesnesi);
      const { startIndex } = getStickyRoundRobinStartIndex(g.girdi_id, hedefler, 1);
      adimlar.push(anahtarlar(hedefler.map((_, ofset) => hedefler[(startIndex + ofset) % n])));
    }
    sonuclar["round-robin"] = adimlar;
    tuketim["round-robin"] = 0;
  }
  _setSecureRandomFloatSource(null);
  return { akis, sonuclar, tuketim };
}

function ureticiOzeti(): string {
  const icerik = fs.readFileSync(import.meta.path, "utf-8").replace(/\r\n/g, "\n");
  return "sha256:" + crypto.createHash("sha256").update(icerik).digest("hex");
}

export async function stratejiAltinlariniUret(): Promise<Map<string, string>> {
  resetAllComboMetrics();
  const ozet = ureticiOzeti();
  const dosyalar = new Map<string, string>();
  for (const g of GIRDILER) {
    const { akis, sonuclar, tuketim } = await girdiyiUret(g);
    const takma_adlar = Object.fromEntries(TAKMA_AD_GIRDILERI.map((a) => [a, normalizeRoutingStrategy(a)]));
    const dosya = {
      kaynak_sha: KAYNAK_SHA,
      kaynak_yollar: KAYNAK_YOLLAR,
      uretici_ozeti: ozet,
      girdi_id: g.girdi_id,
      hedefler: g.hedefler,
      durum: g.durum,
      rastgele_48: akis,
      takma_adlar,
      tuketim,
      sonuclar,
    };
    dosyalar.set(path.join(ALTIN_DIZINI, `${g.girdi_id}.json`), JSON.stringify(dosya, null, 2) + "\n");
  }
  return dosyalar;
}

if (import.meta.main) {
  const dosyalar = await stratejiAltinlariniUret();
  if (process.argv.includes("--kontrol")) {
    const farkli = [...dosyalar].filter(([y, icerik]) => !fs.existsSync(y) || fs.readFileSync(y, "utf-8").replace(/\r\n/g, "\n") !== icerik);
    for (const [y] of farkli) console.error(`farklı: ${path.relative(process.cwd(), y)}`);
    console.log(farkli.length ? `HUKUM: kirmizi ${farkli.length} strateji altını üreticiyle eşleşmiyor` : "HUKUM: yesil");
    process.exit(farkli.length ? 1 : 0);
  }
  fs.mkdirSync(ALTIN_DIZINI, { recursive: true });
  for (const [y, icerik] of dosyalar) fs.writeFileSync(y, icerik, "utf-8");
  console.log(`strateji altınları üretildi: ${dosyalar.size} dosya`);
  process.exit(0);
}
