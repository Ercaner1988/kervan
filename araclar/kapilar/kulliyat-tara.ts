import { spawnSync } from "child_process";
import fs from "fs";
import path from "path";

// PRD FR-2.5: Kırmızı Takım Külliyatı Denetleyicisi (kulliyat-tara.ts)
// ac2f18d commitindeki (önceki sahte teslim) 5 kusur sınıfının tamamını yakalar:
// Sınıf 1: Sabit yanıt (sohbet.rs -> chatcmpl-123, 1700000000, selamlar!)
// Sınıf 2: Bağsız crate (kervan-v1, kervan-paneller)
// Sınıf 3: Kendini doğrulayan parite (kos.ts içinde fetch/spawn yok)
// Sınıf 4: Kanıtsız defter (dosyalar.jsonl içinde boş hedef/kanit)
// Sınıf 5: Elle yazılmış durum (DURUM.md içinde üretim hash'i yok)

export interface KulliyatRaporu {
  sinif1_sabitYanit: boolean;
  sinif2_bagisizCrate: boolean;
  sinif3_pariteBaglantisiz: boolean;
  sinif4_kanitsizDefter: boolean;
  sinif5_elleYazilmisDurum: boolean;
  tumSiniflarYakalandiMi: boolean;
  bulgular: string[];
}

export function gitDosyaIcerigi(commit: string, dosyaYolu: string): string {
  const proc = spawnSync("git", ["show", `${commit}:${dosyaYolu}`], {
    encoding: "utf-8",
    maxBuffer: 20 * 1024 * 1024,
  });
  return proc.status === 0 ? proc.stdout : "";
}

export function kulliyatCommitiniTara(commit = "omniroute-aktarim"): KulliyatRaporu {
  const bulgular: string[] = [];

  // Sınıf 1: Sabit yanıt kontrolü (crates/v1/src/sohbet.rs)
  const sohbetRs = gitDosyaIcerigi(commit, "crates/v1/src/sohbet.rs");
  const sinif1 =
    sohbetRs.includes("chatcmpl-123") ||
    sohbetRs.includes("1700000000") ||
    sohbetRs.includes("selamlar!");
  if (sinif1) {
    bulgular.push("sabit-yanit: crates/v1/src/sohbet.rs içinde sahte sabit yanıt kalıpları bulundu");
  }

  // Sınıf 2: Bağsız crate kontrolü (Cargo.toml içinde kervan-v1, kervan-paneller var ama kök Cargo.toml kervan'a bağlamıyor)
  const rootCargo = gitDosyaIcerigi(commit, "Cargo.toml");
  const sinif2 =
    rootCargo.includes("crates/v1") &&
    rootCargo.includes("crates/paneller") &&
    !rootCargo.includes("kervan-v1 = { path");
  if (sinif2) {
    bulgular.push("bagisiz-crate: kervan-v1 ve kervan-paneller hiçbir ikiliye bağlı değil");
  }

  // Sınıf 3: Kendini doğrulayan parite kontrolü (araclar/parite/kos.ts)
  const kosTs = gitDosyaIcerigi(commit, "araclar/parite/kos.ts");
  const sinif3 =
    kosTs.length > 0 &&
    !kosTs.includes("fetch(") &&
    !kosTs.includes("Bun.spawn") &&
    !kosTs.includes("spawnSync");
  if (sinif3) {
    bulgular.push("kendini-dogrulayan-parite: araclar/parite/kos.ts gerçek Kervan ikilisine/API'sine bağlanmıyor");
  }

  // Sınıf 4: Kanıtsız defter satırı kontrolü (port/defter/dosyalar.jsonl)
  const defterJsonl = gitDosyaIcerigi(commit, "port/defter/dosyalar.jsonl");
  // 23752 planlandi ve hedef/kanit boş (hedef: null, kanit: [])
  const sinif4 =
    defterJsonl.includes(`"durum":"planlandi"`) &&
    (defterJsonl.includes(`"hedef":null`) || defterJsonl.includes(`"kanit":[]`));
  if (sinif4) {
    bulgular.push("kanitsiz-defter: port/defter/dosyalar.jsonl içinde 23 binden fazla kanıtsız/hedefsiz kayıt bulundu");
  }

  // Sınıf 5: Elle yazılmış durum iddiası (port/DURUM.md)
  const durumMd = gitDosyaIcerigi(commit, "port/DURUM.md");
  const sinif5 =
    durumMd.length > 0 &&
    !durumMd.includes("URETILMIS_DURUM_HASH") &&
    durumMd.includes("TAMAMLANDI");
  if (sinif5) {
    bulgular.push("elle-yazilmis-durum: port/DURUM.md üretim özeti (hash) taşımıyor, elle yazılmış iddialar içeriyor");
  }

  const tumSiniflarYakalandiMi = sinif1 && sinif2 && sinif3 && sinif4 && sinif5;

  return {
    sinif1_sabitYanit: sinif1,
    sinif2_bagisizCrate: sinif2,
    sinif3_pariteBaglantisiz: sinif3,
    sinif4_kanitsizDefter: sinif4,
    sinif5_elleYazilmisDurum: sinif5,
    tumSiniflarYakalandiMi,
    bulgular,
  };
}

if (import.meta.main) {
  const commit = process.argv[2] || "omniroute-aktarim";
  console.log(`[kulliyat-tara] Kırmızı takım commit'i '${commit}' taranıyor...`);

  const sonuc = kulliyatCommitiniTara(commit);
  console.log("\n--- TESPİT EDİLEN BULGULAR ---");
  for (const b of sonuc.bulgular) {
    console.log(`❌ [YAKALANDI] ${b}`);
  }

  console.log(`\nSınıf 1 (Sabit Yanıt):           ${sonuc.sinif1_sabitYanit ? "🔴 YAKALANDI (Kırmızı)" : "⚪ Kaçtı"}`);
  console.log(`Sınıf 2 (Bağsız Crate):          ${sonuc.sinif2_bagisizCrate ? "🔴 YAKALANDI (Kırmızı)" : "⚪ Kaçtı"}`);
  console.log(`Sınıf 3 (Parite Bağlantısız):    ${sonuc.sinif3_pariteBaglantisiz ? "🔴 YAKALANDI (Kırmızı)" : "⚪ Kaçtı"}`);
  console.log(`Sınıf 4 (Kanıtsız Defter):       ${sonuc.sinif4_kanitsizDefter ? "🔴 YAKALANDI (Kırmızı)" : "⚪ Kaçtı"}`);
  console.log(`Sınıf 5 (Elle Yazılmış Durum):   ${sonuc.sinif5_elleYazilmisDurum ? "🔴 YAKALANDI (Kırmızı)" : "⚪ Kaçtı"}`);

  if (sonuc.tumSiniflarYakalandiMi) {
    console.log(`\n🎉 BAŞARILI: ac2f18d teslimindeki 5 sahte teslim sınıfının TAMAMI yakalandı.`);
    process.exit(0);
  } else {
    console.error(`\n❌ BAŞARISIZ: Bazı sahte teslim sınıfları kapılardan kaçtı!`);
    process.exit(1);
  }
}
