//! `/kervan/secenekler` ucu: panele "gerçekten seçilebilir" listeyi verir.
//!
//! Nonce'lu sağlayıcı/model/anahtar adları kullanılır: sabit liste döndüren bir
//! uç bu testi geçemez. Kilitli model de burada sınanır — panel onu görür,
//! seçemez; sunucu isteği gelirse 402 verir (ayrı test: kervan-secim birim testi).

use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;

use kervan_sunucu::config::{AnahtarAyar, ModelAyar, SaglayiciAyar};
use kervan_sunucu::server::{self, AppState};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn p5_secenekler_testi() {
    let nonce_model = format!("model-{}", rand::random::<u64>());
    let nonce_anahtar = format!("anahtar-{}", rand::random::<u64>());

    let ayarlar = std::collections::HashMap::from([(
        "gemini".to_string(),
        SaglayiciAyar {
            model_dugmesi: Some("dugme".into()),
            model_oge: Some("oge".into()),
            modeller: Some(vec![
                ModelAyar {
                    kod: nonce_model.clone(),
                    etiket: "Serbest".into(),
                    kilitli: false,
                    kilit_nedeni: None,
                },
                ModelAyar {
                    kod: format!("{nonce_model}-kilitli"),
                    etiket: "Ucretli".into(),
                    kilitli: true,
                    kilit_nedeni: Some("ucretli plan gerekiyor".into()),
                },
            ]),
            anahtarlar: Some(vec![AnahtarAyar {
                kod: nonce_anahtar.clone(),
                dugme: ".ds-toggle-button".into(),
                etiket: "Search".into(),
            }]),
            ..Default::default()
        },
    )]);
    let saglayicilar = kervan_saglayici::saglayicilar::kayit_defteri(&ayarlar);
    assert!(
        saglayicilar.contains_key("gemini"),
        "test kurulumu: gemini defterde olmalı"
    );

    let temp_dir =
        std::env::temp_dir().join(format!("kervan_secenekler_test_{}", rand::random::<u64>()));
    let _ = std::fs::create_dir_all(&temp_dir);
    let defter = kervan_saglayici::kota::Defter::ac(temp_dir.join("defter.log")).unwrap();

    let ortam = kervan_havuz::pool::Ortam {
        profiles_root: temp_dir.clone(),
        browser: kervan_tarayici::browser::BrowserBinary(std::path::PathBuf::from(".")),
        geckodriver: std::path::PathBuf::from("."),
        headless: true,
    };
    let pool = kervan_havuz::pool::Pool::new(
        vec![],
        kervan_tarayici::config::Pacing::default(),
        &Default::default(),
        &Default::default(),
        ortam,
    )
    .unwrap();

    let state = Arc::new(AppState {
        pool,
        defter,
        kota_ayar: Default::default(),
        saglayicilar,
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
    });

    let dinleyici = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let adres = dinleyici.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(dinleyici, server::router(state)).await.unwrap();
    });

    let govde: Value = tokio::task::spawn_blocking(move || {
        let istemci = ureq::Agent::new_with_config(
            ureq::config::Config::builder()
                .timeout_global(Some(Duration::from_secs(5)))
                .build(),
        );
        let mut yanit = istemci
            .get(&format!("http://{adres}/kervan/secenekler"))
            .call()
            .expect("/kervan/secenekler istegi basarili olmali");
        yanit.body_mut().read_json().unwrap()
    })
    .await
    .unwrap();

    let gemini = &govde["saglayicilar"]["gemini"];
    let yanit_modelleri = gemini["modeller"].as_array().expect("modeller dizi olmali");
    assert_eq!(
        yanit_modelleri.len(),
        2,
        "iki model donmeli: {yanit_modelleri:?}"
    );
    assert_eq!(yanit_modelleri[0]["kod"], nonce_model.as_str());
    assert_eq!(yanit_modelleri[0]["kilitli"], false);
    let kilitli_kod = format!("{nonce_model}-kilitli");
    assert_eq!(yanit_modelleri[1]["kod"], kilitli_kod.as_str());
    assert_eq!(yanit_modelleri[1]["kilitli"], true);
    assert_eq!(yanit_modelleri[1]["kilit_nedeni"], "ucretli plan gerekiyor");

    let yanit_anahtarlari = gemini["anahtarlar"].as_array().expect("anahtarlar dizi");
    assert_eq!(yanit_anahtarlari[0]["kod"], nonce_anahtar.as_str());

    // Secenegi olmayan saglayici listede YOK: panelde bos baslik gorunmemeli.
    assert!(
        govde["saglayicilar"].get("qwen").is_none(),
        "secenegi olmayan saglayici listelenmemeli: {govde}"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}
