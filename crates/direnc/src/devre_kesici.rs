//! Devre Kesici (Circuit Breaker) Durum Makinesi
//!
//! KAPALI (Closed) → BOZULMUŞ (Degraded) → AÇIK (Open) → YARI-AÇIK (Half-Open) → KAPALI
//! OmniRoute shared/utils/circuitBreaker.ts mimarisinin saf Rust uygulaması.

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DirencHatasi {
    #[error("Devre kesici açık: {ad} ({kalan_sure_ms}ms sonra tekrar denenecek)")]
    DevreAcik { ad: String, kalan_sure_ms: u64 },
    #[error("Sağlayıcı soğuma durumunda: {ad} (Retry-After: {retry_after_saniye}s)")]
    Sogumada { ad: String, retry_after_saniye: u64 },
    #[error("Yarı açık deneme limiti aşıldı: {0}")]
    YariAcikDolu(String),
    #[error("İşlem hatası: {0}")]
    IslemHatasi(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Durum {
    /// Normal çalışma, tüm istekler geçer
    Kapali,
    /// Hata oranı yükseldi, istekler geçer ancak uyarı verilir
    Bozulmus,
    /// Devre kesildi, gelen istekler derhal reddedilir (503)
    Acik,
    /// İyileşme denemesi: sınırlı sayıda test isteğine izin verilir
    YariAcik,
}

#[derive(Debug, Clone)]
pub struct DevreKesiciYapilandirma {
    pub hata_esigi: u32,
    pub bozulma_esigi: u32,
    pub soguma_suresi: Duration,
    pub yari_acik_izin: u32,
}

impl Default for DevreKesiciYapilandirma {
    fn default() -> Self {
        Self {
            hata_esigi: 3,
            bozulma_esigi: 2,
            soguma_suresi: Duration::from_secs(30),
            yari_acik_izin: 1,
        }
    }
}

pub struct DevreKesici {
    pub ad: String,
    pub yapilandirma: DevreKesiciYapilandirma,
    durum: Durum,
    ardisik_hata_sayisi: u32,
    son_hata_zamani: Option<Instant>,
    yari_acik_deneme_sayisi: u32,
}

impl DevreKesici {
    pub fn yeni(ad: impl Into<String>, yapilandirma: Option<DevreKesiciYapilandirma>) -> Self {
        Self {
            ad: ad.into(),
            yapilandirma: yapilandirma.unwrap_or_default(),
            durum: Durum::Kapali,
            ardisik_hata_sayisi: 0,
            son_hata_zamani: None,
            yari_acik_deneme_sayisi: 0,
        }
    }

    /// Mevcut durumu zamana ve sayaçlara göre günceller.
    pub fn durum(&mut self) -> Durum {
        self.durum_zamaniyla(Instant::now())
    }

    /// Belirtilen zamana göre durumu günceller (test edilebilirlik için).
    pub fn durum_zamaniyla(&mut self, simdi: Instant) -> Durum {
        if self.durum == Durum::Acik
            && let Some(son_hata) = self.son_hata_zamani
            && simdi >= son_hata
            && simdi.duration_since(son_hata) >= self.yapilandirma.soguma_suresi
        {
            self.durum = Durum::YariAcik;
            self.yari_acik_deneme_sayisi = 0;
        }
        self.durum
    }

    /// İstek yapılabilir mi denetler.
    pub fn istek_izni_al(&mut self) -> Result<(), DirencHatasi> {
        self.istek_izni_al_zamaniyla(Instant::now())
    }

    /// Belirtilen zamana göre istek izni denetler.
    pub fn istek_izni_al_zamaniyla(&mut self, simdi: Instant) -> Result<(), DirencHatasi> {
        match self.durum_zamaniyla(simdi) {
            Durum::Kapali | Durum::Bozulmus => Ok(()),
            Durum::Acik => {
                let kalan = if let Some(son_hata) = self.son_hata_zamani {
                    if simdi >= son_hata {
                        let gecen = simdi.duration_since(son_hata);
                        if gecen < self.yapilandirma.soguma_suresi {
                            (self.yapilandirma.soguma_suresi - gecen).as_millis() as u64
                        } else {
                            0
                        }
                    } else {
                        self.yapilandirma.soguma_suresi.as_millis() as u64
                    }
                } else {
                    0
                };
                Err(DirencHatasi::DevreAcik {
                    ad: self.ad.clone(),
                    kalan_sure_ms: kalan,
                })
            }
            Durum::YariAcik => {
                if self.yari_acik_deneme_sayisi < self.yapilandirma.yari_acik_izin {
                    self.yari_acik_deneme_sayisi += 1;
                    Ok(())
                } else {
                    Err(DirencHatasi::YariAcikDolu(self.ad.clone()))
                }
            }
        }
    }

    /// Başarılı bir çağrı sonrası durumları sıfırlar.
    pub fn basari_kaydet(&mut self) {
        self.ardisik_hata_sayisi = 0;
        self.durum = Durum::Kapali;
        self.son_hata_zamani = None;
        self.yari_acik_deneme_sayisi = 0;
    }

    /// Başarısız bir çağrı sonrası hata sayacını artırır ve gerekiyorsa devreyi açar.
    pub fn hata_kaydet(&mut self) {
        self.hata_kaydet_zamaniyla(Instant::now());
    }

    /// Belirtilen zamanla hata kaydeder.
    pub fn hata_kaydet_zamaniyla(&mut self, simdi: Instant) {
        self.ardisik_hata_sayisi += 1;
        self.son_hata_zamani = Some(simdi);

        if self.durum == Durum::YariAcik {
            // Yarı açık durumdaki deneme başarısız olursa derhal tekrar AÇIK duruma döner
            self.durum = Durum::Acik;
        } else if self.ardisik_hata_sayisi >= self.yapilandirma.hata_esigi {
            self.durum = Durum::Acik;
        } else if self.ardisik_hata_sayisi >= self.yapilandirma.bozulma_esigi {
            self.durum = Durum::Bozulmus;
        }
    }

    pub fn ardisik_hata(&self) -> u32 {
        self.ardisik_hata_sayisi
    }
}
