//! Kota ogrenicinin olcumu.
//!
//! NEDEN BURASI: `kota.rs`'in kendi basindaki tabloda "defter buyudukce `ac()` ne
//! kadar suruyor" elle olculmus ve Turso gocu o sayilara dayanarak reddedilmis.
//! Elle olculen bir sayi bir kez dogrudur; burada her istekte yeniden olculur,
//! yani karar sessizce eskimez.
//!
//! Olculen uc is var ve hepsi surec basina degil ISTEK basina donuyor:
//!   - `tahmin_et`  : ozet cikarilirken tum olay akisi bir kez taraniyor
//!   - JSONL ayrisma: `Defter::ac` her satiri `serde_json` ile cozuyor
//!   - `kota_sinyali`: her yanit metni isaret listesine karsi sinaniyor

use divan::{Bencher, black_box};
use kervan::kota::{Olay, Sonuc, kota_sinyali, tahmin_et};

fn main() {
    divan::main();
}

/// Gercek defterin sekli: her pencerede birkac basari, sonra bir kota carpmasi.
///
/// Duz "hepsi basari" bir akis `tahmin_et`'in asil isini — pencere ve kapasite
/// cikarimini — hic tetiklemezdi; olcum kolay yolu olcerdi.
fn defter(olay_sayisi: usize) -> Vec<Olay> {
    const PENCERE_BASINA: usize = 30;
    (0..olay_sayisi)
        .map(|i| Olay {
            t: 1_780_000_000 + i as u64 * 60,
            hesap: "ercan-gemini-1".into(),
            saglayici: Some(if i % 3 == 0 { "gemini" } else { "kimi" }.into()),
            sonuc: if i % PENCERE_BASINA == PENCERE_BASINA - 1 {
                Sonuc::Kota
            } else if i % 7 == 3 {
                Sonuc::Hata
            } else {
                Sonuc::Basari
            },
            not: None,
        })
        .collect()
}

/// 88 = bugunku gercek defter, 20.000 = modulun kendi aylik ongorusu,
/// 200.000 = yillarca kullanim. Ucu birlikte olculuyor ki maliyetin defter
/// boyuyla nasil buyudugu gorunur olsun.
#[divan::bench(args = [88, 20_000, 200_000])]
fn tahmin_et_akis(bencher: Bencher, n: usize) {
    let olaylar = defter(n);
    bencher.bench(|| tahmin_et(black_box(&olaylar)));
}

/// `Defter::ac` yolunun asil bedeli: satir satir JSONL ayrisma.
#[divan::bench(args = [88, 20_000])]
fn defter_ayristir(bencher: Bencher, n: usize) {
    let metin = defter(n)
        .iter()
        .map(|o| serde_json::to_string(o).expect("olay yazilamadi"))
        .collect::<Vec<_>>()
        .join("\n");
    bencher.bench(|| {
        let mut sayi = 0usize;
        for satir in black_box(&metin).lines() {
            if serde_json::from_str::<Olay>(satir).is_ok() {
                sayi += 1;
            }
        }
        sayi
    });
}

/// Her istek bir satir yaziyor: `Defter::ekle`'nin diske dokunmayan kismi.
#[divan::bench]
fn olay_yaz(bencher: Bencher) {
    let olay = Olay {
        t: 1_786_823_552,
        hesap: "ercan-gemini-1".into(),
        saglayici: Some("gemini".into()),
        sonuc: Sonuc::Kota,
        not: Some("You've reached your limit".into()),
    };
    bencher.bench(|| serde_json::to_string(black_box(&olay)));
}

/// Isaret listesindeki son ogenin eslesmesi en kotu hal: liste bastan sona taranir.
#[divan::bench]
fn kota_sinyali_eslesen(bencher: Bencher) {
    let isaretler = isaretler();
    let metin = "Uzgunum, bu modelde kotaniz doldu. Daha sonra tekrar deneyin.";
    bencher.bench(|| kota_sinyali(black_box(metin), black_box(&isaretler)));
}

/// Asil sicak yol: yanitlarin cogu kota mesaji DEGIL, yani hicbir isaret tutmaz.
#[divan::bench]
fn kota_sinyali_eslesmeyen(bencher: Bencher) {
    let isaretler = isaretler();
    let metin = "Ankara. Nufusu 2024 sayimina gore yaklasik 5,8 milyon.";
    bencher.bench(|| kota_sinyali(black_box(metin), black_box(&isaretler)));
}

/// Uzun yanit uzunluk kapisindan doner; olculmezse o kapinin ucuz kaldigini
/// kimse dogrulamaz — her model yaniti bu yoldan geciyor.
#[divan::bench]
fn kota_sinyali_uzun_yanit(bencher: Bencher) {
    let isaretler = isaretler();
    let metin = "Kota limitleri soyle isler: ".repeat(400);
    bencher.bench(|| kota_sinyali(black_box(&metin), black_box(&isaretler)));
}

fn isaretler() -> Vec<String> {
    vec![
        "reached your limit".into(),
        "sınırına ulaştınız".into(),
        "limitine ulaştınız".into(),
        "kotanız doldu".into(),
    ]
}
