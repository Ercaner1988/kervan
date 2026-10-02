use kervan_direnc::{DevreKesici, SogumaTakipcisi};
use kervan_yonlendirme::{AnlamsalOnbellek, HizSinirlayici};
use std::sync::Mutex;
use std::time::Duration;

use crate::guvenlik::GuvenlikPolitikasi;

/// V1 HTTP API uçları için paylaşılan durum
pub struct V1Durum {
    pub guvenlik: GuvenlikPolitikasi,
    pub hiz_sinirlayici: HizSinirlayici,
    pub onbellek: AnlamsalOnbellek,
    pub direnc_devre: Mutex<DevreKesici>,
    pub direnc_soguma: Mutex<SogumaTakipcisi>,
}

impl Default for V1Durum {
    fn default() -> Self {
        Self {
            guvenlik: GuvenlikPolitikasi::default(),
            hiz_sinirlayici: HizSinirlayici::yeni(100.0, 10.0), // Kapasite 100, saniyede 10 token
            onbellek: AnlamsalOnbellek::yeni(500, Duration::from_secs(300)),
            direnc_devre: Mutex::new(DevreKesici::yeni("kervan-ana-devre", None)),
            direnc_soguma: Mutex::new(SogumaTakipcisi::yeni(
                Duration::from_secs(2),
                Duration::from_secs(60),
            )),
        }
    }
}
