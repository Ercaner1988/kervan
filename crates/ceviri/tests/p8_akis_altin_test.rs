//! FR-6.3 akış paritesi: `port/altin/akis_*.json` (gerçek OmniRoute `translateResponse`
//! çıktısı, `araclar/uretec/akis-altin-uret.ts`) olay olay Kervan akış çeviricileriyle
//! karşılaştırılır. Her olayın ürettiği parça listesi tam alan eşitliğiyle denetlenir.

use serde_json::Value;
use std::fs;
use std::path::Path;

use kervan_ceviri::*;

fn altinlar() -> Vec<(String, Value)> {
    let dizin = Path::new("../../port/altin");
    let mut sonuc: Vec<_> = fs::read_dir(dizin)
        .expect("port/altin okunamadı")
        .filter_map(|g| g.ok())
        .map(|g| g.file_name().to_string_lossy().into_owned())
        .filter(|ad| ad.starts_with("akis_") && ad.ends_with(".json"))
        .map(|ad| {
            let icerik = fs::read_to_string(dizin.join(&ad)).expect("altın okunamadı");
            let json: Value = serde_json::from_str(&icerik).expect("altın JSON değil");
            (ad, json)
        })
        .collect();
    sonuc.sort_by(|a, b| a.0.cmp(&b.0));
    sonuc
}

#[test]
fn p8_akis_altin_parite_testi() {
    let altinlar = altinlar();
    assert_eq!(altinlar.len(), 9, "9 akış altını bekleniyordu");
    for (ad, altin) in altinlar {
        assert_eq!(altin["kaynak_sha"], "dbe703a", "{ad}");
        assert_eq!(altin["bosaltma"], Value::Array(vec![]), "{ad}");
        let saat_ms = altin["sabit_saat_ms"].as_u64().expect("sabit_saat_ms");
        let olaylar = altin["olaylar"].as_array().expect("olaylar");
        let beklenen = altin["parcalar"].as_array().expect("parcalar");
        assert_eq!(olaylar.len(), beklenen.len(), "{ad}");

        let mut claude = ClaudeAkisCevirici::default();
        let mut gemini = GeminiAkisCevirici::default();
        for (i, (olay, beklenen)) in olaylar.iter().zip(beklenen).enumerate() {
            let parcalar = match altin["bicim"].as_str() {
                Some("claude") => {
                    let o: ClaudeOlay = serde_json::from_value(olay.clone())
                        .unwrap_or_else(|e| panic!("{ad} olay {i} ayrıştırılamadı: {e}"));
                    claude.cevir(&o, saat_ms / 1000)
                }
                Some("gemini") => {
                    let y: GeminiYanit = serde_json::from_value(olay.clone())
                        .unwrap_or_else(|e| panic!("{ad} olay {i} ayrıştırılamadı: {e}"));
                    gemini.cevir(&y, saat_ms)
                }
                b => panic!("{ad}: bilinmeyen biçim {b:?}"),
            };
            assert_eq!(
                serde_json::to_value(&parcalar).unwrap(),
                *beklenen,
                "{ad} olay {i}"
            );
        }
    }
}
