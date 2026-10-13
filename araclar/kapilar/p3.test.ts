import { describe, expect, it } from "bun:test";
import { kulliyatCommitiniTara } from "./kulliyat-tara.ts";
import { mutantlarTomlOku, pariteBaglantisiniDenetle, mutantlariDene, MutantTanimi } from "./mutasyon-kontrol.ts";
import { durumMdKontrol, readmeKontrol, durumDenetle } from "./durum-kontrol.ts";
import { defteriDenetle, DefterKaydi } from "./defter-kontrol.ts";
import fs from "fs";
import os from "os";
import path from "path";

describe("p3::kapilar", () => {
  // P3-A1: Kırmızı Takım Külliyatı Taraması
  it("p3::a1_kulliyat_taramasi_ac2f18d_5_sinifi_yakalar_ve_temiz_agac_yesildir", () => {
    // 1. ac2f18d (omniroute-aktarim) commitinde 5 sınıfın tamamı kırmızı (yakalandı) olmalı
    const sonuc = kulliyatCommitiniTara("ac2f18d");
    expect(sonuc.sinif1_sabitYanit).toBe(true);
    expect(sonuc.sinif2_bagisizCrate).toBe(true);
    expect(sonuc.sinif3_pariteBaglantisiz).toBe(true);
    expect(sonuc.sinif4_kanitsizDefter).toBe(true);
    expect(sonuc.sinif5_elleYazilmisDurum).toBe(true);
    expect(sonuc.tumSiniflarYakalandiMi).toBe(true);
    expect(sonuc.bulgular.length).toBeGreaterThanOrEqual(5);

    // 2. Mevcut temiz ağaçta (HEAD) sahte teslim iddiaları tetiklenmemeli
    const temizSonuc = kulliyatCommitiniTara("HEAD");
    expect(temizSonuc.tumSiniflarYakalandiMi).toBe(false);
  }, 300000); // ölçüm: ısınmış target/kulliyat ile ~20 sn + ~130 sn (soğuk ilk koşu ~10 dk)

  // P3-A2: Mutasyon Kapısı ve Parite Bağlantısı
  it("p3::a2_mutasyon_ve_parite_baglantisi", () => {
    // 1. port/mutantlar.toml okunabilir olmalı ve en az 2 mutant içermeli
    const mutantlar = mutantlarTomlOku();
    expect(mutantlar.length).toBeGreaterThanOrEqual(2);

    // 2. Parite bağlantı denetimi: Mevcut araclar/parite (boş veya bağlı) yeşildir
    const mevcutParite = pariteBaglantisiniDenetle();
    expect(mevcutParite.uygun).toBe(true);

    // 3. Bağsız parite betiği tespit edilince kırmızı olmalı
    const tempPariteDizini = path.join("araclar", "parite_temp_test");
    if (!fs.existsSync(tempPariteDizini)) {
      fs.mkdirSync(tempPariteDizini, { recursive: true });
    }
    const sahteKosTs = path.join(tempPariteDizini, "sahte-kos.ts");
    fs.writeFileSync(sahteKosTs, `
      // Kendini doğrulayan sahte parite: Kervan ikilisine veya fetch'e bağlanmıyor!
      const a = 1;
      const b = 1;
      console.log("Parite eşit:", a === b);
    `, "utf-8");

    const sahtePariteSonuc = pariteBaglantisiniDenetle(tempPariteDizini);
    expect(sahtePariteSonuc.uygun).toBe(false);
    expect(sahtePariteSonuc.hatalar.some((h) => h.includes("Kendini doğrulayan parite"))).toBe(true);

    fs.unlinkSync(sahteKosTs);
    fs.rmdirSync(tempPariteDizini);

    // 4. Bilerek zayıf bırakılmış testte yaşayan mutant testi kırmızıya çevirir
    // Geçici kökte koşar (depoda başıboş dosya bırakmaz); günlük dizini mutasyon kapısında zorunlu.
    const tempKok = fs.mkdtempSync(path.join(os.tmpdir(), "kervan_a2_"));
    const tempMutantDosya = "temp_mutant_hedef.txt";
    fs.writeFileSync(path.join(tempKok, tempMutantDosya), "export const DEGER = 42;", "utf-8");
    // Kapı yalnız cargo test hedefi ya da var olan betik dosyası kabul eder: mutasyondan etkilenmeyen betik.
    fs.writeFileSync(path.join(tempKok, "zayif.ts"), "console.log(\"HUKUM: yesil\");", "utf-8");

    const zayifMutant: MutantTanimi = {
      ad: "zayif-test-yasayan-mutant",
      dosya: tempMutantDosya,
      bul: "42",
      yerine: "999",
      // Test mutasyondan etkilenmiyor (her zaman yeşil) -> mutant YAŞAR -> mutantlariDene HATA verir
      oldurmesi_gereken_test: ["bun zayif.ts"],
    };

    const denemeSonuc = mutantlariDene([zayifMutant], { gunlukDizini: path.join(tempKok, "gunluk"), kokDizin: tempKok });
    expect(denemeSonuc.uygun).toBe(false);
    expect(denemeSonuc.hatalar.some((h) => h.includes("YAŞADI"))).toBe(true);

    fs.rmSync(tempKok, { recursive: true, force: true });
  }, 30000);

  // P3-A3: Üretilmiş Durum ve Belge Bütünlüğü
  it("p3::a3_durum_ve_belge_butunlugu", () => {
    // 1. Mevcut port/DURUM.md ve README.md geçerli olmalı
    const durumSonuc = durumDenetle();
    expect(durumSonuc.uygun).toBe(true);
    expect(durumSonuc.hatalar.length).toBe(0);

    // 2. Elle değiştirilmiş (hash bozulmuş) DURUM.md -> kırmızı olmalı
    const sahteDurumMd = "temp_sahte_durum.md";
    fs.writeFileSync(sahteDurumMd, `<!-- URETILMIS_DURUM_HASH: 1234567890abcdef -->
# Sahte Durum
Bu metin elle değiştirildi!
`, "utf-8");

    const bozukDurumSonuc = durumMdKontrol(sahteDurumMd);
    expect(bozukDurumSonuc.uygun).toBe(false);
    expect(bozukDurumSonuc.hata).toContain("elle düzenlenmiş");
    fs.unlinkSync(sahteDurumMd);

    // 3. Eksik zorunlu bölüm içeren README -> kırmızı olmalı
    const sahteReadme = "temp_sahte_readme.md";
    fs.writeFileSync(sahteReadme, `# Kervan
Sadece tanıtım metni, Gereksinimler veya Kurulum başlığı yok!
`, "utf-8");

    const bozukReadmeSonuc = readmeKontrol(sahteReadme);
    expect(bozukReadmeSonuc.uygun).toBe(false);
    expect(bozukReadmeSonuc.hatalar.some((h) => h.includes("bölümü silinmiş veya bulunamadı"))).toBe(true);
    fs.unlinkSync(sahteReadme);
  }, 30000);

  // P3-A4: Defter Kapısı İhlalleri
  it("p3::a4_defter_kapisi_ihlalleri", () => {
    // 1. Mevcut defter geçerli olmalı
    const defterSonuc = defteriDenetle();
    expect(defterSonuc.uygun).toBe(true);

    // 2-3. Geçici defter dizini (depoda başıboş dosya bırakmaz); kaynak gerçek bir ref dosyası (286 satır),
    // hedef/kanıt gerçek dosyalar. Kapı kökü deponun kendisi, defter dosyaları geçici dizinde.
    const kaynak = "ref/omniroute-dbe703a/open-sse/services/combo/applyStrategyOrdering.ts";
    const qs = "ref/omniroute-dbe703a/open-sse/services/combo/quotaStrategies.ts";
    const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "kervan_a4d_"));
    const denetle = (kayitlar: DefterKaydi[]) => {
      fs.writeFileSync(path.join(tmp, "dosyalar.jsonl"), kayitlar.map((k) => JSON.stringify(k)).join("\n") + "\n", "utf-8");
      const dav = { id: "dav-ok", tanim: "fixture", durum: "dogrulandi", hedef: "Cargo.toml", kanit: ["port/altin/claude_1_sade.json"], kaynak, satir: [1, 50] };
      fs.writeFileSync(path.join(tmp, "davranislar.jsonl"), JSON.stringify(dav) + "\n", "utf-8");
      return defteriDenetle(path.join(tmp, "dosyalar.jsonl"), false, process.cwd(), { davranislarYolu: path.join(tmp, "davranislar.jsonl") });
    };
    try {
      // Sahte hedef, sahte kanıt, pencere boşluğu (51-99 ve 151-286) ve gerekçesiz kapsam-dışı
      const ihlalSonuc = denetle([
        { id: "rec-1", kaynak, satir: [1, 50], durum: "dogrulandi", hedef: "olmayan/hedef/dosya.rs", kanit: ["port/altin/claude_1_sade.json"] },
        { id: "rec-2", kaynak, satir: [100, 150], durum: "dogrulandi", hedef: "Cargo.toml", kanit: ["olmayan/kanit/rapor.json"] },
        { id: "rec-3", kaynak: qs, satir: [1, 20], durum: "kapsam-disi" },
      ] as DefterKaydi[]);
      expect(ihlalSonuc.uygun).toBe(false);
      expect(ihlalSonuc.hatalar.some((h) => h.includes("'hedef' bir dosya değil ya da bulunamadı"))).toBe(true);
      expect(ihlalSonuc.hatalar.some((h) => h.includes("Kanıt dosyası bulunamadı"))).toBe(true);
      expect(ihlalSonuc.hatalar.some((h) => h.includes("satır aralığı boşluğu tespit edildi"))).toBe(true);
      expect(ihlalSonuc.hatalar.some((h) => h.includes("gerekçe belirtilmemiş"))).toBe(true);

      // Kusursuz: pencereler boşluksuz, hedef ve kanıt gerçek, kapsam-dışı gerekçeli
      const kusursuz = denetle([
        { id: "rec-ok-1", kaynak, satir: [1, 150], durum: "dogrulandi", hedef: "Cargo.toml", kanit: ["port/altin/claude_1_sade.json"] },
        { id: "rec-ok-2", kaynak, satir: [151, 286], durum: "dogrulandi", hedef: "Cargo.toml", kanit: ["port/altin/claude_1_sade.json"] },
        { id: "rec-ok-3", kaynak: qs, satir: [1, 300], durum: "kapsam-disi", gerekce: "Test için kapsam dışı." },
        { id: "rec-ok-4", kaynak: qs, satir: [301, 600], durum: "kapsam-disi", gerekce: "Test için kapsam dışı." },
        { id: "rec-ok-5", kaynak: qs, satir: [601, 855], durum: "kapsam-disi", gerekce: "Test için kapsam dışı." },
      ] as DefterKaydi[]);
      expect(kusursuz.hatalar).toEqual([]);
      expect(kusursuz.uygun).toBe(true);
    } finally {
      fs.rmSync(tmp, { recursive: true, force: true });
    }
  }, 30000);
});
