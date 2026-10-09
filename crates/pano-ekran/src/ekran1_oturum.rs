//! # ekran1_oturum
//! Ekran 1: Oturumlar ve Sağlayıcılar
//! Standart: KERVAN-ARAYUZ-ISTEMI.md & KERVAN-PANO-ANTIGRAVITY-ISTEMI.md

use eframe::egui::{self, Color32, RichText, Ui};
use kervan_pano_veri::{HesapDurumu, Ozet};
use kervan_tani::{TaniKarti, YetenekKarti};

pub fn render(
    ui: &mut Ui,
    ozet: &Ozet,
    secili_hesap_indeks: &mut usize,
    ornek_tani: &Option<TaniKarti>,
    ornek_yetenek: &Option<YetenekKarti>,
) {
    ui.horizontal(|ui| {
        // SOL PANEL: HESAP LİSTESİ (Genişlik: 420 px)
        egui::Frame::group(ui.style())
            .fill(ui.visuals().window_fill())
            .stroke(ui.visuals().window_stroke())
            .inner_margin(12.0)
            .show(ui, |ui| {
                ui.set_width(420.0);
                ui.horizontal(|ui| {
                    ui.heading("Oturumlar & Hesaplar");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("+ Hesap Ekle").clicked() {
                            // Hesap ekleme diyaloğu
                        }
                    });
                });
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (i, h) in ozet.hesaplar.iter().enumerate() {
                        let secili = *secili_hesap_indeks == i;
                        ui.group(|ui| {
                            let baslik = match &h.durum {
                                HesapDurumu::Hazir => {
                                    RichText::new(format!("● {}", h.ad)).color(Color32::from_rgb(46, 125, 79))
                                }
                                HesapDurumu::KotaBekliyor { kalan_dk } => {
                                    RichText::new(format!("◐ {} ({} dk ⏳)", h.ad, kalan_dk))
                                        .color(Color32::from_rgb(217, 164, 65))
                                }
                                HesapDurumu::GirisGerekli => {
                                    RichText::new(format!("○ {} [Giriş Gerekli]", h.ad))
                                        .color(Color32::from_rgb(122, 106, 85))
                                }
                                HesapDurumu::InsanKapisi => {
                                    RichText::new(format!("⚠ {} [İnsan Kapısı]", h.ad))
                                        .color(Color32::from_rgb(192, 79, 66))
                                }
                            };

                            if ui.selectable_label(secili, baslik).clicked() {
                                *secili_hesap_indeks = i;
                            }

                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(h.saglayicilar.join(" · "))
                                        .size(11.0)
                                        .color(Color32::GRAY),
                                );
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.small_button("👁 Aç").clicked() {}
                                    if ui.small_button("↻ Yenile").clicked() {}
                                });
                            });
                        });
                        ui.add_space(4.0);
                    }
                });
            });

        ui.add_space(8.0);

        // SAĞ PANEL: AYRINTI VE TANI KARTI
        egui::Frame::group(ui.style())
            .fill(ui.visuals().window_fill())
            .stroke(ui.visuals().window_stroke())
            .inner_margin(12.0)
            .show(ui, |ui| {
                ui.set_min_width(500.0);
                if let Some(secili) = ozet.hesaplar.get(*secili_hesap_indeks) {
                    ui.heading(format!("Hesap Ayrıntısı: {}", secili.ad));
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.group(|ui| {
                            ui.label(RichText::new("Saatlik İstek Tavanı:").size(11.0));
                            if secili.saatlik_tavan == 0 {
                                ui.label(RichText::new("Sınırsız").strong());
                            } else {
                                ui.label(
                                    RichText::new(format!("{} istek / saat", secili.saatlik_tavan))
                                        .strong()
                                        .color(Color32::from_rgb(22, 83, 126)),
                                );
                            }
                        });

                        ui.group(|ui| {
                            ui.label(RichText::new("Kota Penceresi:").size(11.0));
                            ui.label(RichText::new(&secili.kota_penceresi).strong());
                        });
                    });

                    ui.add_space(8.0);

                    // TANI KARTI BÖLÜMÜ (KERVAN-TANI-ISTEMI FR-8)
                    egui::Frame::group(ui.style())
                        .stroke(egui::Stroke::new(1.5, Color32::from_rgb(22, 83, 126)))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("📋 Son Tanı Kartı (TaniKarti)")
                                    .strong()
                                    .color(Color32::from_rgb(22, 83, 126)),
                            );
                            ui.separator();

                            if let Some(tani) = ornek_tani {
                                ui.horizontal(|ui| {
                                    ui.label("Sınıf:");
                                    ui.label(RichText::new(tani.sinif.etiket()).strong());
                                    ui.label(format!("(Güven: {:?})", tani.guven));
                                });
                                for k in &tani.kanit {
                                    ui.label(
                                        RichText::new(format!("• [{}] {}", k.tur, k.metin))
                                            .size(11.0),
                                    );
                                }
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Öneri:").strong());
                                    ui.label(&tani.oneri.ayrinti);
                                    if ui.button(format!("[{}]", tani.oneri.eylem)).clicked() {}
                                });
                            } else {
                                ui.label(RichText::new("Henüz tanı kaydı yok").italics());
                            }
                        });

                    ui.add_space(8.0);

                    // YETENEK KARTI BÖLÜMÜ (KERVAN-TANI-ISTEMI §7)
                    egui::Frame::group(ui.style()).show(ui, |ui| {
                        ui.label(RichText::new("⚡ Sağlayıcı Yetenek Kartı (YetenekKarti)").strong());
                        ui.separator();

                        if let Some(yet) = ornek_yetenek {
                            for y in &yet.yetenekler {
                                let durum_ikonu = if y.var {
                                    RichText::new("✓").color(Color32::from_rgb(46, 125, 79)).strong()
                                } else {
                                    RichText::new("✗").color(Color32::from_rgb(192, 79, 66)).strong()
                                };
                                ui.horizontal(|ui| {
                                    ui.label(durum_ikonu);
                                    ui.label(RichText::new(&y.ad).strong());
                                    if let Some(kanit) = &y.kanit {
                                        ui.label(RichText::new(kanit).size(11.0).color(Color32::GRAY));
                                    }
                                });
                            }
                        } else {
                            ui.label(RichText::new("Henüz yetenek taraması yapılmadı").italics());
                        }
                    });
                } else {
                    ui.label("Seçili hesap bulunamadı.");
                }
            });
    });
}
