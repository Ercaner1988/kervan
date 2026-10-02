use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthBelirteci {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OAuthIstemcisi {
    pub client_id: String,
    pub client_secret: String,
    pub token_url: String,
}

impl OAuthIstemcisi {
    pub fn yeni(
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
        token_url: impl Into<String>,
    ) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            token_url: token_url.into(),
        }
    }

    /// Yetkilendirme kodu (auth code) ile erişim belirteci alma isteğini oluşturur
    pub fn kod_takas_govdesi(&self, kod: &str, redirect_uri: &str) -> Vec<(&'static str, String)> {
        vec![
            ("grant_type", "authorization_code".to_string()),
            ("code", kod.to_string()),
            ("redirect_uri", redirect_uri.to_string()),
            ("client_id", self.client_id.clone()),
            ("client_secret", self.client_secret.clone()),
        ]
    }

    /// Yenileme belirteci (refresh token) ile yeni belirteç alma isteğini oluşturur
    pub fn yenileme_govdesi(&self, refresh_token: &str) -> Vec<(&'static str, String)> {
        vec![
            ("grant_type", "refresh_token".to_string()),
            ("refresh_token", refresh_token.to_string()),
            ("client_id", self.client_id.clone()),
            ("client_secret", self.client_secret.clone()),
        ]
    }

    /// JSON yanıtından OAuth belirtecini ayrıştırır
    pub fn belirtec_ayristir(&self, json_metni: &str) -> Result<OAuthBelirteci, serde_json::Error> {
        serde_json::from_str(json_metni)
    }
}
