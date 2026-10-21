//! Doğrudan REST yukarı akışı ve yukarı akış hatasının HTTP yanıtına çevrilmesi.
//!
//! `server.rs` 1500 satır sınırını aşmasın diye ayrıldı; davranış aynı.

use std::sync::Arc;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::server::{AppState, hata};

/// Yukarı akış hatası sağlayıcının mı suçu? Yalnız bunlar devre kesiciyi ve
/// LKGP gecikme tablosunu besler. Anahtar eksikliği, istemci JSON hatası ve
/// "devre zaten açık" yanıtı sağlıklı sağlayıcıyı kapatmamalı.
fn saglayici_arizasi(e: &kervan_yukari::YukariHatasi) -> bool {
    use kervan_yukari::YukariHatasi as H;
    matches!(e, H::AgHatasi(_))
        || matches!(e, H::HttpHatasi(429, _, _) | H::HttpHatasi(500..=599, _, _))
}

/// Aynı istek başka bir adayda yeniden denenmeli mi?
fn yeniden_denenebilir(e: &kervan_yukari::YukariHatasi) -> bool {
    use kervan_yukari::YukariHatasi as H;
    match e {
        // İstemcinin isteği bozuk: başka sağlayıcıda da bozuk olur.
        H::Desteklenmiyor(_) | H::JsonHatasi(_) => false,
        H::HttpHatasi(kod, _, _) => *kod == 408 || *kod == 429 || *kod >= 500,
        // Anahtar eksikliği bu hedefin yapılandırma sorunu; sıradaki denenebilir.
        _ => true,
    }
}

/// REST yukarı akışına devret ve sonucu akıllı yönlendiriciye bildir.
///
/// Başarı/hata burada yazılır: devre kesici ve LKGP gecikme tablosu tarayıcı
/// yoluyla aynı adları kullandığı için REST hedefleri de aynı havuzda
/// değerlendirilir. Akışta başarı **akış bittiğinde** yazılır; el sıkışmada
/// değil. Akış ortasında kopma başarı sayılmaz.
#[allow(
    clippy::result_large_err,
    reason = "hata yaniti istemciye aynen doner; Boxing cagri yerlerini karmasiklastirirdi"
)]
pub(crate) async fn rest_denemesi(
    st: &Arc<AppState>,
    hedef: &kervan_yukari::Hedef,
    raw_val: &serde_json::Value,
    alt_model: Option<&str>,
    stream: bool,
) -> Result<Response, (Response, bool)> {
    let t = std::time::Instant::now();
    if stream {
        match st.yukari.gonder_akis(hedef, raw_val, alt_model).await {
            Ok(mut rx) => {
                let st_bitis = st.clone();
                let ad = hedef.ad.clone();
                let baslangic = t;
                let mut bitti = false;
                let mut hata = false;
                let stream = futures_util::stream::poll_fn(move |cx| {
                    match rx.poll_recv(cx) {
                        std::task::Poll::Ready(Some(Ok(parca))) => {
                            if parca.contains("[DONE]") {
                                bitti = true;
                            }
                            std::task::Poll::Ready(Some(Ok(axum::body::Bytes::from(parca))))
                        }
                        std::task::Poll::Ready(Some(Err(e))) => {
                            hata = true;
                            std::task::Poll::Ready(Some(Err(std::io::Error::other(e.to_string()))))
                        }
                        std::task::Poll::Ready(None) => {
                            // Akışın gerçek sonucu burada bilinir. Kayıt ayrı
                            // göreve bırakılır: poll içinde await edilemez.
                            let st2 = st_bitis.clone();
                            let ad2 = ad.clone();
                            let gecen = baslangic.elapsed().as_millis() as u64;
                            tokio::spawn(async move {
                                if bitti && !hata {
                                    st2.router.record_success(&ad2, gecen).await;
                                } else {
                                    st2.router.record_failure(&ad2).await;
                                }
                            });
                            std::task::Poll::Ready(None)
                        }
                        std::task::Poll::Pending => std::task::Poll::Pending,
                    }
                });
                let body = axum::body::Body::from_stream(stream);
                let mut resp = Response::new(body);
                *resp.status_mut() = StatusCode::OK;
                resp.headers_mut().insert(
                    axum::http::header::CONTENT_TYPE,
                    axum::http::HeaderValue::from_static("text/event-stream"),
                );
                resp.headers_mut().insert(
                    axum::http::header::CACHE_CONTROL,
                    axum::http::HeaderValue::from_static("no-cache"),
                );
                resp.headers_mut().insert(
                    axum::http::header::CONNECTION,
                    axum::http::HeaderValue::from_static("keep-alive"),
                );
                Ok(resp)
            }
            Err(e) => {
                let yeniden = yeniden_denenebilir(&e);
                if saglayici_arizasi(&e) {
                    st.router.record_failure(&hedef.ad).await;
                }
                Err((yukari_hatasini_don(e), yeniden))
            }
        }
    } else {
        match st.yukari.gonder(hedef, raw_val, alt_model).await {
            Ok(yanit) => {
                st.router
                    .record_success(&hedef.ad, t.elapsed().as_millis() as u64)
                    .await;
                Ok((StatusCode::OK, Json(yanit)).into_response())
            }
            Err(e) => {
                let yeniden = yeniden_denenebilir(&e);
                if saglayici_arizasi(&e) {
                    st.router.record_failure(&hedef.ad).await;
                }
                Err((yukari_hatasini_don(e), yeniden))
            }
        }
    }
}

fn yukari_hatasini_don(err: kervan_yukari::YukariHatasi) -> Response {
    match err {
        kervan_yukari::YukariHatasi::AnahtarYok(env_ad) => hata(
            StatusCode::SERVICE_UNAVAILABLE,
            &format!(
                "Sağlayıcı API anahtarı bulunamadı: Ortam değişkeni '{}' tanımlı değil",
                env_ad
            ),
        ),
        kervan_yukari::YukariHatasi::Desteklenmiyor(alan) => hata(
            StatusCode::NOT_IMPLEMENTED,
            &format!("desteklenmiyor: {}", alan),
        ),
        kervan_yukari::YukariHatasi::DevreAcik { ad, kalan_sure_ms } => hata(
            StatusCode::SERVICE_UNAVAILABLE,
            &format!(
                "Devre kesici açık: {} ({}ms sonra tekrar denenecek)",
                ad, kalan_sure_ms
            ),
        ),
        kervan_yukari::YukariHatasi::Sogumada {
            ad,
            retry_after_saniye,
        } => {
            let mut resp = hata(
                StatusCode::TOO_MANY_REQUESTS,
                &format!(
                    "Sağlayıcı soğumada: {} (Retry-After: {}s)",
                    ad, retry_after_saniye
                ),
            );
            if let Ok(val) = axum::http::HeaderValue::from_str(&retry_after_saniye.to_string()) {
                resp.headers_mut()
                    .insert(axum::http::header::RETRY_AFTER, val);
            }
            resp
        }
        kervan_yukari::YukariHatasi::HttpHatasi(429, govde, retry_after) => {
            let mut resp = hata(
                StatusCode::TOO_MANY_REQUESTS,
                &format!("Yukarı akış 429 hatası: {}", govde),
            );
            if let Some(sn) = retry_after
                && let Ok(val) = axum::http::HeaderValue::from_str(&sn.to_string())
            {
                resp.headers_mut()
                    .insert(axum::http::header::RETRY_AFTER, val);
            }
            resp
        }
        kervan_yukari::YukariHatasi::HttpHatasi(durum, govde, _) => {
            let status = StatusCode::from_u16(durum).unwrap_or(StatusCode::BAD_GATEWAY);
            hata(
                status,
                &format!("Yukarı akış HTTP hatası ({}): {}", durum, govde),
            )
        }
        kervan_yukari::YukariHatasi::AgHatasi(m) => hata(
            StatusCode::BAD_GATEWAY,
            &format!("Yukarı akış ağ bağlantı hatası: {}", m),
        ),
        kervan_yukari::YukariHatasi::JsonHatasi(m) => hata(
            StatusCode::BAD_GATEWAY,
            &format!("Yukarı akış JSON hatası: {}", m),
        ),
    }
}
