//! `kervan-yukari` — REST yukarı akış istemcisi.
//!
//! OpenAI, Claude ve Gemini uyumlu REST sağlayıcılarına HTTP üzerinden
//! kayıpsız istek iletimi, yanıt alımı, devre kesici (circuit breaker),
//! soğuma takipçisi ve hata temizliğini sağlar.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use serde_json::Value;
use tokio::sync::Mutex;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum YukariHatasi {
    AnahtarYok(String),
    Desteklenmiyor(String),
    HttpHatasi(u16, String, Option<u64>),
    AgHatasi(String),
    JsonHatasi(String),
    DevreAcik { ad: String, kalan_sure_ms: u64 },
    Sogumada { ad: String, retry_after_saniye: u64 },
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
            Self::HttpHatasi(durum, govde, retry_after) => {
                if let Some(sn) = retry_after {
                    write!(f, "Yukarı akış HTTP hatası ({}) [Retry-After: {}s]: {}", durum, sn, govde)
                } else {
                    write!(f, "Yukarı akış HTTP hatası ({}): {}", durum, govde)
                }
            }
            Self::AgHatasi(mesaj) => {
                write!(f, "Yukarı akış ağ bağlantı hatası: {}", mesaj)
            }
            Self::JsonHatasi(mesaj) => {
                write!(f, "Yukarı akış JSON ayrıştırma hatası: {}", mesaj)
            }
            Self::DevreAcik { ad, kalan_sure_ms } => {
                write!(f, "Devre kesici açık: {} ({}ms sonra tekrar denenecek)", ad, kalan_sure_ms)
            }
            Self::Sogumada { ad, retry_after_saniye } => {
                write!(f, "Sağlayıcı soğumada: {} (Retry-After: {}s)", ad, retry_after_saniye)
            }
        }
    }
}

impl std::error::Error for YukariHatasi {}

/// Hata mesajındaki gizli anahtarları, başlıkları ve iç sistem yollarını temizler.
pub fn temizle_hata(govde: &str, gizli_anahtar: &str) -> String {
    let mut temiz = govde.to_string();

    // 1. Bilinen API anahtarını maskele
    let trimmed = gizli_anahtar.trim();
    if !trimmed.is_empty() {
        temiz = temiz.replace(trimmed, "[GIZLENDI]");
    }

    // 2. Bearer token ve authorization kalıplarını maskele
    if let Ok(re_bearer) = regex::Regex::new(r"(?i)bearer\s+([a-zA-Z0-9_\-\.]{8,})") {
        temiz = re_bearer.replace_all(&temiz, "Bearer [GIZLENDI]").to_string();
    }
    if let Ok(re_key) = regex::Regex::new(r#"(?i)(api[_-]?key|token|secret)["']?\s*[:=]\s*["']?([a-zA-Z0-9_\-\.]{8,})"#) {
        temiz = re_key.replace_all(&temiz, "$1=[GIZLENDI]").to_string();
    }

    // 3. Dosya yollarını maskele (Windows ve Unix iç yolları)
    if let Ok(re_win) = regex::Regex::new(r#"[a-zA-Z]:[\\/][^"'\s\r\n\t]+"#) {
        temiz = re_win.replace_all(&temiz, "[GIZLI_YOL]").to_string();
    }
    if let Ok(re_unix) = regex::Regex::new(r#"/(home|Users|var|etc|usr|tmp)/[^"'\s\r\n\t]+"#) {
        temiz = re_unix.replace_all(&temiz, "[GIZLI_YOL]").to_string();
    }

    // 4. Kısaltma (azami 500 karakter)
    if temiz.len() > 500 {
        format!("{}...", &temiz[..500])
    } else {
        temiz
    }
}

#[derive(Clone)]
pub struct Yukari {
    ajan: ureq::Agent,
    pub direnc: Arc<Mutex<kervan_direnc::DirencYoneticisi>>,
}

impl Default for Yukari {
    fn default() -> Self {
        Self::new()
    }
}

impl Yukari {
    pub fn new() -> Self {
        Self::yeni_direncli(kervan_direnc::DirencYoneticisi::default())
    }

    pub fn yeni_direncli(direnc: kervan_direnc::DirencYoneticisi) -> Self {
        let config = ureq::config::Config::builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(60)))
            .build();
        let ajan = ureq::Agent::new_with_config(config);
        Self {
            ajan,
            direnc: Arc::new(Mutex::new(direnc)),
        }
    }

    /// OpenAI-biçimli istemci gövdesini yukarı akışa kayıpsız iletir ve yanıtı döner.
    pub async fn gonder(
        &self,
        hedef: &Hedef,
        istemci_govdesi: &Value,
        istenen_model: Option<&str>,
    ) -> Result<Value, YukariHatasi> {
        // 1. Anahtarı ortam değişkeninden oku
        let anahtar = std::env::var(&hedef.anahtar_env)
            .map_err(|_| YukariHatasi::AnahtarYok(hedef.anahtar_env.clone()))?;

        if anahtar.trim().is_empty() {
            return Err(YukariHatasi::AnahtarYok(hedef.anahtar_env.clone()));
        }

        // 2. Dayanıklılık izni al (devre kesici ve soğuma kontrolü)
        {
            let mut d = self.direnc.lock().await;
            if let Err(e) = d.istek_izni_al(&hedef.ad) {
                return match e {
                    kervan_direnc::DirencHatasi::DevreAcik { ad, kalan_sure_ms } => {
                        Err(YukariHatasi::DevreAcik { ad, kalan_sure_ms })
                    }
                    kervan_direnc::DirencHatasi::Sogumada { ad, retry_after_saniye } => {
                        Err(YukariHatasi::Sogumada { ad, retry_after_saniye })
                    }
                    kervan_direnc::DirencHatasi::YariAcikDolu(ad) => {
                        Err(YukariHatasi::DevreAcik { ad, kalan_sure_ms: 1000 })
                    }
                    kervan_direnc::DirencHatasi::IslemHatasi(m) => {
                        Err(YukariHatasi::AgHatasi(m))
                    }
                };
            }
        }

        // 3. Yukarı akış modelini belirle
        let yukari_model = istenen_model.unwrap_or(&hedef.model);

        // 4. İsteği gönder
        let sonuc = match hedef.bicim.as_str() {
            "openai" => self.gonder_openai(hedef, &anahtar, istemci_govdesi, yukari_model).await,
            "claude" => self.gonder_claude(hedef, &anahtar, istemci_govdesi, yukari_model).await,
            "gemini" => self.gonder_gemini(hedef, &anahtar, istemci_govdesi, yukari_model).await,
            diger => Err(YukariHatasi::Desteklenmiyor(format!("Bilinmeyen biçim: {}", diger))),
        };

        // 5. Dayanıklılık durumunu güncelle
        {
            let mut d = self.direnc.lock().await;
            match &sonuc {
                Ok(_) => {
                    d.basari_kaydet(&hedef.ad);
                }
                Err(YukariHatasi::HttpHatasi(durum, _, retry_after)) => {
                    let durum = Some(*durum);
                    let retry_after = *retry_after;
                    d.hata_kaydet(&hedef.ad, durum, retry_after);
                }
                Err(YukariHatasi::AgHatasi(_)) => {
                    d.hata_kaydet(&hedef.ad, None, None);
                }
                Err(_) => {}
            }
        }

        sonuc
    }

    async fn gonder_openai(
        &self,
        hedef: &Hedef,
        anahtar: &str,
        istemci_govdesi: &Value,
        yukari_model: &str,
    ) -> Result<Value, YukariHatasi> {
        let mut yukari_govde = istemci_govdesi.clone();
        if let Some(obj) = yukari_govde.as_object_mut() {
            obj.insert("model".to_string(), Value::String(yukari_model.to_string()));
            obj.insert("stream".to_string(), Value::Bool(false));
        }

        let yol = hedef.yol.as_deref().unwrap_or("/v1/chat/completions");
        let url = format!("{}{}", hedef.taban_url, yol);
        let auth_header = format!("Bearer {}", anahtar);
        let anahtar_kopya = anahtar.to_string();

        let ajan = self.ajan.clone();
        let timeout = hedef.zaman_asimi;

        tokio::task::spawn_blocking(move || {
            let res = ajan
                .post(&url)
                .header("Authorization", &auth_header)
                .header("Content-Type", "application/json")
                .config()
                .http_status_as_error(false)
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
                            .map_err(|e| YukariHatasi::JsonHatasi(temizle_hata(&e.to_string(), &anahtar_kopya)))
                    } else {
                        let retry_after = response.headers().get("retry-after")
                            .or_else(|| response.headers().get("Retry-After"))
                            .and_then(|v| v.to_str().ok())
                            .and_then(|v| v.trim().parse::<u64>().ok());

                        let hata_govdesi = response
                            .body_mut()
                            .read_to_string()
                            .unwrap_or_else(|_| String::new());
                        let temizlenmis = temizle_hata(&hata_govdesi, &anahtar_kopya);
                        Err(YukariHatasi::HttpHatasi(durum, temizlenmis, retry_after))
                    }
                }
                Err(ureq::Error::StatusCode(status)) => {
                    Err(YukariHatasi::HttpHatasi(status, "HTTP Hata Durumu".to_string(), None))
                }
                Err(e) => Err(YukariHatasi::AgHatasi(temizle_hata(&e.to_string(), &anahtar_kopya))),
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
                .http_status_as_error(false)
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
                            .map_err(|e| YukariHatasi::JsonHatasi(temizle_hata(&e.to_string(), &anahtar_kopya)))?;
                        let openai_yanit = kervan_ceviri::claude_dan_openai_yaniti(&claude_yanit, &model_kopya)
                            .map_err(|e| YukariHatasi::JsonHatasi(temizle_hata(&e.to_string(), &anahtar_kopya)))?;
                        serde_json::to_value(&openai_yanit)
                            .map_err(|e| YukariHatasi::JsonHatasi(e.to_string()))
                    } else {
                        let retry_after = response.headers().get("retry-after")
                            .or_else(|| response.headers().get("Retry-After"))
                            .and_then(|v| v.to_str().ok())
                            .and_then(|v| v.trim().parse::<u64>().ok());

                        let hata_govdesi = response
                            .body_mut()
                            .read_to_string()
                            .unwrap_or_else(|_| String::new());
                        let temizlenmis = temizle_hata(&hata_govdesi, &anahtar_kopya);
                        Err(YukariHatasi::HttpHatasi(durum, temizlenmis, retry_after))
                    }
                }
                Err(ureq::Error::StatusCode(status)) => {
                    Err(YukariHatasi::HttpHatasi(status, "HTTP Hata Durumu".to_string(), None))
                }
                Err(e) => Err(YukariHatasi::AgHatasi(temizle_hata(&e.to_string(), &anahtar_kopya))),
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
                .http_status_as_error(false)
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
                            .map_err(|e| YukariHatasi::JsonHatasi(temizle_hata(&e.to_string(), &anahtar_kopya)))?;
                        let openai_yanit = kervan_ceviri::gemini_den_openai_yaniti(&gemini_yanit, &model_kopya)
                            .map_err(|e| YukariHatasi::JsonHatasi(temizle_hata(&e.to_string(), &anahtar_kopya)))?;
                        serde_json::to_value(&openai_yanit)
                            .map_err(|e| YukariHatasi::JsonHatasi(e.to_string()))
                    } else {
                        let retry_after = response.headers().get("retry-after")
                            .or_else(|| response.headers().get("Retry-After"))
                            .and_then(|v| v.to_str().ok())
                            .and_then(|v| v.trim().parse::<u64>().ok());

                        let hata_govdesi = response
                            .body_mut()
                            .read_to_string()
                            .unwrap_or_else(|_| String::new());
                        let temizlenmis = temizle_hata(&hata_govdesi, &anahtar_kopya);
                        Err(YukariHatasi::HttpHatasi(durum, temizlenmis, retry_after))
                    }
                }
                Err(ureq::Error::StatusCode(status)) => {
                    Err(YukariHatasi::HttpHatasi(status, "HTTP Hata Durumu".to_string(), None))
                }
                Err(e) => Err(YukariHatasi::AgHatasi(temizle_hata(&e.to_string(), &anahtar_kopya))),
            }
        })
        .await
        .map_err(|e| YukariHatasi::AgHatasi(format!("spawn_blocking hatası: {}", e)))?
    }
}
