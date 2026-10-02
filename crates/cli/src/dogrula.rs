use kervan_depo::{Depo, RusqliteDepo};
use kervan_katalog::Katalog;

#[derive(Debug, PartialEq, Eq)]
pub struct DogrulamaRaporu {
    pub veritabani_saglam_mi: bool,
    pub tablo_sayisi: usize,
    pub kayitli_saglayici_sayisi: usize,
    pub kayitli_model_sayisi: usize,
    pub durum: String,
}

pub fn sistemi_dogrula(depo: &RusqliteDepo, katalog: &Katalog) -> DogrulamaRaporu {
    let db_saglik = depo.saglik_kontrolu().unwrap_or(false);
    let tablo_sayisi = depo.tablo_sayisi().unwrap_or(0);
    let saglayici_sayisi = katalog.toplam_saglayici();
    let model_sayisi = katalog.toplam_model();

    let durum = if db_saglik && tablo_sayisi >= 134 && saglayici_sayisi >= 250 {
        "Sistem tam ve sağlıklı.".to_string()
    } else {
        "Bazı bütünlük kontrollerinde eksikler tespit edildi.".to_string()
    };

    DogrulamaRaporu {
        veritabani_saglam_mi: db_saglik,
        tablo_sayisi,
        kayitli_saglayici_sayisi: saglayici_sayisi,
        kayitli_model_sayisi: model_sayisi,
        durum,
    }
}
