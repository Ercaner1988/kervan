//! # bicim
//! Kervan - Kilim Tema Görünüm Sınıfları ve Stil İzolasyonu
//!
//! Kural: Kilim-tema yalnızca görsel sınıfları (renk, doku, kenarlık ve dolgu) belirler;
//! arayüzün geometrisine, genişliklerine (width), esnek akışına ve konumlanmasına
//! ASLA doğrudan müdahale etmez.
//!
//! Görünüm Sınıfları:
//! 1. `Zemin / Parşömen`: Ana panel ve zemin renkleri
//! 2. `Gölgeli Hane`: Kartlar, istatistik kutuları ve iç çerçeveler
//! 3. `Vurgulu Hane`: Seçili hesap veya aktif odak çerçevesi
//! 4. `Durum Rozeti / Anlam Rengi`: Yeşil (Hazır), Hardal (Kota), Soluk (Giriş), Mercan (İnsan Kapısı), Çini (Vurgu)
//! 5. `Eylem Düğmesi`: Kilim paletine uyumlu düğme ve etiket görselleri

use eframe::egui::{Color32, CornerRadius, Frame, Margin, Stroke, Ui};
use kervan_pano_veri::HesapDurumu;
use kilim_tema::{Kilim, acik, koyu};

/// Etkin egui bağlamından Kilim paletini türetir
pub fn palet(ui: &Ui) -> Kilim {
    if ui.visuals().dark_mode {
        koyu()
    } else {
        acik()
    }
}

/// 1. Zemin / Parşömen Sınıfı:
///    Panel veya pencerelerin ana zemin dolgusu
pub fn parsemen_zemini(ui: &Ui) -> Color32 {
    palet(ui).zem
}

/// 2. Gölgeli Hane Sınıfı (İç Çerçeve):
///    Kartlar, listeler, tanı kutuları için hafif gölgeli/kenarlıklı yüzey çerçevesi
pub fn golgeli_hane(ui: &Ui) -> Frame {
    let p = palet(ui);
    Frame::group(ui.style())
        .fill(p.yuzey)
        .stroke(Stroke::new(1.0, p.cizgi))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::same(10))
}

/// 3. Vurgulu Hane Sınıfı:
///    Seçili hesap veya aktif rota kartı için vurgu çerçevesi
pub fn vurgulu_hane(ui: &Ui, vurgu_rengi: Color32) -> Frame {
    let p = palet(ui);
    Frame::group(ui.style())
        .fill(p.yuzey)
        .stroke(Stroke::new(1.5, vurgu_rengi))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::same(10))
}

/// 4. Durum Rozeti & Anlam Renkleri (Semantik Renk Sınıfı):
///    Kilim paletinden hesap durumu rengini döner
pub fn hesap_durum_rengi(durum: &HesapDurumu, ui: &Ui) -> Color32 {
    let p = palet(ui);
    match durum {
        HesapDurumu::Hazir => p.yesil,
        HesapDurumu::KotaBekliyor { .. } => p.hardal,
        HesapDurumu::GirisGerekli => p.soluk,
        HesapDurumu::InsanKapisi => p.mercan,
    }
}

/// Tanı kartı için çini/vurgu rengi
pub fn cini_rengi(ui: &Ui) -> Color32 {
    palet(ui).cini
}

/// Metin ana mürekkep rengi
pub fn murekkep_rengi(ui: &Ui) -> Color32 {
    palet(ui).murekkep
}

/// Metin ikincil/soluk rengi
pub fn soluk_renk(ui: &Ui) -> Color32 {
    palet(ui).soluk
}

/// Başarı yeşili
pub fn basari_rengi(ui: &Ui) -> Color32 {
    palet(ui).yesil
}

/// Uyarı / dikkat hardal rengi
pub fn uyari_rengi(ui: &Ui) -> Color32 {
    palet(ui).hardal
}

/// Hata / insan kapısı mercan rengi
pub fn hata_rengi(ui: &Ui) -> Color32 {
    palet(ui).mercan
}
