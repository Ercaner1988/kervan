use axum::{
    Json,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde_json::json;
use std::net::IpAddr;

/// OmniRoute Sert Kural #15 & #17: Loopback koruması ve API anahtarı denetimi
#[derive(Debug, Clone)]
pub struct GuvenlikPolitikasi {
    pub yalniz_loopback: bool,
    pub api_anahtari: Option<String>,
}

impl Default for GuvenlikPolitikasi {
    fn default() -> Self {
        Self {
            yalniz_loopback: true,
            api_anahtari: None,
        }
    }
}

impl GuvenlikPolitikasi {
    /// İstemcinin IP adresi ve istek başlıklarına göre erişim izinli mi?
    pub fn erisim_kontrol(
        &self,
        istemci_ip: Option<IpAddr>,
        headers: &HeaderMap,
    ) -> Result<(), Box<Response>> {
        // 1. Loopback kontrolü
        if self.yalniz_loopback
            && let Some(ip) = istemci_ip
            && !ip.is_loopback()
        {
            let hata = json!({
                "error": {
                    "message": "Erişim reddedildi: Kervan varsayılan olarak yalnız loopback (127.0.0.1) bağlantılarına izin verir.",
                    "type": "access_denied",
                    "code": "non_loopback_forbidden"
                }
            });
            return Err(Box::new(
                (StatusCode::FORBIDDEN, Json(hata)).into_response(),
            ));
        }

        // 2. API Anahtarı kontrolü (varsa doğrulanır)
        if let Some(ref beklenen_anahtar) = self.api_anahtari {
            let auth_header = headers.get("authorization").and_then(|h| h.to_str().ok());
            let api_key_header = headers.get("x-api-key").and_then(|h| h.to_str().ok());

            let mut eslesti = false;
            if let Some(auth) = auth_header
                && let Some(token) = auth.strip_prefix("Bearer ")
                && token.trim() == beklenen_anahtar
            {
                eslesti = true;
            }
            if let Some(key) = api_key_header
                && key.trim() == beklenen_anahtar
            {
                eslesti = true;
            }

            if !eslesti {
                let hata = json!({
                    "error": {
                        "message": "Geçersiz veya eksik API anahtarı.",
                        "type": "authentication_error",
                        "code": "invalid_api_key"
                    }
                });
                return Err(Box::new(
                    (StatusCode::UNAUTHORIZED, Json(hata)).into_response(),
                ));
            }
        }

        Ok(())
    }

    /// OmniRoute Sert Kural #12: Ham hata yığınını ve veritabanı hatalarını istemciye sızdırma
    pub fn guvenli_hata_yaniti(durum: StatusCode, mesaj: &str) -> Response {
        let temiz_mesaj = if mesaj.contains("sqlite") || mesaj.contains("syntax error") {
            "İç sunucu işlemi sırasında bir hata oluştu."
        } else {
            mesaj
        };

        let hata = json!({
            "error": {
                "message": temiz_mesaj,
                "type": "api_error",
                "code": durum.as_u16()
            }
        });
        (durum, Json(hata)).into_response()
    }
}
