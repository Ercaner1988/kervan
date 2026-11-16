//! `kervan-yukari` — REST yukarı akış istemcisi.
//!
//! OpenAI, Claude ve Gemini uyumlu REST sağlayıcılarına HTTP üzerinden
//! kayıpsız istek iletimi, yanıt alımı, devre kesici (circuit breaker),
//! soğuma takipçisi ve hata temizliğini sağlar.

use serde_json::Value;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hedef {
    pub ad: String,
    pub bicim: String,
    pub taban_url: String,
    pub anahtar_env: String,
    pub model: String,
    pub yol: Option<String>,
    pub zaman_asimi: Duration,
}

impl Hedef {
    pub fn rest_ayardan(
        ad: &str,
        ayar: &kervan_tarayici::config::RestAyar,
        varsayilan_timeout: Duration,
    ) -> Self {
        Self {
            ad: ad.to_string(),
            bicim: ayar.bicim.clone(),
            taban_url: ayar.taban_url.trim_end_matches('/').to_string(),
            anahtar_env: ayar.anahtar_env.clone(),
            model: ayar.model.clone(),
            yol: ayar.yol.clone(),
            zaman_asimi: varsayilan_timeout,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum YukariHatasi {
    AnahtarYok(String),
    Desteklenmiyor(String),
    HttpHatasi(u16, String, Option<u64>),
    AgHatasi(String),
    JsonHatasi(String),
    DevreAcik { ad: String, kalan_sure_ms: u64 },
    Sogumada { ad: String, retry_after_saniye: u64 },
}

/// Çeviri hatası: desteklenmeyen alan adıyla iletilir (yeniden sarılmaz), diğerleri istemci JSON hatası.
impl From<kervan_ceviri::CeviriHatasi> for YukariHatasi {
    fn from(e: kervan_ceviri::CeviriHatasi) -> Self {
        match e {
            kervan_ceviri::CeviriHatasi::Desteklenmiyor(alan) => Self::Desteklenmiyor(alan),
            kervan_ceviri::CeviriHatasi::GecersizIstek(m)
            | kervan_ceviri::CeviriHatasi::AyrirmaHatasi(m) => Self::JsonHatasi(m),
        }
    }
}

impl fmt::Display for YukariHatasi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AnahtarYok(env_ad) => {
                write!(
                    f,
                    "Sağlayıcı API anahtarı bulunamadı: Ortam değişkeni '{}' tanımlı değil",
                    env_ad
                )
            }
            Self::Desteklenmiyor(alan) => {
                write!(f, "Yukarı akış için desteklenmeyen alan: {}", alan)
            }
            Self::HttpHatasi(durum, govde, retry_after) => {
                if let Some(sn) = retry_after {
                    write!(
                        f,
                        "Yukarı akış HTTP hatası ({}) [Retry-After: {}s]: {}",
                        durum, sn, govde
                    )
                } else {
                    write!(f, "Yukarı akış HTTP hatası ({}): {}", durum, govde)
                }
            }
            Self::AgHatasi(mesaj) => {
                write!(f, "Yukarı akış ağ bağlantı hatası: {}", mesaj)
            }
            Self::JsonHatasi(mesaj) => {
                write!(f, "Yukarı akış JSON ayrıştırma hatası: {}", mesaj)
            }
            Self::DevreAcik { ad, kalan_sure_ms } => {
                write!(
                    f,
                    "Devre kesici açık: {} ({}ms sonra tekrar denenecek)",
                    ad, kalan_sure_ms
                )
            }
            Self::Sogumada {
                ad,
                retry_after_saniye,
            } => {
                write!(
                    f,
                    "Sağlayıcı soğumada: {} (Retry-After: {}s)",
                    ad, retry_after_saniye
                )
            }
        }
    }
}

impl std::error::Error for YukariHatasi {}

/// Hata mesajındaki gizli anahtarları, başlıkları ve iç sistem yollarını temizler.
pub fn temizle_hata(govde: &str, gizli_anahtar: &str) -> String {
    let mut temiz = govde.to_string();

    // 1. Bilinen API anahtarını maskele
    let trimmed = gizli_anahtar.trim();
    if !trimmed.is_empty() {
        temiz = temiz.replace(trimmed, "[GIZLENDI]");
    }

    // 2. Bearer token ve authorization kalıplarını maskele
    if let Ok(re_bearer) = regex::Regex::new(r"(?i)bearer\s+([a-zA-Z0-9_\-\.]{8,})") {
        temiz = re_bearer
            .replace_all(&temiz, "Bearer [GIZLENDI]")
            .to_string();
    }
    if let Ok(re_key) = regex::Regex::new(
        r#"(?i)(api[_-]?key|token|secret)["']?\s*[:=]\s*["']?([a-zA-Z0-9_\-\.]{8,})"#,
    ) {
        temiz = re_key.replace_all(&temiz, "$1=[GIZLENDI]").to_string();
    }

    // 3. Dosya yollarını maskele (Windows ve Unix iç yolları)
    if let Ok(re_win) = regex::Regex::new(r#"[a-zA-Z]:[\\/][^"'\s\r\n\t]+"#) {
        temiz = re_win.replace_all(&temiz, "[GIZLI_YOL]").to_string();
    }
    if let Ok(re_unix) = regex::Regex::new(r#"/(home|Users|var|etc|usr|tmp)/[^"'\s\r\n\t]+"#) {
        temiz = re_unix.replace_all(&temiz, "[GIZLI_YOL]").to_string();
    }

    // 4. Kısaltma (azami 500 bayt, karakter sınırında: çok baytlı harfin ortasından kesilmez)
    if temiz.len() > 500 {
        format!("{}...", &temiz[..temiz.floor_char_boundary(500)])
    } else {
        temiz
    }
}

pub mod akis;

/// Yukarı akış anahtarını çözen işlev: ortam değişkeni adı → anahtar değeri.
///
/// Varsayılan çözücü önce bu oturumun kasasını, sonra süreç ortamını okur;
/// anahtar hiçbir zaman yapılandırma dosyasına ya da diske yazılmaz. Dikiş,
/// testin anahtarı süreç ortamına yazmadan (Rust 2024'te bu `unsafe`'tir) aynı
/// yolu sınamasına izin verir.
pub type AnahtarCozucu = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// Panelden girilen anahtarların yalnız bellekte durduğu kasa.
///
/// NEDEN DİSK DEĞİL: `kervan.toml` bir yapılandırma dosyasıdır ve yedeklenir,
/// sürüm denetimine girebilir, paylaşılabilir. Gizli anahtarı oraya yazmak
/// onu sıradan bir ayar gibi dağıtırdı. Bu yüzden anahtar süreç ömrüyle
/// sınırlıdır: Kervan kapanınca kasa boşalır, kullanıcı yeniden girer.
///
/// NEDEN `std::env::set_var` DEĞİL: Rust 2024'te `unsafe`'tir ve bu süreç
/// çok iş parçacıklıdır (sunucu, pano yoklaması, arayüz). Ortamı yazmak veri
/// yarışı doğururdu; kasa ise kendi kilidiyle korunur.
static OTURUM_KASASI: std::sync::OnceLock<
    std::sync::RwLock<std::collections::HashMap<String, String>>,
> = std::sync::OnceLock::new();

fn oturum_kasasi() -> &'static std::sync::RwLock<std::collections::HashMap<String, String>> {
    OTURUM_KASASI.get_or_init(|| std::sync::RwLock::new(std::collections::HashMap::new()))
}

/// Kilit zehirlenmişse içeriği kurtarır: anahtar çözümü, başka bir iş
/// parçacığının çökmesi yüzünden tümden durmamalı. Kasa yalnız anahtar/değer
/// çifti tuttuğu için yarım kalmış bir yazma tutarsızlık üretmez.
fn kasa_oku() -> std::sync::RwLockReadGuard<'static, std::collections::HashMap<String, String>> {
    oturum_kasasi().read().unwrap_or_else(|e| e.into_inner())
}

fn kasa_yaz() -> std::sync::RwLockWriteGuard<'static, std::collections::HashMap<String, String>> {
    oturum_kasasi().write().unwrap_or_else(|e| e.into_inner())
}

/// Anahtarı bu oturumun kasasına koyar. `false` dönerse hiçbir şey yazılmadı.
///
/// Ad ve değer boşluklardan arındırılır: yapıştırma artığı bir boşluk,
/// "tanımlı ama geçersiz" bir anahtar üretirdi. Farklı bir ad/değerle aynı
/// anahtar yeniden girilirse eskisi ezilir — panelde son girilen doğrudur.
pub fn oturum_anahtari_ekle(ad: &str, deger: &str) -> bool {
    let ad = ad.trim();
    let deger = deger.trim();
    if ad.is_empty() || deger.is_empty() {
        return false;
    }
    kasa_yaz().insert(ad.to_string(), deger.to_string());
    true
}

/// Bu oturumda girilmiş anahtarı siler. `true` dönerse kasa değişti.
///
/// Gerekli: panelden girilen yanlış bir anahtar, süreç kapanana kadar ortam
/// değişkenini gölgeler. Kaldırma yolu olmadan kullanıcı Kervan'ı yeniden
/// başlatmaya zorlanırdı.
pub fn oturum_anahtari_sil(ad: &str) -> bool {
    kasa_yaz().remove(ad.trim()).is_some()
}

/// Anahtar bu oturumda panelden mi girilmiş (ortamdan değil)?
pub fn oturum_anahtari_var(ad: &str) -> bool {
    kasa_oku().contains_key(ad.trim())
}

/// Anahtarı çözer: önce bu oturumun kasası, sonra süreç ortamı.
///
/// Sıra bilinçli: panelde girilen değer, o oturum için en güncel bilgidir ve
/// ortam değişkeni eskiyse kullanıcının gözü önünde düzeltebildiği değer
/// kazanmalı. Tek çözüm noktası burasıdır; `Yukari` ve panel aynı sırayı
/// ayrı ayrı yazmaz.
pub fn anahtar_coz(ad: &str) -> Option<String> {
    let ad = ad.trim();
    if ad.is_empty() {
        return None;
    }
    if let Some(deger) = kasa_oku().get(ad) {
        return Some(deger.clone());
    }
    std::env::var(ad).ok()
}

fn ortamdan_anahtar(ad: &str) -> Option<String> {
    anahtar_coz(ad)
}

#[derive(Clone)]
pub struct Yukari {
    pub(crate) ajan: ureq::Agent,
    pub direnc: Arc<Mutex<kervan_direnc::DirencYoneticisi>>,
    pub(crate) anahtar_cozucu: AnahtarCozucu,
}

impl Default for Yukari {
    fn default() -> Self {
        Self::new()
    }
}

impl Yukari {
    pub fn new() -> Self {
        Self::yeni_direncli(kervan_direnc::DirencYoneticisi::default())
    }

    pub fn yeni_direncli(direnc: kervan_direnc::DirencYoneticisi) -> Self {
        Self::yeni_cozuculu(direnc, Arc::new(ortamdan_anahtar))
    }

    /// Direnç yöneticisi ve anahtar çözücüyü birlikte verir.
    ///
    /// Çözücü, ortamı okumayan bir işlevle değiştirilebilir; üretimde
    /// `Yukari::new` ile aynı davranışı (`ortamdan_anahtar`) korur.
    pub fn yeni_cozuculu(direnc: kervan_direnc::DirencYoneticisi, cozucu: AnahtarCozucu) -> Self {
        let config = ureq::config::Config::builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(60)))
            .build();
        let ajan = ureq::Agent::new_with_config(config);
        Self {
            ajan,
            direnc: Arc::new(Mutex::new(direnc)),
            anahtar_cozucu: cozucu,
        }
    }

    /// Hedefin anahtarını çözücüden okur; yoksa ya da boşsa `AnahtarYok`.
    pub(crate) fn anahtar(&self, hedef: &Hedef) -> Result<String, YukariHatasi> {
        let anahtar = (self.anahtar_cozucu)(&hedef.anahtar_env)
            .ok_or_else(|| YukariHatasi::AnahtarYok(hedef.anahtar_env.clone()))?;
        if anahtar.trim().is_empty() {
            return Err(YukariHatasi::AnahtarYok(hedef.anahtar_env.clone()));
        }
        Ok(anahtar)
    }

    /// OpenAI-biçimli istemci gövdesini yukarı akışa kayıpsız iletir ve yanıtı döner.
    pub async fn gonder(
        &self,
        hedef: &Hedef,
        istemci_govdesi: &Value,
        istenen_model: Option<&str>,
    ) -> Result<Value, YukariHatasi> {
        // 0. Desteklenmeyen alanlar anahtar, izin ve ağdan önce
        akis::on_denetim(hedef, istemci_govdesi)?;

        // 1. Anahtarı çözücüden oku (üretimde ortam değişkeni)
        let anahtar = self.anahtar(hedef)?;

        // 2. Dayanıklılık izni al (devre kesici ve soğuma kontrolü)
        {
            let mut d = self.direnc.lock().await;
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

        // 3. Yukarı akış modelini belirle
        let yukari_model = istenen_model.unwrap_or(&hedef.model);

        // 4. İsteği gönder
        let sonuc = match hedef.bicim.as_str() {
            "openai" => {
                self.gonder_openai(hedef, &anahtar, istemci_govdesi, yukari_model)
                    .await
            }
            "claude" => {
                self.gonder_claude(hedef, &anahtar, istemci_govdesi, yukari_model)
                    .await
            }
            "gemini" => {
                self.gonder_gemini(hedef, &anahtar, istemci_govdesi, yukari_model)
                    .await
            }
            diger => Err(YukariHatasi::Desteklenmiyor(format!(
                "Bilinmeyen biçim: {}",
                diger
            ))),
        };

        // 5. Dayanıklılık durumunu güncelle
        {
            let mut d = self.direnc.lock().await;
            match &sonuc {
                Ok(_) => {
                    d.basari_kaydet(&hedef.ad);
                }
                Err(YukariHatasi::HttpHatasi(durum, _, retry_after)) => {
                    let durum = Some(*durum);
                    let retry_after = *retry_after;
                    d.hata_kaydet(&hedef.ad, durum, retry_after);
                }
                Err(YukariHatasi::AgHatasi(_)) => {
                    d.hata_kaydet(&hedef.ad, None, None);
                }
                // Anahtar, JSON, desteklenmeyen alan: kesiciyi beslemez, yarı-açık yuvasını bırakır.
                Err(_) => d.notr_kaydet(&hedef.ad),
            }
        }

        sonuc
    }

    /// OpenAI-biçimli istemci gövdesini yukarı akışa akışlı (SSE) olarak iletir ve parçaları döner.
    pub async fn gonder_akis(
        &self,
        hedef: &Hedef,
        istemci_govdesi: &Value,
        istenen_model: Option<&str>,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String, YukariHatasi>>, YukariHatasi> {
        akis::gonder_akis(self, hedef, istemci_govdesi, istenen_model).await
    }

    async fn gonder_openai(
        &self,
        hedef: &Hedef,
        anahtar: &str,
        istemci_govdesi: &Value,
        yukari_model: &str,
    ) -> Result<Value, YukariHatasi> {
        let mut yukari_govde = istemci_govdesi.clone();
        if let Some(obj) = yukari_govde.as_object_mut() {
            obj.insert("model".to_string(), Value::String(yukari_model.to_string()));
            obj.insert("stream".to_string(), Value::Bool(false));
        }

        let yol = hedef.yol.as_deref().unwrap_or("/v1/chat/completions");
        let url = format!("{}{}", hedef.taban_url, yol);
        let auth_header = format!("Bearer {}", anahtar);
        let anahtar_kopya = anahtar.to_string();

        let ajan = self.ajan.clone();
        let timeout = hedef.zaman_asimi;

        tokio::task::spawn_blocking(move || {
            let res = ajan
                .post(&url)
                .header("Authorization", &auth_header)
                .header("Content-Type", "application/json")
                .config()
                .http_status_as_error(false)
                .timeout_global(Some(timeout))
                .build()
                .send_json(&yukari_govde);

            match res {
                Ok(mut response) => {
                    let durum = response.status().as_u16();
                    if (200..300).contains(&durum) {
                        response.body_mut().read_json::<Value>().map_err(|e| {
                            YukariHatasi::JsonHatasi(temizle_hata(&e.to_string(), &anahtar_kopya))
                        })
                    } else {
                        let retry_after = response
                            .headers()
                            .get("retry-after")
                            .or_else(|| response.headers().get("Retry-After"))
                            .and_then(|v| v.to_str().ok())
                            .and_then(|v| v.trim().parse::<u64>().ok());

                        let hata_govdesi = response
                            .body_mut()
                            .read_to_string()
                            .unwrap_or_else(|_| String::new());
                        let temizlenmis = temizle_hata(&hata_govdesi, &anahtar_kopya);
                        Err(YukariHatasi::HttpHatasi(durum, temizlenmis, retry_after))
                    }
                }
                Err(ureq::Error::StatusCode(status)) => Err(YukariHatasi::HttpHatasi(
                    status,
                    "HTTP Hata Durumu".to_string(),
                    None,
                )),
                Err(e) => Err(YukariHatasi::AgHatasi(temizle_hata(
                    &e.to_string(),
                    &anahtar_kopya,
                ))),
            }
        })
        .await
        .map_err(|e| YukariHatasi::AgHatasi(format!("spawn_blocking hatası: {}", e)))?
    }

    async fn gonder_claude(
        &self,
        hedef: &Hedef,
        anahtar: &str,
        istemci_govdesi: &Value,
        yukari_model: &str,
    ) -> Result<Value, YukariHatasi> {
        let openai_istek: kervan_ceviri::OpenAiIstek =
            serde_json::from_value(istemci_govdesi.clone()).map_err(|e| {
                YukariHatasi::JsonHatasi(format!("İstemci gövdesi ayrıştırılamadı: {}", e))
            })?;

        let claude_istek = kervan_ceviri::openai_den_claude_istegi(&openai_istek, yukari_model)?;

        let yukari_govde = serde_json::to_value(&claude_istek)
            .map_err(|e| YukariHatasi::JsonHatasi(e.to_string()))?;

        let yol = hedef.yol.as_deref().unwrap_or("/v1/messages");
        let url = format!("{}{}", hedef.taban_url, yol);
        let anahtar_kopya = anahtar.to_string();
        let model_kopya = yukari_model.to_string();

        let ajan = self.ajan.clone();
        let timeout = hedef.zaman_asimi;

        tokio::task::spawn_blocking(move || {
            let res = ajan
                .post(&url)
                .header("x-api-key", &anahtar_kopya)
                .header("anthropic-version", "2023-06-01")
                .header("Content-Type", "application/json")
                .config()
                .http_status_as_error(false)
                .timeout_global(Some(timeout))
                .build()
                .send_json(&yukari_govde);

            match res {
                Ok(mut response) => {
                    let durum = response.status().as_u16();
                    if (200..300).contains(&durum) {
                        let claude_yanit = response
                            .body_mut()
                            .read_json::<kervan_ceviri::ClaudeYanit>()
                            .map_err(|e| {
                                YukariHatasi::JsonHatasi(temizle_hata(
                                    &e.to_string(),
                                    &anahtar_kopya,
                                ))
                            })?;
                        let openai_yanit =
                            kervan_ceviri::claude_dan_openai_yaniti(&claude_yanit, &model_kopya)
                                .map_err(|e| {
                                    YukariHatasi::JsonHatasi(temizle_hata(
                                        &e.to_string(),
                                        &anahtar_kopya,
                                    ))
                                })?;
                        serde_json::to_value(&openai_yanit)
                            .map_err(|e| YukariHatasi::JsonHatasi(e.to_string()))
                    } else {
                        let retry_after = response
                            .headers()
                            .get("retry-after")
                            .or_else(|| response.headers().get("Retry-After"))
                            .and_then(|v| v.to_str().ok())
                            .and_then(|v| v.trim().parse::<u64>().ok());

                        let hata_govdesi = response
                            .body_mut()
                            .read_to_string()
                            .unwrap_or_else(|_| String::new());
                        let temizlenmis = temizle_hata(&hata_govdesi, &anahtar_kopya);
                        Err(YukariHatasi::HttpHatasi(durum, temizlenmis, retry_after))
                    }
                }
                Err(ureq::Error::StatusCode(status)) => Err(YukariHatasi::HttpHatasi(
                    status,
                    "HTTP Hata Durumu".to_string(),
                    None,
                )),
                Err(e) => Err(YukariHatasi::AgHatasi(temizle_hata(
                    &e.to_string(),
                    &anahtar_kopya,
                ))),
            }
        })
        .await
        .map_err(|e| YukariHatasi::AgHatasi(format!("spawn_blocking hatası: {}", e)))?
    }

    async fn gonder_gemini(
        &self,
        hedef: &Hedef,
        anahtar: &str,
        istemci_govdesi: &Value,
        yukari_model: &str,
    ) -> Result<Value, YukariHatasi> {
        let openai_istek: kervan_ceviri::OpenAiIstek =
            serde_json::from_value(istemci_govdesi.clone()).map_err(|e| {
                YukariHatasi::JsonHatasi(format!("İstemci gövdesi ayrıştırılamadı: {}", e))
            })?;

        let gemini_istek = kervan_ceviri::openai_den_gemini_istegi(&openai_istek, yukari_model)?;

        let yukari_govde = serde_json::to_value(&gemini_istek)
            .map_err(|e| YukariHatasi::JsonHatasi(e.to_string()))?;

        let yol = hedef
            .yol
            .clone()
            .unwrap_or_else(|| format!("/v1beta/models/{}:generateContent", yukari_model));
        let url = format!("{}{}", hedef.taban_url, yol);
        let anahtar_kopya = anahtar.to_string();
        let model_kopya = yukari_model.to_string();

        let ajan = self.ajan.clone();
        let timeout = hedef.zaman_asimi;

        tokio::task::spawn_blocking(move || {
            let res = ajan
                .post(&url)
                .header("x-goog-api-key", &anahtar_kopya)
                .header("Content-Type", "application/json")
                .config()
                .http_status_as_error(false)
                .timeout_global(Some(timeout))
                .build()
                .send_json(&yukari_govde);

            match res {
                Ok(mut response) => {
                    let durum = response.status().as_u16();
                    if (200..300).contains(&durum) {
                        let gemini_yanit = response
                            .body_mut()
                            .read_json::<kervan_ceviri::GeminiYanit>()
                            .map_err(|e| {
                                YukariHatasi::JsonHatasi(temizle_hata(
                                    &e.to_string(),
                                    &anahtar_kopya,
                                ))
                            })?;
                        let openai_yanit =
                            kervan_ceviri::gemini_den_openai_yaniti(&gemini_yanit, &model_kopya)
                                .map_err(|e| {
                                    YukariHatasi::JsonHatasi(temizle_hata(
                                        &e.to_string(),
                                        &anahtar_kopya,
                                    ))
                                })?;
                        serde_json::to_value(&openai_yanit)
                            .map_err(|e| YukariHatasi::JsonHatasi(e.to_string()))
                    } else {
                        let retry_after = response
                            .headers()
                            .get("retry-after")
                            .or_else(|| response.headers().get("Retry-After"))
                            .and_then(|v| v.to_str().ok())
                            .and_then(|v| v.trim().parse::<u64>().ok());

                        let hata_govdesi = response
                            .body_mut()
                            .read_to_string()
                            .unwrap_or_else(|_| String::new());
                        let temizlenmis = temizle_hata(&hata_govdesi, &anahtar_kopya);
                        Err(YukariHatasi::HttpHatasi(durum, temizlenmis, retry_after))
                    }
                }
                Err(ureq::Error::StatusCode(status)) => Err(YukariHatasi::HttpHatasi(
                    status,
                    "HTTP Hata Durumu".to_string(),
                    None,
                )),
                Err(e) => Err(YukariHatasi::AgHatasi(temizle_hata(
                    &e.to_string(),
                    &anahtar_kopya,
                ))),
            }
        })
        .await
        .map_err(|e| YukariHatasi::AgHatasi(format!("spawn_blocking hatası: {}", e)))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Her test kendi adını kullanır: kasa süreç genelinde tek bir
    // `static`'tir ve testler paralel koşar. Ortak ad kullanmak testleri
    // birbirine bağımlı kılardı.

    #[test]
    fn oturum_kasasi_ortami_golgelemeli() {
        // Varsayılan çözücünün sırası: önce kasa, sonra ortam. Panelden
        // girilen değer kullanıcının gözü önünde düzelttiği güncel bilgidir.
        let ad = "KERVAN_TEST_KASA_GOLGELEME";
        // Ortamda böyle bir değişken yoksa da test anlamlı: kasaya yazılan
        // değer çözülmeli.
        assert!(oturum_anahtari_ekle(ad, "kasadan"));
        assert_eq!(anahtar_coz(ad).as_deref(), Some("kasadan"));
        assert!(oturum_anahtari_var(ad));
        assert!(oturum_anahtari_sil(ad));
        assert!(!oturum_anahtari_var(ad));
        // Kasa silindikten sonra ortamda da yok → çözüm yok.
        assert_eq!(anahtar_coz(ad), None);
    }

    #[test]
    fn bos_anahtar_kasaya_girmez() {
        // Boşluk artığı bir "tanımlı ama geçersiz" anahtar üretmemeli.
        let ad = "KERVAN_TEST_KASA_BOS";
        assert!(!oturum_anahtari_ekle(ad, "   "));
        assert!(!oturum_anahtari_ekle("   ", "deger"));
        assert!(!oturum_anahtari_var(ad));
        assert!(!oturum_anahtari_sil(ad));
    }

    #[test]
    fn yapistirilan_bosluklar_kirpilir() {
        let ad = "KERVAN_TEST_KASA_KIRP";
        assert!(oturum_anahtari_ekle(
            "  KERVAN_TEST_KASA_KIRP  ",
            "  deger  "
        ));
        assert_eq!(anahtar_coz(ad).as_deref(), Some("deger"));
        // Aynı ad yeniden girilirse eskisi ezilir: son girilen doğrudur.
        assert!(oturum_anahtari_ekle(ad, "yeni"));
        assert_eq!(anahtar_coz(ad).as_deref(), Some("yeni"));
        assert!(oturum_anahtari_sil(ad));
    }

    #[test]
    fn anahtar_yoksa_cozum_none() {
        // Hiçbir yerde tanımsız bir ad `None` döner; `Yukari` bunu
        // `AnahtarYok` hatasına çevirir.
        assert_eq!(anahtar_coz("KERVAN_TEST_HIC_YOK_12345"), None);
        assert_eq!(anahtar_coz(""), None);
    }

    #[test]
    fn ortam_degiskeni_yedek_kaynaktir() {
        // Süreç ortamında gerçekten var olan bir değişken (PATH) kasadan
        // bağımsız olarak çözülmeli — kasa ortamı GÖLGELEMİYORSA bu yol
        // kırılırdı ve mevcut kurulumlar sessizce anahtarsız kalırdı.
        assert!(anahtar_coz("PATH").is_some());
    }
}
