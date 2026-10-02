use rand::Rng;
use serde::{Deserialize, Serialize};

/// OmniRoute 19 Kanonik Kombo Yönlendirme Stratejisi
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StratejiTuru {
    Priority,
    RoundRobin,
    Weighted,
    Random,
    StrictRandom,
    LeastUsed,
    Lkgp,
    P2c,
    Auto,
    CostOptimized,
    ContextOptimized,
    ContextRelay,
    CacheOptimized,
    QuotaShare,
    QuotaWeighted,
    FillFirst,
    ResetAware,
    ResetWindow,
    Headroom,
}

impl StratejiTuru {
    pub fn metinden(ad: &str) -> Option<Self> {
        match ad {
            "priority" => Some(Self::Priority),
            "round-robin" => Some(Self::RoundRobin),
            "weighted" => Some(Self::Weighted),
            "random" => Some(Self::Random),
            "strict-random" => Some(Self::StrictRandom),
            "least-used" => Some(Self::LeastUsed),
            "lkgp" => Some(Self::Lkgp),
            "p2c" => Some(Self::P2c),
            "auto" => Some(Self::Auto),
            "cost-optimized" => Some(Self::CostOptimized),
            "context-optimized" => Some(Self::ContextOptimized),
            "context-relay" => Some(Self::ContextRelay),
            "cache-optimized" => Some(Self::CacheOptimized),
            "quota-share" => Some(Self::QuotaShare),
            "quota-weighted" => Some(Self::QuotaWeighted),
            "fill-first" => Some(Self::FillFirst),
            "reset-aware" => Some(Self::ResetAware),
            "reset-window" => Some(Self::ResetWindow),
            "headroom" => Some(Self::Headroom),
            _ => None,
        }
    }
}

/// Yönlendirme hedefi tanımı ve çalışma zamanı metrikleri
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hedef {
    pub id: String,
    pub saglayici: String,
    pub model: String,
    pub agirlik: u32,
    pub maliyet_milyon_basina: f64,
    pub baglam_boyutu: usize,
    pub kullanim_sayisi: u64,
    pub kalan_kota: u64,
    pub toplam_kota: u64,
    pub sifirlanma_suresi_sn: u64,
    pub aktif_istekler: u32,
    pub onbellek_anahtari: Option<String>,
    pub son_basarili_mi: bool,
    pub hata_orani: f64,
    pub gecikme_ms: u64,
}

impl Default for Hedef {
    fn default() -> Self {
        Self {
            id: String::new(),
            saglayici: String::new(),
            model: String::new(),
            agirlik: 1,
            maliyet_milyon_basina: 1.0,
            baglam_boyutu: 8192,
            kullanim_sayisi: 0,
            kalan_kota: 1000,
            toplam_kota: 1000,
            sifirlanma_suresi_sn: 60,
            aktif_istekler: 0,
            onbellek_anahtari: None,
            son_basarili_mi: true,
            hata_orani: 0.0,
            gecikme_ms: 50,
        }
    }
}

/// Yönlendirme kararı bağlamı
#[derive(Debug, Clone, Default)]
pub struct YonlendirmeBaglami {
    pub oturum_anahtari: Option<String>,
    pub son_bilinen_iyi_id: Option<String>,
    pub talep_edilen_baglam_uzunlugu: usize,
    pub onbellek_aranan_anahtar: Option<String>,
    pub rr_sayaci: usize,
}

/// 19 stratejiye göre hedefleri sıralar
pub fn hedefleri_sirala<R: Rng>(
    strateji: StratejiTuru,
    hedefler: &[Hedef],
    baglam: &YonlendirmeBaglami,
    mut rng: R,
) -> Vec<Hedef> {
    if hedefler.is_empty() {
        return Vec::new();
    }

    let mut sonuclar = hedefler.to_vec();

    match strateji {
        StratejiTuru::Priority => {
            // Tanımlandığı sıralama korunur (öncelik sırası)
        }
        StratejiTuru::RoundRobin => {
            let n = sonuclar.len();
            let shift = baglam.rr_sayaci % n;
            sonuclar.rotate_left(shift);
        }
        StratejiTuru::Weighted => {
            // Ağırlığa göre sıralama (büyük ağırlık öne)
            sonuclar.sort_by_key(|a| std::cmp::Reverse(a.agirlik));
        }
        StratejiTuru::Random => {
            for i in (1..sonuclar.len()).rev() {
                let j = rng.random_range(0..=i);
                sonuclar.swap(i, j);
            }
        }
        StratejiTuru::StrictRandom => {
            for i in (1..sonuclar.len()).rev() {
                let j = rng.random_range(0..=i);
                sonuclar.swap(i, j);
            }
        }
        StratejiTuru::LeastUsed => {
            // En az kullanılan (kullanim_sayisi + aktif_istekler) öne gelir
            sonuclar.sort_by_key(|h| (h.kullanim_sayisi, h.aktif_istekler));
        }
        StratejiTuru::Lkgp => {
            // Last Known Good Provider: Son başarılı ID eşleşeni en başa taşı
            if let Some(ref iyi_id) = baglam.son_bilinen_iyi_id
                && let Some(idx) = sonuclar.iter().position(|h| &h.id == iyi_id)
            {
                let iyi_hedef = sonuclar.remove(idx);
                sonuclar.insert(0, iyi_hedef);
            }
        }
        StratejiTuru::P2c => {
            // Power of Two Choices: Rastgele 2 aday seç, aktif yükü az olanı başa koy
            if sonuclar.len() >= 2 {
                let i1 = rng.random_range(0..sonuclar.len());
                let mut i2 = rng.random_range(0..sonuclar.len());
                while i2 == i1 && sonuclar.len() > 1 {
                    i2 = rng.random_range(0..sonuclar.len());
                }
                let yuk1 = sonuclar[i1].aktif_istekler as u64 + sonuclar[i1].gecikme_ms;
                let yuk2 = sonuclar[i2].aktif_istekler as u64 + sonuclar[i2].gecikme_ms;
                let kazanan = if yuk1 <= yuk2 { i1 } else { i2 };
                let hedef = sonuclar.remove(kazanan);
                sonuclar.insert(0, hedef);
            }
        }
        StratejiTuru::Auto => {
            // Hata oranı ve gecikme skoru en düşük olan öne gelir
            sonuclar.sort_by(|a, b| {
                let skor_a = (a.hata_orani * 1000.0) + (a.gecikme_ms as f64);
                let skor_b = (b.hata_orani * 1000.0) + (b.gecikme_ms as f64);
                skor_a
                    .partial_cmp(&skor_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        StratejiTuru::CostOptimized => {
            // En düşük maliyetli öne gelir
            sonuclar.sort_by(|a, b| {
                a.maliyet_milyon_basina
                    .partial_cmp(&b.maliyet_milyon_basina)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        StratejiTuru::ContextOptimized => {
            // İstem bağlamına en yakın ve yeterli olan öne gelir
            let aranan = baglam.talep_edilen_baglam_uzunlugu;
            sonuclar.sort_by_key(|h| {
                if h.baglam_boyutu >= aranan {
                    h.baglam_boyutu - aranan
                } else {
                    usize::MAX / 2 + (aranan - h.baglam_boyutu)
                }
            });
        }
        StratejiTuru::ContextRelay => {
            // Bağlam boyutu en geniş olan en öne geçer (uzun konuşmalar için aktarım)
            sonuclar.sort_by_key(|a| std::cmp::Reverse(a.baglam_boyutu));
        }
        StratejiTuru::CacheOptimized => {
            // Önceden bu anahtarla eşleşen önbellek afinitesi olan öne gelir
            if let Some(ref aranan_anahtar) = baglam.onbellek_aranan_anahtar {
                sonuclar.sort_by_key(|h| {
                    if h.onbellek_anahtari.as_deref() == Some(aranan_anahtar.as_str()) {
                        0
                    } else {
                        1
                    }
                });
            }
        }
        StratejiTuru::QuotaShare => {
            // Kalan kota oranı (kalan / toplam) en yüksek olan öne gelir
            sonuclar.sort_by(|a, b| {
                let oran_a = if a.toplam_kota > 0 {
                    a.kalan_kota as f64 / a.toplam_kota as f64
                } else {
                    0.0
                };
                let oran_b = if b.toplam_kota > 0 {
                    b.kalan_kota as f64 / b.toplam_kota as f64
                } else {
                    0.0
                };
                oran_b
                    .partial_cmp(&oran_a)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        StratejiTuru::QuotaWeighted => {
            // Kalan kotası mutlak olarak en fazla olan öne gelir
            sonuclar.sort_by_key(|a| std::cmp::Reverse(a.kalan_kota));
        }
        StratejiTuru::FillFirst => {
            // Birincil kotası bitene kadar birinciyi kullan, sırayla tüket
            sonuclar.sort_by_key(|h| if h.kalan_kota > 0 { 0 } else { 1 });
        }
        StratejiTuru::ResetAware => {
            // Kota sıfırlanma süresi en yakın olan (en çabuk tazelenecek olan) öne gelir
            sonuclar.sort_by_key(|h| h.sifirlanma_suresi_sn);
        }
        StratejiTuru::ResetWindow => {
            // Sıfırlanma penceresi durumuna göre sırala
            sonuclar.sort_by_key(|a| a.sifirlanma_suresi_sn);
        }
        StratejiTuru::Headroom => {
            // En fazla boş kapasitesi (headroom = toplam - aktif) olan öne gelir
            sonuclar.sort_by_key(|a| {
                std::cmp::Reverse(a.toplam_kota.saturating_sub(a.aktif_istekler as u64))
            });
        }
    }

    sonuclar
}
