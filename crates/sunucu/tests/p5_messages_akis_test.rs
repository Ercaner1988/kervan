// Anthropic Messages köprüsü — AKIŞ (`stream: true`) testi.
//
// NEDEN VAR: Claude Code her isteği `stream: true` ile gönderiyor. 2026-11-14'te
// ölçüldü: uç akışsız JSON döndüğünde istemci turu düşürüyordu ("jev olmamış").
// Bu test iki şeyi ayrı ayrı kanıtlar: (1) istemciye `text/event-stream` olarak
// geçerli bir Anthropic olay dizisi gider, (2) araç çağrısı akışta `tool_use`
// bloğu + `input_json_delta` olarak taşınır ve `stop_reason` "tool_use" olur —
// yani Claude Code'un araç turu bu uçtan geçebilir.

use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use kervan_sunucu::server::{self, AppState};

/// Anthropic olay dizisini `(event, data)` çiftlerine ayırır.
fn olaylari_coz(govde: &str) -> Vec<(String, Value)> {
    let mut olaylar = Vec::new();
    for blok in govde.split("\n\n") {
        let blok = blok.trim();
        if blok.is_empty() {
            continue;
        }
        let mut ad = None;
        let mut veri = None;
        for satir in blok.lines() {
            if let Some(kalan) = satir.strip_prefix("event: ") {
                ad = Some(kalan.to_string());
            } else if let Some(kalan) = satir.strip_prefix("data: ") {
                veri = serde_json::from_str::<Value>(kalan).ok();
            }
        }
        if let (Some(ad), Some(veri)) = (ad, veri) {
            olaylar.push((ad, veri));
        }
    }
    olaylar
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn p5_messages_akisi_arac_cagrisini_tasir() {
    let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream_listener.local_addr().unwrap();

    let gelen_govde = Arc::new(Mutex::new(String::new()));
    let gelen_govde_klon = gelen_govde.clone();

    // Yukarı akış tek parça araç çağrısı döndürür (OpenAI biçimi).
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

        let response_body = json!({
            "id": "chatcmpl-akis-testi",
            "object": "chat.completion",
            "created": 1700000000,
            "model": "mock-model",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_akis_1",
                        "type": "function",
                        "function": {
                            "name": "Read",
                            "arguments": "{\"file_path\":\"Cargo.toml\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {"prompt_tokens": 5, "completion_tokens": 9, "total_tokens": 14}
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
            anahtar_env: "TEST_P5_AKIS_API_KEY".into(),
            model: "mock-model".into(),
            yol: None,
            zaman_asimi: Duration::from_secs(5),
        },
    );

    let anahtar_cozucu: kervan_yukari::AnahtarCozucu =
        Arc::new(|_: &str| Some("akis-test-anahtari".to_string()));

    let temp_dir = std::env::temp_dir().join(format!("kervan_akis_test_{}", rand::random::<u64>()));
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

    // Claude Code gibi konuş: stream true, araç tanımı Anthropic biçiminde.
    let (icerik_turu, govde) = tokio::task::spawn_blocking(move || {
        let client = ureq::Agent::new_with_config(
            ureq::config::Config::builder()
                .timeout_global(Some(Duration::from_secs(10)))
                .build(),
        );

        let istek = json!({
            "model": "kervan/mock_provider",
            "max_tokens": 1024,
            "stream": true,
            "messages": [{
                "role": "user",
                "content": [{"type": "text", "text": "Cargo.toml surumunu Read ile oku."}]
            }],
            "tools": [{
                "name": "Read",
                "description": "Dosya okur",
                "input_schema": {"type": "object", "properties": {"file_path": {"type": "string"}}}
            }]
        });

        let mut resp = client
            .post(&format!("http://{}/v1/messages", server_addr))
            .header("Content-Type", "application/json")
            .send_json(&istek)
            .expect("Kervan /v1/messages isteği kabul etmelidir");

        let icerik_turu = resp
            .headers()
            .get("content-type")
            .and_then(|d| d.to_str().ok())
            .unwrap_or_default()
            .to_string();
        let govde = resp
            .body_mut()
            .read_to_string()
            .expect("akış gövdesi okunabilmelidir");
        (icerik_turu, govde)
    })
    .await
    .unwrap();

    // (0) Araç tanımı çekirdeğe ULAŞTI: akış testinin ayrılmaz ön koşulu.
    let giden: Value = serde_json::from_str(&gelen_govde.lock().unwrap().clone())
        .expect("yukarı akışa giden gövde JSON olmalı");
    assert_eq!(
        giden["tools"][0]["function"]["name"], "Read",
        "Anthropic araç tanımı OpenAI biçimine çevrilmemiş: {giden}"
    );
    assert_eq!(
        giden["stream"], false,
        "çekirdeğe akışsız istek gitmeli (uç olayları tamamlanmış yanıttan kurar)"
    );

    // (1) Yanıt gerçekten olay dizisi.
    assert!(
        icerik_turu.starts_with("text/event-stream"),
        "yanıt text/event-stream olmalı, gelen: {icerik_turu}"
    );
    let olaylar = olaylari_coz(&govde);
    let adlar: Vec<&str> = olaylar.iter().map(|(a, _)| a.as_str()).collect();
    assert_eq!(
        adlar.first().copied(),
        Some("message_start"),
        "ilk olay message_start olmalı, gelen sıra: {adlar:?}\ngövde: {govde}"
    );
    assert_eq!(
        adlar.last().copied(),
        Some("message_stop"),
        "son olay message_stop olmalı, gelen sıra: {adlar:?}\ngövde: {govde}"
    );

    // (2) Araç çağrısı akışta tool_use bloğu olarak taşınıyor.
    let arac_blok = olaylar
        .iter()
        .find(|(a, v)| a == "content_block_start" && v["content_block"]["type"] == "tool_use")
        .map(|(_, v)| v.clone())
        .expect("akışta tool_use içerik bloğu yok");
    assert_eq!(arac_blok["content_block"]["name"], "Read");
    assert_eq!(arac_blok["content_block"]["id"], "call_akis_1");

    let arguman = olaylar
        .iter()
        .find(|(a, v)| a == "content_block_delta" && v["delta"]["type"] == "input_json_delta")
        .map(|(_, v)| {
            v["delta"]["partial_json"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        })
        .expect("akışta input_json_delta yok");
    let arguman_json: Value =
        serde_json::from_str(&arguman).expect("partial_json geçerli JSON olmalı");
    assert_eq!(arguman_json["file_path"], "Cargo.toml");

    // (3) Bitiş nedeni araç turunu bildiriyor.
    let bitis = olaylar
        .iter()
        .find(|(a, _)| a == "message_delta")
        .map(|(_, v)| v.clone())
        .expect("akışta message_delta yok");
    assert_eq!(
        bitis["delta"]["stop_reason"], "tool_use",
        "OpenAI tool_calls bitişi Anthropic tool_use olmalı"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}
