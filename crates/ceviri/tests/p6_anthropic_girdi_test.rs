//! Anthropic gövdesinin iki kabul biçimi.
//!
//! NEDEN VAR (2026-11-13): `/v1/messages` canlı denemesinde düz dizeli gövde
//! `invalid type: string "ping", expected a sequence` ile 400 döndü. Anthropic
//! API `content` ve `system` alanlarında hem dizeyi hem blok dizisini kabul
//! eder; tek biçim beklemek basit istemcileri (curl, SDK'nın kısa yolu) kırıyordu.

use kervan_ceviri::*;

fn claude_istegi(govde: serde_json::Value) -> ClaudeIstek {
    serde_json::from_value(govde).unwrap_or_else(|e| panic!("gövde ayrıştırılamadı: {e}"))
}

#[test]
fn p6_duz_dize_icerik_blok_dizisine_cevrilir() {
    let istek = claude_istegi(serde_json::json!({
        "model": "kervan/openrouter",
        "max_tokens": 8,
        "messages": [{"role": "user", "content": "ping"}]
    }));

    assert_eq!(istek.messages.len(), 1);
    assert_eq!(istek.messages[0].content.len(), 1);
    assert_eq!(istek.messages[0].content[0].r#type, "text");
    assert_eq!(istek.messages[0].content[0].text, "ping");
    assert!(istek.messages[0].content[0].cache_control.is_none());
}

#[test]
fn p6_blok_dizisi_bicimi_korunur() {
    let istek = claude_istegi(serde_json::json!({
        "model": "m",
        "max_tokens": 8,
        "messages": [{"role": "user", "content": [
            {"type": "text", "text": "bir"},
            {"type": "text", "text": "iki"}
        ]}]
    }));

    let bloklar = &istek.messages[0].content;
    assert_eq!(bloklar.len(), 2);
    assert_eq!(bloklar[1].text, "iki");
}

#[test]
fn p6_duz_dize_system_tek_bloga_cevrilir() {
    let istek = claude_istegi(serde_json::json!({
        "model": "m",
        "max_tokens": 8,
        "system": "Sen kısa yanıt veren bir asistansın.",
        "messages": [{"role": "user", "content": [{"type": "text", "text": "selam"}]}]
    }));

    let sistem = istek.system.expect("system çözülmeliydi");
    assert_eq!(sistem.len(), 1);
    assert_eq!(sistem[0].r#type, "text");
    assert_eq!(sistem[0].text, "Sen kısa yanıt veren bir asistansın.");
}

#[test]
fn p6_system_yoksa_none_kalir() {
    let istek = claude_istegi(serde_json::json!({
        "model": "m",
        "max_tokens": 8,
        "messages": [{"role": "user", "content": "selam"}]
    }));

    assert!(istek.system.is_none());
}

#[test]
fn p6_bos_blok_turu_reddedilir() {
    let sonuc: Result<ClaudeIstek, _> = serde_json::from_value(serde_json::json!({
        "model": "m",
        "max_tokens": 8,
        "messages": [{"role": "user", "content": [{"type": "  ", "text": "x"}]}]
    }));

    let hata = sonuc.expect_err("boş 'type' reddedilmeliydi");
    assert!(
        hata.to_string().contains("'type' alanı boş olamaz"),
        "beklenmeyen hata metni: {hata}"
    );
}

#[test]
fn p6_metinsiz_blok_cozumlenir_ceviride_hata_verir() {
    // Görsel/araç bloklarında `text` yoktur. Ayrıştırma "missing field `text`"
    // ile düşmemeli; hata çeviri katmanında açık olmalı.
    let istek = claude_istegi(serde_json::json!({
        "model": "m",
        "max_tokens": 8,
        "messages": [{"role": "user", "content": [
            {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": ""}}
        ]}]
    }));

    assert_eq!(istek.messages[0].content[0].r#type, "image");
    assert_eq!(istek.messages[0].content[0].text, "");

    let hata = claude_den_openai_istegi(&istek, "auto")
        .expect_err("metin dışı blok çeviride reddedilmeliydi");
    assert!(
        matches!(hata, CeviriHatasi::Desteklenmiyor(_)),
        "beklenmeyen hata: {hata:?}"
    );
}
