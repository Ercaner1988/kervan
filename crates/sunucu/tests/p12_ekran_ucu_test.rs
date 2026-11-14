//! `/kimlikler/{k}/ekran` tanı ucunun sözleşmesi.
//!
//! NEDEN: tarayıcı tarafı arızalarını metinden değil GÖRÜNTÜDEN teşhis etmek
//! gerekiyor (Ercan, 2026-11-14: "ekran görüntüsü alarak çalışırsan daha net bir
//! veri ile hareket edebilirsin"). Uç kendi başına bir iş yapmıyor — var olan
//! pencereyi fotoğraflıyor. Bu yüzden sınırları kesin olmalı:
//!
//!   1. Bilinmeyen kimlik → 404 (sessizce boş görüntü değil).
//!   2. Oturum kapalı → 409. Uç TANIM GEREĞİ pencere AÇMAZ: teşhis için
//!      pencere açmak, teşhis edilen durumu değiştirir.
//!
//! 200 yolu burada sınanmıyor; gerçek tarayıcı gerektirir ve canlı ölçümle
//! doğrulandı (2026-11-14: Gemini ana sayfası, 258.492 B PNG).

use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;

use kervan_sunucu::server::{self, AppState};

/// Tek hesaplı, tek sağlayıcılı havuz — hiçbir tarayıcı AÇILMAZ.
///
/// `Pool::new` yalnız kimlik/slot tablosunu kurar; WebDriver bağlantısı
/// `goster` çağrılana kadar kurulmaz. Bu yüzden geckodriver olmadan da
/// "oturum kapalı" durumu gerçekten elde edilir.
fn durum() -> Arc<AppState> {
    let temp_dir = std::env::temp_dir().join(format!("kervan_p12_test_{}", rand::random::<u64>()));
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
        vec![kervan_tarayici::config::Account {
            name: "hesap0".into(),
            provider: None,
            providers: vec!["gemini".into()],
        }],
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
        saglayicilar: Default::default(),
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
        azami_deneme: 3,
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

fn istemci() -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_global(Some(Duration::from_secs(10)))
            .build(),
    )
}

fn ekran(addr: std::net::SocketAddr, k: usize) -> Result<Vec<u8>, u16> {
    let url = format!("http://{addr}/kimlikler/{k}/ekran");
    match istemci().get(&url).call() {
        Ok(r) => Ok(r.into_body().read_to_vec().expect("govde okunmali")),
        Err(ureq::Error::StatusCode(kod)) => Err(kod),
        Err(e) => panic!("beklenmeyen tasiyici hatasi: {e}"),
    }
}

/// Bilinmeyen kimlik sessizce boş görüntü DÖNMEZ; 404 açık hatadır.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn bilinmeyen_kimlik_404_doner() {
    let addr = sunucu_ac(durum()).await;
    let kod = tokio::task::spawn_blocking(move || ekran(addr, 99))
        .await
        .unwrap()
        .unwrap_err();
    assert_eq!(kod, 404, "olmayan kimlik 404 olmali");
}

/// Oturum kapalıyken tanı ucu pencere AÇMAZ; 409 ile "önce göster" der.
///
/// 500 olsaydı çağıran taraf bunu sunucu arızası sanardı; 409 niyeti söyler.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn kapali_oturum_409_doner() {
    let addr = sunucu_ac(durum()).await;
    let kod = tokio::task::spawn_blocking(move || ekran(addr, 0))
        .await
        .unwrap()
        .unwrap_err();
    assert_eq!(
        kod, 409,
        "kapali oturumda 409 beklenir; pencere acmak teshisi bozardi"
    );
}

/// 409 gövdesi yönlendirme taşımalı: sonraki adımı söylemeyen hata işe yaramaz.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn kapali_oturum_hatasi_sonraki_adimi_soyler() {
    let addr = sunucu_ac(durum()).await;
    let govde: Value = tokio::task::spawn_blocking(move || {
        let url = format!("http://{addr}/kimlikler/0/ekran");
        // Durum kodunu hata saymayan istemci: 409 gövdesini okumak istiyoruz.
        let ajan = ureq::Agent::new_with_config(
            ureq::config::Config::builder()
                .timeout_global(Some(Duration::from_secs(10)))
                .http_status_as_error(false)
                .build(),
        );
        let mut r = ajan.get(&url).call().expect("tasima hatasi olmamali");
        assert_eq!(r.status().as_u16(), 409, "409 beklenir");
        r.body_mut().read_json().expect("JSON govde")
    })
    .await
    .unwrap();
    let mesaj = govde["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(
        mesaj.contains("/goster"),
        "hata sonraki adimi soylemeli, gelen: {mesaj}"
    );
}
