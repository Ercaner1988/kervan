use eframe::egui;

/// Paneller arasında paylaşılan durum
#[derive(Debug, Clone, Default)]
pub struct PanelDurumu {
    pub aktif_istek_sayisi: u32,
    pub toplam_istek: u64,
    pub son_hata: Option<String>,
    pub aranan_model: String,
    pub pii_acik_mi: bool,
}

/// Her bir Kervan masaüstü paneli için temel sözleşme (ADR 0008)
pub trait KervanPaneli: Send + Sync {
    /// Panel tekil kimliği
    fn kimlik(&self) -> &'static str;

    /// Kenar çubuğunda ve başlıkta görünen ad
    fn baslik(&self) -> &'static str;

    /// Menüde gösterilecek simge / emoji
    fn simge(&self) -> &'static str;

    /// Panel arayüzünü çizer
    fn ciz(&mut self, ui: &mut egui::Ui, durum: &mut PanelDurumu);
}
