use crate::gosterge::GostergePaneli;
use crate::guvenlik::GuvenlikPaneli;
use crate::modeller::ModelYonetimPaneli;
use crate::trait_tanimi::{KervanPaneli, PanelDurumu};
use eframe::egui;

/// Merkezi Panel Kayıt Sistemi (ADR 0008)
pub struct PanelKayit {
    paneller: Vec<Box<dyn KervanPaneli>>,
    aktif_indeks: usize,
}

impl Default for PanelKayit {
    fn default() -> Self {
        Self::varsayilan()
    }
}

impl PanelKayit {
    pub fn yeni() -> Self {
        Self {
            paneller: Vec::new(),
            aktif_indeks: 0,
        }
    }

    /// Varsayılan standart panellerle başlatır
    pub fn varsayilan() -> Self {
        let mut kayit = Self::yeni();
        kayit.kaydet(Box::new(GostergePaneli));
        kayit.kaydet(Box::new(ModelYonetimPaneli::default()));
        kayit.kaydet(Box::new(GuvenlikPaneli));
        kayit
    }

    /// Yeni panel kaydeder
    pub fn kaydet(&mut self, panel: Box<dyn KervanPaneli>) {
        self.paneller.push(panel);
    }

    /// Toplam kayıtlı panel sayısı
    pub fn panel_sayisi(&self) -> usize {
        self.paneller.len()
    }

    /// Sol kenar çubuğu navigasyonunu çizer
    pub fn kenar_cubugu_ciz(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.add_space(8.0);
            for (i, p) in self.paneller.iter().enumerate() {
                let secili = self.aktif_indeks == i;
                let etiket = format!("{} {}", p.simge(), p.baslik());
                if ui.selectable_label(secili, etiket).clicked() {
                    self.aktif_indeks = i;
                }
            }
        });
    }

    /// Seçili paneli ana çalışma alanında çizer
    pub fn aktif_paneli_ciz(&mut self, ui: &mut egui::Ui, durum: &mut PanelDurumu) {
        if let Some(panel) = self.paneller.get_mut(self.aktif_indeks) {
            panel.ciz(ui, durum);
        }
    }
}
