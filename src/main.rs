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
mod saglayicilar;
mod server;
mod surec;
mod tara;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use browser::{detect_login, BrowserBinary, LoginState, Session};

/// Saglayici verilmediginde kullanilan. Tanimlar `saglayicilar.rs`'te, ezmeler
/// `kervan.toml`'un `[providers.*]` bolumunde.
const VARSAYILAN_SAGLAYICI: &str = "gemini";

/// Bir yanit icin ust sinir. Uzun istemlerde Gemini dakikalar surebiliyor.
const YANIT_ZAMAN_ASIMI: std::time::Duration = std::time::Duration::from_secs(120);

#[tokio::main]
async fn main() -> Result<()> {
    // Ilk is: tarayicilar bu surecle birlikte olsun. Sizan tarayici profili kilitli
    // birakiyor ve sonraki kosuyu modal bir pencerede asiyor.
    surec::agaci_bagla();

    let args: Vec<String> = std::env::args().collect();
    let hesap = ilk_konumsal(&args).unwrap_or("varsayilan");
    // Gorunur kip varsayilan: insan kapisi icin pencerenin gorulebilmesi gerek.
    let headless = args.iter().any(|a| a == "--headless");

    let profiles_root = profiles_root();
    let browser = BrowserBinary::discover()?;
    let geckodriver = geckodriver_yolu()?;

    // Saglayici tanimlari: yerlesik varsayilanlar + kervan.toml ezmeleri.
    let cfg_yolu = config::varsayilan_yol();
    let cfg = config::Config::load(&cfg_yolu)?;
    let defter_saglayici = saglayicilar::kayit_defteri(&cfg.providers);
    let saglayici_adi = args
        .iter()
        .position(|a| a == "--saglayici")
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
        .unwrap_or(VARSAYILAN_SAGLAYICI);
    let saglayici = saglayicilar::bul(&defter_saglayici, saglayici_adi)?;

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

        println!("yapilandirma : {}", cfg_yolu.display());
        println!("hesap havuzu ({} hesap):", cfg.accounts.len());
        for a in &cfg.accounts {
            println!("  - {} ({})", a.name, a.saglayicilar().join(", "));
        }
        println!(
            "hiz butcesi  : {} ms + 0-{} ms sapma, saatlik tavan {}",
            cfg.pacing.min_delay_ms, cfg.pacing.jitter_ms, cfg.pacing.hourly_cap
        );

        // Ilk hesabin saglayicisi varsayilan olur: model belirtmeyen istemci en
        // azindan tanimli bir seye duser, "gemini" diye sabit bir varsayima degil.
        let varsayilan = cfg
            .accounts
            .first()
            .and_then(|a| a.saglayicilar().first().cloned())
            .unwrap_or_else(|| VARSAYILAN_SAGLAYICI.to_string());

        // Saglayici basina es zamanlilik tavani; verilmeyene 1 uygulanir.
        let eszamanli: std::collections::HashMap<String, u32> = cfg
            .providers
            .iter()
            .filter_map(|(ad, a)| a.eszamanli.map(|n| (ad.clone(), n)))
            .collect();

        // `cfg.pacing` az sonra havuza tasiniyor; tavan bilgisi burada aliniyor.
        let tavan = cfg.pacing.hourly_cap;
        let havuz = pool::Pool::new(
            cfg.accounts,
            cfg.pacing,
            &defter_saglayici,
            &eszamanli,
            pool::Ortam { profiles_root, browser, geckodriver, headless },
        )?;
        let defter = kota::Defter::ac(kota::varsayilan_yol())?;
        println!("kota defteri : {}", kota::yol_goster(&defter).display());

        // Saatlik tavan penceresi bellekte durdugu icin yeniden baslatmada
        // bosaliyordu; defterden geri kuruluyor. Sayiyi basmak sart: tohumlanmadigi
        // gun sessizce eski davranisa donerdik ve fark edilmezdi.
        if tavan > 0 {
            let n = havuz.tavani_tohumla(&defter).await;
            println!("tavan tohumu : son bir saatten {n} istek defterden geri yuklendi");
        }

        let state = Arc::new(server::AppState {
            pool: havuz,
            defter,
            kota_ayar: cfg.kota,
            saglayicilar: defter_saglayici,
            varsayilan_saglayici: varsayilan,
            timeout: YANIT_ZAMAN_ASIMI,
        });

        // Bosta tarayici supurucusu. Havuzun buyuyebilmesini saglayan sey bu:
        // hesap sayisinin tavani kod degil bellekti.
        if cfg.bosta_dakika > 0 {
            let esik = std::time::Duration::from_secs(cfg.bosta_dakika * 60);
            println!("bosta kapatma: {} dk", cfg.bosta_dakika);
            let s = state.clone();
            tokio::spawn(async move {
                // Esigin altinda bir tur: gecikme en fazla bir tur kadar sarkar.
                let mut tik = tokio::time::interval(esik.min(Duration::from_secs(60)));
                loop {
                    tik.tick().await;
                    s.pool.bosta_kapat(esik).await;
                }
            });
        } else {
            println!("bosta kapatma: kapali");
        }

        return server::serve(state, port).await;
    }

    // --tara: profil dizinindeki her profili acip gercekte hangi hesabin ve hangi
    // saglayicilarin oturumlu oldugunu OLC. Istem harcamaz.
    if args.iter().any(|a| a == "--tara") {
        let mut profiller: Vec<String> = std::fs::read_dir(&profiles_root)
            .with_context(|| format!("profil dizini okunamadi: {}", profiles_root.display()))?
            .filter_map(|g| g.ok())
            .filter(|g| g.path().is_dir())
            .map(|g| g.file_name().to_string_lossy().into_owned())
            .collect();
        profiller.sort();
        println!("\n{} profil taranacak, hicbir istem gonderilmeyecek.\n", profiller.len());
        let raporlar = tara::tara(
            &profiller,
            &defter_saglayici,
            &profiles_root,
            &browser,
            &geckodriver,
        )
        .await;
        tara::rapor_bas(&raporlar);
        return Ok(());
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

        println!("saglayici    : {saglayici_adi}");
        let sonuclar = kurulum::calistir(
            &epostalar,
            saglayici_adi,
            &saglayici,
            &profiles_root,
            &browser,
            &geckodriver,
            &cfg_yolu,
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

        browser::elle_giris(&profiles_root, hesap, &browser, &saglayici.url)?;

        // Surecin bitmesini BEKLEYEMEYIZ: Firefox/Zen bir baslatici surec kullaniyor,
        // o hemen cikip asil tarayiciyi ayri surecte birakiyor — `wait()` aninda
        // donuyor ve dogrulama kullanici giris yapmadan calisiyordu (olculdu).
        // Bitisi insan bildirir. Tarayicinin DUZGUN kapatilmasi ayrica sart:
        // cerezler o sirada diske yaziliyor, zorla oldurmek girisi kaybettirebilir.
        println!("Giris bitince tarayiciyi kapat, sonra buraya donup Enter'a bas.");
        let mut satir = String::new();
        std::io::stdin().read_line(&mut satir).ok();

        println!("dogrulaniyor...");
        // Tarayici penceresi kapansa da surec kilidi birkac saniye elinde tutuyor.
        browser::kilidin_birakilmasini_bekle(
            &profiles_root.join(hesap),
            std::time::Duration::from_secs(20),
        )
        .await;
        // Dogrulama WebDriver ile: giris tuttu mu, havuzun gordugu haliyle bakiyoruz.
        let oturum = Session::open(&profiles_root, hesap, &browser, &geckodriver, true)
            .await
            .context(
                "profil acilamadi — tarayici penceresi hala acik olabilir. Kapat ve \
                 `--giris` ile yeniden dene.",
            )?;
        oturum.goto(&saglayici.url).await?;
        let durum = detect_login(&oturum.client, &saglayici.selectors).await?;
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
        .unwrap_or(saglayici.url.as_str());
    oturum.goto(adres).await?;
    println!("acilan sayfa : {}", oturum.client.current_url().await?);

    match detect_login(&oturum.client, &saglayici.selectors).await? {
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
        match browser::ask(&oturum.client, &saglayici.selectors, istem, YANIT_ZAMAN_ASIMI).await {
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
        println!("\n{}", browser::dom_ozeti(&oturum.client, &saglayici.selectors).await?);
    }

    // --kim: sayfadan oturum sahibini oku (once Google dugmesi, sonra duz metin).
    if args.iter().any(|a| a == "--kim") {
        let k = match browser::google_hesabi(&oturum.client).await {
            Some(e) => Some(e),
            None => browser::sayfada_eposta(&oturum.client).await,
        };
        println!("kimlik       : {}", k.unwrap_or_else(|| "bulunamadi".into()));
    }

    // --incele <css>: bir secicinin esledigi ogeleri tek tek dok.
    if let Some(i) = args.iter().position(|a| a == "--incele") {
        let secici = args.get(i + 1).context("--incele icin bir CSS secici gerekli")?;
        println!("\n{}", browser::oge_dokumu(&oturum.client, secici).await?);
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
/// Deger alan bayraklar. Bunlardan sonraki arguman DEGERDIR, hesap adi degil.
const DEGERLI_BAYRAKLAR: [&str; 5] =
    ["--saglayici", "--port", "--url", "--sor", "--incele"];

/// Ilk konumsal arguman = hesap adi.
///
/// NEDEN ayri fonksiyon: onceki hali "-- ile baslamayan ilk arguman"i aliyordu,
/// yani `kervan --saglayici copilot --giris` komutunda hesap adi "copilot"
/// oluyordu ve o adla bos bir profil aciliyordu. Sessizce YANLIS HESAP acmak bu
/// projenin tekrar tekrar carptigi hata sinifi; deger alan bayraklarin
/// argumanini atlamak onu kapatiyor.
fn ilk_konumsal(args: &[String]) -> Option<&str> {
    let mut i = 1;
    while i < args.len() {
        let a = &args[i];
        if DEGERLI_BAYRAKLAR.contains(&a.as_str()) {
            i += 2;
        } else if a.starts_with("--") {
            i += 1;
        } else {
            return Some(a);
        }
    }
    None
}

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

#[cfg(test)]
mod tests {
    use super::*;

    fn v(l: &[&str]) -> Vec<String> {
        l.iter().map(|s| s.to_string()).collect()
    }

    /// Bayrak degeri hesap adi sanilirsa o adla bos bir profil acilir ve giris
    /// yanlis yere yapilir — sessiz, fark edilmesi zor.
    #[test]
    fn bayrak_degeri_hesap_adi_sanilmaz() {
        let a = v(&["kervan", "--saglayici", "copilot", "--giris"]);
        assert_eq!(ilk_konumsal(&a), None);
    }

    #[test]
    fn hesap_bayraklardan_once_de_sonra_da_bulunur() {
        assert_eq!(
            ilk_konumsal(&v(&["kervan", "buzbey_@hotmail.com", "--saglayici", "copilot"])),
            Some("buzbey_@hotmail.com")
        );
        assert_eq!(
            ilk_konumsal(&v(&["kervan", "--saglayici", "copilot", "buzbey_@hotmail.com"])),
            Some("buzbey_@hotmail.com")
        );
    }
}
