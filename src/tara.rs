//! Profil taramasi: her profilde gercekte hangi hesap ve hangi saglayicilar acik?
//!
//! NEDEN: profil adi kullanicinin NIYETINDEN turuyordu, gercekten giren hesaptan
//! degil. Elle giris sirasinda siralar karisti; ayrica yarim kalan kurulumlar
//! oturumsuz profiller birakti. Bunlari tahminle ayiklamak yanlis hesabi silmek
//! demek olur — tarama olcerek soyluyor.
//!
//! ISTEM HARCAMAZ: yalnizca sayfa acilip oturum durumu okunuyor, hicbir saglayiciya
//! soru sorulmuyor. Kota dolu oldugunda da guvenle calisir.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;

use crate::browser::{detect_login, google_hesabi, BrowserBinary, LoginState, Provider, Session};

pub struct ProfilRaporu {
    pub profil: String,
    /// Sayfadan OKUNAN hesap. None = Google oturumu yok ya da okunamadi.
    pub kimlik: Option<String>,
    pub saglayicilar: Vec<(String, LoginState)>,
    pub hata: Option<String>,
}

impl ProfilRaporu {
    /// Hicbir saglayicida oturum yoksa profil oludur.
    pub fn olu_mu(&self) -> bool {
        self.hata.is_none()
            && !self.saglayicilar.iter().any(|(_, d)| *d == LoginState::Ready)
    }

    pub fn acik_saglayicilar(&self) -> Vec<String> {
        self.saglayicilar
            .iter()
            .filter(|(_, d)| *d == LoginState::Ready)
            .map(|(a, _)| a.clone())
            .collect()
    }
}

pub async fn tara(
    profiller: &[String],
    defter: &HashMap<String, Arc<Provider>>,
    profiles_root: &Path,
    browser: &BrowserBinary,
    geckodriver: &Path,
) -> Vec<ProfilRaporu> {
    let mut adlar: Vec<&String> = defter.keys().collect();
    adlar.sort();

    let mut raporlar = Vec::new();
    for (i, profil) in profiller.iter().enumerate() {
        println!("[{}/{}] {profil}", i + 1, profiller.len());
        match tek_profil(profil, &adlar, defter, profiles_root, browser, geckodriver).await {
            Ok(r) => raporlar.push(r),
            Err(e) => {
                println!("   HATA: {e:#}");
                raporlar.push(ProfilRaporu {
                    profil: profil.clone(),
                    kimlik: None,
                    saglayicilar: Vec::new(),
                    hata: Some(format!("{e:#}")),
                });
            }
        }
    }
    raporlar
}

async fn tek_profil(
    profil: &str,
    adlar: &[&String],
    defter: &HashMap<String, Arc<Provider>>,
    profiles_root: &Path,
    browser: &BrowserBinary,
    geckodriver: &Path,
) -> Result<ProfilRaporu> {
    // Profil basina TEK tarayici; saglayicilar sekmelerde. Her saglayici icin ayri
    // tarayici acmak 17 profilde onlarca gereksiz baslatma olurdu.
    let oturum = Session::open(profiles_root, profil, browser, geckodriver, true).await?;
    let mut kimlik = None;
    let mut durumlar = Vec::new();

    for (n, ad) in adlar.iter().enumerate() {
        let sag = &defter[*ad];
        if n == 0 {
            oturum.goto(&sag.url).await.ok();
        } else {
            oturum.yeni_sekme(&sag.url).await.ok();
        }
        let durum = detect_login(&oturum.client, &sag.selectors)
            .await
            .unwrap_or(LoginState::Unknown);
        // Google hesabi hangi sayfada gorunurse oradan okunur; kimlik profile ait,
        // saglayiciya degil.
        if kimlik.is_none() {
            kimlik = google_hesabi(&oturum.client).await;
        }
        println!("   {ad:<12} {durum:?}");
        durumlar.push(((*ad).clone(), durum));
    }

    oturum.close().await.ok();
    Ok(ProfilRaporu { profil: profil.to_string(), kimlik, saglayicilar: durumlar, hata: None })
}

pub fn rapor_bas(raporlar: &[ProfilRaporu]) {
    println!("\n=== TARAMA SONUCU ===");
    println!("{:<34} {:<30} acik saglayicilar", "profil", "gercekte hangi hesap");
    println!("{}", "-".repeat(90));
    for r in raporlar {
        let kimlik = r.kimlik.clone().unwrap_or_else(|| "?".into());
        let acik = r.acik_saglayicilar();
        let durum = match (&r.hata, acik.is_empty()) {
            (Some(e), _) => format!("HATA: {}", e.chars().take(40).collect::<String>()),
            (None, true) => "— (olu)".to_string(),
            (None, false) => acik.join(", "),
        };
        println!("{:<34} {:<30} {durum}", r.profil, kimlik);
    }

    let olu: Vec<&ProfilRaporu> = raporlar.iter().filter(|r| r.olu_mu()).collect();
    if !olu.is_empty() {
        println!("\nOLU PROFILLER ({}) — hicbir saglayicida oturum yok:", olu.len());
        for r in &olu {
            println!("  {}", r.profil);
        }
        println!("\nSilmeden once bu listeyi gozden gecir; silme islemi geri alinamaz.");
    }

    println!("\n=== kervan.toml icin onerilen hesap listesi ===");
    for r in raporlar {
        let acik = r.acik_saglayicilar();
        if acik.is_empty() {
            continue;
        }
        if let Some(k) = &r.kimlik {
            println!("\n# {k}");
        }
        println!("[[accounts]]");
        println!("name = \"{}\"", r.profil);
        println!("providers = [{}]", acik.iter().map(|a| format!("\"{a}\"")).collect::<Vec<_>>().join(", "));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rapor(durumlar: &[(&str, LoginState)], hata: Option<&str>) -> ProfilRaporu {
        ProfilRaporu {
            profil: "p".into(),
            kimlik: None,
            saglayicilar: durumlar.iter().map(|(a, d)| ((*a).to_string(), *d)).collect(),
            hata: hata.map(String::from),
        }
    }

    /// Bir tek saglayicida bile oturum varsa profil OLU DEGILDIR. Bu karar dogrudan
    /// silmeye baglaniyor; yanlis pozitif, calisan bir hesabin cerezlerini silmek olur.
    #[test]
    fn tek_acik_saglayici_profili_yasatir() {
        let r = rapor(
            &[
                ("claude", LoginState::NeedsLogin),
                ("gemini", LoginState::Ready),
                ("perplexity", LoginState::NeedsLogin),
            ],
            None,
        );
        assert!(!r.olu_mu());
        assert_eq!(r.acik_saglayicilar(), vec!["gemini".to_string()]);
    }

    /// HATA ALAN profil olu SAYILMAZ. Okuyamadigimiz bir profili "bos" varsayip
    /// silmek, olcmedigimiz seyi olculmus gibi davranmaktir — kilitli bir profil ya
    /// da acilmayan bir tarayici bunu tetikleyebilir.
    #[test]
    fn okunamayan_profil_olu_sayilmaz() {
        let r = rapor(&[], Some("profil kilitli"));
        assert!(!r.olu_mu(), "hatali tarama silme gerekcesi olamaz");
    }

    /// Hicbir saglayicida oturum yoksa ve hata da yoksa profil gercekten olu.
    #[test]
    fn hicbir_oturumu_olmayan_profil_olu() {
        let r = rapor(
            &[("claude", LoginState::NeedsLogin), ("gemini", LoginState::Unknown)],
            None,
        );
        assert!(r.olu_mu());
        assert!(r.acik_saglayicilar().is_empty());
    }
}
