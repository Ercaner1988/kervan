use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

/// Belirteç Kovası (Token Bucket) Hız Sınırlayıcı
pub struct HizSinirlayici {
    kapasite: f64,
    doldurma_orani_sn: f64,
    kovalar: Mutex<HashMap<String, Kova>>,
}

struct Kova {
    belirtec_sayisi: f64,
    son_guncelleme: Instant,
}

#[derive(Debug, PartialEq, Eq)]
pub enum HizSiniriKarari {
    IzinVerildi,
    Reddedildi { bekleme_ms: u64 },
}

impl HizSinirlayici {
    pub fn yeni(kapasite: f64, doldurma_orani_sn: f64) -> Self {
        Self {
            kapasite,
            doldurma_orani_sn,
            kovalar: Mutex::new(HashMap::new()),
        }
    }

    /// Belirtilen anahtar için 1 belirteç tüketmeyi dener
    pub fn kontrol_et(&self, anahtar: &str) -> HizSiniriKarari {
        let mut kilit = match self.kovalar.lock() {
            Ok(guard) => guard,
            Err(zehirli) => zehirli.into_inner(),
        };

        let simdi = Instant::now();
        let kova = kilit.entry(anahtar.to_string()).or_insert_with(|| Kova {
            belirtec_sayisi: self.kapasite,
            son_guncelleme: simdi,
        });

        // Geçen süreye göre belirteçleri tazele
        let gecen_sn = simdi.duration_since(kova.son_guncelleme).as_secs_f64();
        kova.belirtec_sayisi =
            (kova.belirtec_sayisi + gecen_sn * self.doldurma_orani_sn).min(self.kapasite);
        kova.son_guncelleme = simdi;

        if kova.belirtec_sayisi >= 1.0 {
            kova.belirtec_sayisi -= 1.0;
            HizSiniriKarari::IzinVerildi
        } else {
            let eksik = 1.0 - kova.belirtec_sayisi;
            let bekleme_sn = eksik / self.doldurma_orani_sn;
            let bekleme_ms = (bekleme_sn * 1000.0).ceil() as u64;
            HizSiniriKarari::Reddedildi { bekleme_ms }
        }
    }
}
