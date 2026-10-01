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
use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};

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
        .route("/v1/chat/completions", post(chat))
        .route("/health", get(|| async { "ok" }))
        .route("/telemetri", get(telemetri_handler))
        // Zcode tarzı dinamik bakiye ve hediye token takibi
        .route("/bakiye", get(bakiye_handler))
        .route("/bakiye/hediye", post(bakiye_hediye_handler))
        // Eklenti WebSocket köprüsü — WebDriver olmadan doğrudan tarayıcı ile konuşur.
        .route("/ws/eklenti", get(ws_eklenti_handler))
        .route("/eklenti/sekmeler", get(eklenti_sekmeler))
        // Hesap durumu — TUI gelene kadarki gorunurluk penceresi. Insan kapisinda
        // bekleyen hesap buradan gorulur.
        .route("/accounts", get(accounts))
        // Insan kapisi cozuldukten sonra hesabi geri al. Bu uc olmadan duraklatilan
        // hesap surec omru boyunca olu kalirdi.
        .route("/accounts/{i}/devam", post(devam))
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
        .with_state(state)
}

async fn ws_eklenti_handler(
    ws: WebSocketUpgrade,
    State(st): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_eklenti_socket(socket, st.eklenti_hub.clone()))
}

async fn handle_eklenti_socket(socket: WebSocket, hub: crate::eklenti::EklentiHub) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (tx_giden, mut rx_giden) =
        tokio::sync::mpsc::unbounded_channel::<crate::eklenti::GidenMesaj>();

    let mut provider_kayitli: Option<String> = None;

    // Kervan'dan gelen komutları eklenti WebSocket'ine basan arka plan görevi
    let giden_gorev = tokio::spawn(async move {
        while let Some(msg) = rx_giden.recv().await {
            if let Ok(json) = serde_json::to_string(&msg)
                && ws_tx.send(WsMessage::Text(json.into())).await.is_err()
            {
                break;
            }
        }
    });

    // Eklentiden gelen mesajları okuyan döngü
    while let Some(Ok(msg)) = ws_rx.next().await {
        if let WsMessage::Text(text) = msg
            && let Ok(gelen) = serde_json::from_str::<crate::eklenti::GelenMesaj>(&text)
        {
            if let crate::eklenti::GelenMesaj::Kayit {
                ref provider,
                ref profile,
                ..
            } = gelen
            {
                let prof = profile.clone().unwrap_or_else(|| "varsayilan".into());
                provider_kayitli = Some(provider.clone());
                hub.sekme_kaydet(provider.clone(), prof, tx_giden.clone())
                    .await;
            }
            hub.mesaji_isle(gelen).await;
        }
    }

    if let Some(p) = provider_kayitli {
        hub.sekme_ayril(&p).await;
    }
    giden_gorev.abort();
}

async fn eklenti_sekmeler(State(st): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let sekmeler = st.eklenti_hub.bagli_sekmeler().await;
    Json(serde_json::json!({
        "bagli_sekmeler": sekmeler,
        "sayi": sekmeler.len(),
    }))
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
    Json(serde_json::json!({
        "accounts": liste,
        "sabit_oturumlar": st.pool.sabit_sayisi().await,
    }))
}

async fn devam(State(st): State<Arc<AppState>>, Path(i): Path<usize>) -> Response {
    if i >= st.pool.slots.len() {
        return hata(StatusCode::NOT_FOUND, &format!("{i} numarali hesap yok"));
    }
    st.pool.devam_ettir(i).await;
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
    /// Hermes ve cogu ajan istemcisi VARSAYILAN OLARAK akis ister. Bunu desteklemeyen
    /// bir uc, istemci tarafinda "empty stream" olarak gorunur ve sessizce yedege duser.
    #[serde(default)]
    pub stream: bool,
    /// OpenAI araç tanımları. Tarayıcı modeli bunları bilmez: istemine biçim
    /// yönergesi eklenir, yanıttaki ```json bloğu `tool_calls`'a çevrilir.
    #[serde(default)]
    pub tools: Vec<serde_json::Value>,
}

#[derive(Deserialize, Serialize, Clone, Default)]
pub struct Message {
    pub role: String,
    /// Araç çağrısı taşıyan asistan mesajında OpenAI `null` gönderir.
    #[serde(default, deserialize_with = "bos_olabilir")]
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

fn bos_olabilir<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(d)?.unwrap_or_default())
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
}

// ─── Uclar ───────────────────────────────────────────────────────────────────

async fn models(State(st): State<Arc<AppState>>) -> Json<ModelList> {
    // Hem havuzdaki hesaplar hem de canli bagli eklenti sekmeleri listelenir.
    let mut ad_seti: std::collections::BTreeSet<String> = st
        .pool
        .slots
        .iter()
        .map(|s| s.provider.ad.clone())
        .collect();
    for ext_s in st.eklenti_hub.bagli_sekmeler().await {
        ad_seti.insert(ext_s);
    }
    let mut adlar: Vec<String> = ad_seti.into_iter().collect();
    adlar.sort();
    Json(ModelList {
        object: "list",
        data: adlar
            .into_iter()
            .map(|ad| ModelInfo {
                id: model_kimligi(&ad),
                object: "model",
                created: simdi(),
                owned_by: "kervan",
            })
            .collect(),
    })
}

async fn chat(State(st): State<Arc<AppState>>, Json(req): Json<ChatRequest>) -> Response {
    let Some(istem) = son_kullanici_mesaji(&req.messages) else {
        return hata(StatusCode::BAD_REQUEST, "messages icinde user rolu yok");
    };

    // Model kimliginden saglayiciyi sec. 'auto' verilirse SmartRouter en uygununu belirler.
    let model_adi = req.model.trim();
    let saglayici = if model_adi.is_empty() || model_adi == "auto" {
        let mut adaylar = st.eklenti_hub.bagli_sekmeler().await;
        if adaylar.is_empty() {
            // Havuzdaki benzersiz saglayicilari aday olarak ekle
            for slot in &st.pool.slots {
                if !adaylar.contains(&slot.provider.ad) {
                    adaylar.push(slot.provider.ad.clone());
                }
            }
        }
        if let Some(secilen) = st.router.select_provider(&adaylar).await {
            println!(
                "akilli-rota   : 'auto' isteği için en uygun sağlayıcı seçildi -> [{secilen}]"
            );
            secilen
        } else {
            st.varsayilan_saglayici.clone()
        }
    } else {
        match crate::saglayicilar::model_saglayicisi(model_adi, &st.saglayicilar) {
            Some(s) => s,
            None => {
                let sade = model_adi.strip_prefix("kervan/").unwrap_or(model_adi);
                if st.eklenti_hub.sekme_var_mi(sade).await {
                    sade.to_string()
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
        }
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

    // EKLENTİ KONTROLÜ: Eğer ilgili sağlayıcı için eklenti sekmesi bağlıysa,
    // WebDriver havuzuna hiç bulaşmadan doğrudan eklenti üzerinden sorulur!
    if st.eklenti_hub.sekme_var_mi(&saglayici).await {
        println!(
            "[{}] istek Kervan Eklentisi uzerinden iletiliyor ({} karakter)",
            saglayici,
            gonderilecek_istem.len()
        );
        let t = std::time::Instant::now();
        let sonuc = match st
            .eklenti_hub
            .sor(&saglayici, &gonderilecek_istem, st.timeout)
            .await
        {
            Err(e) => match e.downcast::<crate::eklenti::YarimYanit>() {
                Ok(crate::eklenti::YarimYanit(yarim)) => kurtar(&st, &saglayici, yarim).await,
                Err(e) => Err(e),
            },
            tamam => tamam,
        };
        match sonuc {
            Ok(yanit) => {
                let latency = t.elapsed().as_millis() as u64;
                st.router.record_success(&saglayici, latency).await;
                println!(
                    "[{}] eklenti yaniti alindi ({:.1} sn)",
                    saglayici,
                    t.elapsed().as_secs_f32()
                );
                return yanit_olustur(&st, &req, yanit, model_kimligi(&saglayici));
            }
            Err(e) => {
                st.router.record_failure(&saglayici).await;
                println!("[{}] eklenti hatasi: {e:#}", saglayici);
                return hata(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("Eklenti hatası: {e:#}"),
                );
            }
        }
    }

    // Havuzdan hesap kirala: konusma kendi hesabina sabitlenir, hiz butcesi burada
    // uygulanir, gerekirse beklenir.
    let mut lease = match st
        .pool
        .acquire_for(konusma_izi(&req.messages), &saglayici)
        .await
    {
        Ok(l) => l,
        Err(e) => return hata(StatusCode::SERVICE_UNAVAILABLE, &format!("{e:#}")),
    };
    // Dizi bossa gecmisi TOHUMLA. Devam eden dizide tohumlamak gecmisi ikinci kez
    // yazmak olurdu; bos dizide tohumlamamak ise modele gecmissiz cevap verdirir.
    let gonderilecek = if lease.yeni_dizi {
        tohum_istemi(&req.messages, TOHUM_AZAMI) + &yonerge
    } else {
        gonderilecek_istem
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
    let mut sonuc = ask(lease.client(), &secililer, &gonderilecek, st.timeout).await;

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
        model_kimligi(&saglayici)
    } else {
        req.model.clone()
    };

    match sonuc {
        Ok(metin) => yanit_olustur(&st, &req, metin, model_adi),
        // Ham neden zinciri ({e:#}) istemciye gider. Hatayi maskelemek, OmniRoute'un
        // gemini-web'de yaptigi ve teshisi imkansizlastiran seydi.
        Err(e) => hata(StatusCode::BAD_GATEWAY, &format!("{e:#}")),
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

fn hata(kod: StatusCode, mesaj: &str) -> Response {
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

/// Konusmanin parmak izi: **ilk kullanici mesajinin** ozeti.
///
/// OpenAI API'sinde oturum kimligi yok, istemciden yeni bir alan istemeden konusmalari
/// ayirt etmenin yolu icerikten tanimak. Ilk kullanici mesaji her turda gonderildigi
/// icin konusma boyunca degismez; farkli konusmalar farkli iz verir.
///
/// Neden yalniz o mesaj, sistem istemi de degil: sistem istemine tarih/saat gibi her
/// istekte degisen seyler girebiliyor. Onu ize katmak izi her turda degistirir, yani
/// sabitlemeyi sessizce ise yaramaz hale getirirdi.
///
/// Iki bilinen sinir: (a) ayni cumleyle baslayan iki ayri konusma ayni ize duser —
/// sonucu yalniz ayni hesaba yonlenmeleridir, yanlis yanit degil; (b) istemci gecmisi
/// yeniden yazarsa (Hermes'te compaction) iz degisir ve konusma baska hesaba tasinabilir.
pub fn konusma_izi(msgs: &[Message]) -> Option<u64> {
    use std::hash::{Hash, Hasher};
    let ilk = msgs.iter().find(|m| m.role == "user")?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    ilk.content.hash(&mut h);
    Some(h.finish())
}

/// Yalnizca son `user` mesaji — gerekcesi dosya basindaki durum gerilimi notunda.
pub fn son_kullanici_mesaji(msgs: &[Message]) -> Option<String> {
    // Araç turu: istemci çağrıları çalıştırıp sonuçlarını `tool` mesajı olarak geri
    // yolladı. Modele giden, son kullanıcı sorusu değil bu sonuçlardır.
    let sonuclar: Vec<_> = msgs.iter().rev().take_while(|m| m.role == "tool").collect();
    if !sonuclar.is_empty() {
        use kervan_akis::translator::{AracCagrisi, AracCagrisiAyiklayici as A};
        let parcalar: Vec<String> = sonuclar
            .iter()
            .rev()
            .map(|m| {
                let ad = m.name.clone().unwrap_or_else(|| "arac".into());
                let cagri = AracCagrisi::GenelFonksiyon {
                    ad,
                    parametreler_json: String::new(),
                };
                A::sistem_geri_bildirimi_uret(&cagri, &m.content)
            })
            .collect();
        return Some(parcalar.join("\n\n"));
    }
    msgs.iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.clone())
}

/// Tohumlanmis gecmis kac karakteri asmasin.
///
/// Istem kutusuna sigmayan bir metin gonderilemez; ustelik uzun tohum her turda
/// degil YALNIZCA dizi degistiginde gidiyor, yani bedeli seyrek.
pub const TOHUM_AZAMI: usize = 24_000;

/// Bos bir sohbet dizisine gonderilecek istem: gecmis + asil soru.
///
/// NEDEN: OpenAI API durumsuz, tarayici sohbeti durumlu. Normalde sureklilik
/// tarayicinin kendi dizisinden gelir ve yalnizca son mesaji gondermek yeter.
/// Ama dizi BOSSA — hesap degisti, kota bitti, tarayici bosta kapanip yeniden
/// acildi — model onceki turlari hic gormemis olur ve gecmissiz cevaplar.
///
/// Ozet cikarmaya gerek yok: istemci gecmisin TAMAMINI zaten her istekte
/// gonderiyor, biz atiyorduk. Elde olani kullanmak ek model cagrisindan,
/// ek kotadan ve ozetleme kaybindan ucuz.
///
/// Sistem mesaji da giriyor: tarayici arayuzunde sistem istemi diye ayri bir yer
/// yok, girmezse modelin yonergeleri kaybolur.
pub fn tohum_istemi(msgs: &[Message], azami: usize) -> String {
    let Some(son) = msgs.iter().rposition(|m| m.role == "user") else {
        return String::new();
    };
    let asil = &msgs[son].content;

    let etiket = |rol: &str| match rol {
        "system" => "Yonerge",
        "assistant" => "Sen",
        _ => "Ben",
    };

    // Sondan basa topla: kirpmak gerekirse EN ESKI turlar dussun, yakin baglam kalsin.
    let mut parcalar: Vec<String> = Vec::new();
    let mut uzunluk = 0usize;
    let mut atlanan = 0usize;
    for m in msgs[..son].iter().rev() {
        if m.content.trim().is_empty() {
            continue;
        }
        let p = format!("{}: {}", etiket(&m.role), m.content.trim());
        if uzunluk + p.chars().count() > azami {
            atlanan += 1;
            continue;
        }
        uzunluk += p.chars().count();
        parcalar.push(p);
    }
    if parcalar.is_empty() {
        return asil.clone();
    }
    parcalar.reverse();

    let mut s = String::from(
        "Bu konusma baska bir oturumdan devrediliyor; asagisi onceki turlar. \
         Okuyup baglam olarak kullan, onlara ayrica cevap verme.\n\n--- onceki turlar ---\n",
    );
    // Sessiz kirpma yok: eksik varsa yazili olsun.
    if atlanan > 0 {
        s.push_str(&format!(
            "(en eski {atlanan} mesaj uzunluk siniri nedeniyle atlandi)\n"
        ));
    }
    s.push_str(&parcalar.join("\n\n"));
    s.push_str("\n--- onceki turlar bitti ---\n\nSimdiki sorum:\n");
    s.push_str(asil);
    s
}

fn simdi() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub async fn serve(state: Arc<AppState>, port: u16) -> Result<()> {
    // 127.0.0.1 SART: 0.0.0.0 tum ag gecidini ve arkasindaki oturum acilmis
    // hesaplari yerel aga acardi. Plandaki guvenlik tabani, madde 3.
    let adres = format!("127.0.0.1:{port}");
    let dinleyici = tokio::net::TcpListener::bind(&adres).await?;
    println!("Kervan dinlemede: http://{adres}/v1");
    axum::serve(dinleyici, router(state)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
