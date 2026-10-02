use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Sağlayıcı bünyesindeki model tanımı
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelTanimi {
    pub id: String,
    pub ad: String,
    pub baglam: usize,
    pub dusunme: bool,
    pub gorsel: bool,
}

/// OmniRoute kayıt defterinden dönüştürülmüş sağlayıcı tanımı
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SaglayiciTanimi {
    pub id: String,
    pub ad: String,
    pub format: String,
    pub taban_url: String,
    pub url_son_ek: String,
    pub auth_tipi: String,
    pub auth_basligi: String,
    pub varsayilan_baglam: usize,
    pub basliklar: HashMap<String, String>,
    pub modeller: Vec<ModelTanimi>,
}

/// Veri-güdümlü Sağlayıcı ve Model Kataloğu
#[derive(Debug, Clone)]
pub struct Katalog {
    saglayicilar: Vec<SaglayiciTanimi>,
    id_haritasi: HashMap<String, usize>,
}

impl Default for Katalog {
    fn default() -> Self {
        Self::yerlesik()
    }
}

impl Katalog {
    /// Derleme anında gömülen JSON dosyasından kataloğu yükler
    pub fn yerlesik() -> Self {
        const KATALOG_VERISI: &str = include_str!("../../../port/katalog/saglayicilar.json");
        Self::json_dan(KATALOG_VERISI).unwrap_or_else(|_| Self {
            saglayicilar: Vec::new(),
            id_haritasi: HashMap::new(),
        })
    }

    /// JSON dizesinden katalog oluşturur
    pub fn json_dan(json_metni: &str) -> Result<Self, serde_json::Error> {
        let saglayicilar: Vec<SaglayiciTanimi> = serde_json::from_str(json_metni)?;
        let mut id_haritasi = HashMap::new();
        for (i, s) in saglayicilar.iter().enumerate() {
            id_haritasi.insert(s.id.to_ascii_lowercase(), i);
        }
        Ok(Self {
            saglayicilar,
            id_haritasi,
        })
    }

    /// ID ile sağlayıcıyı bulur
    pub fn saglayici_bul(&self, id: &str) -> Option<&SaglayiciTanimi> {
        let aranan = id.to_ascii_lowercase();
        self.id_haritasi
            .get(&aranan)
            .and_then(|&idx| self.saglayicilar.get(idx))
    }

    /// Model ID'si ile eşleşen sağlayıcı ve model ikililerini bulur
    pub fn model_ara(&self, model_id: &str) -> Vec<(&SaglayiciTanimi, &ModelTanimi)> {
        let aranan = model_id.to_ascii_lowercase();
        let mut bulunanlar = Vec::new();
        for saglayici in &self.saglayicilar {
            for model in &saglayici.modeller {
                if model.id.to_ascii_lowercase() == aranan {
                    bulunanlar.push((saglayici, model));
                }
            }
        }
        bulunanlar
    }

    /// Toplam kayıtlı sağlayıcı sayısı
    pub fn toplam_saglayici(&self) -> usize {
        self.saglayicilar.len()
    }

    /// Toplam kayıtlı model sayısı
    pub fn toplam_model(&self) -> usize {
        self.saglayicilar.iter().map(|s| s.modeller.len()).sum()
    }

    /// Tüm sağlayıcıların referans listesi
    pub fn tum_saglayicilar(&self) -> &[SaglayiciTanimi] {
        &self.saglayicilar
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yerlesik_katalog_yukleme() {
        let katalog = Katalog::yerlesik();
        assert_eq!(
            katalog.toplam_saglayici(),
            276,
            "Tam olarak 276 sağlayıcı yüklenmeli"
        );
        assert!(
            katalog.toplam_model() > 500,
            "En az 500 model kataloğa dahil olmalı"
        );
    }

    #[test]
    fn test_saglayici_bulma() {
        let katalog = Katalog::yerlesik();

        let anthropic = katalog.saglayici_bul("anthropic");
        assert!(anthropic.is_some(), "Anthropic sağlayıcısı bulunmalı");
        let a = anthropic.unwrap();
        assert_eq!(a.format, "claude");
        assert_eq!(a.auth_basligi, "x-api-key");

        let openai = katalog.saglayici_bul("openai");
        assert!(openai.is_some(), "OpenAI sağlayıcısı bulunmalı");
        let o = openai.unwrap();
        assert_eq!(o.format, "openai");

        let gemini = katalog.saglayici_bul("gemini");
        assert!(gemini.is_some(), "Gemini sağlayıcısı bulunmalı");
    }

    #[test]
    fn test_model_arama() {
        let katalog = Katalog::yerlesik();
        let sonuclar = katalog.model_ara("claude-fable-5");
        assert!(
            !sonuclar.is_empty(),
            "claude-fable-5 modeli en az bir sağlayıcıda bulunmalı"
        );
        assert_eq!(sonuclar[0].0.id, "anthropic");
    }
}
