//! 🎬 Video modu: üretim isteği, görev izleme (`MedyaYoneticisi`) ve sonuç bağlantısı.
//!
//! Sağlayıcı sekmesi ilerleme yüzdesi vermiyor; görev "Üretiliyor" iken yüzde
//! uydurulmaz, dönen gösterge çizilir.

use eframe::egui;
use kervan_akis::audio_video::{
    KayitliMedya, MedyaYoneticisi, VideoDurumu, VideoEnBoyOrani, VideoGorevi, VideoUretimIstegi,
};

use crate::{Eylem, Is, renk};

const ORANLAR: [VideoEnBoyOrani; 4] = [
    VideoEnBoyOrani::Kare1x1,
    VideoEnBoyOrani::Yatay16x9,
    VideoEnBoyOrani::Dikey9x16,
    VideoEnBoyOrani::Sinematik21x9,
];
const KAMERA: [&str; 5] = ["Yok", "Slow Zoom In", "Pan Left", "Pan Right", "Dolly Out"];

pub struct VideoPaneli {
    istem: String,
    negatif: String,
    oran: VideoEnBoyOrani,
    sure: u32,
    cozunurluk: &'static str,
    kamera: &'static str,
    /// Görev kimlikleri, eklenme sırasıyla (yönetici sırasız tutuyor).
    sira: Vec<String>,
    sayac: u32,
}

impl Default for VideoPaneli {
    fn default() -> Self {
        Self {
            istem: String::new(),
            negatif: String::new(),
            oran: VideoEnBoyOrani::Yatay16x9,
            sure: 8,
            cozunurluk: "1080p",
            kamera: "Yok",
            sira: Vec::new(),
            sayac: 0,
        }
    }
}

impl VideoPaneli {
    pub fn ciz(&mut self, ui: &mut egui::Ui, medya: &mut MedyaYoneticisi) -> Option<Eylem> {
        let mut eylem = None;
        ui.columns(2, |k| {
            eylem = self.istek_karti(&mut k[0], medya);
            if let Some(e) = self.gorevler_karti(&mut k[1], medya) {
                eylem = Some(e);
            }
        });
        eylem
    }

    fn istek_karti(&mut self, ui: &mut egui::Ui, medya: &mut MedyaYoneticisi) -> Option<Eylem> {
        let mut eylem = None;
        ui.group(|ui| {
            ui.heading("🎬 Yeni video");
            ui.label("İstem");
            ui.add(egui::TextEdit::multiline(&mut self.istem).desired_rows(3));
            ui.label("Negatif istem (isteğe bağlı)");
            ui.text_edit_singleline(&mut self.negatif);
            ui.label("En-boy oranı");
            ui.horizontal(|ui| {
                for o in ORANLAR {
                    ui.selectable_value(&mut self.oran, o, o.etiket());
                }
            });
            ui.horizontal(|ui| {
                egui::ComboBox::from_label("Süre")
                    .selected_text(format!("{} sn", self.sure))
                    .show_ui(ui, |ui| {
                        for s in [4, 6, 8] {
                            ui.selectable_value(&mut self.sure, s, format!("{s} sn"));
                        }
                    });
                egui::ComboBox::from_label("Çözünürlük")
                    .selected_text(self.cozunurluk)
                    .show_ui(ui, |ui| {
                        for c in ["720p", "1080p"] {
                            ui.selectable_value(&mut self.cozunurluk, c, c);
                        }
                    });
            });
            egui::ComboBox::from_label("Kamera hareketi")
                .selected_text(self.kamera)
                .show_ui(ui, |ui| {
                    for k in KAMERA {
                        ui.selectable_value(&mut self.kamera, k, k);
                    }
                });
            if ui
                .add_enabled(
                    !self.istem.trim().is_empty(),
                    egui::Button::new("▶ Video üret"),
                )
                .clicked()
            {
                let istek = VideoUretimIstegi {
                    istem: self.istem.trim().to_string(),
                    negatif_istem: Some(self.negatif.trim().to_string()).filter(|s| !s.is_empty()),
                    en_boy_orani: self.oran,
                    sure_saniye: self.sure,
                    cozunurluk: self.cozunurluk,
                    kamera_hareketi: Some(self.kamera.to_string()).filter(|k| k != "Yok"),
                };
                self.sayac += 1;
                eylem = Some(self.baslat(medya, format!("video-{}", self.sayac), istek));
            }
        });
        eylem
    }

    /// Görevi yöneticiye kaydedip gönderme eylemini döndürür.
    fn baslat(
        &mut self,
        medya: &mut MedyaYoneticisi,
        id: String,
        istek: VideoUretimIstegi,
    ) -> Eylem {
        let istem = video_istemi(&istek);
        let mut g = VideoGorevi::yeni(id.clone(), istek);
        g.ilerleme_guncelle(0);
        medya.video_gorevi_ekle(g);
        if !self.sira.contains(&id) {
            self.sira.push(id.clone());
        }
        Eylem::Gonder {
            is: Is::Video(id),
            istem,
            dosya: None,
            video: true,
        }
    }

    fn gorevler_karti(&mut self, ui: &mut egui::Ui, medya: &mut MedyaYoneticisi) -> Option<Eylem> {
        let mut yeniden = None;
        ui.group(|ui| {
            ui.heading(format!("Görevler · {}", self.sira.len()));
            if self.sira.is_empty() {
                ui.colored_label(renk::SOLUK, "Gönderilen videolar burada izlenir.");
            }
            for id in self.sira.iter().rev() {
                let Some(g) = medya.video_gorevi_mut(id) else {
                    continue;
                };
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        let baslik: String = g.istek.istem.chars().take(48).collect();
                        ui.label(baslik);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            durum_ciz(ui, &g.durum);
                        });
                    });
                    let i = &g.istek;
                    ui.colored_label(
                        renk::SOLUK,
                        egui::RichText::new(format!(
                            "{} · {} sn · {}",
                            i.en_boy_orani.etiket(),
                            i.sure_saniye,
                            i.cozunurluk
                        ))
                        .small(),
                    );
                    match &g.durum {
                        VideoDurumu::Tamamlandi { video_url, .. } => {
                            ui.horizontal(|ui| {
                                if ui.button("▶ Aç").clicked() {
                                    crate::ac(ui.ctx(), video_url);
                                }
                                if ui.button("📋 Bağlantıyı kopyala").clicked() {
                                    ui.ctx().copy_text(video_url.clone());
                                }
                            });
                        }
                        VideoDurumu::Basarisiz { hata_mesaji } => {
                            ui.colored_label(renk::SOLUK, egui::RichText::new(hata_mesaji).small());
                            if ui.button("↻ Yeniden dene").clicked() {
                                yeniden = Some((id.clone(), g.istek.clone()));
                            }
                        }
                        _ => {}
                    }
                });
            }
        });
        yeniden.map(|(id, istek)| self.baslat(medya, id, istek))
    }

    /// Sağlayıcı yanıtı: bağlantı varsa görev tamamlanır ve arşive girecek kayıt döner.
    pub fn sonuc(
        &mut self,
        medya: &mut MedyaYoneticisi,
        id: &str,
        yanit: Result<String, String>,
    ) -> Option<KayitliMedya> {
        let g = medya.video_gorevi_mut(id)?;
        match yanit.as_deref().map(ilk_baglanti) {
            Ok(Some(url)) => {
                g.tamamla(url, None);
                Some(KayitliMedya {
                    id: id.to_string(),
                    tur: "video",
                    baslik: g.istek.istem.chars().take(40).collect(),
                    dosya_yolu: url.to_string(),
                    boyut_bayt: 0,
                })
            }
            Ok(None) => {
                let ozet: String = yanit.unwrap_or_default().chars().take(120).collect();
                g.hata_ile_kapat(format!("Yanıtta video bağlantısı yok: {ozet}"));
                None
            }
            Err(e) => {
                g.hata_ile_kapat(e.clone());
                None
            }
        }
    }
}

fn durum_ciz(ui: &mut egui::Ui, d: &VideoDurumu) {
    match d {
        VideoDurumu::Kuyrukta { sira } => {
            ui.colored_label(renk::SOLUK, format!("⏳ Kuyrukta · sıra {sira}"));
        }
        VideoDurumu::Uretiliyor { ilerleme_yuzdesi } => {
            if *ilerleme_yuzdesi > 0 {
                ui.colored_label(renk::VURGU, format!("Üretiliyor · %{ilerleme_yuzdesi}"));
            } else {
                ui.colored_label(renk::VURGU, "Üretiliyor");
                ui.spinner();
            }
        }
        VideoDurumu::Tamamlandi { .. } => {
            ui.colored_label(renk::BASARI, "✓ Tamamlandı");
        }
        VideoDurumu::Basarisiz { .. } => {
            ui.colored_label(renk::HATA, "✕ Başarısız");
        }
    }
}

fn video_istemi(i: &VideoUretimIstegi) -> String {
    let mut s = format!(
        "{}. En-boy oranı {}, süre {} sn, çözünürlük {}.",
        i.istem.trim_end_matches('.'),
        i.en_boy_orani.etiket(),
        i.sure_saniye,
        i.cozunurluk
    );
    if let Some(k) = &i.kamera_hareketi {
        s.push_str(&format!(" Kamera hareketi: {k}."));
    }
    if let Some(n) = &i.negatif_istem {
        s.push_str(&format!(" Şunlardan kaçın: {n}."));
    }
    s
}

/// Yanıttaki ilk http(s) bağlantısı (Markdown parantezi ve köşeli ayraç dahil edilmez).
pub fn ilk_baglanti(metin: &str) -> Option<&str> {
    let b = metin.find("https://").or_else(|| metin.find("http://"))?;
    let geri = &metin[b..];
    let son = geri
        .find(|c: char| c.is_whitespace() || matches!(c, ')' | ']' | '"' | '<' | '>'))
        .unwrap_or(geri.len());
    Some(&geri[..son])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn istek() -> VideoUretimIstegi {
        VideoUretimIstegi {
            istem: "Kapadokya balonları.".into(),
            negatif_istem: Some("yazı".into()),
            en_boy_orani: VideoEnBoyOrani::Yatay16x9,
            sure_saniye: 8,
            cozunurluk: "1080p",
            kamera_hareketi: None,
        }
    }

    #[test]
    fn baglanti_markdowndan_ayiklanir() {
        assert_eq!(
            ilk_baglanti("İşte: [Video](https://x.io/a.mp4) bitti"),
            Some("https://x.io/a.mp4")
        );
        assert_eq!(ilk_baglanti("bağlantı yok"), None);
    }

    #[test]
    fn istem_secenekleri_tasir() {
        let s = video_istemi(&istek());
        assert_eq!(
            s,
            "Kapadokya balonları. En-boy oranı 16:9, süre 8 sn, çözünürlük 1080p. Şunlardan kaçın: yazı."
        );
    }

    /// Bağlantılı yanıt görevi tamamlar ve arşiv kaydı verir; bağlantısız yanıt başarısız kılar.
    #[test]
    fn yanit_gorevi_kapatir() {
        let mut p = VideoPaneli::default();
        let mut m = MedyaYoneticisi::yeni();
        let Eylem::Gonder {
            is: Is::Video(id),
            video: true,
            ..
        } = p.baslat(&mut m, "v1".into(), istek())
        else {
            panic!("video gönderimi bekleniyordu")
        };
        let kayit = p
            .sonuc(&mut m, &id, Ok("[Video](https://x.io/a.mp4)".into()))
            .unwrap();
        assert_eq!(kayit.dosya_yolu, "https://x.io/a.mp4");
        assert!(matches!(
            m.video_durumu(&id),
            Some(VideoDurumu::Tamamlandi { .. })
        ));
        p.baslat(&mut m, "v2".into(), istek());
        assert!(p.sonuc(&mut m, "v2", Ok("üretemedim".into())).is_none());
        assert!(matches!(
            m.video_durumu("v2"),
            Some(VideoDurumu::Basarisiz { .. })
        ));
        assert_eq!(p.sira, ["v1", "v2"]);
    }
}
