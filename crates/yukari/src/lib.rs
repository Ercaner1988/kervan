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
            "claude" => {
                // P6 altın parite aşamasında ceviri crate'ine bağlanacak
                Err(YukariHatasi::Desteklenmiyor("claude biçimi P6 dilimindedir".into()))
            }
            "gemini" => {
                // P6 altın parite aşamasında ceviri crate'ine bağlanacak
                Err(YukariHatasi::Desteklenmiyor("gemini biçimi P6 dilimindedir".into()))
            }
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
}
