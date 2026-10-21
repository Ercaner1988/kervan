//! # ekran1_oturum
//! Ekran 1: Oturumlar ve Sağlayıcılar
//! Standart: KERVAN-ARAYUZ-ISTEMI.md & KERVAN-PANO-ANTIGRAVITY-ISTEMI.md
//!
//! Kilim-tema stil ayrımı:
//! - Konumlanma ve sütun oranları: Pano ekran geometrisine aittir
//! - Gölgeli haneler, parşömen zemini ve semantik durum renkleri: `crate::bicim` üzerinden gelir

use eframe::egui::{self, RichText, Ui};
use kervan_pano_veri::{HesapDurumu, Ozet};
use kervan_tani::{TaniKarti, YetenekKarti};

use crate::bicim;

pub fn render(
    ui: &mut Ui,
    ozet: &Ozet,
    secili_hesap_indeks: &mut usize,
    ornek_tani: &Option<TaniKarti>,
    ornek_yetenek: &Option<YetenekKarti>,
) {
    let toplam_genislik = ui.available_width();
    let toplam_yukseklik = ui.available_height();

    // Geniş ekranda iki sütunlu bölünmüş düzen; dar ekranda tek sütunlu dikey akış
    let iki_sutun = toplam_genislik >= 760.0;

    if iki_sutun {
        let sol_genislik = (toplam_genislik * 0.40).clamp(320.0, 460.0);

        ui.horizontal(|ui| {
            // ─── SOL SÜTUN: HESAP LİSTESİ ───
            ui.allocate_ui_with_layout(
                egui::vec2(sol_genislik, toplam_yukseklik),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    bicim::golgeli_hane(ui).show(ui, |ui| {
                        render_hesap_listesi(ui, ozet, secili_hesap_indeks);
                    });
                },
            );

            ui.add_space(8.0);

            // ─── SAĞ SÜTUN: HESAP AYRINTISI VE TANI/YETENEK KARTLARI ───
            let sag_genislik = ui.available_width();
            ui.allocate_ui_with_layout(
                egui::vec2(sag_genislik, toplam_yukseklik),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    bicim::golgeli_hane(ui).show(ui, |ui| {
                        render_hesap_ayrintisi(
                            ui,
                            ozet,
                            secili_hesap_indeks,
                            ornek_tani,
                            ornek_yetenek,
                        );
                    });
                },
            );
        });
    } else {
        // Dar ekran modu: Dikey kaydırılabilir tek sütun
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                bicim::golgeli_hane(ui).show(ui, |ui| {
                    render_hesap_listesi(ui, ozet, secili_hesap_indeks);
                });

                ui.add_space(10.0);

                bicim::golgeli_hane(ui).show(ui, |ui| {
                    render_hesap_ayrintisi(
                        ui,
                        ozet,
                        secili_hesap_indeks,
                        ornek_tani,
                        ornek_yetenek,
                    );
                });
            });
    }
}

/// Sol sütun: Hesap listesi ve üst eylem çubuğu
fn render_hesap_listesi(ui: &mut Ui, ozet: &Ozet, secili_hesap_indeks: &mut usize) {
    ui.horizontal(|ui| {
        ui.heading("Oturumlar & Hesaplar");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("+ Hesap Ekle").clicked() {
                // Hesap ekleme diyaloğu
            }
        });
    });
    ui.separator();

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (i, h) in ozet.hesaplar.iter().enumerate() {
                let secili = *secili_hesap_indeks == i;
                let durum_renk = bicim::hesap_durum_rengi(&h.durum, ui);

                let durum_metni = match &h.durum {
                    HesapDurumu::Hazir => format!("● {}", h.ad),
                    HesapDurumu::KotaBekliyor { kalan_dk } => {
                        format!("◐ {} ({} dk ⏳)", h.ad, kalan_dk)
                    }
                    HesapDurumu::GirisGerekli => format!("○ {} [Giriş Gerekli]", h.ad),
                    HesapDurumu::InsanKapisi => format!("⚠ {} [İnsan Kapısı]", h.ad),
                };

                let baslik = RichText::new(durum_metni).color(durum_renk).strong();

                // Seçili hesap vurgulu hane, diğerleri standart gölgeli hane
                let kart_frame = if secili {
                    bicim::vurgulu_hane(ui, bicim::cini_rengi(ui))
                } else {
                    bicim::golgeli_hane(ui)
                };

                kart_frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.vertical(|ui| {
                        // 1. Satır: Hesap Başlığı (Tam genişlik seçilebilir buton)
                        let btn = ui.add_sized(
                            [ui.available_width(), 26.0],
                            egui::Button::new(baslik).selected(secili),
                        );
                        if btn.clicked() {
                            *secili_hesap_indeks = i;
                        }

                        ui.add_space(4.0);

                        // 2. Satır: Sağlayıcılar solda, Aç ve Yenile butonları sağda
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(h.saglayicilar.join(" · "))
                                    .size(11.0)
                                    .color(bicim::soluk_renk(ui)),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let _ = ui.small_button("👁 Aç").clicked();
                                    let _ = ui.small_button("↻ Yenile").clicked();
                                },
                            );
                        });
                    });
                });
                ui.add_space(6.0);
            }
        });
}

/// Sağ sütun: Seçili hesabın ayrıntıları, sayaçları, tanı ve yetenek kartları
fn render_hesap_ayrintisi(
    ui: &mut Ui,
    ozet: &Ozet,
    secili_hesap_indeks: &mut usize,
    ornek_tani: &Option<TaniKarti>,
    ornek_yetenek: &Option<YetenekKarti>,
) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if let Some(secili) = ozet.hesaplar.get(*secili_hesap_indeks) {
                ui.heading(format!("Hesap Ayrıntısı: {}", secili.ad));
                ui.separator();

                // Üst sayaç kutuları (Gölgeli Haneler)
                ui.horizontal_wrapped(|ui| {
                    bicim::golgeli_hane(ui).show(ui, |ui| {
                        ui.set_min_width(180.0);
                        ui.label(RichText::new("Saatlik İstek Tavanı:").size(11.0));
                        if secili.saatlik_tavan == 0 {
                            ui.label(RichText::new("Sınırsız").strong().size(15.0));
                        } else {
                            ui.label(
                                RichText::new(format!("{} istek / saat", secili.saatlik_tavan))
                                    .strong()
                                    .size(15.0)
                                    .color(bicim::cini_rengi(ui)),
                            );
                        }
                    });

                    ui.add_space(8.0);

                    bicim::golgeli_hane(ui).show(ui, |ui| {
                        ui.set_min_width(180.0);
                        ui.label(RichText::new("Kota Penceresi:").size(11.0));
                        ui.label(
                            RichText::new(&secili.kota_penceresi)
                                .strong()
                                .size(15.0)
                                .color(bicim::basari_rengi(ui)),
                        );
                    });
                });

                ui.add_space(12.0);

                // TANI KARTI BÖLÜMÜ (KERVAN-TANI-ISTEMI FR-8)
                bicim::vurgulu_hane(ui, bicim::cini_rengi(ui)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("📋 Son Tanı Kartı (TaniKarti)")
                                .strong()
                                .color(bicim::cini_rengi(ui)),
                        );
                        ui.separator();

                        if let Some(tani) = ornek_tani {
                            ui.horizontal_wrapped(|ui| {
                                ui.label("Sınıf:");
                                ui.label(RichText::new(tani.sinif.etiket()).strong());
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new(format!("(Güven: {:?})", tani.guven))
                                        .color(bicim::soluk_renk(ui)),
                                );
                            });
                            ui.add_space(4.0);

                            for k in &tani.kanit {
                                ui.label(
                                    RichText::new(format!("• [{}] {}", k.tur, k.metin)).size(11.0),
                                );
                            }
                            ui.add_space(6.0);

                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Öneri:").strong());
                                ui.label(&tani.oneri.ayrinti);
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.button(format!("[{}]", tani.oneri.eylem)).clicked() {}
                                    },
                                );
                            });
                        } else {
                            ui.label(RichText::new("Henüz tanı kaydı yok").italics());
                        }
                    });
                });

                ui.add_space(12.0);

                // YETENEK KARTI BÖLÜMÜ (KERVAN-TANI-ISTEMI §7)
                bicim::golgeli_hane(ui).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("⚡ Sağlayıcı Yetenek Kartı (YetenekKarti)").strong(),
                        );
                        ui.separator();

                        if let Some(yet) = ornek_yetenek {
                            for y in &yet.yetenekler {
                                let durum_ikonu = if y.var {
                                    RichText::new("✓").color(bicim::basari_rengi(ui)).strong()
                                } else {
                                    RichText::new("✗").color(bicim::hata_rengi(ui)).strong()
                                };
                                ui.horizontal(|ui| {
                                    ui.label(durum_ikonu);
                                    ui.label(RichText::new(&y.ad).strong());
                                    if let Some(kanit) = &y.kanit {
                                        ui.label(
                                            RichText::new(kanit)
                                                .size(11.0)
                                                .color(bicim::soluk_renk(ui)),
                                        );
                                    }
                                });
                                ui.add_space(2.0);
                            }
                        } else {
                            ui.label(RichText::new("Henüz yetenek taraması yapılmadı").italics());
                        }
                    });
                });
            } else {
                ui.label("Seçili hesap bulunamadı.");
            }
        });
}
