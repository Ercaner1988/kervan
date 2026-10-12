//! `kervan-ceviri` — Format Çeviricisi
//!
//! OpenAI, Anthropic Claude ve Google Gemini API biçimleri arasında
//! iki yönlü (istek ve yanıt) parite doğrulanmış çeviri katmanı.

pub mod akis;
pub mod claude;
pub mod gemini;
pub mod modeller;

pub use akis::{ClaudeAkisCevirici, GeminiAkisCevirici, openai_bitis_nedeni};
pub use claude::{claude_dan_openai_yaniti, openai_den_claude_istegi};
pub use gemini::{gemini_den_openai_yaniti, openai_den_gemini_istegi};
pub use modeller::*;
