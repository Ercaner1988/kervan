//! Hesap ekleme isteklerinin dogrulanmasi.
//!
//! Neden ayri dosya: `server.rs` uclari tasiyor ve 1500 satir sandik sinirinda;
//! hesap isteginin dogrulanmasi (eposta bicimi, saglayici adi) uclardan bagimsiz
//! bir is. Burada yalniz SAF karar var, yan etki yok.

use std::sync::Arc;

use axum::http::StatusCode;
use axum::response::Response;
use serde::Deserialize;

use crate::server::{AppState, hata};

/// Panelin hesap ekleme istegi. `eposta` yalniz profil adini turetir ve giris
/// sayfasina on-doldurulur; sifre Kervan'a hic girmez.
#[derive(Debug, Deserialize)]
pub struct HesapIste {
    pub eposta: String,
    pub saglayici: String,
}

/// Eposta gibi gorunuyor mu? Agir bir dogrulama degil, yazim hatasi filtresi:
/// `@` ve iki yaninda bosluk olmayan bir metin. Yanlis eposta sessizce bos bir
/// profil acar ve "giris yapilmadi" diye anlasilmaz bir hata verirdi.
pub fn eposta_gecerli_mi(eposta: &str) -> bool {
    let e = eposta.trim();
    if e.chars().any(|c| c.is_whitespace()) || e.len() > 254 {
        return false;
    }
    match e.split_once('@') {
        Some((ad, alan)) => {
            !ad.is_empty() && !alan.is_empty() && alan.contains('.') && !alan.ends_with('.')
        }
        None => false,
    }
}

/// `{eposta, saglayici}` → (profil, saglayici tanimi) ya da istemciye donen hata.
///
/// `Response`'u kutulamaya gerek yok: bu fonksiyon istek basina bir kez cagrilir,
/// sicak yol degil; `Box` yalniz cagri yerlerini dolayli hale getirirdi.
#[allow(
    clippy::result_large_err,
    reason = "istek basina bir kez cagrilir, sicak yol degil; Box yalniz cagri yerlerini dolayli hale getirirdi"
)]
pub fn hesap_hedefi(
    st: &Arc<AppState>,
    iste: &HesapIste,
) -> Result<(String, Arc<crate::browser::Provider>), Response> {
    if !eposta_gecerli_mi(&iste.eposta) {
        return Err(hata(
            StatusCode::BAD_REQUEST,
            "eposta gecersiz: 'ad@alan' biciminde olmali",
        ));
    }
    let saglayici = iste.saglayici.trim();
    match st.saglayicilar.get(saglayici) {
        Some(p) => Ok((crate::kurulum::profil_adi(&iste.eposta), p.clone())),
        None => {
            let mut adlar: Vec<&String> = st.saglayicilar.keys().collect();
            adlar.sort();
            let liste = adlar
                .into_iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            Err(hata(
                StatusCode::BAD_REQUEST,
                &format!("bilinmeyen saglayici '{saglayici}'. Tanimli: {liste}"),
            ))
        }
    }
}
