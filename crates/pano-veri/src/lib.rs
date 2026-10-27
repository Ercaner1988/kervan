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
    // ASCII arama: `to_lowercase` Türkçe 'İ' gibi harflerde bayt uzunluğunu
    // değiştirdiğinden dönen ofset asıl dizgeye uymaz ve maskeleme kayar
    // (sır sızabilir). `to_ascii_lowercase` yalnız A-Z'yi çevirir, ofset korunur.
    while let Some(rel_idx) = sonuc[start..].to_ascii_lowercase().find("cookie:") {
        let idx = start + rel_idx;
        let bas = idx + 7;
        let son = sonuc[bas..]
            .find(|c: char| ['\n', '\r', '"'].contains(&c))
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

/// Panelden canlı izlenen bir eklenti hesabı (`provider:profile`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CanliHesap {
    /// `gemini:ercaner1988@gmail.com` — istekte `kervan.hesap` olarak bu gider.
    pub anahtar: String,
    /// Yalnız profil adı (eklenti kimliği).
    pub profil: String,
    pub saglayici: String,
    /// Eklenti sekmesi şu an bağlı mı?
    pub bagli: bool,
    pub istek_sayisi: u32,
    pub son_saatte: usize,
    pub saatlik_tavan: u32,
    /// İnsan kapısı / duraklatma nedeni (varsa).
    pub duraklatildi: Option<String>,
}

/// Sağlayıcının ölçülmüş model menüsünden bir seçenek.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CanliModel {
    /// `None` = site varsayılanı (`kervan/<saglayici>`).
    pub kod: Option<String>,
    pub etiket: String,
    pub kilitli: bool,
    pub kilit_nedeni: Option<String>,
}

/// Sağlayıcının ölçülmüş aç/kapa özelliği.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CanliAnahtar {
    pub kod: String,
    pub etiket: String,
}

/// Bir sağlayıcının panelde görünen bütün seçenekleri (kervan.toml'dan).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CanliSaglayici {
    pub ad: String,
    pub modeller: Vec<CanliModel>,
    pub anahtarlar: Vec<CanliAnahtar>,
}

/// Panel canlı pano durumu: hesaplar + sağlayıcı seçenekleri.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CanliDurum {
    pub hesaplar: Vec<CanliHesap>,
    pub saglayicilar: Vec<CanliSaglayici>,
    /// Son başarılı yoklama (unix saniye); 0 = henüz yoklanmadı.
    pub guncelleme: u64,
    /// Yoklama hatası (sunucu kapalı vb.).
    pub hata: Option<String>,
}

/// Panelden gönderilen sınamanın durumu.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct GonderimDurumu {
    pub calisiyor: bool,
    pub yanit: String,
    pub hata: Option<String>,
    pub sure_ms: u64,
    /// İsteğin gittiği model kimliği (`kervan/gemini/orta`).
    pub model: String,
    /// İsteğin sabitlendiği hesap (`gemini:...`), boşsa seçim sunucuda.
    pub hesap: String,
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
            // Sahte veri YOK: hesaplar canlı kaynaktan (eklenti hub + havuz)
            // doldurulur. Pano boş açılır, ilk yoklamada gerçek hesaplar gelir.
            hesaplar: Vec::new(),
            modeller: Vec::new(),
            telemetri: TelemetriOzeti::default(),
            canli_akis: Vec::new(),
            son_hatalar: Vec::new(),
            sekmeler: Vec::new(),
            kimlikler: Vec::new(),
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
    fn test_sir_temizligi_turkce_harf_ofseti_kaydirmaz() {
        // 'İ' küçük harfe çevrilince iki kod noktasına ayrılır; dil-hassas
        // arama bayt ofsetini kaydırıp Cookie değerini maskelemeden bırakıyordu.
        // Her 'İ' küçük harfte bir bayt büyür; ofset sağa kayar ve değerin
        // başı maskesiz kalır (sır sızar).
        let kirli = format!("{} Cookie: GİZLİ_DEĞER; devam", "İ".repeat(3));
        let temiz = sir_temizle(&kirli);
        assert!(
            temiz.ends_with("Cookie: [COOKIE_TEMIZLENDI]"),
            "Cookie değeri eksiksiz maskelenmeliydi: {temiz}"
        );
        assert!(!temiz.contains("GİZLİ"), "sır sızdı: {temiz}");
        assert!(temiz.contains("[COOKIE_TEMIZLENDI]"));
    }

    #[test]
    fn test_ozet_yayini_ve_okuma() {
        let okuyucu = PanoOkuyucu::yeni();
        let ozet = okuyucu.guncel_ozet();
        assert_eq!(ozet.baglanti_adresi, "127.0.0.1:20129");
        assert!(
            ozet.hesaplar.is_empty(),
            "başlangıç özeti sahte hesap içermemeli"
        );

        let mut degistirilmis = (*ozet).clone();
        degistirilmis.aktif_strateji = "round-robin".to_string();
        okuyucu.ozet_guncelle(degistirilmis);

        let yeni = okuyucu.guncel_ozet();
        assert_eq!(yeni.aktif_strateji, "round-robin");
    }
}
