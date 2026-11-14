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
//! KAPSAM SINIRI (dürüstçe): akış (SSE) çevirisi YOK DEĞİL, ama çekirdekten
//! akışlı gelmiyor. İstemci `stream: true` isterse çekirdek akışsız koşturulur
//! ve tamamlanmış yanıt Anthropic olay dizisine (message_start →
//! content_block_* → message_delta → message_stop) dökülür. Bu, protokolü
//! karşılar; kazanç ilk belirteci erken göndermek değil, yalnızca biçim
//! uyumudur. Araç (`tools`) taşıyan istekler çevrilir; desteklenmeyen içerik
//! bloğu açık hataya düşer — sessizce düşürülmez.

use std::sync::Arc;

use axum::Json;
use axum::body::to_bytes;
use axum::extract::State;
use axum::http::{StatusCode, header};
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

    // 3. Çekirdeğe ver. `stream` bilerek false: çekirdek akışı bu katmandan
    //    geçmiyor; tamamlanmış yanıt aşağıda Anthropic olay dizisine dökülür.
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
        Ok(yanit) => {
            if istek.stream {
                akisli_yanit(&yanit)
            } else {
                Json(yanit).into_response()
            }
        }
        Err(e) => hata(StatusCode::BAD_GATEWAY, &format!("yanit cevrilemedi: {e}")),
    }
}

/// Anthropic olay dizisini tek seferde üretir (`text/event-stream`).
///
/// NEDEN VAR (2026-11-14): Claude Code `stream: true` gönderiyor. Akışsız JSON
/// dönerse istemci gövdeyi olay olarak okuyamıyor ve turu düşürüyor. Çekirdek
/// akışı bu katmandan geçmediği için olaylar tamamlanmış yanıttan kurulur:
/// protokol karşılanır, ilk belirteç erken gitmez.
fn akisli_yanit(yanit: &kervan_ceviri::ClaudeYanit) -> Response {
    let mut govde = String::with_capacity(512 + yanit.content.len() * 256);

    let baslangic = serde_json::json!({
        "type": "message_start",
        "message": {
            "id": yanit.id,
            "type": "message",
            "role": "assistant",
            "content": [],
            "model": yanit.model,
            "stop_reason": null,
            "stop_sequence": null,
            "usage": {"input_tokens": 0, "output_tokens": 0},
        },
    });
    olay(&mut govde, "message_start", &baslangic);

    for (i, blok) in yanit.content.iter().enumerate() {
        match blok.r#type.as_str() {
            "tool_use" => {
                let acilis = serde_json::json!({
                    "type": "content_block_start",
                    "index": i,
                    "content_block": {
                        "type": "tool_use",
                        "id": blok.id,
                        "name": blok.name,
                        "input": {},
                    },
                });
                olay(&mut govde, "content_block_start", &acilis);
                let arguman = blok
                    .input
                    .clone()
                    .unwrap_or(serde_json::json!({}))
                    .to_string();
                let delta = serde_json::json!({
                    "type": "content_block_delta",
                    "index": i,
                    "delta": {"type": "input_json_delta", "partial_json": arguman},
                });
                olay(&mut govde, "content_block_delta", &delta);
            }
            _ => {
                let acilis = serde_json::json!({
                    "type": "content_block_start",
                    "index": i,
                    "content_block": {"type": "text", "text": ""},
                });
                olay(&mut govde, "content_block_start", &acilis);
                if !blok.text.is_empty() {
                    let delta = serde_json::json!({
                        "type": "content_block_delta",
                        "index": i,
                        "delta": {"type": "text_delta", "text": blok.text},
                    });
                    olay(&mut govde, "content_block_delta", &delta);
                }
            }
        }
        let kapanis = serde_json::json!({"type": "content_block_stop", "index": i});
        olay(&mut govde, "content_block_stop", &kapanis);
    }

    let bitis = serde_json::json!({
        "type": "message_delta",
        "delta": {
            "stop_reason": yanit.stop_reason.clone().unwrap_or_else(|| "end_turn".into()),
            "stop_sequence": null,
        },
        "usage": {"output_tokens": 0},
    });
    olay(&mut govde, "message_delta", &bitis);
    olay(
        &mut govde,
        "message_stop",
        &serde_json::json!({"type": "message_stop"}),
    );

    ([(header::CONTENT_TYPE, "text/event-stream")], govde).into_response()
}

/// Tek bir SSE olayı yazar: `event: <ad>\ndata: <json>\n\n`.
fn olay(govde: &mut String, ad: &str, veri: &serde_json::Value) {
    use std::fmt::Write as _;
    let _ = write!(govde, "event: {ad}\ndata: {veri}\n\n");
}
