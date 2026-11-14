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
                    ..Default::default()
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
        // Ters yön (OpenAI→Anthropic) araç tanımı taşımaz: çekirdeğin döndürdüğü
        // yanıt kendi araç çağrılarını `tool_calls` olarak verir.
        tools: None,
        tool_choice: None,
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
        cevrilmis_mesajlar(m, &mut mesajlar)?;
    }

    // Araç tanımları OpenAI biçimine taşınır. Sessizce düşerse model hiçbir
    // araç çağıramaz ve istemci nedensiz yere "araç yok" görür.
    let tools = istek.tools.as_ref().and_then(|t| {
        let liste: Vec<serde_json::Value> = t
            .iter()
            .map(|arac| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": arac.name,
                        "description": arac.description.clone().unwrap_or_default(),
                        "parameters": arac.input_schema.clone()
                            .unwrap_or_else(|| serde_json::json!({"type": "object", "properties": {}})),
                    }
                })
            })
            .collect();
        (!liste.is_empty()).then_some(liste)
    });

    Ok(OpenAiIstek {
        model: hedef_model.to_string(),
        messages: mesajlar,
        temperature: istek.temperature,
        top_p: None,
        max_tokens: Some(istek.max_tokens),
        stream: istek.stream,
        tools,
    })
}

/// Metinden `<system-reminder>…</system-reminder>` bloklarını atar.
///
/// NEDEN VAR (2026-11-14): Claude Code istemcisi kullanıcı mesajının içine
/// kimlik/hook metinlerini `<system-reminder>` sarmalıyla gömüyor; ölçülen
/// gerçek istekte 13.492 karakterin tamamı bu sarmal, gerçek görev ise
/// `notlar.txt` (10 karakter) idi. Sarmalın içi kullanıcının sorduğu şey
/// değildir; modelin önüne kullanıcı sorusu olarak konunca araç çağırmayı
/// bırakıp talimat metnini özetliyordu (varyant ölçümü: H=0, G=1 araç çağrısı).
/// Blok dışındaki metne DOKUNULMAZ — gerçek soru genelde sarmaldan sonra gelir.
fn system_hatirlaticilari_temizle(metin: &str) -> String {
    const AC: &str = "<system-reminder";
    const KAPA: &str = "</system-reminder>";
    if !metin.contains(AC) {
        return metin.to_string();
    }
    let mut kalan = String::with_capacity(metin.len());
    let mut geri = metin;
    while let Some(bas) = geri.find(AC) {
        kalan.push_str(&geri[..bas]);
        // Açılış etiketi öznitelik taşıyabilir: ilk '>' karakterine kadar at.
        let Some(etiket_sonu) = geri[bas..].find('>') else {
            // Kapanmamış açılış: geri kalanın tamamı sarmaldır.
            geri = "";
            break;
        };
        let govde_basi = bas + etiket_sonu + 1;
        match geri[govde_basi..].find(KAPA) {
            Some(kapa_bas) => geri = &geri[govde_basi + kapa_bas + KAPA.len()..],
            None => {
                geri = "";
                break;
            }
        }
    }
    kalan.push_str(geri);
    kalan
}

/// Bir Anthropic mesajını OpenAI mesaj(lar)ına çevirir.
///
/// Tek Anthropic mesajı birden çok OpenAI mesajı üretebilir: `tool_result`
/// blokları ayrı `role:"tool"` mesajları olur. `text` ile `tool_use` aynı
/// assistant mesajında kalır — OpenAI `content` + `tool_calls` alanlarını
/// birlikte taşır.
fn cevrilmis_mesajlar(m: &ClaudeMesaj, cikti: &mut Vec<OpenAiMesaj>) -> Result<(), CeviriHatasi> {
    // `role:"system"` Anthropic mesaj dizisinde GEÇERSİZDİR (yalnız `user` ve
    // `assistant` vardır); sistem metni ayrı `system` alanında gelir. Claude Code
    // ise dizinin içine ortam açıklaması koyuyor (ölçüldü 2026-11-14: 56.271
    // karakter; yüzlerce beceri tanımı). Bunu sıradan bir kullanıcı mesajı gibi
    // taşımak istemi şişirip modeli araç çağırmaktan vazgeçiriyordu.
    if m.role == "system" {
        return Ok(());
    }
    let mut metin_parcalari: Vec<String> = Vec::new();
    let mut araclar: Vec<OpenAiAracCagrisi> = Vec::new();
    let mut sonuclar: Vec<OpenAiMesaj> = Vec::new();
    for blok in &m.content {
        match blok.r#type.as_str() {
            "text" => {
                let temiz = system_hatirlaticilari_temizle(&blok.text);
                if !temiz.trim().is_empty() {
                    metin_parcalari.push(temiz);
                }
            }
            // Anthropic yanıtındaki araç çağrısı; sonraki turda `tool_use`
            // bloğu olarak geri gönderilir.
            "tool_use" => {
                let ad = blok.name.clone().ok_or_else(|| {
                    CeviriHatasi::GecersizIstek("tool_use bloğunda 'name' yok".to_string())
                })?;
                let id = blok.id.clone().ok_or_else(|| {
                    CeviriHatasi::GecersizIstek("tool_use bloğunda 'id' yok".to_string())
                })?;
                araclar.push(OpenAiAracCagrisi {
                    id,
                    r#type: "function".to_string(),
                    function: OpenAiFonksiyonCagrisi {
                        name: ad,
                        arguments: blok
                            .input
                            .as_ref()
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "{}".to_string()),
                    },
                });
            }
            // Araç sonucu istemciden gelir; OpenAI'de ayrı `role:"tool"`
            // mesajıdır ve `tool_call_id` ile çağrıya bağlanır.
            "tool_result" => {
                let id = blok.tool_use_id.clone().ok_or_else(|| {
                    CeviriHatasi::GecersizIstek(
                        "tool_result bloğunda 'tool_use_id' yok".to_string(),
                    )
                })?;
                sonuclar.push(OpenAiMesaj {
                    role: "tool".to_string(),
                    content: Some(serde_json::Value::String(
                        blok.icerik.clone().unwrap_or_default(),
                    )),
                    name: None,
                    tool_calls: None,
                    tool_call_id: Some(id),
                });
            }
            other => {
                return Err(CeviriHatasi::Desteklenmiyor(format!(
                    "içerik bloğu türü '{other}'"
                )));
            }
        }
    }

    if !metin_parcalari.is_empty() || !araclar.is_empty() {
        let metin = metin_parcalari.join("\n");
        cikti.push(OpenAiMesaj {
            role: m.role.clone(),
            content: Some(serde_json::Value::String(metin)),
            name: None,
            tool_calls: (!araclar.is_empty()).then_some(araclar),
            tool_call_id: None,
        });
    }
    cikti.extend(sonuclar);
    Ok(())
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

    let mut icerik = Vec::with_capacity(2);
    if !metin.is_empty() {
        icerik.push(ClaudeIcerikBloku {
            r#type: "text".to_string(),
            text: metin,
            ..Default::default()
        });
    }
    // Araç çağrıları `tool_use` bloğuna çevrilir. Bu yapılmazsa Claude Code
    // çağrıyı hiç görmez, yalnız boş bir metin alır ve aracı çalıştırmaz
    // (2026-11-14 ölçüldü).
    if let Some(cagrilar) = &secenek.message.tool_calls {
        for c in cagrilar {
            let girdi: serde_json::Value = serde_json::from_str(&c.function.arguments)
                .unwrap_or_else(|_| serde_json::json!({}));
            icerik.push(ClaudeIcerikBloku {
                r#type: "tool_use".to_string(),
                id: Some(c.id.clone()),
                name: Some(c.function.name.clone()),
                input: Some(girdi),
                ..Default::default()
            });
        }
    }
    if icerik.is_empty() {
        icerik.push(ClaudeIcerikBloku {
            r#type: "text".to_string(),
            text: String::new(),
            ..Default::default()
        });
    }

    Ok(ClaudeYanit {
        id: yanit
            .id
            .strip_prefix("chatcmpl-")
            .unwrap_or(&yanit.id)
            .to_string(),
        r#type: "message".to_string(),
        role: "assistant".to_string(),
        content: icerik,
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
            tool_calls: None,
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
