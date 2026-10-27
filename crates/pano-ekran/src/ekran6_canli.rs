//! Canlı pano: bağlı hesaplar, modeller ve modellerin ölçülmüş aç/kapa
//! özellikleri TEK ekranda; üçü de aynı anda seçilebilir.
//!
//! NEDEN AYRI EKRAN: eski beş ekran sağlayıcı merkezliydi ve hesap seçimi ile
//! model/özellik seçimi farklı yerlere dağılmıştı. Kullanıcı "hangi hesap +
//! hangi model + hangi araç" üçlüsünü tek bakışta kurup AYNI istekle sınamak
//! istiyor. Ekranın çizdiği her ad kervan.toml'un ölçülmüş içeriğinden gelir;
//! burada uydurulmuş model/özellik adı yoktur.
//!
//! YERLEŞİM NOTU: `ui.columns(3)` ilk denemede sütunları pencere dışına taşırdı
//! (içerik sütun payından geniş isteyince egui sütunu büyütüyor). Üç sütun bu
//! yüzden `allocate_ui_with_layout` ile AÇIK genişlikle çizilir.

use eframe::egui::{self, Align, Color32, Layout, RichText, Ui};
use kervan_pano_veri::{CanliDurum, CanliHesap, CanliSaglayici, GonderimDurumu};

/// Panelin o anki seçimi. Gönder düğmesi bu üçlüyü tek isteğe çevirir.
#[derive(Debug, Clone, Default)]
pub struct CanliSecim {
    /// `gemini:profil` — istekte `kervan.hesap` olarak gider.
    pub hesap: Option<String>,
    pub saglayici: String,
    /// `None` = site varsayılanı.
    pub model_kodu: Option<String>,
    pub anahtarlar: std::collections::BTreeMap<String, bool>,
    pub istem: String,
}

const KOLON_BOSLUK: f32 = 14.0;
const KOLON_YUKSEKLIK: f32 = 330.0;

/// Hesap seçimi değiştiğinde sağlayıcıya bağlı seçimleri tazeler.
fn hesap_sec(secim: &mut CanliSecim, hesap: &CanliHesap) {
    if secim.saglayici != hesap.saglayici {
        secim.model_kodu = None;
        secim.anahtarlar.clear();
    }
    secim.hesap = Some(hesap.anahtar.clone());
    secim.saglayici = hesap.saglayici.clone();
}

fn saglayici_bul<'a>(durum: &'a CanliDurum, ad: &str) -> Option<&'a CanliSaglayici> {
    durum.saglayicilar.iter().find(|s| s.ad == ad)
}

/// İsteğe gidecek model kimliği: `kervan/<saglayici>[/<kod>]`.
pub fn model_kimligi(secim: &CanliSecim) -> String {
    match secim.model_kodu.as_deref() {
        Some(kod) if !kod.is_empty() => format!("kervan/{}/{}", secim.saglayici, kod),
        _ => format!("kervan/{}", secim.saglayici),
    }
}

fn ust_bilgi(ui: &mut Ui, durum: &CanliDurum) {
    let bagli = durum.hesaplar.iter().filter(|h| h.bagli).count();
    let havuz = durum.hesaplar.len() - bagli;
    ui.horizontal(|ui| {
        ui.label(RichText::new("Canlı pano").strong().size(15.0));
        ui.label(
            RichText::new(format!("{bagli} bağlı eklenti hesabı"))
                .size(11.5)
                .color(Color32::from_rgb(46, 125, 79)),
        );
        if havuz > 0 {
            ui.label(
                RichText::new(format!("· {havuz} havuz yuvası (seçilemez)"))
                    .size(11.0)
                    .color(Color32::GRAY),
            );
        }
        if let Some(hata) = &durum.hata {
            ui.label(
                RichText::new(format!("yoklama hatası: {hata}"))
                    .size(11.0)
                    .color(Color32::from_rgb(192, 79, 66)),
            );
        }
    });
}

fn hesaplar_kolon(ui: &mut Ui, durum: &CanliDurum, secim: &mut CanliSecim) {
    ui.label(RichText::new("Hesaplar").strong().size(12.5));
    ui.label(
        RichText::new("eklenti kimliği = profil adı")
            .size(10.5)
            .color(Color32::GRAY),
    );
    ui.add_space(4.0);
    egui::ScrollArea::vertical()
        .id_salt("canli_hesaplar")
        .max_height(KOLON_YUKSEKLIK)
        .show(ui, |ui| {
            let bagli: Vec<&CanliHesap> = durum.hesaplar.iter().filter(|h| h.bagli).collect();
            if bagli.is_empty() {
                ui.label(
                    RichText::new("Bağlı hesap yok. Tarayıcıda eklenti sekmesi açık mı?")
                        .size(11.0)
                        .color(Color32::GRAY),
                );
            }
            for hesap in bagli {
                let secili = secim.hesap.as_deref() == Some(hesap.anahtar.as_str());
                let dugme = egui::Button::new(
                    RichText::new(&hesap.profil)
                        .size(12.0)
                        .color(Color32::from_rgb(46, 125, 79)),
                )
                .selected(secili)
                .min_size(egui::vec2(ui.available_width(), 24.0));
                if ui.add(dugme).clicked() {
                    hesap_sec(secim, hesap);
                }
                ui.label(
                    RichText::new(format!(
                        "{} · {}/{} istek · son saat {}",
                        hesap.saglayici, hesap.istek_sayisi, hesap.saatlik_tavan, hesap.son_saatte
                    ))
                    .size(10.0)
                    .color(Color32::GRAY),
                );
                if let Some(neden) = &hesap.duraklatildi {
                    ui.label(
                        RichText::new(format!("duraklatıldı: {neden}"))
                            .size(10.0)
                            .color(Color32::from_rgb(217, 164, 65)),
                    );
                }
                ui.add_space(3.0);
            }
        });
}

fn modeller_kolon(ui: &mut Ui, durum: &CanliDurum, secim: &mut CanliSecim) {
    ui.label(RichText::new("Modeller").strong().size(12.5));
    ui.label(
        RichText::new(if secim.saglayici.is_empty() {
            "önce hesap seç".to_string()
        } else {
            format!("sağlayıcı: {}", secim.saglayici)
        })
        .size(10.5)
        .color(Color32::GRAY),
    );
    ui.add_space(4.0);
    egui::ScrollArea::vertical()
        .id_salt("canli_modeller")
        .max_height(KOLON_YUKSEKLIK)
        .show(ui, |ui| {
            let Some(sag) = saglayici_bul(durum, &secim.saglayici) else {
                if !secim.saglayici.is_empty() {
                    ui.label(
                        RichText::new(
                            "Bu sağlayıcı için kervan.toml'da model menüsü tanımlı değil.",
                        )
                        .size(11.0)
                        .color(Color32::GRAY),
                    );
                }
                return;
            };
            if ui
                .add(
                    egui::Button::new("site varsayılanı")
                        .selected(secim.model_kodu.is_none())
                        .min_size(egui::vec2(ui.available_width(), 24.0)),
                )
                .clicked()
            {
                secim.model_kodu = None;
            }
            ui.add_space(3.0);
            for model in &sag.modeller {
                let kod = model.kod.clone().unwrap_or_default();
                let secili = secim.model_kodu.as_deref() == Some(kod.as_str());
                let etiket = if model.kilitli {
                    format!("🔒 {}", model.etiket)
                } else {
                    model.etiket.clone()
                };
                let dugme = egui::Button::new(RichText::new(etiket).size(12.0))
                    .selected(secili)
                    .min_size(egui::vec2(ui.available_width(), 24.0));
                if ui.add_enabled(!model.kilitli, dugme).clicked() {
                    secim.model_kodu = Some(kod.clone());
                }
                if model.kilitli {
                    ui.label(
                        RichText::new(format!(
                            "  {}",
                            model.kilit_nedeni.as_deref().unwrap_or("kilitli")
                        ))
                        .size(10.0)
                        .color(Color32::from_rgb(217, 164, 65)),
                    );
                }
                ui.add_space(2.0);
            }
            ui.add_space(6.0);
            ui.label(
                RichText::new(format!("kimlik: {}", model_kimligi(secim)))
                    .size(10.0)
                    .monospace()
                    .color(Color32::GRAY),
            );
        });
}

fn anahtarlar_kolon(ui: &mut Ui, durum: &CanliDurum, secim: &mut CanliSecim) {
    ui.label(RichText::new("Özellik / araç").strong().size(12.5));
    ui.label(
        RichText::new("sitedeki aç/kapa düğmeleri (ölçülmüş)")
            .size(10.5)
            .color(Color32::GRAY),
    );
    ui.add_space(4.0);
    egui::ScrollArea::vertical()
        .id_salt("canli_anahtarlar")
        .max_height(KOLON_YUKSEKLIK)
        .show(ui, |ui| {
            let anahtarlar = saglayici_bul(durum, &secim.saglayici)
                .map(|s| s.anahtarlar.as_slice())
                .unwrap_or_default();
            if anahtarlar.is_empty() {
                ui.label(
                    RichText::new(if secim.saglayici.is_empty() {
                        "önce hesap seç"
                    } else {
                        "kervan.toml'da bu sağlayıcı için ölçülmüş aç/kapa özelliği yok."
                    })
                    .size(11.0)
                    .color(Color32::GRAY),
                );
            }
            for anahtar in anahtarlar {
                let mut acik = secim.anahtarlar.get(&anahtar.kod).copied().unwrap_or(false);
                if ui
                    .checkbox(&mut acik, RichText::new(&anahtar.etiket).size(12.0))
                    .changed()
                {
                    secim.anahtarlar.insert(anahtar.kod.clone(), acik);
                }
                ui.label(
                    RichText::new(format!("  {}", anahtar.kod))
                        .size(10.0)
                        .monospace()
                        .color(Color32::GRAY),
                );
            }
        });
}

pub fn render(
    ui: &mut Ui,
    durum: &CanliDurum,
    secim: &mut CanliSecim,
    gonderim: &GonderimDurumu,
) -> bool {
    ust_bilgi(ui, durum);
    ui.separator();

    let kolon = ((ui.available_width() - 2.0 * KOLON_BOSLUK) / 3.0).max(160.0);
    ui.horizontal_top(|ui| {
        let kolon_ciz = |ui: &mut Ui, ciz: &mut dyn FnMut(&mut Ui)| {
            ui.allocate_ui_with_layout(
                egui::vec2(kolon, KOLON_YUKSEKLIK + 40.0),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.set_max_width(kolon);
                    ciz(ui);
                },
            );
        };
        kolon_ciz(ui, &mut |ui| hesaplar_kolon(ui, durum, secim));
        ui.add_space(KOLON_BOSLUK);
        kolon_ciz(ui, &mut |ui| modeller_kolon(ui, durum, secim));
        ui.add_space(KOLON_BOSLUK);
        kolon_ciz(ui, &mut |ui| anahtarlar_kolon(ui, durum, secim));
    });

    ui.add_space(4.0);
    ui.separator();

    // ── SEÇİM ÖZETİ + İSTEM + GÖNDER ─────────────────────────────────────────
    let hesap_etiketi = secim
        .hesap
        .clone()
        .unwrap_or_else(|| "hesap seçilmedi".to_string());
    let arac_sayisi = secim.anahtarlar.values().filter(|v| **v).count();
    ui.label(
        RichText::new(format!(
            "seçili: {} × {} · açık araç: {}",
            hesap_etiketi,
            model_kimligi(secim),
            arac_sayisi
        ))
        .size(11.5)
        .monospace(),
    );

    ui.add_space(2.0);
    ui.label(RichText::new("İstem").size(11.0).color(Color32::GRAY));
    ui.add(
        egui::TextEdit::singleline(&mut secim.istem)
            .desired_width(f32::INFINITY)
            .hint_text("Tek satır sınama istemi"),
    );
    ui.add_space(3.0);

    let hazir = secim.hesap.is_some() && !gonderim.calisiyor && !secim.istem.trim().is_empty();
    let mut gonder = false;
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(hazir, egui::Button::new("Gönder (canlı)"))
            .clicked()
        {
            gonder = true;
        }
        if gonderim.calisiyor {
            ui.spinner();
            ui.label(RichText::new("yanıt bekleniyor…").size(11.0));
        } else if !gonderim.model.is_empty() {
            ui.label(
                RichText::new(format!(
                    "son istek: {} · {} · {} ms",
                    gonderim.hesap, gonderim.model, gonderim.sure_ms
                ))
                .size(10.5)
                .color(Color32::GRAY),
            );
        }
        if secim.hesap.is_none() {
            ui.label(
                RichText::new("göndermek için hesap seç")
                    .size(10.5)
                    .color(Color32::GRAY),
            );
        }
    });

    if let Some(hata) = &gonderim.hata {
        ui.label(
            RichText::new(format!("hata: {hata}"))
                .size(11.0)
                .color(Color32::from_rgb(192, 79, 66)),
        );
    }
    if !gonderim.yanit.is_empty() {
        egui::ScrollArea::vertical()
            .id_salt("canli_yanit")
            .max_height(150.0)
            .show(ui, |ui| {
                ui.label(RichText::new(&gonderim.yanit).size(12.0));
            });
    }

    gonder
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_kimligi_kodlu_ve_kodsuz() {
        let mut s = CanliSecim {
            saglayici: "gemini".to_string(),
            ..Default::default()
        };
        assert_eq!(model_kimligi(&s), "kervan/gemini");
        s.model_kodu = Some("orta".to_string());
        assert_eq!(model_kimligi(&s), "kervan/gemini/orta");
    }

    #[test]
    fn test_hesap_secimi_saglayici_degisince_secimleri_temizler() {
        let mut s = CanliSecim {
            saglayici: "gemini".to_string(),
            model_kodu: Some("orta".to_string()),
            ..Default::default()
        };
        s.anahtarlar.insert("derin".to_string(), true);
        let hesap = CanliHesap {
            anahtar: "kimi:k1".to_string(),
            profil: "k1".to_string(),
            saglayici: "kimi".to_string(),
            bagli: true,
            ..Default::default()
        };
        hesap_sec(&mut s, &hesap);
        assert_eq!(s.saglayici, "kimi");
        assert_eq!(s.hesap.as_deref(), Some("kimi:k1"));
        assert!(s.model_kodu.is_none());
        assert!(s.anahtarlar.is_empty());
    }
}
