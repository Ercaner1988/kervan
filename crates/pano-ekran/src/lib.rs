//! # kervan-pano-ekran
//! Kervan sade pano 5 ekran modülleri ve egui çizim işlevleri.
//! Standart: KERVAN-PANO-ANTIGRAVITY-ISTEMI.md & KERVAN-ARAYUZ-ISTEMI.md

pub mod bicim;
pub mod ekran1_oturum;
pub mod ekran2_rota;
pub mod ekran3_izleme;
pub mod ekran4_baglam;
pub mod ekran5_ayarlar;
pub mod ekran6_canli;
pub mod esleme;

use eframe::egui::{self, Color32, RichText, Ui};
use kervan_pano_veri::{CanliDurum, GonderimDurumu, Ozet};
use kervan_tani::{TaniKarti, YetenekKarti};
use kilim_tema::TemaTercihi;

pub use ekran6_canli::CanliSecim;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PanoSekmesi {
    /// Hesaplar + modeller + özellikler tek ekranda (canlı veri).
    #[default]
    Canli = 0,
    Oturumlar = 1,
    Rota = 2,
    Izleme = 3,
    Baglam = 4,
    Ayarlar = 5,
}

pub struct PanoEkraniDurumu {
    pub aktif_sekme: PanoSekmesi,
    pub secili_hesap_indeks: usize,
    pub ornek_tani: Option<TaniKarti>,
    pub ornek_yetenek: Option<YetenekKarti>,
    /// Canlı ekranın hesap + model + araç seçimi.
    pub canli_secim: CanliSecim,
}

impl Default for PanoEkraniDurumu {
    fn default() -> Self {
        Self {
            aktif_sekme: PanoSekmesi::Canli,
            secili_hesap_indeks: 0,
            // Örnek tanı/yetenek kartları KALDIRILDI: içlerinde gerçek olmayan
            // hesap adı ve ölçülmemiş yetenekler vardı. Tanı kartı ancak gerçek
            // bir taramadan sonra doldurulur.
            ornek_tani: None,
            ornek_yetenek: None,
            canli_secim: CanliSecim::default(),
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
        ui.label(
            RichText::new("● açık")
                .color(durum_rengi)
                .strong()
                .size(12.0),
        );

        if !ozet.aktif_strateji.is_empty() {
            ui.add_space(12.0);
            ui.label(RichText::new("strateji:").size(11.0).color(Color32::GRAY));
            ui.label(RichText::new(&ozet.aktif_strateji).strong().size(11.0));
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            kilim_tema::tema_secici(ui, tema_tercihi);
            // G7: düğmeler önceden `let _ = ui.button(...).clicked();` idi —
            // tıklanabiliyor ama hiçbir şey yapmıyordu. Bağlanana kadar kapalı
            // ve nedeni yazılı.
            ui.add_enabled(false, egui::Button::new("🔍 Tanı al"))
                .on_disabled_hover_text("Tanı taraması henüz panele bağlı değil (DALGA-2).");
        });
    });
}

pub fn render_sol_nav(ui: &mut Ui, durum: &mut PanoEkraniDurumu) {
    ui.vertical(|ui| {
        ui.add_space(4.0);

        let sekmeler = [
            (PanoSekmesi::Canli, "◉ Canlı"),
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
            ui.label(
                RichText::new("Henüz istek yok")
                    .size(11.0)
                    .color(Color32::GRAY),
            );
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
            // G8: burada önceden sabit "Boşta CPU: %0.0" yazıyordu — hiç
            // ölçülmemiş bir sayıyı ölçüm gibi göstermek yanlış beyandı.
            // Yerine gerçek telemetri sayacı kondu.
            ui.label(
                RichText::new(format!(
                    "oturum isteği: {} · başarılı: {}",
                    ozet.telemetri.toplam_istek, ozet.telemetri.basarili_istek
                ))
                .size(11.0)
                .color(Color32::GRAY),
            );
        });
    });
}

pub fn render_ana_ekran(
    ui: &mut Ui,
    durum: &mut PanoEkraniDurumu,
    ozet: &Ozet,
    tema_tercihi: &mut TemaTercihi,
    canli: &CanliDurum,
    gonderim: &GonderimDurumu,
) -> bool {
    match durum.aktif_sekme {
        PanoSekmesi::Canli => {
            return ekran6_canli::render(ui, canli, &mut durum.canli_secim, gonderim);
        }
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
            return ekran2_rota::render(ui, ozet, &mut durum.canli_secim, gonderim);
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
    false
}
