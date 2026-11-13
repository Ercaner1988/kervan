// Anthropic Messages köprüsü (`POST /v1/messages`) uçtan uca testi.
//
// NEDEN VAR: Claude Code/Desktop Kervan'a Anthropic biçiminde konuşur; uç bunu
// çekirdeğin OpenAI biçimine çevirir, yanıtı geri çevirir. Test üç şeyi ayrı ayrı
// kanıtlar: (1) istek çekirdeğe OpenAI biçiminde ULAŞTI, (2) yanıt Anthropic
// biçiminde DÖNDÜ, (3) dönen metin bu koşuda üretilen rastgele nonce'u taşıyor —
// yani yanıt gerçekten yukarı akıştan geldi, sabit kalıptan değil.

use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use kervan_sunucu::server::{self, AppState};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn p5_messages_nonce_testi() {
    let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream_listener.local_addr().unwrap();

    // Her koşuda rastgele nonce: sabit yanıt döndüren bir uç bu testi geçemez.
    let nonce_degeri = format!("nonce-{}", rand::random::<u128>());
    let nonce_klon = nonce_degeri.clone();

    // Yukarı akışa ULAŞAN gövde burada saklanır: çevirinin istek yönü kanıtı.
    let gelen_govde = Arc::new(Mutex::new(String::new()));
    let gelen_govde_klon = gelen_govde.clone();

    let env_key_name = "TEST_P5_MESSAGES_API_KEY";
    // Anahtar süreç ortamına YAZILMAZ: çözücü dikişi aynı yolu (ad → değer)
    // süreç-geneli yan etki olmadan sınar. Rust 2024'te `set_var` unsafe'tir ve
    // çok iş parçacıklı testte veri yarışıdır; bu dikiş onu gereksiz kılar.
    let cozucu_ad = env_key_name.to_string();
    let anahtar_cozucu: kervan_yukari::AnahtarCozucu =
        Arc::new(move |ad: &str| (ad == cozucu_ad).then(|| "test-messages-key".to_string()));

    tokio::spawn(async move {
        let (mut socket, _) = upstream_listener.accept().await.unwrap();

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

        if let Some(start) = body_start {
            let son = content_length.map(|cl| start + cl).unwrap_or(buffer.len());
            let govde = String::from_utf8_lossy(&buffer[start..son.min(buffer.len())]).to_string();
            *gelen_govde_klon.lock().unwrap() = govde;
        }

        // Yanıt, GELEN gövdedeki kullanıcı metninden türetilir: nonce istemciden
        // yukarı akışa gidip yanıtla geri dönerse tur gerçekten kapalıdır.
        let gelen = gelen_govde_klon.lock().unwrap().clone();
        let yankilanan = serde_json::from_str::<Value>(&gelen)
            .ok()
            .and_then(|v| v["messages"][1]["content"].as_str().map(str::to_string))
            .unwrap_or_else(|| nonce_klon.clone());

        let response_body = json!({
            "id": "chatcmpl-messages-nonce",
            "object": "chat.completion",
            "created": 1700000000,
            "model": "mock-model",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": format!("Nonce yanıtı: {}", yankilanan)
                },
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 11, "completion_tokens": 7, "total_tokens": 18}
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

    let mut rest_hedefleri = std::collections::HashMap::new();
    rest_hedefleri.insert(
        "mock_provider".into(),
        kervan_yukari::Hedef {
            ad: "mock_provider".into(),
            bicim: "openai".into(),
            taban_url: format!("http://{}", upstream_addr),
            anahtar_env: env_key_name.into(),
            model: "mock-model".into(),
            yol: None,
            zaman_asimi: Duration::from_secs(5),
        },
    );

    let temp_dir =
        std::env::temp_dir().join(format!("kervan_messages_test_{}", rand::random::<u64>()));
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
        yukari: kervan_yukari::Yukari::yeni_cozuculu(Default::default(), anahtar_cozucu),
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

    // Anthropic istemcisi gibi konuş: gövde Anthropic biçiminde.
    let nonce_istek = nonce_degeri.clone();
    let resp_val: Value = tokio::task::spawn_blocking(move || {
        let client = ureq::Agent::new_with_config(
            ureq::config::Config::builder()
                .timeout_global(Some(Duration::from_secs(5)))
                .build(),
        );

        let client_req = json!({
            "model": "kervan/mock_provider",
            "max_tokens": 256,
            "system": [{"type": "text", "text": "Sen kısa yanıt veren bir asistansın."}],
            "messages": [{
                "role": "user",
                "content": [{"type": "text", "text": format!("Nonce nedir? {}", nonce_istek)}]
            }]
        });

        let mut resp = client
            .post(&format!("http://{}/v1/messages", server_addr))
            .header("Content-Type", "application/json")
            .send_json(&client_req)
            .expect("Kervan /v1/messages isteği kabul etmelidir");

        resp.body_mut()
            .read_json()
            .expect("Anthropic biçiminde geçerli JSON dönmelidir")
    })
    .await
    .unwrap();

    // (1) İstek yönü: çekirdeğe OpenAI biçimi ulaştı; Anthropic blok listesi kalmadı.
    let govde = gelen_govde.lock().unwrap().clone();
    assert!(!govde.is_empty(), "Yukarı akış hiç istek gövdesi almadı");
    let giden: Value = serde_json::from_str(&govde)
        .unwrap_or_else(|e| panic!("Yukarı akışa giden gövde JSON değil: {e}\n{govde}"));
    assert!(
        giden.get("system").is_none(),
        "Anthropic 'system' alanı olduğu gibi yukarı akışa gitmiş: {govde}"
    );
    assert_eq!(
        giden["messages"][0]["role"], "system",
        "Anthropic system blokları OpenAI system mesajına çevrilmemiş: {govde}"
    );
    assert_eq!(
        giden["messages"][0]["content"], "Sen kısa yanıt veren bir asistansın.",
        "system metni blok listesinden tek dizeye indirilmemiş: {govde}"
    );
    assert_eq!(
        giden["messages"][1]["content"],
        format!("Nonce nedir? {nonce_degeri}"),
        "kullanıcı içeriği blok listesinden metne indirilmemiş: {govde}"
    );
    assert_eq!(
        giden["stream"], false,
        "çekirdeğe akışsız istek gitmeli (uç SSE çevirisi yapmıyor): {govde}"
    );

    // (2) Yanıt yönü: Anthropic Messages zarfı.
    assert_eq!(resp_val["type"], "message", "yanıt türü 'message' olmalı");
    assert_eq!(resp_val["role"], "assistant");
    assert_eq!(
        resp_val["stop_reason"], "end_turn",
        "OpenAI 'stop' bitişi Anthropic 'end_turn' olmalı"
    );
    assert_eq!(
        resp_val["model"], "kervan/mock_provider",
        "istenen model adı korunmalı"
    );
    assert_eq!(resp_val["usage"]["input_tokens"], 11);
    assert_eq!(resp_val["usage"]["output_tokens"], 7);

    // (3) İçerik yukarı akıştan geldi: rastgele nonce metinde.
    let metin = resp_val["content"][0]["text"]
        .as_str()
        .expect("content[0].text bir dize olmalı");
    assert!(
        metin.contains(&nonce_degeri),
        "Yanıt bu koşunun nonce'unu içermiyor! Beklenen: {nonce_degeri}, Gelen: {metin}"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}
