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
use axum::extract::{Path, State};
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
        // Kota ogrenicinin gordugu: gercek kullanim ve gozlemden cikan limit.
        .route("/kota", get(kota_ozeti))
        .with_state(state)
}

async fn kota_ozeti(State(st): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let mut liste = Vec::new();
    for slot in st.pool.slots.iter() {
        let o = st.defter.ozet(&slot.hesap_adi).await;
        liste.push(serde_json::json!({ "hesap": slot.hesap_adi, "kullanim": o }));
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
    match st.pool.goster(slot.kimlik).await {
        Ok(true) => Json(serde_json::json!({
            "ok": true, "kimlik": slot.hesap_adi, "sekme": slot.provider.ad,
        }))
        .into_response(),
        // Acilmamis oturumu "gosterdim" demek yalan olurdu.
        Ok(false) => hata(
            StatusCode::CONFLICT,
            &format!("'{}' icin tarayici henuz acilmadi", slot.hesap_adi),
        ),
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
    let lease = match st.pool.acquire_for(konusma_izi(&req.messages), &saglayici).await {
        Ok(l) => l,
        Err(e) => return hata(StatusCode::SERVICE_UNAVAILABLE, &format!("{e:#}")),
    };
    println!("[{}/{}] istek ({} karakter)", saglayici, lease.name, istem.len());
    let secililer = lease.provider.selectors.clone();
    let mut sonuc = ask(lease.client(), &secililer, &istem, st.timeout).await;

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
    }

    // Sonucu havuza bildir: basari sayaci sifirlar, hata ustel geri cekilme uygular
    // ve dogrulama duvariysa hesabi insan kapisinda duraklatir.
    match (&kota_carpmasi, &sonuc) {
        (Some(metin), _) => {
            st.defter.yaz(&lease.name, Sonuc::Kota, Some(kirp(metin))).await;
            // Pencere ogrenildiyse onu, ogrenilmediyse muhafazakar varsayilani bekle.
            let ozet = st.defter.ozet(&lease.name).await;
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
            st.defter.yaz(&lease.name, Sonuc::Basari, None).await;
            st.pool.basarili(lease.index).await;
        }
        (None, Err(e)) => {
            st.defter.yaz(&lease.name, Sonuc::Hata, None).await;
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
