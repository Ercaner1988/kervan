//! Toplu hesap kurulumu: eposta listesini alip sirayla giris ekranlarina goturur.
//!
//! NEDEN: tek tek kurulumda her hesap icin komut yazmak, profil adi uydurmak, giris
//! yapmak, dogrulamak ve `kervan.toml`'a elle satir eklemek gerekiyordu. Hesap sayisi
//! 7/24 hedefi icin buyudukce bu akis katlaniyor.
//!
//! NE OTOMATIKLESIR, NE OTOMATIKLESMEZ: profil adi, giris adresi (eposta ON-DOLDURULMUS),
//! dogrulama ve yapilandirma satiri otomatik. Sifre ve iki adimli dogrulama insanda
//! kalir — Kervan sifre saklamaz, yazmaz, gormez. Bu bir eksiklik degil karar: giris
//! anini insan denetiminden gecirmek plandaki insan kapisinin ta kendisi.

use std::path::Path;

use anyhow::{Context, Result};

use crate::browser::{detect_login, BrowserBinary, LoginState, Provider, Session};
use crate::config::Config;

/// Epostadan dosya sistemi icin guvenli, benzersiz profil adi uretir.
///
/// Alfanumerik disindaki HER SEY tireye cevrilir. Bu yalniz estetik degil guvenlik:
/// eposta disaridan gelen bir girdi ve dogrudan dizin yoluna giriyor; `../` ya da
/// ters bolu iceren bir deger profil kokunun disina yazdirabilirdi.
pub fn profil_adi(saglayici: &str, eposta: &str) -> String {
    let temiz: String = eposta
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    format!("{saglayici}-{temiz}")
}

pub struct Sonuc {
    pub eposta: String,
    pub profil: String,
    pub durum: Durum,
}

pub enum Durum {
    /// Zaten girisliydi, dokunulmadi.
    Vardi,
    Kuruldu,
    Basarisiz(String),
    Atlandi,
}

/// Listedeki her eposta icin sirayla: profil ac, giris ekranina goturl, dogrula.
pub async fn calistir(
    epostalar: &[String],
    saglayici_adi: &str,
    saglayici: &Provider,
    profiles_root: &Path,
    browser: &BrowserBinary,
    geckodriver: &Path,
    config_yolu: &Path,
) -> Result<Vec<Sonuc>> {
    let mevcut: Vec<String> = Config::load(config_yolu)
        .map(|c| c.accounts.into_iter().map(|a| a.name).collect())
        .unwrap_or_default();

    let mut sonuclar = Vec::new();
    for (i, eposta) in epostalar.iter().enumerate() {
        let profil = profil_adi(saglayici_adi, eposta);
        println!("\n=== {}/{}  {eposta}", i + 1, epostalar.len());
        println!("profil       : {profil}");

        // Zaten girisli mi? Tekrar calistirilabilir olmasi icin sart: yarim kalmis
        // bir kurulum bastan baslatilabilsin, tamamlanmis olan tekrar sorulmasin.
        if profiles_root.join(&profil).exists() {
            match girisli_mi(profiles_root, &profil, browser, geckodriver, saglayici).await {
                Ok(true) => {
                    println!("durum        : ZATEN GIRISLI, atlaniyor");
                    sonuclar.push(Sonuc { eposta: eposta.clone(), profil, durum: Durum::Vardi });
                    continue;
                }
                Ok(false) => println!("durum        : profil var ama giris yok, devam ediliyor"),
                // Kilitli profil burada normal: gecit calisiyor olabilir.
                Err(e) => {
                    println!("durum        : denetlenemedi ({e:#})");
                    sonuclar.push(Sonuc {
                        eposta: eposta.clone(),
                        profil,
                        durum: Durum::Basarisiz(format!("{e:#}")),
                    });
                    continue;
                }
            }
        }

        let url = saglayici.giris_url(eposta);
        println!("giris adresi : eposta on-doldurulmus");
        if let Err(e) = crate::browser::elle_giris(profiles_root, &profil, browser, &url) {
            println!("HATA         : {e:#}");
            sonuclar.push(Sonuc {
                eposta: eposta.clone(),
                profil,
                durum: Durum::Basarisiz(format!("{e:#}")),
            });
            continue;
        }

        println!("  -> Sifreyi gir (eposta zaten dolu), giris bitince tarayiciyi KAPAT.");
        println!("  -> Sonra Enter'a bas. Bu hesabi atlamak icin 'a' yazip Enter'a bas.");
        let mut satir = String::new();
        std::io::stdin().read_line(&mut satir).ok();
        if satir.trim().eq_ignore_ascii_case("a") {
            println!("atlandi.");
            sonuclar.push(Sonuc { eposta: eposta.clone(), profil, durum: Durum::Atlandi });
            continue;
        }

        // Kapatilan tarayici kilidi hemen birakmiyor; beklemeyi kullaniciya yaptirma.
        crate::browser::kilidin_birakilmasini_bekle(
            &profiles_root.join(&profil),
            std::time::Duration::from_secs(20),
        )
        .await;

        match girisli_mi(profiles_root, &profil, browser, geckodriver, saglayici).await {
            Ok(true) => {
                println!("durum        : GIRISLI");
                sonuclar.push(Sonuc { eposta: eposta.clone(), profil, durum: Durum::Kuruldu });
            }
            Ok(false) => {
                println!("durum        : giris dogrulanamadi");
                sonuclar.push(Sonuc {
                    eposta: eposta.clone(),
                    profil,
                    durum: Durum::Basarisiz("giris dogrulanamadi".into()),
                });
            }
            Err(e) => {
                println!("HATA         : {e:#}");
                sonuclar.push(Sonuc {
                    eposta: eposta.clone(),
                    profil,
                    durum: Durum::Basarisiz(format!("{e:#}")),
                });
            }
        }
    }

    yapilandirmaya_ekle(config_yolu, saglayici_adi, &sonuclar, &mevcut)?;
    Ok(sonuclar)
}

async fn girisli_mi(
    profiles_root: &Path,
    profil: &str,
    browser: &BrowserBinary,
    geckodriver: &Path,
    saglayici: &Provider,
) -> Result<bool> {
    let oturum = Session::open(profiles_root, profil, browser, geckodriver, true).await?;
    oturum.goto(&saglayici.url).await?;
    let durum = detect_login(&oturum.client, &saglayici.selectors).await?;
    oturum.close().await.ok();
    Ok(durum == LoginState::Ready)
}

/// Kurulan hesaplari `kervan.toml`'un SONUNA ekler.
///
/// Dosya ayristirilip yeniden yazilmiyor: oyle yapmak kullanicinin yorumlarini ve
/// duzenini silerdi. Sona eklemek TOML acisindan gecerli ve dosyaya dokunmayan en
/// kucuk degisiklik.
fn yapilandirmaya_ekle(
    yol: &Path,
    saglayici: &str,
    sonuclar: &[Sonuc],
    mevcut: &[String],
) -> Result<()> {
    let yeniler: Vec<&Sonuc> = sonuclar
        .iter()
        .filter(|s| matches!(s.durum, Durum::Kuruldu))
        .filter(|s| !mevcut.contains(&s.profil))
        .collect();
    if yeniler.is_empty() {
        return Ok(());
    }

    let mut metin = String::from("\n# Kervan --kur tarafindan eklendi\n");
    for s in &yeniler {
        metin.push_str(&format!(
            "\n[[accounts]]\nname = \"{}\"\nprovider = \"{saglayici}\"\n",
            s.profil
        ));
    }

    use std::io::Write;
    let mut d = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(yol)
        .with_context(|| format!("yapilandirma acilamadi: {}", yol.display()))?;
    d.write_all(metin.as_bytes())?;
    println!("\n{} hesap {} dosyasina eklendi.", yeniler.len(), yol.display());
    Ok(())
}

pub fn ozet_bas(sonuclar: &[Sonuc]) {
    println!("\n=== ozet ===");
    for s in sonuclar {
        let d = match &s.durum {
            Durum::Vardi => "zaten vardi".to_string(),
            Durum::Kuruldu => "kuruldu".to_string(),
            Durum::Atlandi => "atlandi".to_string(),
            Durum::Basarisiz(e) => format!("BASARISIZ — {e}"),
        };
        println!("  {:<40} {d}", s.eposta);
    }
}

/// Epostalari terminalden okur: her satira bir tane, bos satir bitirir.
pub fn epostalari_oku() -> Result<Vec<String>> {
    println!("Epostalari alt alta yaz, bitince bos satir birak:");
    let mut liste = Vec::new();
    loop {
        let mut satir = String::new();
        if std::io::stdin().read_line(&mut satir)? == 0 {
            break;
        }
        let s = satir.trim();
        if s.is_empty() {
            break;
        }
        // Ayirt edici bir denetim degil, yazim hatasi filtresi: '@' icermeyen bir
        // satir neredeyse kesin yanlis yapistirmadir ve sessizce bos profil acardi.
        if !s.contains('@') {
            println!("  '{s}' eposta gibi gorunmuyor, atlandi");
            continue;
        }
        if liste.iter().any(|v: &String| v == s) {
            println!("  '{s}' zaten listede, atlandi");
            continue;
        }
        liste.push(s.to_string());
    }
    Ok(liste)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Eposta disaridan gelir ve dogrudan dizin yoluna girer: yol ayiraci ya da
    /// `..` gecirmesi profil kokunun disina yazmak demek olurdu.
    #[test]
    fn profil_adi_yol_kacisina_izin_vermez() {
        for kotu in ["../../etc/passwd", "a\\..\\b@x.com", "a/b@x.com"] {
            let ad = profil_adi("gemini", kotu);
            assert!(!ad.contains('/'), "{ad}");
            assert!(!ad.contains('\\'), "{ad}");
            assert!(!ad.contains(".."), "{ad}");
        }
    }

    /// Ayri epostalar ayri profil almali, ayni eposta hep ayni profili.
    #[test]
    fn profil_adi_kararli_ve_benzersiz() {
        let a = profil_adi("gemini", "ercan@gmail.com");
        let b = profil_adi("gemini", "ercan@outlook.com");
        assert_ne!(a, b, "farkli alan adlari ayni profile dusmemeli");
        assert_eq!(a, profil_adi("gemini", "ERCAN@gmail.com"), "buyuk/kucuk fark etmemeli");
    }

    /// Saglayici adi ada girmeli: ayni eposta farkli saglayicilarda ayri oturumdur.
    #[test]
    fn saglayici_profil_adina_girer() {
        assert_ne!(profil_adi("gemini", "a@b.com"), profil_adi("claude", "a@b.com"));
    }
}
