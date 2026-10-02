pub mod modeller;
pub mod oauth;
pub mod rest;
pub mod tarayici;

pub use modeller::{YurutmeHatasi, YurutmeIstegi, YurutmeYaniti};
pub use oauth::{OAuthBelirteci, OAuthIstemcisi};
pub use rest::{ClaudeRestYurutucu, GeminiRestYurutucu, StandartRestYurutucu};
pub use tarayici::TarayiciOturumYurutucu;

use kervan_katalog::Katalog;

/// Sağlayıcı formatına göre doğru yürütücüyü seçen fabrika
#[derive(Debug, Clone, Default)]
pub struct YurutucuFabrikasi {
    pub standart_rest: StandartRestYurutucu,
    pub claude_rest: ClaudeRestYurutucu,
    pub gemini_rest: GeminiRestYurutucu,
    pub tarayici_yurutucu: TarayiciOturumYurutucu,
}

impl YurutucuFabrikasi {
    pub fn yeni() -> Self {
        Self::default()
    }

    /// Sağlayıcı formatına uygun istek yükünü hazırlar
    pub fn istek_hazirla(
        &self,
        katalog: &Katalog,
        istek: &YurutmeIstegi,
    ) -> Result<serde_json::Value, YurutmeHatasi> {
        let saglayici = katalog
            .saglayici_bul(&istek.saglayici_id)
            .ok_or_else(|| YurutmeHatasi::SaglayiciBulunamadi(istek.saglayici_id.clone()))?;

        match saglayici.format.as_str() {
            "claude" => Ok(self.claude_rest.istek_govdesi_olustur(istek)),
            "gemini" => Ok(self.gemini_rest.istek_govdesi_olustur(istek)),
            "tarayici" | "web" => Ok(self.tarayici_yurutucu.yuk_hazirla(istek)),
            _ => Ok(self.standart_rest.istek_govdesi_olustur(istek)),
        }
    }

    /// Sağlayıcı formatına göre dönen yanıtı ayrıştırır
    pub fn yaniti_isle(
        &self,
        katalog: &Katalog,
        saglayici_id: &str,
        ham_yanit: &str,
    ) -> Result<YurutmeYaniti, YurutmeHatasi> {
        let saglayici = katalog
            .saglayici_bul(saglayici_id)
            .ok_or_else(|| YurutmeHatasi::SaglayiciBulunamadi(saglayici_id.to_string()))?;

        match saglayici.format.as_str() {
            "claude" => self.claude_rest.yaniti_isle(ham_yanit),
            "gemini" => self.gemini_rest.yaniti_isle(ham_yanit),
            "tarayici" | "web" => self.tarayici_yurutucu.eklenti_yanitini_isle(ham_yanit),
            _ => self.standart_rest.yaniti_isle(ham_yanit),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standart_rest_istek_ve_yanit() {
        let fabrika = YurutucuFabrikasi::yeni();
        let katalog = Katalog::yerlesik();

        let istek = YurutmeIstegi {
            saglayici_id: "deepseek".to_string(),
            model: "deepseek-chat".to_string(),
            istem: "Merhaba!".to_string(),
            stream: false,
            api_anahtari: Some("sk-test".to_string()),
            ek_parametreler: serde_json::Value::Null,
        };

        let govde = fabrika.istek_hazirla(&katalog, &istek).unwrap();
        assert_eq!(govde["model"], "deepseek-chat");

        let sahte_yanit = serde_json::json!({
            "id": "chatcmpl-1",
            "model": "deepseek-chat",
            "choices": [
                {
                    "message": { "role": "assistant", "content": "Selam, ben DeepSeek!" }
                }
            ],
            "usage": { "total_tokens": 42 }
        })
        .to_string();

        let sonuc = fabrika
            .yaniti_isle(&katalog, "deepseek", &sahte_yanit)
            .unwrap();
        assert_eq!(sonuc.metin, "Selam, ben DeepSeek!");
        assert_eq!(sonuc.kullanim_token, 42);
    }

    #[test]
    fn test_claude_rest_istek_ve_yanit() {
        let fabrika = YurutucuFabrikasi::yeni();
        let katalog = Katalog::yerlesik();

        let istek = YurutmeIstegi {
            saglayici_id: "anthropic".to_string(),
            model: "claude-fable-5".to_string(),
            istem: "Fable'a selam".to_string(),
            stream: false,
            api_anahtari: Some("sk-ant-test".to_string()),
            ek_parametreler: serde_json::Value::Null,
        };

        let govde = fabrika.istek_hazirla(&katalog, &istek).unwrap();
        assert_eq!(govde["model"], "claude-fable-5");
        assert_eq!(govde["max_tokens"], 4096);

        let sahte_claude_yaniti = serde_json::json!({
            "id": "msg-123",
            "model": "claude-fable-5",
            "content": [
                { "type": "text", "text": "Selamlar! Fable 5 hazır." }
            ],
            "usage": {
                "input_tokens": 15,
                "output_tokens": 25
            }
        })
        .to_string();

        let sonuc = fabrika
            .yaniti_isle(&katalog, "anthropic", &sahte_claude_yaniti)
            .unwrap();
        assert_eq!(sonuc.metin, "Selamlar! Fable 5 hazır.");
        assert_eq!(sonuc.kullanim_token, 40);
    }

    #[test]
    fn test_gemini_rest_istek_ve_yanit() {
        let fabrika = YurutucuFabrikasi::yeni();
        let katalog = Katalog::yerlesik();

        let istek = YurutmeIstegi {
            saglayici_id: "gemini".to_string(),
            model: "gemini-2.0-flash".to_string(),
            istem: "Gemini testi".to_string(),
            stream: false,
            api_anahtari: Some("ai-gemini-key".to_string()),
            ek_parametreler: serde_json::Value::Null,
        };

        let govde = fabrika.istek_hazirla(&katalog, &istek).unwrap();
        assert_eq!(govde["contents"][0]["parts"][0]["text"], "Gemini testi");

        let sahte_gemini_yaniti = serde_json::json!({
            "candidates": [
                {
                    "content": {
                        "parts": [{ "text": "Gemini yanıt verdi." }]
                    }
                }
            ],
            "usageMetadata": { "totalTokenCount": 22 }
        })
        .to_string();

        let sonuc = fabrika
            .yaniti_isle(&katalog, "gemini", &sahte_gemini_yaniti)
            .unwrap();
        assert_eq!(sonuc.metin, "Gemini yanıt verdi.");
        assert_eq!(sonuc.kullanim_token, 22);
    }

    #[test]
    fn test_tarayici_oturum_yurutucu() {
        let yurutucu = TarayiciOturumYurutucu;
        let istek = YurutmeIstegi {
            saglayici_id: "chatgpt-web".to_string(),
            model: "gpt-4o".to_string(),
            istem: "Tarayıcı sorusu".to_string(),
            stream: false,
            api_anahtari: None,
            ek_parametreler: serde_json::Value::Null,
        };

        let yuk = yurutucu.yuk_hazirla(&istek);
        assert_eq!(yuk["action"], "sor");
        assert_eq!(yuk["provider"], "chatgpt-web");

        let sahte_eklenti_yaniti = serde_json::json!({
            "yanit": "Web arayüzünden gelen cevap",
            "model": "gpt-4o"
        })
        .to_string();

        let sonuc = yurutucu
            .eklenti_yanitini_isle(&sahte_eklenti_yaniti)
            .unwrap();
        assert_eq!(sonuc.metin, "Web arayüzünden gelen cevap");
    }

    #[test]
    fn test_sahte_oauth_akis_dogrulamasi() {
        let oauth = OAuthIstemcisi::yeni(
            "test-client-id",
            "test-client-secret",
            "https://auth.example.com/oauth/token",
        );

        let govde = oauth.kod_takas_govdesi("auth-code-12345", "http://localhost:20129/callback");
        assert_eq!(govde[0].1, "authorization_code");
        assert_eq!(govde[1].1, "auth-code-12345");

        let sahte_token_yaniti = serde_json::json!({
            "access_token": "mock-access-token-xyz",
            "token_type": "Bearer",
            "expires_in": 3600,
            "refresh_token": "mock-refresh-token-abc",
            "scope": "read write"
        })
        .to_string();

        let belirtec = oauth.belirtec_ayristir(&sahte_token_yaniti).unwrap();
        assert_eq!(belirtec.access_token, "mock-access-token-xyz");
        assert_eq!(belirtec.expires_in, 3600);
        assert_eq!(
            belirtec.refresh_token,
            Some("mock-refresh-token-abc".to_string())
        );
    }
}
