//! # ekran4_baglam
//! Ekran 4: Bağlam ve Araçlar (Context, Sekmeler, Önbellek ve MCP)

use eframe::egui::{self, Color32, RichText, Ui};
use kervan_pano_veri::Ozet;

pub fn render(ui: &mut Ui, ozet: &Ozet) {
    ui.vertical(|ui| {
        ui.heading("Bağlam & Araçlar Masası");
        ui.separator();

        // CANLI SEKME LİSTESİ
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label(RichText::new("Tarayıcı Eklentisi Sekmeleri (/eklenti/sekmeler)").strong());
            ui.separator();

            if ozet.sekmeler.is_empty() {
                ui.label(RichText::new("Bağlı aktif tarayıcı sekmesi yok.").italics());
            } else {
                for s in &ozet.sekmeler {
                    ui.horizontal(|ui| {
                        let ikon = if s.aktif {
                            RichText::new("●").color(Color32::from_rgb(46, 125, 79))
                        } else {
                            RichText::new("○").color(Color32::GRAY)
                        };
                        ui.label(ikon);
                        ui.label(RichText::new(&s.baslik).strong());
                        ui.label(RichText::new(format!("({} ms)", s.ping_ms)).size(11.0).color(Color32::GRAY));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(&s.url).size(11.0).color(Color32::GRAY));
                        });
                    });
                }
            }
        });

        ui.add_space(16.0);

        // DALGA-2 VE DALGA-4 ROZETLERİ
        // G8: iki kutu 380 px sabit genişlikteydi; toplam 760 px + boşluk dar
        // pencerede taşıp metni kesiyordu. Ölçülen düzeltme (2×671+16 = 1358 >
        // 1324 mevcut) hâlâ 38 px taşıyordu: `set_width` çerçevenin kendi kenar
        // boşluğunu hesaba katmıyor. `ui.columns` alanı tam olarak ikiye böler,
        // metin sütun genişliğinde sarar — taşma imkânsız.
        ui.columns(2, |kolonlar| {
            egui::Frame::group(kolonlar[0].style()).show(&mut kolonlar[0], |ui| {
                ui.label(RichText::new("Bağlam Sıkıştırma (Compression)").strong());
                ui.separator();
                ui.label(
                    RichText::new("Kervan'da henüz yok — DALGA-4: Caveman, LLMLingua, Headroom & RTK Modülleri")
                        .color(Color32::from_rgb(180, 80, 50)),
                );
            });

            egui::Frame::group(kolonlar[1].style()).show(&mut kolonlar[1], |ui| {
                ui.label(RichText::new("Önbellek & Konuşma Belleği").strong());
                ui.separator();
                ui.label(
                    RichText::new("Kervan'da henüz yok — DALGA-2: Kalıcı SQLite Önbellek ve Oturum Geçmişi")
                        .color(Color32::from_rgb(180, 80, 50)),
                );
            });
        });

        ui.add_space(16.0);

        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.label(RichText::new("MCP & Ajan Becerileri").strong());
            ui.separator();
            ui.label(
                RichText::new("Kervan yerel MCP sunucusu aktiftir. UI üzerinden beceri ve araç yönetimi: Kervan'da henüz yok — DALGA-4")
                    .size(12.0)
                    .color(Color32::from_rgb(180, 80, 50)),
            );
        });
    });
}
