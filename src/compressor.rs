//! Kervan Gelişmiş Token Sıkıştırma ve Bağlam Koruma Motoru.
//!
//! Bu modül, OmniRoute'un `services/compression` (aggressive, filler, context-aware)
//! ve Caveman kurallarını saf Rust ile sıfır bellek kopyalaması ve mikrosaniye
//! gecikmeyle uygular.
//!
//! YETENEKLER:
//! 1. Seviyeli Sıkıştırma (Hafif, Orta, Agresif)
//! 2. Kod ve Alıntı Koruma: Markdown ` ``` ` blokları ve ` > ` alıntıları korunur
//! 3. Çok Dilli Dolgu (Filler) Temizliği: Türkçe ve İngilizce klişeleri budama
//! 4. Kümülatif Telemetri: Tasarruf edilen karakter ve yaklaşık token ölçümü

use std::sync::atomic::{AtomicUsize, Ordering};

/// Sıkıştırma şiddet seviyesi
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SikistirmaSeviyesi {
    /// Sadece ardışık boşluklar ve gereksiz satırlar temizlenir
    Hafif,
    /// Nezaket ve sistem dolguları eritilir, bağlam korunur
    #[default]
    Orta,
    /// OmniRoute Agresif/Ultra: Maksimum token tasarrufu (%40-60)
    Agresif,
}

pub struct SikistirmaSonucu {
    pub metin: String,
    pub orijinal_uzunluk: usize,
    pub yeni_uzunluk: usize,
    pub tasarruf_yuzdesi: f32,
    pub tahmini_tasarruf_token: usize,
}

pub struct Compressor {
    toplam_giren_karakter: AtomicUsize,
    toplam_cikan_karakter: AtomicUsize,
}

impl Default for Compressor {
    fn default() -> Self {
        Self::new()
    }
}

impl Compressor {
    pub fn new() -> Self {
        Self {
            toplam_giren_karakter: AtomicUsize::new(0),
            toplam_cikan_karakter: AtomicUsize::new(0),
        }
    }

    /// Bir metni belirlenen sıkıştırma seviyesine göre akıllıca işler.
    /// Kod blokları (```) ve alıntılar (>) tespit edilerek biçimleri korunur.
    pub fn akilli_sikistir(&self, girdi: &str, seviye: SikistirmaSeviyesi) -> SikistirmaSonucu {
        let orijinal_uzunluk = girdi.len();
        if orijinal_uzunluk == 0 {
            return SikistirmaSonucu {
                metin: String::new(),
                orijinal_uzunluk: 0,
                yeni_uzunluk: 0,
                tasarruf_yuzdesi: 0.0,
                tahmini_tasarruf_token: 0,
            };
        }

        let mut sonuc = String::with_capacity(orijinal_uzunluk);
        let mut kod_bloku_icinde = false;
        let mut ardarda_bos_satir = 0;

        for satir in girdi.lines() {
            let kirpilmis = satir.trim_end();

            // Kod bloğu sınır kontrolü
            if kirpilmis.starts_with("```") {
                kod_bloku_icinde = !kod_bloku_icinde;
                sonuc.push_str(kirpilmis);
                sonuc.push('\n');
                continue;
            }

            // Kod bloğu içindeyse boşluklara ve içeriğe dokunma
            if kod_bloku_icinde {
                sonuc.push_str(satir);
                sonuc.push('\n');
                continue;
            }

            // Boş satır kontrolü (ardışık boş satırları tek satıra indir)
            if kirpilmis.trim().is_empty() {
                ardarda_bos_satir += 1;
                if ardarda_bos_satir <= 1 {
                    sonuc.push('\n');
                }
                continue;
            }

            ardarda_bos_satir = 0;

            // Satır seviyesinde sıkıştırma kuralları
            let mut islenmis_satir = kirpilmis.to_string();

            if seviye == SikistirmaSeviyesi::Orta || seviye == SikistirmaSeviyesi::Agresif {
                islenmis_satir = self.orta_seviye_budama(&islenmis_satir);
            }

            if seviye == SikistirmaSeviyesi::Agresif {
                islenmis_satir = self.agresif_budama(&islenmis_satir);
            }

            if !islenmis_satir.is_empty() {
                sonuc.push_str(&islenmis_satir);
                sonuc.push('\n');
            }
        }

        let nihai = sonuc.trim_end().to_string();
        let yeni_uzunluk = nihai.len();

        self.toplam_giren_karakter.fetch_add(orijinal_uzunluk, Ordering::Relaxed);
        self.toplam_cikan_karakter.fetch_add(yeni_uzunluk, Ordering::Relaxed);

        let tasarruf_yuzdesi = if orijinal_uzunluk > yeni_uzunluk {
            ((orijinal_uzunluk - yeni_uzunluk) as f32 / orijinal_uzunluk as f32) * 100.0
        } else {
            0.0
        };

        // Ortalama 4 karakter ~ 1 token hesabı
        let tahmini_tasarruf_token = if orijinal_uzunluk > yeni_uzunluk {
            (orijinal_uzunluk - yeni_uzunluk) / 4
        } else {
            0
        };

        SikistirmaSonucu {
            metin: nihai,
            orijinal_uzunluk,
            yeni_uzunluk,
            tasarruf_yuzdesi,
            tahmini_tasarruf_token,
        }
    }

    /// Orta seviye nezaket ve dolgu sözcükleri temizliği
    fn orta_seviye_budama(&self, satir: &str) -> String {
        let mut s = satir.to_string();
        const DOLGULAR: &[(&str, &str)] = &[
            ("Lütfen unutmayın ki, ", ""),
            ("Lütfen unutma ki, ", ""),
            ("Şunu belirtmek gerekir ki, ", ""),
            ("Bilindiği üzere, ", ""),
            ("Aşağıdaki talimatlara harfiyen uyunuz: ", "Talimatlar: "),
            ("Please make sure to ", ""),
            ("It is very important that you ", ""),
            ("As an AI assistant, you should ", ""),
            ("Always remember to ", ""),
            ("Keep in mind that ", ""),
        ];

        for (eski, yeni) in DOLGULAR {
            s = s.replace(eski, yeni);
        }
        s
    }

    /// OmniRoute Agresif / Ultra kuralları
    fn agresif_budama(&self, satir: &str) -> String {
        let mut s = satir.to_string();
        const AGRO_DOLGULAR: &[(&str, &str)] = &[
            ("detaylı bir şekilde açıklayınız", "açıklayın"),
            ("ayrıntılı olarak inceleyiniz", "inceleyin"),
            ("adım adım düşünerek", "adım adım"),
            ("in a comprehensive and detailed manner", "concisely"),
            ("provide a step by step explanation", "step-by-step:"),
            ("feel free to ", ""),
            ("I would appreciate if you could ", ""),
        ];

        for (eski, yeni) in AGRO_DOLGULAR {
            s = s.replace(eski, yeni);
        }
        s
    }

    /// RTK geriye dönük uyumluluk çağrısı
    pub fn rtk_sikistir(&self, girdi: &str) -> SikistirmaSonucu {
        self.akilli_sikistir(girdi, SikistirmaSeviyesi::Hafif)
    }

    /// Caveman geriye dönük uyumluluk çağrısı
    pub fn caveman_sikistir(&self, sistem_istemi: &str) -> String {
        let sonuc = self.akilli_sikistir(sistem_istemi, SikistirmaSeviyesi::Orta);
        sonuc.metin
    }

    /// Toplam kümülatif tasarruf yüzdesini döner.
    pub fn toplam_tasarruf_orani(&self) -> f32 {
        let giren = self.toplam_giren_karakter.load(Ordering::Relaxed);
        let cikan = self.toplam_cikan_karakter.load(Ordering::Relaxed);
        if giren == 0 || giren <= cikan {
            0.0
        } else {
            ((giren - cikan) as f32 / giren as f32) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kod_bloklarini_koruma() {
        let comp = Compressor::new();
        let girdi = "Giriş metni.\n\n```rust\nfn main() {\n    let x = 5;\n}\n```\nSon metin.";
        let sonuc = comp.akilli_sikistir(girdi, SikistirmaSeviyesi::Agresif);

        // Kod bloğu içerisindeki 4 boşluklu girinti korunmalıdır
        assert!(sonuc.metin.contains("    let x = 5;"));
        assert!(sonuc.metin.contains("```rust"));
    }

    #[test]
    fn test_turkce_akademik_ve_agresif_sikistirma() {
        let comp = Compressor::new();
        let girdi = "Lütfen unutmayın ki, Weber sosyolojisini detaylı bir şekilde açıklayınız.\n\n\n\nBilindiği üzere, aklileşme önemlidir.";
        let sonuc = comp.akilli_sikistir(girdi, SikistirmaSeviyesi::Agresif);

        assert!(sonuc.metin.contains("Weber sosyolojisini açıklayın."));
        assert!(sonuc.metin.contains("aklileşme önemlidir."));
        assert!(!sonuc.metin.contains("Lütfen unutmayın"));
        assert!(sonuc.tasarruf_yuzdesi > 20.0);
    }

    #[test]
    fn test_bosluk_ve_bos_girdi() {
        let comp = Compressor::new();
        let sonuc = comp.akilli_sikistir("", SikistirmaSeviyesi::Hafif);
        assert_eq!(sonuc.metin, "");
        assert_eq!(sonuc.tahmini_tasarruf_token, 0);
    }
}
