import { describe, expect, it } from "bun:test";
import { kulliyatCommitiniTara } from "./kulliyat-tara.ts";
import { mutantlarTomlOku, pariteBaglantisiniDenetle, mutantlariDene, MutantTanimi } from "./mutasyon-kontrol.ts";
import { durumMdKontrol, readmeKontrol, durumDenetle } from "./durum-kontrol.ts";
import { defteriDenetle, DefterKaydi } from "./defter-kontrol.ts";
import fs from "fs";
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
  }, 30000);

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
    const tempMutantDosya = "temp_mutant_hedef.txt";
    fs.writeFileSync(tempMutantDosya, "export const DEGER = 42;", "utf-8");

    const zayifMutant: MutantTanimi = {
      ad: "zayif-test-yasayan-mutant",
      dosya: tempMutantDosya,
      bul: "42",
      yerine: "999",
      // Test mutasyondan etkilenmiyor (her zaman yeşil) -> mutant YAŞAR -> mutantlariDene HATA verir
      oldurmesi_gereken_test: ["bun -e \"process.exit(0)\""],
    };

    const denemeSonuc = mutantlariDene([zayifMutant]);
    expect(denemeSonuc.uygun).toBe(false);
    expect(denemeSonuc.hatalar.some((h) => h.includes("YAŞADI"))).toBe(true);

    if (fs.existsSync(tempMutantDosya)) {
      fs.unlinkSync(tempMutantDosya);
    }
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

    // 2. Sahte hedef, sahte kanit ve pencere boşluğu içeren defter fixture'ı
    const sahteDefter = "temp_sahte_defter.jsonl";
    const fixtureKayitlar: DefterKaydi[] = [
      {
        id: "rec-1",
        kaynak: "src/server.ts",
        satir: [1, 50],
        durum: "dogrulandi",
        hedef: "olmayan/hedef/dosya.rs", // Olmayan hedef!
        kanit: ["src/server.ts"],
      },
      {
        id: "rec-2",
        kaynak: "src/server.ts",
        satir: [100, 150], // Satır 51-99 arası boşluk (gap)!
        durum: "dogrulandi",
        hedef: "Cargo.toml", // Var olan hedef
        kanit: ["olmayan/kanit/rapor.json"], // Olmayan kanıt!
      },
      {
        id: "rec-3",
        kaynak: "src/unused.ts",
        satir: [1, 20],
        durum: "kapsam-disi",
        // Gerekçe yok!
      },
    ];

    fs.writeFileSync(
      sahteDefter,
      fixtureKayitlar.map((k) => JSON.stringify(k)).join("\n"),
      "utf-8"
    );

    const ihlalSonuc = defteriDenetle(sahteDefter);
    expect(ihlalSonuc.uygun).toBe(false);
    // 3 ayrı bulgu beklenir:
    // (a) hedef dosyası bulunamadı
    expect(ihlalSonuc.hatalar.some((h) => h.includes("'hedef' dosyası bulunamadı"))).toBe(true);
    // (b) kanıt dosyası bulunamadı
    expect(ihlalSonuc.hatalar.some((h) => h.includes("Kanıt dosyası bulunamadı"))).toBe(true);
    // (c) pencere boşluğu (gap)
    expect(ihlalSonuc.hatalar.some((h) => h.includes("satır aralığı boşluğu tespit edildi"))).toBe(true);
    // (d) kapsam-dışı gerekçesiz
    expect(ihlalSonuc.hatalar.some((h) => h.includes("gerekçe belirtilmemiş"))).toBe(true);

    fs.unlinkSync(sahteDefter);

    // 3. Kusursuz fixture -> yeşil olmalı
    const gecerliDefter = "temp_gecerli_defter.jsonl";
    const gecerliKayitlar: DefterKaydi[] = [
      {
        id: "rec-ok-1",
        kaynak: "src/server.ts",
        satir: [1, 50],
        durum: "dogrulandi",
        hedef: "Cargo.toml",
        kanit: ["Cargo.toml"],
      },
      {
        id: "rec-ok-2",
        kaynak: "src/server.ts",
        satir: [51, 100],
        durum: "dogrulandi",
        hedef: "Cargo.toml",
        kanit: ["Cargo.toml"],
      },
      {
        id: "rec-ok-3",
        kaynak: "src/other.ts",
        satir: [1, 30],
        durum: "kapsam-disi",
        gerekce: "Test yardımcı modülü, Kervan kapsamı dışındadır.",
      },
    ];

    fs.writeFileSync(
      gecerliDefter,
      gecerliKayitlar.map((k) => JSON.stringify(k)).join("\n"),
      "utf-8"
    );

    const kusursuzSonuc = defteriDenetle(gecerliDefter);
    expect(kusursuzSonuc.uygun).toBe(true);
    expect(kusursuzSonuc.hatalar.length).toBe(0);

    fs.unlinkSync(gecerliDefter);
  }, 30000);
});
