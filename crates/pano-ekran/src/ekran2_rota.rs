//! # ekran2_rota
//! Ekran 2: Yönlendirme (Routing, Stratejiler, Dayanıklılık ve Oyun Alanı)
//!
//! Oyun alanı GERÇEK isteği Canlı ekranla aynı kanaldan gönderir: düğme
//! `true` döndürür, çağıran `CanliDepo::gonder` çağırır ve sonucu
//! `GonderimDurumu` üzerinden geri verir. Burada üretilen hiçbir metin
//! ölçüm taklidi değildir (G1).

use eframe::egui::{self, Color32, RichText, Ui};
use kervan_pano_veri::{GonderimDurumu, Ozet};

use crate::ekran6_canli::{CanliSecim, model_kimligi};

pub fn render(ui: &mut Ui, ozet: &Ozet, secim: &mut CanliSecim, gonderim: &GonderimDurumu) -> bool {
    let mut gonder = false;
    ui.vertical(|ui| {
        ui.heading("Yönlendirme & Strateji Masası");
        ui.separator();

        ui.horizontal(|ui| {
            // SOL: STRATEJİLER VE DAYANIKLILIK
            ui.vertical(|ui| {
                ui.set_width(450.0);

                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.label(
                        RichText::new(format!("Yönlendirme Stratejileri ({})", ozet.stratejiler.len()))
                            .strong(),
                    );
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
                    // G6: bu üç satır önceden sabit literaldi ("KAPALI (Sağlıklı)"),
                    // yani ölçülmemiş bir durumu doğrulanmış gibi gösteriyordu.
                    // Router'ın devre kesici durumu henüz panele taşınmadı;
                    // ölçülmeyen şeyi "sağlıklı" diye yazmak yerine açıkça yaz.
                    ui.label(
                        RichText::new("Devre kesici, 429 soğuma ve arındırma göstergeleri bu sürümde panele bağlı değil.")
                            .size(11.0)
                            .color(Color32::GRAY),
                    );

                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Kervan'da henüz yok — DALGA-2: Kaos Testi & Hata Enjeksiyonu")
                            .size(11.0)
                            .color(Color32::from_rgb(180, 80, 50)),
                    );
                });
            });

            ui.add_space(12.0);

            // SAĞ: OYUN ALANI (PLAYGROUND) — gerçek istek kanalı
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
                        let etiket = if secim.saglayici.is_empty() {
                            "sunucu seçer".to_string()
                        } else {
                            model_kimligi(secim)
                        };
                        ui.label(RichText::new(etiket).strong());
                    });
                    ui.horizontal(|ui| {
                        ui.label("Hesap:");
                        let etiket = secim
                            .hesap
                            .clone()
                            .filter(|h| !h.is_empty())
                            .unwrap_or_else(|| "en az kullanılan (otomatik)".to_string());
                        ui.label(RichText::new(etiket).size(12.0));
                    });
                    ui.label(
                        RichText::new("Hesap ve model seçimi Canlı ekrandan yapılır.")
                            .size(10.5)
                            .color(Color32::GRAY),
                    );

                    ui.add_space(4.0);
                    ui.label("İstem:");
                    ui.add_enabled_ui(!gonderim.calisiyor, |ui| {
                        ui.text_edit_multiline(&mut secim.istem);
                    });

                    ui.add_space(4.0);
                    let hazir = !gonderim.calisiyor && !secim.istem.trim().is_empty();
                    if ui
                        .add_enabled(hazir, egui::Button::new("▶ Gönder ve Test Et"))
                        .clicked()
                    {
                        gonder = true;
                    }
                    if gonderim.calisiyor {
                        ui.add(egui::Spinner::new().size(14.0));
                    }

                    if let Some(hata) = &gonderim.hata {
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(format!("Hata: {hata}"))
                                .size(12.0)
                                .color(Color32::from_rgb(192, 79, 66)),
                        );
                    } else if !gonderim.yanit.is_empty() {
                        ui.add_space(6.0);
                        ui.label(RichText::new("Yanıt:").strong());
                        ui.label(
                            RichText::new(format!(
                                "{} · {} · {} ms",
                                gonderim.model, gonderim.hesap, gonderim.sure_ms
                            ))
                            .size(10.5)
                            .color(Color32::GRAY),
                        );
                        egui::ScrollArea::vertical()
                            .id_salt("rota_yanit")
                            .max_height(220.0)
                            .show(ui, |ui| {
                                ui.label(RichText::new(&gonderim.yanit).size(12.0));
                            });
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
    gonder
}
