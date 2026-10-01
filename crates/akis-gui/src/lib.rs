//! Kervan akış panelleri (egui): `kervan-akis` modüllerini arayüzden çağırır.
//!
//! Tasarım: Penpot "KERVAN — Akış" sayfası (2026-10-01). Paneller iş yapmaz, ne
//! yapılacağını `Eylem` olarak döndürür; sağlayıcı sekmesine istek gönderen ve
//! yanıtı `Akis::sonuc` ile geri veren `kervan-gui`dir. Böylece bu sandık sunucuya
//! bağımlı değildir.

pub mod havuz;
pub mod ses;
pub mod video;

use eframe::egui::{self, Color32};
use kervan_akis::audio_video::{KayitliMedya, MedyaYoneticisi};

/// Penpot koyu tema jetonlarının egui karşılıkları.
pub mod renk {
    use super::Color32;
    pub const VURGU: Color32 = Color32::from_rgb(0x5A, 0xA2, 0xF5);
    pub const BASARI: Color32 = Color32::from_rgb(0x3C, 0xC4, 0x7C);
    pub const UYARI: Color32 = Color32::from_rgb(0xE0, 0xA4, 0x3A);
    pub const HATA: Color32 = Color32::from_rgb(0xF4, 0x7C, 0x75);
    pub const SOLUK: Color32 = Color32::from_rgb(0x9A, 0xA3, 0xAF);
}

/// Sağlayıcı sekmesine gönderilen bir işin yanıtı nereye dönecek.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Is {
    Transkript,
    Video(String),
}

/// Panelin gui'den istediği iş.
#[derive(Debug, Clone, PartialEq)]
pub enum Eylem {
    /// İstemi (varsa ekli dosyayla) seçili sağlayıcıya sor; yanıtı `Akis::sonuc`'a ver.
    /// `video` doğruysa istem gui'nin video çerçevesine sarılır.
    Gonder {
        is: Is,
        istem: String,
        dosya: Option<String>,
        video: bool,
    },
    /// Metni sohbet kutusuna taşı.
    SohbeteEkle(String),
}

/// Ses + video panellerinin ortak durumu; üretilen her şey tek medya arşivine düşer.
#[derive(Default)]
pub struct Akis {
    pub ses: ses::SesPaneli,
    pub video: video::VideoPaneli,
    medya: MedyaYoneticisi,
    suzgec: Option<&'static str>,
}

impl Akis {
    /// Arka plan işlerinin (seslendirme) bitenlerini topla. Her karede çağrılır.
    pub fn guncelle(&mut self) {
        if let Some(m) = self.ses.seslendirme_bitti() {
            self.medya.medya_arsivle(m);
        }
    }

    pub fn ses_ciz(&mut self, ui: &mut egui::Ui) -> Option<Eylem> {
        let e = self.ses.ciz(ui);
        ui.add_space(12.0);
        self.arsiv_ciz(ui);
        e
    }

    pub fn video_ciz(&mut self, ui: &mut egui::Ui) -> Option<Eylem> {
        let e = self.video.ciz(ui, &mut self.medya);
        ui.add_space(12.0);
        self.arsiv_ciz(ui);
        e
    }

    /// Gönderilen işin yanıtı (Err = hata metni).
    pub fn sonuc(&mut self, is: Is, yanit: Result<String, String>) {
        match is {
            Is::Transkript => self.ses.transkript_sonucu(yanit),
            Is::Video(id) => {
                if let Some(m) = self.video.sonuc(&mut self.medya, &id, yanit) {
                    self.medya.medya_arsivle(m);
                }
            }
        }
    }

    fn arsiv_ciz(&mut self, ui: &mut egui::Ui) {
        let liste = self.medya.arsivlenen_medyalar();
        ui.group(|ui| {
            ui.strong(format!("🗂 Medya arşivi · {}", liste.len()));
            ui.horizontal(|ui| {
                for (etiket, tur) in [
                    ("Tümü", None),
                    ("🎬 Video", Some("video")),
                    ("🎙 Ses", Some("ses")),
                ] {
                    if ui.selectable_label(self.suzgec == tur, etiket).clicked() {
                        self.suzgec = tur;
                    }
                }
            });
            if liste.is_empty() {
                ui.colored_label(renk::SOLUK, "Henüz üretilmiş medya yok.");
            }
            for m in liste
                .iter()
                .rev()
                .filter(|m| self.suzgec.is_none_or(|t| t == m.tur))
            {
                medya_satiri(ui, m);
            }
        });
    }
}

fn medya_satiri(ui: &mut egui::Ui, m: &KayitliMedya) {
    ui.horizontal(|ui| {
        ui.label(if m.tur == "video" { "🎬" } else { "🎙" });
        ui.vertical(|ui| {
            ui.label(&m.baslik);
            ui.colored_label(renk::SOLUK, egui::RichText::new(&m.dosya_yolu).small());
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("▶ Aç").clicked() {
                ac(ui.ctx(), &m.dosya_yolu);
            }
            if m.boyut_bayt > 0 {
                ui.colored_label(renk::SOLUK, boyut_yazisi(m.boyut_bayt));
            }
        });
    });
}

/// Bağlantıyı tarayıcıda, yerel dosyayı sistemin varsayılan uygulamasıyla açar.
pub fn ac(ctx: &egui::Context, yol: &str) {
    if yol.starts_with("http") {
        ctx.open_url(egui::OpenUrl::new_tab(yol));
        return;
    }
    let komut = if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    if let Err(e) = std::process::Command::new(komut).arg(yol).spawn() {
        eprintln!("açılamadı ({yol}): {e}");
    }
}

pub fn boyut_yazisi(bayt: u64) -> String {
    match bayt {
        b if b >= 1 << 20 => format!("{:.1} MB", b as f64 / f64::from(1 << 20)),
        b if b >= 1 << 10 => format!("{} KB", b >> 10),
        b => format!("{b} B"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boyut_okunur_yazilir() {
        assert_eq!(boyut_yazisi(512), "512 B");
        assert_eq!(boyut_yazisi(6 * 1024), "6 KB");
        assert_eq!(boyut_yazisi(18_400_000), "17.5 MB");
    }
}
