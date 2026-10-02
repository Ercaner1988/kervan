//! OpenAI ↔ Google Gemini Biçim Çeviricisi
//!
//! OmniRoute translator/request/openai-to-gemini.ts ve response/gemini-to-openai.ts
//! kurallarının saf Rust uygulaması.

use anyhow::Result;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::modeller::*;

/// OpenAI istek gövdesini Google Gemini GenerateContent istek gövdesine dönüştürür.
/// - Sistem mesajlarını `system_instruction` alanına taşır.
/// - OpenAI 'assistant' rolünü Gemini 'model' rolüne dönüştürür.
/// - Arka arkaya gelen aynı rollü mesajları tek bir içerikte birleştirir (Gemini kuralı).
pub fn openai_den_gemini_istegi(istek: &OpenAiIstek) -> Result<GeminiIstek> {
    let mut sistem_parcalari = Vec::new();
    let mut ham_icerikler: Vec<GeminiIcerik> = Vec::new();

    for m in &istek.messages {
        if m.role == "system" {
            if let Some(ref icerik) = m.content
                && !icerik.trim().is_empty()
            {
                sistem_parcalari.push(GeminiParca {
                    text: icerik.clone(),
                });
            }
        } else {
            let rol = if m.role == "assistant" {
                "model"
            } else {
                "user"
            };
            if let Some(ref icerik) = m.content
                && !icerik.is_empty()
            {
                ham_icerikler.push(GeminiIcerik {
                    role: rol.to_string(),
                    parts: vec![GeminiParca {
                        text: icerik.clone(),
                    }],
                });
            }
        }
    }

    // Arka arkaya gelen aynı rollü içerikleri birleştir (Gemini API şartı)
    let mut birlestirilmis_icerikler: Vec<GeminiIcerik> = Vec::new();
    for icerik in ham_icerikler {
        if let Some(son) = birlestirilmis_icerikler.last_mut()
            && son.role == icerik.role
        {
            son.parts.extend(icerik.parts);
            continue;
        }
        birlestirilmis_icerikler.push(icerik);
    }

    let system_instruction = if sistem_parcalari.is_empty() {
        None
    } else {
        Some(GeminiIcerik {
            role: "user".to_string(),
            parts: sistem_parcalari,
        })
    };

    let generation_config =
        if istek.temperature.is_some() || istek.top_p.is_some() || istek.max_tokens.is_some() {
            Some(GeminiAyar {
                temperature: istek.temperature,
                top_p: istek.top_p,
                max_output_tokens: istek.max_tokens,
            })
        } else {
            None
        };

    Ok(GeminiIstek {
        contents: birlestirilmis_icerikler,
        system_instruction,
        generation_config,
    })
}

/// Google Gemini GenerateContent yanıtını OpenAI Chat Completion yanıt biçimine dönüştürür.
pub fn gemini_den_openai_yaniti(yanit: &GeminiYanit, istenen_model: &str) -> Result<OpenAiYanit> {
    let simdi = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut metin_toplami = String::new();
    let mut finish_reason = Some("stop".to_string());

    if let Some(ref adaylar) = yanit.candidates
        && let Some(ilk_aday) = adaylar.first()
    {
        if let Some(ref icerik) = ilk_aday.content {
            for parca in &icerik.parts {
                metin_toplami.push_str(&parca.text);
            }
        }

        if let Some(ref sebep) = ilk_aday.finish_reason {
            finish_reason = Some(match sebep.as_str() {
                "STOP" => "stop".to_string(),
                "MAX_TOKENS" => "length".to_string(),
                "SAFETY" => "content_filter".to_string(),
                other => other.to_ascii_lowercase(),
            });
        }
    }

    let secenek = OpenAiSecenek {
        index: 0,
        message: OpenAiMesaj {
            role: "assistant".to_string(),
            content: Some(metin_toplami),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
        finish_reason,
    };

    let kullanim = yanit.usage_metadata.as_ref().map(|u| OpenAiKullanim {
        prompt_tokens: u.prompt_token_count.unwrap_or(0),
        completion_tokens: u.candidates_token_count.unwrap_or(0),
        total_tokens: u.total_token_count.unwrap_or(0),
    });

    Ok(OpenAiYanit {
        id: format!("chatcmpl-gemini-{}", simdi),
        object: "chat.completion".to_string(),
        created: simdi,
        model: istenen_model.to_string(),
        choices: vec![secenek],
        usage: kullanim,
    })
}

/// Google Gemini akış parçasını OpenAI Chat Completion Chunk biçimine dönüştürür.
pub fn gemini_den_openai_parcasi(
    yanit: &GeminiYanit,
    istek_id: &str,
    model: &str,
) -> Result<Option<OpenAiParca>> {
    let simdi = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut metin = String::new();
    let mut finish_reason = None;

    if let Some(ref adaylar) = yanit.candidates
        && let Some(ilk_aday) = adaylar.first()
    {
        if let Some(ref icerik) = ilk_aday.content {
            for parca in &icerik.parts {
                metin.push_str(&parca.text);
            }
        }
        if let Some(ref sebep) = ilk_aday.finish_reason {
            finish_reason = Some(match sebep.as_str() {
                "STOP" => "stop".to_string(),
                "MAX_TOKENS" => "length".to_string(),
                other => other.to_ascii_lowercase(),
            });
        }
    }

    if metin.is_empty() && finish_reason.is_none() {
        return Ok(None);
    }

    Ok(Some(OpenAiParca {
        id: istek_id.to_string(),
        object: "chat.completion.chunk".to_string(),
        created: simdi,
        model: model.to_string(),
        choices: vec![OpenAiParcaSecenek {
            index: 0,
            delta: OpenAiDelta {
                role: None,
                content: if metin.is_empty() { None } else { Some(metin) },
            },
            finish_reason,
        }],
    }))
}
