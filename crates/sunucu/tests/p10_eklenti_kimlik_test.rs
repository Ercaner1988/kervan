//! `/ws/eklenti` ve `/eklenti/token` kimlik denetimi (S1).
//!
//! Denetim bulgusu: herhangi bir web sayfası WebSocket açıp `kayit` göndererek
//! sağlayıcı taklidi yapabiliyordu. Burada üç kapı da gerçek TCP üzerinden
//! sınanır: Host, Origin ve paylaşılan token.

use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use kervan_sunucu::server::{self, AppState};

fn durum() -> Arc<AppState> {
    let temp_dir = std::env::temp_dir().join(format!("kervan_p10_test_{}", rand::random::<u64>()));
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
        rest_hedefleri: Default::default(),
        yukari: kervan_yukari::Yukari::new(),
        yedek_deneme: true,
        azami_deneme: 3,
        panel: Default::default(),
    })
}

/// Sunucuyu rastgele bir portta başlatır; portu döner.
async fn sunucu_baslat(st: Arc<AppState>) -> u16 {
    let dinleyici = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = dinleyici.local_addr().unwrap().port();
    tokio::spawn(async move {
        let _ = axum::serve(dinleyici, server::router(st)).await;
    });
    port
}

/// Ham HTTP isteği gönderir; durum kodunu ve gövdeyi döner.
async fn iste(port: u16, basliklar: &[(&str, &str)]) -> (u16, String) {
    let mut soket = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let mut istek = String::from("GET /eklenti/token HTTP/1.1\r\n");
    for (ad, deger) in basliklar {
        istek.push_str(&format!("{ad}: {deger}\r\n"));
    }
    istek.push_str("Connection: close\r\n\r\n");
    soket.write_all(istek.as_bytes()).await.unwrap();

    let mut yanit = Vec::new();
    soket.read_to_end(&mut yanit).await.unwrap();
    let metin = String::from_utf8_lossy(&yanit).to_string();
    let kod = metin
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let govde = metin
        .split_once("\r\n\r\n")
        .map(|(_, g)| g.to_string())
        .unwrap_or_default();
    (kod, govde)
}

/// WebSocket yükseltme isteği gönderir; durum kodunu döner.
async fn ws_iste(port: u16, basliklar: &[(&str, &str)]) -> u16 {
    let mut soket = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let mut istek = String::from("GET /ws/eklenti HTTP/1.1\r\n");
    for (ad, deger) in basliklar {
        istek.push_str(&format!("{ad}: {deger}\r\n"));
    }
    istek.push_str(
        "Connection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\n\
         Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n",
    );
    soket.write_all(istek.as_bytes()).await.unwrap();

    let mut tampon = [0u8; 128];
    let n = soket.read(&mut tampon).await.unwrap();
    String::from_utf8_lossy(&tampon[..n])
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

#[tokio::test]
async fn token_ucu_yalniz_uzanti_kaynagina_acilir() {
    let st = durum();
    let token = st.eklenti_hub.token().to_string();
    let port = sunucu_baslat(st).await;
    let host = format!("127.0.0.1:{port}");

    let (kod, govde) = iste(
        port,
        &[("Host", &host), ("Origin", "chrome-extension://abcdef")],
    )
    .await;
    assert_eq!(kod, 200, "uzantı kaynağı token alabilmeli: {govde}");
    assert!(govde.contains(&token), "gövde gerçek token'ı taşımalı");

    let (kod, _) = iste(port, &[("Host", &host), ("Origin", "https://evil.example")]).await;
    assert_eq!(kod, 403, "web sayfası token okuyamamalı");

    let (kod, _) = iste(port, &[("Host", &host)]).await;
    assert_eq!(kod, 403, "Origin'siz istek token okuyamamalı");
}

#[tokio::test]
async fn yabanci_host_reddedilir() {
    let st = durum();
    let port = sunucu_baslat(st).await;

    let (kod, _) = iste(
        port,
        &[
            ("Host", "kervan.example.com"),
            ("Origin", "moz-extension://abcd"),
        ],
    )
    .await;
    assert_eq!(kod, 403, "DNS rebinding adı reddedilmeli");
}

#[tokio::test]
async fn websocket_tokensiz_ve_yabanci_kaynakla_acilmaz() {
    let st = durum();
    let port = sunucu_baslat(st).await;
    let host = format!("127.0.0.1:{port}");

    let kod = ws_iste(
        port,
        &[("Host", &host), ("Origin", "chrome-extension://abcdef")],
    )
    .await;
    assert_eq!(kod, 401, "tokensiz WebSocket 401 almalı");

    let kod = ws_iste(
        port,
        &[
            ("Host", &host),
            ("Origin", "https://evil.example"),
            ("X-Kervan-Token", "yanlis"),
        ],
    )
    .await;
    assert_eq!(kod, 403, "web kaynağı token bilse bile reddedilmeli");
}

#[tokio::test]
async fn websocket_dogru_token_ve_uzanti_kaynagiyla_acilir() {
    let st = durum();
    let token = st.eklenti_hub.token().to_string();
    let port = sunucu_baslat(st).await;
    let host = format!("127.0.0.1:{port}");

    let kod = ws_iste(
        port,
        &[
            ("Host", &host),
            ("Origin", "moz-extension://abcd"),
            ("X-Kervan-Token", &token),
        ],
    )
    .await;
    assert_eq!(kod, 101, "doğru token ile yükseltme kabul edilmeli");
}
