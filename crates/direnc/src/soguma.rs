//! Bağlantı ve Sağlayıcı Soğuma (Cooldown / Backoff) Takipçisi
//!
//! 429 Too Many Requests ve aşırı yüklenme durumlarında üstel geri çekilme ve Retry-After yönetimi.
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
        self.sogumada_mi_zamaniyla(hedef, Instant::now())
    }

    /// Belirtilen zamana göre soğuma durumunu denetler.
    pub fn sogumada_mi_zamaniyla(&self, hedef: &str, simdi: Instant) -> bool {
        if let Some(kayit) = self.kayitlar.get(hedef) {
            simdi < kayit.bitis_zamani
        } else {
            false
        }
    }

    /// Kalan soğuma süresini saniye olarak döner. Soğumada değilse 0 döner.
    pub fn kalan_sure_saniye(&self, hedef: &str) -> u64 {
        self.kalan_sure_saniye_zamaniyla(hedef, Instant::now())
    }

    /// Belirtilen zamana göre kalan süreyi saniye olarak döner.
    pub fn kalan_sure_saniye_zamaniyla(&self, hedef: &str, simdi: Instant) -> u64 {
        if let Some(kayit) = self.kayitlar.get(hedef) {
            if simdi < kayit.bitis_zamani {
                let kalan = kayit.bitis_zamani.duration_since(simdi);
                return kalan.as_secs().max(1);
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
        self.soguma_baslat_zamaniyla(hedef, hata_kodu, retry_after_saniye, Instant::now())
    }

    /// Belirtilen zamanla soğuma başlatır (test edilebilirlik için).
    pub fn soguma_baslat_zamaniyla(
        &mut self,
        hedef: &str,
        hata_kodu: Option<&str>,
        retry_after_saniye: Option<u64>,
        simdi: Instant,
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
                bitis_zamani: simdi + sure,
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
