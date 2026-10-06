//! Kervan Akıllı Yönlendirici ve Dayanıklılık Motoru (Smart Router & Resilience).
//!
//! OmniRoute'un çoklu sağlayıcı yönlendirme yeteneklerini (LKGP, Auto-Combo, Round-Robin)
//! ve 3 katmanlı hata kalkanını (Circuit Breaker, Cooldown, Lockout) Rust'ta uygular.

use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RoutingStrategy {
    // 20 herkese açık strateji (ROUTING_STRATEGY_VALUES ile küme eşittir)
    Priority,
    Weighted,
    #[serde(rename = "round-robin")]
    RoundRobin,
    ContextRelay,
    FillFirst,
    P2c,
    Random,
    LeastUsed,
    CostOptimized,
    ResetAware,
    ResetWindow,
    Headroom,
    QuotaWeighted,
    StrictRandom,
    Auto,
    Lkgp,
    ContextOptimized,
    CacheOptimized,
    Fusion,
    Pipeline,
    // 1 dahili strateji (INTERNAL_ROUTING_STRATEGY_VALUES: kullanıcıya açılmaz)
    QuotaShare,
}

impl RoutingStrategy {
    pub const KAMUYA_ACIK: [&'static str; 20] = [
        "priority",
        "weighted",
        "round-robin",
        "context-relay",
        "fill-first",
        "p2c",
        "random",
        "least-used",
        "cost-optimized",
        "reset-aware",
        "reset-window",
        "headroom",
        "quota-weighted",
        "strict-random",
        "auto",
        "lkgp",
        "context-optimized",
        "cache-optimized",
        "fusion",
        "pipeline",
    ];

    pub const DAHILI: [&'static str; 1] = ["quota-share"];

    pub fn is_internal(&self) -> bool {
        matches!(self, Self::QuotaShare)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Priority => "priority",
            Self::Weighted => "weighted",
            Self::RoundRobin => "round-robin",
            Self::ContextRelay => "context-relay",
            Self::FillFirst => "fill-first",
            Self::P2c => "p2c",
            Self::Random => "random",
            Self::LeastUsed => "least-used",
            Self::CostOptimized => "cost-optimized",
            Self::ResetAware => "reset-aware",
            Self::ResetWindow => "reset-window",
            Self::Headroom => "headroom",
            Self::QuotaWeighted => "quota-weighted",
            Self::StrictRandom => "strict-random",
            Self::Auto => "auto",
            Self::Lkgp => "lkgp",
            Self::ContextOptimized => "context-optimized",
            Self::CacheOptimized => "cache-optimized",
            Self::Fusion => "fusion",
            Self::Pipeline => "pipeline",
            Self::QuotaShare => "quota-share",
        }
    }

    pub fn from_str_canonical(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "priority" => Some(Self::Priority),
            "weighted" => Some(Self::Weighted),
            "round-robin" | "roundrobin" => Some(Self::RoundRobin),
            "context-relay" => Some(Self::ContextRelay),
            "fill-first" => Some(Self::FillFirst),
            "p2c" => Some(Self::P2c),
            "random" => Some(Self::Random),
            "least-used" => Some(Self::LeastUsed),
            "cost-optimized" => Some(Self::CostOptimized),
            "reset-aware" => Some(Self::ResetAware),
            "reset-window" => Some(Self::ResetWindow),
            "headroom" => Some(Self::Headroom),
            "quota-weighted" => Some(Self::QuotaWeighted),
            "strict-random" => Some(Self::StrictRandom),
            "auto" => Some(Self::Auto),
            "lkgp" => Some(Self::Lkgp),
            "context-optimized" => Some(Self::ContextOptimized),
            "cache-optimized" => Some(Self::CacheOptimized),
            "fusion" => Some(Self::Fusion),
            "pipeline" => Some(Self::Pipeline),
            "quota-share" => Some(Self::QuotaShare),
            _ => None,
        }
    }

    /// Sabit girdi hedeflerini strateji kuralına ve tohumlu determinizme göre sıralar.
    // Biçim sabit: port/mutantlar.toml "strateji-least-used-yonunu-ters-cevir" bu gövdeyi arar.
    #[rustfmt::skip]
    pub fn sirala(&self, adaylar: &[HedefAdayi], tohum: Option<u64>) -> Vec<HedefAdayi> {
        if adaylar.is_empty() {
            return Vec::new();
        }
        let t = tohum.unwrap_or(42);
        let mut sonuc = adaylar.to_vec();

        match self {
            Self::Priority | Self::FillFirst => {
                // Öncelik sırasını korur
            }
            Self::Weighted => {
                sonuc.sort_by(|a, b| b.weight.cmp(&a.weight).then_with(|| a.ad.cmp(&b.ad)));
            }
            Self::RoundRobin => {
                let shift = (t as usize) % sonuc.len();
                sonuc.rotate_left(shift);
            }
            Self::ContextRelay => {
                // En geniş bağlama sahip olanı öne alır, kalanları korur
                sonuc.sort_by(|a, b| b.context_size.cmp(&a.context_size).then_with(|| a.ad.cmp(&b.ad)));
            }
            Self::P2c => {
                if sonuc.len() >= 2 {
                    // İki aday seç (P2C) ve gecikmesi düşük olanı öne al
                    let i1 = (t as usize) % sonuc.len();
                    let i2 = ((t.wrapping_mul(6364136223846793005).wrapping_add(1)) as usize) % sonuc.len();
                    let secilen_idx = if sonuc[i1].latency_ms <= sonuc[i2].latency_ms { i1 } else { i2 };
                    let secilen = sonuc.remove(secilen_idx);
                    sonuc.insert(0, secilen);
                }
            }
            Self::Random => {
                // Tam Fisher-Yates karıştırması (deterministik LCG)
                let mut state = t;
                for i in (1..sonuc.len()).rev() {
                    state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    let j = (state as usize) % (i + 1);
                    sonuc.swap(i, j);
                }
            }
            Self::StrictRandom => {
                // Desteden kart çeker gibi ilk kazananı seçer, kalanı farklı bir düzende karıştırır
                let state = t.wrapping_add(0x9e3779b97f4a7c15);
                let winner_idx = (state as usize) % sonuc.len();
                let winner = sonuc.remove(winner_idx);
                // Kalanı artan sırada düzenle
                sonuc.sort_by(|a, b| a.execution_key.cmp(&b.execution_key));
                sonuc.insert(0, winner);
            }
            Self::LeastUsed => {
                sonuc.sort_by(|a, b| {
                    a.request_count.cmp(&b.request_count)
                        .then_with(|| a.error_count.cmp(&b.error_count))
                        .then_with(|| a.ad.cmp(&b.ad))
                });
            }
            Self::CostOptimized => {
                sonuc.sort_by(|a, b| {
                    a.cost.partial_cmp(&b.cost).unwrap_or(std::cmp::Ordering::Equal).then_with(|| a.ad.cmp(&b.ad))
                });
            }
            Self::ResetAware => {
                // Kota / sıfırlanma süresi oranına göre skorlar
                sonuc.sort_by(|a, b| {
                    let score_a = (a.free_quota as f64) / (a.reset_seconds as f64 + 1.0);
                    let score_b = (b.free_quota as f64) / (b.reset_seconds as f64 + 1.0);
                    score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal).then_with(|| a.ad.cmp(&b.ad))
                });
            }
            Self::ResetWindow => {
                // Sıfırlanma penceresi yaklaşan (düşük reset_seconds) hedefleri önceler
                sonuc.sort_by(|a, b| {
                    a.reset_seconds.cmp(&b.reset_seconds).then_with(|| b.free_quota.cmp(&a.free_quota)).then_with(|| a.ad.cmp(&b.ad))
                });
            }
            Self::Headroom => {
                sonuc.sort_by(|a, b| b.free_quota.cmp(&a.free_quota).then_with(|| a.ad.cmp(&b.ad)));
            }
            Self::QuotaWeighted => {
                sonuc.sort_by(|a, b| {
                    let cap_a = a.free_quota * (a.weight as u64);
                    let cap_b = b.free_quota * (b.weight as u64);
                    cap_b.cmp(&cap_a).then_with(|| a.ad.cmp(&b.ad))
                });
            }
            Self::Auto => {
                sonuc.sort_by(|a, b| {
                    let score_a = a.latency_ms + (a.error_count as u64) * 5000;
                    let score_b = b.latency_ms + (b.error_count as u64) * 5000;
                    score_a.cmp(&score_b).then_with(|| a.ad.cmp(&b.ad))
                });
            }
            Self::Lkgp => {
                // İlk connection_id taşıyanı veya ilk elemanı öne al
                if let Some(pos) = sonuc.iter().position(|x| x.connection_id.is_some()) {
                    let item = sonuc.remove(pos);
                    sonuc.insert(0, item);
                }
            }
            Self::ContextOptimized => {
                sonuc.sort_by(|a, b| b.context_size.cmp(&a.context_size).then_with(|| a.ad.cmp(&b.ad)));
            }
            Self::CacheOptimized => {
                sonuc.sort_by(|a, b| b.warm_cache.cmp(&a.warm_cache).then_with(|| a.ad.cmp(&b.ad)));
            }
            Self::Fusion => {
                // Model adına göre kümeleyip sıralar
                sonuc.sort_by(|a, b| a.execution_key.cmp(&b.execution_key).then_with(|| a.ad.cmp(&b.ad)));
            }
            Self::Pipeline => {
                // Boru hattı adımlarına göre (weight + context_size)
                sonuc.sort_by(|a, b| (b.weight, b.context_size).cmp(&(a.weight, a.context_size)).then_with(|| a.ad.cmp(&b.ad)));
            }
            Self::QuotaShare => {
                // DRR (Deficit Round Robin): serbest kota ve gecikme oranı
                sonuc.sort_by(|a, b| {
                    let drr_a = (a.free_quota as f64) / (a.latency_ms as f64 + 1.0);
                    let drr_b = (b.free_quota as f64) / (b.latency_ms as f64 + 1.0);
                    drr_b.partial_cmp(&drr_a).unwrap_or(std::cmp::Ordering::Equal).then_with(|| a.ad.cmp(&b.ad))
                });
            }
        }

        sonuc
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HedefAdayi {
    pub ad: String,
    pub execution_key: String,
    #[serde(default)]
    pub connection_id: Option<String>,
    #[serde(default)]
    pub weight: u32,
    #[serde(default)]
    pub cost: f64,
    #[serde(default)]
    pub context_size: usize,
    #[serde(default)]
    pub latency_ms: u64,
    #[serde(default)]
    pub error_count: u32,
    #[serde(default)]
    pub request_count: u64,
    #[serde(default)]
    pub free_quota: u64,
    #[serde(default)]
    pub reset_seconds: u64,
    #[serde(default)]
    pub warm_cache: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ProviderMetrics {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub consecutive_errors: u32,
    pub last_latency_ms: u64,
    pub circuit_open_until: Option<Instant>,
}

pub struct SmartRouter {
    pub strategy: RwLock<RoutingStrategy>,
    metrics: RwLock<HashMap<String, ProviderMetrics>>,
    last_known_good: RwLock<Option<String>>,
    round_robin_counter: std::sync::atomic::AtomicUsize,
}

impl Default for SmartRouter {
    fn default() -> Self {
        Self::new(RoutingStrategy::Lkgp)
    }
}

impl SmartRouter {
    pub fn new(default_strategy: RoutingStrategy) -> Self {
        Self {
            strategy: RwLock::new(default_strategy),
            metrics: RwLock::new(HashMap::new()),
            last_known_good: RwLock::new(None),
            round_robin_counter: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Bir sağlayıcının şu an devre kesicide (kilitli) olup olmadığını denetler.
    pub async fn is_available(&self, provider: &str) -> bool {
        let metrics = self.metrics.read().await;
        if let Some(m) = metrics.get(provider)
            && let Some(until) = m.circuit_open_until
            && Instant::now() < until
        {
            return false; // Devre kesici açık (soğumada)
        }
        true
    }

    /// Adaylar arasından stratejiye uygun en iyi sağlayıcıyı seçer.
    pub async fn select_provider(&self, candidates: &[String]) -> Option<String> {
        if candidates.is_empty() {
            return None;
        }

        // Sadece devre kesicisi açık olmayan (sağlıklı) adayları filtrele
        let mut healthy: Vec<String> = Vec::new();
        for c in candidates {
            if self.is_available(c).await {
                healthy.push(c.clone());
            }
        }

        let pool = if healthy.is_empty() {
            candidates
        } else {
            &healthy
        };

        let strat = *self.strategy.read().await;
        match strat {
            RoutingStrategy::Lkgp => {
                let lkgp = self.last_known_good.read().await;
                if let Some(ref last) = *lkgp
                    && pool.contains(last)
                {
                    return Some(last.clone());
                }
                // LKGP yoksa veya devre dışıysa ilk sağlıklı adaya düş
                pool.first().cloned()
            }
            RoutingStrategy::Auto => {
                // En düşük gecikmeye ve en az hataya sahip olanı puanla
                let metrics = self.metrics.read().await;
                let mut best_score = f64::MAX;
                let mut selected = pool[0].clone();

                for c in pool {
                    if let Some(m) = metrics.get(c) {
                        // Skor = Gecikme (ms) + (Ardışık Hata * 5000ms ceza)
                        let score =
                            (m.last_latency_ms as f64) + ((m.consecutive_errors as f64) * 5000.0);
                        if score < best_score {
                            best_score = score;
                            selected = c.clone();
                        }
                    } else {
                        // Henüz metriği olmayan yeni adayı hemen dene
                        return Some(c.clone());
                    }
                }
                Some(selected)
            }
            RoutingStrategy::RoundRobin => {
                let idx = self
                    .round_robin_counter
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                Some(pool[idx % pool.len()].clone())
            }
            RoutingStrategy::Priority => pool.first().cloned(),
            _ => pool.first().cloned(),
        }
    }

    /// Bir istek başarıyla sonuçlandığında çağrılır.
    pub async fn record_success(&self, provider: &str, latency_ms: u64) {
        let mut metrics = self.metrics.write().await;
        let m = metrics.entry(provider.to_string()).or_default();
        m.total_requests += 1;
        m.successful_requests += 1;
        m.consecutive_errors = 0;
        m.last_latency_ms = latency_ms;
        m.circuit_open_until = None;

        let mut lkgp = self.last_known_good.write().await;
        *lkgp = Some(provider.to_string());
    }

    /// Bir istek hata aldığında çağrılır (Circuit Breaker tetikleyicisi).
    pub async fn record_failure(&self, provider: &str) {
        let mut metrics = self.metrics.write().await;
        let m = metrics.entry(provider.to_string()).or_default();
        m.total_requests += 1;
        m.failed_requests += 1;
        m.consecutive_errors += 1;

        // 3 ardışık hatada devre kesici 30 saniye devreye girer
        if m.consecutive_errors >= 3 {
            println!(
                "devre-kesici : [{provider}] 3 ardışık hata nedeniyle 30 sn devre dışı bırakıldı"
            );
            m.circuit_open_until = Some(Instant::now() + Duration::from_secs(30));
        }

        // LKGP bu sağlayıcı ise sıfırla ki bir sonraki istek başka yere yönlensin
        let mut lkgp = self.last_known_good.write().await;
        if lkgp.as_deref() == Some(provider) {
            *lkgp = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lkgp_son_basariliyi_hatirlar() {
        let router = SmartRouter::new(RoutingStrategy::Lkgp);
        let adaylar = vec!["gemini".to_string(), "kimi".to_string()];

        assert_eq!(
            router.select_provider(&adaylar).await.as_deref(),
            Some("gemini")
        );

        router.record_success("kimi", 200).await;
        assert_eq!(
            router.select_provider(&adaylar).await.as_deref(),
            Some("kimi")
        );

        router.record_failure("kimi").await;
        assert_eq!(
            router.select_provider(&adaylar).await.as_deref(),
            Some("gemini")
        );
    }

    #[tokio::test]
    async fn devre_kesici_uc_hatada_kilitler() {
        let router = SmartRouter::new(RoutingStrategy::Priority);
        let adaylar = vec!["gemini".to_string(), "kimi".to_string()];

        router.record_failure("gemini").await;
        router.record_failure("gemini").await;
        assert!(router.is_available("gemini").await);

        router.record_failure("gemini").await;
        assert!(!router.is_available("gemini").await);

        // Devre kesici açıldığında sonraki sağlıklı adaya geçer
        assert_eq!(
            router.select_provider(&adaylar).await.as_deref(),
            Some("kimi")
        );
    }

    #[tokio::test]
    async fn bos_aday_none_doner() {
        let router = SmartRouter::new(RoutingStrategy::Auto);
        assert_eq!(router.select_provider(&[]).await, None);
    }
}
