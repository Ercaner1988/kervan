use kervan_depo::{Depo, RusqliteDepo};

pub fn veritabani_yedekle(depo: &RusqliteDepo, hedef_yol: &str) -> Result<String, String> {
    depo.yedekle(hedef_yol)
        .map_err(|e| format!("Yedekleme başarısız: {}", e))?;
    Ok(format!("Veritabanı yedeği başarıyla alındı: {}", hedef_yol))
}
