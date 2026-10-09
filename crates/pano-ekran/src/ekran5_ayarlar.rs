//! # ekran5_ayarlar
//! Ekran 5: Ayarlar (Görünüm, kilim-tema, Güvenlik, Sistem Bilgileri)

use eframe::egui::{self, Color32, RichText, Ui};
use kervan_pano_veri::Ozet;
use kilim_tema::TemaTercihi;

pub fn render(ui: &mut Ui, ozet: &Ozet, tema_tercihi: &mut TemaTercihi) {
    ui.vertical(|ui| {
        ui.heading("Ayarlar & Sistem Masası");
        ui.separator();

        ui.horizontal(|ui| {
            // SOL: TEMA VE GÖRÜNÜM
            ui.vertical(|ui| {
                ui.set_width(380.0);

                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.label(RichText::new("Görünüm & Tema (kilim-tema)").strong());
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("Tema Seçici:");
                        kilim_tema::tema_secici(ui, tema_tercihi);
                    });

                    ui.add_space(8.0);
                    ui.label(RichText::new("Kâğıt lifi zemin ve kilim kenarlık paleti devrede.").size(11.0).color(Color32::GRAY));
                });

                ui.add_space(8.0);

                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.label(RichText::new("Sistem ve Çalışma Zamanı (/health)").strong());
                    ui.separator();
                    ui.label(format!("Kervan Sürümü: v{}", ozet.saglik.surum));
                    ui.label(format!("Render Arka Ucu: {}", ozet.saglik.backend));
                    ui.label(format!("Çalışma Süresi: {} sn", ozet.saglik.calisma_suresi_sn));
                });
            });

            ui.add_space(12.0);

            // SAĞ: GÜVENLİK VE SAĞLAYICI ANAHTARLARI
            ui.vertical(|ui| {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.label(RichText::new("REST Sağlayıcı Anahtarları ([rest.*])").strong());
                    ui.label(
                        RichText::new("Güvenlik kuralı: Anahtar değerleri ekranda ve günlükte asla görüntülenmez.")
                            .size(11.0)
                            .color(Color32::from_rgb(192, 79, 66)),
                    );
                    ui.separator();

                    if ozet.kimlikler.is_empty() {
                        ui.label(RichText::new("Yapılandırılmış REST anahtarı bulunamadı.").italics());
                    } else {
                        for k in &ozet.kimlikler {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(k).strong());
                                ui.label(
                                    RichText::new("● Ortam Değişkeninde Tanımlı")
                                        .size(11.0)
                                        .color(Color32::from_rgb(46, 125, 79)),
                                );
                            });
                        }
                    }
                });

                ui.add_space(8.0);

                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.label(
                        RichText::new("Kervan'da henüz yok — DALGA-4: Webhooks, Profil & API Manager Belirteçleri")
                            .size(11.0)
                            .color(Color32::from_rgb(180, 80, 50)),
                    );
                });
            });
        });
    });
}
