import { describe, expect, it } from "bun:test";
import { ulasilabilirlikDenetle } from "./ulasilabilirlik.ts";
import { sabitYanitlariTara, dosyayiTara } from "./sabit-yanit.ts";
import { ucKaydiniDenetle, kayitliRotalariBul } from "./uc-kaydi.ts";
import { muhurDenetle, dosyaSha256 } from "./muhur-kontrol.ts";
import { raporuDenetle, EK_D_MADDELERI } from "./rapor-kontrol.ts";
import fs from "fs";
import os from "os";
import path from "path";

describe("p2::kapilar", () => {
  // P2-A1: Ulaşılabilirlik
  it("p2::a1_ulasilabilirlik_temiz_agacta_gecer", () => {
    const sonuc = ulasilabilirlikDenetle();
    expect(sonuc.uygun).toBe(true);
    expect(sonuc.ulasilamayanlar.length).toBe(0);
  }, 30000);

  // P2-A2: Sabit yanıt
  it("p2::a2_sabit_yanit_test_disi_kalipte_kirmizi_cfg_test_icinde_yesildir", () => {
    // 1. cfg(test) içinde kalıp -> temiz olmalı (yeşil)
    const testIcerik = `
      #[cfg(test)]
      mod tests {
        #[test]
        fn ornek() {
          let s = "chatcmpl-123";
          let t = "1700000000";
        }
      }
    `;
    const testDosya = "temp_test_cfg_test.rs";
    fs.writeFileSync(testDosya, testIcerik, "utf-8");
    const testBulgular = dosyayiTara(testDosya);
    expect(testBulgular.length).toBe(0);
    fs.unlinkSync(testDosya);

    // 2. Üretim kodunda (test dışı) kalıp -> bulgu vermeli (kırmızı)
    const uretimIcerik = `
      pub fn sahte_yanit() -> &'static str {
        "chatcmpl-123"
      }
    `;
    const uretimDosya = "temp_uretim_test.rs";
    fs.writeFileSync(uretimDosya, uretimIcerik, "utf-8");
    const uretimBulgular = dosyayiTara(uretimDosya);
    expect(uretimBulgular.length).toBe(1);
    expect(uretimBulgular[0].kalip).toBe("chatcmpl-123");
    fs.unlinkSync(uretimDosya);

    // 3. Mevcut depo temiz olmalı
    const depoSonuc = sabitYanitlariTara();
    expect(depoSonuc.temizMi).toBe(true);
  }, 30000);

  // P2-A3: Uç kaydı
  it("p2::a3_uc_kaydi_eksik_rota_ve_test_hatalarinda_kirmizidir", () => {
    // 1. Mevcut port/uclar.toml geçerli olmalı
    const onbellekliTestler = new Set(["server::tests::tohum_gecmisi_ve_asil_soruyu_tasir", "p5_e2e_nonce_testi", "p5_models_testi"]);
    const gecerliSonuc = ucKaydiniDenetle(process.cwd(), undefined, onbellekliTestler);
    expect(gecerliSonuc.gecerli).toBe(true);

    // 2. Kayıtsız rota içeren sahte toml -> kırmızı olmalı
    const sahteToml = "temp_eksik_uclar.toml";
    fs.writeFileSync(sahteToml, `
      [[uc]]
      yol = "/v1/chat/completions"
      yontem = "POST"
      durum = "kapsam-disi"
    `, "utf-8"); // Diğer 15 rota eksik!

    const eksikSonuc = ucKaydiniDenetle(process.cwd(), sahteToml, onbellekliTestler);
    expect(eksikSonuc.gecerli).toBe(false);
    expect(eksikSonuc.hatalar.length).toBeGreaterThan(0);
    fs.unlinkSync(sahteToml);

    // 3. Olmayan test kimliği içeren 'gercek' rota -> kırmızı olmalı
    const gecersizTestToml = "temp_gecersiz_test_uclar.toml";
    fs.writeFileSync(gecersizTestToml, `
      [[uc]]
      yol = "/v1/chat/completions"
      yontem = "POST"
      durum = "gercek"
      test = "olmayan::test::xyz_999"
    `, "utf-8");
    const testSonuc = ucKaydiniDenetle(process.cwd(), gecersizTestToml, onbellekliTestler);
    expect(testSonuc.gecerli).toBe(false);
    fs.unlinkSync(gecersizTestToml);
  }, 60000);

  // P2-A4: Mühür kontrolü
  it("p2::a4_muhur_kontrol_degisiklikte_kirmizi_temizde_yesildir", () => {
    // muhurDenetle(kok) kök dizini alır: mühürsüz (boş) kök fail-closed kırmızıdır, git geçmişi
    // olmayan geçici kök de yeşil olamaz. Tam yeşil yolu (imzalı etiket) araclar/kapilar/c1-muhur.test.ts'te.
    const kok = fs.mkdtempSync(path.join(os.tmpdir(), "kervan_a4_"));
    try {
      const bos = muhurDenetle(kok);
      expect(bos.uygun).toBe(false);
      expect(bos.hatalar.some((h) => h.includes("mühürsüz depo"))).toBe(true);

      // Mühür dosyası var ama dosya listesi boş / onaysız -> kırmızı, nedenleri yazılı
      fs.mkdirSync(path.join(kok, "port"), { recursive: true });
      fs.writeFileSync(path.join(kok, "port", "muhur.toml"), "onay_bekliyor = true\n", "utf-8");
      const onaysiz = muhurDenetle(kok);
      expect(onaysiz.uygun).toBe(false);
      expect(onaysiz.hatalar.some((h) => h.includes("onayı bekliyor"))).toBe(true);
    } finally {
      fs.rmSync(kok, { recursive: true, force: true });
    }

    // Bu deponun gerçek mührü: özet işlevi dosya bozulunca değişir (mühür ihlalinin dayanağı).
    const tempDosya = "temp_muhurlu_dosya.txt";
    fs.writeFileSync(tempDosya, "orijinal icerik", "utf-8");
    const once = dosyaSha256(tempDosya);
    fs.writeFileSync(tempDosya, "bozulmus icerik", "utf-8");
    expect(dosyaSha256(tempDosya)).not.toBe(once);
    fs.writeFileSync(tempDosya, "orijinal icerik", "utf-8");
    expect(dosyaSha256(tempDosya)).toBe(once);
    fs.unlinkSync(tempDosya);
  }, 30000);

  // P2-A5: Rapor kontrolü
  it("p2::a5_rapor_kontrol_bagisiz_veya_eksik_maddede_kirmizi_tam_raporda_yesildir", () => {
    // 1. Altı maddesi gerçek dosya:satır kanıtlı, bağsız iddiasız rapor -> yeşil olmalı
    // (tarihsel port/kayit/P1/RAPOR.md C1 öncesi yazıldı; güncel kurala örnek olarak kullanılmaz)
    const iyiRapor = "temp_iyi_rapor.md";
    fs.writeFileSync(
      iyiRapor,
      "# Rapor\n## Ek D Sahte-Kapı\n" + EK_D_MADDELERI.map((m) => `- ${m}: baktım: araclar/kapilar/rapor-kontrol.ts:1`).join("\n") + "\n",
      "utf-8"
    );
    const iyiSonuc = raporuDenetle(iyiRapor);
    fs.unlinkSync(iyiRapor);
    expect(iyiSonuc.hatalar).toEqual([]);
    expect(iyiSonuc.uygun).toBe(true);

    // 2. Eksik Ek D maddeli sahte rapor -> kırmızı olmalı
    const sahteRapor = "temp_sahte_rapor.md";
    fs.writeFileSync(sahteRapor, `
      # Sahte Rapor
      | P1-A1 | Açıklama | [log.log](log.log) | ✅ GEÇTİ |
    `, "utf-8");
    const sahteSonuc = raporuDenetle(sahteRapor);
    expect(sahteSonuc.uygun).toBe(false);
    expect(sahteSonuc.hatalar.some((h) => h.includes("Ek D sahte-kapı listesi maddesi eksik"))).toBe(true);

    // 3. Bağsız 'geçti' iddiası olan rapor -> kırmızı olmalı
    fs.writeFileSync(sahteRapor, `
      # Sahte Rapor
      ## Ek D Sahte-Kapı
      - Sabit yanıt: baktım
      - Kendini doğrulayan kontrol: baktım
      - Ulaşılamayan teslim: baktım
      - Kanıtı aşan rapor: baktım
      - Gevşetme: baktım
      - Başarısız olamayan test: baktım
      
      Bu adımda her şey başarıyla geçti ve tamamlandı.
    `, "utf-8");
    const bagisizSonuc = raporuDenetle(sahteRapor);
    expect(bagisizSonuc.uygun).toBe(false);
    expect(bagisizSonuc.hatalar.some((h) => h.includes("Bağsız başarı iddiası"))).toBe(true);

    fs.unlinkSync(sahteRapor);
  }, 30000);
});
