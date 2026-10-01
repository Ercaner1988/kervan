//! Kervan Akıllı Yönlendirici ve Dayanıklılık Motoru (Smart Router & Resilience).
//!
//! OmniRoute'un çoklu sağlayıcı yönlendirme yeteneklerini (LKGP, Auto-Combo, Round-Robin)
//! ve 3 katmanlı hata kalkanını (Circuit Breaker, Cooldown, Lockout) Rust'ta uygular.

use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingStrategy {
    /// Canlı sağlık, gecikme ve hata geçmişine göre en uygun sağlayıcıyı seçer.
    Auto,
    /// Son başarılı olan sağlayıcıya bağlı kalır (oturum tutarlılığı).
    Lkgp,
    /// Önceden tanımlı öncelik sırasına göre dener.
    Priority,
    /// İstekleri eşit şekilde dağıtır.
    RoundRobin,
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
