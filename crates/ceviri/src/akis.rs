//! Akış (SSE) yanıt çevirisi: Claude / Gemini olayları → OpenAI `chat.completion.chunk`.
//!
//! OmniRoute `translator/response/{claude,gemini}-to-openai.ts` metin kapsamının durumlu
//! Rust uygulaması; altınlar `araclar/uretec/akis-altin-uret.ts` ile gerçek OmniRoute'tan
//! üretilir. Düşünme blokları, araç çağrıları ve önbellek ayrıntıları kapsam dışı
//! (araçlar istek tarafında açık hatayla reddedilir).

use crate::modeller::*;

fn parca(
    id: &str,
    model: &str,
    simdi: u64,
    delta: OpenAiDelta,
    finish_reason: Option<String>,
    usage: Option<OpenAiKullanim>,
) -> OpenAiParca {
    OpenAiParca {
        id: format!("chatcmpl-{id}"),
        object: "chat.completion.chunk".to_string(),
        created: simdi,
        model: model.to_string(),
        choices: vec![OpenAiParcaSecenek {
            index: 0,
            delta,
            finish_reason,
        }],
        usage,
    }
}

fn rol() -> OpenAiDelta {
    OpenAiDelta {
        role: Some("assistant".to_string()),
        content: None,
    }
}

fn metin(t: &str) -> OpenAiDelta {
    OpenAiDelta {
        role: None,
        content: Some(t.to_string()),
    }
}

/// OmniRoute `utils/finishReason.ts` `normalizeOpenAICompatibleFinishReason` (Gemini yolu).
pub fn openai_bitis_nedeni(neden: &str) -> String {
    let n = neden.to_ascii_lowercase();
    match n.as_str() {
        "stop" | "length" | "tool_calls" | "content_filter" | "function_call" => n,
        "max_tokens" | "model_context_window_exceeded" => "length".to_string(),
        "safety" | "recitation" | "blocklist" | "prohibited_content" | "content_filtered"
        | "policy_violation" | "malformed_response" | "refusal" => "content_filter".to_string(),
        "end_turn" | "stop_sequence" | "pause_turn" => "stop".to_string(),
        "tool_use" => "tool_calls".to_string(),
        _ => n,
    }
}

/// OmniRoute `claude-to-openai.ts` `convertStopReason`: tanınmayan her neden "stop".
fn claude_bitis_nedeni(neden: &str) -> &'static str {
    match neden {
        "max_tokens" => "length",
        "tool_use" => "tool_calls",
        _ => "stop",
    }
}

/// Claude Messages akışı → OpenAI parçaları. `message_start`'tan mesaj kimliği, model ve
/// girdi token sayısı tutulur; `message_delta` bitiş parçası usage taşır.
#[derive(Debug, Default)]
pub struct ClaudeAkisCevirici {
    mesaj_id: String,
    model: String,
    girdi_token: u32,
}

impl ClaudeAkisCevirici {
    pub fn cevir(&mut self, olay: &ClaudeOlay, simdi: u64) -> Vec<OpenAiParca> {
        match olay {
            ClaudeOlay::MesajBasladi { message } => {
                self.mesaj_id = if message.id.is_empty() {
                    format!("msg_{}", simdi * 1000)
                } else {
                    message.id.clone()
                };
                self.model = message.model.clone();
                self.girdi_token = message.usage.as_ref().map_or(0, |u| u.input_tokens);
                vec![parca(&self.mesaj_id, &self.model, simdi, rol(), None, None)]
            }
            ClaudeOlay::IcerikDeltasi {
                delta: ClaudeDeltaIcerik::Metin { text },
            } if !text.is_empty() => {
                vec![parca(
                    &self.mesaj_id,
                    &self.model,
                    simdi,
                    metin(text),
                    None,
                    None,
                )]
            }
            ClaudeOlay::MesajDeltasi { delta, usage } => {
                let cikti = usage.as_ref().map_or(0, |u| u.output_tokens);
                let kullanim = (self.girdi_token > 0 || cikti > 0).then(|| OpenAiKullanim {
                    prompt_tokens: self.girdi_token,
                    completion_tokens: cikti,
                    total_tokens: self.girdi_token + cikti,
                });
                let neden = claude_bitis_nedeni(delta.stop_reason.as_deref().unwrap_or(""));
                vec![parca(
                    &self.mesaj_id,
                    &self.model,
                    simdi,
                    OpenAiDelta::default(),
                    Some(neden.to_string()),
                    kullanim,
                )]
            }
            _ => Vec::new(),
        }
    }
}

/// Gemini streamGenerateContent akışı → OpenAI parçaları. İlk olayda rol parçası; her metin
/// parçası ayrı parça; bitiş nedeni ayrı, usage'lı son parça.
#[derive(Debug, Default)]
pub struct GeminiAkisCevirici {
    mesaj_id: Option<String>,
    model: String,
    kullanim: Option<OpenAiKullanim>,
}

impl GeminiAkisCevirici {
    /// `simdi_ms`: yanıt kimliği yoksa OmniRoute gibi `msg_<ms>` kimliği için.
    pub fn cevir(&mut self, yanit: &GeminiYanit, simdi_ms: u64) -> Vec<OpenAiParca> {
        let simdi = simdi_ms / 1000;
        let Some(aday) = yanit.candidates.as_ref().and_then(|a| a.first()) else {
            return Vec::new();
        };
        let mut sonuc = Vec::new();
        if self.mesaj_id.is_none() {
            self.mesaj_id = Some(
                yanit
                    .response_id
                    .clone()
                    .unwrap_or_else(|| format!("msg_{simdi_ms}")),
            );
            self.model = yanit
                .model_version
                .clone()
                .unwrap_or_else(|| "gemini".to_string());
            sonuc.push(parca(self.id(), &self.model, simdi, rol(), None, None));
        }
        if let Some(m) = &yanit.usage_metadata {
            let girdi = m.prompt_token_count.unwrap_or(0);
            let cikti = m.candidates_token_count.unwrap_or(0);
            self.kullanim = Some(OpenAiKullanim {
                prompt_tokens: girdi,
                completion_tokens: cikti,
                total_tokens: m.total_token_count.unwrap_or(girdi + cikti),
            });
        }
        for p in aday.content.iter().flat_map(|c| &c.parts) {
            if !p.text.is_empty() {
                sonuc.push(parca(
                    self.id(),
                    &self.model,
                    simdi,
                    metin(&p.text),
                    None,
                    None,
                ));
            }
        }
        if let Some(neden) = &aday.finish_reason {
            sonuc.push(parca(
                self.id(),
                &self.model,
                simdi,
                OpenAiDelta::default(),
                Some(openai_bitis_nedeni(neden)),
                self.kullanim.clone(),
            ));
        }
        sonuc
    }

    fn id(&self) -> &str {
        self.mesaj_id.as_deref().unwrap_or_default()
    }
}
