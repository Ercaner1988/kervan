//! Hesap kasası uçları: CLI kimlik dosyalarını listeleyip aralarında geçiş.
//!
//! NEDEN AYRI DOSYA: `server.rs` uçları taşıyor ve sandık sınırında; kasa
//! mantığının kendisi `kervan-kasa` sandığında. Burada yalnız HTTP kabuğu var:
//! yol ayrıştırma, hata eşleme, engelleyici işi `spawn_blocking`'e taşıma.
//!
//! SIR SINIRI: yanıt gövdesine token, refresh token, JWT ya da kimlik yükü
//! çıkmaz — yalnız `Kayit::ozet()`. Panelin ihtiyacı eposta/plan/kimlik, sır
//! değil. Sır sunucudan çıkarsa eklenti JS'inde yaşamaya başlar; orası
//! denetlenemeyen bir yüzey.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};

use kervan_kasa::{Cesit, Kasa, Kayit};

use crate::server::{AppState, hata};

#[derive(Debug, Deserialize, Default)]
pub struct KimlikIste {
    #[serde(default)]
    pub kimlik: String,
}

#[allow(
    clippy::result_large_err,
    reason = "istek basina bir kez cagrilir, sicak yol degil; Box yalniz cagri yerlerini dolayli hale getirirdi"
)]
fn cesit_coz(ad: &str) -> Result<Cesit, Response> {
    Cesit::from_ad(ad).ok_or_else(|| {
        hata(
            StatusCode::NOT_FOUND,
            &format!("bilinmeyen kasa cesidi '{ad}'. Tanimli: claude, codex, gemini"),
        )
    })
}

fn ozetler(kayitlar: &[Kayit]) -> Vec<Value> {
    kayitlar.iter().map(Kayit::ozet).collect()
}

/// Bir aracın kasa görünümü: kayıtlı hesaplar + dosyalarda duran aktif oturum.
fn gorunum(kasa: &Kasa, cesit: Cesit) -> Value {
    let kayitlar = kasa.topla(cesit);
    let aktif = kasa.aktif(cesit);
    match (kayitlar, aktif) {
        (Ok(kayitlar), Ok(aktif)) => json!({
            "cesit": cesit.ad(),
            "kayitlar": ozetler(&kayitlar),
            "aktif": aktif.as_ref().map(Kayit::ozet),
            "hata": Value::Null,
        }),
        (Err(e), _) | (_, Err(e)) => json!({
            "cesit": cesit.ad(),
            "kayitlar": [],
            "aktif": Value::Null,
            "hata": format!("{e:#}"),
        }),
    }
}

/// GET /kasa — üç aracın tamamı tek yanıtta. Panel tek turda çizer; araç başına
/// ayrı istek atmak üç kat gecikme ve üç ayrı hata durumu demekti.
pub async fn kasa_tumu(State(_st): State<Arc<AppState>>) -> Response {
    let gorunumler = tokio::task::spawn_blocking(|| {
        let kasa = Kasa::varsayilan();
        Cesit::tumu()
            .into_iter()
            .map(|c| gorunum(&kasa, c))
            .collect::<Vec<_>>()
    })
    .await;

    match gorunumler {
        Ok(v) => Json(json!({
            "cesitler": v,
            "kok": Kasa::varsayilan().kok().to_string_lossy(),
        }))
        .into_response(),
        Err(e) => hata(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("kasa okunamadi: {e}"),
        ),
    }
}

/// GET /kasa/{cesit}
pub async fn kasa_tek(State(_st): State<Arc<AppState>>, Path(cesit): Path<String>) -> Response {
    let cesit = match cesit_coz(&cesit) {
        Ok(c) => c,
        Err(r) => return r,
    };
    match tokio::task::spawn_blocking(move || {
        let kasa = Kasa::varsayilan();
        gorunum(&kasa, cesit)
    })
    .await
    {
        Ok(v) => Json(v).into_response(),
        Err(e) => hata(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("kasa okunamadi: {e}"),
        ),
    }
}

/// POST /kasa/{cesit}/topla — dosyalarda duran oturumu kasaya alır.
///
/// Panel bunu "Listeyi tazele" olarak sunar ve geçişten hemen önce çağırır:
/// geçiş, kasada yazılı olanı geri yazdığı için kasa bayatsa araç eski token'a
/// döner. Tazeleme bu tuzağı kapatır.
pub async fn kasa_topla(State(_st): State<Arc<AppState>>, Path(cesit): Path<String>) -> Response {
    let cesit = match cesit_coz(&cesit) {
        Ok(c) => c,
        Err(r) => return r,
    };
    match tokio::task::spawn_blocking(move || {
        let kasa = Kasa::varsayilan();
        let kayitlar = kasa.topla(cesit)?;
        let aktif = kasa.aktif(cesit)?;
        anyhow::Ok((kayitlar, aktif))
    })
    .await
    {
        Ok(Ok((kayitlar, aktif))) => Json(json!({
            "ok": true,
            "cesit": cesit.ad(),
            "kayitlar": ozetler(&kayitlar),
            "aktif": aktif.as_ref().map(Kayit::ozet),
            "mesaj": format!("{} hesap kasada.", kayitlar.len()),
        }))
        .into_response(),
        Ok(Err(e)) => hata(
            StatusCode::BAD_REQUEST,
            &format!("kasa tazelenemedi: {e:#}"),
        ),
        Err(e) => hata(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("kasa tazelenemedi: {e}"),
        ),
    }
}

/// POST /kasa/{cesit}/gecis — hesabı değiştirir.
///
/// Uzun sürebilir: kasa dosyaların sakinleşmesini bekler (250 ms) ve yazdıktan
/// sonra geri okuyup doğrular. Eklenti servisi bu ucu uzun zaman aşımıyla çağırır.
pub async fn kasa_gecis(
    State(_st): State<Arc<AppState>>,
    Path(cesit): Path<String>,
    Json(iste): Json<KimlikIste>,
) -> Response {
    let cesit = match cesit_coz(&cesit) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let kimlik = iste.kimlik.trim().to_string();
    if kimlik.is_empty() {
        return hata(StatusCode::BAD_REQUEST, "kimlik bos olamaz");
    }
    match tokio::task::spawn_blocking(move || Kasa::varsayilan().gecis(cesit, &kimlik)).await {
        Ok(Ok(sonuc)) => Json(json!({
            "ok": true,
            "cesit": cesit.ad(),
            "degisti": sonuc.degisti,
            "kimlik": sonuc.kimlik,
            "eposta": sonuc.eposta,
            "mesaj": sonuc.mesaj,
        }))
        .into_response(),
        // Kayıt yok / dosyalar yazılıyor / doğrulama tutmadı: hepsi istemci
        // tarafından düzeltilebilir durumlar, 500 değil.
        Ok(Err(e)) => hata(StatusCode::CONFLICT, &format!("{e:#}")),
        Err(e) => hata(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("gecis gorevi calistirilamadi: {e}"),
        ),
    }
}

/// POST /kasa/{cesit}/sil
pub async fn kasa_sil(
    State(_st): State<Arc<AppState>>,
    Path(cesit): Path<String>,
    Json(iste): Json<KimlikIste>,
) -> Response {
    let cesit = match cesit_coz(&cesit) {
        Ok(c) => c,
        Err(r) => return r,
    };
    let kimlik = iste.kimlik.trim().to_string();
    if kimlik.is_empty() {
        return hata(StatusCode::BAD_REQUEST, "kimlik bos olamaz");
    }
    let hedef = kimlik.clone();
    match tokio::task::spawn_blocking(move || Kasa::varsayilan().sil(cesit, &hedef)).await {
        Ok(Ok(silindi)) => Json(json!({ "ok": silindi, "kimlik": kimlik })).into_response(),
        Ok(Err(e)) => hata(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("kayit silinemedi: {e:#}"),
        ),
        Err(e) => hata(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("silme gorevi calistirilamadi: {e}"),
        ),
    }
}
