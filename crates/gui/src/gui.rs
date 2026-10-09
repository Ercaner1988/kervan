//! Kervan Masaüstü Sade Pano (kilim-tema + eframe 0.36 + WGPU).
//!
//! Standart: KERVAN-PANO-ANTIGRAVITY-ISTEMI.md & KERVAN-ARAYUZ-ISTEMI.md
//! - Tek pencere, kilim-tema açık/koyu kâğıt lifi paleti
//! - WGPU Vulkan öncelikli arka uç (yoksa DX12)
//! - Boşta CPU sıfır (koşulsuz request_repaint yok)
//! - Beş ekran modüler kervan-pano-ekran sandığından çağrılır
//! - Sabit/sahte göstergeler (bonus token, saatlik 30 tavanı vb.) tamamen temizlenmiştir.

use crate::server::AppState;
use eframe::egui;
use kervan_pano_ekran::PanoEkraniDurumu;
use kervan_pano_veri::PanoOkuyucu;
use kilim_tema::{TemaTercihi, Varyant};
use std::io::Write;
use std::sync::Arc;

/// Desteklenen Üretim Modları (gui_modeller uyumluluğu için).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UretimModu {
    Sohbet,
    Dusunme,
    DerinArastirma,
    GorselUretimi,
    VideoHazirlama,
    Ses,
}

impl UretimModu {
    pub fn etiket(&self) -> &'static str {
        match self {
            Self::Sohbet => "💬 Sohbet",
            Self::Dusunme => "🧠 Düşünme",
            Self::DerinArastirma => "🔬 Derin Araştırma",
            Self::GorselUretimi => "🎨 Görsel Üretimi",
            Self::VideoHazirlama => "🎬 Video Hazırlama",
            Self::Ses => "🎙 Ses",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DusunmeSeviyesi {
    Dusuk,
    Orta,
    Derin,
}

impl DusunmeSeviyesi {
    pub fn etiket(&self) -> &'static str {
        match self {
            Self::Dusuk => "Düşük",
            Self::Orta => "Orta",
            Self::Derin => "Derin",
        }
    }
}

fn simdi_tarih() -> String {
    let now = std::time::SystemTime::now();
    let sec = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{sec}")
}

pub fn log_yaz(mesaj: &str) {
    eprintln!("{mesaj}");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("kervan_gui.log")
    {
        let _ = writeln!(f, "[{}] {}", simdi_tarih(), mesaj);
    }
}

struct FileLogger;

impl log::Log for FileLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        let t = record.target();
        if t.starts_with("egui")
            || t.starts_with("eframe")
            || t.starts_with("wgpu")
            || t.starts_with("winit")
        {
            log_yaz(&format!(
                "[LOG {}] {}: {}",
                record.level(),
                record.target(),
                record.args()
            ));
        }
    }

    fn flush(&self) {}
}

static LOGGER: FileLogger = FileLogger;

/// Kervan Sade Pano Uygulama Durumu
pub struct KervanApp {
    pub state: Arc<AppState>,
    pub okuyucu: PanoOkuyucu,
    pub pano_durumu: PanoEkraniDurumu,
    pub tema_tercihi: TemaTercihi,
    pub kare_sayisi: u64,
}

impl KervanApp {
    pub fn new(state: Arc<AppState>) -> Self {
        let okuyucu = PanoOkuyucu::yeni();
        let pano_durumu = PanoEkraniDurumu::default();
        let tema_tercihi = TemaTercihi::varsayilan(Varyant::CamGobegiAltin);

        Self {
            state,
            okuyucu,
            pano_durumu,
            tema_tercihi,
            kare_sayisi: 0,
        }
    }

    pub fn baslat(state: Arc<AppState>) -> Result<(), eframe::Error> {
        // 1. Panik yakalayıcı
        std::panic::set_hook(Box::new(|info| {
            let msg = format!("KERVAN GUI PANİK: {info}");
            log_yaz(&msg);
        }));

        // 2. Günlükleyici
        let _ = log::set_logger(&LOGGER).map(|()| log::set_max_level(log::LevelFilter::Debug));
        log_yaz("─── Kervan Sade Pano Başlatılıyor (kilim-tema) ───");

        // 3. Pencere ve grafik seçenekleri
        let options = eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            viewport: egui::ViewportBuilder::default()
                .with_title("Kervan Sade Pano")
                .with_inner_size([1280.0, 800.0])
                .with_min_inner_size([960.0, 600.0])
                .with_position([100.0, 100.0])
                .with_transparent(false)
                .with_decorations(true)
                .with_active(true),
            multisampling: 0,
            depth_buffer: 0,
            stencil_buffer: 0,
            persist_window: false,
            ..Default::default()
        };

        log_yaz("Pencere seçenekleri: renderer=Wgpu, vsync=true, multisampling=0, transparent=false");

        eframe::run_native(
            "kervan_gui",
            options,
            Box::new(|cc| {
                // kilim-tema açık ve koyu temalarını egui context'e kur
                kilim_tema::iki_temayi_kur(&cc.egui_ctx);
                log_yaz("kilim-tema iki_temayi_kur başarıyla bağlandı.");
                Ok(Box::new(Self::new(state)))
            }),
        )
    }
}

impl eframe::App for KervanApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.kare_sayisi += 1;
        let ozet = self.okuyucu.guncel_ozet();

        // ─── 1. ÜST ŞERİT ────────────────────────────────────────────────────────
        egui::Panel::top("pano_ust_serit").show(ui, |ui| {
            kervan_pano_ekran::render_ust_serit(
                ui,
                &ozet,
                &mut self.tema_tercihi,
                &mut self.pano_durumu,
            );
        });

        // ─── 2. ALT ŞERİT (CANLI AKIŞ) ───────────────────────────────────────────
        egui::Panel::bottom("pano_alt_serit").show(ui, |ui| {
            kervan_pano_ekran::render_alt_serit(ui, &ozet);
        });

        // ─── 3. SOL GEZİNTİ ŞERİDİ (5 SEKME) ────────────────────────────────────
        egui::Panel::left("pano_sol_nav").show(ui, |ui| {
            kervan_pano_ekran::render_sol_nav(ui, &mut self.pano_durumu);
        });

        // ─── 4. MERKEZİ EKRAN ALANI (5 EKRANDAN BİRİ) ────────────────────────────
        egui::CentralPanel::default().show(ui, |ui| {
            kervan_pano_ekran::render_ana_ekran(
                ui,
                &mut self.pano_durumu,
                &ozet,
                &mut self.tema_tercihi,
            );
        });

        // Boşta CPU sıfır kuralı: kare işlevinde koşulsuz request_repaint() ÇAĞRILMAZ.
        // Canlılık veya veri yenilemesi gerekiyorsa arka plan kanalı tetikler.
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_sabit_gostergeler_temizlendi() {
        const GUI_KODU: &str = include_str!("gui.rs");
        // Test fonksiyonu öncesindeki üretim kodunu ayıkla
        let uretim_kodu = GUI_KODU.split("#[cfg(test)]").next().unwrap_or(GUI_KODU);
        
        let bonus_sayisi = format!("3.{}.000", "000");
        let hediye_miktari = format!("3000{}", "000");
        let hatali_baslik = format!("— Antigravity {} Denetim Masası", "Gateway");
        let sabit_giris = format!("Günlük Giriş {}: Aktif", "Durumu");

        assert!(!uretim_kodu.contains(&bonus_sayisi), "Bonus literali bulunmamalı");
        assert!(!uretim_kodu.contains(&hediye_miktari), "Hediye miktarı bulunmamalı");
        assert!(!uretim_kodu.contains(&hatali_baslik), "Eski hatalı başlık bulunmamalı");
        assert!(!uretim_kodu.contains(&sabit_giris), "Sabit giriş durumu bulunmamalı");
    }
}
