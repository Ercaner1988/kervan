pub mod dogrula;
pub mod goc;
pub mod yedekle;

pub use dogrula::{DogrulamaRaporu, sistemi_dogrula};
pub use goc::{GocOzeti, GocSecenekleri, goc_calistir};
pub use yedekle::veritabani_yedekle;

use kervan_depo::RusqliteDepo;
use kervan_katalog::Katalog;

/// CLI argümanlarını ayrıştırıp ilgili komutu yürütür
pub fn komut_yurut(
    args: &[String],
    depo: &RusqliteDepo,
    katalog: &Katalog,
) -> Result<String, String> {
    if args.is_empty() {
        return Ok(yardim_metni());
    }

    match args[0].as_str() {
        "goc" => {
            let mut secenekler = GocSecenekleri::default();
            for a in args.iter().skip(1) {
                if a == "--uygula" {
                    secenekler.kuru_kosu = false;
                } else if let Some(yol) = a.strip_prefix("--kaynak=") {
                    secenekler.kaynak_dizin = yol.to_string();
                } else if let Some(db) = a.strip_prefix("--hedef=") {
                    secenekler.hedef_db = db.to_string();
                }
            }
            let ozet = goc_calistir(&secenekler);
            Ok(format!(
                "Göç Özeti: {}\n(Kuru koşu: {}, Aktarılan Hesap: {}, Anahtar: {}, Ayar: {})",
                ozet.durum_mesaji,
                ozet.kuru_kosu_mu,
                ozet.aktarilan_hesap,
                ozet.aktarilan_anahtar,
                ozet.aktarilan_ayarlar
            ))
        }
        "yedekle" => {
            let hedef = args
                .get(1)
                .map(|s| s.as_str())
                .unwrap_or("kervan_yedek.sqlite");
            veritabani_yedekle(depo, hedef)
        }
        "dogrula" => {
            let rapor = sistemi_dogrula(depo, katalog);
            Ok(format!(
                "Doğrulama: {}\nVeritabanı Sağlam: {}, Tablo Sayısı: {}, Sağlayıcı: {}, Model: {}",
                rapor.durum,
                rapor.veritabani_saglam_mi,
                rapor.tablo_sayisi,
                rapor.kayitli_saglayici_sayisi,
                rapor.kayitli_model_sayisi
            ))
        }
        "yardim" | "--help" | "-h" => Ok(yardim_metni()),
        bilinmeyen => Err(format!(
            "Bilinmeyen CLI komutu: '{}'. Kullanılabilir komutlar için 'kervan yardim' yazın.",
            bilinmeyen
        )),
    }
}

pub fn yardim_metni() -> String {
    r#"Kervan CLI İşletim ve Göç Araçları

KULLANIM:
    kervan <alt-komut> [seçenekler]

ALT KOMUTLAR:
    goc [--kaynak=<yol>] [--hedef=<db>] [--uygula]
        OmniRoute veri dizininden verileri içe aktarır (varsayılan: kuru koşu)
    yedekle [hedef_yol]
        Veritabanını güvenli bir şekilde yedekler (VACUUM INTO)
    dogrula
        Veritabanı, sağlayıcı kataloğu ve sistem bütünlüğünü denetler
    yardim
        Bu yardım metnini gösterir
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    use kervan_depo::Depo;

    #[test]
    fn test_cli_yardim_metni() {
        let yardim = yardim_metni();
        assert!(yardim.contains("Kervan CLI"));
        assert!(yardim.contains("goc"));
        assert!(yardim.contains("yedekle"));
        assert!(yardim.contains("dogrula"));
    }

    #[test]
    fn test_cli_goc_kuru_kosu_varsayilan() {
        let temp_dir = std::env::temp_dir().join(format!("kervan_cli_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let db_path = temp_dir.join("test.sqlite");
        let sifreleyici = kervan_depo::Sifreleyici::yeni(Some("test-anahtar-123"));
        let depo = RusqliteDepo::dosya_ac(&db_path, sifreleyici, None).unwrap();
        let katalog = Katalog::yerlesik();

        let args = vec![
            "goc".to_string(),
            format!("--kaynak={}", temp_dir.display()),
        ];
        let cikti = komut_yurut(&args, &depo, &katalog).unwrap();
        assert!(cikti.contains("Kuru koşu: true"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_cli_dogrula() {
        let sifreleyici = kervan_depo::Sifreleyici::yeni(Some("test-anahtar-123"));
        let goc_yolu = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../ref/omniroute-dbe703a/src/lib/db/migrations");

        let depo = if goc_yolu.exists() {
            let d = RusqliteDepo::bellek_ici(sifreleyici, Some(&goc_yolu)).unwrap();
            d.gocleri_calistir().unwrap();
            d
        } else {
            RusqliteDepo::bellek_ici(sifreleyici, None).unwrap()
        };

        let katalog = Katalog::yerlesik();
        let args = vec!["dogrula".to_string()];
        let cikti = komut_yurut(&args, &depo, &katalog).unwrap();
        assert!(cikti.contains("Veritabanı Sağlam: true"));
        assert!(cikti.contains("Sağlayıcı: 276"));
    }
}
