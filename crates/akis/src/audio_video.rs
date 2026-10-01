//! Kervan Ses ve Video Laboratuvarı Motoru (Audio & Video Studio).
//!
//! Bu modül, OmniRoute'un `handlers/audioSpeech`, `handlers/audioTranscription`
//! ve `handlers/videoGeneration` modüllerini saf Rust olarak Kervan'a kazandırır.
//!
//! YETENEKLER:
//! 1. Ses Laboratuvarı (Voxtral / Whisper STT transkripsiyonu & TTS metin seslendirmesi)
//! 2. Görsel/Video Laboratuvarı (Gemini Veo 2 / Wanx video üretimi ve asenkron durum takibi)
//! 3. Yerel Medya Deposu (Üretilen ses ve videoların diskte kataloglanması)

use std::collections::HashMap;
use std::time::Instant;

/// Desteklenen ses formatları
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SesFormati {
    Wav,
    Mp3,
    M4a,
    Ogg,
    Webm,
}

impl SesFormati {
    pub fn mime_tipi(&self) -> &'static str {
        match self {
            Self::Wav => "audio/wav",
            Self::Mp3 => "audio/mpeg",
            Self::M4a => "audio/m4a",
            Self::Ogg => "audio/ogg",
            Self::Webm => "audio/webm",
        }
    }

    pub fn uzanti(&self) -> &'static str {
        match self {
            Self::Wav => "wav",
            Self::Mp3 => "mp3",
            Self::M4a => "m4a",
            Self::Ogg => "ogg",
            Self::Webm => "webm",
        }
    }
}

/// Ses transkripsiyon (STT) isteği
#[derive(Debug, Clone)]
pub struct SesTranskripsiyonIstegi {
    /// İşlenecek ses dosyasının yerel yolu
    pub dosya_yolu: String,
    /// Ses formatı
    pub format: SesFormati,
    /// Hedef dil (örn: "tr", "en", "ar")
    pub dil: String,
    /// Varsa disiplinlerarası kavram sözlüğü filtresi (Weber, Felsefe, Hukuk vb.)
    pub kavram_sozlugu: Option<Vec<String>>,
}

/// Transkripsiyon zaman damgası aralığı
#[derive(Debug, Clone, PartialEq)]
pub struct ZamanAraligi {
    pub baslangic_sn: f32,
    pub bitis_sn: f32,
    pub metin: String,
}

/// Transkripsiyon (STT) nihai sonucu
#[derive(Debug, Clone, PartialEq)]
pub struct TranskriptSonucu {
    pub tam_metin: String,
    pub guven_skoru: f32,
    pub segmentler: Vec<ZamanAraligi>,
}

/// Metinden sese (TTS) seslendirme isteği
#[derive(Debug, Clone)]
pub struct SeslendirmeIstegi {
    pub metin: String,
    pub ses_modeli: String, // örn: "voxtral-natural-tr", "gemini-live-tr"
    pub konusma_hizi: f32,  // 0.5x - 2.0x (1.0 = normal)
    pub cikti_formati: SesFormati,
}

/// Video en-boy oranları
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoEnBoyOrani {
    Kare1x1,
    Yatay16x9,
    Dikey9x16,
    Sinematik21x9,
}

impl VideoEnBoyOrani {
    pub fn etiket(&self) -> &'static str {
        match self {
            Self::Kare1x1 => "1:1",
            Self::Yatay16x9 => "16:9",
            Self::Dikey9x16 => "9:16",
            Self::Sinematik21x9 => "21:9",
        }
    }
}

/// Video üretim isteği (Gemini Veo 2 / Wanx 2.1)
#[derive(Debug, Clone)]
pub struct VideoUretimIstegi {
    pub istem: String,
    pub negatif_istem: Option<String>,
    pub en_boy_orani: VideoEnBoyOrani,
    pub sure_saniye: u32,
    pub cozunurluk: &'static str,        // "720p", "1080p"
    pub kamera_hareketi: Option<String>, // "Pan Left", "Slow Zoom In"
}

/// Asenkron video üretim durum yaşam döngüsü
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VideoDurumu {
    Kuyrukta {
        sira: u32,
    },
    Uretiliyor {
        ilerleme_yuzdesi: u8,
    },
    Tamamlandi {
        video_url: String,
        yerel_dosya_yolu: Option<String>,
    },
    Basarisiz {
        hata_mesaji: String,
    },
}

/// Takip edilen bir video üretim görevi
#[derive(Debug, Clone)]
pub struct VideoGorevi {
    pub id: String,
    pub istek: VideoUretimIstegi,
    pub durum: VideoDurumu,
    pub baslangic_zamani: Instant,
    pub son_guncelleme: Instant,
}

impl VideoGorevi {
    pub fn yeni(id: impl Into<String>, istek: VideoUretimIstegi) -> Self {
        let simdi = Instant::now();
        Self {
            id: id.into(),
            istek,
            durum: VideoDurumu::Kuyrukta { sira: 1 },
            baslangic_zamani: simdi,
            son_guncelleme: simdi,
        }
    }

    pub fn ilerleme_guncelle(&mut self, yuzde: u8) {
        self.durum = VideoDurumu::Uretiliyor {
            ilerleme_yuzdesi: yuzde.min(100),
        };
        self.son_guncelleme = Instant::now();
    }

    pub fn tamamla(&mut self, video_url: impl Into<String>, yerel_yol: Option<String>) {
        self.durum = VideoDurumu::Tamamlandi {
            video_url: video_url.into(),
            yerel_dosya_yolu: yerel_yol,
        };
        self.son_guncelleme = Instant::now();
    }

    pub fn hata_ile_kapat(&mut self, hata: impl Into<String>) {
        self.durum = VideoDurumu::Basarisiz {
            hata_mesaji: hata.into(),
        };
        self.son_guncelleme = Instant::now();
    }
}

/// Tüm ses ve video işlemlerini koordine eden reaktif Medya Yöneticisi
#[derive(Debug, Default)]
pub struct MedyaYoneticisi {
    video_gorevleri: HashMap<String, VideoGorevi>,
    tamamlanan_medyalar: Vec<KayitliMedya>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KayitliMedya {
    pub id: String,
    pub tur: &'static str, // "ses", "video", "gorsel"
    pub baslik: String,
    pub dosya_yolu: String,
    pub boyut_bayt: u64,
}

impl MedyaYoneticisi {
    pub fn yeni() -> Self {
        Self::default()
    }

    pub fn video_gorevi_ekle(&mut self, gorev: VideoGorevi) {
        self.video_gorevleri.insert(gorev.id.clone(), gorev);
    }

    pub fn video_durumu(&self, gorev_id: &str) -> Option<&VideoDurumu> {
        self.video_gorevleri.get(gorev_id).map(|g| &g.durum)
    }

    pub fn video_gorevi_mut(&mut self, gorev_id: &str) -> Option<&mut VideoGorevi> {
        self.video_gorevleri.get_mut(gorev_id)
    }

    pub fn medya_arsivle(&mut self, medya: KayitliMedya) {
        self.tamamlanan_medyalar.push(medya);
    }

    pub fn arsivlenen_medyalar(&self) -> &[KayitliMedya] {
        &self.tamamlanan_medyalar
    }
}

/// Transkripsiyon sonrası kavram sözlüğü filtresi
/// Örneğin ses transkripsiyonunda "Veber" veya "rasyonalleşme" olarak geçen kelimeleri
/// akademik sözlük kuralına göre "Weber" ve "aklîleşme" olarak düzeltir.
pub fn kavram_sozlugu_filtresi_uygula(metin: &str, duzeltmeler: &[(String, String)]) -> String {
    let mut sonuc = metin.to_string();
    for (yanlis, dogru) in duzeltmeler {
        sonuc = sonuc.replace(yanlis, dogru);
    }
    sonuc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kavram_sozlugu_filtresi() {
        let ham_transkript = "veber sosyolojisinde rasyonalleşme süreci bürokrasinin temelidir.";
        let duzeltmeler = vec![
            ("veber".to_string(), "Weber".to_string()),
            ("rasyonalleşme".to_string(), "aklîleşme".to_string()),
        ];
        let duzeltilmis = kavram_sozlugu_filtresi_uygula(ham_transkript, &duzeltmeler);
        assert_eq!(
            duzeltilmis,
            "Weber sosyolojisinde aklîleşme süreci bürokrasinin temelidir."
        );
    }

    #[test]
    fn test_video_gorevi_yasam_dongusu() {
        let istek = VideoUretimIstegi {
            istem: "Antik kütüphanede çalışan düşünür".to_string(),
            negatif_istem: None,
            en_boy_orani: VideoEnBoyOrani::Yatay16x9,
            sure_saniye: 5,
            cozunurluk: "1080p",
            kamera_hareketi: Some("Slow Zoom".to_string()),
        };

        let mut gorev = VideoGorevi::yeni("vid-101", istek);
        assert_eq!(gorev.durum, VideoDurumu::Kuyrukta { sira: 1 });

        gorev.ilerleme_guncelle(45);
        assert_eq!(
            gorev.durum,
            VideoDurumu::Uretiliyor {
                ilerleme_yuzdesi: 45
            }
        );

        gorev.tamamla(
            "https://example.com/video.mp4",
            Some("C:/output/video.mp4".to_string()),
        );
        match gorev.durum {
            VideoDurumu::Tamamlandi {
                video_url,
                yerel_dosya_yolu,
            } => {
                assert_eq!(video_url, "https://example.com/video.mp4");
                assert_eq!(yerel_dosya_yolu, Some("C:/output/video.mp4".to_string()));
            }
            _ => panic!("Durum tamamlandı olmalıydı!"),
        }
    }
}
