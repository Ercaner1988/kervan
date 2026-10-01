//! Kervan WebExtension ile yerel WebSocket köprüsü.
//!
//! NEDEN BU KÖPRÜ VAR:
//! WebDriver (geckodriver/marionette) kullanıldığında tarayıcı 'navigator.webdriver = true'
//! ile damgalanır ve Cloudflare / Turnstile / Kimi bot kalkanları sonsuz doğrulama
//! döngüsüne girer.
//!
//! Bu modül, tarayıcının sıradan bir insan gibi çalışmasını sağlar. Tarayıcı içine
//! yerleştirilen hafif Kervan Eklentisi, 127.0.0.1:20129/ws/eklenti adresine bağlanır.
//! İstemler bu WebSocket üzerinden güvenle iletilir, DOM gözlemcisi (MutationObserver)
//! ile yanıtlar canlı olarak Kervan'a geri akar.

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc, oneshot};

/// Eklentiden Kervan'a gelen mesaj tipleri.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum GelenMesaj {
    /// Eklenti sekmesi açıldığında veya yenilendiğinde kayıt olur.
    Kayit {
        provider: String,
        profile: Option<String>,
        url: Option<String>,
    },
    /// Canlı akış (SSE) parçası veya nihai yanıt.
    Parca {
        id: String,
        delta: String,
        done: bool,
    },
    /// Sayfa içinde karşılaşılan bir hata.
    Hata { id: String, mesaj: String },
    /// İnsan kapısı / güvenlik uyarısı durumu (Turnstile, Cloudflare, vb.)
    Durum {
        provider: String,
        durum: String,
        detay: Option<String>,
    },
    /// Nabız kontrolü.
    Pong,
}

/// Gönderilmek üzere hazırlanan dosya eki (görsel, video, belge).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DosyaEki {
    pub ad: String,
    pub mime_tipi: String,
    pub veri_base64: String,
}

/// Kervan'dan Eklentiye giden emirler.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum GidenMesaj {
    /// Sayfadaki istem kutusuna yazıp gönder.
    Sor {
        id: String,
        provider: String,
        istem: String,
        #[serde(default)]
        dosyalar: Vec<DosyaEki>,
    },
    /// Devam eden üretimi durdur.
    Durdur { id: String },
    /// Nabız sorusu.
    Ping,
}

/// Devam eden bir istemin yanıt kanalı.
pub enum YanitKanali {
    /// Tek seferde tam yanıt bekleyen akış.
    Tekil(oneshot::Sender<Result<String>>),
    /// Canlı akış (streaming) bekleyen akış.
    Akan(mpsc::Sender<Result<String>>),
}

/// Bağlı bir eklenti oturumu.
pub struct BagliSekme {
    pub provider: String,
    pub profile: String,
    pub gonderici: mpsc::UnboundedSender<GidenMesaj>,
}

/// Tüm bağlı eklenti sekmelerini ve bekleyen istekleri yöneten merkezi hub.
#[derive(Clone, Default)]
pub struct EklentiHub {
    /// Sağlayıcı adına göre bağlı sekmeler: provider -> BagliSekme
    sekmeler: Arc<Mutex<HashMap<String, BagliSekme>>>,
    /// İstek kimliğine göre yanıt bekleyen kanallar: id -> YanitKanali
    bekleyen_istekler: Arc<Mutex<HashMap<String, YanitKanali>>>,
    /// Her istek için biriken tam metin (tekil yanıtlar için): id -> String
    biriken_metinler: Arc<Mutex<HashMap<String, String>>>,
    /// Sağlayıcıların anlık durumları (örneğin insan kapısı uyarısı): provider -> String
    durumlar: Arc<Mutex<HashMap<String, String>>>,
}

impl EklentiHub {
    pub fn new() -> Self {
        Self::default()
    }

    /// Yeni bir sekme bağlandığında kaydeder.
    pub async fn sekme_kaydet(
        &self,
        provider: String,
        profile: String,
        gonderici: mpsc::UnboundedSender<GidenMesaj>,
    ) {
        let mut map = self.sekmeler.lock().await;
        println!("eklenti      : '{provider}' ({profile}) sekmesi bağlandı");
        map.insert(
            provider.clone(),
            BagliSekme {
                provider,
                profile,
                gonderici,
            },
        );
    }

    /// Sekme kapandığında kaydı siler.
    pub async fn sekme_ayril(&self, provider: &str) {
        let mut map = self.sekmeler.lock().await;
        println!("eklenti      : '{provider}' sekmesi ayrıldı");
        map.remove(provider);
    }

    /// Bir sağlayıcının eklenti sekmesi şu an canlı mı?
    pub async fn sekme_var_mi(&self, provider: &str) -> bool {
        let map = self.sekmeler.lock().await;
        map.contains_key(provider)
    }

    /// Eklentiye ek dosyalarla birlikte soru sorar ve yanıtın tamamlanmasını bekler.
    pub async fn sor_ekli(
        &self,
        provider: &str,
        istem: &str,
        dosyalar: Vec<DosyaEki>,
        timeout: std::time::Duration,
    ) -> Result<String> {
        let (tx_giden, id) = {
            let map = self.sekmeler.lock().await;
            let sekme = map.get(provider).ok_or_else(|| {
                anyhow::anyhow!("'{provider}' için bağlı bir eklenti sekmesi bulunamadı. Tarayıcıda ilgili sayfa açık mı?")
            })?;
            let id = format!("req-{}", rand::random::<u32>());
            (sekme.gonderici.clone(), id)
        };

        let (resp_tx, resp_rx) = oneshot::channel();
        {
            let mut bekleyen = self.bekleyen_istekler.lock().await;
            bekleyen.insert(id.clone(), YanitKanali::Tekil(resp_tx));
            let mut biriken = self.biriken_metinler.lock().await;
            biriken.insert(id.clone(), String::new());
        }

        // Emri eklentiye gönder
        tx_giden.send(GidenMesaj::Sor {
            id: id.clone(),
            provider: provider.to_string(),
            istem: istem.to_string(),
            dosyalar,
        })?;

        // Yanıtı zaman aşımıyla bekle
        match tokio::time::timeout(timeout, resp_rx).await {
            Ok(Ok(sonuc)) => sonuc,
            Ok(Err(_)) => {
                self.bekleyen_istekler.lock().await.remove(&id);
                self.biriken_metinler.lock().await.remove(&id);
                bail!("istem kanalı beklenmedik şekilde kapandı")
            }
            Err(_) => {
                self.bekleyen_istekler.lock().await.remove(&id);
                self.biriken_metinler.lock().await.remove(&id);
                // Eklentiye iptal emri ilet
                let _ = tx_giden.send(GidenMesaj::Durdur { id });
                bail!("{timeout:?} süre içinde yanıt tamamlanamadı (zaman aşımı)")
            }
        }
    }

    /// Eklentiye bir soru sorar ve yanıtın tamamlanmasını bekler.
    pub async fn sor(
        &self,
        provider: &str,
        istem: &str,
        timeout: std::time::Duration,
    ) -> Result<String> {
        self.sor_ekli(provider, istem, Vec::new(), timeout).await
    }

    /// Eklentiden gelen bir mesajı işler.
    pub async fn mesaji_isle(&self, mesaj: GelenMesaj) {
        match mesaj {
            GelenMesaj::Kayit {
                provider,
                profile,
                url: _,
            } => {
                // Kayıt işi genelde WS el sıkışmasında atanır, ancak dinamik güncelleme de desteklenir.
                let prof = profile.unwrap_or_else(|| "varsayilan".into());
                let mut durumlar = self.durumlar.lock().await;
                durumlar.insert(provider, format!("Hazır ({prof})"));
            }
            GelenMesaj::Parca { id, delta, done } => {
                let mut bitir = false;
                let mut son_metin = String::new();

                {
                    let mut biriken = self.biriken_metinler.lock().await;
                    if let Some(m) = biriken.get_mut(&id) {
                        m.push_str(&delta);
                        if done {
                            son_metin = m.clone();
                            bitir = true;
                        }
                    }
                }

                if bitir {
                    self.biriken_metinler.lock().await.remove(&id);
                    let mut bekleyen = self.bekleyen_istekler.lock().await;
                    if let Some(YanitKanali::Tekil(tx)) = bekleyen.remove(&id) {
                        let _ = tx.send(Ok(son_metin));
                    }
                }
            }
            GelenMesaj::Hata { id, mesaj } => {
                self.biriken_metinler.lock().await.remove(&id);
                let mut bekleyen = self.bekleyen_istekler.lock().await;
                if let Some(YanitKanali::Tekil(tx)) = bekleyen.remove(&id) {
                    let _ = tx.send(Err(anyhow::anyhow!("Eklenti hatası: {mesaj}")));
                }
            }
            GelenMesaj::Durum {
                provider,
                durum,
                detay,
            } => {
                let text = match detay {
                    Some(d) => format!("{durum}: {d}"),
                    None => durum,
                };
                println!("eklenti-durum : [{provider}] {text}");
                let mut durumlar = self.durumlar.lock().await;
                durumlar.insert(provider, text);
            }
            GelenMesaj::Pong => {}
        }
    }

    /// Bağlı tüm sekmelerin listesini döner.
    pub async fn bagli_sekmeler(&self) -> Vec<String> {
        let map = self.sekmeler.lock().await;
        map.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_giden_mesaj_sor_dosyasiz_geriye_uyumluluk() {
        // Eski eklenti/istek formatı (dosyalar alanı yok)
        let json = r#"{"action":"sor","id":"req-1","provider":"gemini","istem":"Merhaba"}"#;
        let msg: GidenMesaj = serde_json::from_str(json).expect("deserialize edilmeli");
        match msg {
            GidenMesaj::Sor {
                id,
                provider,
                istem,
                dosyalar,
            } => {
                assert_eq!(id, "req-1");
                assert_eq!(provider, "gemini");
                assert_eq!(istem, "Merhaba");
                assert!(dosyalar.is_empty(), "dosyalar varsayılan boş vektör olmalı");
            }
            _ => panic!("GidenMesaj::Sor bekleniyordu"),
        }
    }

    #[test]
    fn test_giden_mesaj_sor_dosyali_ser_de() {
        let msg = GidenMesaj::Sor {
            id: "req-2".into(),
            provider: "gemini".into(),
            istem: "Bu resmi incele".into(),
            dosyalar: vec![DosyaEki {
                ad: "test.png".into(),
                mime_tipi: "image/png".into(),
                veri_base64: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==".into(),
            }],
        };

        let json = serde_json::to_string(&msg).expect("serialize edilmeli");
        assert!(json.contains("\"dosyalar\""));
        assert!(json.contains("test.png"));
        assert!(json.contains("image/png"));

        let deserialized: GidenMesaj = serde_json::from_str(&json).expect("deserialize edilmeli");
        match deserialized {
            GidenMesaj::Sor { dosyalar, .. } => {
                assert_eq!(dosyalar.len(), 1);
                assert_eq!(dosyalar[0].ad, "test.png");
                assert_eq!(dosyalar[0].mime_tipi, "image/png");
            }
            _ => panic!("GidenMesaj::Sor bekleniyordu"),
        }
    }
}
