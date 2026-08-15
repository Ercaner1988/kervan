//! Kervan — tarayici surulu, kota ogrenen AI ag gecidi.
//!
//! Su an Asama 2'nin ilk dikey dilimi: bir hesap, bir saglayici, tarayici acilip
//! oturum durumu okunuyor. Sunucu (axum), yonlendirici ve kota ogrenici henuz yok —
//! once tarayici surmenin kendisi kanitlanmali, gerisi kolay kisim.
//!
//! Kullanim:  kervan <hesap-adi> [--headless]

mod browser;
mod config;
mod kota;
mod kurulum;
mod pool;
mod server;
mod surec;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use browser::{detect_login, BrowserBinary, LoginState, Provider, Selectors, Session};

/// gemini.google.com. Asama 3'te yapilandirmaya tasinacak.
const GEMINI: Provider = Provider {
    url: "https://gemini.google.com/app",
    selectors: Selectors {
        // `rich-textarea` sarmalayici oge; WebDriver ona yazamaz ("not interactable").
        // Yazilabilir olan icindeki contenteditable div — dogrudan onu hedefle.
        prompt_input: "div[contenteditable='true']",
        // YALNIZ ServiceLogin: `accounts.google.com` bagi girisli sayfada da var
        // (hesap degistirici), olculdu — girisli 2 adet, cikisli 1 adet. Ayirt
        // etmeyen isaret, isaret degildir.
        login_hint: "a[href*='ServiceLogin']",
        response: "model-response, message-content",
    },
    // Olculdu: bu adres kimlik sayfasina eposta alani DOLU olarak dusuyor
    // (input[name='identifier'] degeri geliyor). Toplu kurulumda kalan tek is sifre.
    giris_kalibi: "https://accounts.google.com/AccountChooser?Email={eposta}\
                   &continue=https://gemini.google.com/app",
};

/// `kervan.toml`'daki saglayici adi.
const SAGLAYICI: &str = "gemini";

/// Bir yanit icin ust sinir. Uzun istemlerde Gemini dakikalar surebiliyor.
const YANIT_ZAMAN_ASIMI: std::time::Duration = std::time::Duration::from_secs(120);

/// `/v1/models`'ta ve combo girdilerinde gorunecek ad.
const MODEL_ID: &str = "kervan/gemini-3.1-pro";

#[tokio::main]
async fn main() -> Result<()> {
    // Ilk is: tarayicilar bu surecle birlikte olsun. Sizan tarayici profili kilitli
    // birakiyor ve sonraki kosuyu modal bir pencerede asiyor.
    surec::agaci_bagla();

    let args: Vec<String> = std::env::args().collect();
    let hesap = args
        .iter()
        .skip(1)
        .find(|a| !a.starts_with("--"))
        .map(String::as_str)
        .unwrap_or("varsayilan");
    // Gorunur kip varsayilan: insan kapisi icin pencerenin gorulebilmesi gerek.
    let headless = args.iter().any(|a| a == "--headless");

    let profiles_root = profiles_root();
    let browser = BrowserBinary::discover()?;
    let geckodriver = geckodriver_yolu()?;

    println!("tarayici     : {}", browser.0.display());
    println!("surucu       : {}", geckodriver.display());
    println!("kip          : {}", if headless { "headless" } else { "gorunur" });

    // Sunucu kipinde tekil oturum HIC acilmaz: hesaplar havuzdan tembel acilir.
    // Once acip hemen kapatmak bir tarayici baslatma bedelini bosuna oderdi.
    if args.iter().any(|a| a == "--serve") {
        let port: u16 = args
            .iter()
            .position(|a| a == "--port")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(20129);

        let yol = config::varsayilan_yol();
        let cfg = config::Config::load(&yol)?;
        println!("yapilandirma : {}", yol.display());
        println!("hesap havuzu ({} hesap):", cfg.accounts.len());
        for a in &cfg.accounts {
            println!("  - {} ({})", a.name, a.provider);
        }
        println!(
            "hiz butcesi  : {} ms + 0-{} ms sapma, saatlik tavan {}",
            cfg.pacing.min_delay_ms, cfg.pacing.jitter_ms, cfg.pacing.hourly_cap
        );

        let havuz = pool::Pool::new(
            cfg.accounts,
            cfg.pacing,
            GEMINI,
            profiles_root,
            browser,
            geckodriver,
            headless,
        );
        let defter = kota::Defter::ac(kota::varsayilan_yol())?;
        println!("kota defteri : {}", kota::yol_goster(&defter).display());

        let state = Arc::new(server::AppState {
            pool: havuz,
            defter,
            kota_ayar: cfg.kota,
            model_id: MODEL_ID,
            timeout: YANIT_ZAMAN_ASIMI,
        });
        return server::serve(state, port).await;
    }

    // --kur: birden cok hesabi sirayla kur.
    if args.iter().any(|a| a == "--kur") {
        let epostalar = match args.iter().position(|a| a == "--eposta") {
            Some(i) => args
                .get(i + 1)
                .context("--eposta icin virgulle ayrilmis liste gerekli")?
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            None => kurulum::epostalari_oku()?,
        };
        if epostalar.is_empty() {
            anyhow::bail!("eposta listesi bos");
        }
        println!("\n{} hesap kurulacak. Sifreler Kervan'a GIRILMEZ; her hesabin", epostalar.len());
        println!("giris ekrani sirayla acilir, sifreyi tarayiciya sen yazarsin.");

        let sonuclar = kurulum::calistir(
            &epostalar,
            SAGLAYICI,
            &GEMINI,
            &profiles_root,
            &browser,
            &geckodriver,
            &config::varsayilan_yol(),
        )
        .await?;
        kurulum::ozet_bas(&sonuclar);
        return Ok(());
    }

    println!("hesap        : {hesap}");

    // --giris: bir kerelik elle giris. Gerekcesi browser::elle_giris uzerinde.
    if args.iter().any(|a| a == "--giris") {
        println!("\nOtomasyon DISINDA tarayici aciliyor — Google, WebDriver ile surulen");
        println!("tarayicida oturum acmayi engelliyor. Bu pencere sira bir tarayici.");
        println!("\n1. Acilan pencerede hesaba giris yap.");
        println!("2. Tarayici penceresini DUZGUN kapat (cerezler o sirada diske yaziliyor).");
        println!("3. Buraya donup Enter'a bas; Kervan girisi dogrulayacak.\n");

        browser::elle_giris(&profiles_root, hesap, &browser, GEMINI.url)?;

        // Surecin bitmesini BEKLEYEMEYIZ: Firefox/Zen bir baslatici surec kullaniyor,
        // o hemen cikip asil tarayiciyi ayri surecte birakiyor — `wait()` aninda
        // donuyor ve dogrulama kullanici giris yapmadan calisiyordu (olculdu).
        // Bitisi insan bildirir. Tarayicinin DUZGUN kapatilmasi ayrica sart:
        // cerezler o sirada diske yaziliyor, zorla oldurmek girisi kaybettirebilir.
        println!("Giris bitince tarayiciyi kapat, sonra buraya donup Enter'a bas.");
        let mut satir = String::new();
        std::io::stdin().read_line(&mut satir).ok();

        println!("dogrulaniyor...");
        // Dogrulama WebDriver ile: giris tuttu mu, havuzun gordugu haliyle bakiyoruz.
        let oturum = Session::open(&profiles_root, hesap, &browser, &geckodriver, true)
            .await
            .context(
                "profil acilamadi — tarayici penceresi hala acik olabilir. Kapat ve \
                 `--giris` ile yeniden dene.",
            )?;
        oturum.goto(GEMINI.url).await?;
        let durum = detect_login(&oturum.client, &GEMINI.selectors).await?;
        oturum.close().await.ok();
        return match durum {
            LoginState::Ready => {
                println!("durum        : GIRISLI — hesap havuza eklenmeye hazir.");
                println!("kervan.toml'a ekle:\n\n[[accounts]]\nname = \"{hesap}\"\n");
                Ok(())
            }
            _ => anyhow::bail!(
                "giris dogrulanamadi ({durum:?}). Pencerede giris tamamlanmamis olabilir; \
                 `--giris` ile yeniden dene."
            ),
        };
    }

    let oturum = Session::open(&profiles_root, hesap, &browser, &geckodriver, headless).await?;
    println!("profil       : {}", oturum.profile_dir.display());

    // --url ile baska bir adres denenebilir (teshis icin; ornegin giris adreslerinin
    // eposta on-doldurmayi destekleyip desteklemedigini olcmek).
    let adres = args
        .iter()
        .position(|a| a == "--url")
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
        .unwrap_or(GEMINI.url);
    oturum.goto(adres).await?;
    println!("acilan sayfa : {}", oturum.client.current_url().await?);

    match detect_login(&oturum.client, &GEMINI.selectors).await? {
        LoginState::Ready => println!("durum        : GIRISLI — istem kutusu bulundu"),
        LoginState::NeedsLogin => println!(
            "durum        : GIRIS GEREKLI — pencerede elle giris yap. Profil kalici, \
             bir daha sorulmayacak."
        ),
        LoginState::Unknown => println!(
            "durum        : BELIRSIZ — ne istem kutusu ne giris baglantisi bulundu. \
             Secici kirilmis ya da dogrulama duvari cikmis olabilir; pencereye bak."
        ),
    }

    // --sor "<istem>" verildiyse gercek bir tur at.
    if let Some(i) = args.iter().position(|a| a == "--sor") {
        let istem = args.get(i + 1).context("--sor icin istem metni gerekli")?;
        println!("\nistem        : {istem}");
        let t = std::time::Instant::now();
        match browser::ask(&oturum.client, &GEMINI.selectors, istem, YANIT_ZAMAN_ASIMI).await {
            Ok(yanit) => {
                println!("sure         : {:.1} sn", t.elapsed().as_secs_f32());
                println!("--- yanit ---\n{yanit}");
            }
            // {e:#} tum neden zincirini basar. Ham hatayi yutmak, OmniRoute'un
            // gemini-web'de yaptigi ve bize saatler kaybettiren seydi.
            Err(e) => println!("HATA         : {e:#}"),
        }
    }

    // --dom: secicilerin sayfada neyi esledigini olc (canlilik sinamasi).
    // --sor'dan SONRA calisir; bos sayfada olcum bir sey soylemez.
    if args.iter().any(|a| a == "--dom") {
        println!("\n{}", browser::dom_ozeti(&oturum.client, &GEMINI.selectors).await?);
    }

    if !headless {
        println!("\nPencere acik. Kapatmak icin Enter'a bas.");
        let mut satir = String::new();
        std::io::stdin().read_line(&mut satir).ok();
    }

    oturum.close().await?;
    Ok(())
}

/// Profiller depo disinda tutulur — icinde cerez ve oturum var, asla commit'lenmez.
fn profiles_root() -> PathBuf {
    std::env::var_os("KERVAN_PROFILES")
        .map(PathBuf::from)
        .unwrap_or_else(|| yerel_veri().join("kervan").join("profiles"))
}

fn geckodriver_yolu() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("KERVAN_GECKODRIVER") {
        return Ok(PathBuf::from(p));
    }
    let p = yerel_veri().join("kervan").join("bin").join("geckodriver.exe");
    if p.is_file() {
        return Ok(p);
    }
    // PATH'te olabilir; Command onu da cozer.
    which_gecko().context(
        "geckodriver bulunamadi. KERVAN_GECKODRIVER ile yol ver ya da \
         %LOCALAPPDATA%\\kervan\\bin\\geckodriver.exe altina koy.",
    )
}

fn which_gecko() -> Result<PathBuf> {
    let out = std::process::Command::new("where").arg("geckodriver").output()?;
    let s = String::from_utf8_lossy(&out.stdout);
    let ilk = s.lines().next().unwrap_or("").trim().to_string();
    if ilk.is_empty() {
        anyhow::bail!("PATH'te geckodriver yok");
    }
    Ok(PathBuf::from(ilk))
}

fn yerel_veri() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}
