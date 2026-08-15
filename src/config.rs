//! `kervan.toml` — hesap listesi ve hiz butcesi.
//!
//! Dosya yoksa tek hesapli makul bir varsayilanla calisir; bu sayede ilk kullanimda
//! yapilandirma yazmak gerekmez.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub accounts: Vec<Account>,
    #[serde(default)]
    pub pacing: Pacing,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Account {
    /// Profil dizini adi. Ayni ad = ayni oturum.
    pub name: String,
    /// Simdilik yalniz "gemini". Saglayici genislemesi Asama 4.
    #[serde(default = "varsayilan_saglayici")]
    pub provider: String,
}

fn varsayilan_saglayici() -> String {
    "gemini".into()
}

/// Hiz butcesi — plandaki "insan hizinda davranmak" listesinin kod karsiligi.
/// Bot tespitini yenmek DEGIL, gercekten normal bir kullanici gibi davranmak:
/// yuku azaltir, hesabi yasatir.
#[derive(Debug, Deserialize, Clone)]
pub struct Pacing {
    /// Ayni hesaba ardisik iki istek arasindaki en az sure.
    pub min_delay_ms: u64,
    /// Uzerine eklenen rastgele sapma. Sabit araliklarla istek atmak, insan
    /// davranisindan en kolay ayirt edilen oruntudur.
    pub jitter_ms: u64,
    /// Hesap basina saatlik tavan. 0 = sinirsiz.
    pub hourly_cap: u32,
}

impl Default for Pacing {
    fn default() -> Self {
        // Muhafazakar varsayilanlar: bir insanin sohbet temposu.
        Self { min_delay_ms: 3000, jitter_ms: 2000, hourly_cap: 30 }
    }
}

impl Config {
    pub fn load(yol: &Path) -> Result<Self> {
        if !yol.exists() {
            return Ok(Self::varsayilan());
        }
        let metin = std::fs::read_to_string(yol)
            .with_context(|| format!("yapilandirma okunamadi: {}", yol.display()))?;
        let mut c: Config = toml::from_str(&metin)
            .with_context(|| format!("yapilandirma ayristirilamadi: {}", yol.display()))?;
        if c.accounts.is_empty() {
            c.accounts = Self::varsayilan().accounts;
        }
        Ok(c)
    }

    fn varsayilan() -> Self {
        Self {
            accounts: vec![Account { name: "ercan-gemini-1".into(), provider: varsayilan_saglayici() }],
            pacing: Pacing::default(),
        }
    }
}

pub fn varsayilan_yol() -> PathBuf {
    std::env::var_os("KERVAN_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("kervan.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hesaplar_ve_hiz_butcesi_okunur() {
        let t = r#"
            [[accounts]]
            name = "a"
            [[accounts]]
            name = "b"
            provider = "gemini"
            [pacing]
            min_delay_ms = 1000
            jitter_ms = 500
            hourly_cap = 10
        "#;
        let c: Config = toml::from_str(t).unwrap();
        assert_eq!(c.accounts.len(), 2);
        assert_eq!(c.accounts[0].name, "a");
        // provider verilmezse varsayilan dolmali
        assert_eq!(c.accounts[0].provider, "gemini");
        assert_eq!(c.pacing.hourly_cap, 10);
    }

    /// pacing blogu hic yoksa muhafazakar varsayilan gelmeli — yapilandirma
    /// eksikligi "sinirsiz hiz" anlamina GELMEMELI.
    #[test]
    fn eksik_pacing_guvenli_varsayilana_duser() {
        let c: Config = toml::from_str("[[accounts]]\nname = \"a\"\n").unwrap();
        assert!(c.pacing.min_delay_ms >= 1000);
        assert!(c.pacing.hourly_cap > 0);
    }
}
