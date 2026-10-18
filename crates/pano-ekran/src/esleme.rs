//! # esleme
//! OmniRoute 57 pano bölümünün Kervan sade panosuna eşleme denetleyicisi.
//! PRD: KERVAN-PANO-ANTIGRAVITY-ISTEMI.md FR-P1.

use serde::{Deserialize, Serialize};

/// Bir bölümün Kervan'daki kaynak ve hazır olma durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KaynakDurumu {
    /// Bugün Kervan'da gerçek kaynağı var, veri gösterilir.
    #[serde(rename = "V")]
    Var,
    /// DALGA-2 ile kalıcı veri katmanı bağlandığında gelecek.
    #[serde(rename = "D2")]
    Dalga2,
    /// DALGA-3 ile pano grafik katmanı bağlandığında gelecek.
    #[serde(rename = "D3")]
    Dalga3,
    /// DALGA-4 ile gelişmiş eklentilerle gelecek.
    #[serde(rename = "D4")]
    Dalga4,
    /// Kapsam dışı (Kervan'da yer almaz).
    #[serde(rename = "K")]
    KapsamDisi,
}

impl KaynakDurumu {
    pub fn rozet_metni(self) -> &'static str {
        match self {
            Self::Var => "CANLI",
            Self::Dalga2 => "Kervan'da henüz yok — DALGA-2",
            Self::Dalga3 => "Kervan'da henüz yok — DALGA-3",
            Self::Dalga4 => "Kervan'da henüz yok — DALGA-4",
            Self::KapsamDisi => "KAPSAM DIŞI",
        }
    }
}

/// Tek bir bölümün eşleme kaydı.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EslemeKaydi {
    pub ad: String,
    pub ekran: u8,
    pub karsilik: String,
    pub kaynak: KaynakDurumu,
    pub aciklama: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EslemeTablosu {
    pub bolum: Vec<EslemeKaydi>,
}

impl EslemeTablosu {
    pub fn toml_metninden(metin: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(metin)
    }

    pub fn varsayilan() -> Self {
        const METIN: &str = include_str!("../esleme.toml");
        Self::toml_metninden(METIN).expect("esleme.toml geçerli toml olmalı")
    }

    /// Belirli bir ekrana ait bölümleri döndürür.
    pub fn ekran_bolumleri(&self, ekran_no: u8) -> Vec<&EslemeKaydi> {
        self.bolum.iter().filter(|b| b.ekran == ekran_no).collect()
    }
}

/// `omniroute-bolumleri.txt` dosyasındaki bölüm adlarını sıralı liste olarak döndürür.
pub fn beklenen_bolumler_listesi() -> (String, Vec<String>) {
    const ICERIK: &str = include_str!("../omniroute-bolumleri.txt");
    let mut satirlar = ICERIK.lines().map(|s| s.trim()).filter(|s| !s.is_empty());
    let sha = satirlar
        .next()
        .expect("İlk satır kaynak SHA olmalıdır")
        .to_string();
    let bolumler: Vec<String> = satirlar.map(|s| s.to_string()).collect();
    (sha, bolumler)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::fs;
    use std::path::Path;

    #[test]
    fn test_esleme_tam_ve_birebir() {
        let (sha, beklenenler) = beklenen_bolumler_listesi();
        assert_eq!(
            sha, "dbe703a0000b303cd7b1cf5879cb8740e5bfce71",
            "Kaynak SHA dbe703a olmalıdır"
        );
        assert_eq!(
            beklenenler.len(),
            57,
            "OmniRoute anlık görüntüsünde tam 57 pano bölümü olmalı"
        );

        let tablo = EslemeTablosu::varsayilan();
        assert_eq!(
            tablo.bolum.len(),
            57,
            "esleme.toml tam 57 kayıt içermelidir"
        );

        let mut gorulen = HashSet::new();
        for k in &tablo.bolum {
            assert!(
                k.ekran >= 1 && k.ekran <= 5,
                "Ekran numarası 1 ile 5 arasında olmalıdır: {}",
                k.ad
            );
            assert!(
                gorulen.insert(&k.ad),
                "Bölüm birden fazla kez tanımlanmış: {}",
                k.ad
            );
        }

        for beklenen in &beklenenler {
            assert!(
                gorulen.contains(beklenen),
                "Beklenen bölüm eşleme tablosunda bulunamadı: {}",
                beklenen
            );
        }
    }

    #[test]
    fn test_d_satirlari_ve_kaynak_dagilimi() {
        let tablo = EslemeTablosu::varsayilan();
        let mut v_sayisi = 0;
        let mut d_sayisi = 0;
        let mut k_sayisi = 0;

        for k in &tablo.bolum {
            match k.kaynak {
                KaynakDurumu::Var => v_sayisi += 1,
                KaynakDurumu::Dalga2 | KaynakDurumu::Dalga3 | KaynakDurumu::Dalga4 => d_sayisi += 1,
                KaynakDurumu::KapsamDisi => k_sayisi += 1,
            }
        }

        assert_eq!(v_sayisi + d_sayisi + k_sayisi, 57);
        assert!(v_sayisi >= 15, "En az 15 V (canlı) satırı olmalı");
        assert!(d_sayisi >= 30, "Gelecek dalgalar için en az 30 D satırı olmalı");
        assert_eq!(k_sayisi, 2, "Gamification ve leaderboard kapsam dışıdır (K=2)");
    }

    #[test]
    #[ignore = "ref/ dizini gerektirir (FR-P1 (a) testi)"]
    fn bolum_listesi_guncel() {
        let ref_yol = Path::new("ref/omniroute-dbe703a/src/app/(dashboard)/dashboard");
        let ref_yol_ust = Path::new("../../ref/omniroute-dbe703a/src/app/(dashboard)/dashboard");
        let gercek_yol = if ref_yol.exists() {
            ref_yol
        } else if ref_yol_ust.exists() {
            ref_yol_ust
        } else {
            panic!("ref/omniroute-dbe703a dizini bulunamadı! Fail-closed kuralı gereği test düşer.");
        };

        let mut bulunan_dizinler = Vec::new();
        for entry in fs::read_dir(gercek_yol).expect("dashboard dizini okunabilmeli") {
            let entry = entry.expect("dizin girdisi");
            if entry.file_type().expect("dosya türü").is_dir() {
                bulunan_dizinler.push(entry.file_name().to_string_lossy().to_string());
            }
        }
        bulunan_dizinler.sort();

        let (_sha, beklenenler) = beklenen_bolumler_listesi();
        assert_eq!(
            bulunan_dizinler, beklenenler,
            "omniroute-bolumleri.txt dosyasındaki liste gerçek anlık görüntü ile bayt bayt özdeş olmalıdır"
        );
    }
}
