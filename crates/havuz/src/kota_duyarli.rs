//! Kota duyarlı stratejilerin saf (pure) çekirdeği: `headroom`, `reset-aware`,
//! `reset-window`, `quota-weighted`.
//!
//! OmniRoute `open-sse/services/combo/quotaScoring.ts`, `headroomRanking.ts` ve
//! `quotaStrategies.ts` (dbe703a) formüllerinin Rust karşılığı. Bu modül hiçbir G/Ç
//! yapmaz: kota anlık görüntüsü çağıran tarafından `KotaSnapshot` olarak verilir.
//! Böylece sıralama determinist ve test edilebilir kalır.
//!
//! Bilinçli sapmalar (kod içinde ayrıca işaretli):
//! - `percent_used` sonlu değilse yok sayılır (OmniRoute yalnız üst düzey alanda
//!   sonluluk denetler; pencere alanında NaN sızar).
//! - `reset_at` zaten epoch ms olarak gelir; ISO metin ayrıştırma çağıranın işidir.

/// OmniRoute `RESET_AWARE_SESSION_WINDOW_MS`.
pub const OTURUM_PENCERE_MS: f64 = 5.0 * 60.0 * 60.0 * 1000.0;
/// OmniRoute `RESET_AWARE_WEEKLY_WINDOW_MS`.
pub const HAFTALIK_PENCERE_MS: f64 = 7.0 * 24.0 * 60.0 * 60.0 * 1000.0;
/// OmniRoute `RESET_AWARE_SESSION_REMAINING_WEIGHT`.
pub const OTURUM_KALAN_AGIRLIK: f64 = 0.45;
/// OmniRoute `RESET_AWARE_SESSION_RESET_PRESSURE_WEIGHT`.
pub const OTURUM_BASKI_AGIRLIK: f64 = 0.55;
/// OmniRoute `RESET_AWARE_WEEKLY_REMAINING_WEIGHT`.
pub const HAFTALIK_KALAN_AGIRLIK: f64 = 0.25;
/// OmniRoute `RESET_AWARE_WEEKLY_RESET_PRESSURE_WEIGHT`.
pub const HAFTALIK_BASKI_AGIRLIK: f64 = 0.75;
/// OmniRoute `RESET_WINDOW_DEFAULT_TIE_BAND_MS`.
pub const RESET_WINDOW_VARSAYILAN_BANT_MS: f64 = 60_000.0;
/// OmniRoute `MAX_RR_COUNTERS` (tie döndürme sayaç tavanı).
pub const AZAMI_RR_SAYACI: usize = 500;

/// `clamp01`.
pub fn kirp01(x: f64) -> f64 {
    if !x.is_finite() {
        // JS Math.min(1, Math.max(0, NaN)) NaN döner; motorun kendi ürettiği
        // değerler sonludur, dışarıdan gelen NaN 0 sayılır (belgelenmiş sapma).
        return 0.0;
    }
    x.clamp(0.0, 1.0)
}

fn kirp_yuzde(x: f64) -> f64 {
    if !x.is_finite() {
        return 0.0;
    }
    x.clamp(0.0, 100.0)
}

/// Kota penceresi adları — OmniRoute `RESET_WINDOW_NAMES` ile aynı küme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Pencere {
    Weekly,
    Session,
    Monthly,
}

impl Pencere {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Weekly => "weekly",
            Self::Session => "session",
            Self::Monthly => "monthly",
        }
    }
}

/// Tek kota penceresi (`QuotaWindowSnapshot`'ın skorlamada kullanılan alanları).
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct KotaPenceresi {
    /// 0..1 doluluk; yoksa üst düzey `percentUsed`, o da yoksa 0.5 varsayılır.
    #[serde(default)]
    pub percent_used: Option<f64>,
    /// Sıfırlanma anı (epoch ms).
    #[serde(default)]
    pub reset_at: Option<i64>,
}

/// Hesap (bağlantı) düzeyinde kota anlık görüntüsü.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct KotaSnapshot {
    /// Sağlayıcı "kota bitti" dediyse skor `-∞`, kalan yüzde 0.
    #[serde(default)]
    pub limit_reached: bool,
    /// Üst düzey doluluk; pencereler yoksa yedek.
    #[serde(default)]
    pub percent_used: Option<f64>,
    #[serde(default)]
    pub oturum: Option<KotaPenceresi>,
    #[serde(default)]
    pub haftalik: Option<KotaPenceresi>,
    #[serde(default)]
    pub aylik: Option<KotaPenceresi>,
    /// Tek sinyalli üst düzey `quota.resetAt` (epoch ms).
    #[serde(default)]
    pub reset_at: Option<i64>,
    /// `windows` haritasındaki en erken sıfırlanma (epoch ms) — adlandırma
    /// sağlayıcıya bağlı olduğunda kullanılır (`#9330`).
    #[serde(default)]
    pub en_erken_pencere_reset_ms: Option<i64>,
    /// Anlık görüntü `quota-weighted` bayatlık sınırını aştı (`fetchedAt`).
    #[serde(default)]
    pub bayat: bool,
}

impl KotaSnapshot {
    /// Pencerelerin doluluk oranı: (util5h, util7d) — `headroom` sinyalinin kaynağı.
    ///
    /// `limit_reached` ise ikisi de 1,0 (tam dolu). Eksik pencere üst düzey
    /// `percent_used`e, o da yoksa 0,5 doluluğa düşer (OmniRoute kuralı).
    pub fn doluluk_orani(&self) -> (f64, f64) {
        if self.limit_reached {
            return (1.0, 1.0);
        }
        let (_, oturum_kalan, haftalik_kalan) = self.doluluklar();
        (1.0 - oturum_kalan, 1.0 - haftalik_kalan)
    }

    fn pencere(&self, p: Pencere) -> Option<&KotaPenceresi> {
        match p {
            Pencere::Session => self.oturum.as_ref(),
            Pencere::Weekly => self.haftalik.as_ref(),
            Pencere::Monthly => self.aylik.as_ref(),
        }
    }

    /// OmniRoute `resolveWindowRemaining`: eksik pencere üst düzey `percentUsed`e,
    /// o da yoksa 0.5 doluluğa düşer.
    fn doluluklar(&self) -> (f64, f64, f64) {
        let genel = match self.percent_used {
            Some(v) if v.is_finite() => kirp01(v),
            _ => 0.5,
        };
        let kalan = |p: Pencere| {
            let dolu = self
                .pencere(p)
                .and_then(|w| w.percent_used)
                .filter(|v| v.is_finite())
                .unwrap_or(genel);
            kirp01(1.0 - dolu)
        };
        (genel, kalan(Pencere::Session), kalan(Pencere::Weekly))
    }
}

/// OmniRoute `RESET_AWARE_DEFAULTS` (normalize edilmiş hâliyle).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResetAwareAyar {
    pub oturum_agirlik: f64,
    pub haftalik_agirlik: f64,
    pub tie_band: f64,
    pub tukanme_koruma: f64,
}

impl Default for ResetAwareAyar {
    fn default() -> Self {
        Self::agirliklardan(0.35, 0.65, 5.0, 10.0)
    }
}

impl ResetAwareAyar {
    /// OmniRoute `resolveResetAwareConfig`: ağırlıklar normalize edilir, yüzdeler
    /// [0, 100] aralığına kırpılır ve 100'e bölünür.
    pub fn agirliklardan(
        oturum: f64,
        haftalik: f64,
        tie_band_yuzde: f64,
        tukanme_yuzde: f64,
    ) -> Self {
        let (o, h) =
            if oturum.is_finite() && haftalik.is_finite() && oturum >= 0.0 && haftalik >= 0.0 {
                (oturum, haftalik)
            } else {
                (0.35, 0.65)
            };
        let toplam = o + h;
        let oturum_agirlik = if toplam > 0.0 { o / toplam } else { 0.35 };
        Self {
            oturum_agirlik,
            haftalik_agirlik: 1.0 - oturum_agirlik,
            tie_band: kirp_yuzde(tie_band_yuzde) / 100.0,
            tukanme_koruma: kirp_yuzde(tukanme_yuzde) / 100.0,
        }
    }
}

/// OmniRoute `resolveResetWindowConfig`.
#[derive(Debug, Clone, PartialEq)]
pub struct ResetWindowAyar {
    pub pencereler: Vec<Pencere>,
    pub tie_band_ms: f64,
}

impl Default for ResetWindowAyar {
    fn default() -> Self {
        Self {
            pencereler: vec![Pencere::Weekly],
            tie_band_ms: RESET_WINDOW_VARSAYILAN_BANT_MS,
        }
    }
}

/// OmniRoute `getResetUrgency`: reset anı yoksa 0.5, geçmişteyse 1, aksi
/// `clamp01(1 - kalanMs / pencereMs)`.
pub fn sifirlanma_aciliyeti(reset_at: Option<i64>, pencere_ms: f64, simdi_ms: i64) -> f64 {
    let Some(reset) = reset_at else {
        return 0.5;
    };
    let kalan = reset as f64 - simdi_ms as f64;
    if kalan <= 0.0 {
        return 1.0;
    }
    kirp01(1.0 - kalan / pencere_ms)
}

/// OmniRoute `scoreQuotaWindow`.
pub fn skor_pencere(
    kalan: f64,
    reset_at: Option<i64>,
    pencere_ms: f64,
    kalan_agirlik: f64,
    baski_agirlik: f64,
    simdi_ms: i64,
) -> f64 {
    let k = kirp01(kalan);
    let baski = sifirlanma_aciliyeti(reset_at, pencere_ms, simdi_ms) * (1.0 - k);
    kalan_agirlik * k + baski_agirlik * baski
}

/// OmniRoute `scoreResetAwareQuota`. Anlık görüntü yoksa 0.5, `limitReached` ise `-∞`.
pub fn reset_aware_skor(snap: Option<&KotaSnapshot>, ayar: &ResetAwareAyar, simdi_ms: i64) -> f64 {
    let Some(q) = snap else {
        return 0.5;
    };
    if q.limit_reached {
        return f64::NEG_INFINITY;
    }
    let (_, oturum_kalan, haftalik_kalan) = q.doluluklar();
    let oturum_skor = skor_pencere(
        oturum_kalan,
        q.oturum.and_then(|w| w.reset_at),
        OTURUM_PENCERE_MS,
        OTURUM_KALAN_AGIRLIK,
        OTURUM_BASKI_AGIRLIK,
        simdi_ms,
    );
    let haftalik_skor = skor_pencere(
        haftalik_kalan,
        q.haftalik.and_then(|w| w.reset_at).or(q.reset_at),
        HAFTALIK_PENCERE_MS,
        HAFTALIK_KALAN_AGIRLIK,
        HAFTALIK_BASKI_AGIRLIK,
        simdi_ms,
    );
    let mut skor = ayar.oturum_agirlik * oturum_skor + ayar.haftalik_agirlik * haftalik_skor;
    if ayar.tukanme_koruma > 0.0 && oturum_kalan < ayar.tukanme_koruma {
        skor *= (oturum_kalan / ayar.tukanme_koruma).max(0.05);
    }
    skor
}

/// OmniRoute `getResetAwareRemainingPercent`: iki pencerenin en darının yüzdesi,
/// 6 ondalığa yuvarlanmış (`Number(x.toFixed(6))`).
pub fn kalan_yuzde(snap: Option<&KotaSnapshot>) -> f64 {
    let Some(q) = snap else {
        return 100.0;
    };
    if q.limit_reached {
        return 0.0;
    }
    let (_, oturum, haftalik) = q.doluluklar();
    let dar = oturum.min(haftalik) * 100.0;
    (dar * 1e6).round() / 1e6
}

/// OmniRoute `getResetWindowTimestampMs`: yapılandırılmış pencereler → `windows`
/// haritasının en erkeni → üst düzey `resetAt`. Bulunamazsa `None`.
pub fn reset_window_an_ms(snap: Option<&KotaSnapshot>, pencereler: &[Pencere]) -> Option<i64> {
    let q = snap?;
    // OmniRoute: limitReached ise Infinity.
    if q.limit_reached {
        return None;
    }
    let mut secilen: Option<i64> = None;
    for p in pencereler {
        if let Some(ms) = q.pencere(*p).and_then(|w| w.reset_at) {
            secilen = Some(secilen.map_or(ms, |s: i64| s.min(ms)));
        }
    }
    secilen.or(q.en_erken_pencere_reset_ms).or(q.reset_at)
}

/// OmniRoute `getResetWindowRemainingMs`: sıfırlanmaya kalan ms; bilinmiyorsa `+∞`.
pub fn reset_window_kalan_ms(
    snap: Option<&KotaSnapshot>,
    pencereler: &[Pencere],
    simdi_ms: i64,
) -> f64 {
    match reset_window_an_ms(snap, pencereler) {
        None => f64::INFINITY,
        Some(ms) => (ms as f64 - simdi_ms as f64).max(0.0),
    }
}

/// OmniRoute `computeHeadroom`: `1 - max(util5h, util7d)`, sonlu olmayan 0 sayılır
/// (tam boşluk — fail-open).
pub fn headroom(util5h: f64, util7d: f64) -> f64 {
    let u5 = if util5h.is_finite() {
        kirp01(util5h)
    } else {
        0.0
    };
    let u7 = if util7d.is_finite() {
        kirp01(util7d)
    } else {
        0.0
    };
    1.0 - u5.max(u7)
}

/// OmniRoute `pickWeightedIndex`: yalnız pozitif ağırlıklar; ilk kümülatif ağırlık
/// `r`'yi kesin aşan kazanır (yarım-açık aralık). Pozitif ağırlık yoksa `None`.
pub fn agirlikli_secim(agirliklar: &[f64], r: f64) -> Option<usize> {
    let mut toplam = 0.0;
    let mut var = false;
    for w in agirliklar {
        if *w > 0.0 {
            toplam += *w;
            var = true;
        }
    }
    if !var || toplam == 0.0 {
        return None;
    }
    let mut acc = 0.0;
    let mut son = None;
    for (i, w) in agirliklar.iter().enumerate() {
        if *w <= 0.0 {
            continue;
        }
        acc += *w;
        son = Some(i);
        if acc > r {
            return Some(i);
        }
    }
    son
}

/// OmniRoute `resolveQuotaWeightedFloor`: geçersiz/yok değer 1'e düşer, sonuç
/// [0, 100] aralığında.
pub fn quota_weighted_floor(deger: Option<f64>) -> f64 {
    match deger {
        Some(v) if v.is_finite() => v.clamp(0.0, 100.0),
        _ => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kirp01_ve_yuzde() {
        assert_eq!(kirp01(-1.0), 0.0);
        assert_eq!(kirp01(2.0), 1.0);
        assert_eq!(kirp01(f64::NAN), 0.0);
        assert_eq!(quota_weighted_floor(None), 1.0);
        assert_eq!(quota_weighted_floor(Some(500.0)), 100.0);
        assert_eq!(quota_weighted_floor(Some(-5.0)), 0.0);
    }

    #[test]
    fn test_sifirlanma_aciliyeti_sinirlari() {
        assert_eq!(sifirlanma_aciliyeti(None, OTURUM_PENCERE_MS, 0), 0.5);
        assert_eq!(sifirlanma_aciliyeti(Some(-1), OTURUM_PENCERE_MS, 0), 1.0);
        assert_eq!(sifirlanma_aciliyeti(Some(0), OTURUM_PENCERE_MS, 0), 1.0);
        // Tam pencere kadar uzak: aciliyet 0.
        let uzak = OTURUM_PENCERE_MS as i64 + 1;
        assert_eq!(sifirlanma_aciliyeti(Some(uzak), OTURUM_PENCERE_MS, 0), 0.0);
        // Yarım pencere kadar uzak: 0.5.
        let yarim = (OTURUM_PENCERE_MS / 2.0) as i64;
        assert!((sifirlanma_aciliyeti(Some(yarim), OTURUM_PENCERE_MS, 0) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_skor_pencere_formulu() {
        // kalan 1, aciliyet 0.5 → 0.45*1 + 0.55*0.5*0 = 0.45
        let s = skor_pencere(1.0, None, OTURUM_PENCERE_MS, 0.45, 0.55, 0);
        assert!((s - 0.45).abs() < 1e-9);
        // kalan 0, aciliyet 1 → 0 + 0.55
        let s = skor_pencere(0.0, Some(-1), OTURUM_PENCERE_MS, 0.45, 0.55, 0);
        assert!((s - 0.55).abs() < 1e-9);
    }

    #[test]
    fn test_reset_aware_skor_yok_ve_limit() {
        let ayar = ResetAwareAyar::default();
        assert_eq!(reset_aware_skor(None, &ayar, 0), 0.5);
        let q = KotaSnapshot {
            limit_reached: true,
            ..Default::default()
        };
        assert_eq!(reset_aware_skor(Some(&q), &ayar, 0), f64::NEG_INFINITY);
    }

    #[test]
    fn test_tukanme_korumasi_skoru_dusurur() {
        let ayar = ResetAwareAyar::default();
        let q = KotaSnapshot {
            percent_used: Some(0.95),
            ..Default::default()
        };
        let skor = reset_aware_skor(Some(&q), &ayar, 0);
        let korumasiz = ResetAwareAyar {
            tukanme_koruma: 0.0,
            ..ayar
        };
        let ham = reset_aware_skor(Some(&q), &korumasiz, 0);
        assert!(skor < ham, "koruma skoru dusurmeli: {skor} < {ham}");
        // taban 0.05: oturum kalanı 0.05 / 0.10 = 0.5 çarpanı
        assert!((skor / ham - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_kalan_yuzde_dar_pencere() {
        assert_eq!(kalan_yuzde(None), 100.0);
        let q = KotaSnapshot {
            oturum: Some(KotaPenceresi {
                percent_used: Some(0.25),
                reset_at: None,
            }),
            haftalik: Some(KotaPenceresi {
                percent_used: Some(0.5),
                reset_at: None,
            }),
            ..Default::default()
        };
        assert_eq!(kalan_yuzde(Some(&q)), 50.0);
        // Eksik pencere üst düzey percentUsed'e düşer.
        let q = KotaSnapshot {
            percent_used: Some(0.8),
            haftalik: Some(KotaPenceresi {
                percent_used: Some(0.1),
                reset_at: None,
            }),
            ..Default::default()
        };
        assert_eq!(kalan_yuzde(Some(&q)), 20.0);
    }

    #[test]
    fn test_reset_window_kalan_ms_cozumleme_sirasi() {
        let a = ResetWindowAyar::default();
        assert_eq!(reset_window_kalan_ms(None, &a.pencereler, 0), f64::INFINITY);
        let q = KotaSnapshot {
            en_erken_pencere_reset_ms: Some(5_000),
            reset_at: Some(9_000),
            ..Default::default()
        };
        assert_eq!(
            reset_window_kalan_ms(Some(&q), &a.pencereler, 1_000),
            4_000.0
        );
        // Yapılandırılmış pencere varsa haritadan önce gelir.
        let q = KotaSnapshot {
            haftalik: Some(KotaPenceresi {
                percent_used: None,
                reset_at: Some(3_000),
            }),
            en_erken_pencere_reset_ms: Some(5_000),
            ..Default::default()
        };
        assert_eq!(
            reset_window_kalan_ms(Some(&q), &a.pencereler, 1_000),
            2_000.0
        );
        // Geçmiş sıfırlanma 0'a kırpılır.
        assert_eq!(reset_window_kalan_ms(Some(&q), &a.pencereler, 10_000), 0.0);
        // limitReached → sonsuz.
        let q = KotaSnapshot {
            limit_reached: true,
            haftalik: Some(KotaPenceresi {
                percent_used: None,
                reset_at: Some(3_000),
            }),
            ..Default::default()
        };
        assert_eq!(
            reset_window_kalan_ms(Some(&q), &a.pencereler, 0),
            f64::INFINITY
        );
    }

    #[test]
    fn test_doluluk_orani() {
        // Pencereler varsa üst düzey yüzde yok sayılır.
        let q = KotaSnapshot {
            percent_used: Some(0.9),
            oturum: Some(KotaPenceresi {
                percent_used: Some(0.2),
                reset_at: None,
            }),
            haftalik: Some(KotaPenceresi {
                percent_used: Some(0.6),
                reset_at: None,
            }),
            ..Default::default()
        };
        let (o, h) = q.doluluk_orani();
        assert!((o - 0.2).abs() < 1e-9);
        assert!((h - 0.6).abs() < 1e-9);

        // Eksik pencere üst düzeye düşer; o da yoksa 0,5.
        let q = KotaSnapshot {
            percent_used: Some(0.25),
            ..Default::default()
        };
        let (o, h) = q.doluluk_orani();
        assert!((o - 0.25).abs() < 1e-9);
        assert!((h - 0.25).abs() < 1e-9);
        let (o, h) = KotaSnapshot::default().doluluk_orani();
        assert!((o - 0.5).abs() < 1e-9);
        assert!((h - 0.5).abs() < 1e-9);

        // limitReached her iki pencereyi dolu sayar.
        let q = KotaSnapshot {
            limit_reached: true,
            oturum: Some(KotaPenceresi {
                percent_used: Some(0.0),
                reset_at: None,
            }),
            ..Default::default()
        };
        assert_eq!(q.doluluk_orani(), (1.0, 1.0));

        // Sonlu olmayan değerler yok sayılır (NaN sızmaz).
        let q = KotaSnapshot {
            percent_used: Some(f64::NAN),
            oturum: Some(KotaPenceresi {
                percent_used: Some(f64::INFINITY),
                reset_at: None,
            }),
            ..Default::default()
        };
        assert_eq!(q.doluluk_orani(), (0.5, 0.5));
    }

    #[test]
    fn test_headroom_hesabi() {
        assert_eq!(headroom(0.2, 0.6), 0.4);
        assert_eq!(headroom(f64::NAN, 0.0), 1.0);
        assert_eq!(headroom(2.0, 2.0), 0.0);
    }

    #[test]
    fn test_agirlikli_secim_yarim_acik_aralik() {
        // r=0 sıfır ağırlıklı başa düşmemeli.
        assert_eq!(agirlikli_secim(&[0.0, 1.0], 0.0), Some(1));
        // Kümülatif 0.5 r'yi kesin aşmalı: r=0.5 ikinci öğe.
        assert_eq!(agirlikli_secim(&[0.5, 0.5], 0.5), Some(1));
        assert_eq!(agirlikli_secim(&[0.5, 0.5], 0.4999), Some(0));
        // Son öğeye kalan.
        assert_eq!(agirlikli_secim(&[0.5, 0.5], 0.99), Some(1));
        // Pozitif yok.
        assert_eq!(agirlikli_secim(&[0.0, -1.0], 0.0), None);
        assert_eq!(agirlikli_secim(&[], 0.0), None);
    }

    #[test]
    fn test_ayar_normalizasyonu() {
        let a = ResetAwareAyar::agirliklardan(1.0, 3.0, 5.0, 10.0);
        assert!((a.oturum_agirlik - 0.25).abs() < 1e-9);
        assert!((a.haftalik_agirlik - 0.75).abs() < 1e-9);
        assert!((a.tie_band - 0.05).abs() < 1e-9);
        // Toplam 0 → varsayılan oturum ağırlığı.
        let a = ResetAwareAyar::agirliklardan(0.0, 0.0, 5.0, 10.0);
        assert!((a.oturum_agirlik - 0.35).abs() < 1e-9);
    }
}
