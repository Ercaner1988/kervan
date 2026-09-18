//! Istek yolunun olcumu: bir `/v1/chat/completions` istegi tarayiciya dokunmadan
//! once yapilan tum is.
//!
//! NEDEN BURASI: tarayici turu saniyeler suruyor, buradaki islerin toplami ise
//! mikrosaniyeler — ama hepsi HER istekte ve kilit altinda calisiyor
//! (`konusma_izi` havuzdan hesap kiralanirken, `tohum_istemi` dizi bos oldugunda
//! tum gecmisi kopyalayarak). Gecmis buyudukce bu isler sessizce buyuyor; olcum
//! bunun ne zaman oldugunu soyler.

use std::collections::HashMap;

use divan::{Bencher, black_box};
use kervan::pool::insan_kapisi_gerekir;
use kervan::saglayicilar::{kayit_defteri, model_saglayicisi};
use kervan::server::{
    ChatRequest, Message, TOHUM_AZAMI, konusma_izi, son_kullanici_mesaji, tohum_istemi,
};

fn main() {
    divan::main();
}

fn mesaj(rol: &str, icerik: &str) -> Message {
    Message {
        role: rol.to_string(),
        content: icerik.to_string(),
    }
}

/// Ajan istemcisinin gonderdigi sekil: bir sistem yonergesi, sonra soru-cevap
/// turleri. `tur` = kac tur gecmis birikmis.
fn konusma(tur: usize) -> Vec<Message> {
    let mut msgs = vec![mesaj(
        "system",
        "Sen Kervan uzerinden calisan bir asistansin. Kisa ve kesin cevap ver.",
    )];
    for i in 0..tur {
        msgs.push(mesaj(
            "user",
            &format!("{i}. tur sorusu: {}", "ayrinti ".repeat(20)),
        ));
        msgs.push(mesaj(
            "assistant",
            &format!("{i}. tur cevabi: {}", "aciklama ".repeat(40)),
        ));
    }
    msgs.push(mesaj("user", "asil soru: bu konusmanin ozeti ne?"));
    msgs
}

/// Govde ayrisma: axum `Json<ChatRequest>` ile bunu yapiyor.
#[divan::bench(args = [1, 20])]
fn istek_ayristir(bencher: Bencher, tur: usize) {
    let govde = serde_json::to_string(&serde_json::json!({
        "model": "kervan/gemini-3.1-pro",
        "stream": true,
        "messages": konusma(tur),
    }))
    .expect("govde yazilamadi");
    bencher
        .bench(|| serde_json::from_str::<ChatRequest>(black_box(&govde)).map(|r| r.messages.len()));
}

/// Konusma parmak izi: hesap sabitlemenin tamami buna dayaniyor, her istekte bir kez.
#[divan::bench(args = [1, 20])]
fn iz(bencher: Bencher, tur: usize) {
    let msgs = konusma(tur);
    bencher.bench(|| konusma_izi(black_box(&msgs)));
}

#[divan::bench(args = [1, 20])]
fn son_mesaj(bencher: Bencher, tur: usize) {
    let msgs = konusma(tur);
    bencher.bench(|| son_kullanici_mesaji(black_box(&msgs)));
}

/// Bos dizide gecmis tohumlama: tum gecmis tek metne dokuluyor.
#[divan::bench(args = [1, 20])]
fn tohum(bencher: Bencher, tur: usize) {
    let msgs = konusma(tur);
    bencher.bench(|| tohum_istemi(black_box(&msgs), black_box(TOHUM_AZAMI)));
}

/// Sinir asilan hal: en eski turlar dusuruluyor, yani karakter sayimi her mesajda
/// yapiliyor. Uzun konusmalarda asil bedel burada.
#[divan::bench]
fn tohum_kirpilan(bencher: Bencher) {
    let msgs = konusma(60);
    bencher.bench(|| tohum_istemi(black_box(&msgs), black_box(4_000)));
}

/// Model kimliginden saglayici secimi: esitlik degil ICERME araniyor, yani
/// defterdeki her ad taraniyor ve eslesenler uzunluga gore siralaniyor.
#[divan::bench(args = ["kervan/gemini-3.1-pro", "kervan/copilot-m365", "bilinmeyen-model"])]
fn model_secimi(bencher: Bencher, model: &str) {
    let defter = kayit_defteri(&HashMap::new());
    bencher.bench(|| model_saglayicisi(black_box(model), black_box(&defter)));
}

/// Hata siniflandirmasi: her basarisiz istek bu listeden geciyor. Yanlis tarafa
/// dusmesi ya saglam bir hesabi duraklatir ya duvari gorunmez kilar.
#[divan::bench(args = ["gecici ariza: yanit zaman asimina ugradi", "Verify it's you to continue"])]
fn insan_kapisi(bencher: Bencher, hata: &str) {
    bencher.bench(|| insan_kapisi_gerekir(black_box(hata)));
}
