//! Bağlantı ve Sağlayıcı Soğuma (Cooldown / Backoff) Takipçisi
//!
//! 429 Too Many Requests ve 5xx hatalarında üstel geri çekilme ve Retry-After yönetimi.
//! OmniRoute providerCooldownTracker.ts mimarisine uygun saf Rust uygulaması.

use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct SogumaKaydi {
    pub hedef: String,
    pub bitis_zamani: Instant,
    pub ardisik_hata: u32,
    pub son_hata_kodu: Option<String>,
}

pub struct SogumaTakipcisi {
    kayitlar: HashMap<String, SogumaKaydi>,
    taban_sure: Duration,
    tavan_sure: Duration,
}

impl Default for SogumaTakipcisi {
    fn default() -> Self {
        Self::yeni(Duration::from_secs(5), Duration::from_secs(300))
    }
}

impl SogumaTakipcisi {
    pub fn yeni(taban_sure: Duration, tavan_sure: Duration) -> Self {
        Self {
            kayitlar: HashMap::new(),
            taban_sure,
            tavan_sure,
        }
    }

    /// Bir hedefin (sağlayıcı veya bağlantı ID'si) şu anda soğumada olup olmadığını denetler.
    pub fn sogumada_mi(&self, hedef: &str) -> bool {
        if let Some(kayit) = self.kayitlar.get(hedef) {
            Instant::now() < kayit.bitis_zamani
        } else {
            false
        }
    }

    /// Kalan soğuma süresini milisaniye olarak döner. Soğumada değilse 0 döner.
    pub fn kalan_sure_ms(&self, hedef: &str) -> u64 {
        if let Some(kayit) = self.kayitlar.get(hedef) {
            let simdi = Instant::now();
            if simdi < kayit.bitis_zamani {
                return (kayit.bitis_zamani - simdi).as_millis() as u64;
            }
        }
        0
    }

    /// Hata oluştuğunda soğuma süresini hesaplayıp kaydeder.
    /// Varsa HTTP `Retry-After` süresi doğrudan uygulanır.
    pub fn soguma_baslat(
        &mut self,
        hedef: &str,
        hata_kodu: Option<&str>,
        retry_after_saniye: Option<u64>,
    ) -> Duration {
        let mevcut_hata = self
            .kayitlar
            .get(hedef)
            .map(|k| k.ardisik_hata)
            .unwrap_or(0);
        let yeni_hata = mevcut_hata + 1;

        let sure = if let Some(saniye) = retry_after_saniye {
            Duration::from_secs(saniye)
        } else {
            // Üstel geri çekilme: taban * 2^(hata - 1), tavan ile sınırlı
            let carpan = 1u64
                .checked_shl(yeni_hata.saturating_sub(1))
                .unwrap_or(u64::MAX);
            let hesaplanan = self.taban_sure.saturating_mul(carpan as u32);
            if hesaplanan > self.tavan_sure {
                self.tavan_sure
            } else {
                hesaplanan
            }
        };

        self.kayitlar.insert(
            hedef.to_string(),
            SogumaKaydi {
                hedef: hedef.to_string(),
                bitis_zamani: Instant::now() + sure,
                ardisik_hata: yeni_hata,
                son_hata_kodu: hata_kodu.map(|s| s.to_string()),
            },
        );

        sure
    }

    /// Başarılı işlem sonrası soğumayı ve sayaçları sıfırlar.
    pub fn basari_bildir(&mut self, hedef: &str) {
        self.kayitlar.remove(hedef);
    }
}
