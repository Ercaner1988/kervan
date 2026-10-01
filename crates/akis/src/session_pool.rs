//! Kervan Çoklu Oturum ve Zen Kapsayıcı (Containers) Rotasyon Motoru.
//!
//! Bu modül, OmniRoute'un `sessionPool`, `fingerprintRotator` ve `accountFallback`
//! mimarisini Zen Browser / Firefox Multi-Account Containers (`contextualIdentities`)
//! altyapısıyla birleştirerek saf Rust olarak uygular.
//!
//! AMAÇ: Aynı anda 8 farklı Google hesabının ve bunlara bağlı farklı sağlayıcıların
//! (Gemini, ChatGPT, Mistral, Copilot vb.) tek bir tarayıcı penceresinde, çerezleri
//! birbirine karışmadan, kalıcı olarak ve insan hız bütçesini aşmadan çalıştırılmasıdır.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Desteklenen sağlayıcı türü
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Saglayici {
    Gemini,
    ChatGpt,
    Mistral,
    Copilot,
    DeepSeek,
    Qwen,
    Kimi,
    Zai,
    Perplexity,
    Ozel(String),
}

impl Saglayici {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Gemini => "gemini",
            Self::ChatGpt => "chatgpt",
            Self::Mistral => "mistral",
            Self::Copilot => "copilot",
            Self::DeepSeek => "deepseek",
            Self::Qwen => "qwen",
            Self::Kimi => "kimi",
            Self::Zai => "z.ai",
            Self::Perplexity => "perplexity",
            Self::Ozel(s) => s.as_str(),
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "gemini" => Self::Gemini,
            "chatgpt" => Self::ChatGpt,
            "mistral" => Self::Mistral,
            "copilot" => Self::Copilot,
            "deepseek" => Self::DeepSeek,
            "qwen" => Self::Qwen,
            "kimi" => Self::Kimi,
            "z.ai" | "zai" => Self::Zai,
            "perplexity" => Self::Perplexity,
            diger => Self::Ozel(diger.to_string()),
        }
    }
}

/// Oturumun anlık sağlık ve kullanılabilirlik durumu
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OturumDurumu {
    /// İstek almaya hazır
    Hazir,
    /// Saatlik veya dakikalık kota doldu, belirli bir zamana kadar beklemede
    KotaliBekleme { acilis_zamani: Instant },
    /// Doğrulama / Cloudflare / İnsan müdahalesi gerekiyor
    InsanKapisi { aciklama: String },
    /// Geçici hata aldı, kısa süreli soğuma periyodunda (cooldown)
    Sogumada { deneme_sayisi: u32, bitis: Instant },
    /// Tamamen devre dışı bırakıldı
    DevreDisi,
}

/// Tekil bir hesap/sekme oturumu
#[derive(Debug, Clone)]
pub struct HesapOturumu {
    /// Tekil oturum tanıtıcısı (örn: "ercan-gemini-1")
    pub id: String,
    /// Bağlı olduğu sağlayıcı
    pub saglayici: Saglayici,
    /// Zen Browser kapsayıcı kimliği (örn: "firefox-container-1", "userContextId=1")
    pub cookie_store_id: String,
    /// WebExtension sekme numarası (tabId)
    pub tab_id: u32,
    /// Oturum durumu
    pub durum: OturumDurumu,
    /// Toplam başarılı istek sayısı
    pub basarili_istek_sayisi: u64,
    /// Son 1 saat içinde yapılan isteklerin zaman damgaları
    pub son_bir_saatteki_istekler: Vec<Instant>,
    /// Saatlik tavan (0 = sınırsız)
    pub saatlik_tavan: u32,
    /// İki istek arasındaki asgari bekleme süresi (hız bütçesi)
    pub asgari_bekleme: Duration,
    /// En son ne zaman istek yapıldı
    pub son_istek_zamani: Option<Instant>,
}

impl HesapOturumu {
    pub fn yeni(
        id: impl Into<String>,
        saglayici: Saglayici,
        cookie_store_id: impl Into<String>,
        tab_id: u32,
    ) -> Self {
        Self {
            id: id.into(),
            saglayici,
            cookie_store_id: cookie_store_id.into(),
            tab_id,
            durum: OturumDurumu::Hazir,
            basarili_istek_sayisi: 0,
            son_bir_saatteki_istekler: Vec::new(),
            saatlik_tavan: 30, // Varsayılan insan benzeri saatlik limit
            asgari_bekleme: Duration::from_millis(3000),
            son_istek_zamani: None,
        }
    }

    /// Bir saatten eski istekleri temizler
    pub fn istekleri_buda(&mut self, simdi: Instant) {
        self.son_bir_saatteki_istekler
            .retain(|t| simdi.duration_since(*t) < Duration::from_secs(3600));
    }

    /// Oturumun şu an istek almaya uygun olup olmadığını kontrol eder
    pub fn musait_mi(&mut self, simdi: Instant) -> bool {
        self.istekleri_buda(simdi);

        match &self.durum {
            OturumDurumu::Hazir => {
                // Saatlik tavan kontrolü
                if self.saatlik_tavan > 0
                    && self.son_bir_saatteki_istekler.len() as u32 >= self.saatlik_tavan
                {
                    return false;
                }
                // Hız bütçesi (pacing) kontrolü
                if let Some(son) = self.son_istek_zamani
                    && simdi.duration_since(son) < self.asgari_bekleme
                {
                    return false;
                }
                true
            }
            OturumDurumu::Sogumada { bitis, .. } => simdi >= *bitis,
            OturumDurumu::KotaliBekleme { acilis_zamani } => simdi >= *acilis_zamani,
            OturumDurumu::InsanKapisi { .. } | OturumDurumu::DevreDisi => false,
        }
    }

    /// İstek başlangıcını kaydeder
    pub fn istek_kaydet(&mut self, simdi: Instant) {
        self.son_istek_zamani = Some(simdi);
        self.son_bir_saatteki_istekler.push(simdi);
        self.basarili_istek_sayisi += 1;
        self.durum = OturumDurumu::Hazir;
    }

    /// Hata durumunda kademeli geri çekilme (exponential backoff / circuit breaker)
    pub fn hata_kaydet(&mut self, simdi: Instant, hata_mesaji: &str) {
        let mevcut_deneme = match self.durum {
            OturumDurumu::Sogumada { deneme_sayisi, .. } => deneme_sayisi + 1,
            _ => 1,
        };

        if hata_mesaji.to_ascii_lowercase().contains("captcha")
            || hata_mesaji.to_ascii_lowercase().contains("verify")
        {
            self.durum = OturumDurumu::InsanKapisi {
                aciklama: hata_mesaji.to_string(),
            };
            return;
        }

        // 5s, 15s, 45s, 120s kademeli soğuma
        let bekleme_sn = match mevcut_deneme {
            1 => 5,
            2 => 15,
            3 => 45,
            _ => 120,
        };

        self.durum = OturumDurumu::Sogumada {
            deneme_sayisi: mevcut_deneme,
            bitis: simdi + Duration::from_secs(bekleme_sn),
        };
    }
}

/// Tüm sağlayıcı ve kapsayıcıları yöneten merkezi Oturum Havuzu
#[derive(Debug, Default)]
pub struct OturumHavuzu {
    /// Oturum kimliği -> HesapOturumu
    pub oturumlar: HashMap<String, HesapOturumu>,
    /// Sağlayıcı bazında yuvarlak masa (round-robin) son seçilen indeks
    pub rotasyon_sayaclari: HashMap<Saglayici, usize>,
}

impl OturumHavuzu {
    pub fn yeni() -> Self {
        Self::default()
    }

    /// Yeni bir oturum kaydeder veya günceller
    pub fn oturum_kaydet(&mut self, oturum: HesapOturumu) {
        self.oturumlar.insert(oturum.id.clone(), oturum);
    }

    /// Sekme ID'sine göre oturumu bulur
    pub fn tab_id_ile_bul(&mut self, tab_id: u32) -> Option<&mut HesapOturumu> {
        self.oturumlar.values_mut().find(|o| o.tab_id == tab_id)
    }

    /// Belirli bir sağlayıcı için en uygun, müsait hesabı seçer.
    /// Eğer ilk hesap meşgul veya kotalıysa, rotasyonla bir sonraki hesaba geçer (OmniRoute fallback mantığı).
    pub fn en_uygun_hesabi_sec(
        &mut self,
        saglayici: &Saglayici,
        simdi: Instant,
    ) -> Option<HesapOturumu> {
        // Bu sağlayıcıya ait tüm oturumları listele
        let mut aday_idleri: Vec<String> = self
            .oturumlar
            .values()
            .filter(|o| o.saglayici == *saglayici)
            .map(|o| o.id.clone())
            .collect();

        if aday_idleri.is_empty() {
            return None;
        }

        aday_idleri.sort();

        // Round-robin rotasyon başlangıç noktası
        let sayac = self
            .rotasyon_sayaclari
            .entry(saglayici.clone())
            .or_insert(0);
        let toplam_aday = aday_idleri.len();

        for i in 0..toplam_aday {
            let indeks = (*sayac + i) % toplam_aday;
            let id = &aday_idleri[indeks];

            if let Some(oturum) = self.oturumlar.get_mut(id)
                && oturum.musait_mi(simdi)
            {
                *sayac = (indeks + 1) % toplam_aday;
                return Some(oturum.clone());
            }
        }

        None
    }

    /// Havuzdaki toplam aktif oturum sayısı
    pub fn toplam_oturum_sayisi(&self) -> usize {
        self.oturumlar.len()
    }

    /// Sağlayıcıya göre hesap sayısı
    pub fn saglayici_hesap_sayisi(&self, saglayici: &Saglayici) -> usize {
        self.oturumlar
            .values()
            .filter(|o| o.saglayici == *saglayici)
            .count()
    }

    /// Havuzun anlık durum raporu
    pub fn durum_raporu(&mut self, simdi: Instant) -> Vec<HesapDurumOzeti> {
        self.oturumlar
            .values_mut()
            .map(|o| {
                o.istekleri_buda(simdi);
                HesapDurumOzeti {
                    id: o.id.clone(),
                    saglayici: o.saglayici.as_str().to_string(),
                    cookie_store_id: o.cookie_store_id.clone(),
                    tab_id: o.tab_id,
                    musait: o.musait_mi(simdi),
                    saatlik_kullanim: o.son_bir_saatteki_istekler.len() as u32,
                    saatlik_tavan: o.saatlik_tavan,
                    toplam_basarili: o.basarili_istek_sayisi,
                }
            })
            .collect()
    }
}

/// Arayüz veya loglama için hafif durum özeti
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HesapDurumOzeti {
    pub id: String,
    pub saglayici: String,
    pub cookie_store_id: String,
    pub tab_id: u32,
    pub musait: bool,
    pub saatlik_kullanim: u32,
    pub saatlik_tavan: u32,
    pub toplam_basarili: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oturum_rotasyonu_ve_fallback() {
        let mut havuz = OturumHavuzu::yeni();
        let simdi = Instant::now();

        // 3 adet Gemini hesabı ekle (farklı Zen kapsayıcılarında)
        let mut h1 = HesapOturumu::yeni("gemini-hesap-1", Saglayici::Gemini, "container-1", 101);
        h1.saatlik_tavan = 1; // 1 istek sonra dolsun
        havuz.oturum_kaydet(h1);

        let h2 = HesapOturumu::yeni("gemini-hesap-2", Saglayici::Gemini, "container-2", 102);
        havuz.oturum_kaydet(h2);

        let h3 = HesapOturumu::yeni("gemini-hesap-3", Saglayici::Gemini, "container-3", 103);
        havuz.oturum_kaydet(h3);

        assert_eq!(havuz.saglayici_hesap_sayisi(&Saglayici::Gemini), 3);

        // İlk seçim: hesap-1 gelmeli
        let secilen1 = havuz
            .en_uygun_hesabi_sec(&Saglayici::Gemini, simdi)
            .unwrap();
        assert_eq!(secilen1.id, "gemini-hesap-1");

        // Hesap-1'e istek kaydet (artık tavanı doldu)
        if let Some(o) = havuz.oturumlar.get_mut("gemini-hesap-1") {
            o.istek_kaydet(simdi);
        }

        // İkinci seçim: hesap-1 dolu olduğu için otomatik hesap-2'ye geçmeli (Fallback)
        let secilen2 = havuz
            .en_uygun_hesabi_sec(&Saglayici::Gemini, simdi)
            .unwrap();
        assert_eq!(secilen2.id, "gemini-hesap-2");

        // Üçüncü seçim: hesap-3 gelmeli (Round-robin)
        let secilen3 = havuz
            .en_uygun_hesabi_sec(&Saglayici::Gemini, simdi)
            .unwrap();
        assert_eq!(secilen3.id, "gemini-hesap-3");
    }

    #[test]
    fn test_hata_sonrasi_soguma_cooldown() {
        let mut o = HesapOturumu::yeni("mistral-1", Saglayici::Mistral, "container-default", 201);
        let simdi = Instant::now();

        assert!(o.musait_mi(simdi));

        // Hata aldır
        o.hata_kaydet(simdi, "429 Too Many Requests");

        // Hemen ardından müsait olmamalı
        assert!(!o.musait_mi(simdi));

        // 6 saniye sonra soğuma bitmiş olmalı
        let alti_sn_sonra = simdi + Duration::from_secs(6);
        assert!(o.musait_mi(alti_sn_sonra));
    }
}
