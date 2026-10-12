//! OpenAI ↔ Anthropic Claude Biçim Çeviricisi
//!
//! OmniRoute translator/request/openai-to-claude.ts kurallarının saf Rust uygulaması.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::modeller::*;

/// OpenAI istek gövdesini Anthropic Claude Messages istek gövdesine dönüştürür.
/// - FR-5.3: `tools` veya çok parçalı içerik varsa yukarı akışa gitmeden açık hata döner.
/// - Boş metin Anthropic API kuralı gereği `.` ile doldurulur.
/// - Sistem mesajları `system` bloğuna taşınır.
pub fn openai_den_claude_istegi(
    istek: &OpenAiIstek,
    hedef_model: &str,
) -> Result<ClaudeIstek, CeviriHatasi> {
    // 1. Desteklenmeyen alan denetimleri (FR-5.3)
    istek.desteklenmeyeni_denetle()?;

    let mut sistem_bloklari = Vec::new();
    let mut claude_mesajlar = Vec::new();

    for m in &istek.messages {
        let metin = m.duz_metin().unwrap_or_default();
        if m.role == "system" {
            if !metin.trim().is_empty() {
                sistem_bloklari.push(ClaudeSistemBloku {
                    r#type: "text".to_string(),
                    text: metin,
                    cache_control: Some(ClaudeCacheControl {
                        r#type: "ephemeral".to_string(),
                        ttl: Some("1h".to_string()),
                    }),
                });
            }
        } else {
            let rol = if m.role == "assistant" {
                "assistant"
            } else {
                "user"
            };
            // Anthropic boş metni reddeder; OmniRoute boş içerikte "." yerleştirir
            let son_metin = if metin.is_empty() {
                ".".to_string()
            } else {
                metin
            };

            let cache_control = if rol == "assistant" {
                Some(ClaudeCacheControl {
                    r#type: "ephemeral".to_string(),
                    ttl: None,
                })
            } else {
                None
            };

            claude_mesajlar.push(ClaudeMesaj {
                role: rol.to_string(),
                content: vec![ClaudeIcerikBloku {
                    r#type: "text".to_string(),
                    text: son_metin,
                    cache_control,
                }],
            });
        }
    }

    let system = if sistem_bloklari.is_empty() {
        None
    } else {
        Some(sistem_bloklari)
    };

    Ok(ClaudeIstek {
        model: hedef_model.to_string(),
        max_tokens: istek.max_tokens.unwrap_or(64000),
        stream: istek.stream,
        messages: claude_mesajlar,
        system,
        temperature: istek.temperature,
    })
}

/// Anthropic Claude yanıtını OpenAI Chat Completion yanıt biçimine dönüştürür.
pub fn claude_dan_openai_yaniti(
    yanit: &ClaudeYanit,
    istenen_model: &str,
) -> Result<OpenAiYanit, CeviriHatasi> {
    let simdi = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs(),
        Err(_) => 0,
    };

    let mut metin_toplami = String::new();
    for blok in &yanit.content {
        metin_toplami.push_str(&blok.text);
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
        message: OpenAiMesajCikti {
            role: "assistant".to_string(),
            content: Some(metin_toplami),
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
) -> Result<Option<OpenAiParca>, CeviriHatasi> {
    let simdi = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs(),
        Err(_) => 0,
    };

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
            ClaudeDeltaIcerik::Diger => Ok(None),
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
        // Hata olayı parça değildir; akış yöneticisi onu istemciye hata olarak iletir.
        ClaudeOlay::Hata { .. } | ClaudeOlay::Diger => Ok(None),
    }
}
