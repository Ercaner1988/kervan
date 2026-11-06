//! OpenAI uyumlu ag gecidi ucu.
//!
//! DURUM GERILIMI (bilinen sinirlama, sessizce gecistirilmiyor):
//! OpenAI `/v1/chat/completions` DURUMSUZ — istemci her istekte tum konusma gecmisini
//! gonderir. Tarayici sohbeti ise DURUMLU — kendi konusma dizisini kendi tutar.
//! 30 bin token'lik gecmisi sohbet kutusuna yazmak pratik degil, hem de anlamsiz:
//! sayfa zaten hatirliyor.
//!
//! v1 karari: **yalnizca son kullanici mesaji** gonderilir, sureklilik tarayicinin
//! kendi konusma dizisinden gelir. Ayni oturum (= ayni hesap profili) boyunca bu
//! dogru davranir. Sinirlama: farkli konusmalar ayni tarayici dizisine karisir.
//! Asama 3'te hesap havuzu gelince her istemci oturumuna ayri sekme/dizi verilecek.
//! Bu bir ADR konusu — plandaki "hesap / profil / oturum" terim ayrimi tam da bunun icin.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::kota::{self, Defter};
use crate::pool::Pool;
use anyhow::Result;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::chat::chat;
use crate::eklenti_uc::{eklenti_sekmeler, jeton, on_denetim, ws_eklenti_handler};
use crate::hesap::{HesapIste, hesap_hedefi};

// İstem kurulumu ayrı sandıkta (sandık sınırı): mesaj tipi ve tohum/iz kurucuları
// burada yeniden dışa aktarılır, böylece `server::Message` yolu değişmez.
pub use kervan_istem::{Message, TOHUM_AZAMI, konusma_izi, son_kullanici_mesaji, tohum_istemi};

// Sohbet gövdesi `chat.rs`e taşındı (sandık sınırı); `server::ChatRequest` yolu
// bench ve dış kullanım için korunur.
pub use crate::chat::ChatRequest;

pub struct AppState {
    pub pool: Pool,
    pub defter: Defter,
    pub kota_ayar: crate::config::Kota,
    pub saglayicilar: std::collections::HashMap<String, std::sync::Arc<crate::browser::Provider>>,
    /// Model istenmediginde kullanilacak saglayici (yapilandirmadaki ilk hesabinki).
    pub varsayilan_saglayici: String,
    pub timeout: std::time::Duration,
    pub eklenti_hub: crate::eklenti::EklentiHub,
    pub compressor: crate::compressor::Compressor,
    pub router: crate::router::SmartRouter,
    pub sayaclar: Sayaclar,
    pub rest_hedefleri: std::collections::HashMap<String, kervan_yukari::Hedef>,
    pub yukari: kervan_yukari::Yukari,
    /// 'auto' isteği seçilen adayda başarısız olursa sıradaki aday denenir mi?
    pub yedek_deneme: bool,
    /// Tek istekte denenecek azami aday sayısı (yedek_deneme açıkken).
    pub azami_deneme: u32,
    /// Panelden hesap ekleme ve yeniden başlatma için gereken ortam.
    pub panel: PanelOrtami,
}

/// Hesap ekleme akışının ihtiyaç duyduğu ortam bilgileri.
///
/// Havuz bunları kendi içinde tutuyor; panelden bir giriş penceresi açmak ve
/// girişi doğrulamak için bir kez daha gerekiyor. `Default` bilinçli: testler
/// ve yalnız sohbet yolunu kuran kurulumlar bu alanı doldurmak zorunda kalmasın.
/// Boş ortamda uçlar 503 döner — sessizce yanlış tarayıcı açmak yok.
#[derive(Clone, Default)]
pub struct PanelOrtami {
    pub profiles_root: std::path::PathBuf,
    pub browser: Option<crate::browser::BrowserBinary>,
    pub geckodriver: std::path::PathBuf,
    pub config_yolu: std::path::PathBuf,
    /// `/kapat` buraya haber verir; `serve` zarifçe kapanır.
    pub kapatma: std::sync::Arc<tokio::sync::Notify>,
}

impl AppState {
    // Biçim sabit: port/mutantlar.toml "rest-kancasini-kaldir" bu satırı arar.
    #[rustfmt::skip]
    pub fn rest_hedefi_coz(&self, model_adi: &str) -> Option<(kervan_yukari::Hedef, Option<String>)> {
        let sade = model_adi.strip_prefix("kervan/").unwrap_or(model_adi);
        let mut parcalar = sade.splitn(2, '/');
        let saglayici_ad = parcalar.next()?;
        let alt_model = parcalar.next().map(|s| s.to_string());

        let hedef = self.rest_hedefleri.get(saglayici_ad)?;
        Some((hedef.clone(), alt_model))
    }
}

/// Durum çubuğunun okuduğu akış sayaçları (süreç ömrü boyunca).
#[derive(Default)]
pub struct Sayaclar {
    /// Yarıda kesilip devam istemiyle dikilen yanıtlar (`stream_recovery`).
    pub kurtarilan_yanit: std::sync::atomic::AtomicU64,
    /// Yanıtlardan ayıklanıp `tool_calls` olarak dönen araç çağrıları (`translator`).
    pub arac_cagrisi: std::sync::atomic::AtomicU64,
}

/// `kervan/<saglayici>` — `/v1/models` bunu listeler, istemci bunu ister.
pub(crate) fn model_kimligi(saglayici: &str) -> String {
    format!("kervan/{saglayici}")
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/v1/models", get(models))
        // Panelin secim listesi: her saglayicinin GERCEKTEN uygulanabilir model
        // kodlari ve ac/kapa anahtarlari. Yalniz yapilandirilmis (olculmus)
        // olanlar listelenir; olmayan secenek gostermek yanlis model riskidir.
        .route("/kervan/secenekler", get(secenekler))
        // OLCUM: bagli her hesabin GERCEK menusunu sayfadan okur. Tahmin yok.
        .route("/kervan/olc", get(crate::olc::olc))
        .route("/v1/chat/completions", post(chat))
        .route("/health", get(|| async { "ok" }))
        .route("/telemetri", get(telemetri_handler))
        // Zcode tarzı dinamik bakiye ve hediye token takibi
        .route("/bakiye", get(bakiye_handler))
        .route("/bakiye/hediye", post(bakiye_hediye_handler))
        // Eklenti WebSocket köprüsü — WebDriver olmadan doğrudan tarayıcı ile konuşur.
        .route("/ws/eklenti", get(ws_eklenti_handler))
        // Token dağıtımı: yalnız uzantı kaynaklı isteklere açılır.
        // Biçim sabit: uc-kaydi tarayıcısı çok satırlı .route çağrısını çözemez.
        .route("/eklenti/token", get(jeton).options(on_denetim))
        .route("/eklenti/sekmeler", get(eklenti_sekmeler))
        // Hesap durumu — TUI gelene kadarki gorunurluk penceresi. Insan kapisinda
        // bekleyen hesap buradan gorulur.
        .route("/accounts", get(accounts))
        // Insan kapisi cozuldukten sonra hesabi geri al. Bu uc olmadan duraklatilan
        // hesap surec omru boyunca olu kalirdi.
        .route("/accounts/{i}/devam", post(devam))
        // Elle duraklat: panelin "Duraklat" dugmesi. Otomatik duraklatmayla
        // ayni `paused_reason` alanini kullanir; `/devam` geri acar.
        .route("/accounts/{i}/duraklat", post(duraklat))
        // Gizli duran tarayiciyi ONE GETIR: model/dusunme duzeyi secmek, bir istemi
        // durdurup duzeltmek ya da dogrulama cozmek icin. Gizli kip headless DEGIL,
        // kucultulmus gercek pencere — headless olsaydi bu uc imkansiz olurdu.
        .route("/accounts/{i}/goster", post(goster))
        // Pencereyi geri gizle VE insanin elinden birak. Birakilmadigi surece
        // bosta supurucusu o tarayiciya dokunmuyor — kullanici giris yaparken
        // pencereyi altindan kapatmasin diye.
        .route("/accounts/{i}/gizle", post(gizle))
        // Kimlik (profil) bazli surumler: elle giris yaparken slot numarasi degil
        // hesap adi dogal olan. `/kimlikler` sirali listeyi verir.
        .route("/kimlikler", get(kimlikler))
        .route("/kimlikler/{k}/goster", post(kimlik_goster))
        .route("/kimlikler/{k}/gizle", post(kimlik_gizle))
        // Kota ogrenicinin gordugu: gercek kullanim ve gozlemden cikan limit.
        .route("/kota", get(kota_ozeti))
        // Akıllı yönlendirici: GET durum ve adayları, POST stratejiyi değiştirir.
        .route("/yonlendirme", get(yonlendirme_durum).post(yonlendirme_sec))
        // Panelden hesap ekleme. Insan kapisinin arayuz tarafi: once otomasyonsuz
        // bir pencere acilir (sifre Kervan'a girmez), giris bitince dogrulanir ve
        // hesap yapilandirmaya yazilir. Yazma tek basina yetmez — havuz acilista
        // kurulur, bu yuzden yeni hesap `/kapat` + yeniden baslatma ile yururluge girer.
        .route("/hesap/ac", post(hesap_ac))
        .route("/hesap/dogrula", post(hesap_dogrula))
        // Kendini kapat: yapilandirma degistikten sonra havuzu tazelemenin yolu.
        .route("/kapat", post(kapat))
        .with_state(state)
}

async fn telemetri_handler(State(st): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let strat = *st.router.strategy.read().await;
    let tasarruf = st.compressor.toplam_tasarruf_orani();
    let bagli = st.eklenti_hub.bagli_sekmeler().await;

    Json(serde_json::json!({
        "aktif_yonlendirme_stratejisi": format!("{strat:?}"),
        "token_tasarruf_orani_yuzde": format!("{:.1}%", tasarruf),
        "bagli_web_sekmeleri": bagli,
        "durum": "saglikli",
    }))
}

/// 'auto' isteklerinin yarıştığı aday listesi: eklenti sekmeleri + havuz
/// hesapları + REST hedefleri. Aynı birleştirme `chat` içinde de yapılır;
/// burada yalnız görünürlük için döndürülür.
///
/// GÖRÜNÜRLÜK düzeyi sağlayıcıdır (panelde hesap hesap listelemek gürültü
/// olurdu). Yönlendirme kararı ise `yonlendirme_hedefleri` ile HESAP düzeyinde
/// verilir.
async fn yonlendirme_adaylari(st: &Arc<AppState>) -> Vec<String> {
    let mut adaylar = st.eklenti_hub.bagli_sekmeler().await;
    for slot in &st.pool.slots {
        if !adaylar.contains(&slot.provider.ad) {
            adaylar.push(slot.provider.ad.clone());
        }
    }
    for rest_ad in st.rest_hedefleri.keys() {
        if !adaylar.contains(rest_ad) {
            adaylar.push(rest_ad.clone());
        }
    }
    adaylar.sort();
    adaylar
}

/// Yönlendirme kararının gerçek adayları: (sağlayıcı, hesap) çiftleri.
///
/// NEDEN hesap düzeyi: kota ve hız bütçesi (kimlik, sağlayıcı) biriminde
/// işliyor (`Slot`). Sağlayıcı adı tek aday olsaydı `least-used` / `lkgp` /
/// kota stratejileri hep aynı hesabı seçer, diğer hesaplar boş dururdu.
///
/// - Havuz hesapları: her `Slot` ayrı hedef (`connection_id` = hesap adı).
/// - Eklenti sekmesi: hesap kimliği `saglayici:profil` biçiminde gelir; profil
///   varsa `connection_id` odur.
/// - REST hedefi: tek bağlantı, `connection_id` = sağlayıcı adı.
///
/// Aynı `execution_key` iki kez üretilmez: `Pool::slots` içinde aynı
/// (sağlayıcı, hesap) çifti zaten tekildir, ama eklenti sekmesi ile havuz aynı
/// profili gösterebildiği için tekilleştirme yapılır.
pub(crate) async fn yonlendirme_hedefleri(st: &Arc<AppState>) -> Vec<crate::router::Hedef> {
    let mut hedefler: Vec<crate::router::Hedef> = Vec::new();
    let mut eklenen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut ekle = |h: crate::router::Hedef, eklenen: &mut std::collections::HashSet<String>| {
        if eklenen.insert(h.execution_key.clone()) {
            hedefler.push(h);
        }
    };

    for slot in &st.pool.slots {
        ekle(
            crate::router::Hedef::hesap(&slot.provider.ad, &slot.hesap_adi),
            &mut eklenen,
        );
    }
    for sekme in st.eklenti_hub.bagli_sekmeler().await {
        // `bagli_sekmeler` sağlayıcı adı döndürüyor; hesap kimliği ayrı uçtan
        // (`bagli_hesaplar`) geliyor ve `auto` yolunda hesap seçimi havuz
        // üzerinden yapılır. Sağlayıcı düzeyinde kalır.
        ekle(crate::router::Hedef::saglayici(&sekme), &mut eklenen);
    }
    for rest_ad in st.rest_hedefleri.keys() {
        ekle(crate::router::Hedef::saglayici(rest_ad), &mut eklenen);
    }

    hedefler.sort_by(|a, b| a.execution_key.cmp(&b.execution_key));
    hedefler
}

/// Kota defterindeki ölçümü stratejilerin okuyacağı anlık görüntüye çevirir.
///
/// Ölçüm BUGÜN istek/olay sayısıdır: `Defter` token tüketimini tutmuyor. Bu
/// yüzden tavanlar da istek tavanıdır (`[kota] tavan_5saat_istek`,
/// `tavan_hafta_istek`); token tavanı sanılıp yazılırsa oran sessizce yanlış olur.
///
/// Tavan tanımlı DEĞİLSE o pencere için anlık görüntü üretilmez: kota duyarlı
/// stratejiler sinyalsiz çalışır ve OmniRoute fail-open davranışıyla pratikte
/// `priority` gibi sıralar. `limit_reached` da üretilmiyor — defterde "son olay
/// kotaydı" bilgisi var ama pencere başına güvenilir bir bayrak yok; olmayan
/// sinyali varmış gibi göstermek yönlendirmeyi yanıltırdı.
pub(crate) async fn kota_anlik_goruntuleri(
    st: &Arc<AppState>,
) -> std::collections::HashMap<String, kervan_havuz::kota_duyarli::KotaSnapshot> {
    let mut harita = std::collections::HashMap::new();
    let t5 = st.kota_ayar.tavan_5saat_istek;
    let th = st.kota_ayar.tavan_hafta_istek;
    if t5.is_none() && th.is_none() && st.kota_ayar.saglayici_tavan.is_empty() {
        return harita;
    }
    let oran =
        |sayi: usize, tavan: Option<u64>| tavan.map(|t| (sayi as f64 / t as f64).clamp(0.0, 1.0));
    // (hesap, sağlayıcı, 5s tavanı, hafta tavanı) üçlüsünden anlık görüntü kurar.
    // Tarayıcı hesabı = hesap başına tavan; REST = sağlayıcı başına tavan.
    macro_rules! goruntu {
        ($harita:expr, $hesap:expr, $saglayici:expr, $t5:expr, $th:expr) => {{
            let bes = match $t5 {
                Some(_) => {
                    let n = st
                        .defter
                        .son_izler($hesap, $saglayici, 5 * 3600)
                        .await
                        .len();
                    oran(n, $t5)
                }
                None => None,
            };
            let hafta = match $th {
                Some(_) => {
                    let n = st
                        .defter
                        .son_izler($hesap, $saglayici, 7 * 24 * 3600)
                        .await
                        .len();
                    oran(n, $th)
                }
                None => None,
            };
            if bes.is_some() || hafta.is_some() {
                $harita.insert(
                    crate::router::hedef_anahtari($saglayici, Some($hesap)),
                    kervan_havuz::kota_duyarli::KotaSnapshot {
                        oturum: bes.map(|p| kervan_havuz::kota_duyarli::KotaPenceresi {
                            percent_used: Some(p),
                            reset_at: None,
                        }),
                        haftalik: hafta.map(|p| kervan_havuz::kota_duyarli::KotaPenceresi {
                            percent_used: Some(p),
                            reset_at: None,
                        }),
                        ..Default::default()
                    },
                );
            }
        }};
    }
    for slot in &st.pool.slots {
        goruntu!(harita, &slot.hesap_adi, &slot.provider.ad, t5, th);
    }
    // REST hedefi: hesap kimliği yok, defter anahtarı sağlayıcı adının kendisi.
    // Sıralamadaki `execution_key` de yalın sağlayıcı adıdır (Hedef::saglayici),
    // bu yüzden aynı haritaya çakışmadan girer.
    for (ad, t) in &st.kota_ayar.saglayici_tavan {
        goruntu!(harita, ad, ad, t.tavan_5saat_istek, t.tavan_hafta_istek);
    }
    harita
}

async fn yonlendirme_durum(State(st): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let strat = *st.router.strategy.read().await;
    let adaylar = yonlendirme_adaylari(&st).await;
    Json(serde_json::json!({
        "strateji": strat.as_str(),
        "desteklenir": strat.desteklenir(),
        "adaylar": adaylar,
        "kullanilabilir": crate::router::RoutingStrategy::KAMUYA_ACIK,
    }))
}

#[derive(Deserialize)]
struct YonlendirmeSec {
    strateji: String,
}

/// Stratejiyi çalışma anında değiştirir. Yalnız tam kanonik ad kabul edilir:
/// `normallestir` bilinmeyen adı sessizce priority'ye düşürür, bir kontrol
/// ucunda sessiz düşüş istenmez.
async fn yonlendirme_sec(
    State(st): State<Arc<AppState>>,
    Json(req): Json<YonlendirmeSec>,
) -> Response {
    match crate::router::RoutingStrategy::from_str_canonical(&req.strateji) {
        Some(s) if s.is_internal() => hata(
            StatusCode::BAD_REQUEST,
            &format!(
                "'{}' dahili bir stratejidir; kullanıcıya açık değil",
                s.as_str()
            ),
        ),
        Some(s) => {
            *st.router.strategy.write().await = s;
            println!(
                "akilli-rota   : strateji çalışma anında değiştirildi -> [{}]",
                s.as_str()
            );
            Json(serde_json::json!({
                "ok": true,
                "strateji": s.as_str(),
                "desteklenir": s.desteklenir(),
            }))
            .into_response()
        }
        None => hata(
            StatusCode::BAD_REQUEST,
            &format!(
                "'{}' bilinmeyen strateji. Kullanilabilir: {}",
                req.strateji,
                crate::router::RoutingStrategy::KAMUYA_ACIK.join(", ")
            ),
        ),
    }
}

#[derive(Deserialize)]
struct HediyeIstek {
    hesap: String,
    saglayici: String,
    miktar: u64,
}

async fn bakiye_handler(State(st): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let bakiyeler = st.defter.tum_bakiyeler().await;
    Json(serde_json::json!({
        "hesaplar": bakiyeler,
        "not": "Zcode tarzi dinamik bakiye, haftasonu hediye ve giris takip verileri",
    }))
}

async fn bakiye_hediye_handler(
    State(st): State<Arc<AppState>>,
    Json(req): Json<HediyeIstek>,
) -> Response {
    match st.defter.hediye_ekle(&req.hesap, &req.saglayici, req.miktar).await {
        Ok(()) => Json(serde_json::json!({
            "ok": true,
            "mesaj": format!("{}+{} icin {} hediye token eklendi", req.hesap, req.saglayici, req.miktar),
        })).into_response(),
        Err(e) => hata(StatusCode::INTERNAL_SERVER_ERROR, &format!("{e:#}")),
    }
}

async fn kota_ozeti(State(st): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let mut liste = Vec::new();
    for slot in st.pool.slots.iter() {
        let o = st.defter.ozet(&slot.hesap_adi, &slot.provider.ad).await;
        // Saglayici adi ZORUNLU: ozet artik hesap+saglayici cifti icin cikiyor, yani
        // ayni hesabin dort satiri dort ayri sayi tasiyor. Adi basmazsak cikti
        // ayirt edilemez dort ayni satir gibi gorunur.
        liste.push(serde_json::json!({
            "hesap": slot.hesap_adi,
            "saglayici": slot.provider.ad,
            "kullanim": o,
        }));
    }
    // REST hedefleri ayrı liste: hesap kimlikleri yok, defter anahtarı sağlayıcının
    // kendisi. Ücretsiz katman (OpenRouter vb.) 429'ları burada öğrenilir.
    let mut rest_liste = Vec::new();
    for ad in st.rest_hedefleri.keys() {
        let o = st.defter.ozet(ad, ad).await;
        rest_liste.push(serde_json::json!({
            "saglayici": ad,
            "kullanim": o,
            "tavan": st.kota_ayar.saglayici_tavan.get(ad),
        }));
    }
    Json(serde_json::json!({
        "defter": kota::yol_goster(&st.defter).display().to_string(),
        "hesaplar": liste,
        "rest": rest_liste,
        "not": "tahmin yalnizca gozlenmis kota carpmalarindan cikar; kanit 0 ise tahmin yok",
    }))
}

async fn accounts(State(st): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let mut liste = Vec::new();
    for (i, slot) in st.pool.slots.iter().enumerate() {
        let s = slot.state.lock().await;
        liste.push(serde_json::json!({
            "index": i,
            "name": slot.hesap_adi,
            // Ayni kimlik numarasini paylasan yuvalar TEK tarayiciyi paylasir;
            // sekmeleri farklidir.
            "kimlik": st.pool.kimlikler[slot.kimlik].ad,
            "provider": slot.provider.ad,
            "model": model_kimligi(&slot.provider.ad),
            "istek_sayisi": s.total,
            "son_saatte": s.recent.len(),
            "saatlik_tavan": st.pool.pacing.hourly_cap,
            "ust_uste_hata": s.consecutive_errors,
            "duraklatildi": s.paused_reason,
        }));
    }
    // Panelin hesap ekleme secicisi tanimli saglayicilarin listesine ihtiyac
    // duyar; havuzdan turetilemez cunku henuz hesabi olmayan saglayici da
    // secilebilir olmali. Bu yuzden tanim defterinin adlari burada verilir.
    let mut saglayicilar: Vec<&String> = st.saglayicilar.keys().collect();
    saglayicilar.sort();
    Json(serde_json::json!({
        "accounts": liste,
        "sabit_oturumlar": st.pool.sabit_sayisi().await,
        "saglayicilar": saglayicilar,
    }))
}

async fn devam(State(st): State<Arc<AppState>>, Path(i): Path<usize>) -> Response {
    if i >= st.pool.slots.len() {
        return hata(StatusCode::NOT_FOUND, &format!("{i} numarali hesap yok"));
    }
    st.pool.devam_ettir(i).await;
    Json(serde_json::json!({ "ok": true, "name": st.pool.slots[i].hesap_adi })).into_response()
}

/// Panelin "Duraklat" dugmesi. Neden metni sabittir: kullanici kaynakli
/// oldugu `/accounts` ciktisinda gorunur, boylece otomatik duraklatmadan
/// ayirt edilebilir.
async fn duraklat(State(st): State<Arc<AppState>>, Path(i): Path<usize>) -> Response {
    if i >= st.pool.slots.len() {
        return hata(StatusCode::NOT_FOUND, &format!("{i} numarali hesap yok"));
    }
    st.pool.duraklat(i, "elle duraklatildi").await;
    Json(serde_json::json!({ "ok": true, "name": st.pool.slots[i].hesap_adi })).into_response()
}

/// Arayüzün "Göster" düğmesi: `i` numaralı hesabın penceresini öne getirir.
pub async fn hesabi_goster(st: Arc<AppState>, i: usize) -> bool {
    goster(State(st), Path(i)).await.status().is_success()
}

async fn goster(State(st): State<Arc<AppState>>, Path(i): Path<usize>) -> Response {
    let Some(slot) = st.pool.slots.get(i) else {
        return hata(StatusCode::NOT_FOUND, &format!("{i} numarali hesap yok"));
    };
    kimlik_goster(
        State(st.clone()),
        Path(slot.kimlik),
        Query(GosterSorgu::default()),
    )
    .await
}

async fn gizle(State(st): State<Arc<AppState>>, Path(i): Path<usize>) -> Response {
    let Some(slot) = st.pool.slots.get(i) else {
        return hata(StatusCode::NOT_FOUND, &format!("{i} numarali hesap yok"));
    };
    kimlik_gizle(State(st.clone()), Path(slot.kimlik)).await
}

/// Kimlik listesi: sirasi `/kimlikler/{k}/goster` icin gecerli.
async fn kimlikler(State(st): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let liste: Vec<_> = st
        .pool
        .kimlikler
        .iter()
        .enumerate()
        .map(|(k, kim)| {
            let saglayicilar: Vec<&str> = st
                .pool
                .slots
                .iter()
                .filter(|s| s.kimlik == k)
                .map(|s| s.provider.ad.as_str())
                .collect();
            serde_json::json!({ "k": k, "hesap": kim.ad, "saglayicilar": saglayicilar })
        })
        .collect();
    Json(serde_json::json!({ "kimlikler": liste }))
}

#[derive(Deserialize, Default)]
pub struct GosterSorgu {
    /// Virgulle ayrilmis saglayici adlari; sekme olarak acilir.
    #[serde(default)]
    pub sekmeler: Option<String>,
}

async fn kimlik_goster(
    State(st): State<Arc<AppState>>,
    Path(k): Path<usize>,
    Query(q): Query<GosterSorgu>,
) -> Response {
    let Some(kim) = st.pool.kimlikler.get(k) else {
        return hata(StatusCode::NOT_FOUND, &format!("{k} numarali kimlik yok"));
    };
    // Bilinmeyen saglayici adi SESSIZCE atlanmaz: yanlis yazilmis bir ad,
    // "sekmeyi actim" deyip acmamak olurdu.
    let mut acilacak = Vec::new();
    for ad in q
        .sekmeler
        .as_deref()
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        match crate::saglayicilar::bul(&st.saglayicilar, ad) {
            Ok(s) => acilacak.push(s),
            Err(e) => return hata(StatusCode::BAD_REQUEST, &format!("{e:#}")),
        }
    }
    let adlar: Vec<&str> = acilacak.iter().map(|s| s.ad.as_str()).collect();
    match st.pool.goster(k, &acilacak).await {
        Ok(()) => Json(serde_json::json!({
            "ok": true,
            "hesap": kim.ad,
            "acilan_sekmeler": adlar,
            "not": "pencere insanin elinde; bosta supurucusu dokunmayacak. \
                    Bitince: POST /kimlikler/{k}/gizle",
        }))
        .into_response(),
        Err(e) => hata(StatusCode::INTERNAL_SERVER_ERROR, &format!("{e:#}")),
    }
}

async fn kimlik_gizle(State(st): State<Arc<AppState>>, Path(k): Path<usize>) -> Response {
    let Some(kim) = st.pool.kimlikler.get(k) else {
        return hata(StatusCode::NOT_FOUND, &format!("{k} numarali kimlik yok"));
    };
    match st.pool.gizle(k).await {
        Ok(acikti) => Json(serde_json::json!({ "ok": true, "hesap": kim.ad, "acikti": acikti }))
            .into_response(),
        Err(e) => hata(StatusCode::INTERNAL_SERVER_ERROR, &format!("{e:#}")),
    }
}

#[derive(Serialize)]
struct ModelList {
    object: &'static str,
    data: Vec<ModelInfo>,
}

#[derive(Serialize)]
struct ModelInfo {
    id: String,
    object: &'static str,
    created: u64,
    owned_by: &'static str,
    /// Menude var ama bu hesapta secilemiyor (ucretli plan). Alan her zaman
    /// gonderilir: istemci `false` gorurse "secilebilir" demektir.
    kilitli: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    kilit_nedeni: Option<String>,
}

// ─── Uclar ───────────────────────────────────────────────────────────────────

/// Panelin seçim listesi: sağlayıcı başına gerçekten uygulanabilir seçenekler.
///
/// Yalnız `kervan.toml`da ÖLÇÜLMÜŞ olarak tanımlananlar listelenir: model kodları
/// ve aç/kapa anahtarları. Tanımsız bir seçeneği burada göstermek, panelde
/// "seçilebilir" izlenimi yaratıp isteği 400'e düşürürdü.
///
/// ÖLÇÜM DEFTERİ AYRICA VERİLİR. `kervan.toml` listesi sağlayıcı GENELİ bir
/// varsayımdır; site menüsü hesaba göre değişiyor (bir hesapta yalnız "Otomatik",
/// başka hesapta dört model). Defterde o sağlayıcı için ölçüm varsa:
///   - `kaynak = "yapilandirma"` ile listenin varsayım olduğu AÇIKÇA yazılır,
///   - `hesaplar` altında her hesabın ÖLÇÜLMÜŞ kataloğu ve özellikleri verilir.
///
/// Böylece panel "kervan.toml ne diyorsa o" ile "site ne gösteriyor" farkını
/// görebilir; ikisini tek listeye karıştırmak sessiz yanlış model riskidir.
async fn secenekler(State(st): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let defter = crate::olcum::yukle();
    let mut saglayicilar = serde_json::Map::new();
    for (ad, s) in &st.saglayicilar {
        let olculen: Vec<crate::olcum::HesapOlcumu> = defter
            .iter()
            .filter(|o| &o.saglayici == ad)
            .cloned()
            .collect();
        let kayit = kervan_secim::secenek_kaydi(s.model.as_ref(), &s.anahtarlar);
        if let Some(v) = crate::olcum::panel_kaydi(kayit, &olculen) {
            saglayicilar.insert(ad.clone(), v);
        }
    }
    Json(serde_json::json!({ "saglayicilar": saglayicilar }))
}

async fn models(State(st): State<Arc<AppState>>) -> Json<ModelList> {
    // Hem havuzdaki hesaplar hem de canli bagli eklenti sekmeleri ve REST sağlayıcıları listelenir.
    let mut ad_seti: std::collections::BTreeSet<String> = st
        .pool
        .slots
        .iter()
        .map(|s| s.provider.ad.clone())
        .collect();
    for ext_s in st.eklenti_hub.bagli_sekmeler().await {
        ad_seti.insert(ext_s);
    }
    for rest_ad in st.rest_hedefleri.keys() {
        ad_seti.insert(rest_ad.clone());
    }
    let mut adlar: Vec<String> = ad_seti.into_iter().collect();
    adlar.sort();

    // Her saglayici icin: `kervan/<ad>` (site varsayilani) + OLCULMUS ve
    // yapilandirilmis modeller (`kervan/<ad>/<kod>`). Kilitli secenekler de
    // listelenir ama `kilitli: true` ile: hesabin potansiyeli gorunur, istemci
    // secerse 402 alir — sessizce varsayilana dusmez.
    // Ölçüm defteri: hesap bazlı gerçek. Ölçüm varsa o hesabın kataloğu
    // `kervan/<sag>/<kod>@<profil>` kimliğiyle listelenir; yoksa sağlayıcı
    // geneli yapılandırma kullanılır. İki kaynağı karıştırmıyoruz: hangi
    // girdinin ölçüm hangisinin varsayım olduğu böylece görünür kalır.
    let defter = crate::olcum::yukle();
    let bagli = st.eklenti_hub.bagli_hesaplar().await;
    let sekmeler = st.eklenti_hub.bagli_sekmeler().await;

    let mut veri: Vec<ModelInfo> = Vec::new();
    for ad in adlar {
        veri.push(ModelInfo {
            id: model_kimligi(&ad),
            object: "model",
            created: simdi(),
            owned_by: "kervan",
            kilitli: false,
            kilit_nedeni: None,
        });
        let olculen: Vec<&crate::olcum::HesapOlcumu> = defter
            .iter()
            .filter(|o| o.saglayici == ad && sekmeler.contains(&o.saglayici))
            .collect();
        if !olculen.is_empty() {
            for o in olculen {
                // Hesap varsayılanı: model seçmeden o hesabı kullan.
                veri.push(ModelInfo {
                    id: format!("{}@{}", model_kimligi(&ad), o.hesap),
                    object: "model",
                    created: simdi(),
                    owned_by: "kervan",
                    kilitli: false,
                    kilit_nedeni: None,
                });
                for m in &o.modeller {
                    veri.push(ModelInfo {
                        id: format!("{}/{}@{}", model_kimligi(&ad), m.kod, o.hesap),
                        object: "model",
                        created: simdi(),
                        owned_by: "kervan",
                        kilitli: !m.secilebilir,
                        kilit_nedeni: (!m.secilebilir)
                            .then(|| "bu hesapta seçilemedi (ölçüldü)".to_string()),
                    });
                }
            }
            continue;
        }
        // Ölçüm yok: sağlayıcı geneli yapılandırma. Bağlı hesap varsa hesap
        // ekiyle de sunulur; kilit durumu yapılandırmadan gelir ve bu bir
        // VARSAYIMDIR (ölçüm değil) — panel bunu ölçümle güncelleyebilir.
        let hesaplar: Vec<&str> = bagli
            .iter()
            .filter_map(|k| k.split_once(':'))
            .filter(|(s, _)| *s == ad)
            .map(|(_, p)| p)
            .collect();
        for h in hesaplar {
            veri.push(ModelInfo {
                id: format!("{}@{}", model_kimligi(&ad), h),
                object: "model",
                created: simdi(),
                owned_by: "kervan",
                kilitli: false,
                kilit_nedeni: None,
            });
        }
        if let Some(s) = st.saglayicilar.get(&ad)
            && let Some(menu) = &s.model
        {
            for m in &menu.modeller {
                veri.push(ModelInfo {
                    id: format!("{}/{}", model_kimligi(&ad), m.kod),
                    object: "model",
                    created: simdi(),
                    owned_by: "kervan",
                    kilitli: m.kilitli,
                    kilit_nedeni: m.kilit_nedeni.clone(),
                });
            }
        }
    }

    // HESAP BAZLI kimlikler yukarıdaki döngüde üretilir:
    // `kervan/<sag>/<kod>@<profil>`. Ölçüm YOKSA hiç üretilmez — ölçülmemiş
    // bir hesap için "seçilebilir" iddiası yazılmaz.
    //
    // DİKKAT: bu blok bir kez daha eklenirse her hesap modeli İKİ KEZ listelenir
    // (2026-10-25'te yaşandı: 72 model, 36 olması gerekirken). Katalog tek
    // kaynaktan, tek döngüde üretilir.
    Json(ModelList {
        object: "list",
        data: veri,
    })
}

pub(crate) fn hata(kod: StatusCode, mesaj: &str) -> Response {
    (
        kod,
        Json(serde_json::json!({ "error": { "message": mesaj } })),
    )
        .into_response()
}
pub(crate) fn simdi() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Otomasyon disinda bir giris penceresi acar ve hemen doner.
///
/// NEDEN AYRI UC: giris gercek bir insan tarafindan gercek bir tarayicida
/// yapilmali (Google, WebDriver imzali tarayicida oturum acmayi engelliyor).
/// Bu yuzden istek pencereyi acip doner; dogrulama ayri cagrilir.
async fn hesap_ac(State(st): State<Arc<AppState>>, Json(iste): Json<HesapIste>) -> Response {
    let (profil, saglayici) = match hesap_hedefi(&st, &iste) {
        Ok(v) => v,
        Err(yanit) => return yanit,
    };
    let url = saglayici.giris_url(iste.eposta.trim());
    let root = st.panel.profiles_root.clone();
    let Some(binary) = st.panel.browser.clone() else {
        return hata(
            StatusCode::SERVICE_UNAVAILABLE,
            "panel ortami yapilandirilmamis: tarayici ikilisi bilinmiyor",
        );
    };
    let p = profil.clone();
    let u = url.clone();
    let sonuc = tokio::task::spawn_blocking(move || {
        crate::browser::elle_giris(&root, &p, &binary, &u).map(|_| ())
    })
    .await;
    match sonuc {
        Ok(Ok(())) => Json(serde_json::json!({
            "ok": true,
            "profil": profil,
            "url": url,
            "mesaj": "Giris penceresi acildi. Girisi tamamlayip pencereyi KAPAT, sonra \"Girisi dogrula\"ya bas.",
        }))
        .into_response(),
        Ok(Err(e)) => hata(
            StatusCode::BAD_GATEWAY,
            &format!("giris penceresi acilamadi: {e:#}"),
        ),
        Err(e) => hata(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("giris gorevi calistirilamadi: {e}"),
        ),
    }
}

/// Acilan profilde giris gercekten tamamlanmis mi? Tamamlandiysa hesabi
/// yapilandirmaya yazar. Havuz acilista kuruldugu icin yanit `yeniden_baslat`
/// isaretini de tasir — yalan soylemektense gerekli adimi acikca bildiririz.
async fn hesap_dogrula(State(st): State<Arc<AppState>>, Json(iste): Json<HesapIste>) -> Response {
    let (profil, saglayici) = match hesap_hedefi(&st, &iste) {
        Ok(v) => v,
        Err(yanit) => return yanit,
    };
    let Some(binary) = st.panel.browser.clone() else {
        return hata(
            StatusCode::SERVICE_UNAVAILABLE,
            "panel ortami yapilandirilmamis: tarayici ikilisi bilinmiyor",
        );
    };
    let oturum = match crate::browser::Session::open(
        &st.panel.profiles_root,
        &profil,
        &binary,
        &st.panel.geckodriver,
        true,
    )
    .await
    {
        Ok(o) => o,
        Err(e) => return hata(StatusCode::BAD_GATEWAY, &format!("profil acilamadi: {e:#}")),
    };
    let durum = async {
        oturum.goto(&saglayici.url).await?;
        crate::browser::detect_login(&oturum.client, &saglayici.selectors).await
    }
    .await;
    if let Err(h) = oturum.close().await {
        eprintln!("uyari: giris oturumu kapatilamadi: {h:#}");
    }

    match durum {
        Ok(crate::browser::LoginState::Ready) => {
            match crate::kurulum::hesabi_yapilandirmaya_ekle(
                &st.panel.config_yolu,
                &saglayici.ad,
                &profil,
            ) {
                Ok(true) => Json(serde_json::json!({
                    "ok": true,
                    "giris": true,
                    "profil": profil,
                    "eklendi": true,
                    "yeniden_baslat": true,
                    "mesaj": "Giris dogrulandi ve hesap kervan.toml'a eklendi. Havuz acilista kuruldugu icin Kervan yeniden baslatilmali.",
                }))
                .into_response(),
                Ok(false) => Json(serde_json::json!({
                    "ok": true,
                    "giris": true,
                    "profil": profil,
                    "eklendi": false,
                    "yeniden_baslat": false,
                    "mesaj": "Giris dogrulandi; hesap zaten kervan.toml'da kayitli.",
                }))
                .into_response(),
                Err(e) => hata(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("hesap yapilandirmaya yazilamadi: {e:#}"),
                ),
            }
        }
        Ok(crate::browser::LoginState::NeedsLogin) => Json(serde_json::json!({
            "ok": true,
            "giris": false,
            "profil": profil,
            "mesaj": "Giris tamamlanmamis. Pencereyi acip girisi bitir, sonra tekrar dogrula.",
        }))
        .into_response(),
        Ok(crate::browser::LoginState::Unknown) => Json(serde_json::json!({
            "ok": true,
            "giris": false,
            "belirsiz": true,
            "profil": profil,
            "mesaj": "Ne istem kutusu ne giris baglantisi gorundu (dogrulama duvari ya da degisen arayuz olabilir). Pencereyi acip sayfayi kontrol et.",
        }))
        .into_response(),
        Err(e) => hata(
            StatusCode::BAD_GATEWAY,
            &format!("giris durumu okunamadi: {e:#}"),
        ),
    }
}

/// Zarif kapanis. Yapilandirma degistiginde havuzu tazelemenin en kucuk yolu:
/// Kervan kendi kendine kapanir, panel onu yeniden baslatir.
async fn kapat(State(st): State<Arc<AppState>>) -> Json<serde_json::Value> {
    st.panel.kapatma.notify_waiters();
    Json(serde_json::json!({ "ok": true, "mesaj": "Kervan kapaniyor." }))
}

pub async fn serve(state: Arc<AppState>, port: u16) -> Result<()> {
    // 127.0.0.1 SART: 0.0.0.0 tum ag gecidini ve arkasindaki oturum acilmis
    // hesaplari yerel aga acardi. Plandaki guvenlik tabani, madde 3.
    let adres = format!("127.0.0.1:{port}");
    let dinleyici = tokio::net::TcpListener::bind(&adres).await?;
    println!("Kervan dinlemede: http://{adres}/v1");
    let kapatma = state.panel.kapatma.clone();
    axum::serve(dinleyici, router(state))
        .with_graceful_shutdown(async move { kapatma.notified().await })
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::{arac_cagrilarini_ayikla, arac_yonergesi};
    use crate::hesap::eposta_gecerli_mi;
    use std::time::Duration;

    /// Yazim hatasi filtresi: '@' tek ve iki yaninda icerik olmali, alan adi
    /// nokta tasimali. Gecerli bir eposta sessizce bos profil acmamali.
    #[test]
    fn eposta_filtresi_yazim_hatasini_yakalar() {
        for iyi in ["a@b.co", "ercan.er@medeniyet.edu.tr", "x+y@z.io"] {
            assert!(eposta_gecerli_mi(iyi), "{iyi} kabul edilmeliydi");
        }
        for kotu in [
            "", "ercan", "@b.co", "ercan@", "ercan@b", "a b@c.co", "a@b.",
        ] {
            assert!(!eposta_gecerli_mi(kotu), "{kotu} reddedilmeliydi");
        }
    }

    fn m(role: &str, content: &str) -> Message {
        Message {
            role: role.into(),
            content: content.into(),
            ..Default::default()
        }
    }

    fn hava_araci() -> Vec<serde_json::Value> {
        vec![serde_json::json!({"type": "function", "function": {
            "name": "hava", "description": "Hava durumu",
            "parameters": {"type": "object", "properties": {"il": {"type": "string"}}}}})]
    }

    /// Yalnız istemcinin tanımladığı araç `tool_calls` olur; blok metinden çıkar.
    #[test]
    fn tanimli_arac_tool_calls_olur() {
        let metin =
            "Bakayım.\n```json\n{\"name\": \"hava\", \"arguments\": {\"il\": \"İzmir\"}}\n```"
                .to_string();
        let (icerik, cagrilar) = arac_cagrilarini_ayikla(&hava_araci(), metin);
        assert_eq!(cagrilar.len(), 1);
        assert_eq!(cagrilar[0]["function"]["name"], "hava");
        assert_eq!(cagrilar[0]["function"]["arguments"], r#"{"il":"İzmir"}"#);
        assert_eq!(icerik, "Bakayım.");
    }

    /// Tanımsız araç adı ya da araç tanımı yoksa metin hiç değişmez.
    #[test]
    fn tanimsiz_arac_metne_dokunmaz() {
        let metin = "```json\n{\"name\": \"sil\", \"arguments\": {}}\n```".to_string();
        assert!(
            arac_cagrilarini_ayikla(&hava_araci(), metin.clone())
                .1
                .is_empty()
        );
        assert_eq!(arac_cagrilarini_ayikla(&[], metin.clone()), (metin, vec![]));
        assert_eq!(arac_yonergesi(&[]), "");
        assert!(arac_yonergesi(&hava_araci()).contains("- hava: Hava durumu"));
    }

    /// Araç turunda modele son kullanıcı sorusu değil araç sonuçları gider.
    #[test]
    fn arac_sonucu_turu_geri_bildirim_gonderir() {
        let mut sonuc = m("tool", "22°C güneşli");
        sonuc.name = Some("hava".into());
        let msgs = vec![m("user", "İzmir'de hava?"), m("assistant", ""), sonuc];
        let istem = son_kullanici_mesaji(&msgs).unwrap();
        assert!(
            istem.contains("22°C güneşli") && istem.contains("hava"),
            "{istem}"
        );
        assert!(!istem.contains("İzmir'de hava?"));
    }

    /// OpenAI araç çağrılı asistan mesajında `content: null` gönderir; reddedilmemeli.
    #[test]
    fn null_icerik_kabul_edilir() {
        let m: Message =
            serde_json::from_str(r#"{"role":"assistant","content":null,"tool_calls":[]}"#).unwrap();
        assert_eq!(m.content, "");
    }

    /// Gecmisin tamami degil SON kullanici mesaji gitmeli; ayrica araya asistan
    /// mesaji girse bile dogru olani secmeli.
    #[test]
    fn son_kullanici_mesaji_secilir() {
        let msgs = vec![
            m("system", "sen bir asistansin"),
            m("user", "ilk soru"),
            m("assistant", "ilk cevap"),
            m("user", "asil soru"),
        ];
        assert_eq!(son_kullanici_mesaji(&msgs).as_deref(), Some("asil soru"));
    }

    /// Bos dizide gecmis GITMELI: tohum, onceki turlari ve asil soruyu birlikte
    /// tasimali. Yoksa model konusmanin ortasinda gecmissiz cevap verir.
    #[test]
    fn tohum_gecmisi_ve_asil_soruyu_tasir() {
        let msgs = vec![
            m("system", "Kisa cevap ver."),
            m("user", "Turkiye'nin baskenti neresi?"),
            m("assistant", "Ankara."),
            m("user", "Peki nufusu?"),
        ];
        let t = tohum_istemi(&msgs, TOHUM_AZAMI);
        assert!(
            t.contains("Kisa cevap ver."),
            "sistem yonergesi dusmemeli: {t}"
        );
        assert!(t.contains("baskenti"), "onceki soru olmali");
        assert!(t.contains("Ankara."), "onceki cevap olmali");
        assert!(t.ends_with("Peki nufusu?"), "asil soru SONDA olmali: {t}");
    }

    /// Tek mesajlik konusmada tohumlanacak gecmis yok — istem oldugu gibi gitmeli,
    /// basina gereksiz aciklama eklenmemeli.
    #[test]
    fn tek_mesajda_tohum_sade_kalir() {
        assert_eq!(
            tohum_istemi(&[m("user", "merhaba")], TOHUM_AZAMI),
            "merhaba"
        );
    }

    /// Sinir asilinca EN ESKI turlar dusmeli (yakin baglam degerlidir) ve dusen
    /// olmasi YAZILMALI. Sessiz kirpma, eksik baglami tam sanmaktir.
    #[test]
    fn tohum_kirpmasi_yazili_ve_yakin_baglami_korur() {
        let msgs = vec![
            m("user", &"e".repeat(300)),
            m("assistant", "eski cevap"),
            m("user", "YAKIN SORU"),
            m("assistant", "YAKIN CEVAP"),
            m("user", "asil soru"),
        ];
        let t = tohum_istemi(&msgs, 60);
        assert!(t.contains("atlandi"), "kirpma yazili olmali: {t}");
        assert!(t.contains("YAKIN CEVAP"), "yakin baglam korunmali: {t}");
        assert!(!t.contains(&"e".repeat(300)), "en eski uzun tur dusmeliydi");
        assert!(t.ends_with("asil soru"));
    }

    /// Kullanici mesaji yoksa uydurma yapmadan None donmeli (uc 400 verir).
    #[test]
    fn kullanici_mesaji_yoksa_none() {
        let msgs = vec![m("system", "yalniz sistem")];
        assert_eq!(son_kullanici_mesaji(&msgs), None);
    }

    /// Konusma buyudukce iz DEGISMEMELI — degisirse ikinci tur baska hesaba duser ve
    /// tarayicidaki sohbet gecmisi bosa cikar. Sabitlemenin tamami buna dayaniyor.
    #[test]
    fn iz_konusma_boyunca_sabit_kalir() {
        let tur1 = vec![m("system", "sen bir asistansin"), m("user", "ilk soru")];
        let tur2 = vec![
            m("system", "sen bir asistansin"),
            m("user", "ilk soru"),
            m("assistant", "ilk cevap"),
            m("user", "ikinci soru"),
        ];
        assert_eq!(konusma_izi(&tur1), konusma_izi(&tur2));
    }

    /// Sistem istemi degisse bile iz sabit kalmali: oraya tarih/saat gibi her istekte
    /// degisen seyler giriyor, ize katilsa sabitleme sessizce ise yaramaz olurdu.
    #[test]
    fn degisen_sistem_istemi_izi_bozmaz() {
        let a = vec![
            m("system", "tarih: 2026-08-15 10:00"),
            m("user", "ilk soru"),
        ];
        let b = vec![
            m("system", "tarih: 2026-08-15 10:42"),
            m("user", "ilk soru"),
        ];
        assert_eq!(konusma_izi(&a), konusma_izi(&b));
    }

    /// Ayri konusmalar ayri ize dusmeli, yoksa hepsi tek hesaba yigilir.
    #[test]
    fn ayri_konusmalar_ayri_ize_duser() {
        let a = vec![m("user", "ilk soru")];
        let b = vec![m("user", "bambaska bir soru")];
        assert_ne!(konusma_izi(&a), konusma_izi(&b));
        assert_eq!(konusma_izi(&[]), None);
    }

    #[tokio::test]
    async fn rest_hedefi_cozumleme_ve_yonlendirme() {
        let ayar = kervan_tarayici::config::RestAyar {
            bicim: "openai".into(),
            taban_url: "https://api.deepseek.com".into(),
            anahtar_env: "DEEPSEEK_API_KEY".into(),
            model: "deepseek-chat".into(),
            yol: None,
        };
        let mut rest_hedefleri = std::collections::HashMap::new();
        rest_hedefleri.insert(
            "deepseek".into(),
            kervan_yukari::Hedef::rest_ayardan("deepseek", &ayar, Duration::from_secs(5)),
        );

        let temp_dir = std::env::temp_dir().join(format!("kervan_test_{}", rand::random::<u64>()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let defter_yolu = temp_dir.join("defter.log");
        let defter = Defter::ac(defter_yolu).unwrap();

        let ortam = crate::pool::Ortam {
            profiles_root: temp_dir.clone(),
            browser: crate::browser::BrowserBinary(std::path::PathBuf::from(".")),
            geckodriver: std::path::PathBuf::from("."),
            headless: true,
        };

        let pool = Pool::new(
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
            eklenti_hub: crate::eklenti::EklentiHub::new(),
            compressor: crate::compressor::Compressor::new(),
            router: crate::router::SmartRouter::new(crate::router::RoutingStrategy::Lkgp),
            sayaclar: Default::default(),
            rest_hedefleri,
            yukari: kervan_yukari::Yukari::new(),
            yedek_deneme: true,
            azami_deneme: 3,
            panel: Default::default(),
        });

        // 1. REST model çözümlenir
        let (hedef, alt) = state.rest_hedefi_coz("kervan/deepseek").unwrap();
        assert_eq!(hedef.ad, "deepseek");
        assert_eq!(alt, None);

        // 2. Alt model çözümlenir
        let (hedef2, alt2) = state
            .rest_hedefi_coz("kervan/deepseek/deepseek-reasoner")
            .unwrap();
        assert_eq!(hedef2.ad, "deepseek");
        assert_eq!(alt2, Some("deepseek-reasoner".into()));

        // 3. REST olmayan model çözülmez (None)
        assert!(state.rest_hedefi_coz("kervan/gemini").is_none());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
