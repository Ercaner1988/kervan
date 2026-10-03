import { describe, expect, it } from "bun:test";
import { ulasilabilirlikDenetle } from "./ulasilabilirlik.ts";
import { sabitYanitlariTara, dosyayiTara } from "./sabit-yanit.ts";
import { ucKaydiniDenetle, kayitliRotalariBul } from "./uc-kaydi.ts";
import { muhurDenetle, dosyaSha256 } from "./muhur-kontrol.ts";
import { raporuDenetle } from "./rapor-kontrol.ts";
import fs from "fs";
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
    const onbellekliTestler = new Set(["server::tests::tohum_gecmisi_ve_asil_soruyu_tasir"]);
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
    // 1. P2 fazında henüz mühür yokken geçerli
    const p2Sonuc = muhurDenetle("P2");
    expect(p2Sonuc.uygun).toBe(true);

    // 2. Sahte mühür dosyasıyla değiştirilmiş dosya testi -> kırmızı olmalı
    const tempDosya = "temp_muhurlu_dosya.txt";
    fs.writeFileSync(tempDosya, "orijinal icerik", "utf-8");
    const sahteSha = dosyaSha256(tempDosya);

    const tempMuhur = "temp_muhur.toml";
    fs.writeFileSync(tempMuhur, `
      [[dosya]]
      yol = "${tempDosya}"
      sha256 = "${sahteSha}"
    `, "utf-8");

    // Dosyayı boz
    fs.writeFileSync(tempDosya, "bozulmus icerik", "utf-8");
    const bozukSonuc = muhurDenetle("P4", tempMuhur);
    expect(bozukSonuc.uygun).toBe(false);
    expect(bozukSonuc.hatalar.some((h) => h.includes("MÜHÜR İHLALİ"))).toBe(true);

    // 3. Dosyayı eski haline getir -> yeşil olmalı
    fs.writeFileSync(tempDosya, "orijinal icerik", "utf-8");
    const temizSonuc = muhurDenetle("P4", tempMuhur);
    expect(temizSonuc.uygun).toBe(true);

    fs.unlinkSync(tempDosya);
    fs.unlinkSync(tempMuhur);
  }, 30000);

  // P2-A5: Rapor kontrolü
  it("p2::a5_rapor_kontrol_bagisiz_veya_eksik_maddede_kirmizi_tam_raporda_yesildir", () => {
    // 1. port/kayit/P1/RAPOR.md tamdır -> yeşil olmalı
    const p1Sonuc = raporuDenetle("port/kayit/P1/RAPOR.md");
    expect(p1Sonuc.uygun).toBe(true);

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
