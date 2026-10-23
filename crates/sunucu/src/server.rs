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
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::browser::ask;
use crate::kota::{self, Defter, Sonuc};
use crate::pool::Pool;
use anyhow::Result;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::eklenti_uc::{eklenti_sekmeler, jeton, on_denetim, ws_eklenti_handler};
use crate::hesap::{HesapIste, hesap_hedefi};
use crate::rest::rest_denemesi;

// İstem kurulumu ayrı sandıkta (sandık sınırı): mesaj tipi ve tohum/iz kurucuları
// burada yeniden dışa aktarılır, böylece `server::Message` yolu değişmez.
pub use kervan_istem::{Message, TOHUM_AZAMI, konusma_izi, son_kullanici_mesaji, tohum_istemi};

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
fn model_kimligi(saglayici: &str) -> String {
    format!("kervan/{saglayici}")
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/v1/models", get(models))
        // Panelin secim listesi: her saglayicinin GERCEKTEN uygulanabilir model
        // kodlari ve ac/kapa anahtarlari. Yalniz yapilandirilmis (olculmus)
        // olanlar listelenir; olmayan secenek gostermek yanlis model riskidir.
        .route("/kervan/secenekler", get(secenekler))
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
    Json(serde_json::json!({
        "defter": kota::yol_goster(&st.defter).display().to_string(),
        "hesaplar": liste,
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

// ─── OpenAI bicimleri ────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ChatRequest {
    #[serde(default)]
    pub model: String,
    pub messages: Vec<Message>,
    /// Çoğu ajan istemcisi VARSAYILAN OLARAK akis ister. Bunu desteklemeyen
    /// bir uc, istemci tarafinda "empty stream" olarak gorunur ve sessizce yedege duser.
    #[serde(default)]
    pub stream: bool,
    /// OpenAI araç tanımları. Tarayıcı modeli bunları bilmez: istemine biçim
    /// yönergesi eklenir, yanıttaki ```json bloğu `tool_calls`'a çevrilir.
    #[serde(default)]
    pub tools: Vec<serde_json::Value>,
    /// Kervan'a özgü alanlar (site aç/kapa özellikleri). OpenAI istemcileri bunu
    /// bilmez ve göndermez; panel gönderir. Sessiz yok sayılmaz: tanımsız anahtar
    /// ya da anahtar desteklemeyen yol AÇIK hataya düşer.
    #[serde(default)]
    pub kervan: Option<KervanIstegi>,
}

/// `kervan` alanı: sitedeki aç/kapa özelliklerinin istenen durumu.
///
/// Örnek: `{"anahtarlar": {"derin": true, "arama": false}}`.
#[derive(Debug, Default, Deserialize)]
pub struct KervanIstegi {
    #[serde(default)]
    pub anahtarlar: std::collections::BTreeMap<String, bool>,
}

#[derive(Serialize)]
struct ChatResponse {
    id: String,
    object: &'static str,
    created: u64,
    model: String,
    choices: Vec<Choice>,
    usage: Usage,
}

#[derive(Serialize)]
struct Choice {
    index: u32,
    message: Message,
    finish_reason: &'static str,
}

/// Tarayici arayuzu token saymiyor. Uydurma sayi vermek, kota hesabini bozar ve
/// istemciyi yaniltir — sifir doner, bilinmedigi acikca belli olur.
/// Asama 3'teki kota ogrenici istek SAYISINDAN ogrenecek, token'dan degil.
#[derive(Serialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
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
async fn secenekler(State(st): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let mut saglayicilar = serde_json::Map::new();
    for (ad, s) in &st.saglayicilar {
        if let Some(v) = kervan_secim::secenek_kaydi(s.model.as_ref(), &s.anahtarlar) {
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
    Json(ModelList {
        object: "list",
        data: veri,
    })
}

async fn chat(State(st): State<Arc<AppState>>, Json(raw_val): Json<serde_json::Value>) -> Response {
    let req: ChatRequest = match serde_json::from_value(raw_val.clone()) {
        Ok(r) => r,
        Err(e) => {
            return hata(
                StatusCode::BAD_REQUEST,
                &format!("Geçersiz ChatRequest gövdesi: {}", e),
            );
        }
    };

    let model_adi = req.model.trim();

    // 1. REST yukarı akış kancası: model [rest.*] yapılandırmasına çözülürse yukari'ya devret
    if let Some((hedef, alt_model)) = st.rest_hedefi_coz(model_adi) {
        return match rest_denemesi(&st, &hedef, &raw_val, alt_model.as_deref(), req.stream).await {
            Ok(yanit) => yanit,
            Err((yanit, _)) => yanit,
        };
    }

    let Some(istem) = son_kullanici_mesaji(&req.messages) else {
        return hata(StatusCode::BAD_REQUEST, "messages icinde user rolu yok");
    };

    let otomatik = model_adi.is_empty() || model_adi == "auto";

    // Model kimliginden saglayiciyi sec. 'auto' verilirse SmartRouter adayları sıralar.
    // Model KODU ayrica cozulur: `kervan/gemini/yuksek`. Bos ise site varsayilani.
    let mut model_kodu: Option<String> = None;

    // Ac/kapa ozellikleri: "auto" yolunda hangi saglayicinin secilecegi onceden
    // bilinmedigi icin uygulanamaz — sessizce yok saymak yerine acik hata.
    let istenen_anahtarlar: Vec<(String, bool)> = req
        .kervan
        .as_ref()
        .map(|k| k.anahtarlar.iter().map(|(a, b)| (a.clone(), *b)).collect())
        .unwrap_or_default();
    if otomatik && !istenen_anahtarlar.is_empty() {
        return hata(
            StatusCode::BAD_REQUEST,
            "ac/kapa anahtarlari icin saglayiciyi acikca belirt ('auto' ile uygulanamaz)",
        );
    }
    // Cozulmus istek: (tanim, istenen durum). Cozulmemis kod istek GONDERILMEDEN hata.
    let mut anahtar_istegi: Vec<(crate::browser::Anahtar, bool)> = Vec::new();
    let (adaylar, yedeklenebilir) = if otomatik {
        // 'auto' adayları ÜÇ kaynaktan gelir: bağlı eklenti sekmeleri, havuz
        // hesapları ve REST hedefleri. Böylece yerel (anahtarlı) ve web
        // (oturumlu) sağlayıcılar tek yönlendirme kararında yarışır.
        let adaylar = yonlendirme_adaylari(&st).await;
        if adaylar.is_empty() {
            (vec![st.varsayilan_saglayici.clone()], false)
        } else {
            let sirali = st.router.aday_sirasi(&adaylar).await;
            (sirali, true)
        }
    } else {
        let (saglayici, kod) = match crate::saglayicilar::model_coz(model_adi, &st.saglayicilar) {
            Some(v) => v,
            None => {
                let sade = model_adi.strip_prefix("kervan/").unwrap_or(model_adi);
                if st.eklenti_hub.sekme_var_mi(sade).await {
                    (sade.to_string(), None)
                } else {
                    let mut mevcut: Vec<String> = st
                        .pool
                        .slots
                        .iter()
                        .map(|s| model_kimligi(&s.provider.ad))
                        .collect::<std::collections::BTreeSet<_>>()
                        .into_iter()
                        .collect();
                    for ext_s in st.eklenti_hub.bagli_sekmeler().await {
                        mevcut.push(model_kimligi(&ext_s));
                    }
                    mevcut.push("auto".into());
                    mevcut.sort();
                    mevcut.dedup();
                    return hata(
                        StatusCode::BAD_REQUEST,
                        &format!(
                            "'{}' hangi saglayiciya ait bilinmiyor. Kullanilabilir: {}",
                            req.model,
                            mevcut.join(", ")
                        ),
                    );
                }
            }
        };
        // Model kodu ve anahtarlar SITeye dokunmadan once tanima gore cozulur:
        // tanimsiz (400), tanimsiz menu (501), kilitli (402) durumlari burada
        // kesilir. Mantik `kervan-secim` sandiginda; burasi yalniz HTTP eslemesi.
        let menu = st
            .saglayicilar
            .get(&saglayici)
            .and_then(|s| s.model.as_ref());
        let tanimli_anahtarlar = st
            .saglayicilar
            .get(&saglayici)
            .map(|s| s.anahtarlar.as_slice())
            .unwrap_or_default();
        let cozulen = match kervan_secim::coz_istek(
            &saglayici,
            menu,
            kod.as_deref(),
            tanimli_anahtarlar,
            &istenen_anahtarlar,
        ) {
            Ok(v) => v,
            Err(e) => {
                let durum = StatusCode::from_u16(e.durum).unwrap_or(StatusCode::BAD_REQUEST);
                return hata(durum, &e.mesaj);
            }
        };
        model_kodu = cozulen.model_kodu;
        anahtar_istegi = cozulen.anahtarlar;
        (vec![saglayici], false)
    };

    // TOKEN SIKIŞTIRMA (RTK + Caveman): Gelen istemi göndermeden önce gereksiz boşluk ve kalıplardan arındır
    let sikistirma = st.compressor.rtk_sikistir(&istem);
    let mut gonderilecek_istem = if sikistirma.tasarruf_yuzdesi > 3.0 {
        println!(
            "sıkıştırma   : %{:.1} token tasarrufu sağlandı ({} -> {} karakter)",
            sikistirma.tasarruf_yuzdesi, sikistirma.orijinal_uzunluk, sikistirma.yeni_uzunluk
        );
        sikistirma.metin.clone()
    } else {
        istem.clone()
    };
    // Yönerge sıkıştırmadan SONRA eklenir: biçim örneği bozulmamalı.
    let yonerge = arac_yonergesi(&req.tools);
    gonderilecek_istem.push_str(&yonerge);

    // Araç isteğinde gerçek işlev çağrısını yalnız REST hedefleri yapabilir;
    // tarayıcı yolu araçları yönergeyle taklit eder. REST adayı varsa diğerleri
    // elenir. Yoksa istek tamamen düşmesin diye tarayıcı yoluyla denenir ve bu
    // açıkça uyarılır.
    let mut adaylar = adaylar;
    if otomatik && !req.tools.is_empty() {
        if adaylar.iter().any(|ad| st.rest_hedefleri.contains_key(ad)) {
            adaylar.retain(|ad| st.rest_hedefleri.contains_key(ad));
        } else {
            println!(
                "akilli-rota   : uyari: araç isteği var ama REST adayı yok; tarayıcı yolu yalnız yönergeyle denenecek"
            );
        }
    }

    // Yedek deneme: yalnız 'auto' isteğinde ve akış/yanıt başlamadan önceki
    // hatada sıradaki adaya geçilir. `azami_deneme` kaç aday deneneceğini sınırlar.
    let azami = if yedeklenebilir && st.yedek_deneme {
        st.azami_deneme.max(1) as usize
    } else {
        1
    };

    let mut son_hata: Option<Response> = None;
    // Secimler tek yapida toplanir: deneme dongusu icinde ayni sekilde kullanilir.
    let secim = SecimIstegi {
        model_kodu: model_kodu.clone(),
        anahtarlar: anahtar_istegi.clone(),
    };
    for ad in adaylar.iter().take(azami) {
        if otomatik {
            println!("akilli-rota   : 'auto' adayı deneniyor -> [{ad}]");
        }

        let sonuc = if let Some(hedef) = st.rest_hedefleri.get(ad).cloned() {
            rest_denemesi(&st, &hedef, &raw_val, None, req.stream).await
        } else if st.eklenti_hub.sekme_var_mi(ad).await {
            // Eklenti yolunda model/anahtar secimi HENUZ uygulanmiyor (D1d). Sessizce
            // sekmenin o anki durumuyla cevap vermek yanlis model/ozellik olurdu.
            if model_kodu.is_some() || !anahtar_istegi.is_empty() {
                Err((
                    hata(
                        StatusCode::NOT_IMPLEMENTED,
                        &format!(
                            "'{ad}' eklenti sekmesi uzerinden calisiyor; model/ac-kapa secimi bu \
                             yolda henuz uygulanmiyor. Eklenti sekmesini kapatip havuz yolunu \
                             kullan ya da secimsiz '{}' iste.",
                            model_kimligi(ad)
                        ),
                    ),
                    false,
                ))
            } else {
                eklenti_denemesi(&st, ad, &gonderilecek_istem, &req).await
            }
        } else {
            havuz_denemesi(&st, ad, &req, &istem, &yonerge, &gonderilecek_istem, &secim).await
        };

        match sonuc {
            Ok(yanit) => return yanit,
            Err((yanit, yeniden)) => {
                son_hata = Some(yanit);
                if !yeniden {
                    break;
                }
            }
        }
    }

    son_hata.unwrap_or_else(|| {
        hata(
            StatusCode::SERVICE_UNAVAILABLE,
            "kullanilabilir saglayici bulunamadi",
        )
    })
}

/// Eklenti sekmesi üzerinden tek deneme. Yanıt kotaysa bu model yanıtı değildir:
/// 429 döner, sağlayıcı sağlıksız işaretlenir ve yedek deneme sıradaki adaya geçer.
#[allow(
    clippy::result_large_err,
    reason = "hata yaniti istemciye aynen doner; Boxing cagri yerlerini karmasiklastirirdi"
)]
async fn eklenti_denemesi(
    st: &Arc<AppState>,
    saglayici: &str,
    gonderilecek_istem: &str,
    req: &ChatRequest,
) -> Result<Response, (Response, bool)> {
    println!(
        "[{}] istek Kervan Eklentisi uzerinden iletiliyor ({} karakter)",
        saglayici,
        gonderilecek_istem.len()
    );
    let t = std::time::Instant::now();
    let sonuc = match st
        .eklenti_hub
        .sor(saglayici, gonderilecek_istem, st.timeout)
        .await
    {
        Err(e) => match e.downcast::<crate::eklenti::YarimYanit>() {
            Ok(crate::eklenti::YarimYanit(yarim)) => kurtar(st, saglayici, yarim).await,
            Err(e) => Err(e),
        },
        tamam => tamam,
    };
    match sonuc {
        Ok(yanit) => {
            if kota::kota_sinyali(&yanit, &st.kota_ayar.isaretler) {
                st.router.record_failure(saglayici).await;
                println!("[{}] eklenti KOTA bildirdi", saglayici);
                return Err((
                    hata(
                        StatusCode::TOO_MANY_REQUESTS,
                        "saglayici kotasini bildirdi (eklenti)",
                    ),
                    true,
                ));
            }
            let latency = t.elapsed().as_millis() as u64;
            st.router.record_success(saglayici, latency).await;
            println!(
                "[{}] eklenti yaniti alindi ({:.1} sn)",
                saglayici,
                t.elapsed().as_secs_f32()
            );
            Ok(yanit_olustur(st, req, yanit, model_kimligi(saglayici)))
        }
        Err(e) => {
            st.router.record_failure(saglayici).await;
            println!("[{}] eklenti hatasi: {e:#}", saglayici);
            Err((
                hata(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("Eklenti hatası: {e:#}"),
                ),
                true,
            ))
        }
    }
}

/// Tek bir denemede site arayuzune uygulanacak secimler (model + ac/kapa).
///
/// NEDEN TEK YAPI: ikisi de ayni yerde cozulur, ayni `ask` cagrisina gider ve
/// ikisi de "uygulanamazsa acik hata" kuralina tabidir. Ayri parametreler
/// `havuz_denemesi` imzasini okunamaz hale getiriyordu.
#[derive(Debug, Default)]
struct SecimIstegi {
    model_kodu: Option<String>,
    anahtarlar: Vec<(crate::browser::Anahtar, bool)>,
}

/// WebDriver havuzundan tek deneme. Hesap kiralanamazsa ya da model yanıtı
/// gelmezse hata yedeklenebilir sayılır (akış henüz başlamadı).
#[allow(
    clippy::result_large_err,
    reason = "hata yaniti istemciye aynen doner; Boxing cagri yerlerini karmasiklastirirdi"
)]
async fn havuz_denemesi(
    st: &Arc<AppState>,
    saglayici: &str,
    req: &ChatRequest,
    istem: &str,
    yonerge: &str,
    gonderilecek_istem: &str,
    secim: &SecimIstegi,
) -> Result<Response, (Response, bool)> {
    // Model kodu cozulurken dogrulanmisti; burada yalniz eslestirme yapilir.
    // Yine de savunma amacli: tanimsiz kod istek gonderilmeden acik hataya duser.
    let model_istegi: Option<(crate::browser::ModelMenusu, String)> =
        match secim.model_kodu.as_deref() {
            None => None,
            Some(kod) => {
                let menu = st.saglayicilar.get(saglayici).and_then(|s| s.model.clone());
                let etiket = menu
                    .as_ref()
                    .and_then(|m| m.etiket(kod))
                    .map(str::to_string);
                match (menu, etiket) {
                    (Some(m), Some(e)) => Some((m, e)),
                    _ => {
                        return Err((
                            hata(
                                StatusCode::BAD_REQUEST,
                                &format!("'{saglayici}' icin tanimsiz model kodu '{kod}'"),
                            ),
                            false,
                        ));
                    }
                }
            }
        };
    // Havuzdan hesap kirala: konusma kendi hesabina sabitlenir, hiz butcesi burada
    // uygulanir, gerekirse beklenir.
    let mut lease = match st
        .pool
        .acquire_for(konusma_izi(&req.messages), saglayici)
        .await
    {
        Ok(l) => l,
        Err(e) => {
            return Err((
                hata(StatusCode::SERVICE_UNAVAILABLE, &format!("{e:#}")),
                true,
            ));
        }
    };
    // Dizi bossa gecmisi TOHUMLA. Devam eden dizide tohumlamak gecmisi ikinci kez
    // yazmak olurdu; bos dizide tohumlamamak ise modele gecmissiz cevap verdirir.
    let gonderilecek = if lease.yeni_dizi {
        tohum_istemi(&req.messages, TOHUM_AZAMI) + yonerge
    } else {
        gonderilecek_istem.to_string()
    };
    println!(
        "[{}/{}] istek ({} karakter{})",
        saglayici,
        lease.name,
        gonderilecek.len(),
        if lease.yeni_dizi && gonderilecek.len() > istem.len() {
            ", yeni dizi — gecmis tohumlandi"
        } else {
            ""
        }
    );
    let secililer = lease.provider.selectors.clone();
    let t_wd = std::time::Instant::now();
    let model_cagrisi = model_istegi.as_ref().map(|(m, e)| (m, e.as_str()));
    let mut sonuc = ask(
        lease.client(),
        &secililer,
        &gonderilecek,
        st.timeout,
        model_cagrisi,
        &secim.anahtarlar,
    )
    .await;

    // Saglayici "limite ulastin" dediyse bu bir YANIT DEGIL. Oldugu gibi dondurmek
    // istemciye modelin cevabi diye limit mesajini vermek olurdu; ustelik kota
    // ogrenici de olayi hic gormezdi.
    let kota_carpmasi = match &sonuc {
        Ok(m) if kota::kota_sinyali(m, &st.kota_ayar.isaretler) => Some(m.clone()),
        _ => None,
    };
    if let Some(m) = &kota_carpmasi {
        sonuc = Err(anyhow::anyhow!("saglayici kota bildirdi: {m}"));
    }
    if let Err(e) = &sonuc {
        println!("[{}] HATA: {e:#}", lease.name);
        // INSAN KAPISI: dogrulama katmani cikmissa pencere one getirildi ve insan
        // cozecek. Oturumu insanin eline birakiyoruz ki bosta supurucu 15 dakika
        // sonra pencereyi kapatmasin — cozum yarida kalirdi.
        if format!("{e:#}").contains(crate::browser::INSAN_KAPISI) {
            lease.insan_eline_birak();
            println!(
                "[{}] oturum insanin elinde: dogrulamayi coz, sonra istegi tekrarla.",
                lease.name
            );
        }
    }

    // Sonucu havuza bildir: basari sayaci sifirlar, hata ustel geri cekilme uygular
    // ve dogrulama duvariysa hesabi insan kapisinda duraklatir.
    match (&kota_carpmasi, &sonuc) {
        (Some(metin), _) => {
            st.router.record_failure(&lease.provider.ad).await;
            st.defter
                .yaz(
                    &lease.name,
                    &lease.provider.ad,
                    Sonuc::Kota,
                    Some(kirp(metin)),
                )
                .await;
            // Pencere ogrenildiyse onu, ogrenilmediyse muhafazakar varsayilani bekle.
            let ozet = st.defter.ozet(&lease.name, &lease.provider.ad).await;
            // Ogrenilmis pencere varsa onu bekle; yoksa muhafazakar varsayilan.
            // `max(1)`: ogrenilen pencere sifir dakika cikarsa beklemeden yeniden
            // denemek limite carpip durmak olurdu.
            let dk = ozet
                .tahmin
                .pencere_dk
                .map(|d| d.max(1))
                .unwrap_or(st.kota_ayar.bilinmeyen_pencere_dk);
            println!("[{}] KOTA doldu, {dk} dk bekletiliyor", lease.name);
            st.pool
                .kota_doldu(lease.index, Duration::from_secs(dk * 60))
                .await;
        }
        (None, Ok(_)) => {
            let latency = t_wd.elapsed().as_millis() as u64;
            st.router.record_success(&lease.provider.ad, latency).await;
            st.defter
                .yaz(&lease.name, &lease.provider.ad, Sonuc::Basari, None)
                .await;
            st.pool.basarili(lease.index).await;
        }
        (None, Err(e)) => {
            st.router.record_failure(&lease.provider.ad).await;
            st.defter
                .yaz(&lease.name, &lease.provider.ad, Sonuc::Hata, None)
                .await;
            st.pool.basarisiz(lease.index, &format!("{e:#}")).await;
        }
    }

    let model_adi = if req.model.is_empty() {
        model_kimligi(saglayici)
    } else {
        req.model.clone()
    };

    match sonuc {
        Ok(metin) => Ok(yanit_olustur(st, req, metin, model_adi)),
        // Ham neden zinciri ({e:#}) istemciye gider. Hatayi maskelemek, OmniRoute'un
        // gemini-web'de yaptigi ve teshisi imkansizlastiran seydi.
        Err(e) => Err((hata(StatusCode::BAD_GATEWAY, &format!("{e:#}")), true)),
    }
}

/// Eklenti zaman aşımında kesilen yanıtı bir kez devam istemiyle sürdürür ve diker.
/// Devam da gelmezse yarım metni yanıt diye vermek yerine hata döner.
async fn kurtar(st: &AppState, saglayici: &str, yarim: String) -> Result<String> {
    use kervan_akis::stream_recovery::DikisBirlestirici as Dikis;
    println!(
        "[{saglayici}] yanit yarida kesildi ({} karakter), devam isteniyor",
        yarim.len()
    );
    let devam = st
        .eklenti_hub
        .sor(saglayici, &Dikis::devam_istemi_olustur(&yarim), st.timeout)
        .await
        .map_err(|e| anyhow::anyhow!("yanit yarida kesildi, devami da alinamadi: {e:#}"))?;
    st.sayaclar
        .kurtarilan_yanit
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Ok(Dikis::dikis_ekle(&yarim, &devam))
}

/// Başarılı yanıtı istemcinin istediği biçimde (akış ya da tek parça) kurar; araç
/// tanımı geldiyse yanıttaki çağrıları `tool_calls`'a çevirir.
fn yanit_olustur(st: &AppState, req: &ChatRequest, metin: String, model: String) -> Response {
    let (icerik, cagrilar) = arac_cagrilarini_ayikla(&req.tools, metin);
    st.sayaclar
        .arac_cagrisi
        .fetch_add(cagrilar.len() as u64, std::sync::atomic::Ordering::Relaxed);
    let bitis = if cagrilar.is_empty() {
        "stop"
    } else {
        "tool_calls"
    };
    if req.stream {
        return sse_yanit(&icerik, &cagrilar, bitis, &model);
    }
    Json(ChatResponse {
        id: format!("chatcmpl-kervan-{}", simdi()),
        object: "chat.completion",
        created: simdi(),
        model,
        choices: vec![Choice {
            index: 0,
            message: Message {
                role: "assistant".into(),
                content: icerik,
                tool_calls: (!cagrilar.is_empty()).then_some(serde_json::Value::Array(cagrilar)),
                ..Default::default()
            },
            finish_reason: bitis,
        }],
        usage: Usage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        },
    })
    .into_response()
}

fn arac_adlari(tools: &[serde_json::Value]) -> Vec<&str> {
    tools
        .iter()
        .filter_map(|t| t["function"]["name"].as_str())
        .collect()
}

/// Araç tanımlarını tarayıcı modelinin anlayacağı düz yönergeye çevirir.
pub fn arac_yonergesi(tools: &[serde_json::Value]) -> String {
    if arac_adlari(tools).is_empty() {
        return String::new();
    }
    let mut y = String::from(
        "\n\n[ARAÇLAR] Bir aracı çağırman gerekirse yanıtına YALNIZCA şu biçimde bir blok koy, \
         sonucu bir sonraki mesajda alacaksın:\n```json\n{\"name\": \"ARAÇ_ADI\", \"arguments\": {}}\n```\n\
         Kullanılabilir araçlar:\n",
    );
    for t in tools {
        let f = &t["function"];
        if let Some(ad) = f["name"].as_str() {
            let aciklama = f["description"].as_str().unwrap_or("");
            y.push_str(&format!(
                "- {ad}: {aciklama} (parametreler: {})\n",
                f["parameters"]
            ));
        }
    }
    y
}

/// Yanıttaki ```json çağrılarından YALNIZ istemcinin tanımladığı araçları `tool_calls`
/// biçimine çevirir. Eşleşme yoksa metin olduğu gibi kalır.
pub fn arac_cagrilarini_ayikla(
    tools: &[serde_json::Value],
    metin: String,
) -> (String, Vec<serde_json::Value>) {
    use kervan_akis::translator::{AracCagrisi, AracCagrisiAyiklayici};
    let adlar = arac_adlari(tools);
    if adlar.is_empty() {
        return (metin, Vec::new());
    }
    let sonuc = AracCagrisiAyiklayici::ayikla(&metin);
    let cagrilar: Vec<_> = sonuc
        .cagrilar
        .into_iter()
        .filter_map(|c| match c {
            AracCagrisi::GenelFonksiyon {
                ad,
                parametreler_json,
            } if adlar.contains(&ad.as_str()) => Some(serde_json::json!({
                "id": format!("call_{:08x}", rand::random::<u32>()),
                "type": "function",
                "function": { "name": ad, "arguments": parametreler_json },
            })),
            _ => None,
        })
        .collect();
    if cagrilar.is_empty() {
        (metin, cagrilar)
    } else {
        (sonuc.temiz_metin, cagrilar)
    }
}

/// Kota mesajindan deftere yazilacak kisa kesit. Amac gercek ifadeyi ogrenmek,
/// sohbet biriktirmek degil.
fn kirp(m: &str) -> String {
    m.chars().take(200).collect()
}

pub(crate) fn hata(kod: StatusCode, mesaj: &str) -> Response {
    (
        kod,
        Json(serde_json::json!({ "error": { "message": mesaj } })),
    )
        .into_response()
}

/// OpenAI uyumlu SSE akisi.
///
/// ponytail: tarayici bize metni parca parca degil, tamamlandiginda veriyor
/// (bitisi kararlilikla algiliyoruz), o yuzden tek parca + [DONE] gonderiyoruz.
/// Protokol acisindan gecerli; istemci akis bekledigi icin bu sart.
/// Yukseltme yolu: `ask` icindeki yoklama dongusu zaten metnin BUYUDUGUNU goruyor —
/// oradan bir kanal acilirsa gercek artimli akis neredeyse bedava gelir. Ihtiyac
/// olunca yap; su an tek parca yeterli ve yanlis degil.
fn sse_yanit(metin: &str, cagrilar: &[serde_json::Value], neden: &str, model: &str) -> Response {
    let id = format!("chatcmpl-kervan-{}", simdi());
    let olustu = simdi();
    let mut delta = serde_json::json!({ "role": "assistant", "content": metin });
    if !cagrilar.is_empty() {
        // Akışta her çağrı sırasını `index` ile taşır.
        let sirali: Vec<_> = cagrilar
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let mut c = c.clone();
                c["index"] = i.into();
                c
            })
            .collect();
        delta["tool_calls"] = sirali.into();
    }
    let parca = serde_json::json!({
        "id": id, "object": "chat.completion.chunk", "created": olustu, "model": model,
        "choices": [{ "index": 0, "delta": delta }]
    });
    let bitis = serde_json::json!({
        "id": id, "object": "chat.completion.chunk", "created": olustu, "model": model,
        "choices": [{ "index": 0, "delta": {}, "finish_reason": neden }]
    });
    let govde = format!("data: {parca}\n\ndata: {bitis}\n\ndata: [DONE]\n\n");
    (
        StatusCode::OK,
        [
            ("content-type", "text/event-stream"),
            ("cache-control", "no-cache"),
        ],
        govde,
    )
        .into_response()
}

fn simdi() -> u64 {
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
    use crate::hesap::eposta_gecerli_mi;

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
