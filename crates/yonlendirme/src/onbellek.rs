use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Anlamsal / İstem Önbelleği Kaydı
struct OnbellekKaydi {
    yanit: String,
    olusturulma: Instant,
    son_erisim: Instant,
}

/// Anlamsal / Hash İstem Önbelleği
pub struct AnlamsalOnbellek {
    kayitlar: Mutex<HashMap<u64, OnbellekKaydi>>,
    azami_kapasite: usize,
    ttl: Duration,
}

impl AnlamsalOnbellek {
    pub fn yeni(azami_kapasite: usize, ttl: Duration) -> Self {
        Self {
            kayitlar: Mutex::new(HashMap::new()),
            azami_kapasite,
            ttl,
        }
    }

    /// İstem ve model parametrelerinden 64-bit özet üretir
    pub fn ozetle(model: &str, istem: &str, sicaklik: f32) -> u64 {
        let mut hasher = DefaultHasher::new();
        model.hash(&mut hasher);
        istem.hash(&mut hasher);
        ((sicaklik * 100.0) as i32).hash(&mut hasher);
        hasher.finish()
    }

    /// Önbellekten getirir (TTL kontrolü ve son erişim güncellemesiyle)
    pub fn getir(&self, ozet: u64) -> Option<String> {
        let mut kilit = match self.kayitlar.lock() {
            Ok(guard) => guard,
            Err(zehirli) => zehirli.into_inner(),
        };

        if let Some(kayit) = kilit.get_mut(&ozet)
            && kayit.olusturulma.elapsed() <= self.ttl
        {
            kayit.son_erisim = Instant::now();
            return Some(kayit.yanit.clone());
        }
        None
    }

    /// Önbelleğe kaydeder; kapasite dolduğunda en eski erişileni tahliye eder (LRU)
    pub fn kaydet(&self, ozet: u64, yanit: String) {
        let mut kilit = match self.kayitlar.lock() {
            Ok(guard) => guard,
            Err(zehirli) => zehirli.into_inner(),
        };

        let simdi = Instant::now();

        // Süresi dolmuşları temizle
        let ttl = self.ttl;
        kilit.retain(|_, v| v.olusturulma.elapsed() <= ttl);

        // Kapasite aşıldıysa en eski erişileni çıkar
        if kilit.len() >= self.azami_kapasite
            && let Some((&en_eski_anahtar, _)) = kilit.iter().min_by_key(|(_, v)| v.son_erisim)
        {
            kilit.remove(&en_eski_anahtar);
        }

        kilit.insert(
            ozet,
            OnbellekKaydi {
                yanit,
                olusturulma: simdi,
                son_erisim: simdi,
            },
        );
    }

    /// Mevcut geçerli kayıt sayısını döner
    pub fn kayit_sayisi(&self) -> usize {
        let kilit = match self.kayitlar.lock() {
            Ok(guard) => guard,
            Err(zehirli) => zehirli.into_inner(),
        };
        kilit.len()
    }
}
