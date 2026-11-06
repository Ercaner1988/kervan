//! Kervan Akıllı Yönlendirici ve Dayanıklılık Motoru (Smart Router & Resilience).
//!
//! OmniRoute'un çoklu sağlayıcı yönlendirme yeteneklerini (LKGP, Auto-Combo, Round-Robin)
//! ve 3 katmanlı hata kalkanını (Circuit Breaker, Cooldown, Lockout) Rust'ta uygular.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::kota_duyarli as kota;

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
    /// Kota duyarlı dördü (`reset-aware`, `reset-window`, `headroom`, `quota-weighted`)
    /// ancak çağıran `StratejiDurumu`'na kota sinyali beslediğinde ayırt edici olur; sinyal
    /// yokken OmniRoute gibi girdi sırasına düşerler. Taşınmayanlar (`auto`, `fusion`,
    /// `pipeline`, `cache-optimized`, `quota-share`) her durumda `priority` davranır.
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
                | Self::ResetAware
                | Self::ResetWindow
                | Self::Headroom
                | Self::QuotaWeighted
        )
    }

    /// OmniRoute strateji çekirdeği sırası (applyStrategyOrdering + weighted/round-robin
    /// çekirdekleri). Sıralamalar kararlıdır: eşitlikte özgün sıra korunur. `rastgele`
    /// [0, 1) kayan üretir (üretimde gerçek RNG, testte altındaki akış).
    // Biçim sabit: port/mutantlar.toml "strateji-least-used-yonunu-ters-cevir" bu gövdeyi arar.
    #[rustfmt::skip]
    #[allow(clippy::unnecessary_sort_by, reason = "sort_by biçimi port/mutantlar.toml mutantının aradığı gövdedir")]
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
            // OmniRoute `orderTargetsByHeadroom` / `rankByHeadroom`: en çok boş kapasite
            // (1 - max(util5h, util7d)) başa; eşitlikte özgün sıra.
            Self::Headroom => {
                let sinyal = |h: &Hedef| durum.headroom.get(&h.execution_key);
                let mut dekor: Vec<(usize, f64)> = sonuc.iter().enumerate().map(|(i, h)| {
                    (i, sinyal(h).map_or(1.0, |s| kota::headroom(s.util5h, s.util7d, s.util_gun)))
                }).collect();
                dekor.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal).then(a.0.cmp(&b.0)));
                sonuc = dekor.iter().map(|(i, _)| sonuc[*i].clone()).collect();
            }
            // OmniRoute `orderTargetsByResetWindow`: sıfırlanmaya kalan ms artan; bilinmeyen
            // (+∞) sona. En iyi sonlu ve tie_band > 0 ise baştaki eşitlik kümesi döndürülür.
            Self::ResetWindow => {
                let kalan = |h: &Hedef| kota::reset_window_kalan_ms(
                    durum.kota.get(&h.execution_key),
                    &durum.reset_window_ayar.pencereler,
                    durum.simdi_ms,
                );
                let mut dekor: Vec<(usize, f64)> = sonuc.iter().enumerate().map(|(i, h)| (i, kalan(h))).collect();
                dekor.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal).then(a.0.cmp(&b.0)));
                // Boş dekorda değer kullanılmaz; `f64::MAX` seçildi ki bu satır
                // `port/mutantlar.toml` içindeki fiyat mutantının metniyle
                // çakışmasın (mutant tek bir yeri hedeflemeli).
                let en_iyi = dekor.first().map(|d| d.1).unwrap_or(f64::MAX);
                let bant = durum.reset_window_ayar.tie_band_ms;
                let sirali: Vec<usize> = dekor.iter().map(|(i, _)| *i).collect();
                let sira = if en_iyi.is_finite() && bant > 0.0 {
                    let bagli: Vec<usize> = dekor.iter().filter(|(_, k)| *k - en_iyi <= bant).map(|(i, _)| *i).collect();
                    if bagli.len() > 1 {
                        one_cikanlari_dondur(durum, "reset-window", &sonuc, &sirali, &bagli)
                    } else {
                        sirali
                    }
                } else {
                    sirali
                };
                sonuc = sira.iter().map(|i| sonuc[*i].clone()).collect();
            }
            // OmniRoute `orderTargetsByResetAwareQuota`: skor azalan, eşitlikte özgün indeks;
            // baştaki eşitlik kümesi (`en_iyi - skor <= tie_band`) döndürülür.
            Self::ResetAware => {
                let skor = |h: &Hedef| kota::reset_aware_skor(
                    durum.kota.get(&h.execution_key),
                    &durum.reset_aware_ayar,
                    durum.simdi_ms,
                );
                let mut dekor: Vec<(usize, f64)> = sonuc.iter().enumerate().map(|(i, h)| (i, skor(h))).collect();
                dekor.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal).then(a.0.cmp(&b.0)));
                let en_iyi = dekor.first().map(|d| d.1).unwrap_or(0.0);
                let bant = durum.reset_aware_ayar.tie_band;
                // -∞ - (-∞) = NaN → karşılaştırma false: OmniRoute da eşitlik kurmaz.
                let bagli: Vec<usize> = dekor.iter().filter(|(_, s)| en_iyi - *s <= bant).map(|(i, _)| *i).collect();
                let sirali: Vec<usize> = dekor.iter().map(|(i, _)| *i).collect();
                let sira = if bagli.len() > 1 {
                    one_cikanlari_dondur(durum, "reset-aware", &sonuc, &sirali, &bagli)
                } else {
                    sirali
                };
                sonuc = sira.iter().map(|i| sonuc[*i].clone()).collect();
            }
            // OmniRoute `orderTargetsByQuotaWeighted`: A havuzu (yer var + taze) yoksa B;
            // ağırlık `max(0, skor) / (1 + uçuştaki)`, yarım-açık ağırlıklı çekiliş.
            // SAPMA: OmniRoute seçilebilir hedef kalmazsa boş liste döner; Kervan'da boş
            // liste "aday yok" demek olacağından girdi sırası korunur (fail-open).
            // SAPMA: kazanan için `incrementInflight` yapılmaz (Kervan'da bırakma yolu yok).
            Self::QuotaWeighted => {
                let skor = |i: usize| kota::reset_aware_skor(
                    durum.kota.get(&sonuc[i].execution_key),
                    &durum.reset_aware_ayar,
                    durum.simdi_ms,
                );
                let yuzde = |i: usize| kota::kalan_yuzde(durum.kota.get(&sonuc[i].execution_key));
                let bayat = |i: usize| durum.kota.get(&sonuc[i].execution_key).is_some_and(|s| s.bayat);
                let floor = durum.quota_weighted_floor;
                let oda_var = |i: usize| floor == 0.0 || yuzde(i) > floor;
                let uygun: Vec<usize> = (0..sonuc.len()).filter(|i| yuzde(*i) > 0.0).collect();
                let a: Vec<usize> = uygun.iter().copied().filter(|i| oda_var(*i) && !bayat(*i)).collect();
                let b: Vec<usize> = uygun.iter().copied().filter(|i| !oda_var(*i) || bayat(*i)).collect();
                let secili: Vec<usize> = if a.is_empty() { b.clone() } else { a.clone() };
                if !secili.is_empty() {
                    let agirliklar: Vec<f64> = secili.iter().map(|i| {
                        let yuk = durum.inflight.get(&baglanti_anahtari(&sonuc[*i])).copied().unwrap_or(0) as f64;
                        skor(*i).max(0.0) / (1.0 + yuk)
                    }).collect();
                    let toplam: f64 = agirliklar.iter().sum();
                    let kazanan = if toplam > 0.0 {
                        kota::agirlikli_secim(&agirliklar, rastgele() * toplam)
                    } else {
                        None
                    };
                    let kazanan = kazanan.unwrap_or_else(|| rastgele_tam(rastgele, secili.len()));
                    let mut digerleri: Vec<usize> = secili.iter().enumerate()
                        .filter(|(j, _)| *j != kazanan)
                        .map(|(_, i)| *i)
                        .collect();
                    skora_gore_sirala(&mut digerleri, &skor);
                    let mut sira = vec![secili[kazanan]];
                    sira.extend(digerleri);
                    if !a.is_empty() {
                        let mut yedek = b.clone();
                        skora_gore_sirala(&mut yedek, &skor);
                        sira.extend(yedek);
                    }
                    sonuc = sira.iter().map(|i| sonuc[*i].clone()).collect();
                }
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

/// OmniRoute `rotateLeadingTies`: baştaki eşitlik kümesini sayaçla döndürür, eşit
/// olmayanları özgün sırada arkaya ekler. `sirali` ve `bagli`, `hedefler` içindeki
/// konumlardır; `bagli` `sirali` ile aynı düzendedir. Dönen liste de `hedefler`
/// konumlarıdır.
fn one_cikanlari_dondur(
    durum: &mut StratejiDurumu,
    onek: &str,
    hedefler: &[Hedef],
    sirali: &[usize],
    bagli: &[usize],
) -> Vec<usize> {
    let mut dondurulmus = bagli.to_vec();
    if dondurulmus.len() > 1 {
        let anahtar = format!("{onek}:{}", durum.combo_adi);
        let sayac = durum
            .rr_kenar_sayaclari
            .iter()
            .find(|(k, _)| *k == anahtar)
            .map(|(_, v)| *v)
            .unwrap_or(0);
        let var = durum.rr_kenar_sayaclari.iter().any(|(k, _)| *k == anahtar);
        if !var && durum.rr_kenar_sayaclari.len() >= kota::AZAMI_RR_SAYACI {
            durum.rr_kenar_sayaclari.remove(0);
        }
        match durum
            .rr_kenar_sayaclari
            .iter_mut()
            .find(|(k, _)| *k == anahtar)
        {
            Some((_, v)) => *v = sayac + 1,
            None => durum.rr_kenar_sayaclari.push((anahtar, sayac + 1)),
        }
        let bas = (sayac as usize) % dondurulmus.len();
        dondurulmus.rotate_left(bas);
    }
    // OmniRoute `tiedExecutionKeys`: küme execution_key üzerinden kurulur, bu yüzden
    // aynı execution_key'e sahip ikinci bir hedef de kuyruktan düşer.
    let kume: Vec<&str> = dondurulmus
        .iter()
        .map(|i| hedefler[*i].execution_key.as_str())
        .collect();
    let mut sonuc = dondurulmus;
    sonuc.extend(
        sirali
            .iter()
            .copied()
            .filter(|i| !kume.contains(&hedefler[*i].execution_key.as_str())),
    );
    sonuc
}

/// OmniRoute `sortByScoreThenIndex`: skor azalan, eşitlikte özgün konum artan.
fn skora_gore_sirala(s: &mut [usize], skor: &dyn Fn(usize) -> f64) {
    s.sort_by(|a, b| {
        skor(*b)
            .partial_cmp(&skor(*a))
            .unwrap_or(Ordering::Equal)
            .then(a.cmp(b))
    });
}

/// OmniRoute `getInflight(connectionId ?? "")`: uçuştaki istek anahtarı bağlantı
/// kimliğidir; bağlantısız hedefler tek bir "boş" kovayı paylaşır (OmniRoute ile aynı).
fn baglanti_anahtari(h: &Hedef) -> String {
    h.connection_id.clone().unwrap_or_default()
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

impl Hedef {
    /// Sağlayıcı düzeyi hedef: hesap bilinmiyor (eklenti sekmesi, REST hedefi ya
    /// da tek hesaplı sağlayıcı). `execution_key` = sağlayıcı adı.
    pub fn saglayici(ad: &str) -> Self {
        Self {
            model_str: ad.to_string(),
            provider: ad.to_string(),
            execution_key: ad.to_string(),
            connection_id: None,
            weight: 1.0,
        }
    }

    /// Hesap düzeyi hedef: `execution_key` = `saglayici/hesap`, `connection_id` =
    /// hesap adı, `model_str` = sağlayıcı (fiyat/bağlam tabloları sağlayıcıya
    /// bağlıdır; hesap seçimi fiyatı değiştirmez).
    pub fn hesap(saglayici: &str, hesap: &str) -> Self {
        Self {
            model_str: saglayici.to_string(),
            provider: saglayici.to_string(),
            execution_key: hedef_anahtari(saglayici, Some(hesap)),
            connection_id: Some(hesap.to_string()),
            weight: 1.0,
        }
    }
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

/// Bir hedefin kota doygunluğu (OmniRoute `HeadroomSaturation` + günlük pencere eki).
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HeadroomSinyali {
    #[serde(default)]
    pub util5h: f64,
    #[serde(default)]
    pub util7d: f64,
    /// Günlük (geriye dönük 24 saat) doluluk; 0,0 = günlük tavan tanımlı değil.
    #[serde(default)]
    pub util_gun: f64,
}

/// Stratejilerin okuduğu durum: fiyat ve bağlam tabloları, istek metrikleri
/// (comboMetrics byModel/byTarget), LKGP kaydı, round-robin sayacı, strict-random destesi,
/// kota anlık görüntüleri ve kota duyarlı stratejilerin ayarları.
#[derive(Debug, Clone)]
pub struct StratejiDurumu {
    /// model_str → girdi fiyatı; olmayan model ∞ sayılır.
    pub fiyat: HashMap<String, f64>,
    /// model_str → bağlam sınırı; olmayan model 0 sayılır.
    pub baglam: HashMap<String, u64>,
    /// (sağlayıcı, bağlantı) — son başarılı hedef.
    pub lkgp: Option<(String, Option<String>)>,
    pub rr_sayac: u64,
    /// execution_key → kota anlık görüntüsü (`reset-aware`, `reset-window`,
    /// `quota-weighted`). Sinyal yoksa o hedef OmniRoute gibi 0,5 skorla / bilinmeyen
    /// sıfırlanmayla değerlendirilir.
    pub kota: HashMap<String, kota::KotaSnapshot>,
    /// execution_key → (util5h, util7d) doygunluğu (`headroom`). Sinyal yoksa tam
    /// boşluk (1,0) sayılır — OmniRoute fail-open davranışı.
    pub headroom: HashMap<String, HeadroomSinyali>,
    pub reset_aware_ayar: kota::ResetAwareAyar,
    pub reset_window_ayar: kota::ResetWindowAyar,
    /// `quota-weighted` yüzde eşiği (0–100). OmniRoute `resolveQuotaWeightedFloor`.
    pub quota_weighted_floor: f64,
    /// bağlantı anahtarı → uçuştaki istek sayısı (`quota-weighted` ağırlığı).
    pub inflight: HashMap<String, u32>,
    /// Sıralamanın tek "şimdi"si (epoch ms) — OmniRoute `#9330` gerekçesi: sıralama
    /// içinde saat yeniden okunmaz.
    pub simdi_ms: i64,
    /// Tie-band döndürme sayacının anahtar öneki (`reset-aware:<combo>`).
    ///
    /// Üretimde boş: Kervan'da tek "combo" var (istek başına tek hedef listesi),
    /// bu yüzden sayaç anahtarı `onek:` olur ve tüm hedefler aynı sayacı paylaşır.
    /// Çok-combo desteklenirse buraya combo kimliği yazılmalı.
    pub combo_adi: String,
    model: HashMap<String, ModelSayimi>,
    hedef_istek: HashMap<String, u64>,
    deste: Option<Deste>,
    /// OmniRoute `rrCounters`: (anahtar, sayaç) ekleme sırasıyla. `MAX_RR_COUNTERS`
    /// dolunca en eski (baştaki) atılır.
    rr_kenar_sayaclari: Vec<(String, u64)>,
}

impl Default for StratejiDurumu {
    fn default() -> Self {
        Self {
            fiyat: HashMap::new(),
            baglam: HashMap::new(),
            lkgp: None,
            rr_sayac: 0,
            kota: HashMap::new(),
            headroom: HashMap::new(),
            reset_aware_ayar: kota::ResetAwareAyar::default(),
            reset_window_ayar: kota::ResetWindowAyar::default(),
            quota_weighted_floor: 1.0,
            inflight: HashMap::new(),
            simdi_ms: 0,
            combo_adi: String::new(),
            model: HashMap::new(),
            hedef_istek: HashMap::new(),
            deste: None,
            rr_kenar_sayaclari: Vec::new(),
        }
    }
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

    /// Sağlayıcı adlarından aday sırası. Hesap düzeyi gerektirmeyen çağrılar için
    /// kolaylık sarmalayıcısı; üretim yolu [`Self::hedef_sirasi`].
    ///
    /// Üretimde fiyat ve bağlam tablosu YOK (OmniRoute'ta varsayılan, LiteLLM ve
    /// models.dev katmanları var): cost-optimized ve context-optimized burada
    /// sırayı değiştirmez. Strateji `kervan.toml` `[yonlendirme]` bölümünden
    /// kurulur ve `/yonlendirme` ile değiştirilebilir.
    pub async fn aday_sirasi(&self, candidates: &[String]) -> Vec<String> {
        let hedefler: Vec<Hedef> = candidates
            .iter()
            .map(|c| Hedef {
                model_str: c.clone(),
                provider: c.clone(),
                execution_key: c.clone(),
                connection_id: None,
                weight: 1.0,
            })
            .collect();
        self.hedef_sirasi(&hedefler)
            .await
            .into_iter()
            .map(|h| h.execution_key)
            .collect()
    }

    /// Hesap düzeyi adayları stratejiye göre SIRALAR (tüm liste).
    ///
    /// Aday = (sağlayıcı, hesap). Aynı sağlayıcının hesapları ayrı adaydır: kota
    /// ve kullanım sayacı hesap başına işlediği için `least-used` gibi stratejiler
    /// ancak böyle hesap seçebilir. Devre kesici hâlâ SAĞLAYICI düzeyindedir
    /// (`Hedef::provider`), bu yüzden bir sağlayıcının bütün hesapları birlikte
    /// elenir — tek bozuk hesap sağlayıcının tamamını düşürmez.
    pub async fn hedef_sirasi(&self, candidates: &[Hedef]) -> Vec<Hedef> {
        if candidates.is_empty() {
            return Vec::new();
        }

        // Sadece devre kesicisi açık olmayan (sağlıklı) adayları filtrele
        let mut healthy: Vec<Hedef> = Vec::new();
        for c in candidates {
            if self.is_available(&c.provider).await {
                healthy.push(c.clone());
            }
        }
        let pool: &[Hedef] = if healthy.is_empty() {
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
        let mut durum = self.durum.write().await;
        // OmniRoute `#9330`: sıralama içinde saat yeniden okunmaz, tek "şimdi"
        // yazılır. Bu alan yazılmadığı sürece `reset-aware`in basınç terimi
        // sessizce ölüydü (hep 0 ms).
        durum.simdi_ms = simdi_epoch_ms();
        let sirali = strat.sirala(pool, &mut durum, &mut || rand::random::<f64>());
        if strat == RoutingStrategy::RoundRobin {
            durum.rr_sayac += 1;
        }
        sirali
    }

    /// Kota sinyallerini TEK yerden besler: `execution_key` → anlık görüntü.
    ///
    /// Aynı harita hem `kota` (reset-aware / reset-window / quota-weighted) hem
    /// `headroom` (headroom stratejisi) alanlarını doldurur; `headroom` sinyali
    /// anlık görüntüden TÜRETİLİR ki iki kaynak birbirinden ayrı düşmesin.
    ///
    /// Haritada olmayan hedef "sinyal yok" demektir: kota stratejileri OmniRoute
    /// fail-open davranışıyla (0,5 skor / tam boşluk) değerlendirir, yani pratikte
    /// `priority` gibi davranır. Sinyalin nasıl üretildiği çağıranın işidir
    /// (sunucu: kota defteri özeti + `[kota]` tavanları); router G/Ç yapmaz.
    pub async fn kota_sinyalleri_besle(&self, sinyaller: HashMap<String, kota::KotaSnapshot>) {
        let mut durum = self.durum.write().await;
        durum.headroom.clear();
        for (anahtar, snap) in &sinyaller {
            let (util5h, util7d, util_gun) = snap.doluluk_orani();
            durum.headroom.insert(
                anahtar.clone(),
                HeadroomSinyali {
                    util5h,
                    util7d,
                    util_gun,
                },
            );
        }
        durum.kota = sinyaller;
    }

    /// Adaylar arasından stratejinin (OmniRoute çekirdeği, `RoutingStrategy::sirala`)
    /// ilk hedefini seçer.
    pub async fn select_provider(&self, candidates: &[String]) -> Option<String> {
        self.aday_sirasi(candidates).await.into_iter().next()
    }

    /// Bir istek başarıyla sonuçlandığında çağrılır.
    pub async fn record_success(&self, provider: &str, latency_ms: u64) {
        self.record_success_hesap(provider, None, latency_ms).await;
    }

    /// Hesap bilinen başarı kaydı. Devre kesici yine SAĞLAYICI düzeyinde tutulur;
    /// yalnız `hedef_istek` sayacı ve LKGP hesap kimliğini görür, böylece
    /// `least-used` / `lkgp` hesap seçebilir.
    pub async fn record_success_hesap(&self, provider: &str, hesap: Option<&str>, latency_ms: u64) {
        let mut metrics = self.metrics.write().await;
        let m = metrics.entry(provider.to_string()).or_default();
        m.total_requests += 1;
        m.successful_requests += 1;
        m.consecutive_errors = 0;
        m.last_latency_ms = latency_ms;
        m.circuit_open_until = None;

        let anahtar = hedef_anahtari(provider, hesap);
        let mut durum = self.durum.write().await;
        durum.kaydet(provider, &anahtar, true, latency_ms as f64);
        durum.lkgp = Some((provider.to_string(), hesap.map(str::to_string)));
    }

    /// Bir istek hata aldığında çağrılır (Circuit Breaker tetikleyicisi).
    pub async fn record_failure(&self, provider: &str) {
        self.record_failure_hesap(provider, None).await;
    }

    /// Hesap bilinen hata kaydı (devre kesici yine sağlayıcı düzeyinde).
    pub async fn record_failure_hesap(&self, provider: &str, hesap: Option<&str>) {
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
        let anahtar = hedef_anahtari(provider, hesap);
        let mut durum = self.durum.write().await;
        durum.kaydet(provider, &anahtar, false, 0.0);
        // LKGP bu sağlayıcı ise sıfırla ki bir sonraki istek başka yere yönlensin
        if durum.lkgp.as_ref().is_some_and(|(p, _)| p == provider) {
            durum.lkgp = None;
        }
    }
}

/// Sıralamanın "şimdi"si: epoch milisaniye. Sistem saati 1970 öncesine
/// ayarlıysa (ya da saat okunamazsa) 0 döner — sıralama yine çalışır, ancak
/// `reset-at` geçmişte kalır ve aciliyet 1 (en yüksek basınç) sayılır.
fn simdi_epoch_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Aday kimliği: hesap varsa `saglayici/hesap`, yoksa sağlayıcı adı. Kota ve
/// kullanım sayaçlarının anahtarı budur; `pool.rs` tercih eşleştirmesi de aynı
/// biçimi kullanır.
pub fn hedef_anahtari(saglayici: &str, hesap: Option<&str>) -> String {
    match hesap.filter(|h| !h.is_empty()) {
        Some(h) => format!("{saglayici}/{h}"),
        None => saglayici.to_string(),
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

    // ---- Kota duyarlı çekirdekler (reset-aware / reset-window / headroom / quota-weighted) ----

    fn hedef(ad: &str) -> Hedef {
        Hedef {
            model_str: ad.to_string(),
            provider: ad.to_string(),
            execution_key: ad.to_string(),
            connection_id: None,
            weight: 1.0,
        }
    }

    /// Sinyal beslemesi: `headroom` haritası anlık görüntüden TÜRETİLİR ve
    /// sıralama dolu hedefi sona atar. Sinyalsiz hedef tam boş sayılır.
    #[tokio::test]
    async fn kota_sinyalleri_headroom_siralamasini_besler() {
        let router = SmartRouter::new(RoutingStrategy::Headroom);
        let hedefler = vec![hedef("dolu"), hedef("bos"), hedef("sinyalsiz")];
        let mut m = HashMap::new();
        m.insert(
            "dolu".to_string(),
            kota::KotaSnapshot {
                oturum: Some(kota::KotaPenceresi {
                    percent_used: Some(0.95),
                    reset_at: None,
                }),
                haftalik: Some(kota::KotaPenceresi {
                    percent_used: Some(0.95),
                    reset_at: None,
                }),
                ..Default::default()
            },
        );
        m.insert(
            "bos".to_string(),
            kota::KotaSnapshot {
                oturum: Some(kota::KotaPenceresi {
                    percent_used: Some(0.1),
                    reset_at: None,
                }),
                ..Default::default()
            },
        );
        router.kota_sinyalleri_besle(m).await;
        let sira: Vec<String> = router
            .hedef_sirasi(&hedefler)
            .await
            .into_iter()
            .map(|h| h.execution_key)
            .collect();
        assert_eq!(sira.last().map(String::as_str), Some("dolu"));
        assert!(sira[..2].contains(&"bos".to_string()));
        // Sıralama "şimdi"yi kendisi yazar; 0'da kalmamalı.
        assert!(router.durum.read().await.simdi_ms > 0);
    }

    /// Sinyal haritası boşken `kota`/`headroom` temizlenir: eski sinyal, kapanmış
    /// bir hedefin sıralamayı sessizce yönlendirmesine izin vermez.
    #[tokio::test]
    async fn kota_sinyalleri_bosaltilabilir() {
        let router = SmartRouter::new(RoutingStrategy::Headroom);
        let mut m = HashMap::new();
        m.insert(
            "a".to_string(),
            kota::KotaSnapshot {
                limit_reached: true,
                ..Default::default()
            },
        );
        router.kota_sinyalleri_besle(m).await;
        assert_eq!(router.durum.read().await.kota.len(), 1);
        router.kota_sinyalleri_besle(HashMap::new()).await;
        let d = router.durum.read().await;
        assert!(d.kota.is_empty());
        assert!(d.headroom.is_empty());
    }

    /// Hesap düzeyi hedef: `execution_key` = `saglayici/hesap`, `connection_id` =
    /// hesap adı, `model_str` = sağlayıcı (fiyat/bağlam tabloları sağlayıcıya bağlı).
    fn hesap_hedefi(saglayici: &str, hesap: &str) -> Hedef {
        Hedef {
            model_str: saglayici.to_string(),
            provider: saglayici.to_string(),
            execution_key: format!("{saglayici}/{hesap}"),
            connection_id: Some(hesap.to_string()),
            weight: 1.0,
        }
    }

    fn sirala_adlar(
        strat: RoutingStrategy,
        hedefler: &[Hedef],
        durum: &mut StratejiDurumu,
        r: f64,
    ) -> Vec<String> {
        strat
            .sirala(hedefler, durum, &mut || r)
            .into_iter()
            .map(|h| h.execution_key)
            .collect()
    }

    /// OmniRoute `rotateLeadingTies`: eşitlik kümesi her çağrıda bir kayar; sayaç
    /// `combo_adi` altında tutulur.
    #[test]
    fn reset_aware_esitlik_kumesi_doner() {
        let hedefler = vec![hedef("a"), hedef("b"), hedef("c")];
        let mut durum = StratejiDurumu::default();
        let mut siralar = Vec::new();
        for _ in 0..3 {
            siralar.push(sirala_adlar(
                RoutingStrategy::ResetAware,
                &hedefler,
                &mut durum,
                0.0,
            ));
        }
        assert_eq!(
            siralar,
            vec![
                vec!["a".to_string(), "b".to_string(), "c".to_string()],
                vec!["b".to_string(), "c".to_string(), "a".to_string()],
                vec!["c".to_string(), "a".to_string(), "b".to_string()],
            ]
        );
    }

    /// Aynı döndürme `reset-window` için ayrı sayaç anahtarıyla çalışır ve sayacı
    /// `reset-aware` ile paylaşmaz.
    #[test]
    fn reset_window_esitlik_kumesi_ayri_sayacla_doner() {
        let hedefler = vec![hedef("a"), hedef("b")];
        let mut durum = StratejiDurumu::default();
        for h in &hedefler {
            durum.kota.insert(
                h.execution_key.clone(),
                kota::KotaSnapshot {
                    haftalik: Some(kota::KotaPenceresi {
                        percent_used: Some(0.5),
                        reset_at: Some(1_000),
                    }),
                    ..Default::default()
                },
            );
        }
        let ilk = sirala_adlar(RoutingStrategy::ResetWindow, &hedefler, &mut durum, 0.0);
        let ikinci = sirala_adlar(RoutingStrategy::ResetWindow, &hedefler, &mut durum, 0.0);
        assert_eq!(ilk, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(ikinci, vec!["b".to_string(), "a".to_string()]);
        assert!(
            durum
                .rr_kenar_sayaclari
                .iter()
                .any(|(k, _)| k == "reset-window:"),
            "reset-window kendi sayaç anahtarını yazmalı"
        );
        assert!(
            !durum
                .rr_kenar_sayaclari
                .iter()
                .any(|(k, _)| k == "reset-aware:"),
            "reset-aware sayacı reset-window çağrısıyla yazılmamalı"
        );
    }

    /// `MAX_RR_COUNTERS` dolunca en eski (baştaki) sayaç atılır; tavan aşılmaz.
    #[test]
    fn rr_sayaci_tavaninda_en_eski_atilir() {
        let hedefler = vec![hedef("a"), hedef("b")];
        let mut durum = StratejiDurumu::default();
        for i in 0..kota::AZAMI_RR_SAYACI {
            durum.rr_kenar_sayaclari.push((format!("dolgu{i}"), 0));
        }
        sirala_adlar(RoutingStrategy::ResetAware, &hedefler, &mut durum, 0.0);
        assert_eq!(durum.rr_kenar_sayaclari.len(), kota::AZAMI_RR_SAYACI);
        assert!(
            durum
                .rr_kenar_sayaclari
                .iter()
                .any(|(k, _)| k == "reset-aware:"),
            "yeni sayaç eklenmeli"
        );
        assert!(
            !durum.rr_kenar_sayaclari.iter().any(|(k, _)| k == "dolgu0"),
            "en eski sayaç atılmalı"
        );
    }

    /// Eşitlik yoksa (skorlar farklı) döndürme devreye girmez; sıra skor azalan.
    /// Sıfırlanma anı çok ileride verilir: aciliyet 0 olur ve yalnız kalan pay
    /// konuşur. (Sıfırlanma anı yokken aciliyet 0,5'tir ve dolu hesap basınç
    /// terimiyle öne geçebilir — OmniRoute formülünün kendisi böyledir.)
    #[test]
    fn reset_aware_esit_olmayan_skorlar_donmez() {
        let hedefler = vec![hedef("dolu"), hedef("bos")];
        let uzak = Some(1_000_000_000_000);
        let mut durum = StratejiDurumu::default();
        for (ad, dolu) in [("dolu", 0.9), ("bos", 0.1)] {
            let pencere = kota::KotaPenceresi {
                percent_used: Some(dolu),
                reset_at: uzak,
            };
            durum.kota.insert(
                ad.to_string(),
                kota::KotaSnapshot {
                    percent_used: Some(dolu),
                    oturum: Some(pencere),
                    haftalik: Some(pencere),
                    ..Default::default()
                },
            );
        }
        for _ in 0..2 {
            assert_eq!(
                sirala_adlar(RoutingStrategy::ResetAware, &hedefler, &mut durum, 0.0),
                vec!["bos".to_string(), "dolu".to_string()]
            );
        }
        assert!(
            durum.rr_kenar_sayaclari.is_empty(),
            "tek elemanlı eşitlik kümesi sayaç yazmamalı"
        );
    }

    /// SAPMA (belgelenmiş): seçilebilir hedef yoksa OmniRoute boş liste döner,
    /// Kervan girdi sırasını korur (fail-open).
    #[test]
    fn kota_agirlikli_secilebilir_yoksa_girdi_sirasi() {
        let hedefler = vec![hedef("a"), hedef("b")];
        let mut durum = StratejiDurumu::default();
        for h in &hedefler {
            durum.kota.insert(
                h.execution_key.clone(),
                kota::KotaSnapshot {
                    limit_reached: true,
                    ..Default::default()
                },
            );
        }
        assert_eq!(
            sirala_adlar(RoutingStrategy::QuotaWeighted, &hedefler, &mut durum, 0.5),
            vec!["a".to_string(), "b".to_string()]
        );
    }

    /// Bayat anlık görüntü A havuzuna girmez: taze ama dolu hesap, bayat ama boş
    /// hesabı yener (OmniRoute `QUOTA_WEIGHTED_MAX_SNAPSHOT_AGE_MS` gerekçesi).
    #[test]
    fn kota_agirlikli_bayat_hedef_b_havuzuna_duser() {
        let hedefler = vec![hedef("taze"), hedef("bayat")];
        let mut durum = StratejiDurumu::default();
        durum.kota.insert(
            "taze".to_string(),
            kota::KotaSnapshot {
                percent_used: Some(0.9),
                ..Default::default()
            },
        );
        durum.kota.insert(
            "bayat".to_string(),
            kota::KotaSnapshot {
                percent_used: Some(0.0),
                bayat: true,
                ..Default::default()
            },
        );
        assert_eq!(
            sirala_adlar(RoutingStrategy::QuotaWeighted, &hedefler, &mut durum, 0.0),
            vec!["taze".to_string(), "bayat".to_string()]
        );
    }

    /// `floor` A havuzunu belirler: taban üstünde oda yoksa hedef B'ye düşer.
    #[test]
    fn kota_agirlikli_floor_havuzu_belirler() {
        let hedefler = vec![hedef("dar"), hedef("genis"), hedef("bayat")];
        let kur = || {
            let mut durum = StratejiDurumu::default();
            durum.kota.insert(
                "dar".to_string(),
                kota::KotaSnapshot {
                    percent_used: Some(0.5),
                    ..Default::default()
                },
            );
            durum.kota.insert(
                "genis".to_string(),
                kota::KotaSnapshot {
                    percent_used: Some(0.2),
                    ..Default::default()
                },
            );
            durum.kota.insert(
                "bayat".to_string(),
                kota::KotaSnapshot {
                    percent_used: Some(0.0),
                    bayat: true,
                    ..Default::default()
                },
            );
            durum
        };
        // floor = 0: yalnız bayat B'de; A = {dar, genis}, r = 0.999 → kümülatif
        // ağırlığı r'yi ilk AŞAN kazanır (genis).
        let mut d0 = kur();
        d0.quota_weighted_floor = 0.0;
        assert_eq!(
            sirala_adlar(RoutingStrategy::QuotaWeighted, &hedefler, &mut d0, 0.999),
            vec!["genis".to_string(), "dar".to_string(), "bayat".to_string()]
        );
        // floor = 100: hiçbir hedefin "odası" yok → A boş, B = üçü; bayat da yarışır
        // ve ağırlıklı çekilişi kazanır (kümülatif ağırlık r'yi en sonda aşar).
        let mut d100 = kur();
        d100.quota_weighted_floor = 100.0;
        assert_eq!(
            sirala_adlar(RoutingStrategy::QuotaWeighted, &hedefler, &mut d100, 0.999),
            vec!["bayat".to_string(), "dar".to_string(), "genis".to_string()]
        );
    }

    /// `pickWeightedIndex` sıfır ağırlığı atlar (hiçbir zaman seçilmez).
    #[test]
    fn agirlikli_secim_sifir_agirligi_atlar() {
        assert_eq!(kota::agirlikli_secim(&[0.0, 3.0], 0.0), Some(1));
        assert_eq!(kota::agirlikli_secim(&[0.0, 0.0], 0.5), None);
    }

    /// `headroom`: sinyali olmayan hedef tam boş (1,0) sayılır — OmniRoute fail-open.
    #[test]
    fn headroom_sinyalsiz_hedef_tam_bos_sayilir() {
        let hedefler = vec![hedef("bilinmeyen"), hedef("dolu")];
        let mut durum = StratejiDurumu::default();
        durum.headroom.insert(
            "dolu".to_string(),
            HeadroomSinyali {
                util5h: 0.9,
                util7d: 0.8,
                util_gun: 0.0,
            },
        );
        assert_eq!(
            sirala_adlar(RoutingStrategy::Headroom, &hedefler, &mut durum, 0.0),
            vec!["bilinmeyen".to_string(), "dolu".to_string()]
        );
    }

    /// Hesap düzeyi aday: aynı sağlayıcının iki hesabı AYRI hedeftir ve
    /// `least-used` sayacı hesap başına işler. Sağlayıcı adı tek anahtar olsaydı
    /// iki hesap aynı sayacı paylaşır, biri hiç seçilmezdi.
    #[tokio::test]
    async fn hedef_sirasi_hesaplari_ayri_sayar() {
        let router = SmartRouter::new(RoutingStrategy::LeastUsed);
        let hedefler = vec![hesap_hedefi("gemini", "a"), hesap_hedefi("gemini", "b")];
        // 'b' iki istek almış: en-az-kullanılan 'a'yı öne almalı.
        router.record_success_hesap("gemini", Some("b"), 10).await;
        router.record_success_hesap("gemini", Some("b"), 10).await;

        let sirali = router.hedef_sirasi(&hedefler).await;
        let adlar: Vec<&str> = sirali.iter().map(|h| h.execution_key.as_str()).collect();
        assert_eq!(adlar, vec!["gemini/a", "gemini/b"]);
        assert!(
            sirali.iter().all(|h| h.provider == "gemini"),
            "sağlayıcı alanı devre kesici için korunmalı"
        );
        assert_eq!(sirali[0].connection_id.as_deref(), Some("a"));
    }

    /// LKGP hesabı hatırlar: son başarılı hesap, aynı sağlayıcının diğer
    /// hesabından önce gelir.
    #[tokio::test]
    async fn hedef_sirasi_lkgp_hesabi_secer() {
        let router = SmartRouter::new(RoutingStrategy::Lkgp);
        let hedefler = vec![hesap_hedefi("gemini", "a"), hesap_hedefi("gemini", "b")];
        router.record_success_hesap("gemini", Some("b"), 10).await;

        let sirali = router.hedef_sirasi(&hedefler).await;
        assert_eq!(sirali[0].execution_key, "gemini/b");
    }

    /// Devre kesici SAĞLAYICI düzeyinde kalır: sağlayıcı açılınca bütün
    /// hesapları birlikte elenir, başka sağlayıcının hesabı etkilenmez.
    #[tokio::test]
    async fn hedef_sirasi_devre_kesici_saglayici_duzeyinde() {
        let router = SmartRouter::new(RoutingStrategy::Priority);
        let hedefler = vec![
            hesap_hedefi("gemini", "a"),
            hesap_hedefi("gemini", "b"),
            hesap_hedefi("kimi", "a"),
        ];
        for _ in 0..3 {
            router.record_failure_hesap("gemini", Some("a")).await;
        }

        let sirali = router.hedef_sirasi(&hedefler).await;
        let adlar: Vec<&str> = sirali.iter().map(|h| h.execution_key.as_str()).collect();
        assert_eq!(adlar, vec!["kimi/a"]);
    }
}
