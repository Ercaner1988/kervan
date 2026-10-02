use crate::trait_tanimi::{KervanPaneli, PanelDurumu};
use eframe::egui;

/// Gösterge Paneli (OmniRoute Dashboard karşılığı)
#[derive(Debug, Default)]
pub struct GostergePaneli;

impl KervanPaneli for GostergePaneli {
    fn kimlik(&self) -> &'static str {
        "gosterge"
    }

    fn baslik(&self) -> &'static str {
        "Gösterge Paneli"
    }

    fn simge(&self) -> &'static str {
        "📊"
    }

    fn ciz(&mut self, ui: &mut egui::Ui, durum: &mut PanelDurumu) {
        ui.heading("📊 Sistem Durumu ve Metrikler");
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.group(|ui| {
                ui.label("Aktif İstekler");
                ui.heading(format!("{}", durum.aktif_istek_sayisi));
            });
            ui.group(|ui| {
                ui.label("Toplam İstek");
                ui.heading(format!("{}", durum.toplam_istek));
            });
            ui.group(|ui| {
                ui.label("Geçit Durumu");
                ui.colored_label(
                    egui::Color32::from_rgb(0, 200, 0),
                    "● Aktif (127.0.0.1:20129)",
                );
            });
        });

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(8.0);

        ui.label("Hızlı Erişim:");
        ui.horizontal(|ui| {
            if ui.button("➕ Yeni İstek Simülasyonu").clicked() {
                durum.toplam_istek += 1;
            }
            if ui.button("🔄 Sayaçları Sıfırla").clicked() {
                durum.aktif_istek_sayisi = 0;
            }
        });
    }
}
