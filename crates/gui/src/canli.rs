//! Panonun canlı veri kaynağı: eklenti hesapları, havuz yuvaları ve
//! `kervan.toml`dan gelen model/özellik seçenekleri.
//!
//! NEDEN BU MODÜL: pano daha önce `PanoOkuyucu::yeni()` içindeki elle yazılmış
//! yer tutucudan besleniyordu (var olmayan hesaplar, uydurma istek sayıları).
//! Burada kaynak doğrudan süreç içindeki `AppState`: ayrı bir HTTP yoklama yolu,
//! ayrı bir kimlik doğrulama ve port varsayımı yok.
//!
//! İki iş parçacığı:
//! 1. **Yoklayıcı** — 1,5 sn'de bir `CanliDurum` + `Ozet` üretir.
//! 2. **Gönderici** — panelden "Gönder"e basıldığında tek isteği yerel sunucuya
//!    (`127.0.0.1:<port>`) gönderir ve yanıtı panele taşır.

use crate::server::AppState;
use kervan_pano_veri::{
    CanliAnahtar, CanliDurum, CanliHesap, CanliModel, CanliSaglayici, GonderimDurumu, HesapDurumu,
    HesapOzeti, Ozet, PanoOkuyucu, TelemetriOzeti,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const YOKLAMA_ARALIGI: Duration = Duration::from_millis(1500);

/// Panelden gönderilecek tek sınama isteği.
#[derive(Debug, Clone)]
pub struct GonderIstegi {
    pub port: u16,
    /// `kervan/gemini/orta`
    pub model: String,
    /// `gemini:profil` — boşsa sunucu en-az-kullanılanı seçer.
    pub hesap: String,
    pub istem: String,
    pub anahtarlar: BTreeMap<String, bool>,
}

/// Pano verisinin tek sahibi: yoklayıcı + gönderici + özet okuyucu.
pub struct CanliDepo {
    durum: Arc<Mutex<CanliDurum>>,
    gonderim: Arc<Mutex<GonderimDurumu>>,
    okuyucu: Arc<PanoOkuyucu>,
    port: u16,
}

impl CanliDepo {
    pub fn yeni(state: Arc<AppState>, port: u16) -> Self {
        let durum = Arc::new(Mutex::new(CanliDurum::default()));
        let gonderim = Arc::new(Mutex::new(GonderimDurumu::default()));
        let okuyucu = Arc::new(PanoOkuyucu::yeni());

        let d = Arc::clone(&durum);
        let o = Arc::clone(&okuyucu);
        let _ = std::thread::Builder::new()
            .name("kervan-pano-yokla".to_string())
            .spawn(move || {
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        let mut k = d.lock().unwrap_or_else(|e| e.into_inner());
                        k.hata = Some(format!("yoklama çalışma zamanı kurulamadı: {e}"));
                        return;
                    }
                };
                loop {
                    let (yeni_durum, yeni_ozet) = rt.block_on(yokla(&state, port));
                    {
                        let mut k = d.lock().unwrap_or_else(|e| e.into_inner());
                        *k = yeni_durum;
                    }
                    o.ozet_guncelle(yeni_ozet);
                    std::thread::sleep(YOKLAMA_ARALIGI);
                }
            });

        Self {
            durum,
            gonderim,
            okuyucu,
            port,
        }
    }

    pub fn durum(&self) -> CanliDurum {
        self.durum.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn gonderim(&self) -> GonderimDurumu {
        self.gonderim
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Diğer beş ekranın tükettiği özet (artık canlı veriden üretiliyor).
    pub fn ozet(&self) -> Arc<Ozet> {
        self.okuyucu.guncel_ozet()
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Tek sınama isteğini arka planda gönderir; sonuç `gonderim()` ile okunur.
    pub fn gonder(&self, istek: GonderIstegi) {
        {
            let mut g = self.gonderim.lock().unwrap_or_else(|e| e.into_inner());
            *g = GonderimDurumu {
                calisiyor: true,
                model: istek.model.clone(),
                hesap: istek.hesap.clone(),
                ..Default::default()
            };
        }
        let gonderim = Arc::clone(&self.gonderim);

        let _ = std::thread::Builder::new()
            .name("kervan-pano-gonder".to_string())
            .spawn(move || {
                let bas = Instant::now();
                let govde = serde_json::json!({
                    "model": istek.model,
                    "messages": [{ "role": "user", "content": istek.istem }],
                    "stream": false,
                    "kervan": {
                        "hesap": if istek.hesap.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(istek.hesap.clone()) },
                        "anahtarlar": istek.anahtarlar,
                    }
                });
                let url = format!("http://127.0.0.1:{}/v1/chat/completions", istek.port);
                let sonuc = ureq::post(&url)
                    .config()
                    .http_status_as_error(false)
                    .timeout_global(Some(Duration::from_secs(180)))
                    .build()
                    .send_json(&govde);
                let sure_ms = bas.elapsed().as_millis() as u64;

                let (yanit, hata) = match sonuc {
                    Ok(mut response) => {
                        let kod = response.status().as_u16();
                        let metin = response
                            .body_mut()
                            .read_to_string()
                            .unwrap_or_else(|_| String::new());
                        if (200..300).contains(&kod) {
                            let icerik = serde_json::from_str::<serde_json::Value>(&metin)
                                .ok()
                                .and_then(|v| {
                                    v["choices"][0]["message"]["content"]
                                        .as_str()
                                        .map(|s| s.to_string())
                                })
                                .unwrap_or(metin);
                            (icerik, None)
                        } else {
                            let mesaj = serde_json::from_str::<serde_json::Value>(&metin)
                                .ok()
                                .and_then(|v| {
                                    v["error"]["message"].as_str().map(|s| s.to_string())
                                })
                                .unwrap_or(metin);
                            (String::new(), Some(format!("HTTP {kod}: {mesaj}")))
                        }
                    }
                    Err(e) => (String::new(), Some(format!("istek gönderilemedi: {e}"))),
                };

                let mut g = gonderim.lock().unwrap_or_else(|e| e.into_inner());
                g.calisiyor = false;
                g.yanit = yanit;
                g.hata = hata;
                g.sure_ms = sure_ms;
            });
    }
}

fn simdi_sn() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Tek yoklama: canlı pano durumu + diğer ekranların özeti.
async fn yokla(state: &AppState, port: u16) -> (CanliDurum, Ozet) {
    let tavan = state.pool.pacing.hourly_cap;
    let mut hesaplar: Vec<CanliHesap> = Vec::new();

    // 1. Eklentiye bağlı hesaplar (`provider:profile` anahtarları).
    for anahtar in state.eklenti_hub.bagli_hesaplar().await {
        let (saglayici, profil) = match anahtar.split_once(':') {
            Some((s, p)) => (s.to_string(), p.to_string()),
            None => (anahtar.clone(), anahtar.clone()),
        };
        hesaplar.push(CanliHesap {
            anahtar,
            profil,
            saglayici,
            bagli: true,
            istek_sayisi: 0,
            son_saatte: 0,
            saatlik_tavan: tavan,
            duraklatildi: None,
        });
    }

    // 2. Havuz yuvaları (tarayıcı sürücülü hesaplar). Bunlar eklenti hesabı
    //    değildir: panelde görünür ama istek sabitlemek için seçilemez.
    let mut toplam_istek: u64 = 0;
    for slot in &state.pool.slots {
        let s = slot.state.lock().await;
        toplam_istek += u64::from(s.total);
        hesaplar.push(CanliHesap {
            anahtar: String::new(),
            profil: slot.hesap_adi.clone(),
            saglayici: slot.provider.ad.clone(),
            bagli: false,
            istek_sayisi: s.total,
            son_saatte: s.recent.len(),
            saatlik_tavan: tavan,
            duraklatildi: s.paused_reason.clone(),
        });
    }

    // 3. Sağlayıcı seçenekleri: yalnız kervan.toml'da ÖLÇÜLMÜŞ olanlar.
    let mut saglayicilar: Vec<CanliSaglayici> = state
        .saglayicilar
        .iter()
        .map(|(ad, s)| CanliSaglayici {
            ad: ad.clone(),
            modeller: s
                .model
                .as_ref()
                .map(|m| {
                    m.modeller
                        .iter()
                        .map(|x| CanliModel {
                            kod: Some(x.kod.clone()),
                            etiket: x.etiket.clone(),
                            kilitli: x.kilitli,
                            kilit_nedeni: x.kilit_nedeni.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            anahtarlar: s
                .anahtarlar
                .iter()
                .map(|a| CanliAnahtar {
                    kod: a.kod.clone(),
                    etiket: a.etiket.clone(),
                })
                .collect(),
        })
        .collect();
    saglayicilar.sort_by(|a, b| a.ad.cmp(&b.ad));

    // 4. Diğer ekranların özeti (artık canlı).
    let ozet_hesaplar: Vec<HesapOzeti> = hesaplar
        .iter()
        .map(|h| HesapOzeti {
            ad: if h.bagli {
                h.profil.clone()
            } else {
                format!("{} (havuz)", h.profil)
            },
            saglayicilar: vec![h.saglayici.clone()],
            durum: if h.duraklatildi.is_some() {
                HesapDurumu::InsanKapisi
            } else {
                HesapDurumu::Hazir
            },
            saatlik_tavan: h.saatlik_tavan,
            kota_penceresi: String::new(),
        })
        .collect();
    let mut modeller: Vec<String> = Vec::new();
    for s in &saglayicilar {
        modeller.push(format!("kervan/{}", s.ad));
        for m in &s.modeller {
            if let Some(kod) = &m.kod {
                modeller.push(format!("kervan/{}/{}", s.ad, kod));
            }
        }
    }
    let bagli_sayisi = hesaplar.iter().filter(|h| h.bagli).count();
    let ozet = Ozet {
        sunucu_acik: true,
        baglanti_adresi: format!("127.0.0.1:{port}"),
        hesaplar: ozet_hesaplar,
        modeller,
        telemetri: TelemetriOzeti {
            toplam_istek,
            aktif_yuvalar: bagli_sayisi,
            ..Default::default()
        },
        kimlikler: state.pool.kimlikler.iter().map(|k| k.ad.clone()).collect(),
        ..Default::default()
    };

    (
        CanliDurum {
            hesaplar,
            saglayicilar,
            guncelleme: simdi_sn(),
            hata: None,
        },
        ozet,
    )
}
