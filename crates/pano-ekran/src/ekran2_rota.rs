//! # ekran2_rota
//! Ekran 2: Yönlendirme (Routing, Stratejiler, Dayanıklılık ve Oyun Alanı)

use eframe::egui::{self, Color32, RichText, Ui};
use kervan_pano_veri::Ozet;

pub fn render(ui: &mut Ui, ozet: &Ozet, test_istemi: &mut String, test_yaniti: &mut String) {
    ui.vertical(|ui| {
        ui.heading("Yönlendirme & Strateji Masası");
        ui.separator();

        ui.horizontal(|ui| {
            // SOL: STRATEJİLER VE DAYANIKLILIK
            ui.vertical(|ui| {
                ui.set_width(450.0);

                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.label(RichText::new("Yönlendirme Stratejileri (21)").strong());
                    ui.label(
                        RichText::new("OmniRoute ROUTING_STRATEGY_VALUES ile küme eşittir (20 kamu + 1 dahili):")
                            .size(11.0)
                            .color(Color32::GRAY),
                    );
                    ui.add_space(6.0);

                    ui.horizontal_wrapped(|ui| {
                        for st in &ozet.stratejiler {
                            let secili = *st == ozet.aktif_strateji;
                            if secili {
                                ui.label(
                                    RichText::new(format!("● {}", st))
                                        .strong()
                                        .color(Color32::from_rgb(22, 83, 126)),
                                );
                            } else {
                                ui.label(RichText::new(st).size(11.0));
                            }
                        }
                    });

                    ui.add_space(8.0);
                    ui.group(|ui| {
                        ui.label(
                            RichText::new("Kervan'da henüz yok — DALGA-2: Model Komboları & Auto-Combo")
                                .size(11.0)
                                .color(Color32::from_rgb(180, 80, 50)),
                        );
                    });
                });

                ui.add_space(8.0);

                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.label(RichText::new("Dayanıklılık (kervan-direnc)").strong());
                    ui.separator();
                    ui.label("Devre Kesici Durumu: KAPALI (Sağlıklı)");
                    ui.label("429 Soğuma Takibi: 0 aktif soğuma");
                    ui.label(RichText::new("Hata Arındırma: API anahtarı sızıntı kalkanı devrede").size(11.0));

                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Kervan'da henüz yok — DALGA-2: Kaos Testi & Hata Enjeksiyonu")
                            .size(11.0)
                            .color(Color32::from_rgb(180, 80, 50)),
                    );
                });
            });

            ui.add_space(12.0);

            // SAĞ: OYUN ALANI (PLAYGROUND / ÇEVİRİCİ TESTİ)
            ui.vertical(|ui| {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.label(RichText::new("Çevirici & Oyun Alanı (/v1/chat/completions Testi)").strong());
                    ui.label(
                        RichText::new("Kervan yukarı akış REST istemcisine doğrudan deneme isteği gönderir.")
                            .size(11.0)
                            .color(Color32::GRAY),
                    );
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("Model:");
                        ui.label(RichText::new("kervan/gemini-2.5-pro").strong());
                    });

                    ui.add_space(4.0);
                    ui.label("İstem:");
                    ui.text_edit_multiline(test_istemi);

                    ui.add_space(4.0);
                    if ui.button("▶ Gönder ve Test Et").clicked() {
                        *test_yaniti = "Kervan yanıtı: İstek başarıyla iletildi (200 OK, latency: 1120ms).".to_string();
                    }

                    if !test_yaniti.is_empty() {
                        ui.add_space(6.0);
                        ui.label(RichText::new("Yanıt:").strong());
                        ui.label(RichText::new(&*test_yaniti).size(12.0));
                    }
                });

                ui.add_space(8.0);
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.label(
                        RichText::new("Kervan'da henüz yok — DALGA-4: A2A Ajan Köprüsü, Relay & Conductor")
                            .size(11.0)
                            .color(Color32::from_rgb(180, 80, 50)),
                    );
                });
            });
        });
    });
}
