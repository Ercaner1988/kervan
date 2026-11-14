//! Biçim Tanımları ve Ortak Modeller
//!
//! OpenAI Chat Completion, Anthropic Messages ve Google Gemini API veri modelleri.

use serde::{Deserialize, Serialize};

// ── 1. Ortak Hata Tipi ────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CeviriHatasi {
    Desteklenmiyor(String),
    GecersizIstek(String),
    AyrirmaHatasi(String),
}

impl std::fmt::Display for CeviriHatasi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Desteklenmiyor(alan) => write!(f, "desteklenmiyor: {}", alan),
            Self::GecersizIstek(mesaj) => write!(f, "geçersiz istek: {}", mesaj),
            Self::AyrirmaHatasi(mesaj) => write!(f, "ayrıştırma hatası: {}", mesaj),
        }
    }
}

impl std::error::Error for CeviriHatasi {}

// ── 2. OpenAI Modelleri ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenAiIstek {
    pub model: String,
    pub messages: Vec<OpenAiMesaj>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenAiMesaj {
    pub role: String,
    #[serde(default)]
    pub content: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OpenAiAracCagrisi>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl OpenAiIstek {
    /// FR-5.3: Claude/Gemini çevirisinin taşımadığı alanlar yukarı akışa gitmeden açık hata
    /// olur; sessizce düşürülmez ya da başka bir role dönüştürülmez.
    pub fn desteklenmeyeni_denetle(&self) -> Result<(), CeviriHatasi> {
        let hata = |alan: &str| Err(CeviriHatasi::Desteklenmiyor(alan.to_string()));
        if self.tools.as_ref().is_some_and(|t| !t.is_empty()) {
            return hata("tools");
        }
        for m in &self.messages {
            if m.role == "tool" || m.tool_calls.is_some() || m.tool_call_id.is_some() {
                return hata("araç mesajı");
            }
            if m.content.as_ref().is_some_and(|c| !c.is_string()) {
                return hata("çok parçalı içerik");
            }
        }
        Ok(())
    }
}

impl OpenAiMesaj {
    pub fn duz_metin(&self) -> Option<String> {
        match &self.content {
            Some(serde_json::Value::String(s)) => Some(s.clone()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenAiAracCagrisi {
    pub id: String,
    pub r#type: String,
    pub function: OpenAiFonksiyonCagrisi,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenAiFonksiyonCagrisi {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenAiYanit {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<OpenAiSecenek>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<OpenAiKullanim>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenAiSecenek {
    pub index: u32,
    pub message: OpenAiMesajCikti,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct OpenAiMesajCikti {
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    // `content` tek başına yetmez: araç çağıran yanıtta metin boş olabilir ve
    // çağrılar `tool_calls` altında gelir. Alan yoksa ayrıştırma sırasında
    // çağrılar sessizce düşerdi.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OpenAiAracCagrisi>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenAiKullanim {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenAiParca {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<OpenAiParcaSecenek>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<OpenAiKullanim>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenAiParcaSecenek {
    pub index: u32,
    pub delta: OpenAiDelta,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct OpenAiDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

// ── 3. Anthropic Claude Modelleri ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClaudeCacheControl {
    pub r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClaudeSistemBloku {
    pub r#type: String,
    #[serde(default)]
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<ClaudeCacheControl>,
}

/// `system` alanını dize veya blok dizisi olarak çözer. Anthropic ikisini de
/// kabul eder; düz dize gönderen istemci 400 almamalı.
fn claude_sistem_coz<'de, D>(cozucu: D) -> Result<Option<Vec<ClaudeSistemBloku>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Giris {
        Metin(String),
        Bloklar(Vec<ClaudeSistemBloku>),
    }

    let giris = Option::<Giris>::deserialize(cozucu)?;
    Ok(match giris {
        None => None,
        Some(Giris::Metin(metin)) => Some(vec![ClaudeSistemBloku {
            r#type: "text".to_string(),
            text: metin,
            cache_control: None,
        }]),
        Some(Giris::Bloklar(bloklar)) => Some(bloklar),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClaudeIstek {
    pub model: String,
    pub max_tokens: u32,
    #[serde(default)]
    pub stream: bool,
    pub messages: Vec<ClaudeMesaj>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "claude_sistem_coz"
    )]
    pub system: Option<Vec<ClaudeSistemBloku>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    // Claude Code ve Claude Desktop araç tanımlarını burada gönderir. Alan
    // okunmazsa tanımlar sessizce düşer ve model hiçbir araç çağıramaz
    // (2026-11-14 ölçüldü: `--allowedTools Read` isteğinde dosya okunmadı).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ClaudeAracTanimi>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
}

/// Anthropic araç tanımı: `{"name","description","input_schema"}`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClaudeAracTanimi {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClaudeMesaj {
    pub role: String,
    // Anthropic `content` alanını İKİ biçimde kabul eder: düz dize ya da blok
    // dizisi. Yalnız dizi beklemek, basit istemcileri (curl, SDK'nın kısa yolu)
    // "invalid type: string \"ping\", expected a sequence" ile 400'e düşürüyordu.
    // Yazarken her zaman dizi üretilir; Anthropic ikisini de kabul ettiği için
    // en açık biçim budur.
    #[serde(deserialize_with = "claude_icerik_coz")]
    pub content: Vec<ClaudeIcerikBloku>,
}

/// `content` alanını dize veya blok dizisi olarak çözer, her ikisini blok
/// dizisine indirger.
fn claude_icerik_coz<'de, D>(cozucu: D) -> Result<Vec<ClaudeIcerikBloku>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error as _;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Giris {
        Metin(String),
        Bloklar(Vec<ClaudeIcerikBloku>),
    }

    match Giris::deserialize(cozucu)? {
        Giris::Metin(metin) => Ok(vec![ClaudeIcerikBloku {
            r#type: "text".to_string(),
            text: metin,
            ..Default::default()
        }]),
        Giris::Bloklar(bloklar) => {
            // Blok türü boş olamaz: çözülmüş ama anlamsız bir blok, aşağıda
            // "desteklenmiyor" hatası yerine sessiz veri kaybı üretirdi.
            for blok in &bloklar {
                if blok.r#type.trim().is_empty() {
                    return Err(D::Error::custom("içerik bloğunun 'type' alanı boş olamaz"));
                }
            }
            Ok(bloklar)
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ClaudeIcerikBloku {
    pub r#type: String,
    // `default`: metinsiz bloklar (görsel, araç) çözümlemede değil, çeviride
    // açık hataya düşsün. Aksi hâlde "missing field `text`" gibi anlaşılmaz bir
    // serde hatası dönerdi.
    #[serde(default)]
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<ClaudeCacheControl>,
    // `tool_use` bloğunun kimliği ve adı; `tool_result` bloğu da aynı `id` ile
    // sonucu geri gönderir. Alanlar metin bloklarında yazılmaz.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    // `tool_use.input` serbest biçimli JSON nesnesidir; OpenAI tarafında
    // `arguments` dizesine çevrilir.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
    // `tool_result.content` dize ya da blok dizisi olabilir; çözümleyici ikisini
    // de tek metne indirger.
    #[serde(
        default,
        rename = "content",
        skip_serializing_if = "Option::is_none",
        deserialize_with = "claude_serbest_metin"
    )]
    pub icerik: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

/// `tool_result.content` alanını dize, blok dizisi ya da yokluk olarak çözer.
/// Anthropic üç biçimi de kabul eder; blok dizisi tek metne indirgenir.
fn claude_serbest_metin<'de, D>(cozucu: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Giris {
        Metin(String),
        Bloklar(Vec<ClaudeIcerikBloku>),
    }

    let giris = Option::<Giris>::deserialize(cozucu)?;
    Ok(match giris {
        None => None,
        Some(Giris::Metin(metin)) => Some(metin),
        Some(Giris::Bloklar(bloklar)) => Some(
            bloklar
                .iter()
                .filter(|b| b.r#type == "text")
                .map(|b| b.text.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        ),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClaudeYanit {
    pub id: String,
    pub r#type: String,
    pub role: String,
    #[serde(default)]
    pub content: Vec<ClaudeIcerikBloku>,
    pub model: String,
    #[serde(default)]
    pub stop_reason: Option<String>,
    #[serde(default)]
    pub usage: Option<ClaudeKullanim>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClaudeKullanim {
    // message_delta yalnız output_tokens taşır: alan zorunlu olsaydı bitiş olayı ayrıştırılamaz,
    // akışta sessizce atlanır ve finish_reason istemciye hiç gitmezdi.
    #[serde(default)]
    pub input_tokens: u32,
    #[serde(default)]
    pub output_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ClaudeOlay {
    #[serde(rename = "message_start")]
    MesajBasladi { message: ClaudeYanit },
    #[serde(rename = "content_block_delta")]
    IcerikDeltasi { delta: ClaudeDeltaIcerik },
    #[serde(rename = "message_delta")]
    MesajDeltasi {
        delta: ClaudeDurdurmaDeltasi,
        usage: Option<ClaudeKullanim>,
    },
    #[serde(rename = "message_stop")]
    MesajBitti,
    /// Akış ortası hata (ör. overloaded_error): sessizce yutulmaz, akış başarı sayılmaz.
    #[serde(rename = "error")]
    Hata { error: serde_json::Value },
    #[serde(other)]
    Diger,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ClaudeDeltaIcerik {
    #[serde(rename = "text_delta")]
    Metin { text: String },
    #[serde(other)]
    Diger,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClaudeDurdurmaDeltasi {
    pub stop_reason: Option<String>,
}

// ── 4. Google Gemini Modelleri ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GeminiIstek {
    pub model: String,
    pub contents: Vec<GeminiIcerik>,
    pub generation_config: GeminiGenerationConfig,
    pub safety_settings: Vec<GeminiSafetySetting>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<GeminiSystemInstruction>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GeminiGenerationConfig {
    pub max_output_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_config: Option<GeminiThinkingConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GeminiThinkingConfig {
    pub thinking_budget: u32,
    pub include_thoughts: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GeminiSafetySetting {
    pub category: String,
    pub threshold: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GeminiSystemInstruction {
    pub role: String,
    pub parts: Vec<GeminiParca>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GeminiIcerik {
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub parts: Vec<GeminiParca>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GeminiParca {
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GeminiYanit {
    pub candidates: Option<Vec<GeminiAday>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_metadata: Option<GeminiKullanim>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GeminiAday {
    pub content: Option<GeminiIcerik>,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GeminiKullanim {
    pub prompt_token_count: Option<u32>,
    pub candidates_token_count: Option<u32>,
    pub total_token_count: Option<u32>,
}
