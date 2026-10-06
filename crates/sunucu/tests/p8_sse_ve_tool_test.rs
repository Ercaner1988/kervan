use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

use kervan_sunucu::server::{self, AppState};

// PRD FR-6.3 ve FR-6.4: Gerçek SSE ve Araç Çağrısı Geçişi Testi
// 1. FR-6.3: Gecikmeli sahte sunucuyla ilk parça tüm yanıt bitmeden ulaşır; nonce parçalarının sırası korunur; [DONE] ile biter.
// 2. FR-6.3: Claude ve Gemini akış SSE çevirisi OpenAI chunk biçimine dönüştürülür.
// 3. FR-6.4: OpenAI hedeflerde araç çağrısı (tools / tool_calls) akışsız ve akışlı kayıpsız iletilir.
// 4. FR-6.4: Claude ve Gemini hedeflerde akışta tools gelirse yukarı akışa istek gitmeden açık hata (501) döner.

fn kur_test_state(
    rest_hedefleri: std::collections::HashMap<String, kervan_yukari::Hedef>,
    yukari: kervan_yukari::Yukari,
) -> Arc<AppState> {
    let temp_dir = std::env::temp_dir().join(format!("kervan_p8_test_{}", rand::random::<u64>()));
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
        router: kervan_sunucu::router::SmartRouter::new(kervan_sunucu::router::RoutingStrategy::Lkgp),
        sayaclar: Default::default(),
        rest_hedefleri,
        yukari,
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
pub async fn test_p8_gercek_sse_zamanlama_ve_parca_sirasi() {
    let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream_listener.local_addr().unwrap();

    let nonce_a = format!("NONCE_A_{:08x}", rand::random::<u32>());
    let nonce_b = format!("NONCE_B_{:08x}", rand::random::<u32>());
    let nonce_a_klon = nonce_a.clone();
    let nonce_b_klon = nonce_b.clone();

    tokio::spawn(async move {
        let (mut socket, _) = upstream_listener.accept().await.unwrap();
        let mut buf = [0u8; 1024];
        let _ = socket.read(&mut buf).await;

        let chunk1 = format!(
            "data: {{\"id\":\"chatcmpl-stream\",\"object\":\"chat.completion.chunk\",\"choices\":[{{\"delta\":{{\"content\":\"{}\"}}}}]}}\n\n",
            nonce_a_klon
        );
        let chunk2 = format!(
            "data: {{\"id\":\"chatcmpl-stream\",\"object\":\"chat.completion.chunk\",\"choices\":[{{\"delta\":{{\"content\":\"{}\"}}}}]}}\n\n",
            nonce_b_klon
        );
        let done = "data: [DONE]\n\n";

        let baslik = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n";
        socket.write_all(baslik.as_bytes()).await.unwrap();
        socket.write_all(chunk1.as_bytes()).await.unwrap();
        socket.flush().await.unwrap();

        // İkinci parça öncesinde bilerek gecikme koy (FR-6.3 gecikmeli sunucu kabul kriteri)
        tokio::time::sleep(Duration::from_millis(150)).await;

        socket.write_all(chunk2.as_bytes()).await.unwrap();
        socket.write_all(done.as_bytes()).await.unwrap();
        socket.flush().await.unwrap();
        let _ = socket.shutdown().await;
    });

    unsafe {
        std::env::set_var("TEST_P8_OPENAI_KEY", "sk-test-stream-key");
    }

    let mut rest_map = std::collections::HashMap::new();
    rest_map.insert(
        "test-stream-openai".to_string(),
        kervan_yukari::Hedef {
            ad: "test-stream-openai".into(),
            bicim: "openai".into(),
            taban_url: format!("http://{}", upstream_addr),
            anahtar_env: "TEST_P8_OPENAI_KEY".into(),
            model: "gpt-4o".into(),
            yol: Some("/v1/chat/completions".into()),
            zaman_asimi: Duration::from_secs(5),
        },
    );

    let state = kur_test_state(rest_map, kervan_yukari::Yukari::new());
    let router = server::router(state);

    let kervan_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let kervan_addr = kervan_listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(kervan_listener, router).await.unwrap();
    });

    let t0 = Instant::now();
    let mut stream_socket = tokio::net::TcpStream::connect(kervan_addr).await.unwrap();

    let req_body = json!({
        "model": "kervan/test-stream-openai",
        "messages": [{"role": "user", "content": "merhaba"}],
        "stream": true
    }).to_string();

    let http_req = format!(
        "POST /v1/chat/completions HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        kervan_addr, req_body.len(), req_body
    );
    stream_socket.write_all(http_req.as_bytes()).await.unwrap();

    let mut reader = BufReader::new(stream_socket);
    let mut basliklar_bitti = false;
    let mut chunks = Vec::new();
    let mut ilk_parca_zamani = None;

    let mut line = String::new();
    while reader.read_line(&mut line).await.unwrap() > 0 {
        if !basliklar_bitti {
            if line == "\r\n" || line == "\n" {
                basliklar_bitti = true;
            }
        } else {
            let trimmed = line.trim();
            if trimmed.starts_with("data:") {
                if ilk_parca_zamani.is_none() && trimmed != "data: [DONE]" {
                    ilk_parca_zamani = Some(t0.elapsed());
                }
                chunks.push(trimmed.to_string());
                if trimmed == "data: [DONE]" {
                    break;
                }
            }
        }
        line.clear();
    }

    let toplam_sure = t0.elapsed();
    let ilk_zaman = ilk_parca_zamani.expect("İlk parça gelmiş olmalı");

    // Acceptance 1: İlk parça yukarı akış sonundan önce gelir (t_ilk < 100ms, toplam >= 140ms)
    assert!(
        ilk_zaman < Duration::from_millis(100),
        "İlk parça gecikmesi {}ms, 100ms altında olmalıydı (yukarı akış bitmeden geldi)",
        ilk_zaman.as_millis()
    );
    assert!(
        toplam_sure >= Duration::from_millis(140),
        "Toplam süre {}ms, 140ms üzerinde olmalıydı",
        toplam_sure.as_millis()
    );

    // Acceptance 2: Parçalar sırasıyla geldi ve nonceleri içeriyor
    assert!(chunks.len() >= 3, "En az 2 parça + [DONE] bekleniyordu: {:?}", chunks);
    assert!(chunks[0].contains(&nonce_a), "İlk parça nonce A içermeli");
    assert!(chunks[1].contains(&nonce_b), "İkinci parça nonce B içermeli");
    assert_eq!(chunks[chunks.len() - 1], "data: [DONE]");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
pub async fn test_p8_claude_sse_cevirisi() {
    let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream_listener.local_addr().unwrap();

    let claude_nonce = format!("CLAUDE_NONCE_{:08x}", rand::random::<u32>());
    let claude_nonce_klon = claude_nonce.clone();

    tokio::spawn(async move {
        let (mut socket, _) = upstream_listener.accept().await.unwrap();
        let mut buf = [0u8; 1024];
        let _ = socket.read(&mut buf).await;

        let baslik = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n";
        socket.write_all(baslik.as_bytes()).await.unwrap();

        let e1 = "event: message_start\r\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_p8\",\"type\":\"message\",\"role\":\"assistant\",\"content\":[],\"model\":\"claude-3-7-sonnet\",\"usage\":{\"input_tokens\":5,\"output_tokens\":0}}}\r\n\r\n";
        let e2 = format!(
            "event: content_block_delta\r\ndata: {{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{{\"type\":\"text_delta\",\"text\":\"{}\"}}}}\r\n\r\n",
            claude_nonce_klon
        );
        let e3 = "event: message_delta\r\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":10}}\r\n\r\n";
        let e4 = "event: message_stop\r\ndata: {\"type\":\"message_stop\"}\r\n\r\n";

        socket.write_all(e1.as_bytes()).await.unwrap();
        socket.write_all(e2.as_bytes()).await.unwrap();
        socket.write_all(e3.as_bytes()).await.unwrap();
        socket.write_all(e4.as_bytes()).await.unwrap();
        socket.flush().await.unwrap();
        let _ = socket.shutdown().await;
        drop(socket);
    });

    unsafe {
        std::env::set_var("TEST_P8_CLAUDE_KEY", "sk-ant-test-p8");
    }

    let mut rest_map = std::collections::HashMap::new();
    rest_map.insert(
        "test-claude-stream".to_string(),
        kervan_yukari::Hedef {
            ad: "test-claude-stream".into(),
            bicim: "claude".into(),
            taban_url: format!("http://{}", upstream_addr),
            anahtar_env: "TEST_P8_CLAUDE_KEY".into(),
            model: "claude-3-7-sonnet".into(),
            yol: Some("/v1/messages".into()),
            zaman_asimi: Duration::from_secs(5),
        },
    );

    let state = kur_test_state(rest_map, kervan_yukari::Yukari::new());
    let router = server::router(state);

    let kervan_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let kervan_addr = kervan_listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(kervan_listener, router).await.unwrap();
    });

    let mut stream_socket = tokio::net::TcpStream::connect(kervan_addr).await.unwrap();
    let req_body = json!({
        "model": "kervan/test-claude-stream",
        "messages": [{"role": "user", "content": "selam"}],
        "stream": true
    }).to_string();

    let http_req = format!(
        "POST /v1/chat/completions HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        kervan_addr, req_body.len(), req_body
    );
    stream_socket.write_all(http_req.as_bytes()).await.unwrap();

    let mut reader = BufReader::new(stream_socket);
    let mut basliklar_bitti = false;
    let mut chunks = Vec::new();

    let mut line = String::new();
    while reader.read_line(&mut line).await.unwrap() > 0 {
        if !basliklar_bitti {
            if line == "\r\n" || line == "\n" {
                basliklar_bitti = true;
            }
        } else {
            let trimmed = line.trim();
            if trimmed.starts_with("data:") {
                chunks.push(trimmed.to_string());
                if trimmed == "data: [DONE]" {
                    break;
                }
            }
        }
        line.clear();
    }

    assert!(!chunks.is_empty(), "Claude akış parçaları alınmalıydı");
    let birlesik = chunks.join("\n");
    assert!(birlesik.contains(&claude_nonce), "Claude nonce'u parçalarda bulunmalı");
    assert!(birlesik.contains("chat.completion.chunk"), "OpenAI parça biçimine dönüştürülmeli");
    assert_eq!(chunks[chunks.len() - 1], "data: [DONE]");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
pub async fn test_p8_gemini_sse_cevirisi() {
    let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream_listener.local_addr().unwrap();

    let gemini_nonce = format!("GEMINI_NONCE_{:08x}", rand::random::<u32>());
    let gemini_nonce_klon = gemini_nonce.clone();

    tokio::spawn(async move {
        let (mut socket, _) = upstream_listener.accept().await.unwrap();
        let mut buf = [0u8; 1024];
        let _ = socket.read(&mut buf).await;

        let baslik = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n";
        socket.write_all(baslik.as_bytes()).await.unwrap();

        let d1 = format!(
            "data: {{\"candidates\":[{{\"content\":{{\"parts\":[{{\"text\":\"{}\"}}]}}}}]}}\n\n",
            gemini_nonce_klon
        );
        let d2 = "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"\"}]},\"finishReason\":\"STOP\"}]}\n\n";

        socket.write_all(d1.as_bytes()).await.unwrap();
        socket.write_all(d2.as_bytes()).await.unwrap();
        socket.flush().await.unwrap();
        let _ = socket.shutdown().await;
    });

    unsafe {
        std::env::set_var("TEST_P8_GEMINI_KEY", "gemini-test-p8");
    }

    let mut rest_map = std::collections::HashMap::new();
    rest_map.insert(
        "test-gemini-stream".to_string(),
        kervan_yukari::Hedef {
            ad: "test-gemini-stream".into(),
            bicim: "gemini".into(),
            taban_url: format!("http://{}", upstream_addr),
            anahtar_env: "TEST_P8_GEMINI_KEY".into(),
            model: "gemini-2.5-flash".into(),
            yol: Some("/v1beta/models/gemini-2.5-flash:streamGenerateContent?alt=sse".into()),
            zaman_asimi: Duration::from_secs(5),
        },
    );

    let state = kur_test_state(rest_map, kervan_yukari::Yukari::new());
    let router = server::router(state);

    let kervan_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let kervan_addr = kervan_listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(kervan_listener, router).await.unwrap();
    });

    let mut stream_socket = tokio::net::TcpStream::connect(kervan_addr).await.unwrap();
    let req_body = json!({
        "model": "kervan/test-gemini-stream",
        "messages": [{"role": "user", "content": "selam"}],
        "stream": true
    }).to_string();

    let http_req = format!(
        "POST /v1/chat/completions HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        kervan_addr, req_body.len(), req_body
    );
    stream_socket.write_all(http_req.as_bytes()).await.unwrap();

    let mut reader = BufReader::new(stream_socket);
    let mut basliklar_bitti = false;
    let mut chunks = Vec::new();

    let mut line = String::new();
    while reader.read_line(&mut line).await.unwrap() > 0 {
        if !basliklar_bitti {
            if line == "\r\n" || line == "\n" {
                basliklar_bitti = true;
            }
        } else {
            let trimmed = line.trim();
            if trimmed.starts_with("data:") {
                chunks.push(trimmed.to_string());
                if trimmed == "data: [DONE]" {
                    break;
                }
            }
        }
        line.clear();
    }

    assert!(!chunks.is_empty(), "Gemini akış parçaları alınmalıydı");
    let birlesik = chunks.join("\n");
    assert!(birlesik.contains(&gemini_nonce), "Gemini nonce'u parçalarda bulunmalı");
    assert!(birlesik.contains("chat.completion.chunk"), "OpenAI parça biçimine dönüştürülmeli");
    assert_eq!(chunks[chunks.len() - 1], "data: [DONE]");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
pub async fn test_p8_openai_arac_cagrisi_akissiz() {
    let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream_listener.local_addr().unwrap();

    let tool_nonce = format!("call_nonce_{:08x}", rand::random::<u32>());
    let tool_nonce_klon = tool_nonce.clone();

    tokio::spawn(async move {
        let (mut socket, _) = upstream_listener.accept().await.unwrap();
        let mut buffer = Vec::new();
        let mut tmp = [0u8; 1024];
        let mut body_start = None;
        let mut content_length = None;

        loop {
            let n = socket.read(&mut tmp).await.unwrap();
            if n == 0 { break; }
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

        let req_text = String::from_utf8_lossy(&buffer);
        assert!(req_text.contains("get_current_weather"), "Yukarı akış tools tanımını kayıpsız almalı");

        let response_body = json!({
            "id": "chatcmpl-tool-test",
            "object": "chat.completion",
            "created": 1700000000,
            "model": "gpt-4o",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": tool_nonce_klon,
                        "type": "function",
                        "function": {
                            "name": "get_current_weather",
                            "arguments": "{\"location\":\"Ankara\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 20,
                "total_tokens": 30
            }
        }).to_string();

        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response_body.len(), response_body
        );
        socket.write_all(resp.as_bytes()).await.unwrap();
        let _ = socket.shutdown().await;
    });

    unsafe {
        std::env::set_var("TEST_P8_TOOL_KEY", "sk-tool-key");
    }

    let mut rest_map = std::collections::HashMap::new();
    rest_map.insert(
        "test-tool-openai".to_string(),
        kervan_yukari::Hedef {
            ad: "test-tool-openai".into(),
            bicim: "openai".into(),
            taban_url: format!("http://{}", upstream_addr),
            anahtar_env: "TEST_P8_TOOL_KEY".into(),
            model: "gpt-4o".into(),
            yol: Some("/v1/chat/completions".into()),
            zaman_asimi: Duration::from_secs(5),
        },
    );

    let state = kur_test_state(rest_map, kervan_yukari::Yukari::new());
    let router = server::router(state);

    let kervan_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let kervan_addr = kervan_listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(kervan_listener, router).await.unwrap();
    });

    let client = ureq::Agent::new_with_config(ureq::config::Config::builder().http_status_as_error(false).build());
    let resp: serde_json::Value = client
        .post(&format!("http://{}/v1/chat/completions", kervan_addr))
        .send_json(&json!({
            "model": "kervan/test-tool-openai",
            "messages": [{"role": "user", "content": "hava nasıl?"}],
            "tools": [{
                "type": "function",
                "function": {
                    "name": "get_current_weather",
                    "description": "Hava durumu",
                    "parameters": {"type": "object", "properties": {"location": {"type": "string"}}}
                }
            }],
            "stream": false
        }))
        .unwrap()
        .body_mut()
        .read_json()
        .unwrap();

    let tool_calls = resp["choices"][0]["message"]["tool_calls"].as_array().expect("tool_calls dizisi olmalı");
    assert_eq!(tool_calls[0]["id"], tool_nonce);
    assert_eq!(tool_calls[0]["function"]["name"], "get_current_weather");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
pub async fn test_p8_openai_arac_cagrisi_akisli() {
    let upstream_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = upstream_listener.local_addr().unwrap();

    let stream_tool_nonce = format!("call_stream_{:08x}", rand::random::<u32>());
    let stream_tool_nonce_klon = stream_tool_nonce.clone();

    tokio::spawn(async move {
        let (mut socket, _) = upstream_listener.accept().await.unwrap();
        let mut buf = [0u8; 1024];
        let _ = socket.read(&mut buf).await;

        let baslik = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n";
        socket.write_all(baslik.as_bytes()).await.unwrap();

        let chunk1 = format!(
            "data: {{\"id\":\"chatcmpl-tstream\",\"object\":\"chat.completion.chunk\",\"choices\":[{{\"index\":0,\"delta\":{{\"role\":\"assistant\",\"tool_calls\":[{{\"index\":0,\"id\":\"{}\",\"type\":\"function\",\"function\":{{\"name\":\"hesap_yap\",\"arguments\":\"\"}}}}]}},\"finish_reason\":null}}]}}\n\n",
            stream_tool_nonce_klon
        );
        let chunk2 = "data: {\"id\":\"chatcmpl-tstream\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"x\\\": 42}\"}}]},\"finish_reason\":null}]}\n\n";
        let chunk3 = "data: {\"id\":\"chatcmpl-tstream\",\"object\":\"chat.completion.chunk\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n";
        let done = "data: [DONE]\n\n";

        socket.write_all(chunk1.as_bytes()).await.unwrap();
        socket.write_all(chunk2.as_bytes()).await.unwrap();
        socket.write_all(chunk3.as_bytes()).await.unwrap();
        socket.write_all(done.as_bytes()).await.unwrap();
        socket.flush().await.unwrap();
        let _ = socket.shutdown().await;
    });

    unsafe {
        std::env::set_var("TEST_P8_STREAM_TOOL_KEY", "sk-stream-tool-key");
    }

    let mut rest_map = std::collections::HashMap::new();
    rest_map.insert(
        "test-stream-tool-openai".to_string(),
        kervan_yukari::Hedef {
            ad: "test-stream-tool-openai".into(),
            bicim: "openai".into(),
            taban_url: format!("http://{}", upstream_addr),
            anahtar_env: "TEST_P8_STREAM_TOOL_KEY".into(),
            model: "gpt-4o".into(),
            yol: Some("/v1/chat/completions".into()),
            zaman_asimi: Duration::from_secs(5),
        },
    );

    let state = kur_test_state(rest_map, kervan_yukari::Yukari::new());
    let router = server::router(state);

    let kervan_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let kervan_addr = kervan_listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(kervan_listener, router).await.unwrap();
    });

    let mut stream_socket = tokio::net::TcpStream::connect(kervan_addr).await.unwrap();
    let req_body = json!({
        "model": "kervan/test-stream-tool-openai",
        "messages": [{"role": "user", "content": "hesapla"}],
        "tools": [{
            "type": "function",
            "function": {"name": "hesap_yap", "parameters": {}}
        }],
        "stream": true
    }).to_string();

    let http_req = format!(
        "POST /v1/chat/completions HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        kervan_addr, req_body.len(), req_body
    );
    stream_socket.write_all(http_req.as_bytes()).await.unwrap();

    let mut reader = BufReader::new(stream_socket);
    let mut basliklar_bitti = false;
    let mut chunks = Vec::new();

    let mut line = String::new();
    while reader.read_line(&mut line).await.unwrap() > 0 {
        if !basliklar_bitti {
            if line == "\r\n" || line == "\n" {
                basliklar_bitti = true;
            }
        } else {
            let trimmed = line.trim();
            if trimmed.starts_with("data:") {
                chunks.push(trimmed.to_string());
                if trimmed == "data: [DONE]" {
                    break;
                }
            }
        }
        line.clear();
    }

    let birlesik = chunks.join("\n");
    assert!(birlesik.contains(&stream_tool_nonce), "Akışlı tool çağrısında nonce bulunmalı");
    assert!(birlesik.contains("hesap_yap"), "Fonksiyon adı akışta bulunmalı");
    assert!(birlesik.contains("42"), "Argüman akışta bulunmalı");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
pub async fn test_p8_claude_akista_arac_acik_hata() {
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
            let _ = socket.shutdown().await;
        }
    });

    unsafe {
        std::env::set_var("TEST_P8_CLAUDE_TOOL_KEY", "sk-claude-tool-key");
    }

    let mut rest_map = std::collections::HashMap::new();
    rest_map.insert(
        "test-claude-tool".to_string(),
        kervan_yukari::Hedef {
            ad: "test-claude-tool".into(),
            bicim: "claude".into(),
            taban_url: format!("http://{}", upstream_addr),
            anahtar_env: "TEST_P8_CLAUDE_TOOL_KEY".into(),
            model: "claude-3-7-sonnet".into(),
            yol: Some("/v1/messages".into()),
            zaman_asimi: Duration::from_secs(5),
        },
    );

    let state = kur_test_state(rest_map, kervan_yukari::Yukari::new());
    let router = server::router(state);

    let kervan_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let kervan_addr = kervan_listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(kervan_listener, router).await.unwrap();
    });

    let client = ureq::Agent::new_with_config(ureq::config::Config::builder().http_status_as_error(false).build());
    let resp = client
        .post(&format!("http://{}/v1/chat/completions", kervan_addr))
        .send_json(&json!({
            "model": "kervan/test-claude-tool",
            "messages": [{"role": "user", "content": "hava nasıl?"}],
            "tools": [{
                "type": "function",
                "function": {"name": "test_fonk"}
            }],
            "stream": true
        }))
        .unwrap();

    // FR-6.4 Acceptance: stream=true iken Claude/Gemini açık hata (501 / 400), yukarı akışa istek gitmez (sayaç 0)
    assert_eq!(resp.status().as_u16(), 501);
    let govde = resp.into_body().read_to_string().unwrap();
    assert!(govde.contains("desteklenmiyor: tools"), "Hata mesajı 'desteklenmiyor: tools' içermeli: {}", govde);
    assert_eq!(istek_sayaci.load(Ordering::SeqCst), 0, "Yukarı akışa hiç istek gitmemeli (sayaç 0)");
}
