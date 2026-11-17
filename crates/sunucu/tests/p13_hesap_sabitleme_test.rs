//! Hesap sabitleme kapıları: havuz (`kervan.toml` → `[[accounts]]`) hesapları.
//!
//! NEDEN AYRI TEST (2026-11-17): sabitleme kapıları uzun süre YALNIZ eklenti
//! hesaplarını tanıdı (`eklenti_hub.sekme_var_mi` / `bagli_hesaplar`). Eklenti
//! kapalıyken — yani kervan'ın normal çalışma biçiminde — panelden hesap seçmek
//! `400 "'gemini' icin bagli eklenti hesabi yok; hesap sabitlenemez"` alıyordu.
//! Oysa havuzda tanımlı Zen profilleri (ücretsiz hesaplar dahil) gerçek
//! hesaplardır. Bu testler iki şeyi ayırır:
//!
//!   1. Havuzda TANIMLI hesap kapıya takılmaz (eklenti yoksa da).
//!   2. Havuzda TANIMSIZ hesap sessizce başka hesaba düşmez; 400 + liste döner.
//!
//! Testler tarayıcı AÇMAZ: kapıdan geçen istek WebDriver aşamasında düşer ama
//! oraya kadar gelmiş olması kapının açık olduğunu kanıtlar.

use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;

use kervan_sunucu::server::{self, AppState};

/// İki hesaplı, tek sağlayıcılı havuz — hiçbir tarayıcı açılmaz.
fn durum() -> Arc<AppState> {
    let temp_dir = std::env::temp_dir().join(format!("kervan_p13_test_{}", rand::random::<u64>()));
    let _ = std::fs::create_dir_all(&temp_dir);
    let defter_kota = kervan_saglayici::kota::Defter::ac(temp_dir.join("defter.log")).unwrap();

    let saglayici = Arc::new(kervan_tarayici::browser::Provider {
        ad: "gemini".into(),
        url: "https://gemini.google.com/app".into(),
        selectors: kervan_tarayici::browser::Selectors {
            prompt_input: "textarea".into(),
            login_hint: "a[href*=accounts]".into(),
            response: ".prose".into(),
            gonder: None,
        },
        giris_kalibi: "https://accounts.google.com/?email={eposta}".into(),
        cloudflare: false,
        model: None,
        anahtarlar: vec![],
    });
    let mut defter = std::collections::HashMap::new();
    defter.insert("gemini".to_string(), saglayici);

    let ortam = kervan_havuz::pool::Ortam {
        profiles_root: temp_dir,
        browser: kervan_tarayici::browser::BrowserBinary(std::path::PathBuf::from(".")),
        geckodriver: std::path::PathBuf::from("."),
        headless: true,
    };
    let pool = kervan_havuz::pool::Pool::new(
        vec![
            kervan_tarayici::config::Account {
                name: "ucretsiz@ornek.test".into(),
                provider: None,
                providers: vec!["gemini".into()],
            },
            kervan_tarayici::config::Account {
                name: "ikinci@ornek.test".into(),
                provider: None,
                providers: vec!["gemini".into()],
            },
        ],
        kervan_tarayici::config::Pacing::default(),
        &defter,
        &Default::default(),
        ortam,
    )
    .expect("havuz kurulmali");

    Arc::new(AppState {
        pool,
        defter: defter_kota,
        kota_ayar: Default::default(),
        // Sağlayıcı defteri boş kalırsa `model_coz` kimliği çözemez ve hesap
        // kapılarına hiç ulaşılmaz (400 "hangi saglayiciya ait bilinmiyor").
        saglayicilar: defter.clone(),
        varsayilan_saglayici: "gemini".into(),
        timeout: Duration::from_secs(5),
        eklenti_hub: kervan_sunucu::eklenti::EklentiHub::new(),
        compressor: kervan_sunucu::compressor::Compressor::new(),
        router: kervan_sunucu::router::SmartRouter::new(
            kervan_sunucu::router::RoutingStrategy::Lkgp,
        ),
        sayaclar: Default::default(),
        rest_hedefleri: Default::default(),
        yukari: kervan_yukari::Yukari::new(),
        yedek_deneme: true,
        azami_deneme: 1,
        panel: Default::default(),
    })
}

async fn sunucu_ac(state: Arc<AppState>) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, server::router(state)).await.unwrap();
    });
    addr
}

/// Durum kodunu hata saymayan istemci: 400/503 gövdelerini okumak istiyoruz.
fn istemci() -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_global(Some(Duration::from_secs(20)))
            .http_status_as_error(false)
            .build(),
    )
}

fn gonder(addr: std::net::SocketAddr, govde: Value) -> (u16, String) {
    let mut r = istemci()
        .post(&format!("http://{addr}/v1/chat/completions"))
        .header("Content-Type", "application/json")
        .send_json(govde)
        .expect("tasima hatasi olmamali");
    let kod = r.status().as_u16();
    let metin = r.body_mut().read_to_string().unwrap_or_default();
    (kod, metin)
}

/// Havuzda tanımlı hesap, eklenti hiç yokken kapıya TAKILMAZ.
///
/// Hesap model kimliğine eklenir (`kervan/gemini@ucretsiz@ornek.test` değil —
/// ayırıcı `@` olduğu için profil adı e-postaysa kimlik `kervan/gemini@<eposta>`
/// olur; `hesap_eki` SON `@`dan böler). Bu yüzden burada nokta içeren bir ad
/// kullanılıyor.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn havuz_hesabi_eklenti_yokken_de_kapiya_takilmaz() {
    let addr = sunucu_ac(durum()).await;
    let (kod, govde) = tokio::task::spawn_blocking(move || {
        gonder(
            addr,
            serde_json::json!({
                "model": "kervan/gemini@ucretsiz@ornek.test",
                "messages": [{"role": "user", "content": "merhaba"}]
            }),
        )
    })
    .await
    .unwrap();

    assert_ne!(kod, 400, "havuz hesabi 400 ile reddedilmemeli: {govde}");
    assert!(
        !govde.contains("bagli eklenti hesabi yok"),
        "havuz hesabi icin eklenti sarti aranmamali: {govde}"
    );
    assert!(
        !govde.contains("bagli degil"),
        "havuz hesabi 'bagli degil' sayilmamali: {govde}"
    );
}

/// `kervan.hesap` alanı da aynı kapıdan geçer (model kimliği hesap eki taşımaz).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn kervan_hesap_alani_havuz_hesabini_sabitler() {
    let addr = sunucu_ac(durum()).await;
    let (kod, govde) = tokio::task::spawn_blocking(move || {
        gonder(
            addr,
            serde_json::json!({
                "model": "kervan/gemini",
                "messages": [{"role": "user", "content": "merhaba"}],
                "kervan": {"hesap": "gemini:ikinci@ornek.test"}
            }),
        )
    })
    .await
    .unwrap();

    assert_ne!(kod, 400, "havuz hesabi 400 ile reddedilmemeli: {govde}");
    assert!(
        !govde.contains("bagli degil"),
        "havuz hesabi 'bagli degil' sayilmamali: {govde}"
    );
}

/// Havuzda TANIMSIZ hesap sessizce başka hesaba düşmez: 400 + iki liste.
///
/// Bu, özelliğin güvenlik iddiasıdır — "yanlış hesaptan cevap" sessiz hatadır.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tanimsiz_hesap_400_doner_ve_listeleri_gosterir() {
    let addr = sunucu_ac(durum()).await;
    let (kod, govde) = tokio::task::spawn_blocking(move || {
        gonder(
            addr,
            serde_json::json!({
                "model": "kervan/gemini",
                "messages": [{"role": "user", "content": "merhaba"}],
                "kervan": {"hesap": "gemini:yokboyle@ornek.test"}
            }),
        )
    })
    .await
    .unwrap();

    assert_eq!(kod, 400, "tanimsiz hesap 400 olmali: {govde}");
    assert!(
        govde.contains("ne havuzda tanimli") || govde.contains("bagli degil"),
        "hata sebebi soylenmeli: {govde}"
    );
    assert!(
        govde.contains("Havuz hesaplari"),
        "hata havuz listesini gostermeli: {govde}"
    );
    assert!(
        govde.contains("ikinci@ornek.test") && govde.contains("ucretsiz@ornek.test"),
        "listedeki iki hesap da gorunmeli: {govde}"
    );
}

/// Yabancı sağlayıcı öneki reddedilir: `gemini` isteğinde `codex:...` yanlış hesaptır.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn yabanci_saglayici_oneki_reddedilir() {
    let addr = sunucu_ac(durum()).await;
    let (kod, govde) = tokio::task::spawn_blocking(move || {
        gonder(
            addr,
            serde_json::json!({
                "model": "kervan/gemini",
                "messages": [{"role": "user", "content": "merhaba"}],
                "kervan": {"hesap": "codex:ikinci@ornek.test"}
            }),
        )
    })
    .await
    .unwrap();

    assert_eq!(kod, 400, "yabanci onek 400 olmali: {govde}");
    assert!(
        govde.contains("ait degil"),
        "hata onek uyusmazligini soylemeli: {govde}"
    );
}
