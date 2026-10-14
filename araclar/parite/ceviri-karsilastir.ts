// PRD FR-5.2: çeviri paritesi karşılaştırıcısı.
// Her port/altin/{claude,gemini}_*.json altınının girdisini GERÇEK Kervan çeviri ikilisine
// (kervan-ceviri) stdin'den verir, stdout'u altının çıktısıyla alan alan karşılaştırır.
// Sapma kayıtlı girdiler (araç tanımı, çok parçalı içerik; port/altin/sapmalar.toml) burada
// koşulmaz: Kervan onlarda bilerek hata verir, bunu p6 cargo testi ölçer.
// Kullanım: bun araclar/parite/ceviri-karsilastir.ts   (çıkış 0 = birebir)
import fs from "fs";
import path from "path";

const ALTIN = path.join("port", "altin");

type Altin = { girdi_id: string; girdi: any; cikti: unknown };

const sapmali = (g: any) =>
  Array.isArray(g?.tools) || (g?.messages ?? []).some((m: any) => Array.isArray(m?.content));

/** İki JSON değerinin farklı yapraklarını "yol: kervan=… altın=…" biçiminde döner. */
function farklar(k: unknown, a: unknown, yol = ""): string[] {
  const nesne = (v: unknown) => v !== null && typeof v === "object";
  if (nesne(k) && nesne(a) && Array.isArray(k) === Array.isArray(a)) {
    const anahtarlar = new Set([...Object.keys(k as object), ...Object.keys(a as object)]);
    return [...anahtarlar].flatMap((x) => farklar((k as any)[x], (a as any)[x], `${yol}/${x}`));
  }
  return JSON.stringify(k) === JSON.stringify(a) ? [] : [`${yol || "/"}: kervan=${JSON.stringify(k)} altın=${JSON.stringify(a)}`];
}

const altinlar: [string, Altin][] = fs
  .readdirSync(ALTIN)
  .filter((d) => /^(claude|gemini)_.*\.json$/.test(d))
  .sort()
  .map((d) => [d, JSON.parse(fs.readFileSync(path.join(ALTIN, d), "utf-8"))]);
if (altinlar.length === 0) {
  console.log("HUKUM: kirmizi port/altin altında çeviri altını yok");
  process.exit(1);
}

let hata = 0;
let olculen = 0;
for (const [dosya, altin] of altinlar) {
  if (sapmali(altin.girdi)) continue;
  const hedef = dosya.startsWith("claude_") ? "claude" : "gemini";
  const r = Bun.spawnSync(["cargo", "run", "-q", "-p", "kervan-ceviri", "--", hedef], {
    stdin: Buffer.from(JSON.stringify(altin.girdi)),
    stdout: "pipe",
    stderr: "pipe",
  });
  olculen++;
  if (r.exitCode !== 0) {
    hata++;
    console.error(`${dosya}: kervan-ceviri çıkış ${r.exitCode}: ${r.stderr.toString().trim().slice(0, 400)}`);
    continue;
  }
  let kervan: unknown;
  try {
    kervan = JSON.parse(r.stdout.toString());
  } catch (e: any) {
    hata++;
    console.error(`${dosya}: kervan-ceviri çıktısı JSON değil: ${e.message}`);
    continue;
  }
  const f = farklar(kervan, altin.cikti);
  if (f.length) {
    hata++;
    for (const satir of f.slice(0, 10)) console.error(`${dosya}: ${satir}`);
  }
}

console.log(
  hata || olculen === 0
    ? `HUKUM: kirmizi ${hata} / ${olculen} çeviri altını Kervan çıktısıyla eşleşmiyor`
    : `HUKUM: yesil ${olculen} çeviri altını Kervan çıktısıyla birebir`
);
process.exit(hata || olculen === 0 ? 1 : 0);
