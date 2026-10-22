//! `auto` isteğinin REST adayına düşmesi ve `/yonlendirme` uçlarının
//! gerçek seçim listesini göstermesi. `p5_e2e` yalnız `kervan/<ad>` açık adını
//! sınıyordu; burada akıllı yönlendirme yolu sınanır.

use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use kervan_sunucu::server::{self, AppState};

fn durum(rest_hedefleri: std::collections::HashMap<String, kervan_yukari::Hedef>) -> Arc<AppState> {
    let temp_dir = std::env::temp_dir().join(format!("kervan_p9_test_{}", rand::random::<u64>()));
    let _ = std::fs::create_dir_all(&temp_dir);
    let defter = kervan_saglayici::kota::Defter::ac(temp_dir.join("defter.log")).unwrap();

    let ortam = kervan_havuz::pool::Ortam {
        profiles_root: temp_dir,
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
        panel: Default::default(),
    })
}

/// Sahte OpenAI uyumlu yukarı akış: tek istek alır, sabit yanıt döner.
fn sahte_yukari(
    listener: TcpListener,
    sayac: Arc<AtomicUsize>,
    icerik: &'static str,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        sayac.fetch_add(1, Ordering::SeqCst);

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

        let response_body = json!({
            "id": "chatcmpl-p9",
            "object": "chat.completion",
            "created": 1700000000,
            "model": "mock-model",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": icerik},
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
    })
}

/// Sahte yukarı akış: isteği okur, verilen HTTP durum kodunu döner.
fn sahte_yukari_kod(
    listener: TcpListener,
    sayac: Arc<AtomicUsize>,
    kod: u16,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        sayac.fetch_add(1, Ordering::SeqCst);
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
        let govde = if kod == 200 {
            json!({
                "id": "chatcmpl-p9",
                "object": "chat.completion",
                "created": 1700000000,
                "model": "mock-model",
                "choices": [{
                    "index": 0,
                    "message": {"role": "assistant", "content": "yedek yanit"},
                    "finish_reason": "stop"
                }]
            })
            .to_string()
        } else {
            json!({"error": {"message": "yukari arizasi"}}).to_string()
        };
        let http_response = format!(
            "HTTP/1.1 {kod} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            govde.len(),
            govde
        );
        socket.write_all(http_response.as_bytes()).await.unwrap();
        let _ = socket.shutdown().await;
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
            .timeout_global(Some(Duration::from_secs(5)))
            .build(),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn auto_istegi_rest_adayina_duser() {
    let env_key = "TEST_P9_AUTO_KEY";
    unsafe { std::env::set_var(env_key, "test-key") };

    let up = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let up_addr = up.local_addr().unwrap();
    let sayac = Arc::new(AtomicUsize::new(0));
    drop(sahte_yukari(up, sayac.clone(), "auto yaniti"));

    let mut rest = std::collections::HashMap::new();
    rest.insert(
        "mock_auto".to_string(),
        kervan_yukari::Hedef {
            ad: "mock_auto".into(),
            bicim: "openai".into(),
            taban_url: format!("http://{up_addr}"),
            anahtar_env: env_key.into(),
            model: "mock-model".into(),
            yol: None,
            zaman_asimi: Duration::from_secs(5),
        },
    );
    let addr = sunucu_ac(durum(rest)).await;

    let yanit: Value = tokio::task::spawn_blocking(move || {
        istemci()
            .post(&format!("http://{addr}/v1/chat/completions"))
            .header("Content-Type", "application/json")
            .send_json(json!({
                "model": "auto",
                "messages": [{"role": "user", "content": "merhaba"}],
                // Araç isteği: REST adayı varsa tarayıcı yolu elenir.
                "tools": [{"type": "function", "function": {"name": "f", "parameters": {}}}]
            }))
            .expect("auto istegi kabul edilmeli")
            .body_mut()
            .read_json()
            .expect("gecerli JSON")
    })
    .await
    .unwrap();

    assert_eq!(sayac.load(Ordering::SeqCst), 1, "REST adayi denenmedi");
    assert_eq!(
        yanit["choices"][0]["message"]["content"], "auto yaniti",
        "{yanit}"
    );
}

/// İlk aday 500 dönerse `auto` sıradaki adaya geçmeli. Sıra alfabetiktir
/// (`a_bozuk` önce); LKGP geçmişi boşken kararlı sıralama özgün sırayı korur.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn auto_ilk_aday_bozuksa_yedege_gecer() {
    let env_key = "TEST_P9_FAILOVER_KEY";
    unsafe { std::env::set_var(env_key, "test-key") };

    let bozuk = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let bozuk_addr = bozuk.local_addr().unwrap();
    let saglam = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let saglam_addr = saglam.local_addr().unwrap();
    let bozuk_sayac = Arc::new(AtomicUsize::new(0));
    let saglam_sayac = Arc::new(AtomicUsize::new(0));
    drop(sahte_yukari_kod(bozuk, bozuk_sayac.clone(), 500));
    drop(sahte_yukari_kod(saglam, saglam_sayac.clone(), 200));

    let hedef = |ad: &str, addr: std::net::SocketAddr| kervan_yukari::Hedef {
        ad: ad.into(),
        bicim: "openai".into(),
        taban_url: format!("http://{addr}"),
        anahtar_env: env_key.into(),
        model: "mock-model".into(),
        yol: None,
        zaman_asimi: Duration::from_secs(5),
    };
    let mut rest = std::collections::HashMap::new();
    rest.insert("a_bozuk".to_string(), hedef("a_bozuk", bozuk_addr));
    rest.insert("b_saglam".to_string(), hedef("b_saglam", saglam_addr));
    let addr = sunucu_ac(durum(rest)).await;

    let yanit: Value = tokio::task::spawn_blocking(move || {
        istemci()
            .post(&format!("http://{addr}/v1/chat/completions"))
            .header("Content-Type", "application/json")
            .send_json(json!({
                "model": "auto",
                "messages": [{"role": "user", "content": "merhaba"}]
            }))
            .expect("yedege gecildikten sonra 200 donmeli")
            .body_mut()
            .read_json()
            .unwrap()
    })
    .await
    .unwrap();

    assert_eq!(bozuk_sayac.load(Ordering::SeqCst), 1, "ilk aday denenmedi");
    assert_eq!(saglam_sayac.load(Ordering::SeqCst), 1, "yedege gecilmedi");
    assert_eq!(yanit["choices"][0]["message"]["content"], "yedek yanit");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn yonlendirme_ucu_secim_listesini_gosterir_ve_dahili_reddeder() {
    let mut rest = std::collections::HashMap::new();
    rest.insert(
        "mock_auto".to_string(),
        kervan_yukari::Hedef {
            ad: "mock_auto".into(),
            bicim: "openai".into(),
            taban_url: "http://127.0.0.1:9".into(),
            anahtar_env: "TEST_P9_YOK".into(),
            model: "mock-model".into(),
            yol: None,
            zaman_asimi: Duration::from_secs(1),
        },
    );
    let addr = sunucu_ac(durum(rest)).await;

    let (durum_json, dahili_kod, rr_kod, rr_json) = tokio::task::spawn_blocking(move || {
        let c = istemci();
        let durum_json: Value = c
            .get(&format!("http://{addr}/yonlendirme"))
            .call()
            .unwrap()
            .body_mut()
            .read_json()
            .unwrap();
        let dahili = c
            .post(&format!("http://{addr}/yonlendirme"))
            .header("Content-Type", "application/json")
            .send_json(json!({"strateji": "quota-share"}));
        let dahili_kod = match dahili {
            Ok(r) => r.status().as_u16(),
            Err(ureq::Error::StatusCode(k)) => k,
            Err(e) => panic!("beklenmeyen hata: {e}"),
        };
        let rr = c
            .post(&format!("http://{addr}/yonlendirme"))
            .header("Content-Type", "application/json")
            .send_json(json!({"strateji": "round-robin"}))
            .expect("round-robin kabul edilmeli");
        let rr_kod = rr.status().as_u16();
        let rr_json: Value = rr.into_body().read_json().unwrap();
        (durum_json, dahili_kod, rr_kod, rr_json)
    })
    .await
    .unwrap();

    assert_eq!(durum_json["strateji"], "lkgp");
    assert_eq!(durum_json["adaylar"], json!(["mock_auto"]));
    assert_eq!(dahili_kod, 400, "dahili strateji POST'ta reddedilmeli");
    assert_eq!(rr_kod, 200);
    assert_eq!(rr_json["strateji"], "round-robin");
}
