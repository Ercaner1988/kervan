//! Anthropic Messages ucu: `POST /v1/messages`.
//!
//! NEDEN VAR (2026-11-12): Claude Code ve Claude Desktop istemcileri OpenAI
//! biçimi konuşmaz; `ANTHROPIC_BASE_URL` ile yönlendirildiklerinde `POST
//! /v1/messages` çağırırlar (ölçüldü: `ua=claude-cli/2.1.322 (external,
//! sdk-cli, agent-sdk/0.3.316)`, `POST /v1/messages?beta=true`). Kervan'ın
//! çok hesaplı yönlendirmesini bu istemcilere açmanın tek yolu bu uçtur.
//!
//! TASARIM: uç kendi seçim mantığını YAZMAZ. Gövde Anthropic biçiminden OpenAI
//! biçimine çevrilir, çekirdek `/v1/chat/completions` yoluna verilir, dönen
//! yanıt yeniden Anthropic biçimine çevrilir. Böylece kota defteri, devre
//! kesici, `auto` aday sıralaması ve hesap sabitleme tek yerde kalır.
//!
//! KAPSAM SINIRI (dürüstçe): akış (SSE) çevirisi HENÜZ YOK. İstemci
//! `stream: true` istese de yanıt tek parça JSON olarak döner; Anthropic
//! istemcileri bunu kabul eder ama ilk belirteci bekleme süresi uzar. Araç
//! (`tools`) taşıyan istekler çeviricide açık hataya düşer — sessizce
//! düşürülmez.

use std::sync::Arc;

use axum::Json;
use axum::body::to_bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use kervan_ceviri::{ClaudeIstek, claude_den_openai_istegi, openai_den_claude_yaniti};

use crate::server::{AppState, hata};

/// Anthropic istemcisinin gönderdiği model adını Kervan model kimliğine çevirir.
///
/// Üç kademe, her biri açık: (1) ad Kervan'da tanımlıysa aynen; (2) değilse
/// `KERVAN_ANTHROPIC_MODEL`; (3) o da yoksa `auto`.
fn model_hedefi(gelen: &str) -> String {
    let ad = gelen.trim();
    if ad.is_empty() || ad == "auto" || ad.starts_with("kervan/") {
        return if ad.is_empty() {
            "auto".to_string()
        } else {
            ad.to_string()
        };
    }
    match std::env::var("KERVAN_ANTHROPIC_MODEL") {
        Ok(hedef) if !hedef.trim().is_empty() => hedef.trim().to_string(),
        _ => "auto".to_string(),
    }
}

/// `POST /v1/messages` işleyicisi.
///
/// Ham `serde_json::Value` alınır: Anthropic istemcileri `metadata`,
/// `thinking`, `service_tier` gibi Kervan'ın bilmediği alanlar gönderir ve
/// katı bir `Deserialize` bunları reddedip istemciyi kırardı.
pub(crate) async fn messages(
    State(st): State<Arc<AppState>>,
    Json(gelen): Json<serde_json::Value>,
) -> Response {
    // 1. Anthropic gövdesini ayrıştır. Ayrıştırma hatası sessizce yutulmaz.
    let istek: ClaudeIstek = match serde_json::from_value(gelen.clone()) {
        Ok(i) => i,
        Err(e) => {
            return hata(
                StatusCode::BAD_REQUEST,
                &format!("Anthropic istegi cozulemedi: {e}"),
            );
        }
    };

    if istek.messages.is_empty() {
        return hata(StatusCode::BAD_REQUEST, "messages bos olamaz");
    }

    // 2. Model kimliği çözülür. Anthropic istemcileri `claude-opus-5-5`,
    //    `claude-sonnet-5-5` gibi KENDİ adlarını gönderir; bu adlar Kervan
    //    kayıt defterinde yoktur ve çekirdek açık hata döndürürdü.
    //
    //    KARAR (2026-11-12): istemcinin gönderdiği Claude adı HEDEF SEÇMEK
    //    İÇİN KULLANILMAZ. Hangi hesabın hangi modele gideceği bir yönlendirme
    //    kararıdır ve Kervan'ın kendi mekanizmasına (`auto`, `[rest.*]`,
    //    `kervan/<saglayici>/<kod>`) aittir. Bu yüzden:
    //      - Ad Kervan'da TANIMLIYSA (`kervan/...` ya da `auto`) aynen geçer.
    //      - Tanımlı değilse `KERVAN_ANTHROPIC_MODEL` ortam değişkenine düşer.
    //      - O da yoksa `auto` kullanılır.
    //    Böylece `claude-opus-5-5` istemek "Kervan'ın seçtiği hesap" demek olur;
    //    sessizce bir sağlayıcıya sabitlenmez.
    let model_kervan = model_hedefi(&istek.model);
    let openai_istek = match claude_den_openai_istegi(&istek, &model_kervan) {
        Ok(i) => i,
        Err(e) => {
            return hata(StatusCode::BAD_REQUEST, &format!("istek cevrilemedi: {e}"));
        }
    };

    let govde = match serde_json::to_value(&openai_istek) {
        Ok(v) => v,
        Err(e) => {
            return hata(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("istek yazilamadi: {e}"),
            );
        }
    };

    // 3. Çekirdeğe ver. `stream` bilerek false: akış çevirisi henüz yok.
    let mut govde = govde;
    if govde.get("stream").is_some() {
        govde["stream"] = serde_json::Value::Bool(false);
    }

    let cekirdek = crate::chat::chat(State(st), Json(govde)).await;

    // 4. Çekirdek yanıtı Anthropic biçimine çevrilir. Hata yanıtları olduğu
    //    gibi geçer: Anthropic istemcisi de `{"error":{"message":...}}` okur.
    if cekirdek.status() != StatusCode::OK {
        return cekirdek;
    }

    let bayt = match to_bytes(cekirdek.into_body(), usize::MAX).await {
        Ok(b) => b,
        Err(e) => {
            return hata(
                StatusCode::BAD_GATEWAY,
                &format!("cekirdek yaniti toplanamadi: {e}"),
            );
        }
    };

    let openai_yanit: kervan_ceviri::OpenAiYanit = match serde_json::from_slice(&bayt) {
        Ok(y) => y,
        Err(e) => {
            return hata(
                StatusCode::BAD_GATEWAY,
                &format!("cekirdek yaniti cozulemedi: {e}"),
            );
        }
    };

    match openai_den_claude_yaniti(&openai_yanit, &istek.model) {
        Ok(yanit) => Json(yanit).into_response(),
        Err(e) => hata(StatusCode::BAD_GATEWAY, &format!("yanit cevrilemedi: {e}")),
    }
}
