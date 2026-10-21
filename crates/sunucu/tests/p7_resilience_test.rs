use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use kervan_direnc::{DevreKesiciYapilandirma, DirencYoneticisi};
use kervan_sunucu::server::{self, AppState};

// PRD FR-6.1 ve FR-6.2: Dayanıklılık ve Hata Temizliği Uçtan Uca Testi
// 1. FR-6.1 (a): Ardışık 5xx -> kesici açılır, sonraki istek yukarı akışa gitmeden 503 döner (sayac sabit).
// 2. FR-6.1 (b): 429 + Retry-After -> soğuma, istemciye 429 + Retry-After başlığı, sonraki istek yukarı akışa gitmeden 429.
// 3. FR-6.2: Yankılanan test anahtarı ve iç sistem yolları istemci yanıtında yer almaz, temizlenir.

fn kur_test_state(
    rest_hedefleri: std::collections::HashMap<String, kervan_yukari::Hedef>,
    yukari: kervan_yukari::Yukari,
) -> Arc<AppState> {
    let temp_dir =
        std::env::temp_dir().join(format!("kervan_resilience_test_{}", rand::random::<u64>()));
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

    Arc::new(AppState {
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
        yukari,
        yedek_deneme: true,
        azami_deneme: 3,
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn p7_ardisik_5xx_kesici_503_ve_sayac_sabit() {
    let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream_listener.local_addr().unwrap();

    let istek_sayaci = Arc::new(AtomicUsize::new(0));
    let istek_sayaci_klon = istek_sayaci.clone();

    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = upstream_listener.accept().await else {
                break;
            };
            istek_sayaci_klon.fetch_add(1, Ordering::SeqCst);

            let mut buffer = Vec::new();
            let mut tmp = [0u8; 1024];
            let mut body_start = None;
            let mut content_length = None;

            while let Ok(n) = socket.read(&mut tmp).await {
                if n == 0 {
                    break;
                }
                buffer.extend_from_slice(&tmp[..n]);

                if body_start.is_none()
                    && let Some(pos) = buffer.windows(4).position(|w| w == b"\r\n\r\n")
                {
                    body_start = Some(pos + 4);
                    let header_str = String::from_utf8_lossy(&buffer[..pos]);
                    for line in header_str.lines() {
                        let lower = line.to_lowercase();
                        if lower.starts_with("content-length:")
                            && let Some(val) = line.split(':').nth(1)
                        {
                            content_length = val.trim().parse::<usize>().ok();
                        }
                    }
                }

                if let (Some(start), Some(cl)) = (body_start, content_length)
                    && buffer.len() >= start + cl
                {
                    break;
                }
            }

            let response_body = r#"{"error": "Kritik veritabanı arızası"}"#;
            let http_response = format!(
                "HTTP/1.1 500 Internal Server Error\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            let _ = socket.write_all(http_response.as_bytes()).await;
            let _ = socket.shutdown().await;
        }
    });

    let env_key_name = "TEST_P7_BREAKER_SERVER_KEY";
    unsafe {
        std::env::set_var(env_key_name, "test-p7-breaker-key");
    }

    let mut rest_hedefleri = std::collections::HashMap::new();
    rest_hedefleri.insert(
        "breaker-mock".to_string(),
        kervan_yukari::Hedef {
            ad: "breaker-mock".into(),
            bicim: "openai".into(),
            taban_url: format!("http://{}", upstream_addr),
            anahtar_env: env_key_name.into(),
            model: "mock-model".into(),
            yol: Some("/v1/chat/completions".into()),
            zaman_asimi: Duration::from_secs(5),
        },
    );

    let direnc = DirencYoneticisi::yeni(
        Arc::new(kervan_direnc::GercekSaat),
        Some(DevreKesiciYapilandirma {
            hata_esigi: 3,
            bozulma_esigi: 2,
            soguma_suresi: Duration::from_secs(30),
            yari_acik_izin: 1,
        }),
    );
    let yukari = kervan_yukari::Yukari::yeni_direncli(direnc);
    let state = kur_test_state(rest_hedefleri, yukari);

    let app = server::router(state);
    let server_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = server_listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(server_listener, app).await.unwrap();
    });

    let client = ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_global(Some(Duration::from_secs(5)))
            .build(),
    );

    let chat_url = format!("http://{}/v1/chat/completions", server_addr);
    let req_body = json!({
        "model": "kervan/breaker-mock/mock-model",
        "messages": [{"role": "user", "content": "deneme"}]
    });

    // 1., 2. ve 3. İstekler -> 500 döner (sayaç 1, 2, 3 olur)
    for _ in 1..=3 {
        let res = client.post(&chat_url).send_json(&req_body);
        match res {
            Ok(r) => assert_eq!(r.status().as_u16(), 500),
            Err(ureq::Error::StatusCode(status)) => assert_eq!(status, 500),
            Err(e) => panic!("Beklenmeyen hata: {:?}", e),
        }
    }
    assert_eq!(istek_sayaci.load(Ordering::SeqCst), 3);

    // 4. İstek -> Devre kesici açıldı, yukarı akışa GİTMEDEN 503 Service Unavailable döner!
    let res4 = client.post(&chat_url).send_json(&req_body);
    match res4 {
        Ok(r) => {
            assert_eq!(r.status().as_u16(), 503);
            let body: serde_json::Value = r.into_body().read_json().unwrap();
            let msg = body["error"]["message"].as_str().unwrap();
            assert!(msg.contains("Devre kesici açık"), "Mesaj: {}", msg);
        }
        Err(ureq::Error::StatusCode(status)) => assert_eq!(status, 503),
        Err(e) => panic!("Beklenmeyen hata: {:?}", e),
    }

    // Yukarı akış sayacı 3 kalmalıdır! (4. istek yukarı akışa gitmedi)
    assert_eq!(istek_sayaci.load(Ordering::SeqCst), 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn p7_429_retry_after_ve_soguma_kervan() {
    let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream_listener.local_addr().unwrap();

    let istek_sayaci = Arc::new(AtomicUsize::new(0));
    let istek_sayaci_klon = istek_sayaci.clone();

    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = upstream_listener.accept().await else {
                break;
            };
            istek_sayaci_klon.fetch_add(1, Ordering::SeqCst);

            let mut buffer = Vec::new();
            let mut tmp = [0u8; 1024];
            let mut body_start = None;
            let mut content_length = None;

            while let Ok(n) = socket.read(&mut tmp).await {
                if n == 0 {
                    break;
                }
                buffer.extend_from_slice(&tmp[..n]);

                if body_start.is_none()
                    && let Some(pos) = buffer.windows(4).position(|w| w == b"\r\n\r\n")
                {
                    body_start = Some(pos + 4);
                    let header_str = String::from_utf8_lossy(&buffer[..pos]);
                    for line in header_str.lines() {
                        let lower = line.to_lowercase();
                        if lower.starts_with("content-length:")
                            && let Some(val) = line.split(':').nth(1)
                        {
                            content_length = val.trim().parse::<usize>().ok();
                        }
                    }
                }

                if let (Some(start), Some(cl)) = (body_start, content_length)
                    && buffer.len() >= start + cl
                {
                    break;
                }
            }

            let response_body = r#"{"error": "Rate limit exceeded"}"#;
            let http_response = format!(
                "HTTP/1.1 429 Too Many Requests\r\nContent-Type: application/json\r\nContent-Length: {}\r\nRetry-After: 5\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            let _ = socket.write_all(http_response.as_bytes()).await;
            let _ = socket.shutdown().await;
        }
    });

    let env_key_name = "TEST_P7_RETRY_SERVER_KEY";
    unsafe {
        std::env::set_var(env_key_name, "test-p7-retry-key");
    }

    let mut rest_hedefleri = std::collections::HashMap::new();
    rest_hedefleri.insert(
        "retry-mock".to_string(),
        kervan_yukari::Hedef {
            ad: "retry-mock".into(),
            bicim: "openai".into(),
            taban_url: format!("http://{}", upstream_addr),
            anahtar_env: env_key_name.into(),
            model: "mock-model".into(),
            yol: Some("/v1/chat/completions".into()),
            zaman_asimi: Duration::from_secs(5),
        },
    );

    let state = kur_test_state(rest_hedefleri, kervan_yukari::Yukari::new());

    let app = server::router(state);
    let server_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = server_listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(server_listener, app).await.unwrap();
    });

    let client = ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_global(Some(Duration::from_secs(5)))
            .build(),
    );

    let chat_url = format!("http://{}/v1/chat/completions", server_addr);
    let req_body = json!({
        "model": "kervan/retry-mock/mock-model",
        "messages": [{"role": "user", "content": "rate limit test"}]
    });

    // 1. İstek -> 429 ve Retry-After: 5 döner
    let res1 = client.post(&chat_url).send_json(&req_body);
    match res1 {
        Ok(r) => {
            assert_eq!(r.status().as_u16(), 429);
            assert_eq!(
                r.headers().get("Retry-After").map(|v| v.to_str().unwrap()),
                Some("5")
            );
        }
        Err(ureq::Error::StatusCode(status)) => assert_eq!(status, 429),
        Err(e) => panic!("Beklenmeyen hata: {:?}", e),
    }
    assert_eq!(istek_sayaci.load(Ordering::SeqCst), 1);

    // 2. İstek hemen akabinde -> Soğuma devrede, yukarı akışa GİTMEDEN 429 döner!
    let res2 = client.post(&chat_url).send_json(&req_body);
    match res2 {
        Ok(r) => {
            assert_eq!(r.status().as_u16(), 429);
            assert_eq!(
                r.headers().get("Retry-After").map(|v| v.to_str().unwrap()),
                Some("5")
            );
            let body: serde_json::Value = r.into_body().read_json().unwrap();
            let msg = body["error"]["message"].as_str().unwrap();
            assert!(msg.contains("soğumada"), "Mesaj: {}", msg);
        }
        Err(ureq::Error::StatusCode(status)) => assert_eq!(status, 429),
        Err(e) => panic!("Beklenmeyen hata: {:?}", e),
    }

    // Yukarı akış sayacı 1 kalmalıdır!
    assert_eq!(istek_sayaci.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn p7_hata_temizligi_ve_anahtar_sizdirmazligi() {
    let gizli_anahtar = "sk-super-secret-vault-key-88776655";
    let env_key_name = "TEST_P7_LEAK_SERVER_KEY";
    unsafe {
        std::env::set_var(env_key_name, gizli_anahtar);
    }

    let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream_listener.local_addr().unwrap();

    let yankilanan_govde = format!(
        r#"{{"error": "Unauthorized key: {}", "path": "C:\\Users\\admin\\kervan\\secrets\\vault.json"}}"#,
        gizli_anahtar
    );

    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = upstream_listener.accept().await else {
                break;
            };
            let mut buffer = Vec::new();
            let mut tmp = [0u8; 1024];
            let mut body_start = None;
            let mut content_length = None;

            while let Ok(n) = socket.read(&mut tmp).await {
                if n == 0 {
                    break;
                }
                buffer.extend_from_slice(&tmp[..n]);

                if body_start.is_none()
                    && let Some(pos) = buffer.windows(4).position(|w| w == b"\r\n\r\n")
                {
                    body_start = Some(pos + 4);
                    let header_str = String::from_utf8_lossy(&buffer[..pos]);
                    for line in header_str.lines() {
                        let lower = line.to_lowercase();
                        if lower.starts_with("content-length:")
                            && let Some(val) = line.split(':').nth(1)
                        {
                            content_length = val.trim().parse::<usize>().ok();
                        }
                    }
                }

                if let (Some(start), Some(cl)) = (body_start, content_length)
                    && buffer.len() >= start + cl
                {
                    break;
                }
            }

            let http_response = format!(
                "HTTP/1.1 401 Unauthorized\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                yankilanan_govde.len(),
                yankilanan_govde
            );
            let _ = socket.write_all(http_response.as_bytes()).await;
            let _ = socket.shutdown().await;
        }
    });

    let mut rest_hedefleri = std::collections::HashMap::new();
    rest_hedefleri.insert(
        "leak-mock".to_string(),
        kervan_yukari::Hedef {
            ad: "leak-mock".into(),
            bicim: "openai".into(),
            taban_url: format!("http://{}", upstream_addr),
            anahtar_env: env_key_name.into(),
            model: "mock-model".into(),
            yol: Some("/v1/chat/completions".into()),
            zaman_asimi: Duration::from_secs(5),
        },
    );

    let state = kur_test_state(rest_hedefleri, kervan_yukari::Yukari::new());

    let app = server::router(state);
    let server_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = server_listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(server_listener, app).await.unwrap();
    });

    let client = ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(5)))
            .build(),
    );

    let chat_url = format!("http://{}/v1/chat/completions", server_addr);
    let req_body = json!({
        "model": "kervan/leak-mock/mock-model",
        "messages": [{"role": "user", "content": "leak test"}]
    });

    let res = client.post(&chat_url).send_json(&req_body);
    match res {
        Ok(r) => {
            assert_eq!(r.status().as_u16(), 401);
            let body_str = r.into_body().read_to_string().unwrap();
            // 1. Gizli anahtar asla yer almamalıdır!
            assert!(
                !body_str.contains(gizli_anahtar),
                "Gizli anahtar yanıta sızdı: {}",
                body_str
            );
            assert!(
                body_str.contains("[GIZLENDI]"),
                "Maske bulunamadı: {}",
                body_str
            );

            // 2. İç Windows yolları temizlenmelidir!
            assert!(
                !body_str.contains("C:\\Users\\admin"),
                "İç yol yanıta sızdı: {}",
                body_str
            );
            assert!(
                body_str.contains("[GIZLI_YOL]"),
                "Yol maskesi bulunamadı: {}",
                body_str
            );
        }
        Err(e) => panic!("Beklenmeyen hata: {:?}", e),
    }
}
