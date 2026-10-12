//! P9 Yönlendirme Stratejileri Parite ve İkiz Testleri (FR-7.1, FR-7.2, FR-7.3).
//! Altınlar gerçek OmniRoute kodundan üretilir: araclar/uretec/strateji-altin-uret.ts.

use kervan_havuz::router::{Hedef, RoutingStrategy, StratejiDurumu};
use std::collections::HashMap;
use std::fs;

const ALTINLAR: [&str; 4] = [
    "strateji_girdi1.json",
    "strateji_girdi2.json",
    "strateji_girdi3.json",
    "strateji_girdi4.json",
];

#[derive(serde::Deserialize)]
struct Kayit {
    model_str: String,
    execution_key: String,
    basarili: bool,
    gecikme_ms: f64,
}

#[derive(serde::Deserialize)]
struct Lkgp {
    provider: String,
    connection_id: Option<String>,
}

#[derive(serde::Deserialize)]
struct Durum {
    fiyat: HashMap<String, f64>,
    baglam: HashMap<String, u64>,
    kayitlar: Vec<Kayit>,
    lkgp: Option<Lkgp>,
}

#[derive(serde::Deserialize)]
struct AltinStratejiDosyasi {
    kaynak_yollar: Vec<String>,
    hedefler: Vec<Hedef>,
    durum: Durum,
    rastgele_48: Vec<u64>,
    takma_adlar: HashMap<String, String>,
    tuketim: HashMap<String, usize>,
    sonuclar: HashMap<String, Vec<Vec<String>>>,
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

fn altin_oku(dosya_adi: &str) -> AltinStratejiDosyasi {
    let icerik = fs::read_to_string(altin_yolu(dosya_adi)).expect("altin okunamadi");
    serde_json::from_str(&icerik).expect("altin parse edilemedi")
}

fn durum_kur(altin: &AltinStratejiDosyasi) -> StratejiDurumu {
    let mut d = StratejiDurumu::default();
    d.fiyat = altin.durum.fiyat.clone();
    d.baglam = altin.durum.baglam.clone();
    d.lkgp = altin
        .durum
        .lkgp
        .as_ref()
        .map(|l| (l.provider.clone(), l.connection_id.clone()));
    for k in &altin.durum.kayitlar {
        d.kaydet(&k.model_str, &k.execution_key, k.basarili, k.gecikme_ms);
    }
    d
}

/// Altındaki akışla stratejiyi adım adım koşturur; (sıralar, tüketilen sayı adedi) döner.
fn kostur(
    altin: &AltinStratejiDosyasi,
    strat: RoutingStrategy,
    adim: usize,
) -> (Vec<Vec<String>>, usize) {
    let mut durum = durum_kur(altin);
    let mut i = 0usize;
    let mut rastgele = || {
        let n = *altin.rastgele_48.get(i).expect("rastgele akis tukendi");
        i += 1;
        n as f64 / 2f64.powi(48)
    };
    let mut sonuc = Vec::new();
    for a in 0..adim {
        durum.rr_sayac = a as u64;
        let sirali = strat.sirala(&altin.hedefler, &mut durum, &mut rastgele);
        sonuc.push(sirali.iter().map(kimlik).collect());
    }
    (sonuc, i)
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
    assert!(RoutingStrategy::QuotaShare.is_internal());
    for &kamu in &RoutingStrategy::KAMUYA_ACIK {
        let strat = RoutingStrategy::from_str_canonical(kamu)
            .unwrap_or_else(|| panic!("Strateji cozumlenemedi: {}", kamu));
        assert!(
            !strat.is_internal(),
            "{} kamuya acik olmali ama dahili isaretli",
            kamu
        );
        assert_eq!(strat.as_str(), kamu);
    }
}

/// Her desteklenen strateji, her girdide, her adımda OmniRoute'un sırasını ve
/// rastgele akıştan çektiği sayı adedini birebir tutturmalı.
#[test]
fn test_fr72_altin_parite_girdi1_ve_girdi2() {
    for dosya in ALTINLAR {
        let altin = altin_oku(dosya);
        assert!(
            altin
                .kaynak_yollar
                .iter()
                .any(|y| y.ends_with("applyStrategyOrdering.ts")),
            "{dosya}: altin OmniRoute strateji kodundan uretilmemis"
        );
        for (ad, beklenen) in &altin.sonuclar {
            let strat = RoutingStrategy::from_str_canonical(ad)
                .unwrap_or_else(|| panic!("{dosya}: bilinmeyen strateji {ad}"));
            assert!(
                strat.desteklenir(),
                "{dosya}: altini olan '{ad}' desteklenmiyor isaretli"
            );
            let (gercek, tuketilen) = kostur(&altin, strat, beklenen.len());
            assert_eq!(
                &gercek, beklenen,
                "{dosya}: '{ad}' sirasi OmniRoute'tan farkli"
            );
            assert_eq!(
                tuketilen, altin.tuketim[ad],
                "{dosya}: '{ad}' rastgele akistan farkli sayida cekti"
            );
        }
    }
}

/// Desteklenen her stratejinin her girdide altını olmalı (altınsız "destek" yok).
#[test]
fn test_fr72_desteklenen_her_strateji_altinla_olculur() {
    for dosya in ALTINLAR {
        let altin = altin_oku(dosya);
        for &ad in RoutingStrategy::KAMUYA_ACIK
            .iter()
            .chain(&RoutingStrategy::DAHILI)
        {
            let strat = RoutingStrategy::from_str_canonical(ad).unwrap();
            assert_eq!(
                strat.desteklenir(),
                altin.sonuclar.contains_key(ad),
                "{dosya}: '{ad}' destek beyani ile altin kapsami uyusmuyor"
            );
        }
    }
}

#[test]
fn test_fr72_takma_adlar_omniroute_ile_ayni() {
    let altin = altin_oku(ALTINLAR[0]);
    assert!(!altin.takma_adlar.is_empty());
    for (girdi, beklenen) in &altin.takma_adlar {
        assert_eq!(
            RoutingStrategy::normallestir(girdi).as_str(),
            beklenen,
            "takma ad '{girdi}'"
        );
    }
}

/// FR-7.3: ikizler kodla ayrılır. priority ile fill-first çekirdekte ikizdir (sıra değişmez);
/// random ile strict-random, context-relay ile context-optimized ayrışır. Desteklenmeyenler
/// girdi sırasını döner.
#[test]
fn test_fr73_ikizler_fark_kaniti() {
    let altin = altin_oku("strateji_girdi2.json");
    let s = &altin.sonuclar;
    assert_eq!(s["priority"], s["fill-first"]);
    assert_ne!(s["random"][0], s["strict-random"][0]);
    assert_ne!(s["context-relay"], s["context-optimized"]);

    let girdi_sirasi: Vec<String> = altin.hedefler.iter().map(kimlik).collect();
    for &ad in RoutingStrategy::KAMUYA_ACIK
        .iter()
        .chain(&RoutingStrategy::DAHILI)
    {
        let strat = RoutingStrategy::from_str_canonical(ad).unwrap();
        if !strat.desteklenir() {
            let (gercek, _) = kostur(&altin, strat, 1);
            assert_eq!(
                gercek[0], girdi_sirasi,
                "desteklenmeyen '{ad}' girdi sirasini donmeli"
            );
        }
    }
}

/// Altındaki hedef kimliği: execution_key@model_str (aynı anahtarlı hedefler ayrışır).
fn kimlik(h: &Hedef) -> String {
    format!("{}@{}", h.execution_key, h.model_str)
}
