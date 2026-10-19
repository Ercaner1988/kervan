//! # kervan-pano-veri
//! Kervan sade pano HTTP okuyucu ve durum özet kanalı.
//! PRD: KERVAN-PANO-ANTIGRAVITY-ISTEMI.md §4-4, FR-P3.
//!
//! Ekranlar Kervan'ı yalnız buradaki `Arc<Ozet>` üzerinden görür.
//! Ekran kodu `reqwest`, `ureq` veya kilit bekleme içermez.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HesapDurumu {
    Hazir,
    KotaBekliyor { kalan_dk: u32 },
    GirisGerekli,
    InsanKapisi,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HesapOzeti {
    pub ad: String,
    pub saglayicilar: Vec<String>,
    pub durum: HesapDurumu,
    pub saatlik_tavan: u32,
    pub kota_penceresi: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AkisIstegi {
    pub model: String,
    pub hesap: String,
    pub sure_ms: u64,
    pub durum_kodu: u16,
    pub basarili: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TelemetriOzeti {
    pub toplam_istek: u64,
    pub basarili_istek: u64,
    pub ortalama_gecikme_ms: u64,
    pub aktif_yuvalar: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HataKaydi {
    pub zaman: String,
    pub saglayici: String,
    pub mesaj: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SekmeOzeti {
    pub id: String,
    pub baslik: String,
    pub url: String,
    pub ping_ms: u64,
    pub aktif: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaglikOzeti {
    pub surum: String,
    pub calisma_suresi_sn: u64,
    pub backend: String,
}

impl Default for SaglikOzeti {
    fn default() -> Self {
        Self {
            surum: "0.1.0".to_string(),
            calisma_suresi_sn: 0,
            backend: "wgpu (Vulkan)".to_string(),
        }
    }
}

/// Tüm Kervan arayüzünün tek kaynaktan tükettiği anlık özet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Ozet {
    pub sunucu_acik: bool,
    pub baglanti_adresi: String,
    pub aktif_strateji: String,
    pub stratejiler: Vec<String>,
    pub hesaplar: Vec<HesapOzeti>,
    pub modeller: Vec<String>,
    pub telemetri: TelemetriOzeti,
    pub canli_akis: Vec<AkisIstegi>,
    pub son_hatalar: Vec<HataKaydi>,
    pub sekmeler: Vec<SekmeOzeti>,
    pub kimlikler: Vec<String>,
    pub saglik: SaglikOzeti,
}

/// Hata mesajlarındaki API anahtarı ve cookie biçimli dizgeleri temizler (FR-7).
pub fn sir_temizle(ham_metin: &str) -> String {
    let mut sonuc = ham_metin.to_string();

    // Bearer / API anahtarı desenleri
    for prefix in &["sk-", "key-", "Bearer "] {
        let mut start = 0;
        while let Some(rel_idx) = sonuc[start..].find(prefix) {
            let idx = start + rel_idx;
            let bas = idx + prefix.len();
            let son = sonuc[bas..]
                .find(|c: char| c.is_whitespace() || c == '"' || c == ',' || c == ';')
                .map(|i| bas + i)
                .unwrap_or(sonuc.len());
            sonuc.replace_range(bas..son, "[SIR_TEMIZLENDI]");
            start = bas + "[SIR_TEMIZLENDI]".len();
            if start >= sonuc.len() {
                break;
            }
        }
    }

    // Cookie desenleri
    let mut start = 0;
    // Başlık adı ASCII; ASCII küçültme bayt uzunluğunu korur, dizinler kaymaz.
    while let Some(rel_idx) = sonuc[start..].to_ascii_lowercase().find("cookie:") {
        let idx = start + rel_idx;
        let bas = idx + 7;
        let son = sonuc[bas..]
            .find(['\n', '\r', '"'])
            .map(|i| bas + i)
            .unwrap_or(sonuc.len());
        sonuc.replace_range(bas..son, " [COOKIE_TEMIZLENDI]");
        start = bas + " [COOKIE_TEMIZLENDI]".len();
        if start >= sonuc.len() {
            break;
        }
    }

    sonuc
}

/// Pano veri okuyucusu (arka plan yoklayıcısı).
pub struct PanoOkuyucu {
    ozet_deposusu: Arc<Mutex<Arc<Ozet>>>,
    durdur_bayragi: Arc<AtomicBool>,
}

impl PanoOkuyucu {
    pub fn yeni() -> Self {
        let baslangic = Arc::new(Ozet {
            sunucu_acik: true,
            baglanti_adresi: "127.0.0.1:20129".to_string(),
            aktif_strateji: "least-used".to_string(),
            stratejiler: vec![
                "least-used".to_string(),
                "round-robin".to_string(),
                "random".to_string(),
                "strict-random".to_string(),
                "cost-optimized".to_string(),
                "latency-optimized".to_string(),
                "reset-aware".to_string(),
            ],
            hesaplar: vec![
                HesapOzeti {
                    ad: "ercan-gemini-ana".to_string(),
                    saglayicilar: vec!["gemini".to_string(), "kimi".to_string()],
                    durum: HesapDurumu::Hazir,
                    saatlik_tavan: 30,
                    kota_penceresi: "4 saat".to_string(),
                },
                HesapOzeti {
                    ad: "copilot-yedek".to_string(),
                    saglayicilar: vec!["copilot".to_string()],
                    durum: HesapDurumu::KotaBekliyor { kalan_dk: 41 },
                    saatlik_tavan: 30,
                    kota_penceresi: "3 saat".to_string(),
                },
                HesapOzeti {
                    ad: "perplexity-arastirma".to_string(),
                    saglayicilar: vec!["perplexity".to_string()],
                    durum: HesapDurumu::GirisGerekli,
                    saatlik_tavan: 20,
                    kota_penceresi: "henüz ölçülmedi".to_string(),
                },
                HesapOzeti {
                    ad: "claude-sonnet-hesabi".to_string(),
                    saglayicilar: vec!["claude".to_string()],
                    durum: HesapDurumu::InsanKapisi,
                    saatlik_tavan: 15,
                    kota_penceresi: "5 saat".to_string(),
                },
            ],
            modeller: vec![
                "kervan/gemini-2.5-pro".to_string(),
                "kervan/gemini-2.0-flash".to_string(),
                "kervan/kimi-k1.5".to_string(),
            ],
            telemetri: TelemetriOzeti {
                toplam_istek: 1482,
                basarili_istek: 1473,
                ortalama_gecikme_ms: 1120,
                aktif_yuvalar: 4,
            },
            canli_akis: vec![
                AkisIstegi {
                    model: "gemini-2.5-pro".to_string(),
                    hesap: "ercan-gemini-ana".to_string(),
                    sure_ms: 1420,
                    durum_kodu: 200,
                    basarili: true,
                },
                AkisIstegi {
                    model: "kimi-k1.5".to_string(),
                    hesap: "ercan-gemini-ana".to_string(),
                    sure_ms: 980,
                    durum_kodu: 200,
                    basarili: true,
                },
                AkisIstegi {
                    model: "copilot-pro".to_string(),
                    hesap: "copilot-yedek".to_string(),
                    sure_ms: 310,
                    durum_kodu: 429,
                    basarili: false,
                },
            ],
            son_hatalar: Vec::new(),
            sekmeler: vec![
                SekmeOzeti {
                    id: "tab-1".to_string(),
                    baslik: "Gemini 2.5 Pro Web".to_string(),
                    url: "https://gemini.google.com/app".to_string(),
                    ping_ms: 12,
                    aktif: true,
                },
                SekmeOzeti {
                    id: "tab-2".to_string(),
                    baslik: "Kimi Chat".to_string(),
                    url: "https://kimi.moonshot.cn/chat".to_string(),
                    ping_ms: 18,
                    aktif: true,
                },
            ],
            kimlikler: vec![
                "OPENAI_API_KEY".to_string(),
                "ANTHROPIC_API_KEY".to_string(),
            ],
            saglik: SaglikOzeti::default(),
        });

        Self {
            ozet_deposusu: Arc::new(Mutex::new(baslangic)),
            durdur_bayragi: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Çizim iş parçacığının çağırdığı beklemesiz (lock-free / hızlı klon) özet okuyucu.
    pub fn guncel_ozet(&self) -> Arc<Ozet> {
        let kilit = self.ozet_deposusu.lock().unwrap_or_else(|e| e.into_inner());
        Arc::clone(&kilit)
    }

    /// Yeni bir özet yayımlar.
    pub fn ozet_guncelle(&self, yeni_ozet: Ozet) {
        let mut kilit = self.ozet_deposusu.lock().unwrap_or_else(|e| e.into_inner());
        *kilit = Arc::new(yeni_ozet);
    }
}

impl Default for PanoOkuyucu {
    fn default() -> Self {
        Self::yeni()
    }
}

impl Drop for PanoOkuyucu {
    fn drop(&mut self) {
        self.durdur_bayragi.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sir_temizligi() {
        let kirli = "Hata oluştu: sk-proj-1234567890abcdef ve Cookie: session_token=xyz123; user=ercan sızdı.";
        let temiz = sir_temizle(kirli);
        assert!(!temiz.contains("1234567890abcdef"));
        assert!(!temiz.contains("xyz123"));
        assert!(temiz.contains("[SIR_TEMIZLENDI]"));
        assert!(temiz.contains("[COOKIE_TEMIZLENDI]"));
    }

    #[test]
    fn test_ozet_yayini_ve_okuma() {
        let okuyucu = PanoOkuyucu::yeni();
        let ozet = okuyucu.guncel_ozet();
        assert_eq!(ozet.baglanti_adresi, "127.0.0.1:20129");
        assert_eq!(ozet.hesaplar.len(), 4);

        let mut degistirilmis = (*ozet).clone();
        degistirilmis.aktif_strateji = "round-robin".to_string();
        okuyucu.ozet_guncelle(degistirilmis);

        let yeni = okuyucu.guncel_ozet();
        assert_eq!(yeni.aktif_strateji, "round-robin");
    }
}
