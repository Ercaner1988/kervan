//! Stream Recovery & Throughput Watchdog (Kopan Akış Kurtarma ve Hız Bekçisi)
//!
//! OmniRoute `streamRecovery.ts` ve `throughputWatchdog.ts` mimarisinden
//! saf Rust ile uyarlanmıştır.
//!
//! Yetenekler:
//! 1. `HoldbackBuffer`: İlk SSE pencerelerini tutarak erken kopmalarda istemciye
//!    hiç veri gitmeden şeffaf tekrar deneme (transparent retry) imkanı sunar.
//! 2. `ThroughputWatchdog`: Akış hızını (bayt/sn) izler. Düşünme/akıl yürütme
//!    (reasoning) evrelerini korur, gerçek donmaları tespit eder.
//! 3. `SeamStitcher`: Yarıda kesilen akışlarda kaldığı yerden devam eden yeni
//!    yanıtı dikişsiz ve mükerrer kelimeler olmadan birleştirir.

use std::time::{Duration, Instant};

/// Akışın temiz bir şekilde bittiğini belirten SSE sonlandırma işaretleri.
pub const OPENAI_DONE_MARKER: &str = "[DONE]";
pub const CLAUDE_STOP_MARKER: &str = "message_stop";

/// Tekrar denenebilir akış hatası türleri.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AkisHataTuru {
    ErkenKopma,
    DusukHizDonma,
    ZamanAsimi,
    AgKopmasi(String),
}

/// Holdback arabelleği yapılandırması.
#[derive(Debug, Clone)]
pub struct HoldbackYapilandirma {
    pub bekleme_suresi: Duration,
    pub azami_bayt: usize,
}

impl Default for HoldbackYapilandirma {
    fn default() -> Self {
        Self {
            bekleme_suresi: Duration::from_millis(500),
            azami_bayt: 2048,
        }
    }
}

/// Akışın ilk birkaç parçasını kısa süre tutan tampon.
/// Erken kopma durumunda istemciye veri gitmediği için güvenle yeniden başlatılabilir.
pub struct HoldbackTamponu {
    parcalar: Vec<Vec<u8>>,
    toplam_bayt: usize,
    baslangic: Option<Instant>,
    yapilandirma: HoldbackYapilandirma,
    kesinlesti: bool,
}

impl HoldbackTamponu {
    pub fn yeni(yapilandirma: HoldbackYapilandirma) -> Self {
        Self {
            parcalar: Vec::new(),
            toplam_bayt: 0,
            baslangic: None,
            yapilandirma,
            kesinlesti: false,
        }
    }

    /// Yeni bir parça ekler. Eğer kesinleştiyse parçayı doğrudan döndürür.
    /// Henüz kesinleşmediyse ve limitler aşıldıysa tamponu boşaltıp (flush) döndürür.
    pub fn ekle(&mut self, parca: &[u8]) -> Vec<Vec<u8>> {
        if self.kesinlesti {
            return vec![parca.to_vec()];
        }

        if self.baslangic.is_none() {
            self.baslangic = Some(Instant::now());
        }

        self.parcalar.push(parca.to_vec());
        self.toplam_bayt += parca.len();

        let sure_doldu = self
            .baslangic
            .map(|b| b.elapsed() >= self.yapilandirma.bekleme_suresi)
            .unwrap_or(false);

        if self.toplam_bayt >= self.yapilandirma.azami_bayt || sure_doldu {
            self.bosalt()
        } else {
            Vec::new()
        }
    }

    /// Tamponu zorla kesinleştirir ve içerideki tüm parçaları dışarı verir.
    pub fn bosalt(&mut self) -> Vec<Vec<u8>> {
        if self.kesinlesti {
            return Vec::new();
        }
        self.kesinlesti = true;
        let cikan = std::mem::take(&mut self.parcalar);
        self.toplam_bayt = 0;
        cikan
    }

    /// Şeffaf tekrar deneme için tamponu hiçbir şey kesinleştirmeden sıfırlar.
    pub fn temizle_ve_sifirla(&mut self) {
        self.parcalar.clear();
        self.toplam_bayt = 0;
        self.baslangic = None;
        self.kesinlesti = false;
    }

    pub fn kesinlesti_mi(&self) -> bool {
        self.kesinlesti
    }

    pub fn tampon_bayt(&self) -> usize {
        self.toplam_bayt
    }
}

/// Throughput Bekçisi yapılandırması.
#[derive(Debug, Clone)]
pub struct HizBekcisiAyarlari {
    pub aktif: bool,
    pub isinma_suresi: Duration,
    pub asgari_bayt_saniye: f64,
}

impl Default for HizBekcisiAyarlari {
    fn default() -> Self {
        Self {
            aktif: true,
            isinma_suresi: Duration::from_secs(3),
            asgari_bayt_saniye: 10.0, // Saniyede en az 10 karakter/bayt
        }
    }
}

/// Akış Hız Bekçisi Karar Raporu.
#[derive(Debug, Clone, PartialEq)]
pub struct HizBekcisiKarari {
    pub durdurulmali: bool,
    pub sebep: Option<String>,
    pub faydali_bayt: usize,
    pub bayt_saniye: f64,
    pub korumali_evre: bool,
}

/// Akış hızını ve donmaları denetleyen bekçi motoru.
pub struct HizBekcisi {
    ayarlar: HizBekcisiAyarlari,
    baslangic: Instant,
    faydali_bayt: usize,
    korumali_evre: bool,
}

impl HizBekcisi {
    pub fn yeni(ayarlar: HizBekcisiAyarlari) -> Self {
        Self {
            ayarlar,
            baslangic: Instant::now(),
            faydali_bayt: 0,
            korumali_evre: false,
        }
    }

    /// Gelen SSE parçası içindeki düşünme/akıl yürütme (reasoning) ve içerik baytlarını tartar.
    pub fn ornekle(&mut self, metin: &str) -> HizBekcisiKarari {
        if !self.ayarlar.aktif {
            return HizBekcisiKarari {
                durdurulmali: false,
                sebep: None,
                faydali_bayt: self.faydali_bayt,
                bayt_saniye: 0.0,
                korumali_evre: false,
            };
        }

        // Düşünme / Akıl yürütme evresi tespiti
        if metin.contains("reasoning_content")
            || metin.contains("\"thinking\"")
            || metin.contains("<think>")
            || metin.contains("tool_calls")
        {
            self.korumali_evre = true;
        }

        // Metin içeriği evresine geçiş tespiti
        if (metin.contains("</think>") || metin.contains("\"content\":"))
            && metin.contains("</think>")
        {
            self.korumali_evre = false;
        }

        // Basit faydalı veri hacmi artışı
        let temiz_uzunluk = metin.trim().len();
        self.faydali_bayt += temiz_uzunluk;

        let gecen_sure = self.baslangic.elapsed().as_secs_f64().max(0.001);
        let anlik_hiz = (self.faydali_bayt as f64) / gecen_sure;

        // Isınma süresindeyse veya düşünme evresindeyse durdurma kararı verme
        if self.baslangic.elapsed() < self.ayarlar.isinma_suresi || self.korumali_evre {
            return HizBekcisiKarari {
                durdurulmali: false,
                sebep: None,
                faydali_bayt: self.faydali_bayt,
                bayt_saniye: anlik_hiz,
                korumali_evre: self.korumali_evre,
            };
        }

        // Isınma bitti, korumalı evrede değiliz ve hız sınırın altındaysa alarm ver
        if anlik_hiz < self.ayarlar.asgari_bayt_saniye && self.faydali_bayt < 50 {
            HizBekcisiKarari {
                durdurulmali: true,
                sebep: Some(format!(
                    "Akış hızı yetersiz ({:.1} B/s < {:.1} B/s). Bağlantı dondu.",
                    anlik_hiz, self.ayarlar.asgari_bayt_saniye
                )),
                faydali_bayt: self.faydali_bayt,
                bayt_saniye: anlik_hiz,
                korumali_evre: false,
            }
        } else {
            HizBekcisiKarari {
                durdurulmali: false,
                sebep: None,
                faydali_bayt: self.faydali_bayt,
                bayt_saniye: anlik_hiz,
                korumali_evre: false,
            }
        }
    }
}

/// Akış parçalarını tarayan yardımcılar.
pub struct AkisTarayici;

impl AkisTarayici {
    /// Parça içinde SSE sonlandırma işareti var mı?
    pub fn sonlandi_mi(metin: &str) -> bool {
        metin.contains(OPENAI_DONE_MARKER)
            || metin.contains(CLAUDE_STOP_MARKER)
            || metin.contains("\"finish_reason\":\"stop\"")
            || metin.contains("\"finish_reason\": \"stop\"")
            || metin.contains("\"finish_reason\":\"end_turn\"")
    }

    /// OpenAI SSE akışındaki metin deltalarını toplar.
    pub fn delta_icerik_cikar(sse_satiri: &str) -> Option<String> {
        let trimmed = sse_satiri.trim();
        if !trimmed.starts_with("data:") {
            return None;
        }
        let veri = trimmed.trim_start_matches("data:").trim();
        if veri.is_empty() || veri == OPENAI_DONE_MARKER {
            return None;
        }

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(veri)
            && let Some(choices) = json.get("choices").and_then(|c| c.as_array())
            && let Some(first) = choices.first()
            && let Some(content) = first
                .get("delta")
                .and_then(|d| d.get("content"))
                .and_then(|c| c.as_str())
        {
            return Some(content.to_string());
        }
        None
    }
}

/// Kopan akışları dikişsiz birleştirici (Seam Stitcher).
pub struct DikisBirlestirici;

impl DikisBirlestirici {
    /// İki metin parçası arasındaki kesişim/örtüşme (overlap) payını bulur.
    /// Örneğin "Türkiye Büyük Millet" ile "Millet Meclisi" arasında "Millet" örtüşür.
    pub fn kesisim_uzunlugu(onceki: &str, yeni: &str) -> usize {
        let max_arama = onceki.len().min(yeni.len()).min(500);
        for len in (1..=max_arama).rev() {
            let onceki_idx = onceki.len() - len;
            if onceki.is_char_boundary(onceki_idx) && yeni.is_char_boundary(len) {
                let onceki_sonu = &onceki[onceki_idx..];
                let yeni_basi = &yeni[..len];
                if onceki_sonu == yeni_basi {
                    return len;
                }
            }
        }
        0
    }

    /// Kopan metnin devamını mükerrer başlangıcı budayarak ekler.
    pub fn dikis_ekle(onceki: &str, yeni_devam: &str) -> String {
        if onceki.is_empty() {
            return yeni_devam.to_string();
        }
        if yeni_devam.is_empty() {
            return onceki.to_string();
        }

        let kesisim = Self::kesisim_uzunlugu(onceki, yeni_devam);
        let temiz_ek = if yeni_devam.is_char_boundary(kesisim) {
            &yeni_devam[kesisim..]
        } else {
            yeni_devam
        };
        format!("{}{}", onceki, temiz_ek)
    }

    /// Kopan yanıtı sürdürmek için devam istemi üretir.
    pub fn devam_istemi_olustur(onceki_yanit: &str) -> String {
        let son_parca = if onceki_yanit.len() > 150 {
            let mut start = onceki_yanit.len() - 150;
            while !onceki_yanit.is_char_boundary(start) && start < onceki_yanit.len() {
                start += 1;
            }
            &onceki_yanit[start..]
        } else {
            onceki_yanit
        };

        format!(
            "[SİSTEM: Yanıtın önceki kısmı alındı ancak akış kesildi. Lütfen aşağıdaki son cümleden itibaren HİÇ TEKRAR ETMEDEN kesintisiz devam et]\nSon cümleler:\n\"...{}\"\n\nDevam et:",
            son_parca.trim()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_holdback_tamponu_azami_bayt_ile_bosaltilir() {
        let ayar = HoldbackYapilandirma {
            bekleme_suresi: Duration::from_secs(10),
            azami_bayt: 10,
        };
        let mut tampon = HoldbackTamponu::yeni(ayar);

        // 4 bayt ekle, henüz kesinleşmemeli
        let cikan1 = tampon.ekle(b"1234");
        assert!(cikan1.is_empty());
        assert!(!tampon.kesinlesti_mi());

        // 8 bayt daha ekle (toplam 12 >= 10), kesinleşmeli ve tümü dönmeli
        let cikan2 = tampon.ekle(b"56789012");
        assert_eq!(cikan2.len(), 2);
        assert_eq!(cikan2[0], b"1234");
        assert_eq!(cikan2[1], b"56789012");
        assert!(tampon.kesinlesti_mi());

        // Kesinleştikten sonra gelen doğrudan akar
        let cikan3 = tampon.ekle(b"son");
        assert_eq!(cikan3, vec![b"son".to_vec()]);
    }

    #[test]
    fn test_holdback_tamponu_sifirlama() {
        let mut tampon = HoldbackTamponu::yeni(HoldbackYapilandirma::default());
        tampon.ekle(b"gecici veri");
        assert_eq!(tampon.tampon_bayt(), 11);

        tampon.temizle_ve_sifirla();
        assert_eq!(tampon.tampon_bayt(), 0);
        assert!(!tampon.kesinlesti_mi());
    }

    #[test]
    fn test_hiz_bekcisi_dusunme_evresinde_durdurmaz() {
        let ayar = HizBekcisiAyarlari {
            aktif: true,
            isinma_suresi: Duration::from_millis(10),
            asgari_bayt_saniye: 1000.0, // Çok yüksek eşik
        };
        let mut bekci = HizBekcisi::yeni(ayar);
        std::thread::sleep(Duration::from_millis(20));

        // Düşünme evresi sinyali
        let karar = bekci.ornekle("data: {\"delta\": {\"reasoning_content\": \"adım 1\"}}");
        assert!(karar.korumali_evre);
        assert!(!karar.durdurulmali);
    }

    #[test]
    fn test_akis_tarayici_terminal_isaretleri() {
        assert!(AkisTarayici::sonlandi_mi("data: [DONE]"));
        assert!(AkisTarayici::sonlandi_mi("event: message_stop\ndata: {}"));
        assert!(AkisTarayici::sonlandi_mi("{\"finish_reason\":\"stop\"}"));
        assert!(!AkisTarayici::sonlandi_mi(
            "data: {\"delta\": {\"content\": \"merhaba\"}}"
        ));
    }

    #[test]
    fn test_dikis_kesisim_ve_birlestirme() {
        let onceki = "Kervan projesi yüksek performanslı";
        let yeni = "performanslı ve güvenilir bir köprüdür.";

        let kesisim = DikisBirlestirici::kesisim_uzunlugu(onceki, yeni);
        assert_eq!(kesisim, "performanslı".len());

        let birlesik = DikisBirlestirici::dikis_ekle(onceki, yeni);
        assert_eq!(
            birlesik,
            "Kervan projesi yüksek performanslı ve güvenilir bir köprüdür."
        );
    }

    #[test]
    fn test_devam_istemi_olusturma() {
        let parca = "Bu çalışmada Weber'in bürokrasi teorisi incelenmiş olup";
        let istem = DikisBirlestirici::devam_istemi_olustur(parca);
        assert!(istem.contains("Devam et:"));
        assert!(istem.contains("Weber'in bürokrasi teorisi"));
    }
}
