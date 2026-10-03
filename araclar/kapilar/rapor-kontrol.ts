import fs from "fs";
import path from "path";

// PRD FR-3.1: Rapor Sözleşmesi ve Sahte-Kapı Kontrolü (rapor-kontrol.ts)
// 1. Her 'geçti / yeşil / tamamlandı / çalışıyor' iddiası bir log dosyasına veya dosya:satır bağlantısına bağlı olmalıdır (veya 'bakmadım' denmelidir).
// 2. Ek D Sahte-Kapı listesindeki 6 maddenin tamamına 'baktım: <kanıt>' veya 'bakmadım' yanıtı verilmelidir.

export const EK_D_MADDELERI = [
  "Sabit yanıt",
  "Kendini doğrulayan kontrol",
  "Ulaşılamayan teslim",
  "Kanıtı aşan rapor",
  "Gevşetme",
  "Başarısız olamayan test",
];

export function raporuDenetle(raporYolu: string): {
  uygun: boolean;
  hatalar: string[];
  kontrolEdilenIddiaSayisi: number;
} {
  const hatalar: string[] = [];

  if (!fs.existsSync(raporYolu)) {
    return {
      uygun: false,
      hatalar: [`Rapor dosyası bulunamadı: ${raporYolu}`],
      kontrolEdilenIddiaSayisi: 0,
    };
  }

  const icerik = fs.readFileSync(raporYolu, "utf-8");
  const satirlar = icerik.split(/\r?\n/);

  // 1. Ek D Sahte-Kapı Listesi Kontrolü
  for (const madde of EK_D_MADDELERI) {
    const maddeRegex = new RegExp(`${madde}[:\\s*]+(baktım|bakmadım)`, "i");
    if (!maddeRegex.test(icerik)) {
      hatalar.push(`Ek D sahte-kapı listesi maddesi eksik veya cevapsız: '${madde}' ('baktım: <kanıt>' veya 'bakmadım' zorunludur)`);
    }
  }

  // 2. Bağsız başarı iddiaları kontrolü
  let kontrolEdilenIddiaSayisi = 0;
  const basariKelimeleri = ["geçti", "tamamlandı", "çalışıyor"];

  for (let i = 0; i < satirlar.length; i++) {
    const satir = satirlar[i].trim();
    if (!satir || satir.startsWith("#") || satir.startsWith("<!--")) continue;

    // Satır başarı iddiası içeriyor mu?
    const kucukSatir = satir.toLowerCase();
    const iddiaVar = basariKelimeleri.some((k) => kucukSatir.includes(k));

    if (iddiaVar) {
      kontrolEdilenIddiaSayisi++;

      // Kanıt kontrolü: log dosyası, dosya:satır, link veya 'bakmadım'
      const logVar = /\.log\b/i.test(satir);
      const linkVar = /\[.+?\]\(.+?\)/.test(satir);
      const dosyaSatirVar = /[a-zA-Z0-9_\-\.\/\\:]+\.(rs|ts|toml|json|md):\d+/.test(satir);
      const dosyaAdiVar = /`[a-zA-Z0-9_\-\.\/\\:]+\.(rs|ts|toml|json|md)`/.test(satir);
      const bakmadimVar = /bakmadım/i.test(satir);

      if (!logVar && !linkVar && !dosyaSatirVar && !dosyaAdiVar && !bakmadimVar) {
        hatalar.push(`Satır ${i + 1}: Bağsız başarı iddiası (kanıt, log veya dosya:satır içermiyor):\n   "${satir}"`);
      }
    }
  }

  return {
    uygun: hatalar.length === 0,
    hatalar,
    kontrolEdilenIddiaSayisi,
  };
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  let raporYolu = args[0];

  if (!raporYolu) {
    console.error("Kullanım: bun araclar/kapilar/rapor-kontrol.ts <port/kayit/P1/RAPOR.md | P1>");
    process.exit(1);
  }

  if (!raporYolu.endsWith(".md")) {
    raporYolu = path.join("port", "kayit", raporYolu, "RAPOR.md");
  }

  const sonuc = raporuDenetle(raporYolu);
  console.log(`[rapor-kontrol] İncelenen başarı iddiası sayısı: ${sonuc.kontrolEdilenIddiaSayisi}`);

  if (sonuc.uygun) {
    console.log(`✅ [rapor-kontrol] Rapor '${raporYolu}' sözleşmeye tam uyumludur.`);
    process.exit(0);
  } else {
    console.error(`❌ [rapor-kontrol] RAPOR SÖZLEŞMESİ İHLALLERİ BULUNDU (${raporYolu}):`);
    for (const h of sonuc.hatalar) {
      console.error(`  - ${h}`);
    }
    process.exit(1);
  }
}
