//! P6 Altın Parite Doğrulama Testi
//!
//! PRD FR-5.1 ve FR-5.2: port/altin/*.json dosyalarını okur ve
//! Kervan'ın gerçek Rust çeviri çıktısı ile altın referans çıktısını karşılaştırır.
//! Sapmalar port/altin/sapmalar.toml kaydı ile doğrulanır.

use serde_json::Value;
use std::fs;
use std::path::Path;

use kervan_ceviri::*;

#[test]
fn p6_claude_altin_parite_testi() {
    let altin_dizini = Path::new("../../port/altin");
    let dosyalar = vec![
        ("claude_1_sade.json", false),
        ("claude_2_system_coktur.json", false),
        ("claude_3_parametreler.json", false),
        ("claude_4_bos_icerik.json", false),
        ("claude_5_arac_tanimi.json", true), // FR-5.3 sapması (desteklenmiyor: tools)
        ("claude_6_uzun_icerik.json", false),
    ];

    for (dosya_adi, sapma_bekleniyor) in dosyalar {
        // Testler çalışma dizini veya alt sandık dizininden koşabilir
        let tam_yol = if altin_dizini.exists() {
            altin_dizini.join(dosya_adi)
        } else {
            Path::new("port/altin").join(dosya_adi)
        };

        assert!(tam_yol.exists(), "Altın dosya bulunamadı: {:?}", tam_yol);
        let icerik = fs::read_to_string(&tam_yol).expect("Altın dosya okunamadı");
        let altin_json: Value = serde_json::from_str(&icerik).expect("Altın dosya JSON değil");

        assert_eq!(altin_json["kaynak_sha"], "dbe703a");
        let girdi: OpenAiIstek = serde_json::from_value(altin_json["girdi"].clone())
            .expect("Girdi OpenAiIstek'e ayrıştırılamadı");

        let kervan_sonucu = openai_den_claude_istegi(&girdi, "claude-3-7-sonnet");

        if sapma_bekleniyor {
            assert!(
                kervan_sonucu.is_err(),
                "Dosya {} için açık hata (sapma) bekleniyordu ama başarılı oldu",
                dosya_adi
            );
            assert_eq!(
                kervan_sonucu.unwrap_err(),
                CeviriHatasi::Desteklenmiyor("tools".to_string())
            );
        } else {
            assert!(
                kervan_sonucu.is_ok(),
                "Dosya {} çevirisi başarısız oldu: {:?}",
                dosya_adi,
                kervan_sonucu.err()
            );
            let kervan_istek = kervan_sonucu.unwrap();
            let kervan_val =
                serde_json::to_value(&kervan_istek).expect("ClaudeIstek serileştirilemedi");
            let beklenen_val = &altin_json["cikti"];

            assert_eq!(
                kervan_val, *beklenen_val,
                "Altın parite farkı tespit edildi: {}",
                dosya_adi
            );
        }
    }
}

#[test]
fn p6_gemini_altin_parite_testi() {
    let altin_dizini = Path::new("../../port/altin");
    let dosyalar = vec![
        ("gemini_1_sade.json", false),
        ("gemini_2_system_coktur.json", false),
        ("gemini_3_parametreler.json", false),
        ("gemini_4_bos_icerik.json", false),
        ("gemini_5_arac_tanimi.json", true), // FR-5.3 sapması (desteklenmiyor: tools)
        ("gemini_6_uzun_icerik.json", false),
    ];

    for (dosya_adi, sapma_bekleniyor) in dosyalar {
        let tam_yol = if altin_dizini.exists() {
            altin_dizini.join(dosya_adi)
        } else {
            Path::new("port/altin").join(dosya_adi)
        };

        assert!(tam_yol.exists(), "Altın dosya bulunamadı: {:?}", tam_yol);
        let icerik = fs::read_to_string(&tam_yol).expect("Altın dosya okunamadı");
        let altin_json: Value = serde_json::from_str(&icerik).expect("Altın dosya JSON değil");

        assert_eq!(altin_json["kaynak_sha"], "dbe703a");
        let girdi: OpenAiIstek = serde_json::from_value(altin_json["girdi"].clone())
            .expect("Girdi OpenAiIstek'e ayrıştırılamadı");

        let kervan_sonucu = openai_den_gemini_istegi(&girdi, "gemini-2.5-flash");

        if sapma_bekleniyor {
            assert!(
                kervan_sonucu.is_err(),
                "Dosya {} için açık hata (sapma) bekleniyordu ama başarılı oldu",
                dosya_adi
            );
            assert_eq!(
                kervan_sonucu.unwrap_err(),
                CeviriHatasi::Desteklenmiyor("tools".to_string())
            );
        } else {
            assert!(
                kervan_sonucu.is_ok(),
                "Dosya {} çevirisi başarısız oldu: {:?}",
                dosya_adi,
                kervan_sonucu.err()
            );
            let kervan_istek = kervan_sonucu.unwrap();
            let kervan_val =
                serde_json::to_value(&kervan_istek).expect("GeminiIstek serileştirilemedi");
            let beklenen_val = &altin_json["cikti"];

            assert_eq!(
                kervan_val, *beklenen_val,
                "Altın parite farkı tespit edildi: {}",
                dosya_adi
            );
        }
    }
}

#[test]
fn p6_yanit_donusumleri_testi() {
    // 1. Claude yanıtı
    let claude_yanit = ClaudeYanit {
        id: "msg_p6_123".to_string(),
        r#type: "message".to_string(),
        role: "assistant".to_string(),
        content: vec![ClaudeIcerikBloku {
            r#type: "text".to_string(),
            text: "Claude parite yanıtı".to_string(),
            cache_control: None,
        }],
        model: "claude-3-7-sonnet".to_string(),
        stop_reason: Some("end_turn".to_string()),
        usage: Some(ClaudeKullanim {
            input_tokens: 25,
            output_tokens: 35,
        }),
    };

    let openai_claude = claude_dan_openai_yaniti(&claude_yanit, "kervan/claude-test")
        .expect("Claude yanıt dönüşümü başarılı olmalı");

    assert_eq!(openai_claude.id, "chatcmpl-msg_p6_123");
    assert_eq!(
        openai_claude.choices[0].message.content,
        Some("Claude parite yanıtı".to_string())
    );
    assert_eq!(
        openai_claude.choices[0].finish_reason,
        Some("stop".to_string())
    );
    assert_eq!(openai_claude.usage.as_ref().unwrap().total_tokens, 60);

    // 2. Gemini yanıtı
    let gemini_yanit = GeminiYanit {
        candidates: Some(vec![GeminiAday {
            content: Some(GeminiIcerik {
                role: "model".to_string(),
                parts: vec![GeminiParca {
                    text: "Gemini parite yanıtı".to_string(),
                }],
            }),
            finish_reason: Some("STOP".to_string()),
        }]),
        usage_metadata: Some(GeminiKullanim {
            prompt_token_count: Some(15),
            candidates_token_count: Some(25),
            total_token_count: Some(40),
        }),
    };

    let openai_gemini = gemini_den_openai_yaniti(&gemini_yanit, "kervan/gemini-test")
        .expect("Gemini yanıt dönüşümü başarılı olmalı");

    assert!(openai_gemini.id.starts_with("chatcmpl-gemini-"));
    assert_eq!(
        openai_gemini.choices[0].message.content,
        Some("Gemini parite yanıtı".to_string())
    );
    assert_eq!(
        openai_gemini.choices[0].finish_reason,
        Some("stop".to_string())
    );
    assert_eq!(openai_gemini.usage.as_ref().unwrap().total_tokens, 40);
}
