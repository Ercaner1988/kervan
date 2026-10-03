//! `kervan-direnc` — Kervan 3 Katmanlı Dayanıklılık (Resilience) Kütüphanesi.
//!
//! 1. Devre Kesici (Circuit Breaker: CLOSED → DEGRADED → OPEN → HALF_OPEN → CLOSED)
//! 2. Bağlantı ve Sağlayıcı Soğuma (Cooldown / Exponential Backoff / Retry-After)
//! 3. Model Kilidi (Model Lockout / Exclusion)

pub mod devre_kesici;
pub mod kilit;
pub mod saat;
pub mod soguma;

pub use devre_kesici::{DevreKesici, DevreKesiciYapilandirma, DirencHatasi, Durum};
pub use kilit::ModelKilidi;
pub use saat::{GercekSaat, Saat, TestSaati};
pub use soguma::SogumaTakipcisi;

use std::collections::HashMap;
use std::sync::Arc;

/// Devre kesici, soğuma takipçisi ve model kilidini merkezi olarak yöneten birim.
pub struct DirencYoneticisi {
    kesiciler: HashMap<String, DevreKesici>,
    soguma: SogumaTakipcisi,
    kilit: ModelKilidi,
    saat: Arc<dyn Saat>,
    varsayilan_kesici_ayar: DevreKesiciYapilandirma,
}

impl Default for DirencYoneticisi {
    fn default() -> Self {
        Self::yeni(Arc::new(GercekSaat), None)
    }
}

impl DirencYoneticisi {
    pub fn yeni(saat: Arc<dyn Saat>, ayar: Option<DevreKesiciYapilandirma>) -> Self {
        Self {
            kesiciler: HashMap::new(),
            soguma: SogumaTakipcisi::default(),
            kilit: ModelKilidi::default(),
            saat,
            varsayilan_kesici_ayar: ayar.unwrap_or_default(),
        }
    }

    /// Bir hedefe istek göndermeden önce izin denetler.
    /// Soğumadaysa `Sogumada`, devre açıksa `DevreAcik` hatası döner.
    pub fn istek_izni_al(&mut self, hedef_ad: &str) -> Result<(), DirencHatasi> {
        let simdi = self.saat.simdi();

        // 1. Soğuma kontrolü (429 Retry-After)
        if self.soguma.sogumada_mi_zamaniyla(hedef_ad, simdi) {
            let kalan = self.soguma.kalan_sure_saniye_zamaniyla(hedef_ad, simdi);
            return Err(DirencHatasi::Sogumada {
                ad: hedef_ad.to_string(),
                retry_after_saniye: kalan,
            });
        }

        // 2. Devre kesici kontrolü
        let ayar = self.varsayilan_kesici_ayar.clone();
        let kesici = self
            .kesiciler
            .entry(hedef_ad.to_string())
            .or_insert_with(|| DevreKesici::yeni(hedef_ad, Some(ayar)));

        kesici.istek_izni_al_zamaniyla(simdi)
    }

    /// Başarılı istek sonrası sayaçları ve soğuma durumunu sıfırlar.
    pub fn basari_kaydet(&mut self, hedef_ad: &str) {
        if let Some(kesici) = self.kesiciler.get_mut(hedef_ad) {
            kesici.basari_kaydet();
        }
        self.soguma.basari_bildir(hedef_ad);
    }

    /// Başarısız istek sonrası hata kaydeder.
    /// - 429: Soğuma başlatır (devre kesiciyi beslemez).
    /// - 5xx / Ağ Hatası (None veya >= 500): Devre kesiciyi besler.
    /// - 4xx (429 hariç): Devre kesiciyi veya soğumayı beslemez.
    pub fn hata_kaydet(
        &mut self,
        hedef_ad: &str,
        durum_kodu: Option<u16>,
        retry_after_saniye: Option<u64>,
    ) {
        let simdi = self.saat.simdi();

        if durum_kodu == Some(429) {
            self.soguma
                .soguma_baslat_zamaniyla(hedef_ad, Some("429"), retry_after_saniye, simdi);
        } else if durum_kodu.map_or(true, |kod| kod >= 500) {
            let ayar = self.varsayilan_kesici_ayar.clone();
            let kesici = self
                .kesiciler
                .entry(hedef_ad.to_string())
                .or_insert_with(|| DevreKesici::yeni(hedef_ad, Some(ayar)));
            kesici.hata_kaydet_zamaniyla(simdi);
        }
    }

    pub fn kesici_durumu(&mut self, hedef_ad: &str) -> Durum {
        let simdi = self.saat.simdi();
        let ayar = self.varsayilan_kesici_ayar.clone();
        let kesici = self
            .kesiciler
            .entry(hedef_ad.to_string())
            .or_insert_with(|| DevreKesici::yeni(hedef_ad, Some(ayar)));
        kesici.durum_zamaniyla(simdi)
    }

    pub fn sogumada_mi(&self, hedef_ad: &str) -> bool {
        let simdi = self.saat.simdi();
        self.soguma.sogumada_mi_zamaniyla(hedef_ad, simdi)
    }

    pub fn model_kilidi(&mut self) -> &mut ModelKilidi {
        &mut self.kilit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn test_direnc_yoneticisi_5xx_ve_kesici_acma() {
        let test_saati = Arc::new(TestSaati::yeni(Instant::now()));
        let mut yonetici = DirencYoneticisi::yeni(
            test_saati.clone(),
            Some(DevreKesiciYapilandirma {
                hata_esigi: 3,
                bozulma_esigi: 2,
                soguma_suresi: Duration::from_secs(10),
                yari_acik_izin: 1,
            }),
        );

        // Başlangıçta istek izni var
        assert!(yonetici.istek_izni_al("hedef-1").is_ok());

        // 1. ve 2. 5xx hatası
        yonetici.hata_kaydet("hedef-1", Some(500), None);
        assert_eq!(yonetici.kesici_durumu("hedef-1"), Durum::Kapali);
        assert!(yonetici.istek_izni_al("hedef-1").is_ok());

        yonetici.hata_kaydet("hedef-1", Some(502), None);
        assert_eq!(yonetici.kesici_durumu("hedef-1"), Durum::Bozulmus);
        assert!(yonetici.istek_izni_al("hedef-1").is_ok());

        // 3. 5xx hatası -> Devre açılır!
        yonetici.hata_kaydet("hedef-1", Some(503), None);
        assert_eq!(yonetici.kesici_durumu("hedef-1"), Durum::Acik);

        // İstek izni reddedilmeli
        match yonetici.istek_izni_al("hedef-1") {
            Err(DirencHatasi::DevreAcik { ad, .. }) => assert_eq!(ad, "hedef-1"),
            _ => panic!("Açık devrede istek izni reddedilmeli"),
        }

        // Saati 11 saniye ilerletelim (gerçek uyuma yok!)
        test_saati.ilerlet(Duration::from_secs(11));
        assert_eq!(yonetici.kesici_durumu("hedef-1"), Durum::YariAcik);
        assert!(yonetici.istek_izni_al("hedef-1").is_ok());

        // Başarılı işlem sonrası kapalıya döner
        yonetici.basari_kaydet("hedef-1");
        assert_eq!(yonetici.kesici_durumu("hedef-1"), Durum::Kapali);
    }

    #[test]
    fn test_direnc_yoneticisi_429_retry_after() {
        let test_saati = Arc::new(TestSaati::yeni(Instant::now()));
        let mut yonetici = DirencYoneticisi::yeni(test_saati.clone(), None);

        // 429 hatası ve Retry-After 5 saniye
        yonetici.hata_kaydet("hedef-2", Some(429), Some(5));

        // Soğuma devrede, devre kesici ise KAPALI kalmalı (429 kesiciyi beslemez)
        assert_eq!(yonetici.kesici_durumu("hedef-2"), Durum::Kapali);
        assert!(yonetici.sogumada_mi("hedef-2"));

        match yonetici.istek_izni_al("hedef-2") {
            Err(DirencHatasi::Sogumada {
                ad,
                retry_after_saniye,
            }) => {
                assert_eq!(ad, "hedef-2");
                assert_eq!(retry_after_saniye, 5);
            }
            _ => panic!("429 sonrası soğumada reddedilmeli"),
        }

        // 4 saniye sonra hala soğumada
        test_saati.ilerlet(Duration::from_secs(4));
        assert!(yonetici.sogumada_mi("hedef-2"));

        // 2 saniye daha ilerlet (toplam 6 sn) -> soğuma biter
        test_saati.ilerlet(Duration::from_secs(2));
        assert!(!yonetici.sogumada_mi("hedef-2"));
        assert!(yonetici.istek_izni_al("hedef-2").is_ok());
    }

    #[test]
    fn test_direnc_yoneticisi_4xx_hata_kesiciyi_etkilemez() {
        let mut yonetici = DirencYoneticisi::default();

        // 400 ve 404 hataları devre kesiciyi beslemez
        for _ in 0..10 {
            yonetici.hata_kaydet("hedef-3", Some(400), None);
            yonetici.hata_kaydet("hedef-3", Some(404), None);
        }

        assert_eq!(yonetici.kesici_durumu("hedef-3"), Durum::Kapali);
        assert!(yonetici.istek_izni_al("hedef-3").is_ok());
    }
}
