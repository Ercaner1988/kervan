use crate::modeller::{YurutmeHatasi, YurutmeIstegi, YurutmeYaniti};
use serde_json::json;

/// Standart OpenAI Uyumlu REST Yürütücüsü (DeepSeek, Groq, Mistral, Together vb.)
#[derive(Debug, Clone, Default)]
pub struct StandartRestYurutucu;

impl StandartRestYurutucu {
    pub fn istek_govdesi_olustur(&self, istek: &YurutmeIstegi) -> serde_json::Value {
        json!({
            "model": istek.model,
            "messages": [
                { "role": "user", "content": istek.istem }
            ],
            "stream": istek.stream
        })
    }

    pub fn yaniti_isle(&self, json_metni: &str) -> Result<YurutmeYaniti, YurutmeHatasi> {
        let deger: serde_json::Value = serde_json::from_str(json_metni)
            .map_err(|e| YurutmeHatasi::FormatHatasi(e.to_string()))?;

        let metin = deger["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let model = deger["model"].as_str().unwrap_or("bilinmeyen").to_string();
        let token = deger["usage"]["total_tokens"].as_u64().unwrap_or(0) as usize;

        Ok(YurutmeYaniti {
            metin,
            kullanim_token: token,
            model,
        })
    }
}

/// Anthropic Claude Messages Uyumlu REST Yürütücüsü
#[derive(Debug, Clone, Default)]
pub struct ClaudeRestYurutucu;

impl ClaudeRestYurutucu {
    pub fn istek_govdesi_olustur(&self, istek: &YurutmeIstegi) -> serde_json::Value {
        json!({
            "model": istek.model,
            "messages": [
                { "role": "user", "content": istek.istem }
            ],
            "max_tokens": 4096,
            "stream": istek.stream
        })
    }

    pub fn yaniti_isle(&self, json_metni: &str) -> Result<YurutmeYaniti, YurutmeHatasi> {
        let deger: serde_json::Value = serde_json::from_str(json_metni)
            .map_err(|e| YurutmeHatasi::FormatHatasi(e.to_string()))?;

        let mut metin = String::new();
        if let Some(parcalar) = deger["content"].as_array() {
            for p in parcalar {
                if p["type"] == "text"
                    && let Some(t) = p["text"].as_str()
                {
                    metin.push_str(t);
                }
            }
        }

        let model = deger["model"].as_str().unwrap_or("claude").to_string();
        let girdi = deger["usage"]["input_tokens"].as_u64().unwrap_or(0) as usize;
        let cikti = deger["usage"]["output_tokens"].as_u64().unwrap_or(0) as usize;

        Ok(YurutmeYaniti {
            metin,
            kullanim_token: girdi + cikti,
            model,
        })
    }
}

/// Google Gemini Uyumlu REST Yürütücüsü
#[derive(Debug, Clone, Default)]
pub struct GeminiRestYurutucu;

impl GeminiRestYurutucu {
    pub fn istek_govdesi_olustur(&self, istek: &YurutmeIstegi) -> serde_json::Value {
        json!({
            "contents": [
                {
                    "role": "user",
                    "parts": [{ "text": istek.istem }]
                }
            ]
        })
    }

    pub fn yaniti_isle(&self, json_metni: &str) -> Result<YurutmeYaniti, YurutmeHatasi> {
        let deger: serde_json::Value = serde_json::from_str(json_metni)
            .map_err(|e| YurutmeHatasi::FormatHatasi(e.to_string()))?;

        let mut metin = String::new();
        if let Some(adaylar) = deger["candidates"].as_array()
            && let Some(ilk) = adaylar.first()
            && let Some(parcalar) = ilk["content"]["parts"].as_array()
        {
            for p in parcalar {
                if let Some(t) = p["text"].as_str() {
                    metin.push_str(t);
                }
            }
        }

        let token = deger["usageMetadata"]["totalTokenCount"]
            .as_u64()
            .unwrap_or(0) as usize;

        Ok(YurutmeYaniti {
            metin,
            kullanim_token: token,
            model: "gemini".to_string(),
        })
    }
}
