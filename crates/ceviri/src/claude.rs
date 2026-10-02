//! OpenAI ↔ Anthropic Claude Biçim Çeviricisi
//!
//! OmniRoute translator/request/openai-to-claude.ts ve response/claude-to-openai.ts
//! kurallarının saf Rust uygulaması.

use anyhow::Result;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::modeller::*;

/// OpenAI istek gövdesini Anthropic Claude Messages istek gövdesine dönüştürür.
/// - Sistem mesajlarını ayrıştırır ve üst düzey `system` alanına taşır.
/// - Boş metin bloklarını temizler (Anthropic 400 kuralı).
/// - Varsayılan `max_tokens` (4096) atar.
pub fn openai_den_claude_istegi(istek: &OpenAiIstek) -> Result<ClaudeIstek> {
    let mut sistem_mesajlari = Vec::new();
    let mut claude_mesajlar = Vec::new();

    for m in &istek.messages {
        if m.role == "system" {
            if let Some(ref icerik) = m.content
                && !icerik.trim().is_empty()
            {
                sistem_mesajlari.push(icerik.clone());
            }
        } else {
            let rol = if m.role == "assistant" {
                "assistant"
            } else {
                "user"
            };
            let mut icerik_bloklari = Vec::new();

            if let Some(ref icerik) = m.content
                && !icerik.is_empty()
            {
                icerik_bloklari.push(ClaudeIcerikBloku::Metin {
                    text: icerik.clone(),
                });
            }

            if !icerik_bloklari.is_empty() {
                claude_mesajlar.push(ClaudeMesaj {
                    role: rol.to_string(),
                    content: icerik_bloklari,
                });
            }
        }
    }

    let system = if sistem_mesajlari.is_empty() {
        None
    } else {
        Some(sistem_mesajlari.join("\n\n"))
    };

    Ok(ClaudeIstek {
        model: istek.model.clone(),
        messages: claude_mesajlar,
        max_tokens: istek.max_tokens.unwrap_or(4096),
        system,
        temperature: istek.temperature,
        top_p: istek.top_p,
        stream: istek.stream,
    })
}

/// Anthropic Claude yanıtını OpenAI Chat Completion yanıt biçimine dönüştürür.
pub fn claude_dan_openai_yaniti(yanit: &ClaudeYanit, istenen_model: &str) -> Result<OpenAiYanit> {
    let simdi = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut metin_toplami = String::new();
    for blok in &yanit.content {
        match blok {
            ClaudeIcerikBloku::Metin { text } => {
                metin_toplami.push_str(text);
            }
        }
    }

    let finish_reason = yanit
        .stop_reason
        .as_deref()
        .map(|r| match r {
            "end_turn" => "stop".to_string(),
            "max_tokens" => "length".to_string(),
            "stop_sequence" => "stop".to_string(),
            other => other.to_string(),
        })
        .unwrap_or_else(|| "stop".to_string());

    let secenek = OpenAiSecenek {
        index: 0,
        message: OpenAiMesaj {
            role: "assistant".to_string(),
            content: Some(metin_toplami),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
        finish_reason: Some(finish_reason),
    };

    let kullanim = yanit.usage.as_ref().map(|u| OpenAiKullanim {
        prompt_tokens: u.input_tokens,
        completion_tokens: u.output_tokens,
        total_tokens: u.input_tokens + u.output_tokens,
    });

    Ok(OpenAiYanit {
        id: format!("chatcmpl-{}", yanit.id),
        object: "chat.completion".to_string(),
        created: simdi,
        model: istenen_model.to_string(),
        choices: vec![secenek],
        usage: kullanim,
    })
}

/// Anthropic SSE olayını OpenAI Chat Completion Chunk biçimine dönüştürür.
pub fn claude_dan_openai_parcasi(
    olay: &ClaudeOlay,
    istek_id: &str,
    model: &str,
) -> Result<Option<OpenAiParca>> {
    let simdi = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    match olay {
        ClaudeOlay::MesajBasladi { .. } => Ok(Some(OpenAiParca {
            id: istek_id.to_string(),
            object: "chat.completion.chunk".to_string(),
            created: simdi,
            model: model.to_string(),
            choices: vec![OpenAiParcaSecenek {
                index: 0,
                delta: OpenAiDelta {
                    role: Some("assistant".to_string()),
                    content: None,
                },
                finish_reason: None,
            }],
        })),
        ClaudeOlay::IcerikDeltasi { delta } => match delta {
            ClaudeDeltaIcerik::Metin { text } => Ok(Some(OpenAiParca {
                id: istek_id.to_string(),
                object: "chat.completion.chunk".to_string(),
                created: simdi,
                model: model.to_string(),
                choices: vec![OpenAiParcaSecenek {
                    index: 0,
                    delta: OpenAiDelta {
                        role: None,
                        content: Some(text.clone()),
                    },
                    finish_reason: None,
                }],
            })),
        },
        ClaudeOlay::MesajDeltasi { delta, .. } => {
            let finish = delta.stop_reason.as_deref().map(|r| match r {
                "end_turn" => "stop".to_string(),
                "max_tokens" => "length".to_string(),
                other => other.to_string(),
            });
            Ok(Some(OpenAiParca {
                id: istek_id.to_string(),
                object: "chat.completion.chunk".to_string(),
                created: simdi,
                model: model.to_string(),
                choices: vec![OpenAiParcaSecenek {
                    index: 0,
                    delta: OpenAiDelta::default(),
                    finish_reason: finish,
                }],
            }))
        }
        ClaudeOlay::MesajBitti => Ok(None),
    }
}
