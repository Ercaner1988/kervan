//! Aciliş yolunun olcumu: `kervan.toml` ayrisma ve saglayici kayit defteri.
//!
//! NEDEN BURASI: ikisi de surec basina bir kez calisiyor, ama ikisi de HESAP
//! SAYISIYLA buyuyor ve 7/24 hedefi hesap sayisini buyutuyor. Ayrica kayit defteri
//! `Provider` tanimlarini bastan kuruyor; secici ezmeleri arttikca is artiyor.

use std::collections::HashMap;

use divan::{Bencher, black_box};
use kervan::config::{Config, SaglayiciAyar};
use kervan::kurulum::profil_adi;
use kervan::saglayicilar::{bul, kayit_defteri};

fn main() {
    divan::main();
}

/// Gercek yapilandirmanin sekli: hesaplar + hiz butcesi + kota isaretleri +
/// saglayici ezmeleri.
fn config_metni(hesap: usize) -> String {
    let mut s = String::from(
        "bosta_dakika = 15\n\n\
         [pacing]\n\
         min_delay_ms = 3000\n\
         jitter_ms = 2000\n\
         hourly_cap = 30\n\n\
         [kota]\n\
         isaretler = [\"reached your limit\", \"kotanız doldu\"]\n\
         bilinmeyen_pencere_dk = 60\n\n\
         [providers.claude]\n\
         response = \".font-claude-message\"\n\
         eszamanli = 2\n\n",
    );
    for i in 0..hesap {
        s.push_str(&format!(
            "[[accounts]]\n\
             name = \"ercan-{i}@example.com\"\n\
             providers = [\"gemini\", \"kimi\", \"copilot\"]\n\n"
        ));
    }
    s
}

/// 1 hesap = ilk kurulum, 20 hesap = `kota.rs`'te bellek olcumune konu olan boy.
#[divan::bench(args = [1, 20])]
fn config_ayristir(bencher: Bencher, hesap: usize) {
    let metin = config_metni(hesap);
    bencher.bench(|| toml::from_str::<Config>(black_box(&metin)).map(|c| c.accounts.len()));
}

/// Yerlesik varsayilanlardan kayit defteri kurulumu.
#[divan::bench]
fn defter_varsayilan(bencher: Bencher) {
    let ayarlar = HashMap::new();
    bencher.bench(|| kayit_defteri(black_box(&ayarlar)));
}

/// Yapilandirma ezmeli kurulum: site arayuzu degistiginde gercek kullanim bu.
#[divan::bench]
fn defter_ezmeli(bencher: Bencher) {
    let mut ayarlar: HashMap<String, SaglayiciAyar> = HashMap::new();
    for ad in ["claude", "gemini", "perplexity", "kimi"] {
        ayarlar.insert(
            ad.to_string(),
            SaglayiciAyar {
                response: Some(".yeni-secici".into()),
                prompt_input: Some("div[contenteditable='true']".into()),
                eszamanli: Some(2),
                ..Default::default()
            },
        );
    }
    bencher.bench(|| kayit_defteri(black_box(&ayarlar)));
}

#[divan::bench]
fn defterden_bul(bencher: Bencher) {
    let defter = kayit_defteri(&HashMap::new());
    bencher.bench(|| bul(black_box(&defter), black_box("copilot-m365")).is_ok());
}

/// Profil adi disaridan gelen epostadan turuyor ve dogrudan dizin yoluna giriyor;
/// karakter karakter temizlik yapiliyor.
#[divan::bench(args = ["ercan.aydin@example.com", "ERCAN+Test/../etiket@örnek.com"])]
fn profil(bencher: Bencher, eposta: &str) {
    bencher.bench(|| profil_adi(black_box(eposta)));
}
