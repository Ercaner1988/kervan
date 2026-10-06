//! `akis` — Gerçek zamanlı SSE akış yöneticisi.
//!
//! OpenAI, Anthropic Claude ve Google Gemini yukarı akışlarından SSE formatında
//! verileri gerçek zamanlı olarak okur, dönüştürür ve gecikmesiz iletir (FR-6.3, FR-6.4).

use std::io::{BufRead, BufReader, Read};
use std::sync::Arc;
use serde_json::Value;
use tokio::sync::{mpsc, oneshot, Mutex};

use crate::{temizle_hata, Hedef, Yukari, YukariHatasi};

pub async fn gonder_akis(
    yukari: &Yukari,
    hedef: &Hedef,
    istemci_govdesi: &Value,
    istenen_model: Option<&str>,
) -> Result<mpsc::Receiver<Result<String, YukariHatasi>>, YukariHatasi> {
    // 1. Anahtar kontrolü
    let anahtar = std::env::var(&hedef.anahtar_env)
        .map_err(|_| YukariHatasi::AnahtarYok(hedef.anahtar_env.clone()))?;

    if anahtar.trim().is_empty() {
        return Err(YukariHatasi::AnahtarYok(hedef.anahtar_env.clone()));
    }

    // 2. Dayanıklılık izni al
    {
        let mut d = yukari.direnc.lock().await;
        if let Err(e) = d.istek_izni_al(&hedef.ad) {
            return match e {
                kervan_direnc::DirencHatasi::DevreAcik { ad, kalan_sure_ms } => {
                    Err(YukariHatasi::DevreAcik { ad, kalan_sure_ms })
                }
                kervan_direnc::DirencHatasi::Sogumada { ad, retry_after_saniye } => {
                    Err(YukariHatasi::Sogumada { ad, retry_after_saniye })
                }
                kervan_direnc::DirencHatasi::YariAcikDolu(ad) => {
                    Err(YukariHatasi::DevreAcik { ad, kalan_sure_ms: 1000 })
                }
                kervan_direnc::DirencHatasi::IslemHatasi(m) => {
                    Err(YukariHatasi::AgHatasi(m))
                }
            };
        }
    }

    // 3. FR-6.4: Claude veya Gemini hedeflerde stream=true iken tools varsa açık hata ver
    if hedef.bicim != "openai" {
        if let Some(tools) = istemci_govdesi.get("tools").and_then(|t| t.as_array()) {
            if !tools.is_empty() {
                return Err(YukariHatasi::Desteklenmiyor("tools".to_string()));
            }
        }
    }

    let yukari_model = istenen_model.unwrap_or(&hedef.model).to_string();
    let (tx, rx) = mpsc::channel::<Result<String, YukariHatasi>>(32);
    let (init_tx, init_rx) = oneshot::channel::<Result<(), YukariHatasi>>();

    let ajan = yukari.ajan.clone();
    let hedef_kopya = hedef.clone();
    let govde_kopya = istemci_govdesi.clone();
    let direnc = yukari.direnc.clone();

    tokio::task::spawn_blocking(move || {
        match hedef_kopya.bicim.as_str() {
            "openai" => akis_openai(ajan, hedef_kopya, anahtar, govde_kopya, yukari_model, tx, init_tx, direnc),
            "claude" => akis_claude(ajan, hedef_kopya, anahtar, govde_kopya, yukari_model, tx, init_tx, direnc),
            "gemini" => akis_gemini(ajan, hedef_kopya, anahtar, govde_kopya, yukari_model, tx, init_tx, direnc),
            diger => {
                let _ = init_tx.send(Err(YukariHatasi::Desteklenmiyor(format!("Bilinmeyen biçim: {}", diger))));
            }
        }
    });

    match init_rx.await {
        Ok(Ok(())) => Ok(rx),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(YukariHatasi::AgHatasi("Akış başlatılamadı".to_string())),
    }
}

fn akis_openai(
    ajan: ureq::Agent,
    hedef: Hedef,
    anahtar: String,
    istemci_govdesi: Value,
    yukari_model: String,
    tx: mpsc::Sender<Result<String, YukariHatasi>>,
    init_tx: oneshot::Sender<Result<(), YukariHatasi>>,
    direnc: Arc<Mutex<kervan_direnc::DirencYoneticisi>>,
) {
    let mut yukari_govde = istemci_govdesi.clone();
    if let Some(obj) = yukari_govde.as_object_mut() {
        obj.insert("model".to_string(), Value::String(yukari_model));
        obj.insert("stream".to_string(), Value::Bool(true));
    }

    let yol = hedef.yol.as_deref().unwrap_or("/v1/chat/completions");
    let url = format!("{}{}", hedef.taban_url, yol);
    let auth_header = format!("Bearer {}", anahtar);

    let res = ajan
        .post(&url)
        .header("Authorization", &auth_header)
        .header("Content-Type", "application/json")
        .config()
        .http_status_as_error(false)
        .timeout_global(Some(hedef.zaman_asimi))
        .build()
        .send_json(&yukari_govde);

    let response = match res {
        Ok(r) => r,
        Err(e) => {
            direnc.blocking_lock().hata_kaydet(&hedef.ad, None, None);
            let _ = init_tx.send(Err(YukariHatasi::AgHatasi(temizle_hata(&e.to_string(), &anahtar))));
            return;
        }
    };

    let durum = response.status().as_u16();
    if durum < 200 || durum >= 300 {
        let retry_after = response.headers().get("retry-after")
            .or_else(|| response.headers().get("Retry-After"))
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok());
        let mut err_body = String::new();
        let _ = response.into_body().into_reader().read_to_string(&mut err_body);
        let temiz = temizle_hata(&err_body, &anahtar);
        direnc.blocking_lock().hata_kaydet(&hedef.ad, Some(durum), retry_after);
        let _ = init_tx.send(Err(YukariHatasi::HttpHatasi(durum, temiz, retry_after)));
        return;
    }

    let _ = init_tx.send(Ok(()));

    let reader = BufReader::new(response.into_body().into_reader());
    let mut gonderildi_done = false;

    for line_res in reader.lines() {
        match line_res {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if trimmed.starts_with("data:") {
                    if trimmed == "data: [DONE]" {
                        gonderildi_done = true;
                    }
                    if tx.blocking_send(Ok(format!("{}\n\n", trimmed))).is_err() {
                        return;
                    }
                }
            }
            Err(e) => {
                let _ = tx.blocking_send(Err(YukariHatasi::AgHatasi(temizle_hata(&e.to_string(), &anahtar))));
                return;
            }
        }
    }

    if !gonderildi_done {
        let _ = tx.blocking_send(Ok("data: [DONE]\n\n".to_string()));
    }
    direnc.blocking_lock().basari_kaydet(&hedef.ad);
}

fn akis_claude(
    ajan: ureq::Agent,
    hedef: Hedef,
    anahtar: String,
    istemci_govdesi: Value,
    yukari_model: String,
    tx: mpsc::Sender<Result<String, YukariHatasi>>,
    init_tx: oneshot::Sender<Result<(), YukariHatasi>>,
    direnc: Arc<Mutex<kervan_direnc::DirencYoneticisi>>,
) {
    let openai_istek: kervan_ceviri::OpenAiIstek = match serde_json::from_value(istemci_govdesi) {
        Ok(i) => i,
        Err(e) => {
            let _ = init_tx.send(Err(YukariHatasi::JsonHatasi(e.to_string())));
            return;
        }
    };

    let mut claude_istek = match kervan_ceviri::openai_den_claude_istegi(&openai_istek, &yukari_model) {
        Ok(c) => c,
        Err(e) => {
            let _ = init_tx.send(Err(YukariHatasi::Desteklenmiyor(e.to_string())));
            return;
        }
    };
    claude_istek.stream = true;

    let yol = hedef.yol.as_deref().unwrap_or("/v1/messages");
    let url = format!("{}{}", hedef.taban_url, yol);

    let res = ajan
        .post(&url)
        .header("x-api-key", &anahtar)
        .header("anthropic-version", "2023-06-01")
        .header("Content-Type", "application/json")
        .config()
        .http_status_as_error(false)
        .timeout_global(Some(hedef.zaman_asimi))
        .build()
        .send_json(&claude_istek);

    let response = match res {
        Ok(r) => r,
        Err(e) => {
            direnc.blocking_lock().hata_kaydet(&hedef.ad, None, None);
            let _ = init_tx.send(Err(YukariHatasi::AgHatasi(temizle_hata(&e.to_string(), &anahtar))));
            return;
        }
    };

    let durum = response.status().as_u16();
    if durum < 200 || durum >= 300 {
        let retry_after = response.headers().get("retry-after")
            .or_else(|| response.headers().get("Retry-After"))
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok());
        let mut err_body = String::new();
        let _ = response.into_body().into_reader().read_to_string(&mut err_body);
        let temiz = temizle_hata(&err_body, &anahtar);
        direnc.blocking_lock().hata_kaydet(&hedef.ad, Some(durum), retry_after);
        let _ = init_tx.send(Err(YukariHatasi::HttpHatasi(durum, temiz, retry_after)));
        return;
    }

    let _ = init_tx.send(Ok(()));

    let reader = BufReader::new(response.into_body().into_reader());
    let simdi = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let istek_id = format!("chatcmpl-claude-{}", simdi);

    for line_res in reader.lines() {
        match line_res {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if let Some(json_str) = trimmed.strip_prefix("data:") {
                    let json_str = json_str.trim();
                    if json_str == "[DONE]" {
                        break;
                    }
                    if let Ok(olay) = serde_json::from_str::<kervan_ceviri::ClaudeOlay>(json_str) {
                        let bitti = matches!(olay, kervan_ceviri::ClaudeOlay::MesajBitti);
                        if let Ok(Some(parca)) = kervan_ceviri::claude_dan_openai_parcasi(&olay, &istek_id, &yukari_model) {
                            if let Ok(parca_json) = serde_json::to_string(&parca) {
                                if tx.blocking_send(Ok(format!("data: {}\n\n", parca_json))).is_err() {
                                    return;
                                }
                            }
                        }
                        if bitti {
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                let _ = tx.blocking_send(Err(YukariHatasi::AgHatasi(temizle_hata(&e.to_string(), &anahtar))));
                return;
            }
        }
    }

    let _ = tx.blocking_send(Ok("data: [DONE]\n\n".to_string()));
    direnc.blocking_lock().basari_kaydet(&hedef.ad);
}

fn akis_gemini(
    ajan: ureq::Agent,
    hedef: Hedef,
    anahtar: String,
    istemci_govdesi: Value,
    yukari_model: String,
    tx: mpsc::Sender<Result<String, YukariHatasi>>,
    init_tx: oneshot::Sender<Result<(), YukariHatasi>>,
    direnc: Arc<Mutex<kervan_direnc::DirencYoneticisi>>,
) {
    let openai_istek: kervan_ceviri::OpenAiIstek = match serde_json::from_value(istemci_govdesi) {
        Ok(i) => i,
        Err(e) => {
            let _ = init_tx.send(Err(YukariHatasi::JsonHatasi(e.to_string())));
            return;
        }
    };

    let gemini_istek = match kervan_ceviri::openai_den_gemini_istegi(&openai_istek, &yukari_model) {
        Ok(g) => g,
        Err(e) => {
            let _ = init_tx.send(Err(YukariHatasi::Desteklenmiyor(e.to_string())));
            return;
        }
    };

    let varsayilan_yol = format!("/v1beta/models/{}:streamGenerateContent?alt=sse", yukari_model);
    let yol = hedef.yol.as_deref().unwrap_or(&varsayilan_yol);
    let url = format!("{}{}", hedef.taban_url, yol);

    let res = ajan
        .post(&url)
        .header("x-goog-api-key", &anahtar)
        .header("Content-Type", "application/json")
        .config()
        .http_status_as_error(false)
        .timeout_global(Some(hedef.zaman_asimi))
        .build()
        .send_json(&gemini_istek);

    let response = match res {
        Ok(r) => r,
        Err(e) => {
            direnc.blocking_lock().hata_kaydet(&hedef.ad, None, None);
            let _ = init_tx.send(Err(YukariHatasi::AgHatasi(temizle_hata(&e.to_string(), &anahtar))));
            return;
        }
    };

    let durum = response.status().as_u16();
    if durum < 200 || durum >= 300 {
        let retry_after = response.headers().get("retry-after")
            .or_else(|| response.headers().get("Retry-After"))
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok());
        let mut err_body = String::new();
        let _ = response.into_body().into_reader().read_to_string(&mut err_body);
        let temiz = temizle_hata(&err_body, &anahtar);
        direnc.blocking_lock().hata_kaydet(&hedef.ad, Some(durum), retry_after);
        let _ = init_tx.send(Err(YukariHatasi::HttpHatasi(durum, temiz, retry_after)));
        return;
    }

    let _ = init_tx.send(Ok(()));

    let reader = BufReader::new(response.into_body().into_reader());
    let simdi = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let istek_id = format!("chatcmpl-gemini-{}", simdi);

    for line_res in reader.lines() {
        match line_res {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if let Some(json_str) = trimmed.strip_prefix("data:") {
                    let json_str = json_str.trim();
                    if json_str == "[DONE]" {
                        break;
                    }
                    if let Ok(yanit) = serde_json::from_str::<kervan_ceviri::GeminiYanit>(json_str) {
                        if let Ok(Some(parca)) = kervan_ceviri::gemini_den_openai_parcasi(&yanit, &istek_id, &yukari_model) {
                            if let Ok(parca_json) = serde_json::to_string(&parca) {
                                if tx.blocking_send(Ok(format!("data: {}\n\n", parca_json))).is_err() {
                                    return;
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let _ = tx.blocking_send(Err(YukariHatasi::AgHatasi(temizle_hata(&e.to_string(), &anahtar))));
                return;
            }
        }
    }

    let _ = tx.blocking_send(Ok("data: [DONE]\n\n".to_string()));
    direnc.blocking_lock().basari_kaydet(&hedef.ad);
}
