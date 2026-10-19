//! # ekran3_izleme
//! Ekran 3: İzleme (Monitoring, Telemetri ve Hata Kayıtları)

use eframe::egui::{self, Color32, RichText, Ui};
use kervan_pano_veri::Ozet;

pub fn render(ui: &mut Ui, ozet: &Ozet) {
    ui.vertical(|ui| {
        ui.heading("İzleme & Telemetri Masası");
        ui.separator();

        // ÜST SAYAÇLAR
        ui.horizontal(|ui| {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_width(180.0);
                ui.label(RichText::new("Toplam İstek").size(11.0).color(Color32::GRAY));
                ui.label(
                    RichText::new(format!("{}", ozet.telemetri.toplam_istek))
                        .size(24.0)
                        .strong()
                        .color(Color32::from_rgb(22, 83, 126)),
                );
                ui.label(RichText::new("Son 24 saat").size(10.0).color(Color32::GRAY));
            });

            ui.add_space(8.0);

            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_width(180.0);
                ui.label(RichText::new("Başarı Oranı").size(11.0).color(Color32::GRAY));
                let oran = if ozet.telemetri.toplam_istek > 0 {
                    (ozet.telemetri.basarili_istek as f64 / ozet.telemetri.toplam_istek as f64) * 100.0
                } else {
                    100.0
                };
                ui.label(
                    RichText::new(format!("%{:.1}", oran))
                        .size(24.0)
                        .strong()
                        .color(Color32::from_rgb(46, 125, 79)),
                );
                ui.label(RichText::new(format!("{} başarılı", ozet.telemetri.basarili_istek)).size(10.0));
            });

            ui.add_space(8.0);

            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_width(180.0);
                ui.label(RichText::new("Ortalama Gecikme").size(11.0).color(Color32::GRAY));
                ui.label(
                    RichText::new(format!("{} ms", ozet.telemetri.ortalama_gecikme_ms))
                        .size(24.0)
                        .strong()
                        .color(Color32::from_rgb(217, 164, 65)),
                );
                ui.label(RichText::new("Uçtan uca süre").size(10.0).color(Color32::GRAY));
            });
        });

        ui.add_space(16.0);

        // HATA KAYITLARI
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label(RichText::new("Son Hatalar (Sır Temizlenmiş)").strong());
            ui.separator();

            if ozet.son_hatalar.is_empty() {
                ui.label(
                    RichText::new("✓ Son kaydedilen bağlantı veya doğrulama hatası yok.")
                        .color(Color32::from_rgb(46, 125, 79)),
                );
            } else {
                for h in &ozet.son_hatalar {
                    ui.label(format!("[{}] {} — {}", h.zaman, h.saglayici, h.mesaj));
                }
            }
        });

        ui.add_space(16.0);

        // GRAFİK VE KALICI ANALİTİK ROZETLERİ (DÜRÜST EKSİK)
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label(RichText::new("Analitik, Maliyet ve Geçmiş Grafikler").strong());
            ui.separator();
            ui.label(
                RichText::new("Kervan'da henüz yok — DALGA-2: Kalıcı Maliyet, Denetim ve Günlük Arşivi (SQLite/Turso)")
                    .color(Color32::from_rgb(180, 80, 50)),
            );
            ui.label(
                RichText::new("Kervan'da henüz yok — DALGA-3: Radar Sağlayıcı Hız Grafiği & Token Analitiği")
                    .color(Color32::from_rgb(180, 80, 50)),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new("Kapsam Dışı: Gamification (Oyunlaştırma) ve Leaderboard (Kervan mimarisinde yer almaz)")
                    .size(11.0)
                    .color(Color32::GRAY),
            );
        });
    });
}
