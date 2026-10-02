use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct YerlestirmeIstegi {
    pub model: String,
    pub input: serde_json::Value,
    #[serde(default)]
    pub dimensions: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct YerlestirmeVerisi {
    pub object: String,
    pub index: usize,
    pub embedding: Vec<f32>,
}

#[derive(Debug, Serialize)]
pub struct YerlestirmeYaniti {
    pub object: String,
    pub data: Vec<YerlestirmeVerisi>,
    pub model: String,
    pub usage: serde_json::Value,
}

pub async fn yerlestirmeler(
    Json(istek): Json<YerlestirmeIstegi>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let metinler = match istek.input {
        serde_json::Value::String(s) => vec![s],
        serde_json::Value::Array(arr) => arr
            .into_iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect(),
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": {
                        "message": "input dizesi veya dize dizisi olmalıdır",
                        "type": "invalid_request_error"
                    }
                })),
            ));
        }
    };

    let dim = istek.dimensions.unwrap_or(1536);
    let mut data = Vec::new();
    let mut toplam_karakter = 0;

    for (i, metin) in metinler.iter().enumerate() {
        toplam_karakter += metin.len();
        // Sentetik / normalleştirilmiş yerleştirme vektörü
        let mut vec = vec![0.0f32; dim];
        if dim > 0 {
            vec[0] = 0.05;
            vec[dim - 1] = 0.02;
        }
        data.push(YerlestirmeVerisi {
            object: "embedding".to_string(),
            index: i,
            embedding: vec,
        });
    }

    let tahmini_token = (toplam_karakter / 4).max(1);

    Ok(Json(YerlestirmeYaniti {
        object: "list".to_string(),
        data,
        model: istek.model,
        usage: json!({
            "prompt_tokens": tahmini_token,
            "total_tokens": tahmini_token
        }),
    }))
}
