//! FR-4.4: akışsız Claude Messages ve Gemini generateContent yolu uçtan uca.
//! Sahte yukarı akış her koşuda rastgele bir nonce döner; istek yolu ve kimlik başlığı
//! sahte sunucuda yakalanıp doğrulanır. İstemci yanıtı nonce'u taşımalı.

use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kervan_sunucu::server::{self, AppState};

/// Tek istek kabul eden sahte sunucu; ham istek metnini (başlık + gövde) saklar.
fn sahte_yukari(yanit_govdesi: String) -> (String, Arc<Mutex<Vec<String>>>) {
    let dinleyici = TcpListener::bind("127.0.0.1:0").unwrap();
    let adres = dinleyici.local_addr().unwrap();
    let istekler = Arc::new(Mutex::new(Vec::new()));
    let kayit = istekler.clone();
    std::thread::spawn(move || {
        for akis in dinleyici.incoming() {
            let Ok(mut s) = akis else { break };
            let mut tampon = Vec::new();
            let mut parca = [0u8; 4096];
            while let Ok(n) = s.read(&mut parca) {
                if n == 0 {
                    break;
                }
                tampon.extend_from_slice(&parca[..n]);
                let metin = String::from_utf8_lossy(&tampon);
                if let Some(bas) = metin.find("\r\n\r\n") {
                    let uzunluk = metin[..bas]
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                        })
                        .unwrap_or(0);
                    if tampon.len() >= bas + 4 + uzunluk {
                        break;
                    }
                }
            }
            kayit
                .lock()
                .unwrap()
                .push(String::from_utf8_lossy(&tampon).into_owned());
            let cevap = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                yanit_govdesi.len(),
                yanit_govdesi
            );
            let _ = s.write_all(cevap.as_bytes());
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
    });
    (format!("http://{adres}"), istekler)
}

fn kur_test_state(
    rest_hedefleri: std::collections::HashMap<String, kervan_yukari::Hedef>,
) -> Arc<AppState> {
    let temp_dir = std::env::temp_dir().join(format!("kervan_p5_cg_{}", rand::random::<u64>()));
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
        yukari: kervan_yukari::Yukari::new(),
        yedek_deneme: true,
        azami_deneme: 3,
    })
}

/// Hedefi kurar, Kervan'a akışsız istek atar; (istemci yanıtı, yakalanan ham istekler) döner.
async fn kos(
    bicim: &str,
    yol: &str,
    anahtar_env: &str,
    anahtar: &str,
    yanit: Value,
) -> (Value, Vec<String>) {
    let (taban_url, istekler) = sahte_yukari(yanit.to_string());
    unsafe { std::env::set_var(anahtar_env, anahtar) };
    let ad = format!("p5-{bicim}");
    let mut rest = std::collections::HashMap::new();
    rest.insert(
        ad.clone(),
        kervan_yukari::Hedef {
            ad: ad.clone(),
            bicim: bicim.into(),
            taban_url,
            anahtar_env: anahtar_env.into(),
            model: format!("{bicim}-yukari-modeli"),
            yol: Some(yol.into()),
            zaman_asimi: Duration::from_secs(5),
        },
    );
    let dinleyici = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let adres = dinleyici.local_addr().unwrap();
    let state = kur_test_state(rest);
    tokio::spawn(async move { axum::serve(dinleyici, server::router(state)).await.unwrap() });

    let yanit: Value = tokio::task::spawn_blocking(move || {
        let istemci = ureq::Agent::new_with_config(
            ureq::config::Config::builder()
                .timeout_global(Some(Duration::from_secs(5)))
                .build(),
        );
        istemci
            .post(&format!("http://{adres}/v1/chat/completions"))
            .send_json(json!({
                "model": format!("kervan/{ad}"),
                "messages": [{"role": "user", "content": "nonce nedir?"}]
            }))
            .expect("Kervan isteği kabul etmeli")
            .body_mut()
            .read_json()
            .expect("yanıt JSON olmalı")
    })
    .await
    .unwrap();
    let alinan = istekler.lock().unwrap().clone();
    (yanit, alinan)
}

fn icerik(yanit: &Value) -> &str {
    yanit["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or_default()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn p5_claude_akissiz_nonce_testi() {
    let nonce = format!("nonce-claude-{}", rand::random::<u128>());
    let (yanit, alinan) = kos(
        "claude",
        "/v1/messages",
        "TEST_P5_CLAUDE_KEY",
        "sk-ant-test-claude",
        json!({
            "id": "msg_1", "type": "message", "role": "assistant", "model": "claude-yukari-modeli",
            "content": [{"type": "text", "text": format!("yanıt {nonce}")}],
            "stop_reason": "end_turn", "usage": {"input_tokens": 3, "output_tokens": 5}
        }),
    )
    .await;
    assert_eq!(alinan.len(), 1, "yukarı akış tam bir istek almalı");
    let istek = alinan[0].to_ascii_lowercase();
    assert!(
        istek.starts_with("post /v1/messages "),
        "yol: {}",
        &alinan[0][..60.min(alinan[0].len())]
    );
    assert!(
        istek.contains("x-api-key: sk-ant-test-claude"),
        "x-api-key başlığı yok"
    );
    assert!(
        istek.contains("anthropic-version:"),
        "anthropic-version başlığı yok"
    );
    assert!(
        icerik(&yanit).contains(&nonce),
        "istemci yanıtında nonce yok: {yanit}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn p5_gemini_akissiz_nonce_testi() {
    let nonce = format!("nonce-gemini-{}", rand::random::<u128>());
    let (yanit, alinan) = kos(
        "gemini",
        "/v1beta/models/gemini-yukari-modeli:generateContent",
        "TEST_P5_GEMINI_KEY",
        "AIza-test-gemini",
        json!({
            "candidates": [{"content": {"role": "model", "parts": [{"text": format!("yanıt {nonce}")}]}, "finishReason": "STOP"}],
            "usageMetadata": {"promptTokenCount": 3, "candidatesTokenCount": 5, "totalTokenCount": 8}
        }),
    )
    .await;
    assert_eq!(alinan.len(), 1, "yukarı akış tam bir istek almalı");
    let istek = alinan[0].to_ascii_lowercase();
    assert!(
        istek.starts_with("post /v1beta/models/gemini-yukari-modeli:generatecontent "),
        "yol: {}",
        &alinan[0][..80.min(alinan[0].len())]
    );
    assert!(
        istek.contains("x-goog-api-key: aiza-test-gemini"),
        "x-goog-api-key başlığı yok"
    );
    assert!(
        icerik(&yanit).contains(&nonce),
        "istemci yanıtında nonce yok: {yanit}"
    );
}
