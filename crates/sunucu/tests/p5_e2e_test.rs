use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use kervan_sunucu::server::{self, AppState};

// FR-4.4 Uçtan uca nonce testi
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn p5_e2e_nonce_testi() {
    // 1. Sahte yukarı akış HTTP sunucusu kur
    let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream_listener.local_addr().unwrap();

    let istek_sayaci = Arc::new(AtomicUsize::new(0));
    let istek_sayaci_klon = istek_sayaci.clone();

    // Her koşuda rastgele bir nonce üretilir
    let nonce_degeri = format!("nonce-{}", rand::random::<u128>());
    let nonce_klon = nonce_degeri.clone();

    let env_key_name = "TEST_P5_NONCE_API_KEY";
    unsafe {
        std::env::set_var(env_key_name, "test-nonce-key");
    }

    // Sahte yukarı akış sunucusu
    tokio::spawn(async move {
        let (mut socket, _) = upstream_listener.accept().await.unwrap();
        istek_sayaci_klon.fetch_add(1, Ordering::SeqCst);

        let mut buffer = Vec::new();
        let mut tmp = [0u8; 1024];
        let mut body_start = None;
        let mut content_length = None;

        loop {
            let n = socket.read(&mut tmp).await.unwrap();
            if n == 0 {
                break;
            }
            buffer.extend_from_slice(&tmp[..n]);

            if body_start.is_none() {
                if let Some(pos) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
                    body_start = Some(pos + 4);
                    let header_str = String::from_utf8_lossy(&buffer[..pos]);
                    for line in header_str.lines() {
                        let lower = line.to_lowercase();
                        if lower.starts_with("content-length:") {
                            if let Some(val) = line.split(':').nth(1) {
                                content_length = val.trim().parse::<usize>().ok();
                            }
                        }
                    }
                }
            }

            if let (Some(start), Some(cl)) = (body_start, content_length) {
                if buffer.len() >= start + cl {
                    break;
                }
            }
        }

        let response_body = json!({
            "id": "chatcmpl-nonce-response",
            "object": "chat.completion",
            "created": 1700000000,
            "model": "mock-model",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": format!("Nonce yanıtı: {}", nonce_klon)
                },
                "finish_reason": "stop"
            }]
        })
        .to_string();

        let http_response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        socket.write_all(http_response.as_bytes()).await.unwrap();
        let _ = socket.shutdown().await;
    });

    // 2. Kervan AppState kur
    let mut rest_hedefleri = std::collections::HashMap::new();
    let hedef = kervan_yukari::Hedef {
        ad: "mock_provider".into(),
        bicim: "openai".into(),
        taban_url: format!("http://{}", upstream_addr),
        anahtar_env: env_key_name.into(),
        model: "mock-model".into(),
        yol: None,
        zaman_asimi: Duration::from_secs(5),
    };
    rest_hedefleri.insert("mock_provider".into(), hedef);

    let temp_dir =
        std::env::temp_dir().join(format!("kervan_nonce_test_{}", rand::random::<u64>()));
    let _ = std::fs::create_dir_all(&temp_dir);
    let defter_yolu = temp_dir.join("defter.log");
    let defter = kervan_saglayici::kota::Defter::ac(defter_yolu).unwrap();

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
        saglayicilar: Default::default(),
        varsayilan_saglayici: "gemini".into(),
        timeout: Duration::from_secs(5),
        eklenti_hub: kervan_sunucu::eklenti::EklentiHub::new(),
        compressor: kervan_sunucu::compressor::Compressor::new(),
        router: kervan_sunucu::router::SmartRouter::new(
            kervan_sunucu::router::RoutingStrategy::Lkgp,
        ),
        sayaclar: Default::default(),
        rest_hedefleri,
        yukari: kervan_yukari::Yukari::new(),
    });

    // 3. Gerçek server::router'ı axum::serve ile aç
    let server_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = server_listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(server_listener, server::router(state))
            .await
            .unwrap();
    });

    // 4. İstemci olarak istek at (spawn_blocking içinde)
    let resp_val: Value = tokio::task::spawn_blocking(move || {
        let client = ureq::Agent::new_with_config(
            ureq::config::Config::builder()
                .timeout_global(Some(Duration::from_secs(5)))
                .build(),
        );

        let client_req = json!({
            "model": "kervan/mock_provider",
            "messages": [{"role": "user", "content": "Rastgele nonce nedir?"}]
        });

        let client_url = format!("http://{}/v1/chat/completions", server_addr);
        let mut resp = client
            .post(&client_url)
            .header("Content-Type", "application/json")
            .send_json(&client_req)
            .expect("Kervan sunucusu isteği kabul etmelidir");

        resp.body_mut()
            .read_json()
            .expect("Yanıt geçerli JSON olmalıdır")
    })
    .await
    .unwrap();

    // 5. Doğrulamalar:
    // (a) Sahte yukarı akış bu koşuda istek ALDI MI?
    assert!(
        istek_sayaci.load(Ordering::SeqCst) > 0,
        "Sahte yukarı akış hiç istek almadı!"
    );

    // (b) İstemciye dönen yanıt rastgele üretilen nonce'u İÇERİYOR MU?
    let content = resp_val["choices"][0]["message"]["content"]
        .as_str()
        .unwrap();
    assert!(
        content.contains(&nonce_degeri),
        "Yanıt nonce değerini içermiyor! Beklenen: {}, Gelen: {}",
        nonce_degeri,
        content
    );

    unsafe {
        std::env::remove_var(env_key_name);
    }
    let _ = std::fs::remove_dir_all(&temp_dir);
}

// FR-4.6 Model Listesi Testi
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn p5_models_testi() {
    let mut rest_hedefleri = std::collections::HashMap::new();
    rest_hedefleri.insert(
        "mock1".into(),
        kervan_yukari::Hedef {
            ad: "mock1".into(),
            bicim: "openai".into(),
            taban_url: "http://127.0.0.1:80".into(),
            anahtar_env: "ENV1".into(),
            model: "model-1".into(),
            yol: None,
            zaman_asimi: Duration::from_secs(5),
        },
    );
    rest_hedefleri.insert(
        "mock2".into(),
        kervan_yukari::Hedef {
            ad: "mock2".into(),
            bicim: "openai".into(),
            taban_url: "http://127.0.0.1:80".into(),
            anahtar_env: "ENV2".into(),
            model: "model-2".into(),
            yol: None,
            zaman_asimi: Duration::from_secs(5),
        },
    );

    let temp_dir =
        std::env::temp_dir().join(format!("kervan_models_test_{}", rand::random::<u64>()));
    let _ = std::fs::create_dir_all(&temp_dir);
    let defter_yolu = temp_dir.join("defter.log");
    let defter = kervan_saglayici::kota::Defter::ac(defter_yolu).unwrap();

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
        saglayicilar: Default::default(),
        varsayilan_saglayici: "gemini".into(),
        timeout: Duration::from_secs(5),
        eklenti_hub: kervan_sunucu::eklenti::EklentiHub::new(),
        compressor: kervan_sunucu::compressor::Compressor::new(),
        router: kervan_sunucu::router::SmartRouter::new(
            kervan_sunucu::router::RoutingStrategy::Lkgp,
        ),
        sayaclar: Default::default(),
        rest_hedefleri,
        yukari: kervan_yukari::Yukari::new(),
    });

    let server_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = server_listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(server_listener, server::router(state))
            .await
            .unwrap();
    });

    let resp_val: Value = tokio::task::spawn_blocking(move || {
        let client = ureq::Agent::new_with_config(
            ureq::config::Config::builder()
                .timeout_global(Some(Duration::from_secs(5)))
                .build(),
        );

        let mut resp = client
            .get(&format!("http://{}/v1/models", server_addr))
            .call()
            .expect("/v1/models isteği başarılı olmalıdır");

        resp.body_mut().read_json().unwrap()
    })
    .await
    .unwrap();
    let data = resp_val["data"]
        .as_array()
        .expect("data bir dizi olmalıdır");

    let model_idler: Vec<&str> = data.iter().filter_map(|m| m["id"].as_str()).collect();

    // 1. Gerçek REST sağlayıcıları kervan/<ad> olarak listelenir
    assert!(
        model_idler.contains(&"kervan/mock1"),
        "kervan/mock1 listede olmalı"
    );
    assert!(
        model_idler.contains(&"kervan/mock2"),
        "kervan/mock2 listede olmalı"
    );

    // 2. Uydurma OpenAI modelleri (gpt-4o vb.) BULUNMAZ
    assert!(!model_idler.contains(&"gpt-4o"), "gpt-4o listede olmamalı");
    assert!(
        !model_idler.contains(&"gpt-4o-mini"),
        "gpt-4o-mini listede olmamalı"
    );
    assert!(
        !model_idler.contains(&"gpt-3.5-turbo"),
        "gpt-3.5-turbo listede olmamalı"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}
