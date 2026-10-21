//! Eklenti köprüsü uçları: WebSocket, token dağıtımı ve sekme listesi.
//!
//! `server.rs` 1500 satır sınırını aşmasın diye ayrıldı; davranış aynı.
//! Kimlik denetimi (`Host`, `Origin`, token) burada, soket yükseltmesinden
//! ÖNCE yapılır.

use std::sync::Arc;

use axum::Json;
use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::http::header::{self, HeaderMap, HeaderValue};
use axum::response::{IntoResponse, Response};
use futures_util::{SinkExt, StreamExt};

use crate::server::AppState;

pub(crate) async fn ws_eklenti_handler(
    State(st): State<Arc<AppState>>,
    Query(sorgu): Query<std::collections::HashMap<String, String>>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok());
    if !crate::eklenti::host_gecerli_mi(host) || !crate::eklenti::origin_gecerli_mi(origin) {
        return (StatusCode::FORBIDDEN, "yetkisiz kaynak").into_response();
    }
    let token = sorgu
        .get("token")
        .map(String::as_str)
        .or_else(|| headers.get("x-kervan-token").and_then(|v| v.to_str().ok()));
    if !token.is_some_and(|t| crate::eklenti::token_esit_mi(t, st.eklenti_hub.token())) {
        return (StatusCode::UNAUTHORIZED, "gecersiz token").into_response();
    }
    ws.on_upgrade(move |socket| handle_eklenti_socket(socket, st.eklenti_hub.clone()))
}

/// Eklentinin token'ı alacağı uç. `Origin` zorunlu ve uzantı şeması olmalı:
/// düz bir web sayfası buradan token okuyamaz, dolayısıyla `/ws/eklenti`'ye de
/// bağlanamaz.
pub(crate) async fn jeton(State(st): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) else {
        return (StatusCode::FORBIDDEN, "yetkisiz kaynak").into_response();
    };
    if !crate::eklenti::host_gecerli_mi(host) || !crate::eklenti::origin_gecerli_mi(Some(origin)) {
        return (StatusCode::FORBIDDEN, "yetkisiz kaynak").into_response();
    }
    let mut yanit = Json(serde_json::json!({ "token": st.eklenti_hub.token() })).into_response();
    let h = yanit.headers_mut();
    if let Ok(deger) = origin.parse() {
        h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, deger);
    }
    h.insert(header::VARY, HeaderValue::from_static("Origin"));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h.insert(
        header::HeaderName::from_static("access-control-allow-private-network"),
        HeaderValue::from_static("true"),
    );
    yanit
}

/// CORS ön denetimi (özel başlık kullanılmıyor ama tarayıcı sürümleri
/// private-network isteğinde bunu sorabiliyor).
pub(crate) async fn on_denetim(headers: HeaderMap) -> Response {
    let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) else {
        return StatusCode::FORBIDDEN.into_response();
    };
    if !crate::eklenti::origin_gecerli_mi(Some(origin)) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let mut yanit = StatusCode::NO_CONTENT.into_response();
    let h = yanit.headers_mut();
    if let Ok(deger) = origin.parse() {
        h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, deger);
    }
    h.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, OPTIONS"),
    );
    h.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("content-type"),
    );
    h.insert(header::VARY, HeaderValue::from_static("Origin"));
    h.insert(
        header::HeaderName::from_static("access-control-allow-private-network"),
        HeaderValue::from_static("true"),
    );
    yanit
}

async fn handle_eklenti_socket(socket: WebSocket, hub: crate::eklenti::EklentiHub) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (tx_giden, mut rx_giden) = tokio::sync::mpsc::channel::<crate::eklenti::GidenMesaj>(
        crate::eklenti::GIDEN_KUYRUK_KAPASITESI,
    );

    let mut kayitli: Vec<(String, String)> = Vec::new();

    // Kervan'dan gelen komutları eklenti WebSocket'ine basan arka plan görevi
    let giden_gorev = tokio::spawn(async move {
        while let Some(msg) = rx_giden.recv().await {
            if let Ok(json) = serde_json::to_string(&msg)
                && ws_tx.send(WsMessage::Text(json.into())).await.is_err()
            {
                break;
            }
        }
    });

    // Eklentiden gelen mesajları okuyan döngü
    while let Some(Ok(msg)) = ws_rx.next().await {
        if let WsMessage::Text(text) = msg
            && let Ok(gelen) = serde_json::from_str::<crate::eklenti::GelenMesaj>(&text)
        {
            if let crate::eklenti::GelenMesaj::Kayit {
                ref provider,
                ref profile,
                ..
            } = gelen
            {
                let prof = profile.clone().unwrap_or_else(|| "varsayilan".into());
                if !kayitli.iter().any(|(p, f)| p == provider && f == &prof) {
                    kayitli.push((provider.clone(), prof.clone()));
                }
                hub.sekme_kaydet(provider.clone(), prof, tx_giden.clone())
                    .await;
            }
            hub.mesaji_isle(gelen).await;
        }
    }

    // Yalnız bu soketin kaydettiği hesapları düşür; araya giren yeni bir
    // bağlantının aynı anahtarı tazelemiş olma ihtimali `ayril_eger` ile korunur.
    for (provider, profile) in kayitli {
        hub.sekme_ayril_eger(&provider, &profile, &tx_giden).await;
    }
    giden_gorev.abort();
}

pub(crate) async fn eklenti_sekmeler(State(st): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let sekmeler = st.eklenti_hub.bagli_sekmeler().await;
    let hesaplar = st.eklenti_hub.bagli_hesaplar().await;
    Json(serde_json::json!({
        "bagli_sekmeler": sekmeler,
        "hesaplar": hesaplar,
        "sayi": hesaplar.len(),
    }))
}
