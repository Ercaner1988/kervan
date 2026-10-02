//! OmniRoute Çekirdek Bileşenleri Başarım Ölçümü (CodSpeed / Divan)
//!
//! Ölçülen bileşenler:
//! 1. Çeviri hattı: OpenAI ↔ Claude & Gemini istek/yanıt dönüşüm hızı
//! 2. Yönlendirme motoru: 19 kanonik strateji ile hedef sıralama maliyeti
//! 3. Depo kripto hattı: AES-256-GCM + scrypt şifreleme ve deşifreleme süresi

use divan::{Bencher, black_box};
use kervan_ceviri::{OpenAiIstek, OpenAiMesaj, openai_den_claude_istegi, openai_den_gemini_istegi};
use kervan_depo::Sifreleyici;
use kervan_yonlendirme::{Hedef, StratejiTuru, YonlendirmeBaglami, hedefleri_sirala};
use rand::SeedableRng;
use rand::rngs::StdRng;

fn main() {
    divan::main();
}

fn ornek_openai_istek() -> OpenAiIstek {
    OpenAiIstek {
        model: "claude-3-7-sonnet".to_string(),
        messages: vec![
            OpenAiMesaj {
                role: "system".to_string(),
                content: Some("Sen yüksek performanslı bir asistansın.".to_string()),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
            OpenAiMesaj {
                role: "user".to_string(),
                content: Some("Kervan ve OmniRoute karşılaştırmasını açıklar mısın?".to_string()),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
        ],
        temperature: Some(0.7),
        top_p: Some(0.9),
        max_tokens: Some(2048),
        stream: false,
        tools: None,
    }
}

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
            sifirlanma_suresi_sn: 5,
            aktif_istekler: 0,
            onbellek_anahtari: Some("anahtar_c".to_string()),
            son_basarili_mi: true,
            hata_orani: 0.0,
            gecikme_ms: 45,
        },
    ]
}

#[divan::bench]
fn bench_ceviri_openai_den_claude(bencher: Bencher) {
    let istek = ornek_openai_istek();
    bencher.bench_local(|| {
        let sonuc = openai_den_claude_istegi(black_box(&istek));
        let _ = black_box(sonuc);
    });
}

#[divan::bench]
fn bench_ceviri_openai_den_gemini(bencher: Bencher) {
    let istek = ornek_openai_istek();
    bencher.bench_local(|| {
        let sonuc = openai_den_gemini_istegi(black_box(&istek));
        let _ = black_box(sonuc);
    });
}

#[divan::bench]
fn bench_yonlendirme_sirala(bencher: Bencher) {
    let hedefler = ornek_hedefler();
    let baglam = YonlendirmeBaglami {
        talep_edilen_baglam_uzunlugu: 50_000,
        onbellek_aranan_anahtar: Some("anahtar_b".to_string()),
        ..Default::default()
    };
    let mut rng = StdRng::seed_from_u64(42);

    bencher.bench_local(|| {
        let sirali = hedefleri_sirala(
            StratejiTuru::CostOptimized,
            black_box(&hedefler),
            black_box(&baglam),
            &mut rng,
        );
        let _ = black_box(sirali);
    });
}

#[divan::bench]
fn bench_depo_sifreleme(bencher: Bencher) {
    let anahtar = "cok_gizli_anahtar_12345";
    let sifreleyici = Sifreleyici::yeni(Some(anahtar));
    let duz_metin = "sk-ant-api03-ornek-gizli-anahtar-degeri-123456789";

    bencher.bench_local(|| {
        let sifreli = sifreleyici
            .sifrele(black_box(duz_metin))
            .expect("Sifreleme hatasi");
        let _ = black_box(sifreli);
    });
}
