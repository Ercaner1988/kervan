//! 🎙 Ses modu: yazıya dökme (sağlayıcı sekmesine ses dosyası yüklenir) ve
//! seslendirme (Windows'un yerleşik konuşma sentezi, ek bağımlılık yok).

use std::sync::mpsc;

use eframe::egui;
use kervan_akis::audio_video::{
    KayitliMedya, SesFormati, SesTranskripsiyonIstegi, SeslendirmeIstegi, ZamanAraligi,
    kavram_sozlugu_filtresi_uygula,
};

use crate::{Eylem, Is, renk};

const BICIMLER: [SesFormati; 5] = [
    SesFormati::Wav,
    SesFormati::Mp3,
    SesFormati::M4a,
    SesFormati::Ogg,
    SesFormati::Webm,
];
const DILLER: [(&str, &str); 4] = [
    ("tr", "Türkçe"),
    ("en", "İngilizce"),
    ("ar", "Arapça"),
    ("de", "Almanca"),
];

pub struct SesPaneli {
    dosya: String,
    bicim: SesFormati,
    dil: &'static str,
    sozluk_uygula: bool,
    pub sozluk: Vec<(String, String)>,
    yeni: (String, String),
    transkript: Vec<ZamanAraligi>,
    ham: String,
    bekliyor: bool,
    metin: String,
    ses: String,
    sesler: Vec<String>,
    sesler_rx: Option<mpsc::Receiver<Vec<String>>>,
    hiz: f32,
    seslendirme_rx: Option<mpsc::Receiver<Result<KayitliMedya, String>>>,
    biten: Option<KayitliMedya>,
    secici_rx: Option<mpsc::Receiver<String>>,
    durum: Option<(String, egui::Color32)>,
}

impl Default for SesPaneli {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            gonder(&tx, sesleri_listele());
        });
        Self {
            dosya: String::new(),
            bicim: SesFormati::M4a,
            dil: "tr",
            sozluk_uygula: true,
            sozluk: Vec::new(),
            yeni: Default::default(),
            transkript: Vec::new(),
            ham: String::new(),
            bekliyor: false,
            metin: String::new(),
            ses: String::new(),
            sesler: Vec::new(),
            sesler_rx: Some(rx),
            hiz: 1.0,
            seslendirme_rx: None,
            biten: None,
            secici_rx: None,
            durum: None,
        }
    }
}

impl SesPaneli {
    pub fn ciz(&mut self, ui: &mut egui::Ui) -> Option<Eylem> {
        if let Some(liste) = self.sesler_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.sesler = liste;
            self.sesler_rx = None;
        }
        let mut eylem = None;
        ui.columns(2, |k| {
            eylem = self.yaziya_dok_karti(&mut k[0]);
            self.seslendir_karti(&mut k[1]);
        });
        if let Some((m, r)) = &self.durum {
            ui.colored_label(*r, m);
        }
        ui.add_space(8.0);
        ui.columns(2, |k| {
            if let Some(e) = self.transkript_karti(&mut k[0]) {
                eylem = Some(e);
            }
            self.sozluk_karti(&mut k[1]);
        });
        eylem
    }

    fn yaziya_dok_karti(&mut self, ui: &mut egui::Ui) -> Option<Eylem> {
        let mut eylem = None;
        ui.group(|ui| {
            ui.heading("🎙 Yazıya dök");
            ui.label("Dosya");
            if let Some(yol) = self.secici_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
                self.secici_rx = None;
                if !yol.is_empty() {
                    self.dosya = yol;
                }
            }
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.dosya);
                if ui
                    .add_enabled(self.secici_rx.is_none(), egui::Button::new("Seç…"))
                    .clicked()
                {
                    let (tx, rx) = mpsc::channel();
                    self.secici_rx = Some(rx);
                    std::thread::spawn(move || {
                        gonder(&tx, ses_dosyasi_sec());
                    });
                }
                if let Some(b) = BICIMLER
                    .iter()
                    .find(|b| self.dosya.to_ascii_lowercase().ends_with(b.uzanti()))
                {
                    self.bicim = *b;
                }
            });
            ui.horizontal(|ui| {
                egui::ComboBox::from_label("Biçim")
                    .selected_text(self.bicim.uzanti().to_ascii_uppercase())
                    .show_ui(ui, |ui| {
                        for b in BICIMLER {
                            ui.selectable_value(
                                &mut self.bicim,
                                b,
                                b.uzanti().to_ascii_uppercase(),
                            );
                        }
                    });
                let secili = DILLER.iter().find(|d| d.0 == self.dil).map_or("", |d| d.1);
                egui::ComboBox::from_label("Dil")
                    .selected_text(format!("{} — {secili}", self.dil))
                    .show_ui(ui, |ui| {
                        for (kod, ad) in DILLER {
                            ui.selectable_value(&mut self.dil, kod, format!("{kod} — {ad}"));
                        }
                    });
            });
            ui.checkbox(&mut self.sozluk_uygula, "Kavram sözlüğünü uygula");
            let hazir = !self.bekliyor && std::path::Path::new(self.dosya.trim()).is_file();
            let etiket = if self.bekliyor {
                "⏳ Bekleniyor…"
            } else {
                "▶ Yazıya dök"
            };
            if ui.add_enabled(hazir, egui::Button::new(etiket)).clicked() {
                let istek = SesTranskripsiyonIstegi {
                    dosya_yolu: self.dosya.trim().to_string(),
                    format: self.bicim,
                    dil: self.dil.to_string(),
                    kavram_sozlugu: (self.sozluk_uygula && !self.sozluk.is_empty())
                        .then(|| self.sozluk.iter().map(|(_, d)| d.clone()).collect()),
                };
                self.bekliyor = true;
                self.durum = None;
                eylem = Some(Eylem::Gonder {
                    is: Is::Transkript,
                    istem: transkript_istemi(&istek),
                    dosya: Some(istek.dosya_yolu),
                    video: false,
                });
            }
        });
        eylem
    }

    fn seslendir_karti(&mut self, ui: &mut egui::Ui) {
        if let Some(rx) = &self.seslendirme_rx
            && let Ok(sonuc) = rx.try_recv()
        {
            self.seslendirme_rx = None;
            self.durum = Some(match &sonuc {
                Ok(m) => (format!("🔊 Kaydedildi: {}", m.dosya_yolu), renk::BASARI),
                Err(e) => (format!("Seslendirme başarısız: {e}"), renk::HATA),
            });
            self.biten = sonuc.ok();
        }
        ui.group(|ui| {
            ui.heading("🔊 Seslendir");
            ui.label("Metin");
            ui.add(egui::TextEdit::multiline(&mut self.metin).desired_rows(3));
            let ses_adi = if self.ses.is_empty() {
                "Varsayılan ses"
            } else {
                self.ses.as_str()
            };
            egui::ComboBox::from_label("Ses")
                .selected_text(ses_adi)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.ses, String::new(), "Varsayılan ses");
                    for s in &self.sesler {
                        ui.selectable_value(&mut self.ses, s.clone(), s);
                    }
                });
            ui.add(
                egui::Slider::new(&mut self.hiz, 0.5..=2.0)
                    .text("Konuşma hızı")
                    .suffix("×"),
            );
            ui.colored_label(renk::SOLUK, "Çıktı: WAV (Windows konuşma sentezi)");
            let calisiyor = self.seslendirme_rx.is_some();
            let hazir = !calisiyor && !self.metin.trim().is_empty();
            if ui
                .add_enabled(
                    hazir,
                    egui::Button::new(if calisiyor {
                        "⏳ Seslendiriliyor…"
                    } else {
                        "▶ Seslendir"
                    }),
                )
                .clicked()
            {
                let istek = SeslendirmeIstegi {
                    metin: self.metin.trim().to_string(),
                    ses_modeli: self.ses.clone(),
                    konusma_hizi: self.hiz,
                    cikti_formati: SesFormati::Wav,
                };
                let (tx, rx) = mpsc::channel();
                self.seslendirme_rx = Some(rx);
                std::thread::spawn(move || {
                    gonder(&tx, seslendir(&istek));
                });
            }
        });
    }

    fn transkript_karti(&mut self, ui: &mut egui::Ui) -> Option<Eylem> {
        let mut eylem = None;
        ui.group(|ui| {
            ui.heading("Transkript");
            if self.ham.is_empty() {
                ui.colored_label(
                    renk::SOLUK,
                    "Bir ses dosyası yazıya dökülünce burada görünür.",
                );
                return;
            }
            egui::ScrollArea::vertical()
                .max_height(220.0)
                .show(ui, |ui| {
                    if self.transkript.is_empty() {
                        ui.label(&self.ham);
                    }
                    for z in &self.transkript {
                        ui.horizontal_wrapped(|ui| {
                            ui.colored_label(
                                renk::VURGU,
                                egui::RichText::new(sure_araligi(z)).small(),
                            );
                            ui.label(&z.metin);
                        });
                    }
                });
            ui.horizontal(|ui| {
                if ui.button("📋 Kopyala").clicked() {
                    ui.ctx().copy_text(self.ham.clone());
                }
                if ui.button("↪ Sohbete ekle").clicked() {
                    eylem = Some(Eylem::SohbeteEkle(self.ham.clone()));
                }
            });
        });
        eylem
    }

    fn sozluk_karti(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.heading("📖 Kavram sözlüğü");
            let mut sil = None;
            for (i, (y, d)) in self.sozluk.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.colored_label(renk::SOLUK, y);
                    ui.colored_label(renk::SOLUK, "→");
                    ui.label(d);
                    if ui.small_button("✕").clicked() {
                        sil = Some(i);
                    }
                });
            }
            if let Some(i) = sil {
                self.sozluk.remove(i);
            }
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.yeni.0)
                        .hint_text("yanlış")
                        .desired_width(90.0),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.yeni.1)
                        .hint_text("doğru")
                        .desired_width(90.0),
                );
            });
            let (y, d) = (self.yeni.0.trim(), self.yeni.1.trim());
            if ui
                .add_enabled(!y.is_empty() && !d.is_empty(), egui::Button::new("+ Ekle"))
                .clicked()
            {
                self.sozluk.push((y.to_string(), d.to_string()));
                self.yeni = Default::default();
            }
        });
    }

    /// Sağlayıcının yazıya döküm yanıtı.
    pub fn transkript_sonucu(&mut self, yanit: Result<String, String>) {
        self.bekliyor = false;
        match yanit {
            Ok(metin) => {
                let metin = if self.sozluk_uygula {
                    kavram_sozlugu_filtresi_uygula(&metin, &self.sozluk)
                } else {
                    metin
                };
                self.transkript = segmentler(&metin);
                self.ham = metin;
            }
            Err(e) => self.durum = Some((format!("Yazıya dökme başarısız: {e}"), renk::HATA)),
        }
    }

    pub(crate) fn seslendirme_bitti(&mut self) -> Option<KayitliMedya> {
        self.biten.take()
    }
}

/// Arka plan işinin sonucunu panele iletir. Alıcı yoksa panel kapanmıştır;
/// sonucun gidecek yeri kalmadığı için yalnız iz bırakılır.
fn gonder<T>(tx: &mpsc::Sender<T>, deger: T) {
    if tx.send(deger).is_err() {
        eprintln!("ses paneli kapanmış, arka plan sonucu atıldı");
    }
}

fn transkript_istemi(i: &SesTranskripsiyonIstegi) -> String {
    let mut s = format!(
        "Ekteki {} ses dosyasını '{}' dilinde kelimesi kelimesine yazıya dök. Her satırı \
         [dd:ss-dd:ss] metin biçiminde zaman damgasıyla yaz; açıklama ya da özet ekleme.",
        i.format.uzanti().to_ascii_uppercase(),
        i.dil
    );
    if let Some(terimler) = &i.kavram_sozlugu {
        s.push_str(&format!(
            " Şu terimleri bu yazımla kullan: {}.",
            terimler.join(", ")
        ));
    }
    s
}

fn saniye(s: &str) -> Option<f32> {
    let (dk, sn) = s.trim().split_once(':')?;
    Some(dk.parse::<f32>().ok()? * 60.0 + sn.parse::<f32>().ok()?)
}

/// `[00:04-00:09] metin` satırlarını zaman aralıklarına ayırır; biçime uymayan
/// satırlar atlanır (ham metin ayrıca saklanır).
pub fn segmentler(metin: &str) -> Vec<ZamanAraligi> {
    metin
        .lines()
        .filter_map(|l| {
            let (zaman, geri) = l.trim().strip_prefix('[')?.split_once(']')?;
            let (a, b) = zaman.split_once(['-', '–'])?;
            Some(ZamanAraligi {
                baslangic_sn: saniye(a)?,
                bitis_sn: saniye(b)?,
                metin: geri.trim().to_string(),
            })
        })
        .collect()
}

fn sure_araligi(z: &ZamanAraligi) -> String {
    let f = |s: f32| format!("{:02}:{:02}", s as u32 / 60, s as u32 % 60);
    format!("{}–{}", f(z.baslangic_sn), f(z.bitis_sn))
}

/// Windows konuşma sentezi (System.Speech). Metin ve ayarlar ortam değişkeniyle
/// geçer: komut satırına gömülmediği için tırnak/kaçış sorunu yoktur.
const TTS_BETIGI: &str = "Add-Type -AssemblyName System.Speech; \
    $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; \
    if ($env:KERVAN_TTS_SES) { $s.SelectVoice($env:KERVAN_TTS_SES) }; \
    $s.Rate = [int]$env:KERVAN_TTS_HIZ; \
    $s.SetOutputToWaveFile($env:KERVAN_TTS_YOL); \
    $s.Speak($env:KERVAN_TTS_METIN); $s.Dispose()";

fn powershell(betik: &str) -> std::process::Command {
    let mut c = std::process::Command::new("powershell");
    c.args(["-NoProfile", "-NonInteractive", "-Command", betik]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: konsol penceresi açılmasın
    }
    c
}

/// 0.5×–2.0× hızı SAPI'nin -10..10 ölçeğine çevirir (1.0× = 0).
fn sapi_hizi(hiz: f32) -> i32 {
    let r = if hiz >= 1.0 {
        (hiz - 1.0) * 10.0
    } else {
        (hiz - 1.0) * 20.0
    };
    (r.round() as i32).clamp(-10, 10)
}

fn seslendir(i: &SeslendirmeIstegi) -> Result<KayitliMedya, String> {
    let dizin = std::env::var_os("USERPROFILE")
        .map(|p| std::path::PathBuf::from(p).join("Music").join("Kervan"))
        .unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&dizin).map_err(|e| e.to_string())?;
    let damga = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let yol = dizin.join(format!("seslendirme-{damga}.{}", i.cikti_formati.uzanti()));
    let cikti = powershell(TTS_BETIGI)
        .env("KERVAN_TTS_METIN", &i.metin)
        .env("KERVAN_TTS_SES", &i.ses_modeli)
        .env("KERVAN_TTS_HIZ", sapi_hizi(i.konusma_hizi).to_string())
        .env("KERVAN_TTS_YOL", &yol)
        .output()
        .map_err(|e| format!("powershell çalıştırılamadı: {e}"))?;
    if !cikti.status.success() {
        return Err(String::from_utf8_lossy(&cikti.stderr).trim().to_string());
    }
    let boyut_bayt = std::fs::metadata(&yol).map_err(|e| e.to_string())?.len();
    let baslik: String = i.metin.chars().take(40).collect();
    Ok(KayitliMedya {
        id: format!("ses-{damga}"),
        tur: "ses",
        baslik,
        dosya_yolu: yol.to_string_lossy().into_owned(),
        boyut_bayt,
    })
}

/// Windows dosya seçicisi; iptalde boş döner.
fn ses_dosyasi_sec() -> String {
    let betik = "[Console]::OutputEncoding = [Text.Encoding]::UTF8; Add-Type -AssemblyName System.Windows.Forms;         $f = New-Object System.Windows.Forms.OpenFileDialog; $f.Title = 'Kervan - Ses dosyası seç';         $f.Filter = 'Ses (*.wav, *.mp3, *.m4a, *.ogg, *.webm)|*.wav;*.mp3;*.m4a;*.ogg;*.webm|Tüm dosyalar|*.*';         if ($f.ShowDialog() -eq 'OK') { $f.FileName }";
    powershell(betik)
        .output()
        .map(|c| String::from_utf8_lossy(&c.stdout).trim().to_string())
        .unwrap_or_default()
}

/// Kurulu Windows seslerinin adları; başarısızsa boş (varsayılan ses kullanılır).
fn sesleri_listele() -> Vec<String> {
    let betik = "[Console]::OutputEncoding = [Text.Encoding]::UTF8; Add-Type -AssemblyName System.Speech; \
        (New-Object System.Speech.Synthesis.SpeechSynthesizer).GetInstalledVoices() | \
        ForEach-Object { $_.VoiceInfo.Name }";
    powershell(betik)
        .output()
        .map(|c| {
            String::from_utf8_lossy(&c.stdout)
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zaman_damgali_satirlar_ayrilir() {
        let s = segmentler("[00:00-00:04] Weber aklîleşme\nnot\n[01:05–01:09.5] devam");
        assert_eq!(s.len(), 2);
        assert_eq!((s[1].baslangic_sn, s[1].bitis_sn), (65.0, 69.5));
        assert_eq!(s[0].metin, "Weber aklîleşme");
        assert_eq!(sure_araligi(&s[1]), "01:05–01:09");
    }

    #[test]
    fn hiz_sapi_olcegine_iner() {
        assert_eq!(
            [0.5, 1.0, 1.5, 2.0, 3.0].map(sapi_hizi),
            [-10, 0, 5, 10, 10]
        );
    }

    #[test]
    fn istem_sozlugu_tasir() {
        let i = SesTranskripsiyonIstegi {
            dosya_yolu: "a.m4a".into(),
            format: SesFormati::M4a,
            dil: "tr".into(),
            kavram_sozlugu: Some(vec!["Weber".into()]),
        };
        let s = transkript_istemi(&i);
        assert!(
            s.contains("M4A") && s.contains("'tr'") && s.contains("Weber"),
            "{s}"
        );
    }
}
