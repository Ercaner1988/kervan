use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Yürütme talebi
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YurutmeIstegi {
    pub saglayici_id: String,
    pub model: String,
    pub istem: String,
    pub stream: bool,
    pub api_anahtari: Option<String>,
    #[serde(default)]
    pub ek_parametreler: serde_json::Value,
}

/// Yürütme sonucu
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct YurutmeYaniti {
    pub metin: String,
    pub kullanim_token: usize,
    pub model: String,
}

#[derive(Error, Debug)]
pub enum YurutmeHatasi {
    #[error("Sağlayıcı bulunamadı: {0}")]
    SaglayiciBulunamadi(String),
    #[error("Yetkilendirme hatası: API anahtarı eksik veya geçersiz")]
    YetkisizErisim,
    #[error("Ağ / İletişim hatası: {0}")]
    IletisimHatasi(String),
    #[error("Yukarı akış hata kodu {0}: {1}")]
    YukariAkisHatasi(u16, String),
    #[error("Format ayrıştırma hatası: {0}")]
    FormatHatasi(String),
}
