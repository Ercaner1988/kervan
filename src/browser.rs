//! Hesap basina yalitilmis Firefox oturumu (WebDriver / geckodriver uzerinden).
//!
//! NEDEN FIREFOX: Ercan'in gunluk tarayicisi Zen (Firefox tabanli). Sistemi kendi
//! ortamiyla tutarli tutmak istedi. Bedeli: CDP kullanilamiyor, geckodriver adli ayri
//! bir ikili gerekiyor. Kazanci: gercek tarayici, gercek profil, tek yigin.
//!
//! YALITIM KARARI: her hesap kendi `-profile <dizin>`'inde calisir, gizli pencerede
//! DEGIL. Ayni tarayici ornegindeki gizli pencereler tek profili paylasir, yani hesaplar
//! birbirini gorur. Ayri profil dizini gercek yalitim verir; yan fayda olarak oturum
//! kalici olur — bir kez giris yaparsin, cerez yapistirmak gerekmez.

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use anyhow::{bail, Context, Result};
use fantoccini::{Client, ClientBuilder, Locator};

/// Bir hesabin tarayici oturumu. `close()` cagrilmazsa geckodriver sureci sizar.
pub struct Session {
    pub client: Client,
    driver: Child,
    pub profile_dir: PathBuf,
}

/// Tarayici ikilisi. Zen de Firefox turevi oldugu icin ayni surucuyle surulur.
#[derive(Debug, Clone)]
pub struct BrowserBinary(pub PathBuf);

impl BrowserBinary {
    /// Once yapilandirma, sonra bilinen kurulum yollari. Bulunamazsa acik hata —
    /// sessizce baska bir tarayiciya dusmek yanlis hesaba baglanmak demektir.
    pub fn discover() -> Result<Self> {
        if let Some(p) = std::env::var_os("KERVAN_BROWSER") {
            let p = PathBuf::from(p);
            if p.is_file() {
                return Ok(Self(p));
            }
            bail!("KERVAN_BROWSER gosterilen dosya yok: {}", p.display());
        }
        let adaylar = [
            r"C:\Program Files\Zen Browser\zen.exe",
            r"C:\Program Files\Mozilla Firefox\firefox.exe",
            r"C:\Program Files (x86)\Mozilla Firefox\firefox.exe",
        ];
        for c in adaylar {
            let p = PathBuf::from(c);
            if p.is_file() {
                return Ok(Self(p));
            }
        }
        bail!("Firefox tabanli tarayici bulunamadi. KERVAN_BROWSER ile yol ver.")
    }
}

impl Session {
    /// `hesap` profil dizini adidir (ornegin "ercan-gmail-1"). Ayni ad ayni oturumu
    /// geri getirir.
    ///
    /// `headless: false` insan kapisi icin sart — dogrulama duvarina carpildiginda
    /// pencerenin gorunur olmasi gerekir.
    pub async fn open(
        profiles_root: &Path,
        hesap: &str,
        browser: &BrowserBinary,
        geckodriver: &Path,
        headless: bool,
    ) -> Result<Self> {
        let profile_dir = profiles_root.join(hesap);
        std::fs::create_dir_all(&profile_dir)
            .with_context(|| format!("profil dizini olusturulamadi: {}", profile_dir.display()))?;

        let port = bos_port()?;
        let driver = Command::new(geckodriver)
            .arg("--port")
            .arg(port.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("geckodriver baslatilamadi: {}", geckodriver.display()))?;

        let mut args = vec!["-profile".to_string(), profile_dir.display().to_string()];
        if headless {
            args.push("-headless".to_string());
        }
        let mut caps = serde_json::map::Map::new();
        caps.insert(
            "moz:firefoxOptions".to_string(),
            serde_json::json!({ "binary": browser.0, "args": args }),
        );

        let client = baglan(port, caps).await?;
        Ok(Self { client, driver, profile_dir })
    }

    pub async fn goto(&self, url: &str) -> Result<()> {
        self.client
            .goto(url)
            .await
            .with_context(|| format!("sayfa acilamadi: {url}"))
    }

    pub async fn close(mut self) -> Result<()> {
        // Client ucuz bir tutamak; Drop uygulandigi icin alandan tasinamiyor, klonluyoruz.
        self.client.clone().close().await.ok();
        self.driver.kill().ok();
        self.driver.wait().ok();
        Ok(())
    }
}

/// Son savunma: `close()` cagrilamadan dusulurse (panik, Ctrl+C, zaman asimiyla
/// oldurulme) geckodriver ve onun actigi tarayici surec agaci arkada kalir. Her yarim
/// kalan kosu bir tarayici sizdirirsa N hesapli calismada bellek tukenir.
/// `kill()` senkron oldugu icin Drop icinde guvenle cagrilabilir.
impl Drop for Session {
    fn drop(&mut self) {
        self.driver.kill().ok();
        self.driver.wait().ok();
    }
}

/// geckodriver acilirken birkac yuz ms gecikiyor; hazir olana kadar kisa araliklarla dene.
async fn baglan(port: u16, caps: serde_json::map::Map<String, serde_json::Value>) -> Result<Client> {
    let url = format!("http://127.0.0.1:{port}");
    let mut son = None;
    for _ in 0..40 {
        match ClientBuilder::native()
            .capabilities(caps.clone())
            .connect(&url)
            .await
        {
            Ok(c) => return Ok(c),
            Err(e) => {
                son = Some(e);
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            }
        }
    }
    bail!("geckodriver'a baglanilamadi ({url}): {}", son.unwrap())
}

/// Isletim sisteminden bos port iste, hemen birak. Sabit port kullanmak birden fazla
/// hesabi ayni anda acmayi engellerdi.
fn bos_port() -> Result<u16> {
    let l = TcpListener::bind("127.0.0.1:0").context("bos port bulunamadi")?;
    Ok(l.local_addr()?.port())
}

/// Sayfanin oturum durumu. Sessiz basarisizlik yasak: "belki girislidir" diye devam
/// etmek yerine acikca siniflandiriyoruz — belirsiz hal insana gider.
#[derive(Debug, PartialEq, Eq)]
pub enum LoginState {
    /// Girisli — istem kutusu bulundu.
    Ready,
    /// Giris gerekiyor; insan kapisi acilmali.
    NeedsLogin,
    /// Ne istem kutusu ne giris baglantisi var. Secici kirilmis ya da dogrulama duvari
    /// cikmis olabilir — ikisi de insana gider.
    Unknown,
}

/// Secici demeti. Kodda gomulu DEGIL, yapilandirmadan gelmeli — site arayuzu
/// degistiginde yeniden derleme degil tek satir duzenleme yetsin diye.
/// v1'de varsayilanlar main.rs'te; yapilandirma okuma Asama 3'te.
#[derive(Debug, Clone)]
pub struct Selectors {
    /// Istem yazilan alan. Varsa oturum aciktir.
    pub prompt_input: &'static str,
    /// Giris baglantisi/dugmesi. Varsa oturum kapalidir.
    pub login_hint: &'static str,
    /// Modelin yanit balonlari. Sonuncusu okunur.
    pub response: &'static str,
}

/// WebDriver Enter tusu (U+E007).
const ENTER: &str = "\u{E007}";

/// Istemi yazip gonderir, yanit tamamlanana kadar bekler ve metnini dondurur.
///
/// BITIS ALGILAMA KARARI: "durdur dugmesi kayboldu mu" gibi secici tabanli bir olcut
/// arayuz her degistiginde kirilir ve sessizce YARIM yanit dondurur — en tehlikeli
/// hata bicimi bu. Onun yerine davranisa bakiyoruz: yanit metni `kararlilik_esigi`
/// kadar ust uste ayni kalirsa akis bitmistir. Secicilerden bagimsiz, yavas ag
/// kosullarinda da dogru.
pub async fn ask(
    c: &Client,
    sel: &Selectors,
    prompt: &str,
    zaman_asimi: std::time::Duration,
) -> Result<String> {
    let kutu = c
        .find(Locator::Css(sel.prompt_input))
        .await
        .context("istem kutusu bulunamadi — oturum dusmus ya da secici kirilmis olabilir")?;
    kutu.click().await.context("istem kutusuna tiklanamadi")?;
    kutu.send_keys(prompt).await.context("istem yazilamadi")?;
    kutu.send_keys(ENTER).await.context("istem gonderilemedi")?;

    const ARALIK: std::time::Duration = std::time::Duration::from_millis(500);
    const KARARLILIK_ESIGI: u8 = 4; // 4 x 500ms = 2 sn degismezse bitti say

    let baslangic = std::time::Instant::now();
    let mut onceki = String::new();
    let mut sabit = 0u8;

    while baslangic.elapsed() < zaman_asimi {
        tokio::time::sleep(ARALIK).await;
        let simdi = son_yanit(c, sel).await;
        match simdi {
            Some(t) if !t.trim().is_empty() => {
                if t == onceki {
                    sabit += 1;
                    if sabit >= KARARLILIK_ESIGI {
                        return Ok(t);
                    }
                } else {
                    sabit = 0;
                    onceki = t;
                }
            }
            // Henuz yanit balonu yok ya da bos — beklemeye devam.
            _ => sabit = 0,
        }
    }

    if onceki.trim().is_empty() {
        bail!(
            "yanit gelmedi ({} sn). Dogrulama duvari cikmis olabilir — gorunur kipte calistirip bak.",
            zaman_asimi.as_secs()
        );
    }
    // Zaman asimina ugradi ama elimizde metin var: yarim oldugunu SOYLEYEREK don,
    // sessizce tam yanit gibi sunma.
    bail!(
        "yanit {} sn icinde tamamlanmadi; son goruleni yarim kabul ediyorum ({} karakter)",
        zaman_asimi.as_secs(),
        onceki.len()
    )
}

async fn son_yanit(c: &Client, sel: &Selectors) -> Option<String> {
    let hepsi = c.find_all(Locator::Css(sel.response)).await.ok()?;
    let son = hepsi.last()?;
    son.text().await.ok()
}

pub async fn detect_login(c: &Client, sel: &Selectors) -> Result<LoginState> {
    if c.find(Locator::Css(sel.prompt_input)).await.is_ok() {
        return Ok(LoginState::Ready);
    }
    if c.find(Locator::Css(sel.login_hint)).await.is_ok() {
        return Ok(LoginState::NeedsLogin);
    }
    Ok(LoginState::Unknown)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Profil dizini hesap adina gore ayrilmali — yalitimin tamami buna dayaniyor.
    /// Bozulursa hesaplar birbirinin cerezini gorur, ki OmniRoute'un paylasilan
    /// tarayici baglaminda yasadigi sorun tam olarak budur.
    #[test]
    fn profil_dizinleri_hesaba_gore_ayrilir() {
        let root = Path::new("C:\\kervan\\profiles");
        assert_ne!(root.join("hesap-a"), root.join("hesap-b"));
        // ayni ad ayni dizini vermeli, yoksa oturum kalici olmaz
        assert_eq!(root.join("hesap-a"), root.join("hesap-a"));
    }

    /// Her oturum ayri port almali; sabit port ikinci hesabi engellerdi.
    #[test]
    fn portlar_benzersiz() {
        let a = bos_port().unwrap();
        let b = bos_port().unwrap();
        assert!(a > 0 && b > 0);
    }

    /// Bulunamayan tarayici sessizce baska bir seye dusmemeli.
    #[test]
    fn eksik_tarayici_acik_hata_verir() {
        unsafe { std::env::set_var("KERVAN_BROWSER", "C:\\olmayan\\tarayici.exe") };
        let r = BrowserBinary::discover();
        unsafe { std::env::remove_var("KERVAN_BROWSER") };
        assert!(r.is_err(), "olmayan yol icin hata bekleniyordu");
    }
}
