import crypto from "crypto";
import fs from "fs";
import path from "path";

// PRD FR-2.7: Üretilmiş Durum Oluşturucu (durum-uret.ts)
// port/durum.json ve port/uclar.toml verilerinden port/DURUM.md belgesini deterministik olarak üretir.
// Elle yapılan değişikliklerin tespit edilebilmesi için başlığa SHA-256 üretim özeti ekler.

export function durumMarkdownUret(
  durumJsonYolu = path.join("port", "durum.json"),
  uclarTomlYolu = path.join("port", "uclar.toml")
): { markdown: string; hash: string } {
  let durum: any = { faz: "P2", uretim_zamani: new Date().toISOString(), kapilar: [] };
  if (fs.existsSync(durumJsonYolu)) {
    try {
      durum = JSON.parse(fs.readFileSync(durumJsonYolu, "utf-8"));
    } catch {}
  }

  let uclarIcerik = "";
  if (fs.existsSync(uclarTomlYolu)) {
    uclarIcerik = fs.readFileSync(uclarTomlYolu, "utf-8");
  }

  let mdGovde = `# DURUM.md — Kervan Aktarım Durum Defteri (ÜRETİLMİŞTİR)

> ⚠️ BU DOSYA OTOMATİK OLARAK ÜRETİLMİŞTİR. ELLE DÜZENLEMEYİNİZ.
> Düzenlemeler kapı ve defter araçları tarafından ezilir.

- **Mevcut Faz:** ${durum.faz || "P2"}
- **Son Güncelleme:** ${durum.uretim_zamani || new Date().toISOString()}
- **Tüm Kapılar Yeşil mi:** ${durum.tum_kapilar_yesil_mi ? "✅ EVET" : "❌ HAYIR"}

---

## 1. Kapı Sonuçları (port/durum.json)

| Kapı Adı | Çıkış Kodu | Süre (ms) | Log Dosyası |
|---|:---:|:---:|---|
`;

  for (const k of durum.kapilar || []) {
    const icon = k.cikis === 0 ? "✅" : "❌";
    mdGovde += `| ${k.ad} | ${icon} ${k.cikis} | ${k.sure_ms} | [${k.ad}.log](${k.log}) |\n`;
  }

  mdGovde += `
---

## 2. HTTP Rota Durumu (port/uclar.toml)

\`\`\`toml
${uclarIcerik.trim()}
\`\`\`
`;

  // Gövdenin hash'ini hesapla
  const hash = crypto.createHash("sha256").update(mdGovde).digest("hex");
  const tamMarkdown = `<!-- URETILMIS_DURUM_HASH: ${hash} -->\n${mdGovde}`;

  return { markdown: tamMarkdown, hash };
}

if (import.meta.main) {
  const { markdown } = durumMarkdownUret();
  const hedef = path.join("port", "DURUM.md");
  fs.writeFileSync(hedef, markdown, "utf-8");
  console.log(`✅ [durum-uret] '${hedef}' başarıyla üretildi.`);
}
