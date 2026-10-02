use crate::trait_tanimi::{KervanPaneli, PanelDurumu};
use eframe::egui;
use kervan_katalog::Katalog;

/// Model Yönetim Paneli
pub struct ModelYonetimPaneli {
    katalog: Katalog,
}

impl Default for ModelYonetimPaneli {
    fn default() -> Self {
        Self {
            katalog: Katalog::yerlesik(),
        }
    }
}

impl KervanPaneli for ModelYonetimPaneli {
    fn kimlik(&self) -> &'static str {
        "modeller"
    }

    fn baslik(&self) -> &'static str {
        "Model Kataloğu"
    }

    fn simge(&self) -> &'static str {
        "🧠"
    }

    fn ciz(&mut self, ui: &mut egui::Ui, durum: &mut PanelDurumu) {
        ui.heading("🧠 Sağlayıcı ve Model Kataloğu");
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Model Ara:");
            ui.text_edit_singleline(&mut durum.aranan_model);
            if ui.button("Temizle").clicked() {
                durum.aranan_model.clear();
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        let aranan = durum.aranan_model.to_ascii_lowercase();

        egui::ScrollArea::vertical()
            .max_height(400.0)
            .show(ui, |ui| {
                for saglayici in self.katalog.tum_saglayicilar() {
                    let eslesen_modeller: Vec<_> = saglayici
                        .modeller
                        .iter()
                        .filter(|m| {
                            aranan.is_empty()
                                || m.id.to_ascii_lowercase().contains(&aranan)
                                || m.ad.to_ascii_lowercase().contains(&aranan)
                                || saglayici.id.to_ascii_lowercase().contains(&aranan)
                        })
                        .collect();

                    if !eslesen_modeller.is_empty() {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.strong(&saglayici.ad);
                                ui.label(format!(
                                    "(ID: {}, Format: {})",
                                    saglayici.id, saglayici.format
                                ));
                            });
                            for m in eslesen_modeller {
                                ui.horizontal(|ui| {
                                    ui.label(format!("  • {}", m.ad));
                                    ui.small(format!("[{}]", m.id));
                                    if m.dusunme {
                                        ui.small("💡 Düşünme");
                                    }
                                    if m.gorsel {
                                        ui.small("👁 Görsel");
                                    }
                                });
                            }
                        });
                        ui.add_space(4.0);
                    }
                }
            });
    }
}
