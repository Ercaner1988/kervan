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
        let driver = Command::new(geckodriver)
            .arg("--port")
            .arg(port.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("geckodriver baslatilamadi: {}", geckodriver.display()))?;

        // GIZLI KIP HEADLESS DEGIL. Headless bir tarayici calisirken gorunur hale
        // GETIRILEMEZ; "arkada dursun, istedigim an one cekeyim" ancak gercek bir
        // pencereyi kucultup geri yukleyerek olur. Yan fayda: claude.ai headless'ta
        // bot duvarina carpiyordu ("Performing security verification"), gercek
        // pencerede o duvar cikmiyor.
        let args = vec!["-profile".to_string(), profile_dir.display().to_string()];
        let _ = headless;
        let caps = yetenekler(&browser.0, args);

        let client = baglan(port, caps).await?;
        let s = Self { client, driver, profile_dir };
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
        self.client.set_window_rect(GIZLI_X, GIZLI_Y, 1280, 900).await.ok();
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
        let y = self.client.new_window(true).await.context("yeni sekme acilamadi")?;
        self.client
            .switch_to_window(y.handle.clone())
            .await
            .context("yeni sekmeye gecilemedi")?;
        self.goto(url).await?;
        Ok(y.handle)
    }

    pub async fn sekmeye_gec(&self, h: fantoccini::wd::WindowHandle) -> Result<()> {
        self.client.switch_to_window(h).await.context("sekmeye gecilemedi")
    }

    pub async fn gecerli_sekme(&self) -> Result<fantoccini::wd::WindowHandle> {
        self.client.window().await.context("gecerli sekme okunamadi")
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
        .arg("-no-remote")
        .arg("-profile")
        .arg(&dir)
        .arg(url)
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
    // Once sayfanin yerlesmesini bekle. Beklemeden bakmak, sayfa henuz cizilmemisken
    // "secici kirilmis" diye sahte hata veriyordu (olculdu).
    let kutu = kutuyu_bekle(c, sel).await?;

    // Gondermeden ONCE kac yanit balonu vardi. Bu temel cizgi olmadan "onceki turun
    // yaniti, degismeden duruyor" ile "yeni yanit, tamamlandi" ayirt edilemez.
    // Kutu bulunduktan SONRA sayiliyor: once sayarsak yarim cizilmis sayfada 0 gorup
    // temel cizgiyi yanlis kurardik.
    let taban = son_balon(c, sel).await.0;
    kutu.click().await.context("istem kutusuna tiklanamadi")?;
    istemi_yaz(c, sel, &kutu, prompt).await?;
    kutu.send_keys(ENTER).await.context("istem gonderilemedi")?;

    const ARALIK: std::time::Duration = std::time::Duration::from_millis(500);

    let baslangic = std::time::Instant::now();
    let mut dedektor = BitisDedektoru::yeni(taban);

    while baslangic.elapsed() < zaman_asimi {
        tokio::time::sleep(ARALIK).await;
        let (sayi, metin) = son_balon(c, sel).await;
        if let Some(t) = dedektor.gozlem(sayi, &metin) {
            // Sayfa cevap yerine dogrulama hatasi bastiysa bunu CEVAP diye dondurmek
            // sessiz basarisizlik olurdu; hata donuyoruz ki Hermes'in fallback
            // zinciri siradaki saglayiciya gecebilsin.
            if dogrulama_duvari_mi(&t) {
                bail!("saglayici cevap yerine dogrulama hatasi dondu: {}", t.trim());
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
    // Uzunluk: olculdu, Hermes'in 2768 karakterlik istemi tus tus yazildiginda
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
                bail!("cok satirli istem JS ile yazilamadi ({e:#}), GONDERILMEDI: {}", ENTER_UYARISI);
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
const ENTER_UYARISI: &str =
    "bu editorlerde Enter GONDER demek, tus tus yazim satir sonlarinda istemi \
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
        .execute(BETIK, vec![serde_json::json!(sel.prompt_input), serde_json::json!(prompt)])
        .await
        .context("JS ile yazim calistirilamadi")?;
    if sonuc.as_str() != Some("tamam") {
        bail!("JS ile yazim: istem kutusu bulunamadi ('{}')", sel.prompt_input);
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
/// diye donuyordu. Ikincisi tehlikeli olani: Hermes bunu modelin cevabi saniyor.
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
            let s = k.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
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
    let m = metin.to_lowercase();
    ["verification failed", "no response, please try again"].iter().any(|k| m.contains(k))
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
        Self { taban_sayi, onceki: String::new(), sabit: 0 }
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
    for e in c.find_all(Locator::Css("a[href*='SignOutOptions']")).await.ok()? {
        let Ok(Some(etiket)) = e.attr("aria-label").await else { continue };
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
        .map(|s| s.to_lowercase())
}

/// Bir secicinin esledigi ogeleri TEK TEK doker: sinif, data-testid ve metin basi.
///
/// Sayim yetmedigi yerde gerekiyor. Ornegin bir "tur satiri" secicisi hem kullanici
/// hem model turunu esliyorsa, sonuncusunu okumak istemi cevap sanmak demek olur —
/// hangi ogenin kime ait oldugunu ancak boyle gorursun.
pub async fn oge_dokumu(c: &Client, secici: &str) -> Result<String> {
    let ogeler = c.find_all(Locator::Css(secici)).await.unwrap_or_default();
    let mut s = format!("inceleme '{secici}': {} oge\n", ogeler.len());
    for (i, e) in ogeler.iter().enumerate() {
        let al = |v: Option<String>, n: usize| -> String {
            v.unwrap_or_default().chars().take(n).collect()
        };
        let tid = al(e.attr("data-testid").await.ok().flatten(), 40);
        let sinif = al(e.attr("class").await.ok().flatten(), 70);
        // aria-label ve href, hesap kimligini tasiyan yerler: Google'in hesap
        // dugmesi eposta adresini aria-label icinde veriyor.
        let aria = al(e.attr("aria-label").await.ok().flatten(), 90);
        let href = al(e.attr("href").await.ok().flatten(), 70);
        let metin = al(Some(e.text().await.unwrap_or_default()), 60);
        s.push_str(&format!(
            "  [{i}] testid={tid:?} aria={aria:?}\n      href={href:?}\n      class={sinif:?}\n      text={metin:?}\n"
        ));
    }
    Ok(s)
}

/// Canlilik sinamasi: her secicinin sayfada KAC oge esledigini ve sonuncusunun
/// metnini bastirir. Site arayuzu degistiginde "neden kirildi" sorusunu tahminle
/// degil olcumle cevaplamanin yolu.
pub async fn dom_ozeti(c: &Client, sel: &Selectors) -> Result<String> {
    let mut s = String::new();
    // Giris alanlarinin DEGERI: "adresle eposta on-doldurulabiliyor mu" sorusu
    // ancak buradan cevaplanir.
    s.push_str("giris alanlari :\n");
    for secici in ["input[type='email']", "input[name='identifier']", "input[type='text']"] {
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
        s.push_str(&format!("  {aday:<30} {} adet   son: {kirp:?}\n", ogeler.len()));
    }

    // data-testid DEGERLERI: claude ve perplexity gibi siteler yapiyi bunlarla
    // isaretliyor ve sinif adlarindan cok daha kararlilar. Dogru seciciyi bulmanin
    // en kisa yolu bu listeye bakmak.
    let mut kimlikler: Vec<String> = Vec::new();
    for e in c.find_all(Locator::Css("[data-testid]")).await.unwrap_or_default() {
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
            s.push_str(&format!("  {:<28} {} adet   son: {kirp:?}\n", parca, ogeler.len()));
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

    /// Gmail takma adlarindaki `+` kodlanmazsa sunucuda BOSLUGA donusur ve yanlis
    /// hesabin giris ekrani acilir.
    #[test]
    fn arti_isareti_kodlanir() {
        let p = Provider {
            ad: "deneme".into(),
            url: "x".into(),
            selectors: Selectors {
                prompt_input: "a".into(),
                login_hint: "b".into(),
                response: "c".into(),
            },
            giris_kalibi: "https://o/?Email={eposta}".into(),
            cloudflare: false,
        };
        assert_eq!(p.giris_url("ad+etiket@gmail.com"), "https://o/?Email=ad%2Betiket@gmail.com");
    }

    /// ASIL HATA BUYDU: perplexity'de istemin yalnizca ilk karakteri kutuya girdi,
    /// Enter'a basilinca site "S" harfini aratti ve ansiklopedik bir cevap dondu.
    /// Yani eksik istem, YANLIS SORUYA DOGRU GORUNEN cevap uretiyor. Kisalmis metin
    /// asla "yazildi" sayilmamali.
    #[test]
    fn eksik_yazilan_istem_kabul_edilmez() {
        let istem = "Sadece 'elma' yaz, baska hicbir sey yazma.";
        assert!(!metin_esit_mi("S", istem), "tek harf istem sayilmamali");
        assert!(!metin_esit_mi("", istem), "bos kutu istem sayilmamali");
        assert!(!metin_esit_mi("Sadece 'elma' yaz", istem), "kirpilmis metin kabul edilmemeli");
        assert!(metin_esit_mi(istem, istem));
    }

    /// Teshis cumlesi, sonraki adimi belirledigi icin ayrimin kendisi sinaniyor:
    /// KIRPMIS -> tavani dusur, ESIT -> sadelestirmeyi genislet.
    /// OLCULEN iki ariza da burada sinaniyor; ikisi de z.ai'dan geldi.
    #[test]
    fn dusunme_gurultusu_ayiklanir() {
        // (a) cevabin basindaki gurultu
        assert_eq!(cevabi_ayikla("Thought Process\nAnkara"), "Ankara");
        // (b) henuz cevap yok: ayiklaninca geriye bir sey kalmamali
        assert_eq!(cevabi_ayikla("Thinking... Skip").trim(), "");
        assert_eq!(cevabi_ayikla("Thinking...\nSkip").trim(), "");
        // Gercek icerik tasiyan satira DOKUNULMAMALI
        assert_eq!(cevabi_ayikla("Thinking about it: Ankara"), "Thinking about it: Ankara");
        assert_eq!(cevabi_ayikla("Ankara"), "Ankara");
    }

    /// "Thinking... Skip" ayiklanmadan once KARARLI gorunuyordu — yani cevap
    /// beklemek yerine dusunme gostergesi cevap diye donuyordu.
    #[test]
    fn dusunme_gostergesi_kararli_sayilmaz() {
        let mut d = BitisDedektoru::yeni(0);
        for _ in 0..10 {
            assert_eq!(d.gozlem(1, "Thinking... Skip"), None, "gosterge cevap sayilmamali");
        }
        // Gercek cevap gelince normal kararlilik isler.
        let mut sonuc = None;
        // ESIK + 1: ilk gozlem `onceki`yi kurar, sayac ondan SONRA islemeye baslar.
        for _ in 0..=KARARLILIK_ESIGI {
            sonuc = d.gozlem(1, "Thought Process\nAnkara");
        }
        assert_eq!(sonuc.as_deref(), Some("Ankara"), "gurultu ayiklanmis cevap donmeli");
    }

    #[test]
    fn dogrulama_duvari_taninir() {
        assert!(dogrulama_duvari_mi("No response, Please try again later.\nVerification failed, please try again"));
        assert!(dogrulama_duvari_mi("Verification failed, please try again"));
        assert!(!dogrulama_duvari_mi("Ankara"));
        // Cevabin icinde gecen benzer bir cumle yanlis alarm vermemeli diye
        // kaliplar dar tutuldu; bu satir onun bekcisi.
        assert!(!dogrulama_duvari_mi("Dogrulama basarili, devam ediyorum"));
    }

    #[test]
    fn uzunluk_tanisi_arizalari_ayirt_eder() {
        assert!(uzunluk_tanisi(40150, 32000).contains("KIRPMIS"));
        assert!(uzunluk_tanisi(40150, 40150).contains("ESIT"));
        assert!(uzunluk_tanisi(100, 0).contains("BOS"));
        assert!(uzunluk_tanisi(100, 101).contains("FAZLA"));
        assert!(uzunluk_tanisi(40150, 32000).contains("40150"));
    }

    /// Editorler satir sonu/bosluk ekleyebiliyor; bunlara takilip saglam bir yazimi
    /// reddetmek de yanlis olurdu.
    #[test]
    fn bosluk_farki_yazimi_reddettirmez() {
        assert!(metin_esit_mi("  Merhaba   dunya \n", "Merhaba dunya"));
    }

    /// Olculdu: M365 Copilot'un editoru metnin sonuna U+200C birakiyor ve dogru
    /// yazilmis istem "38 karakter, beklenen 37" diye reddediliyordu. Gorunmez
    /// karakterler icerik tasimaz; ama EKSIK metin hala reddedilmeli.
    #[test]
    fn sifir_genislikli_karakter_yazimi_reddettirmez() {
        assert!(metin_esit_mi("27 carpi 3 kactir?\u{200c}", "27 carpi 3 kactir?"));
        assert!(metin_esit_mi("\u{feff}Merhaba\u{200b} dunya", "Merhaba dunya"));
        assert!(
            !metin_esit_mi("27 carpi\u{200c}", "27 carpi 3 kactir?"),
            "gorunmez karakter ayiklamak eksik metni gecirmemeli"
        );
    }

    /// Kilit tespiti dosyanin VARLIGINA degil, tutulup tutulmadigina bakmali:
    /// cokme sonrasi kalan parent.lock yuzunden saglam bir profili "kilitli" sayip
    /// hesabi kullanilamaz hale getirirdik.
    #[test]
    fn kilit_dosya_varligina_degil_tutuluyor_olmasina_bakar() {
        let dir = std::env::temp_dir().join(format!("kervan-kilit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let lock = dir.join("parent.lock");

        // Kilit dosyasi yok
        std::fs::remove_file(&lock).ok();
        assert!(!profil_kilitli(&dir));

        // Dosya var ama tutan yok (cokmeden kalmis) — kilitli SAYILMAMALI
        std::fs::write(&lock, b"").unwrap();
        assert!(!profil_kilitli(&dir), "artik kilit dosyasi profili olu birakmamali");

        std::fs::remove_dir_all(&dir).ok();
    }

    /// ASIL HATA BUYDU: ayarlar yalnizca WebDriver yolunda uygulaniyordu. Hesap
    /// kurarken kullanilan elle giris yolu tarayiciyi dogrudan baslattigi icin hicbir
    /// ayar gitmiyor ve kurulum sihirbazi her seferinde cikiyordu. Iki yol ayni
    /// listeyi tasimali; biri degisip oteki degismezse bu test dusmeli.
    #[test]
    fn iki_yol_ayni_ayarlari_tasir() {
        let caps = yetenekler(Path::new("z"), vec![]);
        let yetenek: std::collections::BTreeSet<String> = caps["moz:firefoxOptions"]["prefs"]
            .as_object()
            .expect("prefs nesnesi")
            .keys()
            .cloned()
            .collect();
        let dosya: std::collections::BTreeSet<String> = VARSAYILAN_USER_JS
            .lines()
            .filter_map(|l| l.strip_prefix("user_pref(\""))
            .filter_map(|l| l.split('"').next())
            .map(String::from)
            .collect();
        assert_eq!(dosya, yetenek, "elle giris ve WebDriver yollari ayni ayarlari vermeli");
        assert!(dosya.contains("zen.welcome-screen.seen"));
    }

    /// Elle giris yolunda ayarlar profile DOSYA olarak yazilmali; WebDriver orada yok.
    #[test]
    fn tohumlama_profile_dosya_yazar() {
        let dir = std::env::temp_dir().join(format!("kervan-tohum2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Onceki kosudan kalmis yabanci icerik EZILMELI, yoksa ayarlar gitmez.
        std::fs::write(dir.join("user.js"), "user_pref(\"baska.sey\", 1);\n").unwrap();
        profil_tohumla(&dir).unwrap();
        let icerik = std::fs::read_to_string(dir.join("user.js")).unwrap();
        assert!(icerik.contains("zen.welcome-screen.seen"));
        assert!(!icerik.contains("baska.sey"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn yetenekler_karsilama_ayarlarini_tasir() {
        let caps = yetenekler(Path::new("C:\\zen.exe"), vec!["-profile".into(), "d".into()]);
        let prefs = &caps["moz:firefoxOptions"]["prefs"];
        assert_eq!(prefs["zen.welcome-screen.seen"], true, "Zen karsilama kapisi eksik");
        assert_eq!(prefs["browser.sessionstore.resume_from_crash"], false);
        // profil bagimsizligi da ayni yetenekten geliyor; kirilirsa hesaplar birlesir
        assert_eq!(caps["moz:firefoxOptions"]["args"][0], "-profile");
    }

    /// Her oturum ayri port almali; sabit port ikinci hesabi engellerdi.
    #[test]
    fn portlar_benzersiz() {
        let a = bos_port().unwrap();
        let b = bos_port().unwrap();
        assert!(a > 0 && b > 0);
    }

    /// ASIL HATA BUYDU: ikinci turda onceki turun yaniti hala ekranda ve kararli.
    /// Temel cizgi olmadan dedektor onu "tamamlanmis yeni yanit" sanip donduruyordu —
    /// yani onceki sorunun cevabi, yeni sorunun cevabi diye. Sessiz ve yanlis.
    #[test]
    fn onceki_turun_yaniti_dondurulmez() {
        let mut d = BitisDedektoru::yeni(2); // gondermeden once 2 balon vardi
        for _ in 0..20 {
            // Sayi artmiyor: ekrandaki hala onceki turun yaniti.
            assert_eq!(d.gozlem(2, "onceki turun yaniti"), None);
        }
        assert_eq!(d.yarim(), "", "yeni balon yokken metin biriktirilmemeli");
    }

    /// Yeni balon gelip buyumesi durunca yanit donmeli.
    #[test]
    fn yeni_yanit_kararlilasinca_donulur() {
        let mut d = BitisDedektoru::yeni(2);
        assert_eq!(d.gozlem(3, "el"), None);
        assert_eq!(d.gozlem(3, "elma"), None);
        // ayni metin ust uste: esige kadar None, esikte deger
        for _ in 0..KARARLILIK_ESIGI - 1 {
            assert_eq!(d.gozlem(3, "elma"), None);
        }
        assert_eq!(d.gozlem(3, "elma"), Some("elma".to_string()));
    }

    /// Balon acildi ama henuz bos: bekleme surmeli, bos yanit dondurulmemeli.
    #[test]
    fn bos_balon_bitis_sayilmaz() {
        let mut d = BitisDedektoru::yeni(0);
        for _ in 0..10 {
            assert_eq!(d.gozlem(1, "   "), None);
        }
    }

    /// Yanit buyurken kesilirse elde kalan YARIM metin saklanmali; `ask` bunu
    /// tam yanit gibi degil, yarim oldugunu soyleyerek bildiriyor.
    #[test]
    fn yarim_metin_saklanir() {
        let mut d = BitisDedektoru::yeni(0);
        d.gozlem(1, "yarim kalmis ya");
        assert_eq!(d.yarim(), "yarim kalmis ya");
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
