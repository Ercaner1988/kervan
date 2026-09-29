//! Task-Aware Smart Router (Görev Farkındalıklı Akıllı Yönlendirici)
//!
//! OmniRoute `taskAwareRouter.ts` ve `taskAwareRouting.ts` mimarisinden
//! saf Rust ile uyarlanmıştır.
//!
//! Kullanıcının gönderdiği istemi semantik olarak sınıflandırarak (kodlama,
//! derin akıl yürütme, akademik metin, görsel/video, çeviri vb.) otomatik
//! olarak en uygun sağlayıcı ve modele yönlendirir.

use std::collections::HashMap;

/// Tespit edilen görev türleri.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GorevTuru {
    Kodlama,
    DerinDusunme,
    AkademikYazim,
    GorselUretim,
    VideoUretim,
    HizliOzet,
    Ceviri,
    GenelSohbet,
}

impl GorevTuru {
    pub fn etiket(&self) -> &'static str {
        match self {
            Self::Kodlama => "kodlama",
            Self::DerinDusunme => "derin_dusunme",
            Self::AkademikYazim => "akademik_yazim",
            Self::GorselUretim => "gorsel_uretim",
            Self::VideoUretim => "video_uretim",
            Self::HizliOzet => "hizli_ozet",
            Self::Ceviri => "ceviri",
            Self::GenelSohbet => "genel_sohbet",
        }
    }
}

/// Yönlendirme hedefi: Sağlayıcı ve Model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RotaHedefi {
    pub saglayici: String,
    pub model: String,
    pub aciklama: String,
}

/// Görev bazlı yönlendirme yapılandırması.
#[derive(Debug, Clone)]
pub struct GorevRotaYapilandirmasi {
    pub aktif: bool,
    ozel_eslesmeler: HashMap<GorevTuru, RotaHedefi>,
}

impl Default for GorevRotaYapilandirmasi {
    fn default() -> Self {
        let mut ozel = HashMap::new();
        ozel.insert(
            GorevTuru::Kodlama,
            RotaHedefi {
                saglayici: "deepseek".to_string(),
                model: "deepseek-coder".to_string(),
                aciklama: "Kod ve yazılım mimarisi için DeepSeek Coder".to_string(),
            },
        );
        ozel.insert(
            GorevTuru::DerinDusunme,
            RotaHedefi {
                saglayici: "deepseek".to_string(),
                model: "deepseek-r1".to_string(),
                aciklama: "Karmaşık mantık, matematik ve CoT akıl yürütme için DeepSeek-R1".to_string(),
            },
        );
        ozel.insert(
            GorevTuru::AkademikYazim,
            RotaHedefi {
                saglayici: "gemini".to_string(),
                model: "gemini-3.1-pro".to_string(),
                aciklama: "Geniş bağlam ve derin akademik inceleme için Gemini 3.1 Pro".to_string(),
            },
        );
        ozel.insert(
            GorevTuru::GorselUretim,
            RotaHedefi {
                saglayici: "gemini".to_string(),
                model: "imagen-3".to_string(),
                aciklama: "Yüksek çözünürlüklü görsel üretimi için Imagen 3".to_string(),
            },
        );
        ozel.insert(
            GorevTuru::VideoUretim,
            RotaHedefi {
                saglayici: "gemini".to_string(),
                model: "veo-2".to_string(),
                aciklama: "Sinematik video oluşturma için Veo 2".to_string(),
            },
        );
        ozel.insert(
            GorevTuru::HizliOzet,
            RotaHedefi {
                saglayici: "gemini".to_string(),
                model: "gemini-3.8-flash".to_string(),
                aciklama: "Hızlı özetleme ve düşük gecikme için Gemini 3.8 Flash".to_string(),
            },
        );
        ozel.insert(
            GorevTuru::Ceviri,
            RotaHedefi {
                saglayici: "mistral".to_string(),
                model: "mistral-large-2".to_string(),
                aciklama: "Yüksek kaliteli dil ve çeviri yeteneği için Mistral Large 2".to_string(),
            },
        );
        ozel.insert(
            GorevTuru::GenelSohbet,
            RotaHedefi {
                saglayici: "gemini".to_string(),
                model: "gemini-3.8-flash".to_string(),
                aciklama: "Genel sohbet için varsayılan Gemini 3.8 Flash".to_string(),
            },
        );

        Self {
            aktif: true,
            ozel_eslesmeler: ozel,
        }
    }
}

/// İstem metnini inceleyen sınıflandırıcı.
pub struct GorevSiniflandirici;

impl GorevSiniflandirici {
    /// İstem metnini analiz ederek en yüksek olasılıklı görev türünü belirler.
    pub fn siniflandir(istem: &str) -> GorevTuru {
        let kucuk = istem.to_lowercase();

        // 1. Görsel & Video Üretimi (Yüksek öncelikli spesifik eylemler)
        if kucuk.contains("video oluştur")
            || kucuk.contains("video üret")
            || kucuk.contains("generate video")
            || kucuk.contains("veo-2")
            || kucuk.contains("animasyon yap")
        {
            return GorevTuru::VideoUretim;
        }

        if kucuk.contains("görsel üret")
            || kucuk.contains("resim çiz")
            || kucuk.contains("generate image")
            || kucuk.contains("imagen-3")
            || kucuk.contains("dall-e")
            || kucuk.contains("illüstrasyon")
        {
            return GorevTuru::GorselUretim;
        }

        // 2. Kodlama & Yazılım Geliştirme Kalıpları
        let kodlama_puani = Self::puanla(&kucuk, &[
            "kod", "fn ", "def ", "function", "class ", "struct ", "impl ", "import ",
            "const ", "let mut", "pub fn", "rust", "python", "javascript", "typescript",
            "```", "bug ", "hata", "düzelt", "refactor", "unit test", "derleme",
            "cargo ", "npm ", "compiler error", "syntax error", "sql query",
            "endpoint", "api route", "git commit", "yazılım"
        ]);

        // 3. Derin Akıl Yürütme & Mantık Kalıpları
        let dusunme_puani = Self::puanla(&kucuk, &[
            "adım adım düşün", "kanıtla", "mantıksal çıkarım", "akıl yürüt",
            "ispatla", "felsefi argüman", "matematiksel model", "neden-sonuç",
            "diyalektik", "analiz et", "eleştirel bakış", "teoremi", "derin düşün",
            "r1", "thinking", "cot", "reasoning"
        ]);

        // 4. Akademik Yazım & Tez Kalıpları
        let akademik_puani = Self::puanla(&kucuk, &[
            "tez ", "literatür taraması", "akademik kaynak", "dipnot", "atıf",
            "kaynakça", "makale özeti", "weber", "sosyolojik", "metodoloji",
            "araştırma yöntemi", "hipotez", "epistemoloji", "ontoloji", "akademik üslup"
        ]);

        // 5. Çeviri Kalıpları
        let ceviri_puani = Self::puanla(&kucuk, &[
            "türkçeye çevir", "ingilizceye çevir", "tercüme et", "translate to",
            "çevirisini yap", "anlamı nedir", "osmanlıca", "arapça", "fransızca"
        ]);

        // 6. Hızlı Özetleme Kalıpları
        let ozet_puani = Self::puanla(&kucuk, &[
            "kısaca özetle", "özet çıkar", "maddeler halinde özetle", "tl;dr",
            "kısa yanıt ver", "hızlıca açıkla", "özetle:"
        ]);

        // Puanları karşılaştır ve en yükseğe karar ver
        let mut puanlar = vec![
            (GorevTuru::Kodlama, kodlama_puani),
            (GorevTuru::DerinDusunme, dusunme_puani),
            (GorevTuru::AkademikYazim, akademik_puani),
            (GorevTuru::Ceviri, ceviri_puani),
            (GorevTuru::HizliOzet, ozet_puani),
        ];

        puanlar.sort_by(|a, b| b.1.cmp(&a.1));

        if let Some((gorev, puan)) = puanlar.first() {
            if *puan > 0 {
                return *gorev;
            }
        }

        GorevTuru::GenelSohbet
    }

    fn puanla(metin: &str, anahtarlar: &[&str]) -> usize {
        let mut toplam = 0;
        for anahtar in anahtarlar {
            if metin.contains(anahtar) {
                toplam += 1;
            }
        }
        toplam
    }
}

/// Akıllı Rota Yöneticisi.
pub struct AkilliRota {
    yapilandirma: GorevRotaYapilandirmasi,
}

impl AkilliRota {
    pub fn yeni(yapilandirma: GorevRotaYapilandirmasi) -> Self {
        Self { yapilandirma }
    }

    /// Eğer kullanıcı `auto` / `otomatik` seçtiyse veya boş bıraktıysa akıllı rota hesaplar.
    /// Kullanıcı belirli bir sağlayıcı ve model seçtiyse (örn. `deepseek`, `deepseek-r1`) seçimi korur.
    pub fn cozumle(&self, talep_saglayici: &str, talep_model: &str, istem: &str) -> RotaHedefi {
        let otomatik_mi = talep_saglayici.is_empty()
            || talep_saglayici == "auto"
            || talep_saglayici == "otomatik";

        if !otomatik_mi && !talep_model.is_empty() && talep_model != "auto" && talep_model != "otomatik" {
            return RotaHedefi {
                saglayici: talep_saglayici.to_string(),
                model: talep_model.to_string(),
                aciklama: "Kullanıcı tercihi korundu".to_string(),
            };
        }

        if !self.yapilandirma.aktif {
            return RotaHedefi {
                saglayici: "gemini".to_string(),
                model: "gemini-3.8-flash".to_string(),
                aciklama: "Varsayılan sağlayıcı (Rota kapalı)".to_string(),
            };
        }

        let gorev = GorevSiniflandirici::siniflandir(istem);
        self.yapilandirma
            .ozel_eslesmeler
            .get(&gorev)
            .cloned()
            .unwrap_or_else(|| RotaHedefi {
                saglayici: "gemini".to_string(),
                model: "gemini-3.8-flash".to_string(),
                aciklama: "Genel rota".to_string(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kodlama_gorevi_tespiti() {
        let istem = "Lütfen Rust dilinde pub fn hesapla fonksiyonunu ve unit testlerini yaz.";
        let gorev = GorevSiniflandirici::siniflandir(istem);
        assert_eq!(gorev, GorevTuru::Kodlama);
    }

    #[test]
    fn test_derin_dusunme_gorevi_tespiti() {
        let istem = "Bu felsefi argümanı adım adım düşünerek mantıksal çıkarım ile kanıtla.";
        let gorev = GorevSiniflandirici::siniflandir(istem);
        assert_eq!(gorev, GorevTuru::DerinDusunme);
    }

    #[test]
    fn test_akademik_yazim_tespiti() {
        let istem = "Weber'in sosyolojik teorisi üzerine hazırladığım tez için literatür taraması ve kaynakça öner.";
        let gorev = GorevSiniflandirici::siniflandir(istem);
        assert_eq!(gorev, GorevTuru::AkademikYazim);
    }

    #[test]
    fn test_gorsel_ve_video_tespiti() {
        let istem_gorsel = "Bana geleceğin kütüphanesini fotogerçekçi bir görsel üret.";
        assert_eq!(GorevSiniflandirici::siniflandir(istem_gorsel), GorevTuru::GorselUretim);

        let istem_video = "Bu sahne için sinematik kalitede akıcı bir video oluştur.";
        assert_eq!(GorevSiniflandirici::siniflandir(istem_video), GorevTuru::VideoUretim);
    }

    #[test]
    fn test_akilli_rota_cozumleme_otomatik() {
        let rota = AkilliRota::yeni(GorevRotaYapilandirmasi::default());
        let hedef = rota.cozumle("auto", "auto", "Rust kodundaki derleme hatasını düzelt");
        assert_eq!(hedef.saglayici, "deepseek");
        assert_eq!(hedef.model, "deepseek-coder");
    }

    #[test]
    fn test_akilli_rota_kullanici_secimini_ezmez() {
        let rota = AkilliRota::yeni(GorevRotaYapilandirmasi::default());
        let hedef = rota.cozumle("copilot", "copilot-pro-creative", "Rust kodundaki hatayı düzelt");
        assert_eq!(hedef.saglayici, "copilot");
        assert_eq!(hedef.model, "copilot-pro-creative");
        assert_eq!(hedef.aciklama, "Kullanıcı tercihi korundu");
    }
}
