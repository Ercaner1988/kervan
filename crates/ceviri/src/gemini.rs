//! OpenAI ↔ Google Gemini Biçim Çeviricisi
//!
//! OmniRoute translator/request/openai-to-gemini.ts kurallarının saf Rust uygulaması.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::modeller::*;

/// OpenAI istek gövdesini Google Gemini GenerateContent istek gövdesine dönüştürür.
/// - FR-5.3: `tools` veya çok parçalı içerik varsa yukarı akışa gitmeden açık hata döner.
/// - Sistem mesajları `systemInstruction` alanına taşınır.
/// - OpenAI 'assistant' rolünü Gemini 'model' rolüne dönüştürür.
/// - Arka arkaya gelen aynı rollü mesajları tek bir içerikte birleştirir (Gemini kuralı).
pub fn openai_den_gemini_istegi(
    istek: &OpenAiIstek,
    hedef_model: &str,
) -> Result<GeminiIstek, CeviriHatasi> {
    // 1. Desteklenmeyen alan denetimleri (FR-5.3)
    if let Some(ref tools) = istek.tools
        && !tools.is_empty()
    {
        return Err(CeviriHatasi::Desteklenmiyor("tools".to_string()));
    }

    for m in &istek.messages {
        if let Some(ref c) = m.content
            && !c.is_string()
        {
            return Err(CeviriHatasi::Desteklenmiyor(
                "çok parçalı içerik".to_string(),
            ));
        }
    }

    let mut sistem_parcalari = Vec::new();
    let mut ham_icerikler: Vec<GeminiIcerik> = Vec::new();

    for m in &istek.messages {
        let metin = m.duz_metin().unwrap_or_default();
        if m.role == "system" {
            if !metin.trim().is_empty() {
                sistem_parcalari.push(GeminiParca { text: metin });
            }
        } else {
            let rol = if m.role == "assistant" {
                "model"
            } else {
                "user"
            };
            ham_icerikler.push(GeminiIcerik {
                role: rol.to_string(),
                parts: vec![GeminiParca { text: metin }],
            });
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
        Some(GeminiSystemInstruction {
            role: "system".to_string(),
            parts: sistem_parcalari,
        })
    };

    let max_output_tokens = istek.max_tokens.unwrap_or(65536);
    let default_budget = 24576u32;
    let thinking_budget = default_budget.min(max_output_tokens / 2);

    let thinking_config = Some(GeminiThinkingConfig {
        thinking_budget,
        include_thoughts: true,
    });

    let generation_config = GeminiGenerationConfig {
        max_output_tokens,
        temperature: istek.temperature,
        top_p: istek.top_p,
        thinking_config,
    };

    let safety_settings = vec![
        GeminiSafetySetting {
            category: "HARM_CATEGORY_HATE_SPEECH".to_string(),
            threshold: "OFF".to_string(),
        },
        GeminiSafetySetting {
            category: "HARM_CATEGORY_DANGEROUS_CONTENT".to_string(),
            threshold: "OFF".to_string(),
        },
        GeminiSafetySetting {
            category: "HARM_CATEGORY_SEXUALLY_EXPLICIT".to_string(),
            threshold: "OFF".to_string(),
        },
        GeminiSafetySetting {
            category: "HARM_CATEGORY_HARASSMENT".to_string(),
            threshold: "OFF".to_string(),
        },
    ];

    Ok(GeminiIstek {
        model: hedef_model.to_string(),
        contents: birlestirilmis_icerikler,
        generation_config,
        safety_settings,
        system_instruction,
    })
}

/// Google Gemini GenerateContent yanıtını OpenAI Chat Completion yanıt biçimine dönüştürür.
pub fn gemini_den_openai_yaniti(
    yanit: &GeminiYanit,
    istenen_model: &str,
) -> Result<OpenAiYanit, CeviriHatasi> {
    let simdi = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs(),
        Err(_) => 0,
    };

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
        message: OpenAiMesajCikti {
            role: "assistant".to_string(),
            content: Some(metin_toplami),
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
) -> Result<Option<OpenAiParca>, CeviriHatasi> {
    let simdi = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs(),
        Err(_) => 0,
    };

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
