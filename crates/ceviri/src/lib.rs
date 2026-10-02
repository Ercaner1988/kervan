//! Kervan Format Çeviricisi
//!
//! OpenAI Chat Completion, Anthropic Claude ve Google Gemini API şemaları arasında
//! kayıpsız, güvenli ve yüksek performanslı iki yönlü biçim dönüşümü sağlar.

pub mod claude;
pub mod gemini;
pub mod modeller;

pub use claude::{claude_dan_openai_parcasi, claude_dan_openai_yaniti, openai_den_claude_istegi};
pub use gemini::{gemini_den_openai_parcasi, gemini_den_openai_yaniti, openai_den_gemini_istegi};
pub use modeller::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_den_claude_istek_donusumu() {
        let openai_istek = OpenAiIstek {
            model: "claude-3-7-sonnet".to_string(),
            messages: vec![
                OpenAiMesaj {
                    role: "system".to_string(),
                    content: Some("Sen yardımcı bir asistansın.".to_string()),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
                OpenAiMesaj {
                    role: "user".to_string(),
                    content: Some("Merhaba Kervan!".to_string()),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
            ],
            temperature: Some(0.7),
            top_p: Some(0.9),
            max_tokens: Some(2048),
            stream: false,
            tools: None,
        };

        let claude_istek =
            openai_den_claude_istegi(&openai_istek).expect("Dönüşüm başarılı olmalı");

        assert_eq!(
            claude_istek.system,
            Some("Sen yardımcı bir asistansın.".to_string())
        );
        assert_eq!(claude_istek.messages.len(), 1);
        assert_eq!(claude_istek.messages[0].role, "user");
        assert_eq!(claude_istek.max_tokens, 2048);
        assert_eq!(claude_istek.temperature, Some(0.7));
    }

    #[test]
    fn test_claude_dan_openai_yanit_donusumu() {
        let claude_yanit = ClaudeYanit {
            id: "msg_12345".to_string(),
            r#type: "message".to_string(),
            role: "assistant".to_string(),
            content: vec![ClaudeIcerikBloku::Metin {
                text: "Merhaba! Nasıl yardımcı olabilirim?".to_string(),
            }],
            model: "claude-3-7-sonnet".to_string(),
            stop_reason: Some("end_turn".to_string()),
            usage: Some(ClaudeKullanim {
                input_tokens: 15,
                output_tokens: 25,
            }),
        };

        let openai_yanit = claude_dan_openai_yaniti(&claude_yanit, "gpt-4o")
            .expect("Yanıt dönüşümü başarılı olmalı");

        assert_eq!(openai_yanit.id, "chatcmpl-msg_12345");
        assert_eq!(openai_yanit.choices.len(), 1);
        assert_eq!(
            openai_yanit.choices[0].message.content,
            Some("Merhaba! Nasıl yardımcı olabilirim?".to_string())
        );
        assert_eq!(
            openai_yanit.choices[0].finish_reason,
            Some("stop".to_string())
        );

        let kullanim = openai_yanit.usage.expect("Kullanım verisi olmalı");
        assert_eq!(kullanim.prompt_tokens, 15);
        assert_eq!(kullanim.completion_tokens, 25);
        assert_eq!(kullanim.total_tokens, 40);
    }

    #[test]
    fn test_claude_akis_parcalari_donusumu() {
        let olay1 = ClaudeOlay::MesajBasladi {
            message: ClaudeYanit {
                id: "msg_1".to_string(),
                r#type: "message".to_string(),
                role: "assistant".to_string(),
                content: vec![],
                model: "claude-3-7".to_string(),
                stop_reason: None,
                usage: None,
            },
        };
        let p1 = claude_dan_openai_parcasi(&olay1, "chatcmpl-test", "gpt-4o")
            .unwrap()
            .unwrap();
        assert_eq!(p1.choices[0].delta.role, Some("assistant".to_string()));

        let olay2 = ClaudeOlay::IcerikDeltasi {
            delta: ClaudeDeltaIcerik::Metin {
                text: "Akış testi".to_string(),
            },
        };
        let p2 = claude_dan_openai_parcasi(&olay2, "chatcmpl-test", "gpt-4o")
            .unwrap()
            .unwrap();
        assert_eq!(p2.choices[0].delta.content, Some("Akış testi".to_string()));

        let olay3 = ClaudeOlay::MesajDeltasi {
            delta: ClaudeDurdurmaDeltasi {
                stop_reason: Some("end_turn".to_string()),
            },
            usage: None,
        };
        let p3 = claude_dan_openai_parcasi(&olay3, "chatcmpl-test", "gpt-4o")
            .unwrap()
            .unwrap();
        assert_eq!(p3.choices[0].finish_reason, Some("stop".to_string()));

        let olay4 = ClaudeOlay::MesajBitti;
        let p4 = claude_dan_openai_parcasi(&olay4, "chatcmpl-test", "gpt-4o").unwrap();
        assert!(p4.is_none());
    }

    #[test]
    fn test_openai_den_gemini_istek_ve_ardisik_birlestirme() {
        let openai_istek = OpenAiIstek {
            model: "gemini-2.5-flash".to_string(),
            messages: vec![
                OpenAiMesaj {
                    role: "system".to_string(),
                    content: Some("Sen Gemini asistanısın.".to_string()),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
                OpenAiMesaj {
                    role: "user".to_string(),
                    content: Some("Parça 1".to_string()),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
                OpenAiMesaj {
                    role: "user".to_string(),
                    content: Some("Parça 2".to_string()),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
                OpenAiMesaj {
                    role: "assistant".to_string(),
                    content: Some("Model yanıtı".to_string()),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
            ],
            temperature: Some(0.2),
            top_p: None,
            max_tokens: Some(1024),
            stream: false,
            tools: None,
        };

        let gemini_istek =
            openai_den_gemini_istegi(&openai_istek).expect("Dönüşüm başarılı olmalı");

        assert!(gemini_istek.system_instruction.is_some());
        assert_eq!(
            gemini_istek.system_instruction.unwrap().parts[0].text,
            "Sen Gemini asistanısın."
        );

        // Arka arkaya 2 user mesajı tek bir user içeriğinde birleştirilmeli
        assert_eq!(gemini_istek.contents.len(), 2);
        assert_eq!(gemini_istek.contents[0].role, "user");
        assert_eq!(gemini_istek.contents[0].parts.len(), 2);
        assert_eq!(gemini_istek.contents[0].parts[0].text, "Parça 1");
        assert_eq!(gemini_istek.contents[0].parts[1].text, "Parça 2");

        assert_eq!(gemini_istek.contents[1].role, "model");
        assert_eq!(gemini_istek.contents[1].parts[0].text, "Model yanıtı");
    }

    #[test]
    fn test_gemini_den_openai_yanit_donusumu() {
        let gemini_yanit = GeminiYanit {
            candidates: Some(vec![GeminiAday {
                content: Some(GeminiIcerik {
                    role: "model".to_string(),
                    parts: vec![
                        GeminiParca {
                            text: "Merhaba ".to_string(),
                        },
                        GeminiParca {
                            text: "Dünya!".to_string(),
                        },
                    ],
                }),
                finish_reason: Some("STOP".to_string()),
            }]),
            usage_metadata: Some(GeminiKullanim {
                prompt_token_count: Some(10),
                candidates_token_count: Some(20),
                total_token_count: Some(30),
            }),
        };

        let openai_yanit = gemini_den_openai_yaniti(&gemini_yanit, "gemini-2.5-flash")
            .expect("Dönüşüm başarılı olmalı");

        assert_eq!(
            openai_yanit.choices[0].message.content,
            Some("Merhaba Dünya!".to_string())
        );
        assert_eq!(
            openai_yanit.choices[0].finish_reason,
            Some("stop".to_string())
        );

        let kullanim = openai_yanit.usage.unwrap();
        assert_eq!(kullanim.total_tokens, 30);
    }
}
