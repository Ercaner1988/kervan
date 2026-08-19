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

use anyhow::Result;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use crate::browser::ask;
use crate::kota::{self, Defter, Sonuc};
use crate::pool::Pool;

pub struct AppState {
    pub pool: Pool,
    pub defter: Defter,
    pub kota_ayar: crate::config::Kota,
    pub saglayicilar: std::collections::HashMap<String, std::sync::Arc<crate::browser::Provider>>,
    /// Model istenmediginde kullanilacak saglayici (yapilandirmadaki ilk hesabinki).
    pub varsayilan_saglayici: String,
    pub timeout: std::time::Duration,
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

async fn goster(State(st): State<Arc<AppState>>, Path(i): Path<usize>) -> Response {
    let Some(slot) = st.pool.slots.get(i) else {
        return hata(StatusCode::NOT_FOUND, &format!("{i} numarali hesap yok"));
    };
    kimlik_goster(State(st.clone()), Path(slot.kimlik), Query(GosterSorgu::default())).await
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
    for ad in q.sekmeler.as_deref().unwrap_or("").split(',').map(str::trim).filter(|s| !s.is_empty())
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
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
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
    // Yalniz HESABI OLAN saglayicilar listelenir. Hesapsiz bir modeli listelemek,
    // istemciye var olmayan bir yol gostermek olurdu — OmniRoute'un katalogunda
    // gercekte calismayan modellerin durmasi tam olarak bu hataydi.
    let mut adlar: Vec<String> =
        st.pool.slots.iter().map(|s| s.provider.ad.clone()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
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

    // Model kimliginden saglayiciyi sec. Taninmayan bir model SESSIZCE varsayilana
    // dusmemeli: istemci claude isteyip gemini cevabi alirsa bunu fark edemez.
    let saglayici = if req.model.trim().is_empty() {
        st.varsayilan_saglayici.clone()
    } else {
        match crate::saglayicilar::model_saglayicisi(&req.model, &st.saglayicilar) {
            Some(s) => s,
            None => {
                let mevcut: Vec<String> = st
                    .pool
                    .slots
                    .iter()
                    .map(|s| model_kimligi(&s.provider.ad))
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect();
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

    // Havuzdan hesap kirala: konusma kendi hesabina sabitlenir, hiz butcesi burada
    // uygulanir, gerekirse beklenir.
    let mut lease = match st.pool.acquire_for(konusma_izi(&req.messages), &saglayici).await {
        Ok(l) => l,
        Err(e) => return hata(StatusCode::SERVICE_UNAVAILABLE, &format!("{e:#}")),
    };
    // Dizi bossa gecmisi TOHUMLA. Devam eden dizide tohumlamak gecmisi ikinci kez
    // yazmak olurdu; bos dizide tohumlamamak ise modele gecmissiz cevap verdirir.
    let gonderilecek = if lease.yeni_dizi {
        tohum_istemi(&req.messages, TOHUM_AZAMI)
    } else {
        istem.clone()
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
            println!("[{}] oturum insanin elinde: dogrulamayi coz, sonra istegi tekrarla.", lease.name);
        }
    }

    // Sonucu havuza bildir: basari sayaci sifirlar, hata ustel geri cekilme uygular
    // ve dogrulama duvariysa hesabi insan kapisinda duraklatir.
    match (&kota_carpmasi, &sonuc) {
        (Some(metin), _) => {
            st.defter.yaz(&lease.name, &lease.provider.ad, Sonuc::Kota, Some(kirp(metin))).await;
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
            st.pool.kota_doldu(lease.index, Duration::from_secs(dk * 60)).await;
        }
        (None, Ok(_)) => {
            st.defter.yaz(&lease.name, &lease.provider.ad, Sonuc::Basari, None).await;
            st.pool.basarili(lease.index).await;
        }
        (None, Err(e)) => {
            st.defter.yaz(&lease.name, &lease.provider.ad, Sonuc::Hata, None).await;
            st.pool.basarisiz(lease.index, &format!("{e:#}")).await;
        }
    }

    let model_adi =
        if req.model.is_empty() { model_kimligi(&saglayici) } else { req.model.clone() };

    if req.stream {
        return match sonuc {
            Ok(metin) => sse_yanit(&metin, &model_adi),
            Err(e) => hata(StatusCode::BAD_GATEWAY, &format!("{e:#}")),
        };
    }

    match sonuc {
        Ok(metin) => Json(ChatResponse {
            id: format!("chatcmpl-kervan-{}", simdi()),
            object: "chat.completion",
            created: simdi(),
            model: model_adi,
            choices: vec![Choice {
                index: 0,
                message: Message { role: "assistant".into(), content: metin },
                finish_reason: "stop",
            }],
            usage: Usage { prompt_tokens: 0, completion_tokens: 0, total_tokens: 0 },
        })
        .into_response(),
        // Ham neden zinciri ({e:#}) istemciye gider. Hatayi maskelemek, OmniRoute'un
        // gemini-web'de yaptigi ve teshisi imkansizlastiran seydi.
        Err(e) => hata(StatusCode::BAD_GATEWAY, &format!("{e:#}")),
    }
}

/// Kota mesajindan deftere yazilacak kisa kesit. Amac gercek ifadeyi ogrenmek,
/// sohbet biriktirmek degil.
fn kirp(m: &str) -> String {
    m.chars().take(200).collect()
}

fn hata(kod: StatusCode, mesaj: &str) -> Response {
    (kod, Json(serde_json::json!({ "error": { "message": mesaj } }))).into_response()
}

/// OpenAI uyumlu SSE akisi.
///
/// ponytail: tarayici bize metni parca parca degil, tamamlandiginda veriyor
/// (bitisi kararlilikla algiliyoruz), o yuzden tek parca + [DONE] gonderiyoruz.
/// Protokol acisindan gecerli; istemci akis bekledigi icin bu sart.
/// Yukseltme yolu: `ask` icindeki yoklama dongusu zaten metnin BUYUDUGUNU goruyor —
/// oradan bir kanal acilirsa gercek artimli akis neredeyse bedava gelir. Ihtiyac
/// olunca yap; su an tek parca yeterli ve yanlis degil.
fn sse_yanit(metin: &str, model: &str) -> Response {
    let id = format!("chatcmpl-kervan-{}", simdi());
    let olustu = simdi();
    let parca = serde_json::json!({
        "id": id, "object": "chat.completion.chunk", "created": olustu, "model": model,
        "choices": [{ "index": 0, "delta": { "role": "assistant", "content": metin } }]
    });
    let bitis = serde_json::json!({
        "id": id, "object": "chat.completion.chunk", "created": olustu, "model": model,
        "choices": [{ "index": 0, "delta": {}, "finish_reason": "stop" }]
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
fn konusma_izi(msgs: &[Message]) -> Option<u64> {
    use std::hash::{Hash, Hasher};
    let ilk = msgs.iter().find(|m| m.role == "user")?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    ilk.content.hash(&mut h);
    Some(h.finish())
}

/// Yalnizca son `user` mesaji — gerekcesi dosya basindaki durum gerilimi notunda.
fn son_kullanici_mesaji(msgs: &[Message]) -> Option<String> {
    msgs.iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.clone())
}

/// Tohumlanmis gecmis kac karakteri asmasin.
///
/// Istem kutusuna sigmayan bir metin gonderilemez; ustelik uzun tohum her turda
/// degil YALNIZCA dizi degistiginde gidiyor, yani bedeli seyrek.
const TOHUM_AZAMI: usize = 24_000;

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
fn tohum_istemi(msgs: &[Message], azami: usize) -> String {
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
        s.push_str(&format!("(en eski {atlanan} mesaj uzunluk siniri nedeniyle atlandi)\n"));
    }
    s.push_str(&parcalar.join("\n\n"));
    s.push_str("\n--- onceki turlar bitti ---\n\nSimdiki sorum:\n");
    s.push_str(asil);
    s
}

fn simdi() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
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
        Message { role: role.into(), content: content.into() }
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
        assert!(t.contains("Kisa cevap ver."), "sistem yonergesi dusmemeli: {t}");
        assert!(t.contains("baskenti"), "onceki soru olmali");
        assert!(t.contains("Ankara."), "onceki cevap olmali");
        assert!(t.ends_with("Peki nufusu?"), "asil soru SONDA olmali: {t}");
    }

    /// Tek mesajlik konusmada tohumlanacak gecmis yok — istem oldugu gibi gitmeli,
    /// basina gereksiz aciklama eklenmemeli.
    #[test]
    fn tek_mesajda_tohum_sade_kalir() {
        assert_eq!(tohum_istemi(&[m("user", "merhaba")], TOHUM_AZAMI), "merhaba");
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
        let a = vec![m("system", "tarih: 2026-08-15 10:00"), m("user", "ilk soru")];
        let b = vec![m("system", "tarih: 2026-08-15 10:42"), m("user", "ilk soru")];
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
