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

use anyhow::Result;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::browser::{ask, Selectors, Session};

/// Tarayici tek sekmede tek konusma yurutur; istekler siraya girmeli.
/// Mutex burada darbogaz degil, dogruluk sarti.
pub struct AppState {
    pub session: Mutex<Session>,
    pub selectors: Selectors,
    pub model_id: &'static str,
    pub timeout: std::time::Duration,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/v1/models", get(models))
        .route("/v1/chat/completions", post(chat))
        .route("/health", get(|| async { "ok" }))
        .with_state(state)
}

// ─── OpenAI bicimleri ────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ChatRequest {
    #[serde(default)]
    pub model: String,
    pub messages: Vec<Message>,
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
    Json(ModelList {
        object: "list",
        data: vec![ModelInfo {
            id: st.model_id.to_string(),
            object: "model",
            created: simdi(),
            owned_by: "kervan",
        }],
    })
}

async fn chat(State(st): State<Arc<AppState>>, Json(req): Json<ChatRequest>) -> Response {
    let Some(istem) = son_kullanici_mesaji(&req.messages) else {
        return hata(StatusCode::BAD_REQUEST, "messages icinde user rolu yok");
    };

    let oturum = st.session.lock().await;
    match ask(&oturum.client, &st.selectors, &istem, st.timeout).await {
        Ok(metin) => Json(ChatResponse {
            id: format!("chatcmpl-kervan-{}", simdi()),
            object: "chat.completion",
            created: simdi(),
            model: if req.model.is_empty() { st.model_id.to_string() } else { req.model },
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

fn hata(kod: StatusCode, mesaj: &str) -> Response {
    (kod, Json(serde_json::json!({ "error": { "message": mesaj } }))).into_response()
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
}
