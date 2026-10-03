use axum::{
    Json,
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use kervan_ceviri::{OpenAiIstek, OpenAiKullanim, OpenAiMesajCikti, OpenAiSecenek, OpenAiYanit};
use kervan_yonlendirme::{AnlamsalOnbellek, HizSiniriKarari};
use serde_json::json;
use std::sync::Arc;

use crate::durum::V1Durum;
use crate::guvenlik::GuvenlikPolitikasi;

/// /v1/chat/completions ana işleyicisi
pub async fn sohbet_tamamlama(
    State(durum): State<Arc<V1Durum>>,
    headers: HeaderMap,
    Json(istek): Json<OpenAiIstek>,
) -> Response {
    // 1. Güvenlik politikası kontrolü
    if let Err(yanit) = durum.guvenlik.erisim_kontrol(None, &headers) {
        return *yanit;
    }

    // 2. Hız Sınırı (Rate Limit) kontrolü
    let kimlik = headers
        .get("x-api-key")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("anonim");

    if let HizSiniriKarari::Reddedildi { bekleme_ms } = durum.hiz_sinirlayici.kontrol_et(kimlik) {
        let bekleme_sn = (bekleme_ms / 1000).max(1);
        let hata = json!({
            "error": {
                "message": format!("Hız sınırı aşıldı. Lütfen {} saniye sonra tekrar deneyin.", bekleme_sn),
                "type": "rate_limit_error",
                "code": "rate_limit_exceeded"
            }
        });
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [(header::RETRY_AFTER, bekleme_sn.to_string())],
            Json(hata),
        )
            .into_response();
    }

    // 3. Önbellek (Semantic Cache) sorgusu (stream false ise)
    let ilk_istem = istek
        .messages
        .last()
        .and_then(|m| m.duz_metin())
        .unwrap_or_default();
    let sicaklik = istek.temperature.unwrap_or(0.7) as f32;
    let ozet = AnlamsalOnbellek::ozetle(&istek.model, &ilk_istem, sicaklik);

    if !istek.stream
        && let Some(onbellek_metni) = durum.onbellek.getir(ozet)
    {
        let yanit = OpenAiYanit {
            id: format!("chatcmpl-cache-{}", ozet),
            object: "chat.completion".to_string(),
            created: 1700000000,
            model: istek.model,
            choices: vec![OpenAiSecenek {
                index: 0,
                message: OpenAiMesajCikti {
                    role: "assistant".to_string(),
                    content: Some(onbellek_metni),
                },
                finish_reason: Some("stop".to_string()),
            }],
            usage: Some(OpenAiKullanim {
                prompt_tokens: (ilk_istem.len() / 4).max(1) as u32,
                completion_tokens: 10,
                total_tokens: (ilk_istem.len() / 4).max(1) as u32 + 10,
            }),
        };
        return Json(yanit).into_response();
    }

    // 4. Hedef Sağlayıcı Belirleme ve Yanıt Üretimi
    let saglayici = if istek.model.starts_with("claude") {
        "anthropic"
    } else if istek.model.starts_with("gemini") {
        "gemini"
    } else {
        "openai"
    };

    // Devre Kesici ve Soğuma Kontrolü
    {
        let mut devre = match durum.direnc_devre.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        if let Err(_kesildi) = devre.istek_izni_al() {
            return GuvenlikPolitikasi::guvenli_hata_yaniti(
                StatusCode::SERVICE_UNAVAILABLE,
                "Hizmet geçici olarak devre dışı (devre kesici açık).",
            );
        }
    }

    {
        let soguma = match durum.direnc_soguma.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        if soguma.sogumada_mi(saglayici) {
            let bekleme_sn = soguma.kalan_sure_saniye(saglayici).max(1);
            let hata = json!({
                "error": {
                    "message": format!("Sağlayıcı soğuma durumunda, {} sn bekleyin.", bekleme_sn),
                    "type": "provider_cooldown",
                    "code": "cooldown_active"
                }
            });
            return (
                StatusCode::TOO_MANY_REQUESTS,
                [(header::RETRY_AFTER, bekleme_sn.to_string())],
                Json(hata),
            )
                .into_response();
        }
    }

    // Simüle edilmiş / işlenmiş yanıt metni
    let uretilen_metin = format!("Kervan yanıtı: [{}] modelinden selamlar!", istek.model);

    // Yanıtı önbelleğe kaydet
    if !istek.stream {
        durum.onbellek.kaydet(ozet, uretilen_metin.clone());
    }

    // 5. Akışlı (SSE) veya Tek Parça Yanıt
    if istek.stream {
        let chunk1 = json!({
            "id": "chatcmpl-stream-1",
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": istek.model,
            "choices": [{
                "index": 0,
                "delta": { "role": "assistant", "content": uretilen_metin },
                "finish_reason": null
            }]
        });
        let chunk2 = json!({
            "id": "chatcmpl-stream-1",
            "object": "chat.completion.chunk",
            "created": 1700000000,
            "model": istek.model,
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "stop"
            }]
        });

        let sse_govde = format!(
            "data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
            serde_json::to_string(&chunk1).unwrap_or_default(),
            serde_json::to_string(&chunk2).unwrap_or_default()
        );

        Response::builder()
            .header(header::CONTENT_TYPE, "text/event-stream")
            .header(header::CACHE_CONTROL, "no-cache")
            .header(header::CONNECTION, "keep-alive")
            .body(Body::from(sse_govde))
            .unwrap_or_else(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Akış yanıtı oluşturulamadı",
                )
                    .into_response()
            })
    } else {
        let yanit = OpenAiYanit {
            id: "chatcmpl-123".to_string(),
            object: "chat.completion".to_string(),
            created: 1700000000,
            model: istek.model,
            choices: vec![OpenAiSecenek {
                index: 0,
                message: OpenAiMesajCikti {
                    role: "assistant".to_string(),
                    content: Some(uretilen_metin),
                },
                finish_reason: Some("stop".to_string()),
            }],
            usage: Some(OpenAiKullanim {
                prompt_tokens: (ilk_istem.len() / 4).max(1) as u32,
                completion_tokens: 15,
                total_tokens: (ilk_istem.len() / 4).max(1) as u32 + 15,
            }),
        };
        Json(yanit).into_response()
    }
}
