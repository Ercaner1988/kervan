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

use anyhow::{Context, Result, bail};
use fantoccini::{Client, ClientBuilder, Locator, elements::Element};

// Alt moduller: iskelet (bu dosya) + olcum araclari. Model/secim mantigi ayri
// sandikta (`kervan-secim`): bu dosya 1500 satir sinirinda ve secim farkli bir
// sorumluluk.
mod olcum;

#[cfg(test)]
pub(crate) use kervan_secim::metin_iceriyor_mu;
pub(crate) use kervan_secim::model_sec;
pub use kervan_secim::{
    Anahtar, AnahtarSonucu, ModelMenusu, ModelSecenegi, SecimSonucu, anahtar_ayarla,
};
pub use olcum::{
    js_calistir, metinle_tikla, metinle_uzerine_gel, oge_dokumu, oge_metni, tikla_ilk, uzerine_gel,
};

/// Gizli pencerenin konumu: gorunur masaustunun sagi/altinda kalan bos alan.
/// WebDriver `set_window_rect` isaretsiz sayi aliyor, yani negatif koordinat yok;
/// gizlemek icin buyuk POZITIF bir konum kullaniliyor.
const GIZLI_X: u32 = 20_000;
const GIZLI_Y: u32 = 20_000;

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
        if profil_kilitli(&profile_dir) {
            bail!(
                "'{hesap}' profili baska bir surecte acik. Genelde ya Kervan gecidi \
                 calisiyordur ya da onceki kosudan artan bir tarayici penceresi vardir. \
                 Onu kapat, sonra yeniden dene."
            );
        }

        let port = bos_port()?;
        let mut driver = Command::new(geckodriver)
            .arg("--port")
            .arg(port.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| {
                format!(
                    "{SURUCU_ARIZASI}: geckodriver baslatilamadi: {}",
                    geckodriver.display()
                )
            })?;

        // GIZLI KIP HEADLESS DEGIL. Headless bir tarayici calisirken gorunur hale
        // GETIRILEMEZ; "arkada dursun, istedigim an one cekeyim" ancak gercek bir
        // pencereyi kucultup geri yukleyerek olur. Yan fayda: claude.ai headless'ta
        // bot duvarina carpiyordu ("Performing security verification"), gercek
        // pencerede o duvar cikmiyor.
        let args = otomasyon_arglari(&profile_dir);
        let _ = headless;
        let caps = yetenekler(&browser.0, args);

        // Baglanti kurulamazsa geckodriver sureci SIZMASIN. Eskiden `?` ile
        // cikiliyordu ve surucu arkada kaliyordu (olculdu: 21:22'de baslamis iki
        // oksuz geckodriver). Sizinti, sonraki denemelerde port/durum karisikligina
        // yol aciyor.
        let client = match baglan(port, caps).await {
            Ok(c) => c,
            Err(e) => {
                if let Err(h) = driver.kill() {
                    eprintln!("uyari: geckodriver kapatilamadi: {h}");
                }
                if let Err(h) = driver.wait() {
                    eprintln!("uyari: geckodriver beklenemedi: {h}");
                }
                return Err(e);
            }
        };
        let s = Self {
            client,
            driver,
            profile_dir,
        };
        if headless {
            s.gizle().await;
        }
        Ok(s)
    }

    /// Pencereyi gozden cekilir: EKRAN DISINA tasinir, kucultulmez.
    ///
    /// NEDEN KUCULTME DEGIL: olculdu — kucultulmus pencerede tarayici cizimi kisiyor
    /// ve Gemini'nin yanit balonu DOM'a hic dusmuyordu (120 sn zaman asimi). Ayni
    /// istem gorunur pencerede 5.9 saniyede dogru yanit verdi. Ekran disindaki
    /// pencere ise normal cizilmeye devam ediyor, sadece gorunmuyor.
    pub async fn gizle(&self) {
        // Hata yutuluyor: gizlenemezse is durmaz, yalnizca pencere gorunur kalir.
        self.client
            .set_window_rect(GIZLI_X, GIZLI_Y, 1280, 900)
            .await
            .ok();
        // Isletim sistemi konumu masaustune KIRPABILIR. Kirptiysa pencere gorunur
        // kalir; bunu sessizce "gizledim" diye gecistirmek yanlis olur.
        if let Ok((x, y, _, _)) = self.client.get_window_rect().await
            && (x + 1200 < GIZLI_X as u64 || y + 800 < GIZLI_Y as u64)
        {
            eprintln!(
                "uyari: pencere ekran disina alinamadi (isletim sistemi {x},{y} konumuna \
                 kirpti); gorunur kalacak"
            );
        }
    }

    /// Pencereyi one getirir — model secimi, istemi durdurma, dogrulama cozme icin.
    /// Insan kapisinin gorunur ayagi bu.
    pub async fn goster(&self) -> Result<()> {
        self.client
            .set_window_rect(80, 60, 1280, 900)
            .await
            .context("pencere one getirilemedi")?;
        Ok(())
    }

    /// Yeni sekme acar ve ona gecer; dondurdugu tutamakla sonra geri donulur.
    ///
    /// NEDEN SEKME: bir profilde birden cok saglayici barinir (ayni Google oturumu
    /// hepsine yetiyor). Sekme yerine ayni sekmede adres degistirmek sohbeti
    /// sifirlardi — Gemini `/app`'e her donuste yeni konusma aciyor, yani konusma
    /// surekliligi sabitlemesi calismaz olurdu. Her saglayici kendi sekmesinde
    /// oturur, aralarinda gecis yapariz.
    pub async fn yeni_sekme(&self, url: &str) -> Result<fantoccini::wd::WindowHandle> {
        let y = self
            .client
            .new_window(true)
            .await
            .context("yeni sekme acilamadi")?;
        self.client
            .switch_to_window(y.handle.clone())
            .await
            .context("yeni sekmeye gecilemedi")?;
        self.goto(url).await?;
        Ok(y.handle)
    }

    pub async fn sekmeye_gec(&self, h: fantoccini::wd::WindowHandle) -> Result<()> {
        self.client
            .switch_to_window(h)
            .await
            .context("sekmeye gecilemedi")
    }

    pub async fn gecerli_sekme(&self) -> Result<fantoccini::wd::WindowHandle> {
        self.client
            .window()
            .await
            .context("gecerli sekme okunamadi")
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

/// Profil su an baska bir surecte acik mi?
///
/// Firefox profil dizininde `parent.lock` tutar ve calistigi surece acik birakir.
/// Dosyanin VARLIGI kanit degil — cokme sonrasi geride kalabiliyor. Yazmaya acmayi
/// denemek canliyi olulden ayiriyor; olculdu: tutan surec varken acilmiyor, o surec
/// olunce ayni dosya sorunsuz aciliyor.
/// Kilidin birakilmasini kisa bir sure bekler.
///
/// Tarayici penceresi kapandiktan SONRA da surec birkac saniye yasiyor ve kilidi
/// hemen birakmiyor. "Kapat, sonra Enter'a bas" akisinda kullanici tam da bu araliga
/// denk gelip "profil baska bir surecte acik" hatasi aliyordu — oysa yapmasi gereken
/// tek sey biraz beklemekti. Beklemeyi kullaniciya yaptirmak yerine biz yapiyoruz.
pub async fn kilidin_birakilmasini_bekle(dir: &Path, azami: std::time::Duration) {
    let baslangic = std::time::Instant::now();
    while profil_kilitli(dir) && baslangic.elapsed() < azami {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}

fn profil_kilitli(dir: &Path) -> bool {
    // Windows'ta parent.lock, Unix'te .parentlock.
    for ad in ["parent.lock", ".parentlock"] {
        let yol = dir.join(ad);
        if yol.exists() && std::fs::OpenOptions::new().write(true).open(&yol).is_err() {
            return true;
        }
    }
    false
}

/// Otomasyon yolunun tarayici argumanlari.
///
/// `-no-remote` SART (elle_giris ile ayni gerekce): onsuz Zen zaten calisan ornege
/// devredip acilir ve WebDriver'in bekledigi Marionette kapisi hic acilmaz — sonuc,
/// `baglan` sonsuza kadar bekler. Olculdu 2026-10-21: havuz istegi 240 sn'yi asdi,
/// ne hata ne kayit dusuldu.
///
/// NEDEN AYRI FONKSIYON: bu liste `Command`'a gomulu kaldigi surece sinanamiyordu,
/// yani `-no-remote` bir kez daha sessizce dusse hata yine "asili kalma" olarak
/// gorunurdu. Simdi ayni liste testte dogrudan okunuyor; iki baslatma yolu
/// (`otomasyon_arglari` / `elle_giris_arglari`) ayrisirsa test kirmizi verir.
pub fn otomasyon_arglari(profile_dir: &Path) -> Vec<String> {
    vec![
        "-no-remote".to_string(),
        "-profile".to_string(),
        profile_dir.display().to_string(),
    ]
}

/// Elle giris yolunun argumanlari: ayni `-no-remote` + `-profile`, arti hedef adres.
pub fn elle_giris_arglari(profile_dir: &Path, url: &str) -> Vec<String> {
    vec![
        "-no-remote".to_string(),
        "-profile".to_string(),
        profile_dir.display().to_string(),
        url.to_string(),
    ]
}

/// Hesabin profiliyle **otomasyon disinda**, sira bir tarayici penceresi acar.
///
/// NEDEN GEREKLI: Google, WebDriver ile surulen tarayicida oturum acmayi engelliyor
/// ("Bu tarayıcı veya uygulama güvenli olmayabilir"). Sayfayi KULLANMAYI degil,
/// yalnizca GIRISI engelliyor — zaten acilmis oturum otomasyonda sorunsuz calisiyor.
///
/// Bunu asmanin teknik yolu otomasyon imzasini gizlemek olurdu (marionette/webdriver
/// bayraklarini kapatmak). Kurmuyoruz: plandaki "yapilmayacaklar" listesinin ilk
/// maddesi bu. Onun yerine dogrusunu yapiyoruz — giris gercekten bir insan tarafindan,
/// gercekten sira bir tarayicida yapiliyor. Gizlenen bir sey yok.
///
/// `-no-remote` sart: onsuz Zen zaten calisan ornege devredip sekmeyi KULLANICININ
/// kendi profilinde acar, yani yanlis hesaba giris yapilir.
pub fn elle_giris(
    profiles_root: &Path,
    hesap: &str,
    browser: &BrowserBinary,
    url: &str,
) -> Result<Child> {
    let dir = profiles_root.join(hesap);
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("profil dizini olusturulamadi: {}", dir.display()))?;
    // Kilitliyken acmak tarayicinin "zaten calisiyor" modal penceresini cikarir ve
    // surec orada asili kalir. Acik hata vermek her zaman iyidir.
    if profil_kilitli(&dir) {
        bail!(
            "'{hesap}' profili baska bir surecte acik. Gecit calisiyorsa durdur \
             (ya da o tarayici penceresini kapat), sonra `--giris` ile yeniden dene."
        );
    }
    // BU YOLDA WEBDRIVER YOK, dolayisiyla yeteneklerle verdigimiz ayarlar da yok.
    // Ilk-acilis sihirbazini kapatan ayarlarin profile DOSYA olarak yazilmasi
    // gerekiyor; tam bu eksik oldugu icin hesap kurarken sihirbaz her seferinde
    // cikiyordu. geckodriver'in user.js'i ezmesi yalniz WebDriver yolunda olur,
    // burada yazdigimiz kalir.
    profil_tohumla(&dir)?;

    Command::new(&browser.0)
        .args(elle_giris_arglari(&dir, url))
        .spawn()
        .with_context(|| format!("tarayici baslatilamadi: {}", browser.0.display()))
}

/// Ilk-acilis ekranlarini kapatan ayarlari profile yazar.
///
/// Firefox `user.js`'i her acilista prefs.js'in uzerine uygular; tarayiciyi kendimiz
/// baslattigimizda ayarlari gecirmenin yolu bu. Var olani EZER: buradaki dosya ya
/// bizimdir ya da geckodriver'dan kalmadir, ikisi de korunmayi gerektirmez.
pub fn profil_tohumla(dir: &Path) -> Result<()> {
    let yol = dir.join("user.js");
    std::fs::write(&yol, VARSAYILAN_USER_JS)
        .with_context(|| format!("profil ayarlari yazilamadi: {}", yol.display()))
}

/// Ayni ayarlar iki yoldan da gitmeli: WebDriver yolunda yeteneklerle, elle giris
/// yolunda bu dosyayla. Tek kaynak olsun diye liste asagida bir kez duruyor ve
/// `yetenekler()` ile birlikte sinaniyor.
const VARSAYILAN_USER_JS: &str = r#"// Kervan olusturdu. Otomasyon profilinde insan mudahalesi bekleyen
// ilk-acilis ekranlarini kapatir. Silinirse yeniden yazilir.
user_pref("zen.welcome-screen.seen", true);
user_pref("browser.aboutwelcome.enabled", false);
user_pref("browser.startup.homepage_override.mstone", "ignore");
user_pref("startup.homepage_welcome_url", "about:blank");
user_pref("startup.homepage_welcome_url.additional", "");
user_pref("browser.laterrun.enabled", false);
user_pref("browser.messaging-system.whatsNewPanel.enabled", false);
user_pref("browser.uitour.enabled", false);
user_pref("browser.shell.checkDefaultBrowser", false);
user_pref("datareporting.policy.dataSubmissionPolicyBypassNotification", true);
user_pref("toolkit.telemetry.reportingpolicy.firstRun", false);
user_pref("browser.sessionstore.resume_from_crash", false);
"#;

/// WebDriver yetenekleri: tarayici ikilisi, profil bagimsizligi ve ilk-acilis
/// ekranlarini kapatan ayarlar.
///
/// SORUN: her hesap sifirdan bir profil oldugu icin tarayici her seferinde kurulum
/// karsilama akisini basa sariyor ve hesaba giris yapmaya sira gelmiyor.
///
/// Ayarlar tahminle degil OLCUMLE bulundu: kurulumu tamamlanmis bir profilin
/// prefs.js'i okundu, Zen'in kapisi `zen.welcome-screen.seen` cikti. Firefox'un kendi
/// ilk-acilis ayarlarini geckodriver zaten yaziyor, Zen'e ozgu olani bilmiyor.
///
/// PROFILE `user.js` YAZMAYI DENEMEK YANLIS YOL: geckodriver profile kendi user.js'ini
/// yaziyor ve bizimkini eziyor (olculdu — dosyada bizim satirlarimiz yerine
/// `app.update.disabledForTesting` gibi geckodriver ayarlari cikti). Dogrusu ayarlari
/// yetenekler uzerinden vermek; geckodriver onlari kendi dosyasiyla BIRLESTIRIYOR.
fn yetenekler(ikili: &Path, args: Vec<String>) -> serde_json::map::Map<String, serde_json::Value> {
    let mut caps = serde_json::map::Map::new();
    caps.insert(
        "moz:firefoxOptions".to_string(),
        serde_json::json!({
            "binary": ikili,
            "args": args,
            "prefs": {
                // Zen'in kendi karsilama akisi — giris yapmayi engelleyen asil ekran.
                "zen.welcome-screen.seen": true,

                // Firefox tarafindaki ilk-acilis ve tanitim ekranlari.
                "browser.aboutwelcome.enabled": false,
                "browser.startup.homepage_override.mstone": "ignore",
                "startup.homepage_welcome_url": "about:blank",
                "startup.homepage_welcome_url.additional": "",
                "browser.laterrun.enabled": false,
                "browser.messaging-system.whatsNewPanel.enabled": false,
                "browser.uitour.enabled": false,
                "browser.shell.checkDefaultBrowser": false,
                "datareporting.policy.dataSubmissionPolicyBypassNotification": true,
                "toolkit.telemetry.reportingpolicy.firstRun": false,

                // Kervan zorla oldurulunce (Drop calismaz) sonraki acilista "oturumu
                // geri yukle" penceresi cikiyor; o da elle tiklanmayi bekleyen engel.
                "browser.sessionstore.resume_from_crash": false,
            }
        }),
    );
    caps
}

/// geckodriver acilirken birkac yuz ms gecikiyor; hazir olana kadar kisa araliklarla dene.
async fn baglan(
    port: u16,
    caps: serde_json::map::Map<String, serde_json::Value>,
) -> Result<Client> {
    // SAYI DEGIL SURE ile sinirli. Eskiden `for _ in 0..40` idi ve "10 saniye"
    // saniliyordu (40 x 250 ms uyku). Oysa her denemenin KENDI baglanti zaman
    // asimi var; olculdu 2026-08-18 — cevap vermeyen bir surucude tek istek
    // 92 saniye surdu. Sure tavani, deneme sayisindan bagimsiz olarak dogruyu
    // soyler ve hata mesajindaki sure gercekten dogru olur.
    //
    // 2026-10-21 DUZELTMESI: yes, `connect` TEK BASINA sinirsiz bekleyebiliyordu.
    // Olculdu: havuz istegi 240 sn'yi asti, ne hata dondu ne kayit dusuldu; arkada
    // oksuz geckodriver kaldi. Artik her deneme `timeout` ile sarili; toplam sure
    // gercekten sinirli. Sure 15 sn'den 90 sn'ye cikti cunku soguk bir Zen acilisi
    // Marionette kapisini 20-40 sn'de aciyor — 15 sn'lik tavan gercek acilislari da
    // kesecekti. Ust sinir yine de kesin: 90 sn.
    const SURUCU_BEKLEME: std::time::Duration = std::time::Duration::from_secs(90);
    let url = format!("http://127.0.0.1:{port}");
    let mut son: Option<String> = None;
    let basladi = std::time::Instant::now();
    while basladi.elapsed() < SURUCU_BEKLEME {
        let kalan = SURUCU_BEKLEME.saturating_sub(basladi.elapsed());
        let deneme = tokio::time::timeout(
            kalan,
            ClientBuilder::native()
                .capabilities(caps.clone())
                .connect(&url),
        )
        .await;
        match deneme {
            Ok(Ok(c)) => return Ok(c),
            Ok(Err(e)) => {
                son = Some(e.to_string());
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            }
            Err(_) => {
                son = Some(format!(
                    "{} sn icinde oturum acilmadi (tarayici Marionette kapisini acmadi)",
                    SURUCU_BEKLEME.as_secs()
                ));
                break;
            }
        }
    }
    bail!(
        "{SURUCU_ARIZASI}: geckodriver'a baglanilamadi ({url}) — {} sn denendi. Bu ariza \
         ORTAMSALDIR, hesapla ilgisi yoktur: surucu ikilisi calisiyor mu, port kullanimda \
         mi, baska bir Kervan ornegi acik mi diye bak. Ayrinti: {}",
        SURUCU_BEKLEME.as_secs(),
        son.unwrap_or_else(|| "deneme yapilamadi".into())
    )
}

/// Isletim sisteminden bos port iste, hemen birak. Sabit port kullanmak birden fazla
/// hesabi ayni anda acmayi engellerdi.
fn bos_port() -> Result<u16> {
    let l = TcpListener::bind("127.0.0.1:0").context("bos port bulunamadi")?;
    Ok(l.local_addr()?.port())
}

/// Sayfanin oturum durumu. Sessiz basarisizlik yasak: "belki girislidir" diye devam
/// etmek yerine acikca siniflandiriyoruz — belirsiz hal insana gider.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
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
    /// Istem yazilan alan.
    pub prompt_input: String,
    /// Giris baglantisi/dugmesi. Varsa oturum kapalidir.
    pub login_hint: String,
    /// Modelin yanit balonlari. Sonuncusu okunur.
    pub response: String,
    /// Istemi gonderen dugme. `None` ise istem kutusuna ENTER basilir.
    ///
    /// AI Studio gibi ENTER'i satir atlama sayan siteler icin (olculdu
    /// 2026-10-23): ENTER yolu sessizce ise yaramaz, 120 sn sonra
    /// "yanit gelmedi" hatasi cikar. Verilirse dugme tiklanir.
    pub gonder: Option<String>,
}

/// Bir web saglayicisi: adresi ve secicileri hep birlikte gezer, birlikte tasinirlar.
/// Asama 4'te her yeni saglayici bunun bir ornegi olacak.
#[derive(Debug, Clone)]
pub struct Provider {
    /// `kervan.toml`'daki ad; model kimliginde de bu gecer (`kervan/claude`).
    pub ad: String,
    pub url: String,
    pub selectors: Selectors,
    /// Eposta ON-DOLDURULMUS giris adresi; `{eposta}` yerine hesap konur.
    /// Saglayici on-doldurmayi desteklemiyorsa duz giris adresi yazilir.
    pub giris_kalibi: String,
    /// Site Cloudflare bot korumasinin arkasinda mi? OLCULEN bir olgu, tercih degil.
    ///
    /// Tek etkisi bosta kapatmadir: bu saglayicinin sekmesi acikken tarayici
    /// kapatilmaz. Cunku her yeniden acilis Cloudflare icin TAZE bir oturum, yani
    /// yeni bir risk degerlendirmesi; ac-kapa dongusu sinamayi davet ediyor.
    pub cloudflare: bool,
    /// Model menusu: `kervan.toml`da OLCULMUS secicilerle tanimlanir.
    /// `None` ise sitede model secimi yapilamiyor; model kimligi istemek acik hatadir.
    pub model: Option<ModelMenusu>,
    /// Ac/kapa ozellikleri: `kervan.toml`da OLCULMUS dugme + ad ile tanimlanir.
    /// Bos ise sitede tanimli ac/kapa ozelligi yok; anahtar istemek acik hatadir.
    pub anahtarlar: Vec<Anahtar>,
}

impl Provider {
    pub fn giris_url(&self, eposta: &str) -> String {
        self.giris_kalibi.replace("{eposta}", &url_kodla(eposta))
    }
}

/// Sorgu dizesi icin en dar kodlama. Ozellikle `+`: Gmail takma adlarinda yaygin ve
/// kodlanmazsa sunucu tarafinda BOSLUGA donusur, yani yanlis hesap acilir.
fn url_kodla(s: &str) -> String {
    let mut o = String::new();
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'@' => {
                o.push(*b as char)
            }
            _ => o.push_str(&format!("%{b:02X}")),
        }
    }
    o
}

/// WebDriver Enter tusu (U+E007).
const ENTER: &str = "\u{E007}";

/// Insan kapisi hatalarinin ONEKI. Cagiran taraf bunu gorunce oturumu insanin
/// elinde sayar (bosta supurucu kapatmaz) ve kullaniciya gosterir.
///
/// Metin uzerinden tanima bilincli bir tercih: hata tipi eklemek pool ve server
/// imzalarina kadar yayilirdi, oysa tek ihtiyac "bu hata insan mudahalesi
/// istiyor mu" sorusu.
pub const INSAN_KAPISI: &str = "INSAN KAPISI";

/// SURUCU ARIZASI onegi: geckodriver baslatilamadi ya da baglanti kurulamadi.
///
/// NEDEN AYRI SINIF: bu ariza ORTAMSALDIR — her hesapta aynidir. Havuz bunu
/// hesaba ozel sanip sirayla butun hesaplari deniyordu; her denemede hesap geri
/// cekilmeye sokuluyor ve zaman yaniyordu. Olculdu 2026-08-18: tek istek 300
/// saniyeye kadar surdu ve dort hesap bosuna cezalandirildi, uzerine hata
/// "yanit gelmedi" gibi gorunup yanlis yere baktirdi.
pub const SURUCU_ARIZASI: &str = "SURUCU ARIZASI";

/// Sayfanin ustunu kapatan dogrulama katmanlari.
///
/// OLCULDU (z.ai, 2026-08-18): istem kutusuna tiklama
/// `<div id="aliyunCaptcha-mask" class="mask-show"> obscures it` diye reddedildi.
/// Yani z.ai'da karsimiza cikan sey CLOUDFLARE DEGIL, ALIYUN dogrulamasi — z.ai
/// Cin merkezli ve Aliyun'un katmanini kullaniyor. Bugun "cloudflare'e takildi"
/// dedigimiz z.ai olaylarinin kaynagi buydu.
///
/// Belirti METIN degil KATMAN oldugu icin `dogrulama_duvari_mi` (cevap balonundaki
/// metni denetler) bunu yakalayamaz; ayri bir olcume ihtiyac var.
const DOGRULAMA_KATMANLARI: [&str; 5] = [
    "#aliyunCaptcha-mask.mask-show",
    "iframe[src*='captcha']",
    "iframe[title*='challenge']",
    "#challenge-running",
    "#cf-challenge-running",
];

/// Acik bir dogrulama katmani varsa hangisi oldugunu dondurur.
///
/// `is_displayed` sart: bu katmanlar DOM'da gizli halde de duruyor olabilir, sadece
/// varliklarina bakmak her istegi sahte insan kapisina dusururdu.
async fn dogrulama_katmani(c: &Client) -> Option<String> {
    for s in DOGRULAMA_KATMANLARI {
        if let Ok(e) = c.find(Locator::Css(s)).await
            && e.is_displayed().await.unwrap_or(false)
        {
            return Some(s.to_string());
        }
    }
    None
}

/// Pencereyi one getirir. Insan kapisinin gorunur ayagi; `Session::goster` ile ayni
/// isi yapiyor ama `ask` elinde yalnizca `Client` tutuyor.
async fn pencereyi_one_getir(c: &Client) {
    c.set_window_rect(80, 60, 1280, 900).await.ok();
}

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
    model: Option<(&ModelMenusu, &str)>,
    anahtarlar: &[(Anahtar, bool)],
) -> Result<String> {
    // Model secimi ISTEM YAZILMADAN ONCE yapilir: sayfa model degistirirken istemi
    // yutabiliyor (olculdu: gemini menusu acikken yazilan istem kutuya dusmedi).
    // Secim uygulanamazsa `model_sec` gurultulu hata verir ve istek hic gitmez —
    // sessizce varsayilan modelle cevap vermek yok.
    if let Some((menu, etiket)) = model {
        let sonuc = model_sec(
            c,
            menu,
            etiket,
            zaman_asimi.min(std::time::Duration::from_secs(60)),
        )
        .await?;
        println!("model        : {etiket} ({sonuc:?})");
    }

    // Ac/kapa ozellikleri de istemden ONCE uygulanir: ayni gerekce (sayfa yeniden
    // cizilirken istem yutulabiliyor) ve ayni kural — uygulanamazsa acik hata.
    for (anahtar, istenen) in anahtarlar {
        let sonuc = anahtar_ayarla(
            c,
            anahtar,
            *istenen,
            zaman_asimi.min(std::time::Duration::from_secs(60)),
        )
        .await?;
        println!(
            "anahtar      : {}={} ({sonuc:?})",
            anahtar.kod,
            if *istenen { "acik" } else { "kapali" }
        );
    }

    // Once sayfanin yerlesmesini bekle. Beklemeden bakmak, sayfa henuz cizilmemisken
    // "secici kirilmis" diye sahte hata veriyordu (olculdu).
    let kutu = kutuyu_bekle(c, sel).await?;

    // Gondermeden ONCE kac yanit balonu vardi. Bu temel cizgi olmadan "onceki turun
    // yaniti, degismeden duruyor" ile "yeni yanit, tamamlandi" ayirt edilemez.
    // Kutu bulunduktan SONRA sayiliyor: once sayarsak yarim cizilmis sayfada 0 gorup
    // temel cizgiyi yanlis kurardik.
    let taban = son_balon(c, sel).await.0;

    // INSAN KAPISI, birinci ayak: katman ZATEN acik mi? Tiklamayi denemeden once
    // bakiyoruz, yoksa 120 saniye bosuna yanit bekleyip "yanit gelmedi" gibi
    // yaniltici bir hata veriyorduk (olculdu: z.ai, 129,6 sn).
    if let Some(katman) = dogrulama_katmani(c).await {
        pencereyi_one_getir(c).await;
        bail!(
            "{INSAN_KAPISI}: dogrulama katmani acik ({katman}). Pencere one getirildi — \
             dogrulamayi elle coz, sonra istegi tekrarla. Duvari asmiyoruz, gosteriyoruz."
        );
    }

    // Ikinci ayak: katman listede olmayan bir seyse tiklama reddi onu yine yakalar.
    // WebDriver bu durumda "is not clickable ... obscures it" diyor ve ustteki
    // ogenin ne oldugunu da yaziyor — teshis icin onu oldugu gibi tasiyoruz.
    if let Err(e) = kutu.click().await {
        let m = e.to_string();
        if m.contains("obscures it") || m.contains("not clickable") {
            pencereyi_one_getir(c).await;
            bail!("{INSAN_KAPISI}: istem kutusunun ustunu bir katman kapatiyor. {m}");
        }
        return Err(e).context("istem kutusuna tiklanamadi");
    }
    istemi_yaz(c, sel, &kutu, prompt).await?;
    match &sel.gonder {
        // Dugmeyle gonderen siteler: ENTER burada satir atlar (AI Studio olcumu).
        // Dugme pasifse birkac kez denenir; bulunamazsa ACIK hata — sessizce
        // ENTER'a donmek "istem gonderildi" yanilsamasi uretirdi.
        Some(secici) => gonder_dugmesine_bas(c, secici).await?,
        None => kutu.send_keys(ENTER).await.context("istem gonderilemedi")?,
    }

    const ARALIK: std::time::Duration = std::time::Duration::from_millis(500);

    let baslangic = std::time::Instant::now();
    let mut dedektor = BitisDedektoru::yeni(taban);

    while baslangic.elapsed() < zaman_asimi {
        tokio::time::sleep(ARALIK).await;
        let (sayi, metin) = son_balon(c, sel).await;
        if let Some(t) = dedektor.gozlem(sayi, &metin) {
            // Sayfa cevap yerine dogrulama hatasi bastiysa bunu CEVAP diye dondurmek
            // sessiz basarisizlik olurdu; hata donuyoruz ki istemcinin fallback
            // zinciri siradaki saglayiciya gecebilsin.
            if dogrulama_duvari_mi(&t) {
                bail!(
                    "saglayici cevap yerine dogrulama hatasi dondu: {}",
                    t.trim()
                );
            }
            return Ok(t);
        }
    }

    if dedektor.yarim().trim().is_empty() {
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
        dedektor.yarim().len()
    )
}

/// Istemi gonderen dugmeye tiklar; dugme gec etkinlesirse kisa sure bekler.
///
/// NEDEN AYRI: bazi sitelerde ENTER gonderim degil SATIR ATLAMA'dir (AI Studio:
/// `ms-run-button` icindeki dugme, Ctrl+Enter). ENTER yolunda istek hic gitmiyor
/// ama bunu ancak 120 saniyelik zaman asimindan sonra "yanit gelmedi" diye
/// ogreniyorduk. Dugme yoksa/pasifse ACIK hata verilir — sessiz ENTER yedegi
/// "gonderildi" yanilsamasi uretirdi.
async fn gonder_dugmesine_bas(c: &Client, secici: &str) -> Result<()> {
    const BEKLEME: std::time::Duration = std::time::Duration::from_secs(10);
    let basla = std::time::Instant::now();
    loop {
        match c.find(Locator::Css(secici)).await {
            Ok(d) => match d.click().await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    // Pasif dugme tiklamayi reddeder; istem yeni yazildigi icin
                    // dugme bir-iki kare gec etkinlesebiliyor.
                    if basla.elapsed() >= BEKLEME {
                        return Err(e).context(format!("gonder dugmesine tiklanamadi: {secici}"));
                    }
                }
            },
            Err(e) => {
                if basla.elapsed() >= BEKLEME {
                    return Err(e).context(format!("gonder dugmesi bulunamadi: {secici}"));
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }
}

/// Istemi yazar ve **yazildigini dogrular**.
///
/// NEDEN DOGRULAMA: olculdu — perplexity'de "Sadece 'elma' yaz." yazildiginda kutuda
/// yalnizca ilk karakter kaldi; Enter'a basilinca site "S" harfini aratti ve
/// ansiklopedik bir cevap dondu. Yani eksik yazilan istem, YANLIS SORUYA DOGRU
/// GORUNEN bir cevap uretiyor — sessiz hatanin en kotu bicimi. Sebebi editorun ilk
/// tustan sonra kendini yeniden cizmesi: sonraki tuslar artik kopmus oge'ye gidiyor.
///
/// Cozum: yaz, geri oku, tutmadiysa ogeyi YENIDEN BUL, temizle, tekrar yaz. Iki
/// denemede de tutmazsa gondermeden hata ver. Yarim istem gondermektense hic
/// gondermemek dogru.
async fn istemi_yaz(
    c: &Client,
    sel: &Selectors,
    kutu: &fantoccini::elements::Element,
    prompt: &str,
) -> Result<()> {
    // UZUN ya da COK SATIRLI istemde once JS.
    //
    // Uzunluk: olculdu, 2768 karakterlik istem tus tus yazildiginda
    // kutuya 2697 karakter dusuyordu — editor kendini yeniden cizerken tuslari
    // yutuyor. Yavas yazim asabilirdi ama 2700 x 35 ms = 95 saniye.
    //
    // Satir sonu: DAHA TEHLIKELI. Bu editorlerde Enter GONDER demek; `send_keys`
    // ile giden her `\n` istemi ortasindan yollar. Olculdu — 263 karakterlik,
    // 7 satirli bir istemde kutuda tam 7 karakter eksik cikti (koruma yakaladi).
    // Uzunluk esigi bunu kacirirdi: kisa ama cok satirli istem tus yolundan
    // gidiyordu.
    //
    // Bu bir gizleme/sahtecilik degil: `execCommand('insertText')` tarayicinin
    // kendi metin girme yolu, editorun `input` olaylarini normal sekilde
    // tetikliyor. Yaptigi tek sey tuslari tek tek gondermek yerine metni bir
    // kerede vermek.
    let cok_satirli = prompt.contains('\n');
    if prompt.chars().count() > JS_ESIGI || cok_satirli {
        match js_ile_yaz(c, sel, prompt).await {
            Ok(()) => {
                // Kutu metnini BIR KEZ okuyup hem karari hem teshisi ondan uretiyoruz;
                // ikinci bir okuma araya baska bir cizim girdiginde yaniltirdi.
                let mut kutuda = String::new();
                if let Ok(taze) = c.find(Locator::Css(&sel.prompt_input)).await {
                    kutuda = kutu_metni(&taze).await;
                    if metin_esit_mi(&kutuda, prompt) {
                        return Ok(());
                    }
                }
                if cok_satirli {
                    bail!(
                        "cok satirli istem JS ile yazildi ama geri okunamadi, GONDERILMEDI. \
                         Tus tus yazmak satir sonlarini GONDER'e cevirip istemi ortasindan \
                         yollardi; yarim gondermektense hic gondermiyoruz. [{}]",
                        uzunluk_tanisi(prompt.chars().count(), kutuda.chars().count())
                    );
                }
            }
            Err(e) if cok_satirli => {
                bail!(
                    "cok satirli istem JS ile yazilamadi ({e:#}), GONDERILMEDI: {}",
                    ENTER_UYARISI
                );
            }
            Err(e) => eprintln!("uyari: JS ile yazim basarisiz ({e:#}), tus tus denenecek"),
        }
    }

    kutu.send_keys(prompt).await.context("istem yazilamadi")?;
    if yazilan_dogru_mu(kutu, prompt).await {
        return Ok(());
    }

    // Ikinci deneme: YAVAS yaz. Bazi editorler (perplexity) ilk tustan sonra kendini
    // yeniden ciziyor ve toplu gonderilen tuslarin gerisini yutuyor; harf harf yazip
    // arada ogeyi tazelemek bunu asiyor. Yan fayda: insan temposuna zaten uygun.
    let taze = c
        .find(Locator::Css(&sel.prompt_input))
        .await
        .context("istem kutusu ikinci denemede bulunamadi")?;
    taze.click().await.ok();
    temizle(&taze).await;
    yavas_yaz(c, sel, prompt).await?;
    let taze = c
        .find(Locator::Css(&sel.prompt_input))
        .await
        .context("istem kutusu yazim sonrasi bulunamadi")?;
    if yazilan_dogru_mu(&taze, prompt).await {
        return Ok(());
    }

    let goruler = kutu_metni(&taze).await;
    bail!(
        "istem kutuya tam yazilamadi, GONDERILMEDI. Beklenen {} karakter, kutuda {} karakter \
         var (\"{}\"). Yarim istem gondermek yanlis soruya cevap almak olurdu.",
        prompt.chars().count(),
        goruler.chars().count(),
        goruler.chars().take(40).collect::<String>()
    )
}

/// Tus yoluna DUSMEME gerekcesi; hata metninde geciyor.
const ENTER_UYARISI: &str = "bu editorlerde Enter GONDER demek, tus tus yazim satir sonlarinda istemi \
     ortasindan yollardi";

/// Bu uzunluktan sonra tus tus yazmak hem kayipli hem cok yavas.
///
/// Altinda kalan istemler icin tus yolu OLCULEREK calistigi bilindigi (gemini,
/// perplexity, iki copilot) icin degistirilmedi; esik yalnizca uzun istemlerde
/// devreye giriyor.
const JS_ESIGI: usize = 500;

/// Istemi tek seferde, tarayicinin kendi metin girme yoluyla yazar.
///
/// `execCommand('insertText')` editorun `beforeinput`/`input` olaylarini normal
/// sekilde tetikler; React/ProseMirror turu editorler bunu kullanicinin yazmasindan
/// ayirt etmez. Dogrudan `value`/`textContent` atamak ise bu olaylari tetiklemez ve
/// editor metni "gormez" — o yuzden oyle yapilmiyor.
async fn js_ile_yaz(c: &Client, sel: &Selectors, prompt: &str) -> Result<()> {
    const BETIK: &str = r#"
        const el = document.querySelector(arguments[0]);
        if (!el) { return 'kutu-yok'; }
        el.focus();
        document.execCommand('selectAll', false, null);
        document.execCommand('insertText', false, arguments[1]);
        return 'tamam';
    "#;
    let sonuc = c
        .execute(
            BETIK,
            vec![
                serde_json::json!(sel.prompt_input),
                serde_json::json!(prompt),
            ],
        )
        .await
        .context("JS ile yazim calistirilamadi")?;
    if sonuc.as_str() != Some("tamam") {
        bail!(
            "JS ile yazim: istem kutusu bulunamadi ('{}')",
            sel.prompt_input
        );
    }
    Ok(())
}

/// Harf harf yazar, arada ogeyi yeniden bulur.
///
/// Editor kendini yeniden cizdiginde elimizdeki tutamak eskir; her parcadan once
/// tazesini almak bunu asiyor. Gecikme ayrica insan temposuna denk dusuyor — plandaki
/// "insan hizinda davran" maddesiyle ayni yone bakiyor, ayrica bir bedeli yok.
async fn yavas_yaz(c: &Client, sel: &Selectors, prompt: &str) -> Result<()> {
    const GECIKME: std::time::Duration = std::time::Duration::from_millis(35);
    let mut oge = c
        .find(Locator::Css(&sel.prompt_input))
        .await
        .context("yavas yazim icin istem kutusu bulunamadi")?;

    for (i, harf) in prompt.chars().enumerate() {
        // Oge eskiyebilir; arada tazele. Her harfte aramak gereksiz yavaslatirdi.
        if i > 0
            && i.is_multiple_of(10)
            && let Ok(t) = c.find(Locator::Css(&sel.prompt_input)).await
        {
            oge = t;
        }
        let s = harf.to_string();
        // SIRALI yazmak sart: paralel gondermek harfleri karistirirdi.
        if oge.send_keys(&s).await.is_err()
            && let Ok(t) = c.find(Locator::Css(&sel.prompt_input)).await
        {
            oge = t;
            oge.send_keys(&s).await.ok();
        }
        tokio::time::sleep(GECIKME).await;
    }
    Ok(())
}

/// contenteditable'da metin `text()`, textarea'da `value` ozelliginde durur.
async fn kutu_metni(e: &fantoccini::elements::Element) -> String {
    if let Ok(Some(v)) = e.prop("value").await
        && !v.is_empty()
    {
        return v;
    }
    e.text().await.unwrap_or_default()
}

/// Geri okuma tutmadiginda sebebi AYIRT EDEN teshis.
///
/// NEDEN: "geri okunamadi" tek basina iki ayri arizayi ortuyordu — kutu metni
/// KIRPMIS olabilir (saglayicinin uzunluk sinirina carpildi) ya da metin tam
/// gelmis ama karsilastirma takilmis olabilir (editor bicimlemesi). Ikisinin
/// cozumu farkli: birincisi tavani dusurmeyi, ikincisi sadelestirmeyi
/// genisletmeyi gerektirir.
///
/// Olculdu 2026-08-18 (gemini): 32.147 karakter geciyor, 40.150 gecmiyor.
/// Hangi ariza oldugunu bu satir olmadan soyleyemedik — tavani tahminle
/// degistirmek yerine once olcebilmek icin var.
fn uzunluk_tanisi(yazilan: usize, okunan: usize) -> String {
    let yorum = if okunan == 0 {
        "kutu BOS okundu — secici yanlis ya da metin hic yerlesmemis"
    } else if okunan < yazilan {
        "kutu metni KIRPMIS — saglayicinin uzunluk sinirina carpildi"
    } else if okunan > yazilan {
        "kutuda FAZLA karakter var — editor kendiliginden ekleme yapmis"
    } else {
        "uzunluklar ESIT — icerik tam, karsilastirma bicimlemede takildi"
    };
    format!("yazilan {yazilan}, kutuda {okunan} karakter: {yorum}")
}

async fn yazilan_dogru_mu(e: &fantoccini::elements::Element, prompt: &str) -> bool {
    metin_esit_mi(&kutu_metni(e).await, prompt)
}

/// Kutudaki metin istemle ayni mi? Bosluk/satir farklari onemsenmiyor — editorler
/// satir sonlarini kendilerine gore bicimliyor — ama EKSIK metin onemseniyor.
///
/// SIFIR GENISLIKLI karakterler de atiliyor. Olculdu: M365 Copilot'un editoru
/// metnin sonuna U+200C (ZWNJ) yerlestiriyor, bu da "beklenen 37, kutuda 38
/// karakter" diye dogru istemi REDDETTIRIYORDU. Bu karakterlerin gorunur karsiligi
/// yok, yani icerik tasimazlar; denetimi gevsetmiyoruz, gurultuyu ayikliyoruz.
fn metin_esit_mi(kutuda: &str, prompt: &str) -> bool {
    let sadelestir = |s: &str| {
        s.chars()
            .filter(|c| !matches!(c, '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{feff}'))
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    sadelestir(kutuda) == sadelestir(prompt)
}

/// Ctrl+A ile secip siler. WebDriver'da degistirici tus "yapiskan"dir: ayni tusu
/// tekrar gondermek onu birakir, yoksa sonraki harfler de Ctrl'lu gider.
async fn temizle(e: &fantoccini::elements::Element) {
    const CTRL: &str = "\u{E009}";
    const SIL: &str = "\u{E017}";
    e.clear().await.ok();
    e.send_keys(&format!("{CTRL}a{CTRL}{SIL}")).await.ok();
}

/// Kac ust uste ayni gozlem "bitti" sayilir. 4 x 500ms = 2 sn.
/// Dusunme panelinin metne karistirdigi ISARET kelimeleri.
///
/// OLCULDU (z.ai, 2026-08-18): yanit secicisi `.chat-assistant` dusunme panelini de
/// kapsiyor ve DOM'da onu ayirt edecek bir sinif/`data-testid` YOK — balonun cocugu
/// sinifsiz tek bir sarmalayici. Yani secici ile ayiklamak mumkun degil, metinden
/// ayiklamak zorundayiz.
///
/// Iki ayri ariza uretiyordu: (a) "Thought Process\nAnkara" -> cevabin basina
/// gurultu; (b) "Thinking... Skip" -> HENUZ CEVAP YOKKEN kararli gorunup cevap
/// diye donuyordu. Ikincisi tehlikeli olani: istemci bunu modelin cevabi saniyor.
const DUSUNME_KELIMELERI: [&str; 4] = ["thought", "process", "thinking", "skip"];

/// Bastaki dusunme satirlarini atar. Bir satir ancak TAMAMEN isaret kelimelerinden
/// olusuyorsa atilir; icinde gercek icerik varsa dokunulmaz.
fn cevabi_ayikla(metin: &str) -> &str {
    let mut kalan = metin.trim_start();
    loop {
        let son = kalan.find('\n').unwrap_or(kalan.len());
        let satir = kalan[..son].trim();
        if satir.is_empty() {
            break;
        }
        let hepsi_isaret = satir.split_whitespace().all(|k| {
            let s = k
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_ascii_lowercase();
            DUSUNME_KELIMELERI.contains(&s.as_str())
        });
        if !hepsi_isaret {
            break;
        }
        kalan = kalan[son..].trim_start();
    }
    kalan
}

/// Sayfanin cevap yerine bastigi dogrulama/hata metni mi?
///
/// OLCULDU: z.ai ardisik otomatik isteklerde balona "No response, Please try again
/// later." + "Verification failed, please try again" yaziyor. Kervan'in yanit
/// yolunda hicbir hata denetimi YOKTU, yani bu metin modelin cevabi olarak
/// donuyordu. Projenin kurali sessiz basarisizligi yasakliyor: cevap degil HATA.
fn dogrulama_duvari_mi(metin: &str) -> bool {
    let m = metin.to_ascii_lowercase();
    ["verification failed", "no response, please try again"]
        .iter()
        .any(|k| m.contains(k))
}

const KARARLILIK_ESIGI: u8 = 4;

/// Yoklama dongusunun karar cekirdegi.
///
/// Ayri bir tur olarak duruyor cunku buradaki hata sessiz ve tehlikeli: temel cizgi
/// olmadan dedektor, gonderimden hemen sonra hala ekranda duran ONCEKI turun yanitini
/// "kararli" gorup dondurur. Ne kilitlenme ne bos yanit uretir — dogru bicimde YANLIS
/// cevap dondurur. Cekirdegi ayirmak onu tarayicisiz sinanabilir kiliyor.
struct BitisDedektoru {
    taban_sayi: usize,
    onceki: String,
    sabit: u8,
}

impl BitisDedektoru {
    fn yeni(taban_sayi: usize) -> Self {
        Self {
            taban_sayi,
            onceki: String::new(),
            sabit: 0,
        }
    }

    /// `sayi`: su anki balon sayisi, `metin`: son balonun metni.
    /// Gonderimden sonra yeni balon eklenmemisse gozlem yok sayilir.
    fn gozlem(&mut self, sayi: usize, metin: &str) -> Option<String> {
        if sayi <= self.taban_sayi {
            self.sabit = 0;
            return None;
        }
        // Gurultuyu once ayikla: "Thinking... Skip" sabit durdugu icin ayiklanmadan
        // KARARLI gorunur ve cevap diye donerdi. Ayiklandiktan sonra bos kaliyorsa
        // yanit henuz baslamamis demektir.
        let metin = cevabi_ayikla(metin);
        if metin.trim().is_empty() {
            self.sabit = 0;
            return None;
        }
        if metin == self.onceki {
            self.sabit += 1;
            if self.sabit >= KARARLILIK_ESIGI {
                return Some(metin.to_string());
            }
        } else {
            self.sabit = 0;
            self.onceki = metin.to_string();
        }
        None
    }

    /// Zaman asiminda elde kalan (yarim) metin.
    fn yarim(&self) -> &str {
        &self.onceki
    }
}

/// (balon sayisi, son balonun metni). Sayfa okunamazsa (0, "") — gozlem atlanir.
async fn son_balon(c: &Client, sel: &Selectors) -> (usize, String) {
    let Ok(hepsi) = c.find_all(Locator::Css(&sel.response)).await else {
        return (0, String::new());
    };
    let metin = match hepsi.last() {
        Some(e) => e.text().await.unwrap_or_default(),
        None => String::new(),
    };
    (hepsi.len(), metin)
}

/// Sayfadaki Google hesap dugmesinden oturumun SAHIBINI okur.
///
/// Neden gerekli: profil adi kullanicinin NIYETINDEN turuyor, gercekte hangi hesabin
/// girdiginden degil. Elle giris sirasinda siralar karisabiliyor (karisti da). Adi
/// dogrulamanin tek saglam yolu hesabi sayfadan okumak.
///
/// Google dugmesinin aria-label'i "Google Hesabı: Ad (eposta)" bicimindedir; parantez
/// icini aliyoruz.
pub async fn google_hesabi(c: &Client) -> Option<String> {
    for e in c
        .find_all(Locator::Css("a[href*='SignOutOptions']"))
        .await
        .ok()?
    {
        let Ok(Some(etiket)) = e.attr("aria-label").await else {
            continue;
        };
        if let Some(bas) = etiket.find('(')
            && let Some(son) = etiket[bas..].find(')')
        {
            let eposta = etiket[bas + 1..bas + son].trim();
            if eposta.contains('@') {
                return Some(eposta.to_string());
            }
        }
    }
    None
}

/// Sayfa metninde eposta gibi gorunen ilk belirteci bulur.
///
/// Google disi saglayicilarda kimlik icin gerekli: Google'in hesap dugmesi yokken
/// (ornegin claude.ai ayar sayfasi) oturumun sahibini baska turlu okuyamiyoruz.
pub async fn sayfada_eposta(c: &Client) -> Option<String> {
    let govde = c.find(Locator::Css("body")).await.ok()?.text().await.ok()?;
    govde
        .split_whitespace()
        .map(|p| p.trim_matches(|k: char| !k.is_alphanumeric() && k != '@' && k != '.' && k != '-'))
        .find(|p| {
            // "a@b.c" en az bicimi: kullanici kismi, alan adi ve bir nokta.
            match p.split_once('@') {
                Some((k, alan)) => !k.is_empty() && alan.contains('.') && !alan.ends_with('.'),
                None => false,
            }
        })
        .map(|s| s.to_ascii_lowercase())
}

/// Canlilik sinamasi: her secicinin sayfada KAC oge esledigini ve sonuncusunun
/// metnini bastirir. Site arayuzu degistiginde "neden kirildi" sorusunu tahminle
/// degil olcumle cevaplamanin yolu.
pub async fn dom_ozeti(c: &Client, sel: &Selectors) -> Result<String> {
    let mut s = String::new();
    // Giris alanlarinin DEGERI: "adresle eposta on-doldurulabiliyor mu" sorusu
    // ancak buradan cevaplanir.
    s.push_str("giris alanlari :\n");
    for secici in [
        "input[type='email']",
        "input[name='identifier']",
        "input[type='text']",
    ] {
        for e in c.find_all(Locator::Css(secici)).await.unwrap_or_default() {
            let deger = e.prop("value").await.ok().flatten().unwrap_or_default();
            s.push_str(&format!("  {secici:<28} deger: {deger:?}\n"));
        }
    }

    // Yeni bir saglayicinin secicilerini TAHMIN etmek yerine olcmenin yolu: sik
    // kullanilan kaliplari sayip son ornegin metnini gostermek. Yapilandirmaya hangi
    // seciciyi yazacagimiz buradan belli olur.
    s.push_str("secici adaylari:\n");
    for aday in [
        "div[contenteditable='true']",
        "textarea",
        "[data-testid]",
        "article",
        ".prose",
        "main p",
    ] {
        let ogeler = c.find_all(Locator::Css(aday)).await.unwrap_or_default();
        if ogeler.is_empty() {
            continue;
        }
        let son = match ogeler.last() {
            Some(e) => e.text().await.unwrap_or_default(),
            None => String::new(),
        };
        let kirp: String = son.chars().take(50).collect();
        s.push_str(&format!(
            "  {aday:<30} {} adet   son: {kirp:?}\n",
            ogeler.len()
        ));
    }

    // data-testid DEGERLERI: claude ve perplexity gibi siteler yapiyi bunlarla
    // isaretliyor ve sinif adlarindan cok daha kararlilar. Dogru seciciyi bulmanin
    // en kisa yolu bu listeye bakmak.
    let mut kimlikler: Vec<String> = Vec::new();
    for e in c
        .find_all(Locator::Css("[data-testid]"))
        .await
        .unwrap_or_default()
    {
        if let Ok(Some(v)) = e.attr("data-testid").await
            && !kimlikler.contains(&v)
        {
            kimlikler.push(v);
        }
    }
    if !kimlikler.is_empty() {
        kimlikler.sort();
        s.push_str(&format!("data-testid    : {}\n", kimlikler.join(", ")));
    }

    for (ad, secici) in [
        ("prompt_input", sel.prompt_input.as_str()),
        ("login_hint", sel.login_hint.as_str()),
        ("response", sel.response.as_str()),
    ] {
        s.push_str(&format!("{ad:14}: {secici}\n"));
        // Virgullu seciciyi parcalarina ayirip her birini ayri sayiyoruz: birlesik
        // sayim hangi parcanin esledigini gizler.
        for parca in secici.split(',').map(str::trim) {
            let ogeler = c.find_all(Locator::Css(parca)).await.unwrap_or_default();
            let son = match ogeler.last() {
                Some(e) => e.text().await.unwrap_or_default(),
                None => String::new(),
            };
            let kirp: String = son.chars().take(60).collect();
            s.push_str(&format!(
                "  {:<28} {} adet   son: {kirp:?}\n",
                parca,
                ogeler.len()
            ));
        }
    }
    Ok(s)
}

/// SIRA ONEMLI: once giris isareti, sonra istem kutusu.
///
/// Olculdu: Gemini'nin CIKIS YAPILMIS sayfasinda da `contenteditable` bir istem
/// kutusu var (tiklayinca girise yonlendiriyor). "Istem kutusu varsa girislidir"
/// varsayimi bu yuzden yanlisti ve hic giris yapilmamis bir profile "GIRISLI"
/// dedirtiyordu — yani havuzun insan kapisi hic acilmazdi. Ayirt eden isaret
/// giris baglantisi; o varsa oturum kapalidir, istem kutusu ne derse desin.
pub async fn detect_login(c: &Client, sel: &Selectors) -> Result<LoginState> {
    detect_login_bekleyerek(c, sel, YERLESME_SURESI).await
}

/// Sayfa yerlesene kadar bekleyip oturum durumunu soyler.
///
/// NEDEN BEKLEME: `goto` belge yuklenince doner, ama bu siteler tek sayfa uygulamasi —
/// arayuz saniyeler sonra ciziliyor. Beklemeden bakmak "secici kirilmis" gibi gorunen
/// SAHTE hatalar uretiyor. Olculdu: claude.ai'da ayni profille ust uste uc kosuda
/// istem kutusu sirasiyla 1, 0 ve 0 adet cikti — secici degismedi, sayfa henuz
/// cizilmemisti. Gemini yeterince hizli oldugu icin bu hata orada gorunmuyordu.
pub async fn detect_login_bekleyerek(
    c: &Client,
    sel: &Selectors,
    zaman_asimi: std::time::Duration,
) -> Result<LoginState> {
    const ARALIK: std::time::Duration = std::time::Duration::from_millis(250);
    let baslangic = std::time::Instant::now();
    let mut kutu_gorulme: Option<std::time::Instant> = None;
    loop {
        // Sira onemli: cikis yapilmis sayfada da istem kutusu olabiliyor.
        if c.find(Locator::Css(&sel.login_hint)).await.is_ok() {
            return Ok(LoginState::NeedsLogin);
        }
        if c.find(Locator::Css(&sel.prompt_input)).await.is_ok() {
            // Kutuyu gorur gormez "girisli" DEMIYORUZ. Olculdu: Copilot'un cikisli
            // sayfasinda textarea, "Sign in to Copilot" blogundan ONCE ciziliyor;
            // ilk tura bakan kod cikis yapilmis profile GIRISLI diyordu. Kutu
            // gorulunce giris isaretine bir sure daha sans veriliyor.
            let ilk = *kutu_gorulme.get_or_insert_with(std::time::Instant::now);
            if ilk.elapsed() >= KUTU_ONAY_SURESI {
                return Ok(LoginState::Ready);
            }
        } else {
            // Kutu kayboldu: sayfa hala yerlesiyor, sayaci sifirla.
            kutu_gorulme = None;
        }
        if baslangic.elapsed() >= zaman_asimi {
            return Ok(LoginState::Unknown);
        }
        tokio::time::sleep(ARALIK).await;
    }
}

/// Istem kutusu goruldukten sonra giris isaretine taninan ek sure.
///
/// Bedeli oturum acilisinda bu kadar gecikme; karsiligi cikis yapilmis bir hesabi
/// "hazir" sanip havuza sokmamak. Sessizce yanlis hesap kullanmak bu projedeki en
/// pahali hata sinifi oldugu icin takas bu yonde yapildi.
const KUTU_ONAY_SURESI: std::time::Duration = std::time::Duration::from_secs(3);

/// Tek sayfa uygulamasinin arayuzunu cizmesi icin taninan sure.
const YERLESME_SURESI: std::time::Duration = std::time::Duration::from_secs(25);

/// Istem kutusu cizilene kadar bekler.
async fn kutuyu_bekle(c: &Client, sel: &Selectors) -> Result<fantoccini::elements::Element> {
    const ARALIK: std::time::Duration = std::time::Duration::from_millis(250);
    let baslangic = std::time::Instant::now();
    loop {
        if let Ok(e) = c.find(Locator::Css(&sel.prompt_input)).await {
            return Ok(e);
        }
        if baslangic.elapsed() >= YERLESME_SURESI {
            bail!(
                "istem kutusu {} sn icinde cizilmedi ('{}') — oturum dusmus ya da secici \
                 kirilmis olabilir",
                YERLESME_SURESI.as_secs(),
                sel.prompt_input
            );
        }
        tokio::time::sleep(ARALIK).await;
    }
}

#[cfg(test)]
mod tests;
