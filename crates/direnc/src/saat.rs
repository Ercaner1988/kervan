//! Saat soyutlaması ve enjekte edilebilir zaman sağlayıcısı.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Zaman kaynağı soyutlaması.
pub trait Saat: Send + Sync {
    fn simdi(&self) -> Instant;
}

/// Gerçek sistem saati (`Instant::now`).
#[derive(Debug, Clone, Default)]
pub struct GercekSaat;

impl Saat for GercekSaat {
    fn simdi(&self) -> Instant {
        Instant::now()
    }
}

/// Testler için saati ilerletilebilir sahte saat kaynağı.
#[derive(Debug, Clone)]
pub struct TestSaati {
    simdi: Arc<Mutex<Instant>>,
}

impl Default for TestSaati {
    fn default() -> Self {
        Self::yeni(Instant::now())
    }
}

impl TestSaati {
    pub fn yeni(baslangic: Instant) -> Self {
        Self {
            simdi: Arc::new(Mutex::new(baslangic)),
        }
    }

    pub fn ilerlet(&self, sure: Duration) {
        if let Ok(mut kilit) = self.simdi.lock() {
            *kilit += sure;
        }
    }

    pub fn ayarla(&self, yeni_an: Instant) {
        if let Ok(mut kilit) = self.simdi.lock() {
            *kilit = yeni_an;
        }
    }
}

impl Saat for TestSaati {
    fn simdi(&self) -> Instant {
        self.simdi.lock().map(|g| *g).unwrap_or_else(|_| Instant::now())
    }
}
