//! `kervan.toml` — hesap listesi ve hiz butcesi.
//!
//! Dosya yoksa tek hesapli makul bir varsayilanla calisir; bu sayede ilk kullanimda
//! yapilandirma yazmak gerekmez.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    /// Bir tarayici kac dakika bosta kalinca kapatilir. 0 = hic kapatma.
    ///
    /// Hesap sayisinin tavani kod degil bellek: olculdu, kimlik basina ~344 MB.
    /// Bosta kapatma olmadan 20 hesap ~7 GB tutar; bununla yalniz o an calisanlar
    /// acik kalir. Oturum diskte oldugu icin kapatmak veri kaybetmez.
    #[serde(default = "varsayilan_bosta_dakika")]
    pub bosta_dakika: u64,
    #[serde(default)]
    pub accounts: Vec<Account>,
    #[serde(default)]
    pub pacing: Pacing,
    #[serde(default)]
    pub kota: Kota,
    /// Saglayici basina secici/adres ezmeleri. Site arayuzu degisince kod degil
    /// bu bolum degisir.
    #[serde(default)]
    pub providers: std::collections::HashMap<String, SaglayiciAyar>,
    /// REST yukarı akış sağlayıcıları ([rest.<ad>]).
    #[serde(default)]
    pub rest: std::collections::HashMap<String, RestAyar>,
}

/// REST yukarı akış sağlayıcı yapılandırması ([rest.<ad>]).
#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
pub struct RestAyar {
    pub bicim: String,
    pub taban_url: String,
    pub anahtar_env: String,
    pub model: String,
    #[serde(default)]
    pub yol: Option<String>,
}

impl RestAyar {
    pub fn dogrula(&self, ad: &str) -> Result<()> {
        match self.bicim.as_str() {
            "openai" | "claude" | "gemini" => Ok(()),
            diger => anyhow::bail!(
                "Geçersiz REST sağlayıcı biçimi: '{}' ([rest.{}])",
                diger,
                ad
            ),
        }
    }
}

/// Yerlesik saglayici tanimini kismen ezer; verilmeyen alan varsayilanda kalir.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct SaglayiciAyar {
    pub url: Option<String>,
    pub giris_kalibi: Option<String>,
    pub prompt_input: Option<String>,
    pub login_hint: Option<String>,
    pub response: Option<String>,
    /// Bu saglayiciya ayni anda kac istek gidebilir. Verilmezse 1.
    ///
    /// Olculdu: ayni siteye ayni anda yedi profille gitmek Cloudflare'i interaktif
    /// sinamaya gecirdi. Cok hesapli olmanin amaci es zamanlilik degil SUREKLILIK,
    /// o yuzden varsayilan sirali. Cloudflare arkasinda olmayan bir saglayicida
    /// hiz gerekiyorsa burada yukseltilir.
    pub eszamanli: Option<u32>,
    /// Site Cloudflare arkasinda mi? Verilirse yerlesik OLCUMU ezer.
    ///
    /// Tek etkisi: bu saglayicinin sekmesi acikken tarayici bosta kapatilmaz.
    pub cloudflare: Option<bool>,
}

/// Kota ogrenicinin ayarlari.
#[derive(Debug, Deserialize, Clone)]
pub struct Kota {
    /// Saglayicinin limit mesajinda gecen ifadeler. Varsayilanlar makul TAHMIN;
    /// gercek metni gorunce buraya eklenir — kod degistirmeden, yeniden derlemeden.
    #[serde(default = "varsayilan_isaretler")]
    pub isaretler: Vec<String>,
    /// Limit gorulunce, pencere daha ogrenilmemisken ne kadar beklenir (dakika).
    #[serde(default = "varsayilan_kota_bekleme")]
    pub bilinmeyen_pencere_dk: u64,
}

fn varsayilan_isaretler() -> Vec<String> {
    vec![
        "reached your limit".into(),
        "sınırına ulaştınız".into(),
        "limitine ulaştınız".into(),
        "kotanız doldu".into(),
    ]
}

fn varsayilan_kota_bekleme() -> u64 {
    60
}

impl Default for Kota {
    fn default() -> Self {
        Self {
            isaretler: varsayilan_isaretler(),
            bilinmeyen_pencere_dk: varsayilan_kota_bekleme(),
        }
    }
}

/// Bir KIMLIK: profil dizini ve o profilde oturum acilmis saglayicilar.
///
/// Bir Google oturumu profildeki tum saglayicilara yetiyor (olculdu: Gemini acikken
/// Perplexity'ye "Continue with Google" ile sifresiz girildi), o yuzden hesap basina
/// ayri profil degil, KIMLIK basina tek profil tutuluyor.
#[derive(Debug, Deserialize, Clone)]
pub struct Account {
    /// Profil dizini adi. Ayni ad = ayni oturum.
    pub name: String,
    /// Tekil bicim — eski yapilandirmalar kirilmasin diye korunuyor.
    #[serde(default)]
    pub provider: Option<String>,
    /// Bu profilde acik olan saglayicilar. Her biri kendi sekmesinde oturur.
    #[serde(default)]
    pub providers: Vec<String>,
}

impl Account {
    pub fn saglayicilar(&self) -> Vec<String> {
        if !self.providers.is_empty() {
            return self.providers.clone();
        }
        vec![self.provider.clone().unwrap_or_else(varsayilan_saglayici)]
    }
}

fn varsayilan_saglayici() -> String {
    "gemini".into()
}

/// Bir tarayiciyi yeniden acmak ~5 sn; 15 dk once kullanilmis bir hesabin hemen
/// tekrar gelmesi beklenmez. Kisa tutmak surekli ac/kapa, uzun tutmak bosuna
/// bellek demek olurdu.
fn varsayilan_bosta_dakika() -> u64 {
    15
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
        Self {
            min_delay_ms: 3000,
            jitter_ms: 2000,
            hourly_cap: 30,
        }
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
        c.dogrula()?;
        Ok(c)
    }

    pub fn dogrula(&self) -> Result<()> {
        for (ad, r) in &self.rest {
            r.dogrula(ad)?;
        }
        Ok(())
    }

    fn varsayilan() -> Self {
        Self {
            bosta_dakika: varsayilan_bosta_dakika(),
            accounts: vec![Account {
                name: "ercan-gemini-1".into(),
                provider: None,
                providers: vec![varsayilan_saglayici()],
            }],
            pacing: Pacing::default(),
            kota: Kota::default(),
            providers: Default::default(),
            rest: Default::default(),
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
        assert_eq!(c.accounts[0].saglayicilar(), vec!["gemini".to_string()]);
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

    #[test]
    fn gecerli_rest_yapilandirmasi_okunur() {
        let t = r#"
            [rest.deepseek]
            bicim = "openai"
            taban_url = "https://api.deepseek.com"
            anahtar_env = "DEEPSEEK_API_KEY"
            model = "deepseek-chat"
        "#;
        let c: Config = toml::from_str(t).unwrap();
        assert_eq!(c.rest.len(), 1);
        let ds = c.rest.get("deepseek").unwrap();
        assert_eq!(ds.bicim, "openai");
        assert_eq!(ds.taban_url, "https://api.deepseek.com");
        assert_eq!(ds.anahtar_env, "DEEPSEEK_API_KEY");
        assert_eq!(ds.model, "deepseek-chat");
        assert!(c.dogrula().is_ok());
    }

    #[test]
    fn gecersiz_rest_bicimi_hata_verir() {
        let t = r#"
            [rest.gecersiz]
            bicim = "bilinmeyen_bicim"
            taban_url = "https://api.example.com"
            anahtar_env = "API_KEY"
            model = "model-1"
        "#;
        let c: Config = toml::from_str(t).unwrap();
        assert!(c.dogrula().is_err());
    }
}
