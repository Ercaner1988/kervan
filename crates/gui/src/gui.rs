//! Kervan Masaüstü Denetim Masası (Antigravity Düzeni + egui + Çoklu Medya).
//!
//! Bu modül, Antigravity IDE arayüz düzenini ve çoklu ortam (Imagen 3 / Veo 2)
//! üretim yeteneklerini sunar:
//! - Üst Çubuk: Başlık ve Hızlı Durum
//! - Sol Panel: Sağlayıcılar & Ajan Ağacı (Canlı bağlı sekme göstergesi ile)
//! - Orta Alan: Model Seçimi (ComboBox), Üretim Modu Pills (Sohbet, Düşünme, Derin Araştırma,
//!   Görsel Üretimi, Video Hazırlama), Düşünme Seviyesi, Hızlı Örnekler, İstem Konsolu,
//!   Medya Galerisi Kartları (Imagen 3 / Veo 2) ve Yanıt Ekranı
//! - Sağ Panel: Zcode Tarzı Dinamik Bakiye, Giriş ve Hediye Takipçisi
//! - Alt Çubuk: Yönlendirme Stratejisi (Auto/LKGP) ve Canlı Token Tasarrufu
//! - Log Defteri: Tüm GUI, OpenGL ve panik olaylarını `kervan_gui.log` dosyasına yazar.

use crate::server::AppState;
use eframe::egui;
use std::io::Write;
use std::sync::Arc;

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
            || t.starts_with("glutin")
            || t.starts_with("glow")
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

/// Egui Ui üzerinde `output_mut` çağrısını doğrudan sağlayan genişletme özelliği.
pub trait UiExt {
    fn output_mut<R>(&self, f: impl FnOnce(&mut egui::PlatformOutput) -> R) -> R;
}

impl UiExt for egui::Ui {
    fn output_mut<R>(&self, f: impl FnOnce(&mut egui::PlatformOutput) -> R) -> R {
        self.ctx().output_mut(f)
    }
}

/// Desteklenen Üretim Modları
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UretimModu {
    /// Standart konuşma ve akıcı yanıtlar
    Sohbet,
    /// Düşünme süreci / Akıl yürütme (Düşünme seviyesi: Düşük, Orta, Derin)
    Dusunme,
    /// Kapsamlı araştırma ve kaynaklı rapor
    DerinArastirma,
    /// Gemini Imagen 3 görsel üretimi (sahne, çizim, fotogerçekçi)
    GorselUretimi,
    /// Gemini Veo 2 video oluşturma (animasyon, sinematik video)
    VideoHazirlama,
    /// Ses dosyasını yazıya dökme ve metni seslendirme (kervan-akis-gui)
    Ses,
}

impl UretimModu {
    pub fn etiket(&self) -> &'static str {
        match self {
            Self::Sohbet => "💬 Sohbet",
            Self::Dusunme => "🧠 Düşünme",
            Self::DerinArastirma => "🔍 Derin Araştırma",
            Self::GorselUretimi => "🎨 Görsel Üretimi",
            Self::VideoHazirlama => "🎬 Video Hazırlama",
            Self::Ses => "🎙 Ses",
        }
    }

    pub fn aciklama(&self) -> &'static str {
        match self {
            Self::Sohbet => "Standart sohbet ve akıcı yanıtlar",
            Self::Dusunme => "Adım adım akıl yürütme ve problem çözme",
            Self::DerinArastirma => "Kapsamlı araştırma, detaylı analiz ve raporlama",
            Self::GorselUretimi => "Gemini Imagen 3 ile yüksek kalitede fotogerçekçi görsel",
            Self::VideoHazirlama => "Gemini Veo 2 ile sinematik kalitede akıcı video",
            Self::Ses => "Ses dosyasını yazıya dök, metni seslendir",
        }
    }
}

/// Düşünme Modu için derinlik seviyeleri
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

pub use crate::gui_modeller::*;

/// "auto" seçiliyse akıllı rota bağlı sekmeler (yoksa havuz) arasından seçer.
async fn saglayici_sec(state: &AppState, secili: String) -> String {
    if secili != "auto" {
        return secili;
    }
    let mut adaylar = state.eklenti_hub.bagli_sekmeler().await;
    if adaylar.is_empty() {
        for slot in &state.pool.slots {
            if !adaylar.contains(&slot.provider.ad) {
                adaylar.push(slot.provider.ad.clone());
            }
        }
    }
    state
        .router
        .select_provider(&adaylar)
        .await
        .unwrap_or_else(|| state.varsayilan_saglayici.clone())
}

pub struct KervanApp {
    state: Arc<AppState>,
    secili_saglayici: String,
    secili_model: String,
    ozel_model_metni: String,
    uretim_modu: UretimModu,
    dusunme_seviyesi: DusunmeSeviyesi,
    istem_metni: String,
    manuel_dosya_yolu: String,
    ekli_dosyalar: Vec<YuklenenDosya>,
    son_yanit: String,
    istek_gonderiliyor: bool,
    hediye_ekleme_miktari: String,
    bagli_sekmeler: Vec<String>,
    kare_sayisi: u64,
    yanit_tx: std::sync::mpsc::Sender<String>,
    yanit_rx: std::sync::mpsc::Receiver<String>,
    sekmeler_rx: std::sync::mpsc::Receiver<Vec<String>>,
    dosya_secici_tx: std::sync::mpsc::Sender<String>,
    dosya_secici_rx: std::sync::mpsc::Receiver<String>,
    akis: kervan_akis_gui::Akis,
    akis_tx: std::sync::mpsc::Sender<(kervan_akis_gui::Is, Result<String, String>)>,
    akis_rx: std::sync::mpsc::Receiver<(kervan_akis_gui::Is, Result<String, String>)>,
    hesaplar: Vec<kervan_akis_gui::havuz::HesapSatiri>,
    hesaplar_rx: std::sync::mpsc::Receiver<Vec<kervan_akis_gui::havuz::HesapSatiri>>,
}

impl KervanApp {
    pub fn new(state: Arc<AppState>) -> Self {
        let (yanit_tx, yanit_rx) = std::sync::mpsc::channel();
        let (sekmeler_tx, sekmeler_rx) = std::sync::mpsc::channel();
        let (dosya_secici_tx, dosya_secici_rx) = std::sync::mpsc::channel();
        let (akis_tx, akis_rx) = std::sync::mpsc::channel();
        let (hesaplar_tx, hesaplar_rx) = std::sync::mpsc::channel();

        // Arka planda bağlı eklenti sekmelerini periyodik tarayan iş parçacığı
        let st_clone = state.clone();
        tokio::spawn(async move {
            loop {
                let tabs = st_clone.eklenti_hub.bagli_sekmeler().await;
                let hesaplar = kervan_akis_gui::havuz::goruntu(&st_clone.pool).await;
                if sekmeler_tx.send(tabs).is_err() || hesaplar_tx.send(hesaplar).is_err() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
            }
        });

        Self {
            state,
            secili_saglayici: "auto".to_string(),
            secili_model: "otomatik".to_string(),
            ozel_model_metni: String::new(),
            uretim_modu: UretimModu::Sohbet,
            dusunme_seviyesi: DusunmeSeviyesi::Orta,
            istem_metni: String::new(),
            manuel_dosya_yolu: String::new(),
            ekli_dosyalar: Vec::new(),
            son_yanit: "Henüz bir istek gönderilmedi. Buradan canlı test yapabilirsiniz."
                .to_string(),
            istek_gonderiliyor: false,
            hediye_ekleme_miktari: "3000000".to_string(),
            bagli_sekmeler: Vec::new(),
            kare_sayisi: 0,
            yanit_tx,
            yanit_rx,
            sekmeler_rx,
            dosya_secici_tx,
            dosya_secici_rx,
            akis: Default::default(),
            akis_tx,
            akis_rx,
            hesaplar: Vec::new(),
            hesaplar_rx,
        }
    }

    /// Akış panelinin istediği işi yürütür: istemi sağlayıcı sekmesine sorar, yanıtı
    /// panele geri verir.
    fn akis_eylemi(&mut self, eylem: kervan_akis_gui::Eylem) {
        use kervan_akis_gui::Eylem;
        match eylem {
            Eylem::SohbeteEkle(metin) => {
                self.istem_metni = metin;
                self.uretim_modu = UretimModu::Sohbet;
            }
            Eylem::Gonder {
                is,
                istem,
                dosya,
                video,
            } => {
                let mut dosyalar = Vec::new();
                if let Some(yol) = dosya {
                    match std::fs::read(&yol) {
                        Ok(bytes) => dosyalar.push(crate::eklenti::DosyaEki {
                            ad: std::path::Path::new(&yol)
                                .file_name()
                                .map_or(yol.clone(), |n| n.to_string_lossy().into_owned()),
                            mime_tipi: dosya_mime_belirle(&yol).to_string(),
                            veri_base64: base64_encode(&bytes),
                        }),
                        Err(e) => {
                            self.akis
                                .sonuc(is, Err(format!("dosya okunamadı ({yol}): {e}")));
                            return;
                        }
                    }
                }
                let istem = if video {
                    istem_cercevele(&istem, UretimModu::VideoHazirlama, self.dusunme_seviyesi)
                } else {
                    istem
                };
                let (state, secili, tx) = (
                    self.state.clone(),
                    self.secili_saglayici.clone(),
                    self.akis_tx.clone(),
                );
                log_yaz(&format!(
                    "Akış isteği: {is:?}, sağlayıcı={secili}, dosya={}",
                    dosyalar.len()
                ));
                tokio::spawn(async move {
                    let saglayici = saglayici_sec(&state, secili).await;
                    let sonuc = state
                        .eklenti_hub
                        .sor_ekli(&saglayici, &istem, dosyalar, state.timeout)
                        .await;
                    if tx
                        .send((is, sonuc.map_err(|e| format!("{saglayici}: {e:#}"))))
                        .is_err()
                    {
                        log_yaz("Akış yanıtı geldi ama pencere kapanmış; yanıt atıldı.");
                    }
                });
            }
        }
    }

    fn dosya_ekle(&mut self, yol: &str) {
        let temiz = yol.trim().trim_matches('"').trim_matches('\'');
        if temiz.is_empty() {
            return;
        }
        let p = std::path::Path::new(temiz);
        if p.is_file() {
            let ad = p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| temiz.to_string());
            let boyut = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
            let mime_tipi = dosya_mime_belirle(temiz).to_string();

            if !self.ekli_dosyalar.iter().any(|d| d.yol == temiz) {
                log_yaz(&format!("Dosya eklendi: {ad} ({boyut} bayt, {mime_tipi})"));
                self.ekli_dosyalar.push(YuklenenDosya {
                    yol: temiz.to_string(),
                    ad,
                    boyut,
                    mime_tipi,
                });
            }
        } else {
            log_yaz(&format!("Dosya bulunamadı veya geçersiz yol: {temiz}"));
        }
    }

    pub fn baslat(state: Arc<AppState>) -> Result<(), eframe::Error> {
        // 1. Panik yakalayıcı (Panic Hook)
        std::panic::set_hook(Box::new(|info| {
            let msg = format!("KERVAN GUI PANİK: {info}");
            log_yaz(&msg);
        }));

        // 2. Günlükleyiciyi bağla
        let _ = log::set_logger(&LOGGER).map(|()| log::set_max_level(log::LevelFilter::Debug));

        log_yaz("─── Kervan GUI Başlatılıyor ───");

        // 3. Pencere ve grafik seçenekleri
        let options = eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            viewport: egui::ViewportBuilder::default()
                .with_title("Kervan Denetim Masası — Antigravity Engine")
                .with_inner_size([1180.0, 720.0])
                .with_min_inner_size([800.0, 500.0])
                .with_position([120.0, 120.0])
                .with_transparent(false)
                .with_decorations(true)
                .with_active(true),
            vsync: true,
            multisampling: 0,
            depth_buffer: 0,
            stencil_buffer: 0,
            persist_window: false,
            ..Default::default()
        };

        log_yaz(
            "Pencere seçenekleri: renderer=Wgpu (DirectX 12), vsync=true, multisampling=0, transparent=false",
        );

        eframe::run_native(
            "kervan_gui",
            options,
            Box::new(|cc| {
                let mut visuals = egui::Visuals::dark();
                visuals.panel_fill = egui::Color32::from_rgb(26, 27, 30);
                visuals.window_fill = egui::Color32::from_rgb(20, 21, 23);
                cc.egui_ctx.set_visuals(visuals);
                log_yaz("Egui Context ve tema görseli yüklendi.");
                Ok(Box::new(Self::new(state)))
            }),
        )
    }
}

impl eframe::App for KervanApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.kare_sayisi += 1;
        if self.kare_sayisi <= 3 || self.kare_sayisi.is_multiple_of(120) {
            log_yaz(&format!(
                "Kare #{:<5} | Alan: {:?} | DPI: {:.2}",
                self.kare_sayisi,
                ctx.screen_rect(),
                ctx.pixels_per_point()
            ));
        }

        // Canlı güncellemeleri çek
        while let Ok(tabs) = self.sekmeler_rx.try_recv() {
            self.bagli_sekmeler = tabs;
        }
        while let Ok(yanit) = self.yanit_rx.try_recv() {
            self.son_yanit = yanit;
            self.istek_gonderiliyor = false;
        }
        while let Ok(yol) = self.dosya_secici_rx.try_recv() {
            self.dosya_ekle(&yol);
        }
        while let Ok((is, yanit)) = self.akis_rx.try_recv() {
            self.akis.sonuc(is, yanit);
        }
        while let Ok(h) = self.hesaplar_rx.try_recv() {
            self.hesaplar = h;
        }
        self.akis.guncelle();

        // Sürükle-bırak desteği: pencereye dosya bırakıldığında otomatik ekle
        ctx.input(|i| {
            for f in &i.raw.dropped_files {
                if let Some(path) = &f.path {
                    let yol_str = path.to_string_lossy().to_string();
                    self.dosya_ekle(&yol_str);
                }
            }
        });

        // ─── 1. ÜST ÇUBUK (HEADER) ──────────────────────────────────────────────
        egui::TopBottomPanel::top("header_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("KERVAN")
                        .strong()
                        .size(16.0)
                        .color(egui::Color32::from_rgb(100, 190, 255)),
                );
                ui.label("— Antigravity Gateway Denetim Masası");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("Kare: #{}", self.kare_sayisi));
                });
            });
        });

        // ─── 2. ALT ÇUBUK (STATUS BAR) ──────────────────────────────────────────
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("● Kervan Gateway 127.0.0.1:20129")
                        .color(egui::Color32::from_rgb(80, 220, 120)),
                );
                ui.separator();

                let tasarruf = self.state.compressor.toplam_tasarruf_orani();
                ui.label(format!("Token Tasarrufu: %{:.1} (RTK + Caveman)", tasarruf));
                ui.separator();

                ui.label("Aktif Strateji:");
                if let Ok(strat) = self.state.router.strategy.try_read() {
                    ui.strong(format!("{:?}", *strat));
                }
                ui.separator();

                ui.label(format!("Bağlı Sekme: {}", self.bagli_sekmeler.len()));
                ui.separator();

                use std::sync::atomic::Ordering::Relaxed;
                let s = &self.state.sayaclar;
                ui.label(
                    egui::RichText::new(format!(
                        "↻ Kurtarılan yanıt: {}",
                        s.kurtarilan_yanit.load(Relaxed)
                    ))
                    .strong()
                    .color(kervan_akis_gui::renk::UYARI),
                )
                .on_hover_text("Yarıda kesilip devam istemiyle tamamlanan yanıtlar");
                ui.separator();
                ui.label(
                    egui::RichText::new(format!(
                        "🛠 Araç çağrısı: {}",
                        s.arac_cagrisi.load(Relaxed)
                    ))
                    .strong()
                    .color(kervan_akis_gui::renk::VURGU),
                )
                .on_hover_text("Yanıtlardan ayıklanıp istemciye tool_calls olarak dönen çağrılar");
            });
        });

        // ─── 3. SOL PANEL (SAĞLAYICILAR VE AJANLAR) ─────────────────────────────
        egui::SidePanel::left("providers_panel")
            .resizable(true)
            .default_width(220.0)
            .min_width(160.0)
            .max_width(320.0)
            .show(ctx, |ui| {
                ui.heading("Sağlayıcılar");
                ui.separator();

                let saglayici_listesi = [
                    ("auto", "Otomatik (Akıllı Rota)"),
                    ("gemini", "Google Gemini (Web)"),
                    ("deepseek", "DeepSeek Chat (Web)"),
                    ("chatgpt", "ChatGPT / Codex (Web)"),
                    ("qwen", "Qwen 2.5 (Web)"),
                    ("kimi", "Kimi AI (Web)"),
                    ("copilot", "Microsoft Copilot (Web)"),
                    ("z.ai", "z.ai Platform (Web)"),
                    ("perplexity", "Perplexity AI (Web)"),
                    ("mistral", "Mistral Le Chat (Web)"),
                ];

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (kod, baslik) in saglayici_listesi {
                        let secili = self.secili_saglayici == kod;
                        let bagli = kod == "auto" || self.bagli_sekmeler.iter().any(|s| s == kod);

                        ui.horizontal(|ui| {
                            let dur_isareti = if bagli {
                                egui::RichText::new("●")
                                    .color(egui::Color32::from_rgb(80, 220, 120))
                            } else {
                                egui::RichText::new("○")
                                    .color(egui::Color32::from_rgb(120, 120, 120))
                            };
                            ui.label(dur_isareti);

                            let mut etiket = egui::RichText::new(baslik);
                            if secili {
                                etiket = etiket
                                    .strong()
                                    .color(egui::Color32::from_rgb(100, 180, 255));
                            }
                            if ui.selectable_label(secili, etiket).clicked() {
                                self.secili_saglayici = kod.to_string();
                                if let Some(amiral) =
                                    amiral_model(&self.secili_saglayici, self.uretim_modu)
                                {
                                    self.secili_model = amiral.to_string();
                                } else {
                                    let modeller = saglayici_modelleri(&self.secili_saglayici);
                                    if !modeller.iter().any(|m| m.kod == self.secili_model) {
                                        self.secili_model =
                                            varsayilan_model(&self.secili_saglayici).to_string();
                                    }
                                }
                                self.ozel_model_metni.clear();
                            }
                        });
                        if secili
                            && let Some(i) = kervan_akis_gui::havuz::ciz(ui, &self.hesaplar, kod)
                        {
                            let state = self.state.clone();
                            tokio::spawn(async move {
                                if !crate::server::hesabi_goster(state, i).await {
                                    log_yaz(&format!(
                                        "{i} numaralı hesabın penceresi gösterilemedi"
                                    ));
                                }
                            });
                        }
                    }
                });
            });

        // ─── 4. SAĞ PANEL (ZCODE TARZI DİNAMİK BAKİYE VE HEDİYE ÖĞRENİCİ) ───────
        egui::SidePanel::right("balance_panel")
            .resizable(true)
            .default_width(240.0)
            .min_width(180.0)
            .max_width(320.0)
            .show(ctx, |ui| {
                ui.heading("Bakiye & Limit Öğrenici");
                ui.separator();

                ui.label(egui::RichText::new("Zcode Dinamik Bakiye").strong());
                ui.label("Günlük Giriş Durumu: Aktif");
                ui.add_space(8.0);

                ui.group(|ui| {
                    ui.label(
                        egui::RichText::new("🎁 Hafta Sonu Bonusu")
                            .color(egui::Color32::from_rgb(255, 215, 0)),
                    );
                    ui.label("Tanımlı Bonus: +3.000.000 Token");
                    ui.horizontal(|ui| {
                        ui.label("Miktar:");
                        ui.text_edit_singleline(&mut self.hediye_ekleme_miktari);
                    });
                    if ui.button("Bonus Token Yükle").clicked()
                        && let Ok(m) = self.hediye_ekleme_miktari.parse::<u64>()
                    {
                        let state = self.state.clone();
                        let s = self.secili_saglayici.clone();
                        tokio::spawn(async move {
                            let _ = state.defter.hediye_ekle("varsayilan", &s, m).await;
                        });
                    }
                });

                ui.add_space(12.0);
                ui.label(egui::RichText::new("Öğrenilen Limitler").strong());
                ui.label("Saatlik İstek Tavanı: 30 istek/saat");
                ui.label("Pencere Uzunluğu: Gözlemleniyor");
            });

        // ─── 5. ORTA ALAN (ÇALIŞMA ALANI & TEST KONSOLU) ─────────────────────────
        egui::CentralPanel::default().show(ctx, |ui| {
            // A) Sağlayıcı ve Model Seçimi Toolbarı
            ui.horizontal_wrapped(|ui| {
                ui.heading(format!("Çalışma Alanı: {}", self.secili_saglayici));
                ui.separator();

                ui.label(egui::RichText::new("Model:").strong());
                let modeller = saglayici_modelleri(&self.secili_saglayici);
                let secili_gorunen = modeller
                    .iter()
                    .find(|m| m.kod == self.secili_model)
                    .map(|m| m.gorunen_ad)
                    .unwrap_or(self.secili_model.as_str());

                egui::ComboBox::from_id_salt("model_secici_combobox")
                    .selected_text(secili_gorunen)
                    .width(320.0)
                    .show_ui(ui, |ui| {
                        for m in modeller {
                            let secili = self.secili_model == m.kod;
                            let etiket = format!("{}\n  ℹ {}", m.gorunen_ad, m.aciklama);
                            if ui.selectable_label(secili, etiket).clicked() {
                                self.secili_model = m.kod.to_string();
                                self.ozel_model_metni.clear();
                            }
                        }
                    });

                ui.separator();
                ui.label(egui::RichText::new("Özel Model:").strong());
                let ozel_edit = ui.add(
                    egui::TextEdit::singleline(&mut self.ozel_model_metni)
                        .hint_text("Web model kodunu girin (Enter)...")
                        .desired_width(180.0),
                );
                if (ozel_edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                    || ui.button("Uygula").on_hover_text("Özel modeli aktif model olarak ayarlar").clicked()
                {
                    let t = self.ozel_model_metni.trim();
                    if !t.is_empty() {
                        self.secili_model = t.to_string();
                    }
                }
            });

            // Seçili modelin detaylı açıklaması ve kodu
            let modeller = saglayici_modelleri(&self.secili_saglayici);
            if let Some(tanim) = modeller.iter().find(|m| m.kod == self.secili_model) {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("ℹ Model Bilgisi:").color(egui::Color32::from_rgb(100, 190, 255)).size(12.0).strong());
                    ui.label(egui::RichText::new(tanim.aciklama).color(egui::Color32::from_rgb(180, 215, 245)).size(12.0));
                    ui.label(egui::RichText::new(format!("(Kod: `{}`)", tanim.kod)).weak().size(11.0));
                });
            } else if !self.secili_model.is_empty() {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("✏ Özel Model:").color(egui::Color32::from_rgb(255, 200, 80)).size(12.0).strong());
                    ui.label(egui::RichText::new(format!("`{}` kullanılıyor", self.secili_model)).color(egui::Color32::from_rgb(255, 220, 140)).size(12.0));
                });
            }

            ui.add_space(4.0);

            // B) Üretim Modu Seçici Butonları (Pills)
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new("Üretim Modu:").strong());

                let modlar = [
                    (UretimModu::Sohbet, "💬 Sohbet"),
                    (UretimModu::Dusunme, "🧠 Düşünme"),
                    (UretimModu::DerinArastirma, "🔍 Derin Araştırma"),
                    (UretimModu::GorselUretimi, "🎨 Görsel Üretimi"),
                    (UretimModu::VideoHazirlama, "🎬 Video Hazırlama"),
                    (UretimModu::Ses, "🎙 Ses"),
                ];

                for (mod_tipi, etiket) in modlar {
                    let secili = self.uretim_modu == mod_tipi;
                    let mut btn_text = egui::RichText::new(etiket);
                    if secili {
                        btn_text = btn_text.strong().color(egui::Color32::from_rgb(255, 215, 0));
                    }
                    if ui.selectable_label(secili, btn_text).clicked() {
                        self.uretim_modu = mod_tipi;
                        if let Some(amiral) = amiral_model(&self.secili_saglayici, mod_tipi) {
                            self.secili_model = amiral.to_string();
                            self.ozel_model_metni.clear();
                        }
                    }
                }
            });

            // Ses ve Video modu: akış panelleri sohbet konsolunun yerini alır.
            if matches!(self.uretim_modu, UretimModu::Ses | UretimModu::VideoHazirlama) {
                ui.label(egui::RichText::new(self.uretim_modu.aciklama()).weak());
                let eylem = egui::ScrollArea::vertical()
                    .show(ui, |ui| match self.uretim_modu {
                        UretimModu::Ses => self.akis.ses_ciz(ui),
                        _ => self.akis.video_ciz(ui),
                    })
                    .inner;
                if let Some(e) = eylem {
                    self.akis_eylemi(e);
                }
                return;
            }

            // C) Düşünme Modu Seçiliyse Düşünme Seviyesi Seçicisi
            if self.uretim_modu == UretimModu::Dusunme {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("⚡ Düşünme Seviyesi:").color(egui::Color32::from_rgb(140, 200, 255)));
                    let seviyeler = [
                        (DusunmeSeviyesi::Dusuk, "Düşük (Hızlı)"),
                        (DusunmeSeviyesi::Orta, "Orta (Dengeli)"),
                        (DusunmeSeviyesi::Derin, "Derin (Kapsamlı Akıl Yürütme)"),
                    ];
                    for (sev, etiket) in seviyeler {
                        let secili = self.dusunme_seviyesi == sev;
                        if ui.selectable_label(secili, etiket).clicked() {
                            self.dusunme_seviyesi = sev;
                        }
                    }
                });
            }

            // D) Hızlı Örnek İstem Butonları
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new("Hızlı Örnekler:").italics());
                if ui.button("🎨 Görsel Örneği").clicked() {
                    self.uretim_modu = UretimModu::GorselUretimi;
                    if let Some(amiral) = amiral_model(&self.secili_saglayici, UretimModu::GorselUretimi) {
                        self.secili_model = amiral.to_string();
                    } else {
                        self.secili_model = "imagen-3".to_string();
                    }
                    self.ozel_model_metni.clear();
                    self.istem_metni = "Fütüristik bir siberpunk İstanbul Boğazı, neon ışıklar altında süzülen uçan vapurlar, yağmurlu gece ve ıslak yansımalar, 8k çözünürlük sinematik konsept sanat.".to_string();
                }
                if ui.button("🎬 Video Örneği").clicked() {
                    self.uretim_modu = UretimModu::VideoHazirlama;
                    if let Some(amiral) = amiral_model(&self.secili_saglayici, UretimModu::VideoHazirlama) {
                        self.secili_model = amiral.to_string();
                    } else {
                        self.secili_model = "veo-2".to_string();
                    }
                    self.ozel_model_metni.clear();
                    self.istem_metni = "Kapadokya vadileri üzerinde gün doğumunda yavaşça yükselen rengarenk sıcak hava balonları, yumuşak sis, altın saat ışığı, 4k drone çekimi.".to_string();
                }
                if ui.button("🔍 Araştırma Örneği").clicked() {
                    self.uretim_modu = UretimModu::DerinArastirma;
                    if let Some(amiral) = amiral_model(&self.secili_saglayici, UretimModu::DerinArastirma) {
                        self.secili_model = amiral.to_string();
                    }
                    self.ozel_model_metni.clear();
                    self.istem_metni = "Kuantum bilgisayarların mevcut şifreleme algoritmaları (RSA, ECC) üzerindeki riskleri ve post-kuantum kriptografi standartları.".to_string();
                }
            });

            ui.separator();

            // Dosya Ekleme Araç Çubuğu (Yerel Seçici + Hızlı Yol Kutusu + Sürükle Bırak İpucu)
            ui.horizontal_wrapped(|ui| {
                if ui.button("📎 Dosya Ekle (Görsel / Video / Belge)").on_hover_text("Windows yerel dosya seçici penceresini açar").clicked() {
                    let tx = self.dosya_secici_tx.clone();
                    std::thread::spawn(move || {
                        let ps_cmd = r#"[System.Reflection.Assembly]::LoadWithPartialName('System.Windows.Forms') | Out-Null; $f = New-Object System.Windows.Forms.OpenFileDialog; $f.Title = 'Kervan - Dosya Seç (Görsel / Video / Belge)'; $f.Filter = 'Tüm Desteklenenler|*.png;*.jpg;*.jpeg;*.webp;*.gif;*.mp4;*.webm;*.mov;*.pdf;*.txt;*.json;*.md;*.csv;*.zip|Görseller (*.png, *.jpg, *.webp)|*.png;*.jpg;*.jpeg;*.webp;*.gif;*.svg|Videolar (*.mp4, *.webm, *.mov)|*.mp4;*.webm;*.mov;*.avi;*.mkv|Belgeler (*.pdf, *.txt, *.json)|*.pdf;*.txt;*.json;*.md;*.csv;*.zip|Tüm Dosyalar (*.*)|*.*'; $f.Multiselect = $true; if ($f.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { $f.FileNames | ForEach-Object { [Console]::WriteLine($_) } }"#;
                        if let Ok(output) = std::process::Command::new("powershell")
                            .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", ps_cmd])
                            .output()
                        {
                            let text = String::from_utf8_lossy(&output.stdout);
                            for line in text.lines() {
                                let l = line.trim().trim_matches('"');
                                if !l.is_empty() {
                                    let _ = tx.send(l.to_string());
                                }
                            }
                        }
                    });
                }

                ui.separator();
                ui.label(egui::RichText::new("veya Dosya Yolu:").weak());
                let yol_edit = ui.add(
                    egui::TextEdit::singleline(&mut self.manuel_dosya_yolu)
                        .hint_text("C:\\ornek\\resim.png (Enter ile ekle)")
                        .desired_width(180.0),
                );
                if (yol_edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) || ui.button("Ekle").clicked() {
                    let yol = self.manuel_dosya_yolu.clone();
                    self.dosya_ekle(&yol);
                    self.manuel_dosya_yolu.clear();
                }

                ui.label(
                    egui::RichText::new("(Dosyaları pencereye doğrudan sürükleyip bırakabilirsiniz)")
                        .italics()
                        .weak()
                        .size(11.0),
                );
            });

            // Ekli Dosyalar Rozetleri
            if !self.ekli_dosyalar.is_empty() {
                ui.add_space(3.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new("Ekli Dosyalar:")
                            .strong()
                            .color(egui::Color32::from_rgb(100, 200, 255)),
                    );

                    let mut silinecek_indeks = None;
                    for (idx, dosya) in self.ekli_dosyalar.iter().enumerate() {
                        let simge = if dosya.mime_tipi.starts_with("image/") {
                            "🖼 Görsel"
                        } else if dosya.mime_tipi.starts_with("video/") {
                            "🎬 Video"
                        } else {
                            "📄 Belge"
                        };

                        let boyut_str = if dosya.boyut < 1024 {
                            format!("{} B", dosya.boyut)
                        } else if dosya.boyut < 1024 * 1024 {
                            format!("{:.1} KB", dosya.boyut as f64 / 1024.0)
                        } else {
                            format!("{:.2} MB", dosya.boyut as f64 / (1024.0 * 1024.0))
                        };

                        let rozet_metni = format!("{} {} ({})", simge, dosya.ad, boyut_str);
                        ui.group(|ui| {
                            ui.label(egui::RichText::new(rozet_metni).size(12.0));
                            if ui.small_button("✕").on_hover_text("Bu dosyayı kaldır").clicked() {
                                silinecek_indeks = Some(idx);
                            }
                        });
                    }

                    if let Some(idx) = silinecek_indeks {
                        self.ekli_dosyalar.remove(idx);
                    }

                    if ui.button("🗑 Tümünü Temizle").clicked() {
                        self.ekli_dosyalar.clear();
                    }
                });
            }

            ui.add_space(4.0);

            // İstem Giriş Alanı
            ui.label(egui::RichText::new(format!("İstem Test Kutusu [{}] :", self.uretim_modu.aciklama())).weak());
            let genislik = ui.available_width();
            ui.add(
                egui::TextEdit::multiline(&mut self.istem_metni)
                    .hint_text("Bir istem yazın veya yukarıdaki hızlı örneklerden birine tıklayın...")
                    .desired_rows(3)
                    .desired_width(genislik),
            );

            ui.horizontal(|ui| {
                let buton_etiketi = if self.istek_gonderiliyor { "⏳ Bekleniyor..." } else { "▶ Gönder" };
                let gonderilebilir = (!self.istem_metni.trim().is_empty() || !self.ekli_dosyalar.is_empty()) && !self.istek_gonderiliyor;
                if ui.button(buton_etiketi).clicked() && gonderilebilir {
                    self.istek_gonderiliyor = true;
                    self.son_yanit = "Yanıt bekleniyor...".to_string();

                    // Ekli dosyaları diskten oku ve Base64'e kodla
                    let mut dosyalar = Vec::new();
                    for ek in &self.ekli_dosyalar {
                        match std::fs::read(&ek.yol) {
                            Ok(bytes) => {
                                dosyalar.push(crate::eklenti::DosyaEki {
                                    ad: ek.ad.clone(),
                                    mime_tipi: ek.mime_tipi.clone(),
                                    veri_base64: base64_encode(&bytes),
                                });
                            }
                            Err(e) => {
                                log_yaz(&format!("Dosya okunamadı ({}): {e}", ek.yol));
                            }
                        }
                    }
                    self.ekli_dosyalar.clear();

                    let state = self.state.clone();
                    let secili = self.secili_saglayici.clone();
                    let secili_mod = self.secili_model.clone();
                    let ham_istem = if self.istem_metni.trim().is_empty() && !dosyalar.is_empty() {
                        "Lütfen ekteki dosyaları incele ve açıkla.".to_string()
                    } else {
                        self.istem_metni.clone()
                    };
                    let son_istem = istem_cercevele(&ham_istem, self.uretim_modu, self.dusunme_seviyesi);
                    let tx = self.yanit_tx.clone();

                    log_yaz(&format!(
                        "İstek gönderiliyor: sağlayıcı={secili}, model={secili_mod}, mod={:?}, dosya_sayisi={}",
                        self.uretim_modu,
                        dosyalar.len()
                    ));

                    // Arka planda asenkron istek
                    tokio::spawn(async move {
                        let saglayici = saglayici_sec(&state, secili).await;
                        let sonuc = match state.eklenti_hub.sor_ekli(&saglayici, &son_istem, dosyalar, state.timeout).await {
                            Ok(y) => format!("[{saglayici} / {secili_mod}]:\n\n{y}"),
                            Err(e) => format!("Hata ({saglayici} / {secili_mod}): {e:#}"),
                        };
                        let _ = tx.send(sonuc);
                    });
                }

                if ui.button("Temizle").clicked() {
                    self.istem_metni.clear();
                    self.ekli_dosyalar.clear();
                }
            });

            ui.separator();
            ui.label("Son Yanıt / Canlı Akış:");

            // Medya Kartları (Imagen 3 & Veo 2)
            let medyalar = medyalari_ayikla(&self.son_yanit);
            if !medyalar.is_empty() {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("🎬 Çoklu Medya Galerisi (Imagen 3 / Veo 2)")
                                .strong()
                                .color(egui::Color32::from_rgb(255, 180, 80)),
                        );
                        ui.label(format!("({} Öğe)", medyalar.len()));
                    });
                    ui.separator();

                    for medya in &medyalar {
                        ui.group(|ui| {
                            match medya.tip {
                                MedyaTipi::Gorsel => {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new("🎨 Üretilen Görsel (Imagen 3)")
                                                .strong()
                                                .color(egui::Color32::from_rgb(100, 200, 255)),
                                        );
                                        let kisa_url = if medya.url.len() > 60 {
                                            format!("{}...", &medya.url[..57])
                                        } else {
                                            medya.url.clone()
                                        };
                                        ui.label(
                                            egui::RichText::new(kisa_url)
                                                .italics()
                                                .color(egui::Color32::from_rgb(180, 180, 180)),
                                        );
                                    });
                                    ui.horizontal(|ui| {
                                        if ui.button("🌐 Tarayıcıda Aç").clicked() {
                                            let _ = std::process::Command::new("cmd")
                                                .args(["/c", "start", &medya.url])
                                                .spawn();
                                        }
                                        if ui.button("📋 URL Kopyala").clicked() {
                                            let url = medya.url.clone();
                                            ui.output_mut(|o| o.copied_text = url.clone());
                                        }
                                    });
                                }
                                MedyaTipi::Video => {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new("🎬 Üretilen Video (Veo 2)")
                                                .strong()
                                                .color(egui::Color32::from_rgb(255, 130, 180)),
                                        );
                                        let kisa_url = if medya.url.len() > 60 {
                                            format!("{}...", &medya.url[..57])
                                        } else {
                                            medya.url.clone()
                                        };
                                        ui.label(
                                            egui::RichText::new(kisa_url)
                                                .italics()
                                                .color(egui::Color32::from_rgb(180, 180, 180)),
                                        );
                                    });
                                    ui.horizontal(|ui| {
                                        if ui.button("▶ Videoyu İzle / Aç").clicked() {
                                            let _ = std::process::Command::new("cmd")
                                                .args(["/c", "start", &medya.url])
                                                .spawn();
                                        }
                                        if ui.button("📋 Video URL Kopyala").clicked() {
                                            let url = medya.url.clone();
                                            ui.output_mut(|o| o.copied_text = url.clone());
                                        }
                                    });
                                }
                            }
                        });
                        ui.add_space(4.0);
                    }
                });
                ui.add_space(4.0);
            }

            let kalan_genislik = ui.available_width();
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.son_yanit)
                        .desired_rows(12)
                        .desired_width(kalan_genislik),
                );
            });
        });

        // Canlı yenileme isteği
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_saglayici_modelleri() {
        let gemini_modeller = saglayici_modelleri("gemini");
        assert!(gemini_modeller.iter().any(|m| m.kod == "gemini-2.5-pro"));
        assert!(gemini_modeller.iter().any(|m| m.kod == "gemini-2.5-flash"));
        assert!(
            gemini_modeller
                .iter()
                .any(|m| m.kod == "gemini-2.0-flash-thinking")
        );
        assert!(
            gemini_modeller
                .iter()
                .any(|m| m.kod == "gemini-deep-research")
        );
        assert!(gemini_modeller.iter().any(|m| m.kod == "imagen-3"));
        assert!(gemini_modeller.iter().any(|m| m.kod == "veo-2"));
        assert!(gemini_modeller.iter().any(|m| m.kod == "gemini-1.5-pro"));
        assert!(gemini_modeller.iter().any(|m| m.kod == "gemini-1.5-flash"));

        let deepseek_modeller = saglayici_modelleri("deepseek");
        assert!(deepseek_modeller.iter().any(|m| m.kod == "deepseek-r1"));
        assert!(deepseek_modeller.iter().any(|m| m.kod == "deepseek-v3"));
        assert!(
            deepseek_modeller
                .iter()
                .any(|m| m.kod == "deepseek-r1-search")
        );
        assert!(deepseek_modeller.iter().any(|m| m.kod == "deepseek-coder"));

        let chatgpt_modeller = saglayici_modelleri("chatgpt");
        assert!(chatgpt_modeller.iter().any(|m| m.kod == "o3-mini-high"));
        assert!(chatgpt_modeller.iter().any(|m| m.kod == "o3-mini"));
        assert!(chatgpt_modeller.iter().any(|m| m.kod == "o1"));
        assert!(chatgpt_modeller.iter().any(|m| m.kod == "o1-pro"));
        assert!(chatgpt_modeller.iter().any(|m| m.kod == "gpt-4o"));
        assert!(chatgpt_modeller.iter().any(|m| m.kod == "gpt-4o-canvas"));
        assert!(chatgpt_modeller.iter().any(|m| m.kod == "searchgpt"));
        assert!(chatgpt_modeller.iter().any(|m| m.kod == "dall-e-3"));
        assert!(chatgpt_modeller.iter().any(|m| m.kod == "sora"));

        let qwen_modeller = saglayici_modelleri("qwen");
        assert!(qwen_modeller.iter().any(|m| m.kod == "qwen-2.5-max"));
        assert!(qwen_modeller.iter().any(|m| m.kod == "qwen-2.5-plus"));
        assert!(qwen_modeller.iter().any(|m| m.kod == "qwen-2.5-coder-32b"));
        assert!(qwen_modeller.iter().any(|m| m.kod == "qwq-32b-preview"));
        assert!(qwen_modeller.iter().any(|m| m.kod == "qwen-deepsearch"));
        assert!(qwen_modeller.iter().any(|m| m.kod == "wanx-2.1"));

        let kimi_modeller = saglayici_modelleri("kimi");
        assert!(kimi_modeller.iter().any(|m| m.kod == "kimi-k1.5-thinking"));
        assert!(kimi_modeller.iter().any(|m| m.kod == "kimi-k1.5"));
        assert!(kimi_modeller.iter().any(|m| m.kod == "kimi-explore"));
        assert!(kimi_modeller.iter().any(|m| m.kod == "moonshot-v1-128k"));

        let copilot_modeller = saglayici_modelleri("copilot");
        assert!(
            copilot_modeller
                .iter()
                .any(|m| m.kod == "copilot-think-deeper")
        );
        assert!(
            copilot_modeller
                .iter()
                .any(|m| m.kod == "copilot-pro-creative")
        );
        assert!(
            copilot_modeller
                .iter()
                .any(|m| m.kod == "copilot-pro-precise")
        );
        assert!(copilot_modeller.iter().any(|m| m.kod == "designer-dalle3"));

        let perplexity_modeller = saglayici_modelleri("perplexity");
        assert!(
            perplexity_modeller
                .iter()
                .any(|m| m.kod == "sonar-deep-research")
        );
        assert!(
            perplexity_modeller
                .iter()
                .any(|m| m.kod == "sonar-reasoning-pro")
        );
        assert!(perplexity_modeller.iter().any(|m| m.kod == "sonar-pro"));
        assert!(
            perplexity_modeller
                .iter()
                .any(|m| m.kod == "claude-3-5-sonnet")
        );

        let mistral_modeller = saglayici_modelleri("mistral");
        assert!(mistral_modeller.iter().any(|m| m.kod == "mistral-large-2"));
        assert!(mistral_modeller.iter().any(|m| m.kod == "pixtral-large"));
        assert!(mistral_modeller.iter().any(|m| m.kod == "codestral"));
        assert!(mistral_modeller.iter().any(|m| m.kod == "mistral-research"));

        let zai_modeller = saglayici_modelleri("z.ai");
        assert!(zai_modeller.iter().any(|m| m.kod == "glm-4-plus"));
        assert!(zai_modeller.iter().any(|m| m.kod == "glm-4-long"));
        assert!(zai_modeller.iter().any(|m| m.kod == "glm-4-alltools"));
        assert!(zai_modeller.iter().any(|m| m.kod == "cogvideox"));

        let auto_modeller = saglayici_modelleri("auto");
        assert!(auto_modeller.iter().any(|m| m.kod == "otomatik"));

        assert_eq!(varsayilan_model("gemini"), "gemini-2.5-pro");
        assert_eq!(varsayilan_model("deepseek"), "deepseek-r1");
        assert_eq!(varsayilan_model("chatgpt"), "o3-mini-high");
        assert_eq!(varsayilan_model("qwen"), "qwen-2.5-max");
        assert_eq!(varsayilan_model("kimi"), "kimi-k1.5-thinking");
        assert_eq!(varsayilan_model("copilot"), "copilot-think-deeper");
        assert_eq!(varsayilan_model("perplexity"), "sonar-deep-research");
        assert_eq!(varsayilan_model("mistral"), "mistral-large-2");
        assert_eq!(varsayilan_model("z.ai"), "glm-4-plus");
        assert_eq!(varsayilan_model("auto"), "otomatik");
        assert_eq!(varsayilan_model("diger"), "varsayilan");
    }

    #[test]
    fn test_amiral_model() {
        assert_eq!(
            amiral_model("gemini", UretimModu::GorselUretimi),
            Some("imagen-3")
        );
        assert_eq!(
            amiral_model("gemini", UretimModu::VideoHazirlama),
            Some("veo-2")
        );
        assert_eq!(
            amiral_model("gemini", UretimModu::Dusunme),
            Some("gemini-3.1-pro")
        );
        assert_eq!(
            amiral_model("gemini", UretimModu::DerinArastirma),
            Some("gemini-deep-research")
        );
        assert_eq!(
            amiral_model("gemini", UretimModu::Sohbet),
            Some("gemini-3.8-flash")
        );

        assert_eq!(
            amiral_model("deepseek", UretimModu::Dusunme),
            Some("deepseek-r1")
        );
        assert_eq!(
            amiral_model("deepseek", UretimModu::DerinArastirma),
            Some("deepseek-r1-search")
        );
        assert_eq!(
            amiral_model("deepseek", UretimModu::Sohbet),
            Some("deepseek-v3")
        );

        assert_eq!(
            amiral_model("chatgpt", UretimModu::GorselUretimi),
            Some("dall-e-3")
        );
        assert_eq!(
            amiral_model("chatgpt", UretimModu::VideoHazirlama),
            Some("sora")
        );
        assert_eq!(
            amiral_model("chatgpt", UretimModu::Dusunme),
            Some("o3-mini-high")
        );
        assert_eq!(
            amiral_model("chatgpt", UretimModu::DerinArastirma),
            Some("searchgpt")
        );
        assert_eq!(amiral_model("chatgpt", UretimModu::Sohbet), Some("gpt-4o"));

        assert_eq!(
            amiral_model("qwen", UretimModu::GorselUretimi),
            Some("wanx-2.1")
        );
        assert_eq!(
            amiral_model("qwen", UretimModu::VideoHazirlama),
            Some("wanx-2.1")
        );
        assert_eq!(
            amiral_model("qwen", UretimModu::Dusunme),
            Some("qwq-32b-preview")
        );
        assert_eq!(
            amiral_model("qwen", UretimModu::DerinArastirma),
            Some("qwen-deepsearch")
        );
        assert_eq!(
            amiral_model("qwen", UretimModu::Sohbet),
            Some("qwen-2.5-max")
        );

        assert_eq!(
            amiral_model("kimi", UretimModu::Dusunme),
            Some("kimi-k1.5-thinking")
        );
        assert_eq!(
            amiral_model("kimi", UretimModu::DerinArastirma),
            Some("kimi-explore")
        );
        assert_eq!(amiral_model("kimi", UretimModu::Sohbet), Some("kimi-k1.5"));

        assert_eq!(
            amiral_model("copilot", UretimModu::GorselUretimi),
            Some("designer-dalle3")
        );
        assert_eq!(
            amiral_model("copilot", UretimModu::Dusunme),
            Some("copilot-think-deeper")
        );
        assert_eq!(
            amiral_model("copilot", UretimModu::DerinArastirma),
            Some("copilot-pro-precise")
        );
        assert_eq!(
            amiral_model("copilot", UretimModu::Sohbet),
            Some("copilot-pro-creative")
        );

        assert_eq!(
            amiral_model("perplexity", UretimModu::Dusunme),
            Some("sonar-reasoning-pro")
        );
        assert_eq!(
            amiral_model("perplexity", UretimModu::DerinArastirma),
            Some("sonar-deep-research")
        );
        assert_eq!(
            amiral_model("perplexity", UretimModu::Sohbet),
            Some("sonar-pro")
        );

        assert_eq!(
            amiral_model("mistral", UretimModu::GorselUretimi),
            Some("pixtral-large")
        );
        assert_eq!(
            amiral_model("mistral", UretimModu::Dusunme),
            Some("mistral-large-2")
        );
        assert_eq!(
            amiral_model("mistral", UretimModu::DerinArastirma),
            Some("mistral-research")
        );
        assert_eq!(
            amiral_model("mistral", UretimModu::Sohbet),
            Some("mistral-large-2")
        );

        assert_eq!(
            amiral_model("z.ai", UretimModu::VideoHazirlama),
            Some("cogvideox")
        );
        assert_eq!(
            amiral_model("z.ai", UretimModu::Dusunme),
            Some("glm-4-plus")
        );
        assert_eq!(
            amiral_model("z.ai", UretimModu::DerinArastirma),
            Some("glm-4-alltools")
        );
        assert_eq!(amiral_model("z.ai", UretimModu::Sohbet), Some("glm-4-plus"));

        assert_eq!(
            amiral_model("auto", UretimModu::GorselUretimi),
            Some("imagen-3")
        );
        assert_eq!(
            amiral_model("auto", UretimModu::VideoHazirlama),
            Some("veo-2")
        );
        assert_eq!(
            amiral_model("auto", UretimModu::Dusunme),
            Some("gemini-3.1-pro")
        );
        assert_eq!(
            amiral_model("auto", UretimModu::DerinArastirma),
            Some("gemini-deep-research")
        );
        assert_eq!(amiral_model("auto", UretimModu::Sohbet), Some("otomatik"));
    }

    #[test]
    fn test_istem_cerceveleme() {
        let istem = "Siberpunk bir şehir manzarası";

        let gorsel = istem_cercevele(istem, UretimModu::GorselUretimi, DusunmeSeviyesi::Orta);
        assert!(gorsel.starts_with("Lütfen şu sahneyi yüksek kalitede, fotogerçekçi ve ayrıntılı bir görsel olarak oluştur (Generate Image):"));
        assert!(gorsel.contains(istem));

        let video = istem_cercevele(istem, UretimModu::VideoHazirlama, DusunmeSeviyesi::Orta);
        assert!(video.starts_with(
            "Lütfen şu sahne için sinematik kalitede akıcı bir video oluştur (Generate Video):"
        ));
        assert!(video.contains(istem));

        let dusunme = istem_cercevele(istem, UretimModu::Dusunme, DusunmeSeviyesi::Derin);
        assert!(dusunme.starts_with("[Düşünme Seviyesi: Derin] Lütfen soruyu adım adım derinlemesine düşünerek ve akıl yürüterek yanıtla:"));
        assert!(dusunme.contains(istem));

        let arastirma = istem_cercevele(istem, UretimModu::DerinArastirma, DusunmeSeviyesi::Dusuk);
        assert!(arastirma.starts_with("[Derin Araştırma Modu] Lütfen konuyu tüm boyutlarıyla, teknik detaylar ve alt başlıklarla derinlemesine araştırıp raporla:"));
        assert!(arastirma.contains(istem));

        let sohbet = istem_cercevele(istem, UretimModu::Sohbet, DusunmeSeviyesi::Orta);
        assert_eq!(sohbet, istem);
    }

    #[test]
    fn test_medyalari_ayikla() {
        let metin = r#"
İşte istenen çıktılar:

![Üretilen Görsel](https://lh3.googleusercontent.com/test-image-123.jpg)

Animasyon videosu:
[Üretilen Video](https://storage.googleapis.com/veo2/sample-video.mp4)
"#;
        let medyalar = medyalari_ayikla(metin);
        assert_eq!(medyalar.len(), 2);

        assert_eq!(medyalar[0].tip, MedyaTipi::Gorsel);
        assert_eq!(medyalar[0].baslik, "Üretilen Görsel");
        assert_eq!(
            medyalar[0].url,
            "https://lh3.googleusercontent.com/test-image-123.jpg"
        );

        assert_eq!(medyalar[1].tip, MedyaTipi::Video);
        assert_eq!(medyalar[1].baslik, "Üretilen Video");
        assert_eq!(
            medyalar[1].url,
            "https://storage.googleapis.com/veo2/sample-video.mp4"
        );
    }

    #[test]
    fn test_medyalari_ayikla_bos_ve_gecersiz() {
        let metin = "Herhangi bir medya içermeyen standart sohbet yanıtı.";
        let medyalar = medyalari_ayikla(metin);
        assert!(medyalar.is_empty());

        let gecersiz = "![svg ikon](data:image/svg+xml;base64,xxx)";
        let medyalar_gecersiz = medyalari_ayikla(gecersiz);
        assert!(medyalar_gecersiz.is_empty());
    }

    #[test]
    fn test_base64_encode() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64_encode(b"Hello, World!"), "SGVsbG8sIFdvcmxkIQ==");
    }

    #[test]
    fn test_dosya_mime_belirle() {
        assert_eq!(dosya_mime_belirle("resim.png"), "image/png");
        assert_eq!(dosya_mime_belirle("FOTO.JPG"), "image/jpeg");
        assert_eq!(dosya_mime_belirle("animasyon.webp"), "image/webp");
        assert_eq!(dosya_mime_belirle("klip.mp4"), "video/mp4");
        assert_eq!(dosya_mime_belirle("film.webm"), "video/webm");
        assert_eq!(dosya_mime_belirle("video.mov"), "video/quicktime");
        assert_eq!(dosya_mime_belirle("rapor.pdf"), "application/pdf");
        assert_eq!(dosya_mime_belirle("not.txt"), "text/plain");
        assert_eq!(dosya_mime_belirle("veri.json"), "application/json");
        assert_eq!(dosya_mime_belirle("dokuman.md"), "text/markdown");
        assert_eq!(dosya_mime_belirle("tablo.csv"), "text/csv");
        assert_eq!(dosya_mime_belirle("arsiv.zip"), "application/zip");
        assert_eq!(
            dosya_mime_belirle("bilinmeyen.xyz"),
            "application/octet-stream"
        );
    }
}
