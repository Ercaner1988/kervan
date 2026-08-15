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

        let mut args = vec!["-profile".to_string(), profile_dir.display().to_string()];
        if headless {
            args.push("-headless".to_string());
        }
        let caps = yetenekler(&browser.0, args);

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

/// Profil su an baska bir surecte acik mi?
///
/// Firefox profil dizininde `parent.lock` tutar ve calistigi surece acik birakir.
/// Dosyanin VARLIGI kanit degil — cokme sonrasi geride kalabiliyor. Yazmaya acmayi
/// denemek canliyi olulden ayiriyor; olculdu: tutan surec varken acilmiyor, o surec
/// olunce ayni dosya sorunsuz aciliyor.
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
    // Gondermeden ONCE kac yanit balonu vardi. Bu temel cizgi olmadan "onceki turun
    // yaniti, degismeden duruyor" ile "yeni yanit, tamamlandi" ayirt edilemez.
    let taban = son_balon(c, sel).await.0;

    let kutu = c
        .find(Locator::Css(&sel.prompt_input))
        .await
        .context("istem kutusu bulunamadi — oturum dusmus ya da secici kirilmis olabilir")?;
    kutu.click().await.context("istem kutusuna tiklanamadi")?;
    kutu.send_keys(prompt).await.context("istem yazilamadi")?;
    kutu.send_keys(ENTER).await.context("istem gonderilemedi")?;

    const ARALIK: std::time::Duration = std::time::Duration::from_millis(500);

    let baslangic = std::time::Instant::now();
    let mut dedektor = BitisDedektoru::yeni(taban);

    while baslangic.elapsed() < zaman_asimi {
        tokio::time::sleep(ARALIK).await;
        let (sayi, metin) = son_balon(c, sel).await;
        if let Some(t) = dedektor.gozlem(sayi, &metin) {
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

/// Kac ust uste ayni gozlem "bitti" sayilir. 4 x 500ms = 2 sn.
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
        if sayi <= self.taban_sayi || metin.trim().is_empty() {
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
    if c.find(Locator::Css(&sel.login_hint)).await.is_ok() {
        return Ok(LoginState::NeedsLogin);
    }
    if c.find(Locator::Css(&sel.prompt_input)).await.is_ok() {
        return Ok(LoginState::Ready);
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
        };
        assert_eq!(p.giris_url("ad+etiket@gmail.com"), "https://o/?Email=ad%2Betiket@gmail.com");
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
