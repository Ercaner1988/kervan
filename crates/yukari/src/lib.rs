//! `kervan-yukari` — REST yukarı akış istemcisi.
//!
//! OpenAI, Claude ve Gemini uyumlu REST sağlayıcılarına HTTP üzerinden
//! kayıpsız istek iletimi ve yanıt alımını sağlar.

use std::fmt;
use std::time::Duration;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hedef {
    pub ad: String,
    pub bicim: String,
    pub taban_url: String,
    pub anahtar_env: String,
    pub model: String,
    pub yol: Option<String>,
    pub zaman_asimi: Duration,
}

impl Hedef {
    pub fn rest_ayardan(ad: &str, ayar: &kervan_tarayici::config::RestAyar, varsayilan_timeout: Duration) -> Self {
        Self {
            ad: ad.to_string(),
            bicim: ayar.bicim.clone(),
            taban_url: ayar.taban_url.trim_end_matches('/').to_string(),
            anahtar_env: ayar.anahtar_env.clone(),
            model: ayar.model.clone(),
            yol: ayar.yol.clone(),
            zaman_asimi: varsayilan_timeout,
        }
    }
}

#[derive(Debug)]
pub enum YukariHatasi {
    AnahtarYok(String),
    Desteklenmiyor(String),
    HttpHatasi(u16, String),
    AgHatasi(String),
    JsonHatasi(String),
}

impl fmt::Display for YukariHatasi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AnahtarYok(env_ad) => {
                write!(f, "Sağlayıcı API anahtarı bulunamadı: Ortam değişkeni '{}' tanımlı değil", env_ad)
            }
            Self::Desteklenmiyor(alan) => {
                write!(f, "Yukarı akış için desteklenmeyen alan: {}", alan)
            }
            Self::HttpHatasi(durum, govde) => {
                write!(f, "Yukarı akış HTTP hatası ({}): {}", durum, govde)
            }
            Self::AgHatasi(mesaj) => {
                write!(f, "Yukarı akış ağ bağlantı hatası: {}", mesaj)
            }
            Self::JsonHatasi(mesaj) => {
                write!(f, "Yukarı akış JSON ayrıştırma hatası: {}", mesaj)
            }
        }
    }
}

impl std::error::Error for YukariHatasi {}

#[derive(Clone)]
pub struct Yukari {
    ajan: ureq::Agent,
}

impl Default for Yukari {
    fn default() -> Self {
        Self::new()
    }
}

impl Yukari {
    pub fn new() -> Self {
        let config = ureq::config::Config::builder()
            .timeout_global(Some(Duration::from_secs(60)))
            .build();
        let ajan = ureq::Agent::new_with_config(config);
        Self { ajan }
    }

    /// OpenAI-biçimli istemci gövdesini yukarı akışa kayıpsız iletir ve yanıtı döner.
    pub async fn gonder(
        &self,
        hedef: &Hedef,
        istemci_govdesi: &Value,
        istenen_model: Option<&str>,
    ) -> Result<Value, YukariHatasi> {
        // 1. Anahtarı ortam değişkeninden oku (dosyaya veya loga asla yazılmaz)
        let anahtar = std::env::var(&hedef.anahtar_env)
            .map_err(|_| YukariHatasi::AnahtarYok(hedef.anahtar_env.clone()))?;

        if anahtar.trim().is_empty() {
            return Err(YukariHatasi::AnahtarYok(hedef.anahtar_env.clone()));
        }

        // 2. Yukarı akış modelini belirle
        let yukari_model = istenen_model.unwrap_or(&hedef.model);

        match hedef.bicim.as_str() {
            "openai" => self.gonder_openai(hedef, &anahtar, istemci_govdesi, yukari_model).await,
            "claude" => self.gonder_claude(hedef, &anahtar, istemci_govdesi, yukari_model).await,
            "gemini" => self.gonder_gemini(hedef, &anahtar, istemci_govdesi, yukari_model).await,
            diger => Err(YukariHatasi::Desteklenmiyor(format!("Bilinmeyen biçim: {}", diger))),
        }
    }

    async fn gonder_openai(
        &self,
        hedef: &Hedef,
        anahtar: &str,
        istemci_govdesi: &Value,
        yukari_model: &str,
    ) -> Result<Value, YukariHatasi> {
        // Gövdeyi kayıpsız kopyala, yalnız model ve stream alanını güncelle
        let mut yukari_govde = istemci_govdesi.clone();
        if let Some(obj) = yukari_govde.as_object_mut() {
            obj.insert("model".to_string(), Value::String(yukari_model.to_string()));
            obj.insert("stream".to_string(), Value::Bool(false));
        }

        let yol = hedef.yol.as_deref().unwrap_or("/v1/chat/completions");
        let url = format!("{}{}", hedef.taban_url, yol);
        let auth_header = format!("Bearer {}", anahtar);

        let ajan = self.ajan.clone();
        let timeout = hedef.zaman_asimi;

        tokio::task::spawn_blocking(move || {
            let res = ajan
                .post(&url)
                .header("Authorization", &auth_header)
                .header("Content-Type", "application/json")
                .config()
                .timeout_global(Some(timeout))
                .build()
                .send_json(&yukari_govde);

            match res {
                Ok(mut response) => {
                    let durum = response.status().as_u16();
                    if durum >= 200 && durum < 300 {
                        response
                            .body_mut()
                            .read_json::<Value>()
                            .map_err(|e| YukariHatasi::JsonHatasi(e.to_string()))
                    } else {
                        let hata_govdesi = response
                            .body_mut()
                            .read_to_string()
                            .unwrap_or_else(|_| String::new());
                        let kisaltilmis = if hata_govdesi.len() > 500 {
                            format!("{}...", &hata_govdesi[..500])
                        } else {
                            hata_govdesi
                        };
                        Err(YukariHatasi::HttpHatasi(durum, kisaltilmis))
                    }
                }
                Err(ureq::Error::StatusCode(status)) => {
                    Err(YukariHatasi::HttpHatasi(status, "HTTP Hata Durumu".to_string()))
                }
                Err(e) => Err(YukariHatasi::AgHatasi(e.to_string())),
            }
        })
        .await
        .map_err(|e| YukariHatasi::AgHatasi(format!("spawn_blocking hatası: {}", e)))?
    }

    async fn gonder_claude(
        &self,
        hedef: &Hedef,
        anahtar: &str,
        istemci_govdesi: &Value,
        yukari_model: &str,
    ) -> Result<Value, YukariHatasi> {
        let openai_istek: kervan_ceviri::OpenAiIstek = serde_json::from_value(istemci_govdesi.clone())
            .map_err(|e| YukariHatasi::JsonHatasi(format!("İstemci gövdesi ayrıştırılamadı: {}", e)))?;

        let claude_istek = kervan_ceviri::openai_den_claude_istegi(&openai_istek, yukari_model)
            .map_err(|e| match e {
                kervan_ceviri::CeviriHatasi::Desteklenmiyor(alan) => YukariHatasi::Desteklenmiyor(alan),
                kervan_ceviri::CeviriHatasi::GecersizIstek(m) => YukariHatasi::JsonHatasi(m),
                kervan_ceviri::CeviriHatasi::AyrirmaHatasi(m) => YukariHatasi::JsonHatasi(m),
            })?;

        let yukari_govde = serde_json::to_value(&claude_istek)
            .map_err(|e| YukariHatasi::JsonHatasi(e.to_string()))?;

        let yol = hedef.yol.as_deref().unwrap_or("/v1/messages");
        let url = format!("{}{}", hedef.taban_url, yol);
        let anahtar_kopya = anahtar.to_string();
        let model_kopya = yukari_model.to_string();

        let ajan = self.ajan.clone();
        let timeout = hedef.zaman_asimi;

        tokio::task::spawn_blocking(move || {
            let res = ajan
                .post(&url)
                .header("x-api-key", &anahtar_kopya)
                .header("anthropic-version", "2023-06-01")
                .header("Content-Type", "application/json")
                .config()
                .timeout_global(Some(timeout))
                .build()
                .send_json(&yukari_govde);

            match res {
                Ok(mut response) => {
                    let durum = response.status().as_u16();
                    if durum >= 200 && durum < 300 {
                        let claude_yanit = response
                            .body_mut()
                            .read_json::<kervan_ceviri::ClaudeYanit>()
                            .map_err(|e| YukariHatasi::JsonHatasi(e.to_string()))?;
                        let openai_yanit = kervan_ceviri::claude_dan_openai_yaniti(&claude_yanit, &model_kopya)
                            .map_err(|e| YukariHatasi::JsonHatasi(e.to_string()))?;
                        serde_json::to_value(&openai_yanit)
                            .map_err(|e| YukariHatasi::JsonHatasi(e.to_string()))
                    } else {
                        let hata_govdesi = response
                            .body_mut()
                            .read_to_string()
                            .unwrap_or_else(|_| String::new());
                        let kisaltilmis = if hata_govdesi.len() > 500 {
                            format!("{}...", &hata_govdesi[..500])
                        } else {
                            hata_govdesi
                        };
                        Err(YukariHatasi::HttpHatasi(durum, kisaltilmis))
                    }
                }
                Err(ureq::Error::StatusCode(status)) => {
                    Err(YukariHatasi::HttpHatasi(status, "HTTP Hata Durumu".to_string()))
                }
                Err(e) => Err(YukariHatasi::AgHatasi(e.to_string())),
            }
        })
        .await
        .map_err(|e| YukariHatasi::AgHatasi(format!("spawn_blocking hatası: {}", e)))?
    }

    async fn gonder_gemini(
        &self,
        hedef: &Hedef,
        anahtar: &str,
        istemci_govdesi: &Value,
        yukari_model: &str,
    ) -> Result<Value, YukariHatasi> {
        let openai_istek: kervan_ceviri::OpenAiIstek = serde_json::from_value(istemci_govdesi.clone())
            .map_err(|e| YukariHatasi::JsonHatasi(format!("İstemci gövdesi ayrıştırılamadı: {}", e)))?;

        let gemini_istek = kervan_ceviri::openai_den_gemini_istegi(&openai_istek, yukari_model)
            .map_err(|e| match e {
                kervan_ceviri::CeviriHatasi::Desteklenmiyor(alan) => YukariHatasi::Desteklenmiyor(alan),
                kervan_ceviri::CeviriHatasi::GecersizIstek(m) => YukariHatasi::JsonHatasi(m),
                kervan_ceviri::CeviriHatasi::AyrirmaHatasi(m) => YukariHatasi::JsonHatasi(m),
            })?;

        let yukari_govde = serde_json::to_value(&gemini_istek)
            .map_err(|e| YukariHatasi::JsonHatasi(e.to_string()))?;

        let yol = hedef.yol.clone().unwrap_or_else(|| {
            format!("/v1beta/models/{}:generateContent", yukari_model)
        });
        let url = format!("{}{}", hedef.taban_url, yol);
        let anahtar_kopya = anahtar.to_string();
        let model_kopya = yukari_model.to_string();

        let ajan = self.ajan.clone();
        let timeout = hedef.zaman_asimi;

        tokio::task::spawn_blocking(move || {
            let res = ajan
                .post(&url)
                .header("x-goog-api-key", &anahtar_kopya)
                .header("Content-Type", "application/json")
                .config()
                .timeout_global(Some(timeout))
                .build()
                .send_json(&yukari_govde);

            match res {
                Ok(mut response) => {
                    let durum = response.status().as_u16();
                    if durum >= 200 && durum < 300 {
                        let gemini_yanit = response
                            .body_mut()
                            .read_json::<kervan_ceviri::GeminiYanit>()
                            .map_err(|e| YukariHatasi::JsonHatasi(e.to_string()))?;
                        let openai_yanit = kervan_ceviri::gemini_den_openai_yaniti(&gemini_yanit, &model_kopya)
                            .map_err(|e| YukariHatasi::JsonHatasi(e.to_string()))?;
                        serde_json::to_value(&openai_yanit)
                            .map_err(|e| YukariHatasi::JsonHatasi(e.to_string()))
                    } else {
                        let hata_govdesi = response
                            .body_mut()
                            .read_to_string()
                            .unwrap_or_else(|_| String::new());
                        let kisaltilmis = if hata_govdesi.len() > 500 {
                            format!("{}...", &hata_govdesi[..500])
                        } else {
                            hata_govdesi
                        };
                        Err(YukariHatasi::HttpHatasi(durum, kisaltilmis))
                    }
                }
                Err(ureq::Error::StatusCode(status)) => {
                    Err(YukariHatasi::HttpHatasi(status, "HTTP Hata Durumu".to_string()))
                }
                Err(e) => Err(YukariHatasi::AgHatasi(e.to_string())),
            }
        })
        .await
        .map_err(|e| YukariHatasi::AgHatasi(format!("spawn_blocking hatası: {}", e)))?
    }
}
