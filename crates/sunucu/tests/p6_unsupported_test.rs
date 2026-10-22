use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use kervan_sunucu::server::{self, AppState};

// PRD FR-5.3: Desteklenmeyen alanlarda açık hata testi
// Claude ve Gemini REST sağlayıcılarına `tools` veya çok parçalı içerik gelirse:
// 1. 400/501 JSON hata döner ("desteklenmiyor: <alan>")
// 2. Yukarı akışa ASLA istek gitmez (sahte sunucu istek sayacı 0 kalır)
// 3. OpenAI biçimli sağlayıcıda aynı `tools` kayıpsız geçer (FR-4.2)

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn p6_desteklenmeyen_alanlar_testi() {
    // 1. Sahte yukarı akış HTTP sunucusu kur
    let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream_listener.local_addr().unwrap();

    let istek_sayaci = Arc::new(AtomicUsize::new(0));
    let istek_sayaci_klon = istek_sayaci.clone();

    let env_key_name = "TEST_P6_API_KEY";
    unsafe {
        std::env::set_var(env_key_name, "test-p6-key");
    }

    // Sahte sunucu arka planda gelen istekleri sayar
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

            let response_body = json!({
                "id": "chatcmpl-mock",
                "object": "chat.completion",
                "created": 1700000000,
                "model": "mock-model",
                "choices": [{
                    "index": 0,
                    "message": { "role": "assistant", "content": "Tamam" },
                    "finish_reason": "stop"
                }]
            })
            .to_string();

            let http_response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            let _ = socket.write_all(http_response.as_bytes()).await;
            let _ = socket.shutdown().await;
        }
    });

    // 2. Kervan AppState kur: claude, gemini ve openai hedefleri
    let mut rest_hedefleri = std::collections::HashMap::new();

    rest_hedefleri.insert(
        "claude-mock".to_string(),
        kervan_yukari::Hedef {
            ad: "claude-mock".into(),
            bicim: "claude".into(),
            taban_url: format!("http://{}", upstream_addr),
            anahtar_env: env_key_name.into(),
            model: "claude-3-7-sonnet".into(),
            yol: Some("/v1/messages".into()),
            zaman_asimi: Duration::from_secs(5),
        },
    );

    rest_hedefleri.insert(
        "gemini-mock".to_string(),
        kervan_yukari::Hedef {
            ad: "gemini-mock".into(),
            bicim: "gemini".into(),
            taban_url: format!("http://{}", upstream_addr),
            anahtar_env: env_key_name.into(),
            model: "gemini-2.5-flash".into(),
            yol: None,
            zaman_asimi: Duration::from_secs(5),
        },
    );

    rest_hedefleri.insert(
        "openai-mock".to_string(),
        kervan_yukari::Hedef {
            ad: "openai-mock".into(),
            bicim: "openai".into(),
            taban_url: format!("http://{}", upstream_addr),
            anahtar_env: env_key_name.into(),
            model: "gpt-4o".into(),
            yol: Some("/v1/chat/completions".into()),
            zaman_asimi: Duration::from_secs(5),
        },
    );

    let temp_dir =
        std::env::temp_dir().join(format!("kervan_unsupported_test_{}", rand::random::<u64>()));
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
        yedek_deneme: true,
        azami_deneme: 3,
        panel: Default::default(),
    });

    let server_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let server_addr = server_listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(server_listener, server::router(state))
            .await
            .unwrap();
    });

    let endpoint = format!("http://{}/v1/chat/completions", server_addr);

    // --- TEST 1: Claude hedefte `tools` gönderimi ---
    let req_claude_tools = json!({
        "model": "kervan/claude-mock",
        "messages": [{ "role": "user", "content": "Hava nasıl?" }],
        "tools": [{
            "type": "function",
            "function": {
                "name": "hava",
                "description": "hava durumu"
            }
        }]
    });

    let ep1 = endpoint.clone();
    let (status1, _govde1) = tokio::task::spawn_blocking(move || {
        let client = ureq::Agent::new_with_config(
            ureq::config::Config::builder()
                .timeout_global(Some(Duration::from_secs(5)))
                .build(),
        );
        let res = client.post(&ep1).send_json(&req_claude_tools);
        match res {
            Ok(mut r) => (
                r.status().as_u16(),
                r.body_mut().read_json::<Value>().unwrap_or_default(),
            ),
            Err(ureq::Error::StatusCode(code)) => (code, json!({})),
            Err(e) => panic!("Beklenmeyen hata: {:?}", e),
        }
    })
    .await
    .unwrap();

    assert_eq!(status1, 501, "Claude hedefte tools 501 dönmeli");
    assert_eq!(
        istek_sayaci.load(Ordering::SeqCst),
        0,
        "Yukarı akışa istek GİTMEMELİ"
    );

    // --- TEST 2: Claude hedefte çok parçalı içerik gönderimi ---
    let req_claude_multipart = json!({
        "model": "kervan/claude-mock",
        "messages": [{
            "role": "user",
            "content": [
                { "type": "text", "text": "Resim açıklaması" },
                { "type": "image_url", "image_url": { "url": "https://example.com/resim.png" } }
            ]
        }]
    });

    let ep2 = endpoint.clone();
    let (status2, _) = tokio::task::spawn_blocking(move || {
        let client = ureq::Agent::new_with_config(
            ureq::config::Config::builder()
                .timeout_global(Some(Duration::from_secs(5)))
                .build(),
        );
        let res = client.post(&ep2).send_json(&req_claude_multipart);
        match res {
            Ok(mut r) => (
                r.status().as_u16(),
                r.body_mut().read_json::<Value>().unwrap_or_default(),
            ),
            Err(ureq::Error::StatusCode(code)) => (code, json!({})),
            Err(e) => panic!("Beklenmeyen hata: {:?}", e),
        }
    })
    .await
    .unwrap();

    assert!(
        status2 == 400 || status2 == 501,
        "Claude hedefte çok parçalı içerik 400 veya 501 dönmeli, alınan: {}",
        status2
    );
    assert_eq!(
        istek_sayaci.load(Ordering::SeqCst),
        0,
        "Yukarı akışa istek GİTMEMELİ"
    );

    // --- TEST 3: Gemini hedefte `tools` gönderimi ---
    let req_gemini_tools = json!({
        "model": "kervan/gemini-mock",
        "messages": [{ "role": "user", "content": "Hava nasıl?" }],
        "tools": [{
            "type": "function",
            "function": {
                "name": "hava",
                "description": "hava durumu"
            }
        }]
    });

    let ep3 = endpoint.clone();
    let (status3, _) = tokio::task::spawn_blocking(move || {
        let client = ureq::Agent::new_with_config(
            ureq::config::Config::builder()
                .timeout_global(Some(Duration::from_secs(5)))
                .build(),
        );
        let res = client.post(&ep3).send_json(&req_gemini_tools);
        match res {
            Ok(mut r) => (
                r.status().as_u16(),
                r.body_mut().read_json::<Value>().unwrap_or_default(),
            ),
            Err(ureq::Error::StatusCode(code)) => (code, json!({})),
            Err(e) => panic!("Beklenmeyen hata: {:?}", e),
        }
    })
    .await
    .unwrap();

    assert!(
        status3 == 400 || status3 == 501,
        "Gemini hedefte tools 400 veya 501 dönmeli"
    );
    assert_eq!(
        istek_sayaci.load(Ordering::SeqCst),
        0,
        "Yukarı akışa istek GİTMEMELİ"
    );

    // --- TEST 4: Gemini hedefte çok parçalı içerik gönderimi ---
    let req_gemini_multipart = json!({
        "model": "kervan/gemini-mock",
        "messages": [{
            "role": "user",
            "content": [
                { "type": "text", "text": "Resim açıklaması" },
                { "type": "image_url", "image_url": { "url": "https://example.com/resim.png" } }
            ]
        }]
    });

    let ep4 = endpoint.clone();
    let (status4, _) = tokio::task::spawn_blocking(move || {
        let client = ureq::Agent::new_with_config(
            ureq::config::Config::builder()
                .timeout_global(Some(Duration::from_secs(5)))
                .build(),
        );
        let res = client.post(&ep4).send_json(&req_gemini_multipart);
        match res {
            Ok(mut r) => (
                r.status().as_u16(),
                r.body_mut().read_json::<Value>().unwrap_or_default(),
            ),
            Err(ureq::Error::StatusCode(code)) => (code, json!({})),
            Err(e) => panic!("Beklenmeyen hata: {:?}", e),
        }
    })
    .await
    .unwrap();

    assert!(
        status4 == 400 || status4 == 501,
        "Gemini hedefte çok parçalı içerik 400 veya 501 dönmeli, alınan: {}",
        status4
    );
    assert_eq!(
        istek_sayaci.load(Ordering::SeqCst),
        0,
        "Yukarı akışa istek GİTMEMELİ"
    );

    // --- TEST 5: OpenAI hedefte `tools` geçer ve yukarı akışa iletilir (FR-4.2) ---
    let req_openai_tools = json!({
        "model": "kervan/openai-mock",
        "messages": [{ "role": "user", "content": "Hava nasıl?" }],
        "tools": [{
            "type": "function",
            "function": {
                "name": "hava",
                "description": "hava durumu"
            }
        }]
    });

    let ep5 = endpoint.clone();
    let (status5, _) = tokio::task::spawn_blocking(move || {
        let client = ureq::Agent::new_with_config(
            ureq::config::Config::builder()
                .timeout_global(Some(Duration::from_secs(5)))
                .build(),
        );
        let mut res = client
            .post(&ep5)
            .send_json(&req_openai_tools)
            .expect("OpenAI tools çağrısı 200 dönmeli");
        (
            res.status().as_u16(),
            res.body_mut().read_json::<Value>().unwrap_or_default(),
        )
    })
    .await
    .unwrap();

    assert_eq!(status5, 200, "OpenAI hedefte tools 200 dönmeli");
    assert_eq!(
        istek_sayaci.load(Ordering::SeqCst),
        1,
        "OpenAI hedefte yukarı akışa istek GİTMELİ (sayac = 1)"
    );
}
