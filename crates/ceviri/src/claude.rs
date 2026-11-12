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

/// Anthropic Messages isteğini OpenAI Chat Completion isteğine dönüştürür.
///
/// NEDEN VAR (2026-11-12): Kervan'ın `/v1/messages` ucu Claude Code/Desktop'ı
/// karşılar; gelen gövde Anthropic biçimindedir, çekirdek ise OpenAI biçimiyle
/// çalışır. Bu yön olmadan uç, çekirdeği ikinci kez yazmak zorunda kalırdı.
///
/// Taşınmayan alan sessizce düşürülmez: `tools`, `tool_choice` ve metin dışı
/// içerik blokları açık hataya döner (FR-5.3 ile aynı kural).
pub fn claude_den_openai_istegi(
    istek: &ClaudeIstek,
    hedef_model: &str,
) -> Result<OpenAiIstek, CeviriHatasi> {
    let mut mesajlar = Vec::with_capacity(istek.messages.len() + 1);

    // Anthropic sistem metni ayrı bir alandır; OpenAI'de ilk `system` mesajıdır.
    // Birden çok blok tek metne birleşir: OpenAI tarafı blok listesini bilmez.
    if let Some(bloklar) = &istek.system {
        let birlesik: String = bloklar
            .iter()
            .filter(|b| b.r#type == "text" && !b.text.trim().is_empty())
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        if !birlesik.is_empty() {
            mesajlar.push(OpenAiMesaj {
                role: "system".to_string(),
                content: Some(serde_json::Value::String(birlesik)),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            });
        }
    }

    for m in &istek.messages {
        let metin = claude_metni(m)?;
        mesajlar.push(OpenAiMesaj {
            role: m.role.clone(),
            content: Some(serde_json::Value::String(metin)),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        });
    }

    Ok(OpenAiIstek {
        model: hedef_model.to_string(),
        messages: mesajlar,
        temperature: istek.temperature,
        top_p: None,
        max_tokens: Some(istek.max_tokens),
        stream: istek.stream,
        tools: None,
    })
}

/// Bir Anthropic mesajının metin içeriğini tek dizeye indirir.
fn claude_metni(m: &ClaudeMesaj) -> Result<String, CeviriHatasi> {
    let mut parcalar = Vec::with_capacity(m.content.len());
    for blok in &m.content {
        match blok.r#type.as_str() {
            "text" => parcalar.push(blok.text.as_str()),
            // Anthropic boş metni reddeder ama istemciden gelebilir; "." ile
            // doldurmak yerine olduğu gibi geçir: boş istem burada hata değildir.
            other => {
                return Err(CeviriHatasi::Desteklenmiyor(format!(
                    "içerik bloğu türü '{other}'"
                )));
            }
        }
    }
    Ok(parcalar.join("\n"))
}

/// OpenAI Chat Completion yanıtını Anthropic Messages yanıtına dönüştürür.
///
/// `/v1/messages` ucunun dönüş yolu; `claude_dan_openai_yaniti`nin tersi.
pub fn openai_den_claude_yaniti(
    yanit: &OpenAiYanit,
    istenen_model: &str,
) -> Result<ClaudeYanit, CeviriHatasi> {
    let secenek = yanit
        .choices
        .first()
        .ok_or_else(|| CeviriHatasi::GecersizIstek("yanitta hiç seçenek yok".to_string()))?;

    let metin = secenek.message.content.clone().unwrap_or_default();

    // OpenAI bitiş nedenleri Anthropic sözlüğüne geri çevrilir. `stop` iki
    // anlama gelebilir; `end_turn` seçilir çünkü istemci "model bitirdi" görür.
    let stop_reason = match secenek.finish_reason.as_deref() {
        Some("length") => "max_tokens",
        Some("tool_calls") => "tool_use",
        Some("stop") | None => "end_turn",
        Some(other) => other,
    }
    .to_string();

    let usage = yanit.usage.as_ref().map(|u| ClaudeKullanim {
        input_tokens: u.prompt_tokens,
        output_tokens: u.completion_tokens,
    });

    Ok(ClaudeYanit {
        id: yanit
            .id
            .strip_prefix("chatcmpl-")
            .unwrap_or(&yanit.id)
            .to_string(),
        r#type: "message".to_string(),
        role: "assistant".to_string(),
        content: vec![ClaudeIcerikBloku {
            r#type: "text".to_string(),
            text: metin,
            cache_control: None,
        }],
        model: istenen_model.to_string(),
        stop_reason: Some(stop_reason),
        usage,
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
