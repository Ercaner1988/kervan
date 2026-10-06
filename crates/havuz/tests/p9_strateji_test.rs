//! P9 Yönlendirme Stratejileri Parite ve İkiz Testleri (FR-7.1, FR-7.2, FR-7.3).

use kervan_havuz::router::{HedefAdayi, RoutingStrategy};
use std::collections::HashMap;
use std::fs;

#[derive(serde::Deserialize)]
struct AltinStratejiDosyasi {
    tohum: u64,
    adaylar: Vec<HedefAdayi>,
    sonuclar: HashMap<String, Vec<String>>,
}

#[test]
fn test_fr71_kume_sayilari_ve_dahili_ayrimi() {
    assert_eq!(
        RoutingStrategy::KAMUYA_ACIK.len(),
        20,
        "Kamuya acik 20 strateji olmalidir"
    );
    assert_eq!(
        RoutingStrategy::DAHILI.len(),
        1,
        "Dahili 1 strateji olmalidir"
    );
    assert_eq!(RoutingStrategy::DAHILI[0], "quota-share");

    // quota-share dahili olmali, digerleri kamuya acik olmali
    assert!(RoutingStrategy::QuotaShare.is_internal());
    for &kamu in &RoutingStrategy::KAMUYA_ACIK {
        let strat = RoutingStrategy::from_str_canonical(kamu)
            .unwrap_or_else(|| panic!("Strateji cozumlenemedi: {}", kamu));
        assert!(
            !strat.is_internal(),
            "{} kamuya acik olmali ama dahili isaretli",
            kamu
        );
    }
}

fn altin_yolu(dosya_adi: &str) -> std::path::PathBuf {
    let d1 = std::path::Path::new("port/altin").join(dosya_adi);
    if d1.exists() {
        return d1;
    }
    let d2 = std::path::Path::new("../../port/altin").join(dosya_adi);
    if d2.exists() {
        return d2;
    }
    panic!("Altin dosya bulunamadi: {}", dosya_adi);
}

#[test]
fn test_fr72_altin_parite_girdi1_ve_girdi2() {
    let dosyalar = [
        "strateji_girdi1.json",
        "strateji_girdi2.json",
    ];

    for dosya_adi in &dosyalar {
        let tam_yol = altin_yolu(dosya_adi);
        let icerik = fs::read_to_string(&tam_yol)
            .unwrap_or_else(|e| panic!("Altin dosya okunamadi {:?}: {}", tam_yol, e));
        let altin: AltinStratejiDosyasi = serde_json::from_str(&icerik)
            .unwrap_or_else(|e| panic!("Altin dosya parse edilemedi {:?}: {}", tam_yol, e));

        assert_eq!(
            altin.sonuclar.len(),
            21,
            "Altin dosyada 21 strateji bulunmalidir"
        );

        for (strat_ad, beklenen_sira) in &altin.sonuclar {
            let strat = RoutingStrategy::from_str_canonical(strat_ad)
                .unwrap_or_else(|| panic!("Bilinmeyen strateji: {}", strat_ad));

            let sirali = strat.sirala(&altin.adaylar, Some(altin.tohum));
            let gercek_sira: Vec<String> = sirali.into_iter().map(|a| a.ad).collect();

            assert_eq!(
                &gercek_sira, beklenen_sira,
                "Strateji '{}' dosya '{:?}' icinde altin sira ile uyusmadi!\nBeklenen: {:?}\nGercek:   {:?}",
                strat_ad, tam_yol, beklenen_sira, gercek_sira
            );
        }
    }
}

#[test]
fn test_fr73_ikizler_fark_kaniti() {
    let tam_yol = altin_yolu("strateji_girdi1.json");
    let icerik = fs::read_to_string(&tam_yol).expect("strateji_girdi1.json okunamadi");
    let altin: AltinStratejiDosyasi =
        serde_json::from_str(&icerik).expect("strateji_girdi1.json parse edilemedi");

    // İkiz 1: random vs strict-random
    let random_strat = RoutingStrategy::Random;
    let strict_strat = RoutingStrategy::StrictRandom;
    let random_sira: Vec<String> = random_strat
        .sirala(&altin.adaylar, Some(altin.tohum))
        .into_iter()
        .map(|a| a.ad)
        .collect();
    let strict_sira: Vec<String> = strict_strat
        .sirala(&altin.adaylar, Some(altin.tohum))
        .into_iter()
        .map(|a| a.ad)
        .collect();

    assert_ne!(
        random_sira, strict_sira,
        "FR-7.3 ihlali: random ve strict-random ayni ciktiyi vermemelidir!"
    );

    // İkiz 2: reset-aware vs reset-window
    let aware_strat = RoutingStrategy::ResetAware;
    let window_strat = RoutingStrategy::ResetWindow;
    let aware_sira: Vec<String> = aware_strat
        .sirala(&altin.adaylar, Some(altin.tohum))
        .into_iter()
        .map(|a| a.ad)
        .collect();
    let window_sira: Vec<String> = window_strat
        .sirala(&altin.adaylar, Some(altin.tohum))
        .into_iter()
        .map(|a| a.ad)
        .collect();

    assert_ne!(
        aware_sira, window_sira,
        "FR-7.3 ihlali: reset-aware ve reset-window ayni ciktiyi vermemelidir!"
    );
}
