//! `akis` — Gerçek zamanlı SSE akış yöneticisi.
//!
//! OpenAI, Anthropic Claude ve Google Gemini yukarı akışlarından SSE formatında
//! verileri gerçek zamanlı olarak okur, dönüştürür ve gecikmesiz iletir (FR-6.3, FR-6.4).

use serde_json::Value;
use std::io::{BufRead, BufReader, Read};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc, oneshot};

use crate::{Hedef, Yukari, YukariHatasi, temizle_hata};

/// Akışın son iletisinin (hata ya da `[DONE]`) gönderim sonucu. Alıcı düştüyse
/// istemci ayrılmıştır: iletinin gideceği, hatanın bildirileceği yer yoktur.
#[allow(
    clippy::let_underscore_must_use,
    reason = "alıcının ayrılması akışın olağan sonu; bildirilecek kimse kalmadı"
)]
fn alici_ayrildiysa_birak<E>(sonuc: Result<(), E>) {
    let _ = sonuc;
}

/// FR-5.3 / FR-6.4: Claude ve Gemini hedefte çevirinin taşımadığı alanlar (araç tanımı, araç
/// mesajı, çok parçalı içerik) anahtar, dayanıklılık izni ve ağdan ÖNCE açık hatayla reddedilir:
/// istemci hatası yarı-açık deneme yuvası tüketmez, eksik anahtar hatasının arkasına saklanmaz.
pub(crate) fn on_denetim(hedef: &Hedef, govde: &Value) -> Result<(), YukariHatasi> {
    if hedef.bicim == "openai" {
        return Ok(());
    }
    if let Some(tools) = govde.get("tools").and_then(|t| t.as_array())
        && !tools.is_empty()
    {
        return Err(YukariHatasi::Desteklenmiyor("tools".to_string()));
    }
    // Ayrıştırılamayan gövde burada geçer; çeviri adımı JsonHatasi verir.
    if let Ok(mut istek) = serde_json::from_value::<kervan_ceviri::OpenAiIstek>(govde.clone()) {
        istek.tools = None; // yukarıda ayrıca denetlendi
        istek
            .desteklenmeyeni_denetle()
            .map_err(YukariHatasi::from)?;
    }
    Ok(())
}

fn simdi_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Çevrilmiş parçaları istemciye SSE satırı olarak yollar; alıcı ayrıldıysa false.
fn parcalari_gonder(
    tx: &mpsc::Sender<Result<String, YukariHatasi>>,
    parcalar: Vec<kervan_ceviri::OpenAiParca>,
) -> bool {
    parcalar.iter().all(|p| match serde_json::to_string(p) {
        Ok(json) => tx.blocking_send(Ok(format!("data: {json}\n\n"))).is_ok(),
        Err(_) => true,
    })
}

/// İzin alınmış akış denemesi. İşçi hangi yoldan biterse bitsin (erken dönüş, istemci
/// ayrılması) düşerken yarı-açık deneme yuvasını bırakır. Koşulsuz güvenli: deneme_birak
/// yalnız devre hâlâ yarı-açıksa etki eder; başarı ya da hata kaydı devreyi zaten kapatır/açar.
struct DenemeYuvasi(Arc<Mutex<kervan_direnc::DirencYoneticisi>>, String);

impl Drop for DenemeYuvasi {
    fn drop(&mut self) {
        self.0.blocking_lock().notr_kaydet(&self.1);
    }
}

pub async fn gonder_akis(
    yukari: &Yukari,
    hedef: &Hedef,
    istemci_govdesi: &Value,
    istenen_model: Option<&str>,
) -> Result<mpsc::Receiver<Result<String, YukariHatasi>>, YukariHatasi> {
    // 0. Desteklenmeyen alanlar anahtar, izin ve ağdan önce
    on_denetim(hedef, istemci_govdesi)?;

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
                kervan_direnc::DirencHatasi::Sogumada {
                    ad,
                    retry_after_saniye,
                } => Err(YukariHatasi::Sogumada {
                    ad,
                    retry_after_saniye,
                }),
                kervan_direnc::DirencHatasi::YariAcikDolu(ad) => Err(YukariHatasi::DevreAcik {
                    ad,
                    kalan_sure_ms: 1000,
                }),
                kervan_direnc::DirencHatasi::IslemHatasi(m) => Err(YukariHatasi::AgHatasi(m)),
            };
        }
    }

    let yukari_model = istenen_model.unwrap_or(&hedef.model).to_string();
    let (tx, rx) = mpsc::channel::<Result<String, YukariHatasi>>(32);
    let (init_tx, init_rx) = oneshot::channel::<Result<(), YukariHatasi>>();

    let ajan = yukari.ajan.clone();
    let hedef_kopya = hedef.clone();
    let govde_kopya = istemci_govdesi.clone();
    let direnc = yukari.direnc.clone();

    tokio::task::spawn_blocking(move || match hedef_kopya.bicim.as_str() {
        "openai" => akis_openai(
            ajan,
            hedef_kopya,
            anahtar,
            govde_kopya,
            yukari_model,
            tx,
            init_tx,
            direnc,
        ),
        "claude" => akis_claude(
            ajan,
            hedef_kopya,
            anahtar,
            govde_kopya,
            yukari_model,
            tx,
            init_tx,
            direnc,
        ),
        "gemini" => akis_gemini(
            ajan,
            hedef_kopya,
            anahtar,
            govde_kopya,
            yukari_model,
            tx,
            init_tx,
            direnc,
        ),
        diger => {
            alici_ayrildiysa_birak(init_tx.send(Err(YukariHatasi::Desteklenmiyor(format!(
                "Bilinmeyen biçim: {}",
                diger
            )))));
        }
    });

    match init_rx.await {
        Ok(Ok(())) => Ok(rx),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(YukariHatasi::AgHatasi("Akış başlatılamadı".to_string())),
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "üç biçim aynı imzayı paylaşır; yapıya toplamak tek kullanımlık soyutlama olurdu"
)]
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
    let _deneme = DenemeYuvasi(direnc.clone(), hedef.ad.clone());
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
            alici_ayrildiysa_birak(init_tx.send(Err(YukariHatasi::AgHatasi(temizle_hata(
                &e.to_string(),
                &anahtar,
            )))));
            return;
        }
    };

    let durum = response.status().as_u16();
    if !(200..300).contains(&durum) {
        let retry_after = response
            .headers()
            .get("retry-after")
            .or_else(|| response.headers().get("Retry-After"))
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok());
        let mut err_body = String::new();
        if let Err(e) = response
            .into_body()
            .into_reader()
            .read_to_string(&mut err_body)
        {
            err_body = format!("(hata gövdesi okunamadı: {e})");
        }
        let temiz = temizle_hata(&err_body, &anahtar);
        direnc
            .blocking_lock()
            .hata_kaydet(&hedef.ad, Some(durum), retry_after);
        alici_ayrildiysa_birak(init_tx.send(Err(YukariHatasi::HttpHatasi(
            durum,
            temiz,
            retry_after,
        ))));
        return;
    }

    if init_tx.send(Ok(())).is_err() {
        // Çağıran ayrıldı: yukarı akışı okumanın alıcısı yok.
        return;
    }

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
                // Akış ortası okuma hatası ağ hatasıdır: kesiciyi besler.
                direnc.blocking_lock().hata_kaydet(&hedef.ad, None, None);
                alici_ayrildiysa_birak(tx.blocking_send(Err(YukariHatasi::AgHatasi(
                    temizle_hata(&e.to_string(), &anahtar),
                ))));
                return;
            }
        }
    }

    if !gonderildi_done {
        alici_ayrildiysa_birak(tx.blocking_send(Ok("data: [DONE]\n\n".to_string())));
    }
    direnc.blocking_lock().basari_kaydet(&hedef.ad);
}

#[allow(
    clippy::too_many_arguments,
    reason = "üç biçim aynı imzayı paylaşır; yapıya toplamak tek kullanımlık soyutlama olurdu"
)]
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
    let _deneme = DenemeYuvasi(direnc.clone(), hedef.ad.clone());
    let openai_istek: kervan_ceviri::OpenAiIstek = match serde_json::from_value(istemci_govdesi) {
        Ok(i) => i,
        Err(e) => {
            alici_ayrildiysa_birak(init_tx.send(Err(YukariHatasi::JsonHatasi(e.to_string()))));
            return;
        }
    };

    let mut claude_istek =
        match kervan_ceviri::openai_den_claude_istegi(&openai_istek, &yukari_model) {
            Ok(c) => c,
            Err(e) => {
                alici_ayrildiysa_birak(init_tx.send(Err(YukariHatasi::from(e))));
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
            alici_ayrildiysa_birak(init_tx.send(Err(YukariHatasi::AgHatasi(temizle_hata(
                &e.to_string(),
                &anahtar,
            )))));
            return;
        }
    };

    let durum = response.status().as_u16();
    if !(200..300).contains(&durum) {
        let retry_after = response
            .headers()
            .get("retry-after")
            .or_else(|| response.headers().get("Retry-After"))
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok());
        let mut err_body = String::new();
        if let Err(e) = response
            .into_body()
            .into_reader()
            .read_to_string(&mut err_body)
        {
            err_body = format!("(hata gövdesi okunamadı: {e})");
        }
        let temiz = temizle_hata(&err_body, &anahtar);
        direnc
            .blocking_lock()
            .hata_kaydet(&hedef.ad, Some(durum), retry_after);
        alici_ayrildiysa_birak(init_tx.send(Err(YukariHatasi::HttpHatasi(
            durum,
            temiz,
            retry_after,
        ))));
        return;
    }

    if init_tx.send(Ok(())).is_err() {
        // Çağıran ayrıldı: yukarı akışı okumanın alıcısı yok.
        return;
    }

    let reader = BufReader::new(response.into_body().into_reader());
    let mut cevirici = kervan_ceviri::ClaudeAkisCevirici::default();

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
                        if let kervan_ceviri::ClaudeOlay::Hata { error } = &olay {
                            // Yukarı akış akışı hatayla kesti: istemciye hata, [DONE] yok,
                            // başarı değil; kesiciyi besler (aşırı yük vb. sunucu tarafı).
                            direnc.blocking_lock().hata_kaydet(&hedef.ad, None, None);
                            alici_ayrildiysa_birak(tx.blocking_send(Err(YukariHatasi::AgHatasi(
                                format!(
                                    "Claude akış hatası: {}",
                                    temizle_hata(&error.to_string(), &anahtar)
                                ),
                            ))));
                            return;
                        }
                        let bitti = matches!(olay, kervan_ceviri::ClaudeOlay::MesajBitti);
                        if !parcalari_gonder(&tx, cevirici.cevir(&olay, simdi_ms() / 1000)) {
                            return;
                        }
                        if bitti {
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                // Akış ortası okuma hatası ağ hatasıdır: kesiciyi besler.
                direnc.blocking_lock().hata_kaydet(&hedef.ad, None, None);
                alici_ayrildiysa_birak(tx.blocking_send(Err(YukariHatasi::AgHatasi(
                    temizle_hata(&e.to_string(), &anahtar),
                ))));
                return;
            }
        }
    }

    alici_ayrildiysa_birak(tx.blocking_send(Ok("data: [DONE]\n\n".to_string())));
    direnc.blocking_lock().basari_kaydet(&hedef.ad);
}

#[allow(
    clippy::too_many_arguments,
    reason = "üç biçim aynı imzayı paylaşır; yapıya toplamak tek kullanımlık soyutlama olurdu"
)]
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
    let _deneme = DenemeYuvasi(direnc.clone(), hedef.ad.clone());
    let openai_istek: kervan_ceviri::OpenAiIstek = match serde_json::from_value(istemci_govdesi) {
        Ok(i) => i,
        Err(e) => {
            alici_ayrildiysa_birak(init_tx.send(Err(YukariHatasi::JsonHatasi(e.to_string()))));
            return;
        }
    };

    let gemini_istek = match kervan_ceviri::openai_den_gemini_istegi(&openai_istek, &yukari_model) {
        Ok(g) => g,
        Err(e) => {
            alici_ayrildiysa_birak(init_tx.send(Err(YukariHatasi::from(e))));
            return;
        }
    };

    let varsayilan_yol = format!(
        "/v1beta/models/{}:streamGenerateContent?alt=sse",
        yukari_model
    );
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
            alici_ayrildiysa_birak(init_tx.send(Err(YukariHatasi::AgHatasi(temizle_hata(
                &e.to_string(),
                &anahtar,
            )))));
            return;
        }
    };

    let durum = response.status().as_u16();
    if !(200..300).contains(&durum) {
        let retry_after = response
            .headers()
            .get("retry-after")
            .or_else(|| response.headers().get("Retry-After"))
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok());
        let mut err_body = String::new();
        if let Err(e) = response
            .into_body()
            .into_reader()
            .read_to_string(&mut err_body)
        {
            err_body = format!("(hata gövdesi okunamadı: {e})");
        }
        let temiz = temizle_hata(&err_body, &anahtar);
        direnc
            .blocking_lock()
            .hata_kaydet(&hedef.ad, Some(durum), retry_after);
        alici_ayrildiysa_birak(init_tx.send(Err(YukariHatasi::HttpHatasi(
            durum,
            temiz,
            retry_after,
        ))));
        return;
    }

    if init_tx.send(Ok(())).is_err() {
        // Çağıran ayrıldı: yukarı akışı okumanın alıcısı yok.
        return;
    }

    let reader = BufReader::new(response.into_body().into_reader());
    let mut cevirici = kervan_ceviri::GeminiAkisCevirici::default();

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
                    if let Ok(yanit) = serde_json::from_str::<kervan_ceviri::GeminiYanit>(json_str)
                        && !parcalari_gonder(&tx, cevirici.cevir(&yanit, simdi_ms()))
                    {
                        return;
                    }
                }
            }
            Err(e) => {
                // Akış ortası okuma hatası ağ hatasıdır: kesiciyi besler.
                direnc.blocking_lock().hata_kaydet(&hedef.ad, None, None);
                alici_ayrildiysa_birak(tx.blocking_send(Err(YukariHatasi::AgHatasi(
                    temizle_hata(&e.to_string(), &anahtar),
                ))));
                return;
            }
        }
    }

    alici_ayrildiysa_birak(tx.blocking_send(Ok("data: [DONE]\n\n".to_string())));
    direnc.blocking_lock().basari_kaydet(&hedef.ad);
}
