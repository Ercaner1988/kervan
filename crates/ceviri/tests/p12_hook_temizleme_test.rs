//! Claude Code gövdesindeki hook/ortam gürültüsünün temizlenmesi.
//!
//! NEDEN VAR (2026-11-14): Claude Code `/v1/messages` gövdesinde kullanıcı
//! mesajının içine 13.492 karakterlik `<system-reminder>` sarmalı, `messages`
//! dizisine de 56.271 karakterlik `role:"system"` elemanı koyuyor. Gerçek görev
//! 10 karakterdi. Model bu gövdeyle araç çağırmayı bırakıp talimat metnini
//! özetliyordu; kısa `messages` ile aynı istek araç çağırıyordu (varyant G=1,
//! H=0). Temizlik çeviri katmanında yapılır: çekirdek (chat.rs) ve tarayıcı
//! yolu aynı temiz gövdeyi görsün.

use kervan_ceviri::*;

fn cevir(govde: serde_json::Value) -> OpenAiIstek {
    let istek: ClaudeIstek = serde_json::from_value(govde).expect("gövde ayrıştırılamadı");
    claude_den_openai_istegi(&istek, "hedef-model").expect("çeviri başarısız")
}

fn metin(m: &OpenAiMesaj) -> String {
    match &m.content {
        Some(serde_json::Value::String(s)) => s.clone(),
        _ => String::new(),
    }
}

#[test]
fn p12_system_rollu_mesaj_ceviriye_girmez() {
    let istek = cevir(serde_json::json!({
        "model": "m",
        "max_tokens": 8,
        "messages": [
            {"role": "user", "content": "notlar.txt dosyasini oku"},
            {"role": "system", "content": "# Environment\nPrimary working directory: C:\\x\nToday's date: 2026-11-14."}
        ]
    }));

    assert_eq!(istek.messages.len(), 1, "role:system elemanı taşınmamalı");
    assert_eq!(istek.messages[0].role, "user");
    assert!(
        !istek.messages[0]
            .content
            .as_ref()
            .map(|c| c.to_string())
            .unwrap_or_default()
            .contains("Primary working directory"),
        "ortam metni modele gitmemeli"
    );
}

#[test]
fn p12_system_reminder_sarmali_atilir_gercek_soru_kalir() {
    let istek = cevir(serde_json::json!({
        "model": "m",
        "max_tokens": 8,
        "messages": [{"role": "user", "content": [
            {"type": "text", "text": "<system-reminder>\nContents of C:\\Users\\x\\.claude\\CLAUDE.md ...\nCo-Authored-By: Claude Code <noreply@anthropic.com>\n</system-reminder><system-reminder>\nAttribution for git commits\n</system-reminder>notlar.txt"}
        ]}]
    }));

    assert_eq!(istek.messages.len(), 1);
    let giden = metin(&istek.messages[0]);
    assert_eq!(giden, "notlar.txt", "sarmal atılmalı, gerçek soru kalmalı");
}

#[test]
fn p12_oznitelikli_ve_kapanmamis_sarmal_da_temizlenir() {
    let istek = cevir(serde_json::json!({
        "model": "m",
        "max_tokens": 8,
        "messages": [{"role": "user", "content": [
            {"type": "text", "text": "once<system-reminder kaynak=\"hook\">gizli</system-reminder> sonra"},
            {"type": "text", "text": "<system-reminder>kapanmamis sarmal"}
        ]}]
    }));

    let giden = metin(&istek.messages[0]);
    assert_eq!(giden, "once sonra");
}

#[test]
fn p12_arac_bloklari_temizlikten_etkilenmez() {
    let istek = cevir(serde_json::json!({
        "model": "m",
        "max_tokens": 8,
        "messages": [
            {"role": "assistant", "content": [
                {"type": "text", "text": "<system-reminder>gurultu</system-reminder>"},
                {"type": "tool_use", "id": "toolu_1", "name": "Read", "input": {"file_path": "notlar.txt"}}
            ]},
            {"role": "user", "content": [
                {"type": "tool_result", "tool_use_id": "toolu_1", "content": "dosya icerigi"}
            ]}
        ]
    }));

    let asistan = istek
        .messages
        .iter()
        .find(|m| m.role == "assistant")
        .expect("assistant mesajı korunmalı");
    assert_eq!(metin(asistan), "", "sarmal metni boşaltılmalı");
    let cagri = asistan
        .tool_calls
        .as_ref()
        .and_then(|c| c.first())
        .expect("tool_calls korunmalı");
    assert_eq!(cagri.function.name, "Read");
    assert_eq!(cagri.function.arguments, "{\"file_path\":\"notlar.txt\"}");

    let arac = istek
        .messages
        .iter()
        .find(|m| m.role == "tool")
        .expect("tool_result ayrı role:tool mesajı olmalı");
    assert_eq!(metin(arac), "dosya icerigi");
    assert_eq!(arac.tool_call_id.as_deref(), Some("toolu_1"));
}
