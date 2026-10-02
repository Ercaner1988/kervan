pub mod hiz_siniri;
pub mod onbellek;
pub mod stratejiler;

pub use hiz_siniri::{HizSiniriKarari, HizSinirlayici};
pub use onbellek::AnlamsalOnbellek;
pub use stratejiler::{Hedef, StratejiTuru, YonlendirmeBaglami, hedefleri_sirala};

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use std::time::Duration;

    fn ornek_hedefler() -> Vec<Hedef> {
        vec![
            Hedef {
                id: "h1".to_string(),
                saglayici: "openai".to_string(),
                model: "gpt-4o".to_string(),
                agirlik: 10,
                maliyet_milyon_basina: 5.0,
                baglam_boyutu: 128_000,
                kullanim_sayisi: 100,
                kalan_kota: 500,
                toplam_kota: 1000,
                sifirlanma_suresi_sn: 30,
                aktif_istekler: 2,
                onbellek_anahtari: Some("anahtar_a".to_string()),
                son_basarili_mi: false,
                hata_orani: 0.1,
                gecikme_ms: 120,
            },
            Hedef {
                id: "h2".to_string(),
                saglayici: "anthropic".to_string(),
                model: "claude-3-5-sonnet".to_string(),
                agirlik: 30,
                maliyet_milyon_basina: 3.0,
                baglam_boyutu: 200_000,
                kullanim_sayisi: 50,
                kalan_kota: 800,
                toplam_kota: 1000,
                sifirlanma_suresi_sn: 10,
                aktif_istekler: 1,
                onbellek_anahtari: Some("anahtar_b".to_string()),
                son_basarili_mi: true,
                hata_orani: 0.01,
                gecikme_ms: 80,
            },
            Hedef {
                id: "h3".to_string(),
                saglayici: "gemini".to_string(),
                model: "gemini-2.0-flash".to_string(),
                agirlik: 5,
                maliyet_milyon_basina: 0.5,
                baglam_boyutu: 1_000_000,
                kullanim_sayisi: 200,
                kalan_kota: 100,
                toplam_kota: 1000,
                sifirlanma_suresi_sn: 60,
                aktif_istekler: 5,
                onbellek_anahtari: None,
                son_basarili_mi: false,
                hata_orani: 0.05,
                gecikme_ms: 40,
            },
        ]
    }

    #[test]
    fn test_tum_19_strateji_calisir() {
        let hedefler = ornek_hedefler();
        let baglam = YonlendirmeBaglami {
            oturum_anahtari: Some("oturum-1".to_string()),
            son_bilinen_iyi_id: Some("h2".to_string()),
            talep_edilen_baglam_uzunlugu: 150_000,
            onbellek_aranan_anahtar: Some("anahtar_b".to_string()),
            rr_sayaci: 1,
        };

        let stratejiler = [
            StratejiTuru::Priority,
            StratejiTuru::RoundRobin,
            StratejiTuru::Weighted,
            StratejiTuru::Random,
            StratejiTuru::StrictRandom,
            StratejiTuru::LeastUsed,
            StratejiTuru::Lkgp,
            StratejiTuru::P2c,
            StratejiTuru::Auto,
            StratejiTuru::CostOptimized,
            StratejiTuru::ContextOptimized,
            StratejiTuru::ContextRelay,
            StratejiTuru::CacheOptimized,
            StratejiTuru::QuotaShare,
            StratejiTuru::QuotaWeighted,
            StratejiTuru::FillFirst,
            StratejiTuru::ResetAware,
            StratejiTuru::ResetWindow,
            StratejiTuru::Headroom,
        ];

        for &st in &stratejiler {
            let rng = StdRng::seed_from_u64(42);
            let sirali = hedefleri_sirala(st, &hedefler, &baglam, rng);
            assert_eq!(sirali.len(), 3, "Strateji {:?} 3 hedef dönmeli", st);
        }
    }

    #[test]
    fn test_oncelik_ve_round_robin() {
        let hedefler = ornek_hedefler();
        let rng = StdRng::seed_from_u64(42);
        let mut baglam = YonlendirmeBaglami::default();

        // Priority
        let p_sirali = hedefleri_sirala(StratejiTuru::Priority, &hedefler, &baglam, rng.clone());
        assert_eq!(p_sirali[0].id, "h1");

        // Round Robin shift = 1
        baglam.rr_sayaci = 1;
        let rr1 = hedefleri_sirala(StratejiTuru::RoundRobin, &hedefler, &baglam, rng.clone());
        assert_eq!(rr1[0].id, "h2");

        // Round Robin shift = 2
        baglam.rr_sayaci = 2;
        let rr2 = hedefleri_sirala(StratejiTuru::RoundRobin, &hedefler, &baglam, rng);
        assert_eq!(rr2[0].id, "h3");
    }

    #[test]
    fn test_agirlik_ve_maliyet() {
        let hedefler = ornek_hedefler();
        let rng = StdRng::seed_from_u64(42);
        let baglam = YonlendirmeBaglami::default();

        // Weighted: h2 (30) > h1 (10) > h3 (5)
        let w_sirali = hedefleri_sirala(StratejiTuru::Weighted, &hedefler, &baglam, rng.clone());
        assert_eq!(w_sirali[0].id, "h2");
        assert_eq!(w_sirali[1].id, "h1");
        assert_eq!(w_sirali[2].id, "h3");

        // Cost: h3 (0.5) < h2 (3.0) < h1 (5.0)
        let c_sirali = hedefleri_sirala(StratejiTuru::CostOptimized, &hedefler, &baglam, rng);
        assert_eq!(c_sirali[0].id, "h3");
        assert_eq!(c_sirali[1].id, "h2");
        assert_eq!(c_sirali[2].id, "h1");
    }

    #[test]
    fn test_lkgp_ve_onbellek() {
        let hedefler = ornek_hedefler();
        let rng = StdRng::seed_from_u64(42);
        let mut baglam = YonlendirmeBaglami {
            son_bilinen_iyi_id: Some("h2".to_string()),
            ..Default::default()
        };

        // LKGP h2'yi öne almalı
        let lkgp = hedefleri_sirala(StratejiTuru::Lkgp, &hedefler, &baglam, rng.clone());
        assert_eq!(lkgp[0].id, "h2");

        // Cache optimized anahtar_b'yi öne almalı
        baglam.onbellek_aranan_anahtar = Some("anahtar_b".to_string());
        let cache = hedefleri_sirala(StratejiTuru::CacheOptimized, &hedefler, &baglam, rng);
        assert_eq!(cache[0].id, "h2");
    }

    #[test]
    fn test_hiz_sinirlayici() {
        let sinirlayici = HizSinirlayici::yeni(2.0, 1.0); // Kapasite 2, saniyede 1 dolum
        assert_eq!(
            sinirlayici.kontrol_et("kullanici-1"),
            HizSiniriKarari::IzinVerildi
        );
        assert_eq!(
            sinirlayici.kontrol_et("kullanici-1"),
            HizSiniriKarari::IzinVerildi
        );

        // 3. istek reddedilmeli
        match sinirlayici.kontrol_et("kullanici-1") {
            HizSiniriKarari::Reddedildi { bekleme_ms } => {
                assert!(bekleme_ms > 0);
            }
            HizSiniriKarari::IzinVerildi => panic!("3. istek reddedilmeliydi"),
        }

        // Başka kullanıcı etkilenmemeli
        assert_eq!(
            sinirlayici.kontrol_et("kullanici-2"),
            HizSiniriKarari::IzinVerildi
        );
    }

    #[test]
    fn test_anlamsal_onbellek() {
        let onbellek = AnlamsalOnbellek::yeni(2, Duration::from_secs(60));
        let ozet1 = AnlamsalOnbellek::ozetle("gpt-4o", "Merhaba dünya", 0.7);
        let ozet2 = AnlamsalOnbellek::ozetle("gpt-4o", "Farklı soru", 0.7);

        assert_eq!(onbellek.getir(ozet1), None);

        onbellek.kaydet(ozet1, "Selam!".to_string());
        assert_eq!(onbellek.getir(ozet1), Some("Selam!".to_string()));

        onbellek.kaydet(ozet2, "Başka yanıt".to_string());
        assert_eq!(onbellek.kayit_sayisi(), 2);

        // 3. kayıt LRU ile en eskiyi çıkarmalı
        let ozet3 = AnlamsalOnbellek::ozetle("gpt-4o", "Üçüncü", 0.7);
        // ozet1'e erişelim ki ozet2 en eski kalsın
        let _ = onbellek.getir(ozet1);
        onbellek.kaydet(ozet3, "Üçüncü yanıt".to_string());

        assert_eq!(onbellek.kayit_sayisi(), 2);
        assert_eq!(onbellek.getir(ozet1), Some("Selam!".to_string()));
        assert_eq!(onbellek.getir(ozet2), None); // ozet2 çıkarılmış olmalı
    }
}
