pub mod gosterge;
pub mod guvenlik;
pub mod kayit;
pub mod modeller;
pub mod trait_tanimi;

pub use gosterge::GostergePaneli;
pub use guvenlik::GuvenlikPaneli;
pub use kayit::PanelKayit;
pub use modeller::ModelYonetimPaneli;
pub use trait_tanimi::{KervanPaneli, PanelDurumu};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_panel_kayit_varsayilan() {
        let kayit = PanelKayit::varsayilan();
        assert_eq!(kayit.panel_sayisi(), 3);
    }

    #[test]
    fn test_panel_ekleme() {
        let mut kayit = PanelKayit::yeni();
        assert_eq!(kayit.panel_sayisi(), 0);

        kayit.kaydet(Box::new(GostergePaneli));
        assert_eq!(kayit.panel_sayisi(), 1);

        kayit.kaydet(Box::new(GuvenlikPaneli));
        assert_eq!(kayit.panel_sayisi(), 2);
    }
}
