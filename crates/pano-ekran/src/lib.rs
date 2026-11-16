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
    /// Ayarlar ekranının REST anahtarı formu (m02007).
    pub ayar_formu: ekran5_ayarlar::AnahtarFormu,
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
            ayar_formu: ekran5_ayarlar::AnahtarFormu::default(),
        }
    }
}

/// Anahtarı tanımsız REST hedeflerinin ortam değişkeni adları.
///
/// NEDEN AYRI İŞLEV: üst şeritteki uyarının kaç anahtarı saydığı sınanabilsin.
/// Uyarı, `Ozet`'in gerçek ölçümüne (`KimlikDurumu.tanimli`) dayanır; sabit bir
/// sayı yazılmaz — "3 anahtar eksik" gibi ölçülmemiş bir iddia yanlış beyan olurdu.
pub fn eksik_anahtarlar(ozet: &Ozet) -> Vec<&str> {
    ozet.kimlikler
        .iter()
        .filter(|k| !k.tanimli)
        .map(|k| k.ad.as_str())
        .collect()
}

/// Üst şerit.
///
/// m02388: anahtar ekleme yolu yalnız 6. sekmenin ("⚙ Ayar") içindeydi; Canlı
/// ekranda duran kullanıcı ne anahtarın tanımsız olduğunu görüyor ne de ekleme
/// yolunu bulabiliyordu ("seçenek/tuş yok"). Üst şerit her sekmede göründüğü
/// için uyarı buraya kondu; düğme Ayarlar'ı açar ve formun hedefini eksik
/// anahtara önceden ayarlar — kullanıcı ikinci bir seçim yapmak zorunda kalmaz.
pub fn render_ust_serit(
    ui: &mut Ui,
    ozet: &Ozet,
    tema_tercihi: &mut TemaTercihi,
    durum: &mut PanoEkraniDurumu,
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

        // m02388: eksik anahtar uyarısı HER sekmede görünür ve yanında doğrudan
        // ekleme yolunu açar. Ad listesi gerçek ölçümden gelir; hiçbir şey
        // tanımsız değilse uyarı çizilmez (yanlış alarm yok).
        let eksik = eksik_anahtarlar(ozet);
        if !eksik.is_empty() {
            ui.add_space(12.0);
            let uyari = Color32::from_rgb(192, 79, 66);
            let adlar = eksik.join(", ");
            ui.label(
                RichText::new(format!("⚠ API anahtarı tanımsız: {adlar}"))
                    .color(uyari)
                    .size(11.5),
            )
            .on_hover_text(
                "Bu hedefler çağrıldığında istek 401/404 ile düşer. \
                 Anahtar yalnız bu oturumun belleğinde tutulur, diske yazılmaz.",
            );
            if ui
                .add(egui::Button::new(
                    RichText::new("Anahtar ekle →").size(11.5),
                ))
                .on_hover_text("Ayarlar sekmesini açar ve hedefi bu anahtara ayarlar.")
                .clicked()
            {
                durum.aktif_sekme = PanoSekmesi::Ayarlar;
                // Hedefi önceden seç: kullanıcı listeden ikinci kez aramasın.
                // `anahtar_formu` seçili hedef listede yoksa ilkine düşer
                // (ekran5_ayarlar::hedef_sec), yani bayat değer zararsızdır.
                durum.ayar_formu.secili_env = eksik[0].to_string();
            }
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
            // m02388: etiket yalnız "⚙ Ayar" idi; REST anahtarının orada
            // olduğunu sezdirmiyordu. Sekme adı iki işi de söyler.
            (PanoSekmesi::Ayarlar, "⚙ Ayar / Anahtar"),
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
            ekran5_ayarlar::render(ui, ozet, tema_tercihi, &mut durum.ayar_formu);
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use kervan_pano_veri::{KimlikDurumu, Ozet};

    fn ozet(kimlikler: Vec<(&str, bool, bool)>) -> Ozet {
        Ozet {
            kimlikler: kimlikler
                .into_iter()
                .map(|(ad, tanimli, oturumda)| KimlikDurumu {
                    ad: ad.to_string(),
                    tanimli,
                    oturumda,
                })
                .collect(),
            ..Ozet::default()
        }
    }

    /// m02388'in çekirdeği: tanımsız anahtar üst şeritte ADIYLA anılmalı.
    #[test]
    fn tanimsiz_anahtar_ust_seritte_gorunur() {
        let o = ozet(vec![
            ("OPENROUTER_API_KEY", false, false),
            ("GEMINI_API_KEY", true, false),
        ]);
        assert_eq!(eksik_anahtarlar(&o), vec!["OPENROUTER_API_KEY"]);
    }

    /// Yanlış alarm: tümü tanımlıysa uyarı çizilmemeli.
    #[test]
    fn hepsi_tanimliysa_uyari_yok() {
        let o = ozet(vec![("OPENROUTER_API_KEY", true, false)]);
        assert!(eksik_anahtarlar(&o).is_empty());
    }

    /// Oturumda girilmiş anahtar "tanımlı" sayılır (kasa çözüyor) → uyarı düşer.
    /// `tanimli` alanı `anahtar_coz`un ölçümüdür, `oturumda` yalnız kaynağı
    /// gösterir; ikisi ayrı kavram ve testte ayrı tutuluyor.
    #[test]
    fn oturumda_girilen_anahtar_uyariyi_dusurur() {
        let o = ozet(vec![("OPENROUTER_API_KEY", true, true)]);
        assert!(eksik_anahtarlar(&o).is_empty());
    }

    /// Tanımsızların TAMAMI listelenir; ilkini seçmek diğerlerini gizlemez.
    #[test]
    fn birden_fazla_eksik_hepsi_sayilir() {
        let o = ozet(vec![
            ("A_KEY", false, false),
            ("B_KEY", false, false),
            ("C_KEY", true, false),
        ]);
        assert_eq!(eksik_anahtarlar(&o), vec!["A_KEY", "B_KEY"]);
    }
}
