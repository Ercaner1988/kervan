//! Kervan Akıllı Yönlendirici ve Dayanıklılık Motoru (Smart Router & Resilience).
//!
//! OmniRoute'un çoklu sağlayıcı yönlendirme yeteneklerini (LKGP, Auto-Combo, Round-Robin)
//! ve 3 katmanlı hata kalkanını (Circuit Breaker, Cooldown, Lockout) Rust'ta uygular.

use std::cmp::Ordering;
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

    /// Yalnız tam kanonik ad (kırpma/küçültme yok); kullanıcı girdisi için `normallestir`.
    pub fn from_str_canonical(s: &str) -> Option<Self> {
        match s {
            "priority" => Some(Self::Priority),
            "weighted" => Some(Self::Weighted),
            "round-robin" => Some(Self::RoundRobin),
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

    /// OmniRoute `normalizeRoutingStrategy`: takma adlar çözülür, bilinmeyen ad `priority` olur.
    pub fn normallestir(s: &str) -> Self {
        // JS trim(): U+FEFF kırpılır, U+0085 kırpılmaz (Rust is_whitespace tersini yapar).
        let kirpik =
            s.trim_matches(|c: char| (c.is_whitespace() && c != '\u{85}') || c == '\u{feff}');
        // JS toLowerCase yerelden bağımsız Unicode küçültmedir; Türkçe metin değil, strateji adı.
        match kirpik.to_lowercase().as_str() {
            "usage" => Self::LeastUsed,
            "context" => Self::ContextOptimized,
            "weekly-reset" | "reset-window-order" => Self::ResetWindow,
            ad => Self::from_str_canonical(ad).unwrap_or(Self::Priority),
        }
    }

    /// Sıralaması OmniRoute strateji çekirdeğiyle altından ölçülerek eşleşen stratejiler.
    /// Diğerleri (kota/sıfırlanma, auto, fusion, pipeline, cache, quota-share) henüz taşınmadı:
    /// `sirala` onlarda girdi sırasını (priority) döner, çağıran uyarı basar.
    pub fn desteklenir(&self) -> bool {
        matches!(
            self,
            Self::Priority
                | Self::FillFirst
                | Self::ContextRelay
                | Self::Weighted
                | Self::RoundRobin
                | Self::P2c
                | Self::Random
                | Self::StrictRandom
                | Self::LeastUsed
                | Self::CostOptimized
                | Self::Lkgp
                | Self::ContextOptimized
        )
    }

    /// OmniRoute strateji çekirdeği sırası (applyStrategyOrdering + weighted/round-robin
    /// çekirdekleri). Sıralamalar kararlıdır: eşitlikte özgün sıra korunur. `rastgele`
    /// [0, 1) kayan üretir (üretimde gerçek RNG, testte altındaki akış).
    // Biçim sabit: port/mutantlar.toml "strateji-least-used-yonunu-ters-cevir" bu gövdeyi arar.
    #[rustfmt::skip]
    #[allow(clippy::unnecessary_sort_by)]
    pub fn sirala(&self, hedefler: &[Hedef], durum: &mut StratejiDurumu, rastgele: &mut dyn FnMut() -> f64) -> Vec<Hedef> {
        let mut sonuc = hedefler.to_vec();
        let n = sonuc.len();
        if n == 0 {
            return sonuc;
        }
        match self {
            Self::Weighted => {
                // OmniRoute toWeight (steps.ts): sonlu değilse 0, [0, 100] aralığına kırpılır.
                let w = |h: &Hedef| if h.weight.is_finite() { h.weight.clamp(0.0, 100.0) } else { 0.0 };
                let toplam: f64 = sonuc.iter().map(w).sum();
                let secilen = if toplam <= 0.0 {
                    rastgele_tam(rastgele, n)
                } else {
                    let mut x = rastgele() * toplam;
                    let mut s = n - 1;
                    for (i, h) in sonuc.iter().enumerate() {
                        x -= w(h);
                        if x <= 0.0 { s = i; break; }
                    }
                    s
                };
                let anahtar = sonuc[secilen].execution_key.clone();
                let mut kalan: Vec<Hedef> = sonuc.iter().filter(|h| h.execution_key != anahtar).cloned().collect();
                kalan.sort_by(|a, b| w(b).partial_cmp(&w(a)).unwrap_or(Ordering::Equal));
                // JS find: aynı anahtarlı hedeflerden ilki başa geçer.
                let ilk = sonuc.iter().find(|h| h.execution_key == anahtar).cloned();
                sonuc = ilk.into_iter().chain(kalan).collect();
            }
            Self::RoundRobin => sonuc.rotate_left((durum.rr_sayac % n as u64) as usize),
            Self::P2c => {
                if n > 1 {
                    let i1 = rastgele_tam(rastgele, n);
                    let mut i2 = rastgele_tam(rastgele, n - 1);
                    if i2 >= i1 { i2 += 1; }
                    let secilen = if durum.p2c_puani(&sonuc[i2]) > durum.p2c_puani(&sonuc[i1]) { i2 } else { i1 };
                    let h = sonuc.remove(secilen);
                    sonuc.insert(0, h);
                }
            }
            Self::Random => karistir(&mut sonuc, rastgele),
            Self::StrictRandom => {
                let anahtarlar: Vec<String> = sonuc.iter().map(|h| h.execution_key.clone()).collect();
                let secilen = durum.desteden_cek(&anahtarlar, rastgele);
                let mut kalan: Vec<Hedef> = sonuc.iter().filter(|h| h.execution_key != secilen).cloned().collect();
                karistir(&mut kalan, rastgele);
                let ilk = sonuc.iter().find(|h| h.execution_key == secilen).cloned();
                sonuc = ilk.into_iter().chain(kalan).collect();
            }
            Self::LeastUsed => {
                if !durum.model.is_empty() {
                    let mut sayimlar: Vec<HedefSayimi> = sonuc.into_iter().map(|hedef| HedefSayimi {
                        request_count: durum.hedef_istek.get(&hedef.execution_key).copied().unwrap_or(0),
                        hedef,
                    }).collect();
                    sayimlar.sort_by(|a, b| a.request_count.cmp(&b.request_count));
                    sonuc = sayimlar.into_iter().map(|s| s.hedef).collect();
                }
            }
            Self::CostOptimized => {
                let fiyat = |h: &Hedef| durum.fiyat_bul(&h.model_str).filter(|f| f.is_finite()).unwrap_or(f64::INFINITY);
                sonuc.sort_by(|a, b| fiyat(a).partial_cmp(&fiyat(b)).unwrap_or(Ordering::Equal));
            }
            Self::Lkgp => {
                if let Some((saglayici, baglanti)) = &durum.lkgp {
                    let mut yer = baglanti.as_deref().filter(|b| !b.is_empty()).and_then(|b| {
                        sonuc.iter().position(|h| &h.provider == saglayici && h.connection_id.as_deref() == Some(b))
                    });
                    if yer.is_none() {
                        let onek = format!("{saglayici}/");
                        yer = sonuc.iter().position(|h| &h.provider == saglayici || h.model_str.starts_with(&onek));
                    }
                    if let Some(i) = yer && i > 0 {
                        let h = sonuc.remove(i);
                        sonuc.insert(0, h);
                    }
                }
            }
            // Hiçbir hedefin bağlamı bilinmiyorsa sıra değişmez (aşağıdaki _ kolu).
            Self::ContextOptimized if sonuc.iter().any(|h| durum.baglam.contains_key(&h.model_str)) => {
                let baglam = |h: &Hedef| durum.baglam.get(&h.model_str).copied().unwrap_or(0);
                sonuc.sort_by_key(|h| std::cmp::Reverse(baglam(h)));
            }
            // priority, fill-first, context-relay: çekirdekte sıra değişmez. Desteklenmeyenler de buraya düşer.
            _ => {}
        }
        sonuc
    }
}

/// OmniRoute `secureRandomInt` test yolu: n ≤ 1 ise akıştan çekmeden 0.
fn rastgele_tam(rastgele: &mut dyn FnMut() -> f64, n: usize) -> usize {
    if n <= 1 {
        return 0;
    }
    ((rastgele() * n as f64).floor() as usize).min(n - 1)
}

/// OmniRoute `fisherYatesShuffle`.
fn karistir<T>(l: &mut [T], rastgele: &mut dyn FnMut() -> f64) {
    for i in (1..l.len()).rev() {
        let j = rastgele_tam(rastgele, i + 1);
        l.swap(i, j);
    }
}

/// JS `Math.round` (pozitif sayılar için yarımı yukarı). floor(x + 0.5) değil:
/// 0.49999999999999994 + 0.5 kayan noktada 1 olur.
fn js_yuvarla(x: f64) -> f64 {
    let f = x.floor();
    if x - f >= 0.5 { f + 1.0 } else { f }
}

/// Bir yönlendirme hedefi (OmniRoute `ResolvedComboTarget`'in sıralamada kullanılan alanları).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Hedef {
    pub model_str: String,
    pub provider: String,
    pub execution_key: String,
    #[serde(default)]
    pub connection_id: Option<String>,
    #[serde(default)]
    pub weight: f64,
}

struct HedefSayimi {
    hedef: Hedef,
    request_count: u64,
}

#[derive(Debug, Clone, Default)]
struct ModelSayimi {
    istek: u64,
    basari: u64,
    toplam_gecikme_ms: f64,
}

#[derive(Debug, Clone)]
struct Deste {
    sira: Vec<String>,
    indeks: usize,
    kume: String,
}

/// Stratejilerin okuduğu durum: fiyat ve bağlam tabloları, istek metrikleri
/// (comboMetrics byModel/byTarget), LKGP kaydı, round-robin sayacı, strict-random destesi.
#[derive(Debug, Clone, Default)]
pub struct StratejiDurumu {
    /// model_str → girdi fiyatı; olmayan model ∞ sayılır.
    pub fiyat: HashMap<String, f64>,
    /// model_str → bağlam sınırı; olmayan model 0 sayılır.
    pub baglam: HashMap<String, u64>,
    /// (sağlayıcı, bağlantı) — son başarılı hedef.
    pub lkgp: Option<(String, Option<String>)>,
    pub rr_sayac: u64,
    model: HashMap<String, ModelSayimi>,
    hedef_istek: HashMap<String, u64>,
    deste: Option<Deste>,
}

impl StratejiDurumu {
    /// OmniRoute `recordComboRequest` karşılığı.
    pub fn kaydet(
        &mut self,
        model_str: &str,
        execution_key: &str,
        basarili: bool,
        gecikme_ms: f64,
    ) {
        // comboMetrics: model_str boşsa kayıt atlanır; execution_key boşsa model_str kullanılır.
        if model_str.is_empty() {
            return;
        }
        let execution_key = if execution_key.is_empty() {
            model_str
        } else {
            execution_key
        };
        let m = self.model.entry(model_str.to_string()).or_default();
        m.istek += 1;
        m.toplam_gecikme_ms += gecikme_ms;
        if basarili {
            m.basari += 1;
        }
        *self
            .hedef_istek
            .entry(execution_key.to_string())
            .or_default() += 1;
    }

    /// OmniRoute `getPricingForModel`: sağlayıcı ve model büyük/küçük harfe duyarsız; model
    /// bulunmazsa "." → "-" ile yeniden denenir. ponytail: sağlayıcı takma adları çözülmez.
    fn fiyat_bul(&self, model_str: &str) -> Option<f64> {
        if let Some(f) = self.fiyat.get(model_str) {
            return Some(*f);
        }
        let bol = |x: &str| {
            let (p, m) = x.split_once('/').unwrap_or(("", x));
            (p.to_lowercase(), m.to_lowercase())
        };
        let (p, m) = bol(model_str);
        let tireli = m.replace('.', "-");
        [m, tireli].into_iter().find_map(|aday| {
            self.fiyat
                .iter()
                .find(|(k, _)| bol(k) == (p.clone(), aday.clone()))
                .map(|(_, f)| *f)
        })
    }

    // ponytail: devre kesici HALF_OPEN cezası (-0,25) yok; açık devreler select_provider'da elenir.
    fn p2c_puani(&self, h: &Hedef) -> f64 {
        let Some(m) = self.model.get(&h.model_str).filter(|m| m.istek > 0) else {
            return 0.5 + 0.25;
        };
        let basari = js_yuvarla(m.basari as f64 / m.istek as f64 * 100.0) / 100.0;
        let gecikme = js_yuvarla(m.toplam_gecikme_ms / m.istek as f64);
        basari
            + if gecikme > 0.0 {
                1.0 / (gecikme + 10.0).log10()
            } else {
                0.25
            }
    }

    /// OmniRoute `getNextFromDeck`: her öğe döngüde bir kez; döngü sonu sonrakinin ilki olmaz;
    /// öğe kümesi değişince deste sıfırlanır.
    fn desteden_cek(&mut self, kimlikler: &[String], rastgele: &mut dyn FnMut() -> f64) -> String {
        match kimlikler {
            [] => return String::new(),
            [tek] => return tek.clone(),
            _ => {}
        }
        let mut sirali = kimlikler.to_vec();
        sirali.sort();
        let kume = sirali.join(",");
        if let Some(d) = &mut self.deste
            && d.kume == kume
            && d.indeks < d.sira.len()
        {
            d.indeks += 1;
            return d.sira[d.indeks - 1].clone();
        }
        let son = self
            .deste
            .as_ref()
            .filter(|d| d.kume == kume)
            .and_then(|d| d.sira.last().cloned());
        let mut yeni = kimlikler.to_vec();
        karistir(&mut yeni, rastgele);
        if son.as_ref() == Some(&yeni[0]) {
            let k = 1 + rastgele_tam(rastgele, yeni.len() - 1);
            yeni.swap(0, k);
        }
        let ilk = yeni[0].clone();
        self.deste = Some(Deste {
            sira: yeni,
            indeks: 1,
            kume,
        });
        ilk
    }
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
    durum: RwLock<StratejiDurumu>,
    desteksiz_uyarildi: std::sync::atomic::AtomicBool,
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
            durum: RwLock::new(StratejiDurumu::default()),
            desteksiz_uyarildi: std::sync::atomic::AtomicBool::new(false),
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

    /// Adaylar arasından stratejinin (OmniRoute çekirdeği, `RoutingStrategy::sirala`) ilk hedefini seçer.
    /// Aday = sağlayıcı adı. Üretimde fiyat ve bağlam tablosu YOK (OmniRoute'ta varsayılan,
    /// LiteLLM ve models.dev katmanları var): cost-optimized ve context-optimized burada sırayı
    /// değiştirmez. Strateji sunucuda şimdilik sabit lkgp kurulur; seçme yolu henüz yok.
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
        if !strat.desteklenir()
            && !self
                .desteksiz_uyarildi
                .swap(true, std::sync::atomic::Ordering::Relaxed)
        {
            eprintln!(
                "akilli-rota   : '{}' stratejisi Kervan'da henüz taşınmadı; priority sırası kullanılıyor",
                strat.as_str()
            );
        }
        let hedefler: Vec<Hedef> = pool
            .iter()
            .map(|c| Hedef {
                model_str: c.clone(),
                provider: c.clone(),
                execution_key: c.clone(),
                connection_id: None,
                weight: 1.0,
            })
            .collect();
        let mut durum = self.durum.write().await;
        let sirali = strat.sirala(&hedefler, &mut durum, &mut || rand::random::<f64>());
        if strat == RoutingStrategy::RoundRobin {
            durum.rr_sayac += 1;
        }
        sirali.into_iter().next().map(|h| h.execution_key)
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

        let mut durum = self.durum.write().await;
        durum.kaydet(provider, provider, true, latency_ms as f64);
        durum.lkgp = Some((provider.to_string(), None));
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

        // ponytail: hata gecikmesi çağırandan gelmiyor, 0 yazılır (OmniRoute gerçek gecikmeyi yazar).
        let mut durum = self.durum.write().await;
        durum.kaydet(provider, provider, false, 0.0);
        // LKGP bu sağlayıcı ise sıfırla ki bir sonraki istek başka yere yönlensin
        if durum.lkgp.as_ref().is_some_and(|(p, _)| p == provider) {
            durum.lkgp = None;
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
    async fn uretim_secimi_strateji_cekirdegini_kullanir() {
        let adaylar = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let rr = SmartRouter::new(RoutingStrategy::RoundRobin);
        let mut secilen = Vec::new();
        for _ in 0..4 {
            secilen.push(rr.select_provider(&adaylar).await.unwrap());
        }
        assert_eq!(secilen, ["a", "b", "c", "a"]);

        let az = SmartRouter::new(RoutingStrategy::LeastUsed);
        az.record_success("a", 10).await;
        az.record_success("b", 10).await;
        assert_eq!(az.select_provider(&adaylar).await.as_deref(), Some("c"));
    }

    #[test]
    fn js_yuvarla_math_round_ile_ayni() {
        // Math.round(0.49999999999999994) === 0; floor(x + 0.5) 1 verirdi.
        assert_eq!(js_yuvarla(0.499_999_999_999_999_94), 0.0);
        assert_eq!(js_yuvarla(2.5), 3.0);
        assert_eq!(js_yuvarla(2.4), 2.0);
    }

    #[tokio::test]
    async fn bos_aday_none_doner() {
        let router = SmartRouter::new(RoutingStrategy::Auto);
        assert_eq!(router.select_provider(&[]).await, None);
    }
}
