pub mod durum;
pub mod guvenlik;
pub mod modeller;
pub mod sohbet;
pub mod yerlestirmeler;

pub use durum::V1Durum;
pub use guvenlik::GuvenlikPolitikasi;

use axum::{
    Router,
    routing::{get, post},
};
use std::sync::Arc;

/// /v1 rotalarını içeren Axum Router oluşturur
pub fn v1_router(durum: Arc<V1Durum>) -> Router {
    Router::new()
        .route("/v1/models", get(modeller::modeller_listesi))
        .route("/v1/chat/completions", post(sohbet::sohbet_tamamlama))
        .route("/v1/embeddings", post(yerlestirmeler::yerlestirmeler))
        .with_state(durum)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode, header},
    };
    use http_body_util::BodyExt;
    use serde_json::json;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_v1_models_listesi() {
        let durum = Arc::new(V1Durum::default());
        let app = v1_router(durum);

        let istek = Request::builder()
            .uri("/v1/models")
            .method("GET")
            .body(Body::empty())
            .unwrap();

        let yanit = app.oneshot(istek).await.unwrap();
        assert_eq!(yanit.status(), StatusCode::OK);

        let govde_bayt = yanit.into_body().collect().await.unwrap().to_bytes();
        let json_val: serde_json::Value = serde_json::from_slice(&govde_bayt).unwrap();
        assert_eq!(json_val["object"], "list");
        assert!(json_val["data"].as_array().unwrap().len() >= 6);
    }

    #[tokio::test]
    async fn test_v1_chat_completions_temel() {
        let durum = Arc::new(V1Durum::default());
        let app = v1_router(durum);

        let istek_govdesi = json!({
            "model": "gpt-4o",
            "messages": [
                { "role": "user", "content": "Selam Kervan!" }
            ],
            "stream": false
        });

        let istek = Request::builder()
            .uri("/v1/chat/completions")
            .method("POST")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(istek_govdesi.to_string()))
            .unwrap();

        let yanit = app.oneshot(istek).await.unwrap();
        assert_eq!(yanit.status(), StatusCode::OK);

        let govde_bayt = yanit.into_body().collect().await.unwrap().to_bytes();
        let json_val: serde_json::Value = serde_json::from_slice(&govde_bayt).unwrap();
        assert_eq!(json_val["object"], "chat.completion");
        assert_eq!(json_val["model"], "gpt-4o");
        assert_eq!(json_val["choices"][0]["message"]["role"], "assistant");
    }

    #[tokio::test]
    async fn test_v1_chat_completions_akisi() {
        let durum = Arc::new(V1Durum::default());
        let app = v1_router(durum);

        let istek_govdesi = json!({
            "model": "gemini-2.0-flash",
            "messages": [
                { "role": "user", "content": "Akış testi" }
            ],
            "stream": true
        });

        let istek = Request::builder()
            .uri("/v1/chat/completions")
            .method("POST")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(istek_govdesi.to_string()))
            .unwrap();

        let yanit = app.oneshot(istek).await.unwrap();
        assert_eq!(yanit.status(), StatusCode::OK);
        assert_eq!(
            yanit.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/event-stream"
        );

        let govde_bayt = yanit.into_body().collect().await.unwrap().to_bytes();
        let metin = String::from_utf8(govde_bayt.to_vec()).unwrap();
        assert!(metin.contains("data: [DONE]"));
    }

    #[tokio::test]
    async fn test_v1_embeddings() {
        let durum = Arc::new(V1Durum::default());
        let app = v1_router(durum);

        let istek_govdesi = json!({
            "model": "text-embedding-3-small",
            "input": "Kervan gömme testi",
            "dimensions": 128
        });

        let istek = Request::builder()
            .uri("/v1/embeddings")
            .method("POST")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(istek_govdesi.to_string()))
            .unwrap();

        let yanit = app.oneshot(istek).await.unwrap();
        assert_eq!(yanit.status(), StatusCode::OK);

        let govde_bayt = yanit.into_body().collect().await.unwrap().to_bytes();
        let json_val: serde_json::Value = serde_json::from_slice(&govde_bayt).unwrap();
        assert_eq!(json_val["object"], "list");
        assert_eq!(
            json_val["data"][0]["embedding"].as_array().unwrap().len(),
            128
        );
    }

    #[tokio::test]
    async fn test_guvenlik_api_anahtari_zorunlulugu() {
        let guvenlik = GuvenlikPolitikasi {
            api_anahtari: Some("gizli-kervan-anahtari".to_string()),
            ..Default::default()
        };

        let durum = V1Durum {
            guvenlik,
            ..Default::default()
        };
        let app = v1_router(Arc::new(durum));

        // Anahtarsız istek 401 dönmeli
        let istek = Request::builder()
            .uri("/v1/chat/completions")
            .method("POST")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                json!({ "model": "gpt-4o", "messages": [] }).to_string(),
            ))
            .unwrap();

        let yanit = app.clone().oneshot(istek).await.unwrap();
        assert_eq!(yanit.status(), StatusCode::UNAUTHORIZED);

        // Doğru anahtarlı istek geçmeli
        let istek_dogru = Request::builder()
            .uri("/v1/chat/completions")
            .method("POST")
            .header(header::CONTENT_TYPE, "application/json")
            .header("x-api-key", "gizli-kervan-anahtari")
            .body(Body::from(
                json!({ "model": "gpt-4o", "messages": [{ "role": "user", "content": "hi" }] })
                    .to_string(),
            ))
            .unwrap();

        let yanit_dogru = app.oneshot(istek_dogru).await.unwrap();
        assert_eq!(yanit_dogru.status(), StatusCode::OK);
    }
}
