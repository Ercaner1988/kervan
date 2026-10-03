//! Model Kilidi (Model Lockout / Exclusion)
//!
//! Belirli bir sağlayıcı veya hesapta desteklenmeyen (404 Not Found, model erişimi yok)
//! modelleri geçici veya kalıcı olarak rotasyon dışına alan koruma katmanı.
//! OmniRoute modelLockoutFilter.ts mimarisine uygun saf Rust uygulaması.

use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct KilitKaydi {
    pub model: String,
    pub bitis_zamani: Option<Instant>, // None = kalıcı kilit
    pub gerekce: String,
}

#[derive(Debug, Default)]
pub struct ModelKilidi {
    kilitler: HashMap<String, KilitKaydi>,
}

impl ModelKilidi {
    pub fn yeni() -> Self {
        Self {
            kilitler: HashMap::new(),
        }
    }

    /// Bir modelin belirli bir anahtar (sağlayıcı::model) altında kilitli olup olmadığını denetler.
    pub fn kilitli_mi(&self, anahtar: &str) -> bool {
        self.kilitli_mi_zamaniyla(anahtar, Instant::now())
    }

    /// Belirtilen zamana göre kilit durumunu denetler.
    pub fn kilitli_mi_zamaniyla(&self, anahtar: &str, simdi: Instant) -> bool {
        if let Some(kayit) = self.kilitler.get(anahtar) {
            if let Some(bitis) = kayit.bitis_zamani {
                simdi < bitis
            } else {
                true // Kalıcı kilit
            }
        } else {
            false
        }
    }

    /// Modeli geçici süreyle kilitler.
    pub fn gecici_kilitle(&mut self, anahtar: &str, model: &str, sure: Duration, gerekce: &str) {
        self.gecici_kilitle_zamaniyla(anahtar, model, sure, gerekce, Instant::now());
    }

    /// Belirtilen zamanla modeli geçici süreyle kilitler.
    pub fn gecici_kilitle_zamaniyla(
        &mut self,
        anahtar: &str,
        model: &str,
        sure: Duration,
        gerekce: &str,
        simdi: Instant,
    ) {
        self.kilitler.insert(
            anahtar.to_string(),
            KilitKaydi {
                model: model.to_string(),
                bitis_zamani: Some(simdi + sure),
                gerekce: gerekce.to_string(),
            },
        );
    }

    /// Modeli kalıcı olarak kilitler.
    pub fn kalici_kilitle(&mut self, anahtar: &str, model: &str, gerekce: &str) {
        self.kilitler.insert(
            anahtar.to_string(),
            KilitKaydi {
                model: model.to_string(),
                bitis_zamani: None,
                gerekce: gerekce.to_string(),
            },
        );
    }

    /// Kilidi kaldırır.
    pub fn kilidi_ac(&mut self, anahtar: &str) {
        self.kilitler.remove(anahtar);
    }
}
