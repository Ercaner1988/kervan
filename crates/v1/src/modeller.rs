use axum::{Json, response::IntoResponse};
use serde_json::json;

/// /v1/models uç noktası
pub async fn modeller_listesi() -> impl IntoResponse {
    let modeller = json!({
        "object": "list",
        "data": [
            {
                "id": "gpt-4o",
                "object": "model",
                "created": 1700000000,
                "owned_by": "openai"
            },
            {
                "id": "gpt-4o-mini",
                "object": "model",
                "created": 1700000000,
                "owned_by": "openai"
            },
            {
                "id": "claude-3-5-sonnet",
                "object": "model",
                "created": 1700000000,
                "owned_by": "anthropic"
            },
            {
                "id": "claude-3-5-haiku",
                "object": "model",
                "created": 1700000000,
                "owned_by": "anthropic"
            },
            {
                "id": "gemini-2.0-flash",
                "object": "model",
                "created": 1700000000,
                "owned_by": "google"
            },
            {
                "id": "gemini-2.5-pro",
                "object": "model",
                "created": 1700000000,
                "owned_by": "google"
            },
            {
                "id": "deepseek-chat",
                "object": "model",
                "created": 1700000000,
                "owned_by": "deepseek"
            },
            {
                "id": "deepseek-reasoner",
                "object": "model",
                "created": 1700000000,
                "owned_by": "deepseek"
            }
        ]
    });

    Json(modeller)
}
