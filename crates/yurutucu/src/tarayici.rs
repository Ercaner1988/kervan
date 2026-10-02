use crate::modeller::{YurutmeHatasi, YurutmeIstegi, YurutmeYaniti};

/// Kervan Web Oturum ve Tarayıcı Köprüsü Yürütücüsü (ADR 0003 & §5-1)
#[derive(Debug, Clone, Default)]
pub struct TarayiciOturumYurutucu;

impl TarayiciOturumYurutucu {
    /// Web oturumu için istek yükü hazırlar
    pub fn yuk_hazirla(&self, istek: &YurutmeIstegi) -> serde_json::Value {
        serde_json::json!({
            "action": "sor",
            "provider": istek.saglayici_id,
            "model": istek.model,
            "istem": istek.istem,
            "stream": istek.stream
        })
    }

    /// Tarayıcı eklentisinden dönen yanıtı ayrıştırır
    pub fn eklenti_yanitini_isle(&self, json_metni: &str) -> Result<YurutmeYaniti, YurutmeHatasi> {
        let deger: serde_json::Value = serde_json::from_str(json_metni)
            .map_err(|e| YurutmeHatasi::FormatHatasi(e.to_string()))?;

        if let Some(hata) = deger["error"].as_str() {
            return Err(YurutmeHatasi::IletisimHatasi(hata.to_string()));
        }

        let metin = deger["yanit"].as_str().unwrap_or("").to_string();
        let model = deger["model"]
            .as_str()
            .unwrap_or("tarayici-model")
            .to_string();
        let token = (metin.len() / 4).max(1);

        Ok(YurutmeYaniti {
            metin,
            kullanim_token: token,
            model,
        })
    }
}
