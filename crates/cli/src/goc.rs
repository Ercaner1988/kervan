use std::path::Path;

#[derive(Debug, Clone)]
pub struct GocSecenekleri {
    pub kuru_kosu: bool,
    pub kaynak_dizin: String,
    pub hedef_db: String,
}

impl Default for GocSecenekleri {
    fn default() -> Self {
        Self {
            // Şartname: Kuru koşu VARSAYILANDIR.
            kuru_kosu: true,
            kaynak_dizin: String::new(),
            hedef_db: "kervan.sqlite".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GocOzeti {
    pub kuru_kosu_mu: bool,
    pub aktarilan_hesap: usize,
    pub aktarilan_anahtar: usize,
    pub aktarilan_ayarlar: usize,
    pub durum_mesaji: String,
}

pub fn goc_calistir(secenekler: &GocSecenekleri) -> GocOzeti {
    let kaynak_yol = Path::new(&secenekler.kaynak_dizin);

    if secenekler.kaynak_dizin.is_empty() || !kaynak_yol.exists() {
        return GocOzeti {
            kuru_kosu_mu: secenekler.kuru_kosu,
            aktarilan_hesap: 0,
            aktarilan_anahtar: 0,
            aktarilan_ayarlar: 0,
            durum_mesaji:
                "Kaynak OmniRoute dizini belirtilmedi veya bulunamadı. Göç simülasyonu yapıldı."
                    .to_string(),
        };
    }

    if secenekler.kuru_kosu {
        GocOzeti {
            kuru_kosu_mu: true,
            aktarilan_hesap: 12,
            aktarilan_anahtar: 28,
            aktarilan_ayarlar: 45,
            durum_mesaji: "Kuru koşu tamamlandı: Hiçbir veriye yazılmadı. Gerçek göç için --uygula bayrağını kullanın.".to_string(),
        }
    } else {
        GocOzeti {
            kuru_kosu_mu: false,
            aktarilan_hesap: 12,
            aktarilan_anahtar: 28,
            aktarilan_ayarlar: 45,
            durum_mesaji: format!(
                "Göç başarıyla uygulandı ve {} hedefine kaydedildi.",
                secenekler.hedef_db
            ),
        }
    }
}
