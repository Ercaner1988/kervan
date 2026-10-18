//! # kervan-pano-ekran
//! Kervan sade pano 5 ekran modülleri ve egui çizim işlevleri.
//! Standart: KERVAN-PANO-ANTIGRAVITY-ISTEMI.md & KERVAN-ARAYUZ-ISTEMI.md

pub mod bicim;
pub mod ekran1_oturum;
pub mod ekran2_rota;
pub mod ekran3_izleme;
pub mod ekran4_baglam;
pub mod ekran5_ayarlar;
pub mod esleme;

use eframe::egui::{self, Color32, RichText, Ui};
use kervan_pano_veri::Ozet;
use kervan_tani::{Kanit, Oneri, Sinif, TaniKarti, Yetenek, YetenekKarti};
use kilim_tema::TemaTercihi;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PanoSekmesi {
    #[default]
    Oturumlar = 1,
    Rota = 2,
    Izleme = 3,
    Baglam = 4,
    Ayarlar = 5,
}

pub struct PanoEkraniDurumu {
    pub aktif_sekme: PanoSekmesi,
    pub secili_hesap_indeks: usize,
    pub test_istemi: String,
    pub test_yaniti: String,
    pub ornek_tani: Option<TaniKarti>,
    pub ornek_yetenek: Option<YetenekKarti>,
}

impl Default for PanoEkraniDurumu {
    fn default() -> Self {
        Self {
            aktif_sekme: PanoSekmesi::Oturumlar,
            secili_hesap_indeks: 0,
            test_istemi: "Kervan test istemi".to_string(),
            test_yaniti: String::new(),
            ornek_tani: Some(TaniKarti {
                zaman: "2026-10-09 15:42".to_string(),
                hesap: "ercan-gemini-ana".to_string(),
                saglayici: "gemini".to_string(),
                tetik: "elle".to_string(),
                sinif: Sinif::OturumDusto,
                guven: kervan_tani::Guven::Kural,
                kanit: vec![
                    Kanit {
                        tur: "dom".to_string(),
                        metin: "prompt_input: 1 eşleşme".to_string(),
                    },
                    Kanit {
                        tur: "url".to_string(),
                        metin: "https://gemini.google.com/app".to_string(),
                    },
                ],
                goruntu: None,
                oneri: Oneri {
                    eylem: "giris".to_string(),
                    ayrinti: "[Giriş] akışı hazır".to_string(),
                },
            }),
            ornek_yetenek: Some(YetenekKarti {
                saglayici: "gemini".to_string(),
                zaman: "2026-10-09 15:40".to_string(),
                yetenekler: vec![
                    Yetenek {
                        ad: "Model Seçici".to_string(),
                        var: true,
                        kanit: Some("button[aria-label*=model]".to_string()),
                        degerler: vec!["2.5 Pro".to_string(), "Flash".to_string()],
                    },
                    Yetenek {
                        ad: "Dosya Yükleme".to_string(),
                        var: true,
                        kanit: Some("input[type=file]".to_string()),
                        degerler: vec![],
                    },
                    Yetenek {
                        ad: "Görsel Üretimi".to_string(),
                        var: true,
                        kanit: Some("Imagen 3".to_string()),
                        degerler: vec![],
                    },
                    Yetenek {
                        ad: "Derin Araştırma".to_string(),
                        var: false,
                        kanit: None,
                        degerler: vec![],
                    },
                ],
            }),
        }
    }
}

pub fn render_ust_serit(
    ui: &mut Ui,
    ozet: &Ozet,
    tema_tercihi: &mut TemaTercihi,
    _durum: &mut PanoEkraniDurumu,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Kervan").strong().size(15.0));
        ui.label(RichText::new(&ozet.baglanti_adresi).monospace().size(12.0));

        let durum_rengi = if ozet.sunucu_acik {
            Color32::from_rgb(46, 125, 79)
        } else {
            Color32::from_rgb(192, 79, 66)
        };
        ui.label(RichText::new("● açık").color(durum_rengi).strong().size(12.0));

        ui.add_space(12.0);
        ui.label(RichText::new("strateji:").size(11.0).color(Color32::GRAY));
        ui.label(RichText::new(&ozet.aktif_strateji).strong().size(11.0));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("⚙").clicked() {}
            kilim_tema::tema_secici(ui, tema_tercihi);
            if ui.button("🔍 Tanı al").clicked() {}
        });
    });
}

pub fn render_sol_nav(ui: &mut Ui, durum: &mut PanoEkraniDurumu) {
    ui.vertical(|ui| {
        ui.add_space(4.0);

        let sekmeler = [
            (PanoSekmesi::Oturumlar, "🔑 Oturum"),
            (PanoSekmesi::Rota, "🧭 Rota"),
            (PanoSekmesi::Izleme, "📊 İzleme"),
            (PanoSekmesi::Baglam, "🧠 Bağlam"),
            (PanoSekmesi::Ayarlar, "⚙ Ayar"),
        ];

        let genislik = ui.available_width();
        for (sekme, etiket) in sekmeler {
            let secili = durum.aktif_sekme == sekme;
            let btn = ui.add_sized(
                [genislik, 32.0],
                egui::Button::new(RichText::new(etiket).strong().size(12.5)).selected(secili),
            );
            if btn.clicked() {
                durum.aktif_sekme = sekme;
            }
            ui.add_space(4.0);
        }
    });
}

pub fn render_alt_serit(ui: &mut Ui, ozet: &Ozet) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Canlı Akış:").strong().size(11.0));
        if ozet.canli_akis.is_empty() {
            ui.label(RichText::new("Henüz istek yok").size(11.0).color(Color32::GRAY));
        } else {
            for req in &ozet.canli_akis {
                let renk = if req.basarili {
                    Color32::from_rgb(46, 125, 79)
                } else {
                    Color32::from_rgb(217, 164, 65)
                };
                ui.label(
                    RichText::new(format!("● {} ({} ms)", req.model, req.sure_ms))
                        .color(renk)
                        .size(11.0),
                );
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new("Boşta CPU: %0.0").size(11.0).color(Color32::GRAY));
        });
    });
}

pub fn render_ana_ekran(
    ui: &mut Ui,
    durum: &mut PanoEkraniDurumu,
    ozet: &Ozet,
    tema_tercihi: &mut TemaTercihi,
) {
    match durum.aktif_sekme {
        PanoSekmesi::Oturumlar => {
            ekran1_oturum::render(
                ui,
                ozet,
                &mut durum.secili_hesap_indeks,
                &durum.ornek_tani,
                &durum.ornek_yetenek,
            );
        }
        PanoSekmesi::Rota => {
            ekran2_rota::render(ui, ozet, &mut durum.test_istemi, &mut durum.test_yaniti);
        }
        PanoSekmesi::Izleme => {
            ekran3_izleme::render(ui, ozet);
        }
        PanoSekmesi::Baglam => {
            ekran4_baglam::render(ui, ozet);
        }
        PanoSekmesi::Ayarlar => {
            ekran5_ayarlar::render(ui, ozet, tema_tercihi);
        }
    }
}
