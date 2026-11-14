//! Eklenti yolu taşıyamadığında havuz yedeğine düşülmesi.
//!
//! NEDEN: `EklentiHub::sekme_var_mi` yalnız "bu sağlayıcı için bir KAYIT var"
//! der, kaydın canlı olduğunu söylemez. Zen yeniden açıldığında sunucudaki
//! kayıt kalıyor, eklentinin sekme haritası boş oluyor ve eklenti `null`
//! dönüyordu. O durumda istek tek adayda 500 "Eklenti hatası" ile kilitleniyor,
//! oysa aynı sağlayıcının havuz hesabı kullanılabilirdi (2026-11-14 ölçümü).
//!
//! Burada sınanan sözleşme iki maddelidir:
//!   1. Eklenti taşıyamazsa istek havuz yoluna düşer (500 değil, havuzun
//!      ürettiği 503).
//!   2. Sağlayıcı KOTA bildirdiğinde (429) havuz DENENMEZ: yeniden denemek
//!      kotayı boşa harcar, yanıt istemciye aynen döner.

use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;

use kervan_sunucu::eklenti::{GelenMesaj, GidenMesaj};
use kervan_sunucu::server::{self, AppState};

/// Boş havuzlu, eklenti kaydı sonradan yapılacak durum.
///
/// Havuz BİLEREK boş: yedek yolunun gerçekten denendiği, dönen hatanın havuz
/// kaynaklı olmasıyla (`... icin tanimli hesap yok`) anlaşılır.
fn durum(timeout: Duration) -> Arc<AppState> {
    let temp_dir = std::env::temp_dir().join(format!("kervan_p11_test_{}", rand::random::<u64>()));
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
        timeout,
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
            .timeout_global(Some(Duration::from_secs(10)))
            .build(),
    )
}

fn istek(addr: std::net::SocketAddr) -> Result<Value, u16> {
    match istemci()
        .post(&format!("http://{addr}/v1/chat/completions"))
        .header("Content-Type", "application/json")
        .send_json(json!({
            "model": "kervan/gemini",
            "messages": [{"role": "user", "content": "merhaba"}]
        })) {
        Ok(r) => Ok(r.into_body().read_json().expect("gecerli JSON")),
        Err(ureq::Error::StatusCode(k)) => Err(k),
        Err(e) => panic!("beklenmeyen tasiyici hatasi: {e}"),
    }
}

/// Eklentiye giden istemi yakalar ve sağlayıcı hatası döndürür.
async fn eklenti_hatali(st: Arc<AppState>, mut rx: tokio::sync::mpsc::Receiver<GidenMesaj>) {
    match rx.recv().await {
        Some(GidenMesaj::Sor { id, .. }) => {
            st.eklenti_hub
                .mesaji_isle(GelenMesaj::Hata {
                    id,
                    mesaj: "sahte taşıyıcı hatası: açık bir sekme bulunamadı".into(),
                })
                .await;
        }
        diger => panic!("eklentiye Sor gitmedi: {diger:?}"),
    }
}

/// Eklentiye giden istemi yakalar ve kota metni içeren TAM yanıt döndürür.
async fn eklenti_kota_bildirir(st: Arc<AppState>, mut rx: tokio::sync::mpsc::Receiver<GidenMesaj>) {
    match rx.recv().await {
        Some(GidenMesaj::Sor { id, .. }) => {
            st.eklenti_hub
                .mesaji_isle(GelenMesaj::Parca {
                    id: id.clone(),
                    delta: "Kotanız doldu, yarın deneyin.".into(),
                    done: true,
                })
                .await;
        }
        diger => panic!("eklentiye Sor gitmedi: {diger:?}"),
    }
}

/// 1) Eklenti kaydı VAR ama eklenti taşıyamıyor → havuz yedeği denenir.
///
/// Dönen 500, eski kusurun ta kendisidir; kabul edilen sonuç havuzun
/// ürettiği hatadır (boş havuzda 503).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn eklenti_tasiyamazsa_havuz_yedegine_duser() {
    let st = durum(Duration::from_secs(5));
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    st.eklenti_hub
        .sekme_kaydet("gemini".into(), "varsayilan".into(), tx)
        .await;
    assert!(st.eklenti_hub.sekme_var_mi("gemini").await);

    let hub = st.clone();
    tokio::spawn(async move { eklenti_hatali(hub, rx).await });

    let addr = sunucu_ac(st).await;
    let kod = tokio::task::spawn_blocking(move || istek(addr))
        .await
        .unwrap()
        .expect_err("eklenti tasiyamadiginda 200 beklenmiyor");

    assert_eq!(
        kod, 503,
        "eklenti hatasi havuz yedegine dusmeli; 500 = eski kilitlenme geri geldi"
    );
}

/// 2) Sağlayıcı KOTA bildirdi → 429 aynen döner, havuz DENENMEZ.
///
/// Havuz denenmiş olsaydı boş havuz 503 üretirdi; 429 görmek yedeğin
/// atlandığının kanıtıdır.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn kota_yaniti_yedege_dusmez() {
    let st = durum(Duration::from_secs(5));
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    st.eklenti_hub
        .sekme_kaydet("gemini".into(), "varsayilan".into(), tx)
        .await;

    let hub = st.clone();
    tokio::spawn(async move { eklenti_kota_bildirir(hub, rx).await });

    let addr = sunucu_ac(st).await;
    let kod = tokio::task::spawn_blocking(move || istek(addr))
        .await
        .unwrap()
        .expect_err("kota yanitinda 200 beklenmiyor");

    assert_eq!(
        kod, 429,
        "kota sinyali istemciye 429 donmeli; 503 = havuz yedegi bosuna denendi"
    );
}
